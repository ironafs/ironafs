// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Site `linux-fdpass` (ADR 0005, row 14 and sheet 14): one descriptor
//! passed over a Unix stream, sent with `sendmsg` and received with
//! `recvmsg` as an `SCM_RIGHTS` control message beside one data byte (a
//! stream socket carries control data only with data, unix(7)).
//!
//! The send uses `MSG_NOSIGNAL`, so a closed peer is an `EPIPE` error and
//! not a signal; the receive uses `MSG_CMSG_CLOEXEC`, so a received
//! descriptor is never inherited across `exec`. Both retry `EINTR`. The
//! control buffer is a [`Control`]: a `struct cmsghdr` followed by two
//! `int`s, zero-initialised, aligned for the header and read back as
//! fields, with no pointer walk. It holds two descriptors, so a sender of
//! two is seen (and both closed) rather than truncated.
//!
//! What the receive returned is checked by [`crate::fdpass::accept`]
//! before any descriptor is adopted, and every descriptor a valid message
//! delivered is adopted as an `OwnedFd` before anything is refused, so
//! none is leaked and none leaves this module as an integer: a pidfd the
//! kernel installed with `SCM_PIDFD` (on a socket with `SO_PASSPIDFD` set
//! outside IronAFS) included, which is then closed.
//!
//! The wrapper says nothing about what the descriptor refers to, nor who
//! is at the other end: the receiver checks the sender, and the sender the
//! recipient before it sends, with `peer_credentials` on the same stream
//! (sheet 14; that policy is the mount step's).

mod abi;

use core::ffi::c_int;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixStream;

use crate::fdpass::{FdPassError, Received, Rules, accept, retried, sent};
use abi::{
    MSG_CMSG_CLOEXEC, MSG_CTRUNC, MSG_NOSIGNAL, SCM_PIDFD, SCM_RIGHTS, SOL_SOCKET, cmsghdr, iovec,
    msghdr,
};

#[cfg(test)]
mod tests;

/// The one data byte a send writes beside the descriptor.
const DATA: u8 = 1;

/// The send's flags: a closed peer is an error, not a signal.
const SEND_FLAGS: c_int = MSG_NOSIGNAL;

/// The receive's flags: received descriptors are close-on-exec.
const RECEIVE_FLAGS: c_int = MSG_CMSG_CLOEXEC;

/// The control buffer: one header and room for two descriptors.
#[repr(C)]
#[derive(Debug)]
struct Control {
    header: cmsghdr,
    descriptors: [c_int; 2],
}

/// `CMSG_ALIGN` (cmsg(3)): `length` rounded up to a multiple of
/// `sizeof(size_t)`, as both C libraries define it.
const fn cmsg_align(length: usize) -> usize {
    let unit = size_of::<usize>();
    (length + unit - 1) & !(unit - 1)
}

/// `CMSG_LEN` (cmsg(3)): a header and `length` bytes of data.
const fn cmsg_len(length: usize) -> usize {
    cmsg_align(size_of::<cmsghdr>()) + length
}

/// `CMSG_SPACE` (cmsg(3)): what a control message of `length` bytes of
/// data occupies.
const fn cmsg_space(length: usize) -> usize {
    cmsg_align(length) + cmsg_align(size_of::<cmsghdr>())
}

/// `CMSG_LEN` of one descriptor, and of two.
const ONE: usize = cmsg_len(size_of::<c_int>());
const TWO: usize = cmsg_len(2 * size_of::<c_int>());

/// `CMSG_SPACE` of one descriptor: the control length a send gives, and
/// the receive buffer's size.
const SPACE: usize = cmsg_space(size_of::<c_int>());

// The values ADR 0005, sheet 14, states for the allowlisted 64-bit
// targets, and the control buffer's shape: its size is the space of one
// descriptor, it is aligned as a header, and the descriptors start where
// `CMSG_DATA` points.
const _: () = assert!(
    ONE == 20
        && TWO == 24
        && SPACE == 24
        && size_of::<Control>() == SPACE
        && align_of::<Control>() == align_of::<cmsghdr>()
        && core::mem::offset_of!(Control, descriptors) == cmsg_len(0)
);

// The same values as the C headers' macros give, which the ABI check
// proves for `abi`'s constants on each target.
const _: () =
    assert!(ONE == abi::CMSG_LEN_ONE && TWO == abi::CMSG_LEN_TWO && SPACE == abi::CMSG_SPACE_ONE);

/// The C facts the receive's checks compare with.
const RULES: Rules = Rules {
    level: SOL_SOCKET,
    rights: SCM_RIGHTS,
    pidfd: SCM_PIDFD,
    truncated: MSG_CTRUNC,
    one: ONE,
    two: TWO,
    capacity: SPACE,
    data: DATA,
};

/// Why a message could not be built: a length that does not fit its C
/// field, which the constants above rule out.
fn unbuildable() -> io::Error {
    io::Error::other("a control length that does not fit its C field")
}

/// Sends `descriptor` over `stream`, beside one data byte.
///
/// The peer gets its own descriptor for the same open file; `descriptor`
/// stays open here.
///
/// # Errors
///
/// [`FdPassError::Send`] with the errno if the send fails (`EPIPE` for a
/// closed peer), and [`FdPassError::ShortSend`] if it reports any count
/// but one byte.
pub fn send_fd(stream: &UnixStream, descriptor: BorrowedFd<'_>) -> Result<(), FdPassError> {
    let mut byte = DATA;
    let mut buffer = iovec {
        iov_base: (&raw mut byte).cast(),
        iov_len: 1,
    };
    let header = cmsghdr::new(ONE, SOL_SOCKET, SCM_RIGHTS)
        .ok_or_else(|| FdPassError::Send(unbuildable()))?;
    let mut control = Control {
        header,
        descriptors: [descriptor.as_raw_fd(), 0],
    };
    let message = msghdr::new(&raw mut buffer, (&raw mut control).cast(), SPACE)
        .ok_or_else(|| FdPassError::Send(unbuildable()))?;
    let count = retried(|| send_message(stream.as_fd(), &message)).map_err(FdPassError::Send)?;
    sent(count)
}

/// Receives exactly one descriptor from `stream`, as an `OwnedFd`, with
/// close-on-exec set.
///
/// # Errors
///
/// [`FdPassError::Receive`] with the errno if the receive fails, and the
/// refusals of the checks (see [`FdPassError`]): on each, every descriptor
/// the message delivered has been closed.
pub fn receive_fd(stream: &UnixStream) -> Result<OwnedFd, FdPassError> {
    let mut byte = 0_u8;
    let mut buffer = iovec {
        iov_base: (&raw mut byte).cast(),
        iov_len: 1,
    };
    let header = cmsghdr::new(0, 0, 0).ok_or_else(|| FdPassError::Receive(unbuildable()))?;
    let mut control = Control {
        header,
        descriptors: [0, 0],
    };
    let mut message = msghdr::new(&raw mut buffer, (&raw mut control).cast(), SPACE)
        .ok_or_else(|| FdPassError::Receive(unbuildable()))?;
    let count =
        retried(|| receive_message(stream.as_fd(), &mut message)).map_err(FdPassError::Receive)?;
    let received = Received {
        count,
        flags: message.flags(),
        control: message.control_length(),
        length: control.header.length(),
        level: control.header.level(),
        kind: control.header.kind(),
        payload: control.descriptors,
        byte,
    };
    accept(&RULES, received, adopt)
}

/// One `sendmsg` call of `message` on `socket`: the bytes it sent.
fn send_message(socket: BorrowedFd<'_>, message: &msghdr) -> io::Result<usize> {
    let descriptor: c_int = socket.as_raw_fd();
    let pointer: *const msghdr = message;
    let flags: c_int = SEND_FLAGS;
    #[expect(unsafe_code, reason = "linux-fdpass")]
    // SAFETY: `sendmsg(2)` reads `*msg` and, through it, `msg_iovlen`
    // buffers of `msg_iov` and `msg_controllen` bytes at `msg_control`; it
    // writes nothing of ours. `pointer` points to `message`, borrowed for
    // the call, which `msghdr::new` built: no address, one `iovec` and the
    // control buffer, every other field (musl's padding included) zero.
    // Its callers build it over their own locals, live and unchanged for
    // the whole call: the `iovec` over one byte, and a `Control` of
    // exactly `msg_controllen` bytes (`SPACE`), aligned for its header,
    // whose header says `CMSG_LEN` of one `int` and whose first `int` is a
    // descriptor borrowed for the call, so it is open. Nothing keeps any
    // pointer afterwards. `flags` is `MSG_NOSIGNAL`, so a closed peer is
    // `EPIPE`, not a signal. Failures are -1 with errno, never undefined
    // behaviour. The call shares no state with other threads. The
    // declarations are in `abi`, proved by the ABI check.
    let count = unsafe { abi::sendmsg(descriptor, pointer, flags) };
    // At once, with no call in between, on the thread that made it.
    usize::try_from(count).map_err(|_| abi::last_error())
}

/// One `recvmsg` call into `message` on `socket`: the bytes it received.
fn receive_message(socket: BorrowedFd<'_>, message: &mut msghdr) -> io::Result<usize> {
    let descriptor: c_int = socket.as_raw_fd();
    let pointer: *mut msghdr = message;
    let flags: c_int = RECEIVE_FLAGS;
    #[expect(unsafe_code, reason = "linux-fdpass")]
    // SAFETY: `recvmsg(2)` writes at most `iov_len` bytes into each of the
    // `msg_iovlen` buffers, at most `msg_controllen` bytes of control data
    // at `msg_control`, and the message's `msg_controllen` and
    // `msg_flags`. `pointer` points to `message`, exclusively borrowed for
    // the call, which `receive_fd` built over its own locals, live and
    // unused for the whole call: one `iovec` over one byte, and a zeroed
    // `Control` of exactly `msg_controllen` bytes (`SPACE`), aligned for
    // its header; so the kernel writes only memory this call may write,
    // and nothing keeps any pointer afterwards. A descriptor the control
    // data delivers (`SCM_RIGHTS`, or a pidfd with `SCM_PIDFD`) is only an
    // integer until `accept` has checked the header and `adopt` owns it. `flags` is `MSG_CMSG_CLOEXEC`. Failures
    // are -1 with errno, never undefined behaviour. The call shares no
    // state with other threads. The declarations are in `abi`, proved by
    // the ABI check.
    let count = unsafe { abi::recvmsg(descriptor, pointer, flags) };
    // At once, with no call in between, on the thread that made it.
    usize::try_from(count).map_err(|_| abi::last_error())
}

/// Takes ownership of a descriptor a receive delivered.
///
/// Only `receive_fd` passes it, through `accept`, which calls it only with
/// a non-negative integer of an `SCM_RIGHTS` or `SCM_PIDFD` message whose
/// header passed every check, once per integer.
fn adopt(descriptor: c_int) -> OwnedFd {
    #[expect(unsafe_code, reason = "linux-fdpass")]
    // SAFETY: `OwnedFd::from_raw_fd` requires an open descriptor that
    // nothing else owns or will close. `accept` passes only a non-negative
    // integer from the data of a control message whose level is
    // `SOL_SOCKET`, whose type is `SCM_RIGHTS` or `SCM_PIDFD` and whose
    // length lies within the returned control data: a descriptor the
    // kernel installed in this process for this receive ("a set of open
    // file descriptors", unix(7), or the peer's pidfd), which no other
    // code has seen, and it passes each one once. So the `OwnedFd`
    // is its only owner, and dropping it closes it.
    unsafe {
        OwnedFd::from_raw_fd(descriptor)
    }
}
