// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_main]

//! Reads arbitrary bytes as what one wait on a route socket and its wake
//! returned (the count and the two `revents` values), and judges it with
//! the validator of ADR 0005, sheet 23's wait (point 6.5: invalid bits, a
//! count that disagrees with the entries, both entries ready), under
//! sheet 23's Linux bits or bits the input picks within their invariants.
//! It asserts: the validator returns; `Ok` comes exactly when the count is
//! the number of entries reported, at least 1, no `POLLNVAL` is on either
//! entry, the route socket holds only `POLLIN`, `POLLERR` and `POLLHUP`,
//! and the wake only `POLLIN`; each report is each bit as it came; an error
//! carries the values given; and the same input gives the same result.

use libfuzzer_sys::fuzz_target;

/// The validator, from the crate's own file: it uses the standard library
/// alone and holds no `unsafe`, so the target checks the code that ships.
#[path = "../../crates/ironafs-os/src/route.rs"]
// The workspace's `cargo fmt` formats the file; formatting it from here
// would follow its unit-test module to a path relative to this target.
#[rustfmt::skip]
#[expect(
    dead_code,
    unreachable_pub,
    reason = "the target drives the wait's validator; the send's, the receive's and the wake's \
              helpers are the site's, and the outcome and error types are public there"
)]
mod route;

use route::{PollRules, Ready, RouteError, judged};

/// Sheet 23's Linux values.
const LINUX: PollRules = PollRules {
    readable: 1,
    error: 8,
    hangup: 16,
    invalid: 32,
};

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

    fn short(&mut self) -> i16 {
        i16::from_le_bytes(self.take())
    }

    fn int(&mut self) -> i32 {
        i32::from_le_bytes(self.take())
    }
}

/// The bits: sheet 23's Linux values if the first byte is even, else four
/// distinct single bits the input picks.
fn rules(bytes: &mut Bytes<'_>) -> PollRules {
    if bytes.byte() % 2 == 0 {
        return LINUX;
    }
    let bit = |shift: u8| 1_i16 << (shift % 16);
    let picked = PollRules {
        readable: bit(bytes.byte()),
        error: bit(bytes.byte()),
        hangup: bit(bytes.byte()),
        invalid: bit(bytes.byte()),
    };
    let all = [picked.readable, picked.error, picked.hangup, picked.invalid];
    if all.iter().fold(0, |sum, bit| sum | bit).count_ones() == 4 {
        picked
    } else {
        LINUX
    }
}

/// Judges one input once.
fn check(input: &[u8]) -> Result<Ready, (i32, i16, i16)> {
    let mut bytes = Bytes(input);
    let rules = rules(&mut bytes);
    let count = bytes.int();
    let socket = bytes.short();
    let wake = bytes.short();
    let entries = i32::from(socket != 0) + i32::from(wake != 0);
    let named = rules.readable | rules.error | rules.hangup;
    let expected_ok = count >= 1
        && count == entries
        && socket & rules.invalid == 0
        && wake & rules.invalid == 0
        && socket & !named == 0
        && wake & !rules.readable == 0;

    let result = judged(rules, count, socket, wake);
    assert!(
        result.is_ok() == expected_ok,
        "a report sheet 23 refuses was accepted, or one it allows refused"
    );
    match result {
        Ok(ready) => {
            assert!(
                ready
                    == Ready {
                        readable: socket & rules.readable != 0,
                        error: socket & rules.error != 0,
                        hangup: socket & rules.hangup != 0,
                        woken: wake & rules.readable != 0,
                    },
                "a report was merged, dropped or invented"
            );
            Ok(ready)
        }
        Err(error) => {
            let carried = match error {
                RouteError::Readiness {
                    count: c,
                    socket: s,
                    wake: w,
                } => Some((c, s, w)),
                _ => None,
            };
            assert!(
                carried == Some((count, socket, wake)),
                "the validator returned another error, or one that carried other values"
            );
            Err((count, socket, wake))
        }
    }
}

fuzz_target!(|input: &[u8]| {
    let first = check(input);
    let second = check(input);
    assert!(first == second, "the same input gave another result");
});
