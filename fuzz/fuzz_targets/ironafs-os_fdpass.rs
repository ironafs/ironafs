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

/// The tests' stand-in descriptor and adopter, from their own file.
#[path = "../../crates/ironafs-os/src/fdpass/tests/held.rs"]
mod held;
#[path = "../../crates/ironafs-os/src/fdpass/tests/invariants.rs"]
mod invariants;

fuzz_target!(|input: &[u8]| {
    invariants::check_input(input);
});
