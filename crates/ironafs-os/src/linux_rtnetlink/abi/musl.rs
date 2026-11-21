// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! musl's `struct msghdr` (site `linux-rtnetlink`): `msg_iovlen` is `int`
//! and `msg_controllen` is `socklen_t`, each beside an `int` padding field
//! that follows it on these little-endian targets (musl's
//! `<sys/socket.h>`). The padding is zero in every value this crate
//! builds.

use core::ffi::{c_int, c_void};

use super::{iovec, socklen_t};

/// `struct msghdr`: one message's address, data buffers and control
/// data.
#[repr(C)]
#[derive(Debug)]
pub(in crate::linux_rtnetlink) struct msghdr {
    msg_name: *mut c_void,
    msg_namelen: socklen_t,
    msg_iov: *mut iovec,
    msg_iovlen: c_int,
    __pad1: c_int,
    msg_control: *mut c_void,
    msg_controllen: socklen_t,
    __pad2: c_int,
    msg_flags: c_int,
}

impl msghdr {
    /// A message whose sender's address goes to `name`, of `name_length`
    /// bytes, and whose data goes to the one buffer at `iov`, with no
    /// control data; every other field (padding included) zero.
    pub(in crate::linux_rtnetlink) fn new(
        name: *mut c_void,
        name_length: socklen_t,
        iov: *mut iovec,
    ) -> Self {
        Self {
            msg_name: name,
            msg_namelen: name_length,
            msg_iov: iov,
            msg_iovlen: 1,
            __pad1: 0,
            msg_control: core::ptr::null_mut(),
            msg_controllen: 0,
            __pad2: 0,
            msg_flags: 0,
        }
    }

    /// The address buffer and its length.
    #[cfg_attr(not(miri), expect(dead_code, reason = "read by the Miri stand-ins"))]
    pub(in crate::linux_rtnetlink) fn name(&self) -> (*mut c_void, socklen_t) {
        (self.msg_name, self.msg_namelen)
    }

    /// The data buffers and their number.
    #[cfg_attr(not(miri), expect(dead_code, reason = "read by the Miri stand-ins"))]
    pub(in crate::linux_rtnetlink) fn buffers(&self) -> (*mut iovec, usize) {
        (
            self.msg_iov,
            usize::try_from(self.msg_iovlen).unwrap_or(usize::MAX),
        )
    }

    /// The control data's length.
    #[cfg_attr(not(miri), expect(dead_code, reason = "read by the Miri stand-ins"))]
    pub(in crate::linux_rtnetlink) fn control_length(&self) -> usize {
        usize::try_from(self.msg_controllen).unwrap_or(usize::MAX)
    }

    /// The address length a receive returned.
    pub(in crate::linux_rtnetlink) fn name_length(&self) -> usize {
        usize::try_from(self.msg_namelen).unwrap_or(usize::MAX)
    }

    /// Sets what a receive returns: the address length and the flags.
    #[cfg_attr(not(miri), expect(dead_code, reason = "written by the Miri stand-ins"))]
    pub(in crate::linux_rtnetlink) fn set_returned(
        &mut self,
        name_length: socklen_t,
        flags: c_int,
    ) {
        self.msg_namelen = name_length;
        self.msg_flags = flags;
    }

    /// The flags a receive returned.
    pub(in crate::linux_rtnetlink) fn flags(&self) -> c_int {
        self.msg_flags
    }
}
