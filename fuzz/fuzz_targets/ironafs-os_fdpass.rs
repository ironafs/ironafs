// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_main]

//! Reads arbitrary bytes as what one receive returned, and checks it with
//! the ancillary-data checks of ADR 0005, sheet 14 (point 6.5: bad
//! `msg_controllen` and `cmsg_len`, wrong level and type), under sheet
//! 14's Linux rules or rules the input picks within their invariants. It
//! asserts: the checks return; `adopt` is called only with non-negative
//! integers, and only for a message whose control length and header pass
//! every check, with exactly the non-negative integers that header
//! delivers; every adopted integer is handed out in `Ok` or dropped; `Ok`
//! comes exactly for one `SCM_RIGHTS` descriptor with the right data byte,
//! no truncation and no end of stream; and the same input gives the same
//! result. Adoption makes a stand-in that records its drop: no integer is
//! ever made a descriptor. The error precedence is the unit tests'.

use std::cell::RefCell;
use std::mem::{Discriminant, discriminant};
use std::rc::Rc;

use libfuzzer_sys::fuzz_target;

/// The checks, from the crate's own file: it uses the standard library
/// alone and holds no `unsafe`, so the target checks the code that ships.
#[path = "../../crates/ironafs-os/src/fdpass.rs"]
// The workspace's `cargo fmt` formats the file; formatting it from here
// would follow its unit-test module to a path relative to this target.
#[rustfmt::skip]
#[expect(
    dead_code,
    unreachable_pub,
    reason = "the target drives the receive checks; the send's and the retry's helpers, and \
              the send and receive errors, are the site's, and the error type is public there"
)]
mod fdpass;

use fdpass::{FdPassError, LINUX, Received, Rules, accept};

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

    fn int(&mut self) -> i32 {
        i32::from_le_bytes(self.take())
    }

    /// A length: any `u64`, the largest kept where `usize` is narrower.
    fn length(&mut self) -> usize {
        usize::try_from(u64::from_le_bytes(self.take())).unwrap_or(usize::MAX)
    }
}

/// The rules: sheet 14's Linux values if the first byte is even, else
/// rules the input picks within their invariants (a one-bit truncation
/// flag, distinct rights and pidfd types, one descriptor's length above
/// zero and below two's, two's within the buffer).
fn rules(bytes: &mut Bytes<'_>) -> Rules {
    if bytes.byte() % 2 == 0 {
        return LINUX;
    }
    let level = bytes.int();
    let rights = bytes.int();
    let pidfd = bytes.int();
    let truncated = 1_i32 << (bytes.byte() % 32);
    let one = 1 + usize::from(bytes.byte());
    let two = one + 1 + usize::from(bytes.byte());
    let capacity = two + usize::from(bytes.byte());
    let data = bytes.byte();
    if rights == pidfd {
        return LINUX;
    }
    Rules {
        level,
        rights,
        pidfd,
        truncated,
        one,
        two,
        capacity,
        data,
    }
}

/// What the receive returned.
fn received(bytes: &mut Bytes<'_>) -> Received {
    Received {
        count: bytes.length(),
        flags: bytes.int(),
        control: bytes.length(),
        length: bytes.length(),
        level: bytes.int(),
        kind: bytes.int(),
        payload: [bytes.int(), bytes.int()],
        byte: bytes.byte(),
    }
}

/// How many integers the header delivers, by sheet 14 stated afresh: only
/// if the control length is at least one descriptor's and at most the
/// buffer, the header's length is within it, its level is the socket
/// level, and it is `SCM_RIGHTS` of one or two descriptors or `SCM_PIDFD`
/// of one. The second value says it is `SCM_RIGHTS`.
fn delivers(rules: &Rules, received: &Received) -> Option<(usize, bool)> {
    let header = (rules.one..=rules.capacity).contains(&received.control)
        && received.length <= received.control
        && received.level == rules.level;
    if !header {
        None
    } else if received.kind == rules.rights && received.length == rules.one {
        Some((1, true))
    } else if received.kind == rules.rights && received.length == rules.two {
        Some((2, true))
    } else if received.kind == rules.pidfd && received.length == rules.one {
        Some((1, false))
    } else {
        None
    }
}

/// The tests' stand-in descriptor and adopter, from their own file.
#[path = "../../crates/ironafs-os/src/fdpass/tests/held.rs"]
mod held;

use held::{Log, adopter};

/// One run's outcome: the integer handed out or the error's kind, and the
/// log once the result is dropped.
type Outcome = (Result<i32, Discriminant<FdPassError>>, Log);

/// Checks one input once.
fn check(input: &[u8]) -> Outcome {
    let mut bytes = Bytes(input);
    let rules = rules(&mut bytes);
    let received = received(&mut bytes);
    let delivered = delivers(&rules, &received);
    let payload = received.payload;
    let expected_ok = received.count != 0
        && received.flags & rules.truncated == 0
        && delivered == Some((1, true))
        && payload[0] >= 0
        && received.byte == rules.data;

    let log = Rc::new(RefCell::new(Log::default()));
    let result = accept(&rules, received, adopter(&log));
    let handed = result.as_ref().ok().map(|held| held.0);
    {
        let log = log.borrow();
        assert!(
            log.adopted.iter().all(|value| *value >= 0),
            "a negative integer was adopted"
        );
        let wanted: Vec<i32> = delivered
            .map(|(number, _)| payload.iter().take(number))
            .into_iter()
            .flatten()
            .copied()
            .filter(|value| *value >= 0)
            .collect();
        assert!(
            log.adopted == wanted,
            "the adopted integers are not the non-negative ones a valid header delivered"
        );
        // Before the result is dropped, every adopted integer but the one
        // handed out has been dropped.
        let mut kept = log.adopted.clone();
        if let Some(value) = handed
            && let Some(at) = kept.iter().position(|kept| *kept == value)
        {
            kept.remove(at);
        }
        let mut dropped = log.dropped.clone();
        kept.sort_unstable();
        dropped.sort_unstable();
        assert!(
            kept == dropped,
            "an adopted integer was neither handed out nor dropped"
        );
    }
    assert!(
        result.is_ok() == expected_ok,
        "Ok for other than one SCM_RIGHTS descriptor, the data byte, no truncation and data, \
         or an error for exactly that"
    );
    if let Some(value) = handed {
        assert!(value == payload[0], "Ok handed out another integer");
    }
    let outcome = result
        .map(|held| held.0)
        .map_err(|error| discriminant(&error));
    assert!(
        Rc::strong_count(&log) == 1,
        "a stand-in outlived the result"
    );
    let log = log.take();
    let mut adopted = log.adopted.clone();
    let mut dropped = log.dropped.clone();
    adopted.sort_unstable();
    dropped.sort_unstable();
    assert!(adopted == dropped, "an adopted integer was never dropped");
    (outcome, log)
}

fuzz_target!(|input: &[u8]| {
    let first = check(input);
    let second = check(input);
    assert!(first == second, "the same input gave another result");
});
