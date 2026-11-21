// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C declarations of site `linux-rtnetlink`, named as the C headers
//! name them, written from `netlink(7)`, `rtnetlink(7)`, `socket(2)`,
//! `bind(2)`, `send(2)`, `recvmsg(2)`, `<sys/socket.h>`, `<sys/uio.h>` and
//! the kernel's `<linux/netlink.h>` and `<linux/rtnetlink.h>` (which glibc
//! and musl both build on: neither declares `struct sockaddr_nl`), and
//! proved on each Linux target by `cargo xtask check-abi` (ADR 0005, point
//! 5.6), with `foreign` and the C library's own file. `struct msghdr` is
//! declared per C library (point 5.2), as `linux-fdpass`'s is; the other
//! structs and the constants are the same in both.
//!
//! The notify socket's groups are the owner's set (ADR 0005, sheet 23):
//! links, the addresses and routes of both families, and the policy rules
//! of both families. The kernel's header names six of them as `RTMGRP_*`
//! masks; the IPv6 rules have only their group number,
//! `RTNLGRP_IPV6_RULE`, whose mask bit is `1 << (number - 1)`
//! (`netlink(7)`: `nl_groups` is a bit mask, one bit per group).

use core::ffi::{c_char, c_int, c_ushort, c_void};

/// `socklen_t`: an unsigned 32-bit integer in glibc and musl.
#[expect(
    non_camel_case_types,
    reason = "the C name, which the ABI check matches"
)]
pub(super) type socklen_t = u32;

/// `sa_family_t`: an unsigned 16-bit integer in glibc and musl.
#[expect(
    non_camel_case_types,
    reason = "the C name, which the ABI check matches"
)]
pub(super) type sa_family_t = u16;

/// `struct sockaddr`: the generic address `bind` and `sendto` take a
/// pointer to.
#[repr(C)]
#[derive(Debug)]
pub(super) struct sockaddr {
    pub(super) sa_family: sa_family_t,
    pub(super) sa_data: [c_char; 14],
}

/// `struct sockaddr_nl` (`netlink(7)`; `<linux/netlink.h>` types the
/// port id `__u32`, where the manual page prints `pid_t`: the header is
/// what the check proves).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "the C names, which the ABI check matches"
)]
pub(super) struct sockaddr_nl {
    pub(super) nl_family: sa_family_t,
    pub(super) nl_pad: c_ushort,
    pub(super) nl_pid: u32,
    pub(super) nl_groups: u32,
}

/// `struct iovec`: one buffer of a message's data (`readv(2)`).
#[repr(C)]
#[derive(Debug)]
pub(super) struct iovec {
    pub(super) iov_base: *mut c_void,
    pub(super) iov_len: usize,
}

/// `AF_NETLINK`: the netlink address family.
pub(super) const AF_NETLINK: c_int = 16;

/// `SOCK_RAW`: netlink's socket type (`netlink(7)` takes `SOCK_RAW` and
/// `SOCK_DGRAM` alike).
pub(super) const SOCK_RAW: c_int = 3;

/// `SOCK_CLOEXEC`: the new descriptor is close-on-exec (Linux 2.6.27).
pub(super) const SOCK_CLOEXEC: c_int = 524_288;

/// `NETLINK_ROUTE`: the routing family (`rtnetlink(7)`).
pub(super) const NETLINK_ROUTE: c_int = 0;

/// `MSG_TRUNC`: in a receive's returned flags, the datagram was larger
/// than the buffer and its tail was discarded (`recvmsg(2)`).
pub(super) const MSG_TRUNC: c_int = 32;

/// `ENOBUFS`: the kernel dropped messages for lack of room
/// (`netlink(7)`).
pub(super) const ENOBUFS: c_int = 105;

/// `RTMGRP_LINK`: link changes.
pub(super) const RTMGRP_LINK: u32 = 1;

/// `RTMGRP_IPV4_IFADDR`: IPv4 address changes.
pub(super) const RTMGRP_IPV4_IFADDR: u32 = 16;

/// `RTMGRP_IPV4_ROUTE`: IPv4 route changes.
pub(super) const RTMGRP_IPV4_ROUTE: u32 = 64;

/// `RTMGRP_IPV4_RULE`: IPv4 policy rule changes.
pub(super) const RTMGRP_IPV4_RULE: u32 = 128;

/// `RTMGRP_IPV6_IFADDR`: IPv6 address changes.
pub(super) const RTMGRP_IPV6_IFADDR: u32 = 256;

/// `RTMGRP_IPV6_ROUTE`: IPv6 route changes.
pub(super) const RTMGRP_IPV6_ROUTE: u32 = 1024;

/// `RTNLGRP_IPV6_RULE`: the IPv6 policy rules' group number.
pub(super) const RTNLGRP_IPV6_RULE: u32 = 19;

// Each C library's `struct msghdr`. No site is in either file, so a musl
// build's sites are the glibc build's.
#[cfg(target_env = "gnu")]
mod gnu;
#[cfg(target_env = "musl")]
mod musl;

#[cfg(target_env = "gnu")]
pub(super) use gnu::msghdr;
#[cfg(target_env = "musl")]
pub(super) use musl::msghdr;

// The foreign functions, or under Miri, which cannot call them, the
// in-crate stand-ins of ADR 0005, point 6.5, which the same calls reach,
// so the wrapper's addresses, buffers and checks run unchanged.
#[cfg(not(miri))]
mod foreign;
#[cfg(miri)]
pub(super) mod stand_in;

#[cfg(not(miri))]
pub(super) use foreign::{bind, recvmsg, sendto, socket};
#[cfg(miri)]
pub(super) use stand_in::{bind, recvmsg, sendto, socket};
