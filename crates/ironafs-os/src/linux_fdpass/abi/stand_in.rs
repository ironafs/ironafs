// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-ins for `sendmsg` and `recvmsg` (ADR 0005, point 6.5),
//! built only under Miri, which cannot call the C library. They model one
//! stream per thread: `sendmsg` reads the whole message the wrapper built
//! (the message, its one buffer, its byte and its control data, so Miri
//! checks every pointer), takes its own copy of each descriptor an
//! `SCM_RIGHTS` header carries, as the kernel does, and queues them with
//! the byte; `recvmsg` takes the oldest queued message, writes its byte,
//! a header and the descriptors into the receiver's buffers, and sets the
//! control length, as the kernel does; with nothing queued it returns 0,
//! the end of the stream. A call without `MSG_NOSIGNAL` (send) or
//! `MSG_CMSG_CLOEXEC` (receive), or with a message of another shape,
//! returns -1.

use core::ffi::c_int;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::os::fd::{BorrowedFd, IntoRawFd, OwnedFd};

use super::{MSG_CMSG_CLOEXEC, MSG_NOSIGNAL, SCM_RIGHTS, SOL_SOCKET, cmsghdr, iovec, msghdr};
use crate::linux_fdpass::{Control, ONE, SPACE, TWO};

thread_local! {
    /// The messages sent and not yet received on this thread.
    static QUEUE: RefCell<VecDeque<(u8, Vec<OwnedFd>)>> = const { RefCell::new(VecDeque::new()) };
}

/// Reads and queues the message at `msg` as `sendmsg` would.
///
/// # Safety
///
/// As the foreign function's: `msg` and every buffer it names are valid
/// for reads of their stated sizes, and each descriptor in its control
/// data is open.
#[expect(unsafe_code, reason = "linux-fdpass: Miri stand-in")]
pub(in crate::linux_fdpass) unsafe fn sendmsg(
    sockfd: c_int,
    msg: *const msghdr,
    flags: c_int,
) -> isize {
    if sockfd < 0 || flags & MSG_NOSIGNAL == 0 {
        return -1;
    }
    #[expect(unsafe_code, reason = "linux-fdpass: Miri stand-in")]
    // SAFETY: the caller promises `msg` is valid for one read; Miri checks.
    let message = unsafe { core::ptr::read(msg) };
    let (buffers, count) = message.buffers();
    if count != 1 || message.control_length() != SPACE {
        return -1;
    }
    #[expect(unsafe_code, reason = "linux-fdpass: Miri stand-in")]
    // SAFETY: `msg_iov` names one valid `iovec`, as promised; Miri checks.
    let buffer = unsafe { core::ptr::read(buffers) };
    if buffer.iov_len != 1 {
        return -1;
    }
    let data: *const u8 = buffer.iov_base.cast_const().cast();
    #[expect(unsafe_code, reason = "linux-fdpass: Miri stand-in")]
    // SAFETY: the buffer is valid for one byte, as promised; Miri checks.
    let byte = unsafe { core::ptr::read(data) };
    let control: *const Control = message.control().cast_const().cast();
    #[expect(unsafe_code, reason = "linux-fdpass: Miri stand-in")]
    // SAFETY: the control data is `SPACE` bytes, a `Control`'s size, valid
    // and aligned as promised; Miri checks.
    let control = unsafe { core::ptr::read(control) };
    let header = &control.header;
    let count = match header.length() {
        ONE => 1,
        TWO => 2,
        _ => return -1,
    };
    if (header.level(), header.kind()) != (SOL_SOCKET, SCM_RIGHTS) {
        return -1;
    }
    let mut copies = Vec::new();
    for &descriptor in control.descriptors.iter().take(count) {
        if descriptor < 0 {
            return -1;
        }
        #[expect(unsafe_code, reason = "linux-fdpass: Miri stand-in")]
        // SAFETY: the caller promises each descriptor in the control data
        // is open for the call.
        let borrowed = unsafe { BorrowedFd::borrow_raw(descriptor) };
        let Ok(copy) = borrowed.try_clone_to_owned() else {
            return -1;
        };
        copies.push(copy);
    }
    QUEUE.with(|queue| queue.borrow_mut().push_back((byte, copies)));
    1
}

/// Writes the oldest queued message into `msg`'s buffers as `recvmsg`
/// would.
///
/// # Safety
///
/// As the foreign function's: `msg` is valid for a read and a write, and
/// every buffer it names for writes of its stated size.
#[expect(unsafe_code, reason = "linux-fdpass: Miri stand-in")]
pub(in crate::linux_fdpass) unsafe fn recvmsg(
    sockfd: c_int,
    msg: *mut msghdr,
    flags: c_int,
) -> isize {
    if sockfd < 0 || flags & MSG_CMSG_CLOEXEC == 0 {
        return -1;
    }
    let Some((byte, descriptors)) = QUEUE.with(|queue| queue.borrow_mut().pop_front()) else {
        return 0;
    };
    #[expect(unsafe_code, reason = "linux-fdpass: Miri stand-in")]
    // SAFETY: the caller promises `msg` is valid for one read; Miri checks.
    let mut message = unsafe { core::ptr::read(msg) };
    let (buffers, count) = message.buffers();
    if count != 1 || message.control_length() < SPACE {
        return -1;
    }
    #[expect(unsafe_code, reason = "linux-fdpass: Miri stand-in")]
    // SAFETY: `msg_iov` names one valid `iovec`, as promised; Miri checks.
    let buffer: iovec = unsafe { core::ptr::read(buffers) };
    if buffer.iov_len < 1 {
        return -1;
    }
    let data: *mut u8 = buffer.iov_base.cast();
    #[expect(unsafe_code, reason = "linux-fdpass: Miri stand-in")]
    // SAFETY: the buffer is valid for one byte of writes; Miri checks.
    let () = unsafe { core::ptr::write(data, byte) };
    let delivered = descriptors.len();
    let mut numbers = [0; 2];
    for (slot, descriptor) in numbers.iter_mut().zip(descriptors) {
        *slot = descriptor.into_raw_fd();
    }
    let length = if delivered == 2 { TWO } else { ONE };
    let Some(header) = cmsghdr::new(length, SOL_SOCKET, SCM_RIGHTS) else {
        return -1;
    };
    let control: *mut Control = message.control().cast();
    let written = Control {
        header,
        descriptors: numbers,
    };
    #[expect(unsafe_code, reason = "linux-fdpass: Miri stand-in")]
    // SAFETY: the control data is at least `SPACE` bytes, a `Control`'s
    // size, valid for writes and aligned as promised; Miri checks.
    let () = unsafe { core::ptr::write(control, written) };
    if message.set_control_length(SPACE).is_none() {
        return -1;
    }
    #[expect(unsafe_code, reason = "linux-fdpass: Miri stand-in")]
    // SAFETY: as the read of `msg` above, for one write.
    let () = unsafe { core::ptr::write(msg, message) };
    1
}
