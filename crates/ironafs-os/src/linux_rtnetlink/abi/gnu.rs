// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! glibc's `struct msghdr`, `struct pollfd` and `nfds_t` (site
//! `linux-rtnetlink`): `msg_iovlen` and `msg_controllen` are `size_t`
//! (`recvmsg(2)`, glibc's `<bits/socket.h>`); `nfds_t` is `unsigned long
//! int` (glibc's `<sys/poll.h>`).

use core::ffi::{c_int, c_short, c_ulong, c_void};

use super::{iovec, socklen_t};

/// `struct msghdr`: one message's address, data buffers and control
/// data.
#[repr(C)]
#[derive(Debug)]
#[expect(
    clippy::struct_field_names,
    reason = "the C names, which the ABI check matches"
)]
pub(in crate::linux_rtnetlink) struct msghdr {
    msg_name: *mut c_void,
    msg_namelen: socklen_t,
    msg_iov: *mut iovec,
    msg_iovlen: usize,
    msg_control: *mut c_void,
    msg_controllen: usize,
    msg_flags: c_int,
}

impl msghdr {
    /// A message whose sender's address goes to `name`, of `name_length`
    /// bytes, and whose data goes to the one buffer at `iov`, with no
    /// control data; every other field zero.
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
            msg_control: core::ptr::null_mut(),
            msg_controllen: 0,
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
        (self.msg_iov, self.msg_iovlen)
    }

    /// The control data's length.
    #[cfg_attr(not(miri), expect(dead_code, reason = "read by the Miri stand-ins"))]
    pub(in crate::linux_rtnetlink) fn control_length(&self) -> usize {
        self.msg_controllen
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

/// `nfds_t`: the number of entries `poll` is given (glibc's `unsigned long int`).
#[expect(
    non_camel_case_types,
    reason = "the C name, which the ABI check matches"
)]
pub(in crate::linux_rtnetlink) type nfds_t = c_ulong;

/// `struct pollfd`: one descriptor `poll` waits on, the events asked for,
/// and the events it reported (`poll(2)`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(in crate::linux_rtnetlink) struct pollfd {
    pub(in crate::linux_rtnetlink) fd: c_int,
    pub(in crate::linux_rtnetlink) events: c_short,
    pub(in crate::linux_rtnetlink) revents: c_short,
}
