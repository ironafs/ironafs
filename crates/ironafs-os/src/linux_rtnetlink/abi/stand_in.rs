// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-ins for `socket`, `bind`, `sendto`, `recvmsg`, `eventfd`
//! and `poll` (ADR 0005, point 6.5), built only under Miri, which cannot
//! make a netlink socket or an `eventfd`. `socket` and `eventfd` return one
//! end of a fresh local socket pair (a descriptor Miri knows, so adopting
//! and closing it is checked); `bind` and `sendto` read every byte the
//! wrapper points them at; `recvmsg` writes a reply a test queued (its
//! bytes, cut to the buffer with `MSG_TRUNC` set when longer, and its
//! sender's address, written up to the shorter of the length the reply
//! says and the caller's buffer, with the reply's length reported whole)
//! through the wrapper's message, as the kernel does, and with none queued
//! returns -1 with `EAGAIN`, as a receive with `MSG_DONTWAIT` does; `poll`
//! reads every entry and writes the `revents` and the count of an answer a
//! test queued ([`answer`]), which may be any values, those sheet 23
//! refuses included.
//! Each records the call, in order, on this thread. A test can [`plan`]
//! the next `sendto` and `recvmsg` answers instead: interrupted by a
//! signal some number of times (-1 with `EINTR`), or failed once with an
//! errno; and queue a `poll` answer of -1 with `EINTR` or another errno.
//! [`last_error`] returns that errno, as the C library's would be.
//!
//! A call of a shape a stand-in does not model is refused with -1 and
//! `EINVAL`, so no earlier errno is reported for it.

use core::ffi::{c_int, c_short, c_uint, c_void};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::os::fd::IntoRawFd;
use std::os::unix::net::UnixStream;

use super::{EAGAIN, MSG_TRUNC, iovec, msghdr, nfds_t, pollfd, sockaddr, sockaddr_nl, socklen_t};
pub(in crate::linux_rtnetlink) use crate::stand_in_errno::last_error;
use crate::stand_in_errno::{refused, set_errno};

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
    /// The bytes of the sender's address a receive wrote.
    Named(usize),
    /// A send answered with a planned failure.
    Failed,
    /// The initial value and the flags.
    Eventfd(c_uint, c_int),
    /// Each entry's events asked for and `revents` as passed (whether its
    /// descriptor was non-negative, too), the count of entries, and the
    /// timeout.
    Poll(Vec<(bool, c_short, c_short)>, nfds_t, c_int),
}

/// A datagram the next `recvmsg` returns.
#[derive(Clone, Debug)]
pub(in crate::linux_rtnetlink) struct Reply {
    pub(in crate::linux_rtnetlink) bytes: Vec<u8>,
    /// The sender's address as written, and the length reported.
    pub(in crate::linux_rtnetlink) sender: sockaddr_nl,
    pub(in crate::linux_rtnetlink) name_length: socklen_t,
    /// Writes no address at all and leaves the address length as the
    /// caller set it (a call that reports success without a sender).
    pub(in crate::linux_rtnetlink) untouched: bool,
}

/// What the next `poll` returns.
#[derive(Clone, Copy, Debug)]
pub(in crate::linux_rtnetlink) enum Answer {
    /// This count, with these `revents` on the route socket and the wake.
    Report(c_int, c_short, c_short),
    /// -1 with this errno (`EINTR` for an interrupted wait).
    Fail(c_int),
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static REPLIES: RefCell<VecDeque<Reply>> = const { RefCell::new(VecDeque::new()) };
    static ANSWERS: RefCell<VecDeque<Answer>> = const { RefCell::new(VecDeque::new()) };
    static SHORT: Cell<bool> = const { Cell::new(false) };
    static PLAN: Cell<Plan> = const { Cell::new(Plan::Answer) };
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
    set_errno(errno);
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

/// Queues `answer` for a later `poll` on this thread.
pub(in crate::linux_rtnetlink) fn answer(answer: Answer) {
    ANSWERS.with(|answers| answers.borrow_mut().push_back(answer));
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
        Err(_) => refused(),
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
        return refused();
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
        return refused();
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
    if sockfd < 0 {
        return refused();
    }
    if planned_failure() {
        return -1;
    }
    let Some(queued) = REPLIES.with(|replies| replies.borrow_mut().pop_front()) else {
        // Nothing queued: a receive that does not wait says so.
        set_errno(EAGAIN);
        return -1;
    };
    #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
    // SAFETY: the caller promises `msg` is valid for one read; Miri checks.
    let mut message = unsafe { core::ptr::read(msg) };
    let (buffers, count) = message.buffers();
    let (name, room) = message.name();
    if count != 1 || message.control_length() != 0 || usize::try_from(room) != Ok(NAME) {
        return refused();
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
    let sender = queued.sender;
    if !queued.untouched {
        // As the kernel does: the sender's address up to the shorter of
        // the reply's address length and the buffer the caller stated.
        let named = usize::try_from(queued.name_length)
            .unwrap_or(usize::MAX)
            .min(NAME);
        let from: *const u8 = (&raw const sender).cast();
        let address: *mut u8 = name.cast();
        record(Call::Named(named));
        #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
        // SAFETY: the address buffer is valid for `msg_namelen` bytes, one
        // `struct sockaddr_nl`, as promised, and `named` is at most that;
        // `from` is `sender`, a local of that struct's size; Miri checks
        // both.
        let () = unsafe { core::ptr::copy_nonoverlapping(from, address, named) };
    }
    let returned = if queued.bytes.len() > buffer.iov_len {
        MSG_TRUNC
    } else {
        0
    };
    let name_length = if queued.untouched {
        room
    } else {
        queued.name_length
    };
    message.set_returned(name_length, returned);
    #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
    // SAFETY: as the read of `msg` above, for one write.
    let () = unsafe { core::ptr::write(msg, message) };
    isize::try_from(written).unwrap_or(-1)
}

/// Makes a descriptor as `eventfd` would.
///
/// # Safety
///
/// None beyond the foreign function's: it takes integers.
#[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
pub(in crate::linux_rtnetlink) unsafe fn eventfd(initval: c_uint, flags: c_int) -> c_int {
    record(Call::Eventfd(initval, flags));
    match UnixStream::pair() {
        Ok((ours, _peer)) => ours.into_raw_fd(),
        Err(_) => refused(),
    }
}

/// Reads every entry and writes the oldest queued answer as `poll`
/// would; with none queued, refused.
///
/// # Safety
///
/// As the foreign function's: `fds` is valid for reads and writes of
/// `nfds` entries.
#[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
pub(in crate::linux_rtnetlink) unsafe fn poll(
    fds: *mut pollfd,
    nfds: nfds_t,
    timeout: c_int,
) -> c_int {
    let Ok(count) = usize::try_from(nfds) else {
        return refused();
    };
    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let pointer: *mut pollfd = fds.wrapping_add(index);
        #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
        // SAFETY: the caller promises `fds` is valid for `nfds` entries,
        // and `index` is below that; Miri checks it and its alignment.
        let entry = unsafe { core::ptr::read(pointer) };
        entries.push(entry);
    }
    record(Call::Poll(
        entries
            .iter()
            .map(|entry| (entry.fd >= 0, entry.events, entry.revents))
            .collect(),
        nfds,
        timeout,
    ));
    let Some(answer) = ANSWERS.with(|answers| answers.borrow_mut().pop_front()) else {
        return refused();
    };
    let (returned, socket, wake) = match answer {
        Answer::Report(returned, socket, wake) => (returned, socket, wake),
        Answer::Fail(code) => {
            set_errno(code);
            return -1;
        }
    };
    if count != 2 {
        return refused();
    }
    for (index, revents) in [socket, wake].into_iter().enumerate() {
        let pointer: *mut pollfd = fds.wrapping_add(index);
        let mut entry = entries.get(index).copied().unwrap_or(pollfd {
            fd: -1,
            events: 0,
            revents: 0,
        });
        entry.revents = revents;
        #[expect(unsafe_code, reason = "linux-rtnetlink: Miri stand-in")]
        // SAFETY: as the read of each entry above, for one write.
        let () = unsafe { core::ptr::write(pointer, entry) };
    }
    returned
}
