// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The retry of a system call a signal interrupted (`EINTR`), shared by the
//! sites whose calls are made again when interrupted: descriptor passing
//! (sheet 14), route-netlink sockets (sheet 23) and the free-space query
//! (sheet 22). It holds no site: it is compiled on every target its
//! callers are, glibc's and musl's alike, and in the tests.

use std::io;

#[cfg(test)]
mod tests;

/// Makes `call` until it is not interrupted by a signal (`EINTR`).
///
/// # Errors
///
/// The first error that is not an interruption.
pub(crate) fn retried<T>(mut call: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    loop {
        match call() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            result => return result,
        }
    }
}
