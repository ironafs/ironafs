// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! glibc's `struct msghdr` and `struct cmsghdr` (site `linux-fdpass`):
//! `msg_iovlen`, `msg_controllen` and `cmsg_len` are `size_t`
//! (`recvmsg(2)`, `cmsg(3)`, glibc's `<bits/socket.h>`).

use core::ffi::{c_int, c_void};

use super::{iovec, socklen_t};

/// `struct msghdr`: one message's address, data buffers and control
/// data.
#[repr(C)]
#[derive(Debug)]
#[expect(
    clippy::struct_field_names,
    reason = "the C names, which the ABI check matches"
)]
pub(in crate::linux_fdpass) struct msghdr {
    msg_name: *mut c_void,
    msg_namelen: socklen_t,
    msg_iov: *mut iovec,
    msg_iovlen: usize,
    msg_control: *mut c_void,
    msg_controllen: usize,
    msg_flags: c_int,
}

/// `struct cmsghdr`: a control message's header; its data follows at
/// `CMSG_ALIGN(sizeof(struct cmsghdr))`.
#[repr(C)]
#[derive(Debug)]
#[expect(
    clippy::struct_field_names,
    reason = "the C names, which the ABI check matches"
)]
pub(in crate::linux_fdpass) struct cmsghdr {
    cmsg_len: usize,
    cmsg_level: c_int,
    cmsg_type: c_int,
}

impl msghdr {
    /// A message with no address, one data buffer at `iov` and
    /// `control_length` bytes of control data at `control`, every other
    /// field (padding included) zero. `None` if the length does not fit
    /// the field.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "musl's has this signature, and its lengths can overflow their fields"
    )]
    pub(in crate::linux_fdpass) fn new(
        iov: *mut iovec,
        control: *mut c_void,
        control_length: usize,
    ) -> Option<Self> {
        Some(Self {
            msg_name: core::ptr::null_mut(),
            msg_namelen: 0,
            msg_iov: iov,
            msg_iovlen: 1,
            msg_control: control,
            msg_controllen: control_length,
            msg_flags: 0,
        })
    }

    /// The data buffers and their number.
    #[cfg_attr(not(miri), expect(dead_code, reason = "read by the Miri stand-ins"))]
    pub(in crate::linux_fdpass) fn buffers(&self) -> (*mut iovec, usize) {
        (self.msg_iov, self.msg_iovlen)
    }

    /// The control data's address.
    #[cfg_attr(not(miri), expect(dead_code, reason = "read by the Miri stand-ins"))]
    pub(in crate::linux_fdpass) fn control(&self) -> *mut c_void {
        self.msg_control
    }

    /// The control length: the buffer's size before a receive, the
    /// control data's length after it. `usize::MAX` if it does not fit,
    /// which every check refuses.
    pub(in crate::linux_fdpass) fn control_length(&self) -> usize {
        self.msg_controllen
    }

    /// Sets the control length, as a receive does; `None` if it does not
    /// fit the field.
    #[cfg_attr(not(miri), expect(dead_code, reason = "written by the Miri stand-ins"))]
    #[expect(
        clippy::unnecessary_wraps,
        reason = "musl's has this signature, and its lengths can overflow their fields"
    )]
    pub(in crate::linux_fdpass) fn set_control_length(&mut self, length: usize) -> Option<()> {
        self.msg_controllen = length;
        Some(())
    }

    /// The flags a receive returned.
    pub(in crate::linux_fdpass) fn flags(&self) -> c_int {
        self.msg_flags
    }
}

impl cmsghdr {
    /// A header of `length` bytes (header and data) at `level`, of
    /// `kind`, padding zero; `None` if the length does not fit the field.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "musl's has this signature, and its lengths can overflow their fields"
    )]
    pub(in crate::linux_fdpass) fn new(length: usize, level: c_int, kind: c_int) -> Option<Self> {
        Some(Self {
            cmsg_len: length,
            cmsg_level: level,
            cmsg_type: kind,
        })
    }

    /// The header's length, `usize::MAX` if it does not fit.
    pub(in crate::linux_fdpass) fn length(&self) -> usize {
        self.cmsg_len
    }

    /// The header's level.
    pub(in crate::linux_fdpass) fn level(&self) -> c_int {
        self.cmsg_level
    }

    /// The header's type.
    pub(in crate::linux_fdpass) fn kind(&self) -> c_int {
        self.cmsg_type
    }
}
