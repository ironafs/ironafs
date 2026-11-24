// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_main]

//! Reads arbitrary bytes as what one route-socket receive returned, and
//! checks it with the netlink sender check of ADR 0005, sheet 23 (point
//! 6.5: short addresses, a wrong family), under sheet 23's Linux rules or
//! rules the input picks within their invariants. It asserts: the check
//! returns; the sender's port is read only after the address's length and
//! family have passed, and then exactly once; `Ok` comes exactly for an
//! address of the rules' length and family, port 0, no truncation flag
//! and a count from 1 to the buffer's size, and carries that count; and
//! the same input gives the same result. The error precedence is the unit
//! tests'.

use std::cell::Cell;
use std::mem::{Discriminant, discriminant};

use libfuzzer_sys::fuzz_target;

/// The checks, from the crate's own file: it uses the standard library
/// alone and holds no `unsafe`, so the target checks the code that ships.
#[path = "../../crates/ironafs-os/src/route.rs"]
// The workspace's `cargo fmt` formats the file; formatting it from here
// would follow its unit-test module to a path relative to this target.
#[rustfmt::skip]
#[expect(
    dead_code,
    unreachable_pub,
    reason = "the target drives the sender check; the send's, the wait's and the wake's \
              helpers are the site's, and the outcome and error types are public there"
)]
mod route;

use route::{LINUX, Received, RouteError, Rules, checked};

/// The input, read front to back; past its end every byte reads as zero.
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let mut out = [0; N];
        let (head, rest) = self.0.split_at(self.0.len().min(N));
        for (slot, byte) in out.iter_mut().zip(head) {
            *slot = *byte;
        }
        self.0 = rest;
        out
    }

    fn byte(&mut self) -> u8 {
        let [byte] = self.take();
        byte
    }

    fn short(&mut self) -> u16 {
        u16::from_le_bytes(self.take())
    }

    fn int(&mut self) -> i32 {
        i32::from_le_bytes(self.take())
    }

    fn word(&mut self) -> u32 {
        u32::from_le_bytes(self.take())
    }

    /// A length: any `u64`, the largest kept where `usize` is narrower.
    fn length(&mut self) -> usize {
        usize::try_from(u64::from_le_bytes(self.take())).unwrap_or(usize::MAX)
    }
}

/// The rules: sheet 23's Linux values if the first byte is even, else
/// rules the input picks within their invariants (a one-bit truncation
/// flag; any family and address length).
fn rules(bytes: &mut Bytes<'_>) -> Rules {
    if bytes.byte() % 2 == 0 {
        return LINUX;
    }
    Rules {
        family: bytes.short(),
        name_length: bytes.length(),
        truncated: 1_i32 << (bytes.byte() % 32),
        ..LINUX
    }
}

/// What the receive returned: short addresses and wrong families are the
/// input's to pick, as every other field.
fn received(bytes: &mut Bytes<'_>) -> Received {
    Received {
        count: bytes.length(),
        capacity: bytes.length(),
        flags: bytes.int(),
        name_length: bytes.length(),
        family: bytes.short(),
    }
}

/// One run's outcome: the length or the error's kind.
type Outcome = Result<usize, Discriminant<RouteError>>;

/// Checks one input once.
fn check(input: &[u8]) -> Outcome {
    let mut bytes = Bytes(input);
    let rules = rules(&mut bytes);
    let received = received(&mut bytes);
    let port = bytes.word();
    let address = received.name_length == rules.name_length && received.family == rules.family;
    let expected_ok = address
        && port == 0
        && received.flags & rules.truncated == 0
        && received.count != 0
        && received.count <= received.capacity;

    let reads = Cell::new(0_u32);
    let result = checked(rules, &received, || {
        reads.set(reads.get() + 1);
        port
    });
    assert!(
        reads.get() == u32::from(address),
        "the port was read before the address passed, or more than once"
    );
    assert!(
        result.is_ok() == expected_ok,
        "Ok for other than the kernel's whole, non-empty datagram, or an error for exactly that"
    );
    if let Ok(count) = &result {
        assert!(*count == received.count, "Ok carried another length");
    }
    if !address {
        assert!(
            matches!(result, Err(RouteError::BadSender)),
            "an address of another length or family was not refused as no sender"
        );
    }
    result.map_err(|error| discriminant(&error))
}

fuzz_target!(|input: &[u8]| {
    let first = check(input);
    let second = check(input);
    assert!(first == second, "the same input gave another result");
});
