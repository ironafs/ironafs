// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Site `linux-peercred` (ADR 0005, row 13 and sheet 13): a Unix stream
//! peer's credentials, through `getsockopt(SOL_SOCKET, SO_PEERCRED)`.
//!
//! It takes a `UnixStream`, but safe Rust can wrap any descriptor as one,
//! so before it reads the credentials it checks what it was given:
//!
//! - the peer's address, through the standard library, which a listener,
//!   an unconnected socket and a socket of another family do not have;
//! - the socket's type, with `getsockopt(SOL_SOCKET, SO_TYPE)`, which must
//!   be `SOCK_STREAM`. A datagram socket pair has credentials cached when
//!   the pair was made, and a datagram socket of a pair can be connected to
//!   another peer without the kernel replacing them, so without this check
//!   the pair's original ids would be reported as the new peer's;
//! - and it refuses the ids -1 the kernel reports for a socket with no
//!   peer credentials.
//!
//! unix(7) gives credentials for connected `AF_UNIX` stream sockets. The
//! values are those the kernel recorded at `connect`, `listen` or
//! `socketpair` (see [`PeerCredentials`]). Which peer may do what (who may
//! send a token, ADR 0004, C43, point 4) is the caller's.

mod abi;

use core::ffi::{c_int, c_void};
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::os::unix::net::UnixStream;

use crate::peer::{PeerCredentials, PeerError, Read, checked};
use abi::{SO_PEERCRED, SO_TYPE, SOCK_STREAM, SOL_SOCKET, socklen_t, ucred};

#[cfg(test)]
mod tests;

/// `struct ucred`'s size, the only length a read may report.
const UCRED_SIZE: usize = size_of::<ucred>();

/// [`UCRED_SIZE`] as the call's length type (12 fits it).
const UCRED_LENGTH: socklen_t = 12;

const _: () = assert!(UCRED_SIZE == 12 && UCRED_LENGTH as usize == UCRED_SIZE);

/// An `int`'s size, the only length a type read may report.
const INT_SIZE: usize = size_of::<c_int>();

/// [`INT_SIZE`] as the call's length type (4 fits it).
const INT_LENGTH: socklen_t = 4;

const _: () = assert!(INT_SIZE == 4 && INT_LENGTH as usize == INT_SIZE);

/// The credentials the kernel recorded for `stream`'s peer when the
/// connection was made.
///
/// # Errors
///
/// [`PeerError::NotConnected`] if the socket has no connected peer (a
/// listener or an unconnected socket wrapped as a stream),
/// [`PeerError::NotStream`] if it is not a stream socket (a datagram
/// socket wrapped as a stream, a pair's included), [`PeerError::Failed`]
/// if a call fails, with its errno, [`PeerError::Length`] if one reports a
/// length other than its value's size, and [`PeerError::NoCredentials`] if
/// the kernel holds none for the peer.
///
/// ```
/// let (ours, theirs) = std::os::unix::net::UnixStream::pair()?;
/// let peer = ironafs_os::peer_credentials(&ours)?;
/// assert_eq!(peer.pid(), i32::try_from(std::process::id())?);
/// # drop(theirs);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn peer_credentials(stream: &UnixStream) -> Result<PeerCredentials, PeerError> {
    // A `UnixStream` can be made from any descriptor in safe Rust. A
    // listener, or an unconnected socket, has no peer address; for a
    // listener `SO_PEERCRED` would answer with the listener's own
    // credentials. The standard library's `getpeername` (`abi::connected`)
    // also refuses a socket that is not a Unix one.
    abi::connected(stream).map_err(PeerError::NotConnected)?;
    stream_type(stream.as_fd())?;
    let mut credentials = ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length: socklen_t = UCRED_LENGTH;
    let result = read(stream.as_fd(), &mut credentials, &mut length);
    checked(Read {
        result,
        returned: usize::try_from(length).unwrap_or(usize::MAX),
        expected: UCRED_SIZE,
        pid: credentials.pid,
        uid: credentials.uid,
        gid: credentials.gid,
    })
}

/// Checks that `socket` is a stream socket: one `getsockopt(SO_TYPE)`
/// call into a local `int`, which must succeed, report an `int`'s length
/// and read `SOCK_STREAM`.
fn stream_type(socket: BorrowedFd<'_>) -> Result<(), PeerError> {
    let descriptor: c_int = socket.as_raw_fd();
    let level: c_int = SOL_SOCKET;
    let option: c_int = SO_TYPE;
    let mut kind: c_int = 0;
    let mut length: socklen_t = INT_LENGTH;
    let value: *mut c_void = (&raw mut kind).cast();
    let size: *mut socklen_t = &raw mut length;
    #[expect(unsafe_code, reason = "linux-peercred")]
    // SAFETY: `getsockopt(2)` with `SOL_SOCKET` and `SO_TYPE` writes at
    // most `*optlen` bytes of one `int` through `optval` and the length it
    // wrote through `optlen`, and reads nothing else of ours. `value`
    // points to `kind` and `size` to `length`, two locals of this
    // function, zero-initialised and set to an `int`'s size, live and
    // unused for the whole call, so the kernel writes only memory this
    // call may write, and nothing keeps either pointer afterwards.
    // `descriptor` is borrowed from the caller's `UnixStream` for the
    // call, so it stays open. A bad descriptor or option is answered with
    // -1 and errno (EBADF, ENOTSOCK, ENOPROTOOPT), never undefined
    // behaviour. The call shares no state with other threads. The
    // declaration is in `abi`, proved by the ABI check.
    let status = unsafe { abi::getsockopt(descriptor, level, option, value, size) };
    if status != 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(PeerError::Failed(abi::last_error()));
    }
    let returned = usize::try_from(length).unwrap_or(usize::MAX);
    if returned != INT_SIZE {
        return Err(PeerError::Length {
            returned,
            expected: INT_SIZE,
        });
    }
    if kind != SOCK_STREAM {
        return Err(PeerError::NotStream(kind));
    }
    Ok(())
}

/// One `getsockopt(SO_PEERCRED)` call on `socket`, into `credentials` with
/// the length in `length`.
fn read(socket: BorrowedFd<'_>, credentials: &mut ucred, length: &mut socklen_t) -> io::Result<()> {
    let descriptor: c_int = socket.as_raw_fd();
    let level: c_int = SOL_SOCKET;
    let option: c_int = SO_PEERCRED;
    let value: *mut c_void = (&raw mut *credentials).cast();
    let size: *mut socklen_t = &raw mut *length;
    #[expect(unsafe_code, reason = "linux-peercred")]
    // SAFETY: `getsockopt(2)` with `SOL_SOCKET` and `SO_PEERCRED` writes at
    // most `*optlen` bytes of one `struct ucred` through `optval` and the
    // length it wrote through `optlen` ("a value-result argument"), and
    // reads nothing else of ours. `value` points to `credentials` and
    // `size` to `length`, two exclusive borrows of the caller's
    // zero-initialised locals, live and unused for the whole call;
    // `length` holds `struct ucred`'s size, so the kernel writes only
    // memory this call may write, and nothing keeps either pointer
    // afterwards. `descriptor` is borrowed from the caller's `UnixStream`
    // for the call, so it stays open and names a Unix stream socket. A bad
    // descriptor or option is answered with -1 and errno (EBADF,
    // ENOTSOCK, ENOPROTOOPT), never undefined behaviour. The call shares
    // no state with other threads. The declaration is in `abi`, proved by
    // the ABI check.
    let status = unsafe { abi::getsockopt(descriptor, level, option, value, size) };
    if status != 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(abi::last_error());
    }
    Ok(())
}
