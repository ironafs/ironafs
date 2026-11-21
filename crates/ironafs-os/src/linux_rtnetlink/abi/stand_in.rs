// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-ins for `socket`, `bind`, `sendto` and `recvmsg` (ADR
//! 0005, point 6.5), built only under Miri, which cannot make a netlink
//! socket. `socket` returns one end of a fresh local socket pair (a
//! descriptor Miri knows, so adopting and closing it is checked); `bind`
//! and `sendto` read every byte the wrapper points them at; `recvmsg`
//! writes a reply a test queued (its bytes, cut to the buffer with
//! `MSG_TRUNC` set when longer, and its sender's address, of the length
//! the reply says) through the wrapper's message, as the kernel does.
//! Each records the call, in order, on this thread. A test can [`plan`]
//! the next `sendto` and `recvmsg` answers instead: interrupted by a
//! signal some number of times (-1 with `EINTR`), or failed once with an
//! errno; [`last_error`] returns that errno, as the C library's would
//! be.

use core::ffi::{c_int, c_void};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io;
use std::os::fd::IntoRawFd;
use std::os::unix::net::UnixStream;

use super::{MSG_TRUNC, iovec, msghdr, sockaddr, sockaddr_nl, socklen_t};

/// The length of `struct sockaddr_nl`.
const NAME: usize = size_of::<sockaddr_nl>();

/// One call a stand-in received, with what it read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::linux_rtnetlink) enum Call {
    Socket(c_int, c_int, c_int),
    /// The address read, and the length.
    Bind(sockaddr_nl, socklen_t),
    /// The bytes, the flags, the address read, and its length.
    Send(Vec<u8>, c_int, sockaddr_nl, socklen_t),
    /// The flags.
    Receive(c_int),
    /// A send answered with a planned failure.
    Failed,
}

/// A datagram the next `recvmsg` returns.
#[derive(Clone, Debug)]
pub(in crate::linux_rtnetlink) struct Reply {
    pub(in crate::linux_rtnetlink) bytes: Vec<u8>,
    /// The sender's address as written, and the length reported.
    pub(in crate::linux_rtnetlink) sender: sockaddr_nl,
    pub(in crate::linux_rtnetlink) name_length: socklen_t,
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static REPLIES: RefCell<VecDeque<Reply>> = const { RefCell::new(VecDeque::new()) };
    static SHORT: Cell<bool> = const { Cell::new(false) };
    static PLAN: Cell<Plan> = const { Cell::new(Plan::Answer) };
    static ERRNO: Cell<c_int> = const { Cell::new(0) };
}

/// `EINTR`.
const EINTR: c_int = 4;

/// The next `sendto` and `recvmsg` answers.
#[derive(Clone, Copy, Debug)]
pub(in crate::linux_rtnetlink) enum Plan {
    /// As the kernel answers.
    Answer,
    /// -1 with `EINTR` for this many calls, then as the kernel answers.
    Interrupt(usize),
    /// -1 with this errno, once.
    Fail(c_int),
}

/// Plans the next calls' answer on this thread.
pub(in crate::linux_rtnetlink) fn plan(next: Plan) {
    PLAN.with(|plan| plan.set(next));
}

/// The errno the last planned failure set.
pub(in crate::linux_rtnetlink) fn last_error() -> io::Error {
    io::Error::from_raw_os_error(ERRNO.with(Cell::get))
}

/// Answers a call as planned: `true` for a planned failure, with its
/// errno set.
fn planned_failure() -> bool {
    let errno = match PLAN.with(Cell::get) {
        Plan::Answer => return false,
        Plan::Interrupt(left) => {
            let next = if left > 1 {
                Plan::Interrupt(left - 1)
            } else {
                Plan::Answer
            };
            PLAN.with(|plan| plan.set(next));
            EINTR
        }
        Plan::Fail(errno) => {
            PLAN.with(|plan| plan.set(Plan::Answer));
            errno
        }
    };
    ERRNO.with(|cell| cell.set(errno));
    true
}

/// The calls made on this thread so far, which it then forgets.
pub(in crate::linux_rtnetlink) fn calls() -> Vec<Call> {
    CALLS.with(|calls| calls.take())
}

/// Queues `reply` for a later `recvmsg` on this thread.
pub(in crate::linux_rtnetlink) fn reply(reply: Reply) {
    REPLIES.with(|replies| replies.borrow_mut().push_back(reply));
}

/// Makes each later `sendto` on this thread report one byte fewer.
pub(in crate::linux_rtnetlink) fn short_sends() {
    SHORT.with(|short| short.set(true));
}

fn record(call: Call) {
    CALLS.with(|calls| calls.borrow_mut().push(call));
}

/// Makes a descriptor as `socket` would.
///
/// # Safety
///
/// None beyond the foreign function's: it takes integers.
#[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
pub(in crate::linux_rtnetlink) unsafe fn socket(
    domain: c_int,
    kind: c_int,
    protocol: c_int,
) -> c_int {
    record(Call::Socket(domain, kind, protocol));
    match UnixStream::pair() {
        Ok((ours, _peer)) => ours.into_raw_fd(),
        Err(_) => -1,
    }
}

/// Reads the address as `bind` would.
///
/// # Safety
///
/// As the foreign function's: `addr` is valid for reads of `addrlen`
/// bytes.
#[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
pub(in crate::linux_rtnetlink) unsafe fn bind(
    sockfd: c_int,
    addr: *const sockaddr,
    addrlen: socklen_t,
) -> c_int {
    if sockfd < 0 || usize::try_from(addrlen) != Ok(NAME) {
        return -1;
    }
    let pointer: *const sockaddr_nl = addr.cast();
    #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
    // SAFETY: the caller promises `addr` is valid for `addrlen` bytes, one
    // `struct sockaddr_nl`; Miri checks it and its alignment.
    let local = unsafe { core::ptr::read(pointer) };
    record(Call::Bind(local, addrlen));
    0
}

/// Reads the datagram and the address as `sendto` would.
///
/// # Safety
///
/// As the foreign function's: `buf` is valid for reads of `len` bytes and
/// `dest_addr` of `addrlen`.
#[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
pub(in crate::linux_rtnetlink) unsafe fn sendto(
    sockfd: c_int,
    buf: *const c_void,
    len: usize,
    flags: c_int,
    dest_addr: *const sockaddr,
    addrlen: socklen_t,
) -> isize {
    if sockfd < 0 || usize::try_from(addrlen) != Ok(NAME) {
        return -1;
    }
    if planned_failure() {
        record(Call::Failed);
        return -1;
    }
    let mut bytes = vec![0_u8; len];
    let source: *const u8 = buf.cast();
    let target: *mut u8 = bytes.as_mut_ptr();
    #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
    // SAFETY: the caller promises `buf` is valid for `len` bytes; `bytes`
    // is a new allocation of `len` bytes; Miri checks both.
    let () = unsafe { core::ptr::copy_nonoverlapping(source, target, len) };
    let pointer: *const sockaddr_nl = dest_addr.cast();
    #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
    // SAFETY: the caller promises `dest_addr` is valid for `addrlen`
    // bytes, one `struct sockaddr_nl`; Miri checks it.
    let to = unsafe { core::ptr::read(pointer) };
    record(Call::Send(bytes, flags, to, addrlen));
    let reported = if SHORT.with(Cell::get) {
        len.saturating_sub(1)
    } else {
        len
    };
    isize::try_from(reported).unwrap_or(-1)
}

/// Writes the oldest queued reply through `msg` as `recvmsg` would; with
/// none queued, -1.
///
/// # Safety
///
/// As the foreign function's: `msg` is valid for a read and a write, its
/// address buffer for writes of its stated length, and its one data
/// buffer for writes of its stated length.
#[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
pub(in crate::linux_rtnetlink) unsafe fn recvmsg(
    sockfd: c_int,
    msg: *mut msghdr,
    flags: c_int,
) -> isize {
    record(Call::Receive(flags));
    if sockfd < 0 || planned_failure() {
        return -1;
    }
    let Some(queued) = REPLIES.with(|replies| replies.borrow_mut().pop_front()) else {
        return -1;
    };
    #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
    // SAFETY: the caller promises `msg` is valid for one read; Miri checks.
    let mut message = unsafe { core::ptr::read(msg) };
    let (buffers, count) = message.buffers();
    let (name, room) = message.name();
    if count != 1 || message.control_length() != 0 || usize::try_from(room) != Ok(NAME) {
        return -1;
    }
    #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
    // SAFETY: `msg_iov` names one valid `iovec`, as promised; Miri checks.
    let buffer: iovec = unsafe { core::ptr::read(buffers) };
    let written = queued.bytes.len().min(buffer.iov_len);
    let source: *const u8 = queued.bytes.as_ptr();
    let target: *mut u8 = buffer.iov_base.cast();
    #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
    // SAFETY: the data buffer is valid for `iov_len` bytes of writes, as
    // promised, and `written` is at most that and the reply's length;
    // Miri checks both.
    let () = unsafe { core::ptr::copy_nonoverlapping(source, target, written) };
    let address: *mut sockaddr_nl = name.cast();
    let sender = queued.sender;
    #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
    // SAFETY: the address buffer is valid for `msg_namelen` bytes, one
    // `struct sockaddr_nl`, aligned, as promised; Miri checks.
    let () = unsafe { core::ptr::write(address, sender) };
    let returned = if queued.bytes.len() > buffer.iov_len {
        MSG_TRUNC
    } else {
        0
    };
    message.set_returned(queued.name_length, returned);
    #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
    // SAFETY: as the read of `msg` above, for one write.
    let () = unsafe { core::ptr::write(msg, message) };
    isize::try_from(written).unwrap_or(-1)
}
