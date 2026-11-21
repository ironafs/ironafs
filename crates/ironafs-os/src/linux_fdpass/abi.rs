// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C declarations of site `linux-fdpass`, named as the C headers name
//! them, written from `unix(7)`, `cmsg(3)`, `recvmsg(2)`, `sendmsg(2)`,
//! `<sys/socket.h>` and `<sys/uio.h>`, and proved on each Linux target by
//! `cargo xtask check-abi` (ADR 0005, point 5.6), with `foreign` and the C
//! library's own file. `struct msghdr` and `struct cmsghdr` are declared
//! per C library (point 5.2): glibc's lengths are `size_t`, musl's are
//! `int` and `socklen_t` beside `int` padding fields, at the same sizes
//! and offsets on these little-endian 64-bit targets. The constants have
//! the same values in both libraries and on both architectures; the
//! `CMSG_*` values are compared with the headers' macros themselves
//! (sites.json's `expressions`).

use core::ffi::{c_int, c_void};

/// `socklen_t`: an unsigned 32-bit integer in glibc and musl.
#[expect(
    non_camel_case_types,
    reason = "the C name, which the ABI check matches"
)]
pub(super) type socklen_t = u32;

/// `struct iovec`: one buffer of a message's data (`readv(2)`).
#[repr(C)]
#[derive(Debug)]
pub(super) struct iovec {
    pub(super) iov_base: *mut c_void,
    pub(super) iov_len: usize,
}

/// `SOL_SOCKET`: the socket level, of the control message.
pub(super) const SOL_SOCKET: c_int = 1;

/// `SCM_RIGHTS`: a control message of descriptors.
pub(super) const SCM_RIGHTS: c_int = 1;

/// `MSG_NOSIGNAL`: a send to a closed peer fails with `EPIPE` and raises
/// no `SIGPIPE`.
pub(super) const MSG_NOSIGNAL: c_int = 16_384;

/// `MSG_CMSG_CLOEXEC`: received descriptors are close-on-exec (Linux
/// 2.6.23).
pub(super) const MSG_CMSG_CLOEXEC: c_int = 1_073_741_824;

/// `MSG_CTRUNC`: control data was discarded for lack of room.
pub(super) const MSG_CTRUNC: c_int = 8;

/// `CMSG_LEN(sizeof(int))`: the header length of one descriptor.
pub(super) const CMSG_LEN_ONE: usize = 20;

/// `CMSG_LEN(2 * sizeof(int))`: the header length of two descriptors.
pub(super) const CMSG_LEN_TWO: usize = 24;

/// `CMSG_SPACE(sizeof(int))`: what a control message of one descriptor
/// occupies.
pub(super) const CMSG_SPACE_ONE: usize = 24;

// Each C library's `struct msghdr` and `struct cmsghdr`. No site is in
// either file, so a musl build's sites are the glibc build's.
#[cfg(target_env = "gnu")]
mod gnu;
#[cfg(target_env = "musl")]
mod musl;

#[cfg(target_env = "gnu")]
pub(super) use gnu::{cmsghdr, msghdr};
#[cfg(target_env = "musl")]
pub(super) use musl::{cmsghdr, msghdr};

// The foreign functions, or under Miri, which cannot call them, the
// in-crate stand-ins of ADR 0005, point 6.5, which the same calls reach,
// so the wrappers' buffers, pointers and checks run unchanged.
#[cfg(not(miri))]
mod foreign;
#[cfg(miri)]
mod stand_in;

#[cfg(not(miri))]
pub(super) use foreign::{recvmsg, sendmsg};
#[cfg(miri)]
pub(super) use stand_in::{recvmsg, sendmsg};
