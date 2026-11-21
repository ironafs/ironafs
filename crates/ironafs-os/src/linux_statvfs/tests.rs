// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of `linux-statvfs` (ADR 0005, sheet 22) that Miri runs too,
//! through the call's stand-in: the query's pointer and arithmetic. The
//! arithmetic's own cases are `space`'s; the tests that need the running
//! kernel (real filesystems, the directory check) are in `kernel`.

use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;

use super::measure;

#[cfg(not(miri))]
mod kernel;
#[cfg(miri)]
mod stand_in;

#[test]
fn measure_queries_the_descriptor_s_filesystem_through_the_call() {
    // Under Miri, which has no filesystem query, the stand-in reports 1000
    // free blocks of 4096 bytes; on the kernel a socket's filesystem
    // answers too.
    let (socket, _peer) = UnixStream::pair().unwrap();
    let free = measure(socket.as_fd()).unwrap();
    if cfg!(miri) {
        assert_eq!(free, 4_096_000);
    }
}
