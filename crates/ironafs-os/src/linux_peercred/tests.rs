// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of `linux-peercred` (ADR 0005, sheet 13) that Miri runs too,
//! through the call's stand-in: a socket pair in one process names this
//! process. The length checks are `peer`'s. The tests that need the running
//! kernel (another uid, miswrapped sockets) are in `kernel`; the failure
//! paths, through the stand-in, in `stand_in`.

use std::os::unix::net::UnixStream;

use super::{UCRED_LENGTH, peer_credentials};

#[cfg(not(miri))]
mod kernel;
#[cfg(miri)]
mod stand_in;

#[test]
fn peer_credentials_of_a_socket_pair_name_this_process() {
    let (ours, theirs) = UnixStream::pair().unwrap();
    let pid = i32::try_from(std::process::id()).unwrap();
    assert_eq!(peer_credentials(&ours).unwrap().pid(), pid);
    assert_eq!(peer_credentials(&theirs).unwrap().pid(), pid);
}

#[test]
fn the_length_passed_is_struct_ucred_s_size() {
    assert_eq!(UCRED_LENGTH, 12);
}
