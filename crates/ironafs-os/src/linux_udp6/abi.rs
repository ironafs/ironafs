// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The C declarations of site `linux-udp6`, named as the C headers name
//! them, written from `socket(2)`, `IPV6_V6ONLY(2const)`, `bind(2)`,
//! `ipv6(7)`, `<sys/socket.h>` and `<netinet/in.h>`, and proved on each
//! Linux target by `cargo xtask check-abi` (ADR 0005, point 5.6), with
//! `foreign`.
//!
//! The structs have the same members, offsets and types in glibc and musl
//! on these targets, so each is one declaration, proved on each library.
//! `struct in6_addr` is a union in both (of 16 bytes, 8 shorts and 4
//! 32-bit words, so aligned to 4); it is declared by its 32-bit view,
//! `s6_addr32`, which both name under the default feature set, so its
//! alignment is C's. The constants have the same values in both libraries
//! and on both architectures.

use core::ffi::{c_char, c_int};

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

/// `in_port_t`: an unsigned 16-bit integer, in network byte order.
#[expect(
    non_camel_case_types,
    reason = "the C name, which the ABI check matches"
)]
pub(super) type in_port_t = u16;

/// `struct sockaddr`: the generic address `bind` takes a pointer to.
#[repr(C)]
#[derive(Debug)]
pub(super) struct sockaddr {
    pub(super) sa_family: sa_family_t,
    pub(super) sa_data: [c_char; 14],
}

/// `struct in6_addr`, by its four 32-bit words in memory order.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct in6_addr {
    pub(super) s6_addr32: [u32; 4],
}

/// `struct sockaddr_in6` (`ipv6(7)`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "the C names, which the ABI check matches"
)]
pub(super) struct sockaddr_in6 {
    pub(super) sin6_family: sa_family_t,
    pub(super) sin6_port: in_port_t,
    pub(super) sin6_flowinfo: u32,
    pub(super) sin6_addr: in6_addr,
    pub(super) sin6_scope_id: u32,
}

/// `AF_INET6`: the IPv6 address family.
pub(super) const AF_INET6: c_int = 10;

/// `SOCK_DGRAM`: a datagram socket.
pub(super) const SOCK_DGRAM: c_int = 2;

/// `SOCK_CLOEXEC`: the new descriptor is close-on-exec (Linux 2.6.27).
pub(super) const SOCK_CLOEXEC: c_int = 524_288;

/// `IPPROTO_IPV6`: the IPv6 option level.
pub(super) const IPPROTO_IPV6: c_int = 41;

/// `IPV6_V6ONLY`: the socket sends and receives IPv6 packets only.
pub(super) const IPV6_V6ONLY: c_int = 26;

/// `EAFNOSUPPORT`: the address family is not supported.
pub(super) const EAFNOSUPPORT: c_int = 97;

/// `EADDRINUSE`: the address is already in use.
pub(super) const EADDRINUSE: c_int = 98;

// The foreign functions, or under Miri, which cannot call them, the
// in-crate stand-ins of ADR 0005, point 6.5, which the same calls reach,
// so the wrapper's pointers, lengths and order run unchanged.
#[cfg(not(miri))]
mod foreign;
#[cfg(miri)]
pub(super) mod stand_in;

#[cfg(not(miri))]
pub(super) use foreign::{bind, setsockopt, socket};
#[cfg(miri)]
pub(super) use stand_in::{bind, setsockopt, socket};
