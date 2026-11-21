// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Site `linux-udp6` (ADR 0005, row 21 and sheet 21): an IPv6-only,
//! close-on-exec UDP socket, bound to an unscoped address and a port, as a
//! `std::net::UdpSocket`. `std` binds as it creates and cannot set
//! `IPV6_V6ONLY` first.
//!
//! The order is fixed: `socket`, whose descriptor is adopted as an
//! `OwnedFd` at once, so every later error path closes it; `setsockopt`
//! with `IPV6_V6ONLY` set to 1, whose failure closes the descriptor
//! unbound; then `bind`. With the option set the socket never takes IPv4
//! traffic, so it can share a port with an IPv4 socket whatever
//! `net.ipv6.bindv6only` says (`IPV6_V6ONLY(2const)`). The address is
//! built here, scope id zero, so an address that needs an interface zone
//! is refused before any call. What `Unsupported` and `InUse` mean is the
//! caller's (ADR 0004, C34).

mod abi;

use core::ffi::{c_int, c_void};
use std::io;
use std::net::{Ipv6Addr, UdpSocket};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

use crate::udp6::{Rules, Udp6Error, bind_error, needs_zone, socket_error};
use abi::{
    AF_INET6, EADDRINUSE, EAFNOSUPPORT, IPPROTO_IPV6, IPV6_V6ONLY, OPTION_LENGTH, SOCK_CLOEXEC,
    SOCK_DGRAM, SOCKADDR_IN6_SIZE, in6_addr, sa_family_t, sockaddr, sockaddr_in6, socklen_t,
};

#[cfg(test)]
mod tests;

/// The errno values the errors are told apart by.
const RULES: Rules = Rules {
    unsupported: EAFNOSUPPORT,
    in_use: EADDRINUSE,
};

/// The socket's type: a datagram socket, close-on-exec.
const KIND: c_int = SOCK_DGRAM | SOCK_CLOEXEC;

/// `IPV6_V6ONLY`'s value: on.
const ON: c_int = 1;

/// The option value's length, `sizeof(int)`.
const ON_LENGTH: socklen_t = OPTION_LENGTH;

/// The address's length, `sizeof(struct sockaddr_in6)`.
const NAME_LENGTH: socklen_t = SOCKADDR_IN6_SIZE;

/// `AF_INET6` as the address's family field.
const FAMILY: sa_family_t = 10;

const _: () = assert!(
    ON_LENGTH as usize == size_of::<c_int>()
        && NAME_LENGTH as usize == size_of::<sockaddr_in6>()
        && FAMILY as c_int == AF_INET6
);

/// An IPv6-only, close-on-exec UDP socket bound to `address` and `port`
/// (port 0: one the system chooses).
///
/// # Errors
///
/// [`Udp6Error::Scoped`] for an address that needs an interface zone,
/// before any call; [`Udp6Error::Unsupported`] if the system offers no
/// IPv6 sockets; [`Udp6Error::InUse`] if the address and port are taken;
/// and [`Udp6Error::Socket`], [`Udp6Error::IpV6Only`] or
/// [`Udp6Error::Bind`], with the errno, for any other failure. On each,
/// no descriptor is left open.
///
/// ```
/// let socket = ironafs_os::udp6_socket(std::net::Ipv6Addr::LOCALHOST, 0)?;
/// assert!(socket.local_addr()?.is_ipv6());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn udp6_socket(address: Ipv6Addr, port: u16) -> Result<UdpSocket, Udp6Error> {
    if needs_zone(&address) {
        return Err(Udp6Error::Scoped(address));
    }
    let socket = open().map_err(|error| socket_error(RULES, error))?;
    restrict(socket.as_fd()).map_err(Udp6Error::IpV6Only)?;
    let name = name(address, port);
    bind_to(socket.as_fd(), &name).map_err(|error| bind_error(RULES, error))?;
    Ok(UdpSocket::from(socket))
}

/// The `struct sockaddr_in6` of `address` and `port`: the family, the port
/// in network byte order, the address's bytes in order, and zero flow
/// information and scope id.
fn name(address: Ipv6Addr, port: u16) -> sockaddr_in6 {
    let octets = address.octets();
    let (quads, _) = octets.as_chunks::<4>();
    let mut words = [0_u32; 4];
    for (word, quad) in words.iter_mut().zip(quads) {
        *word = u32::from_ne_bytes(*quad);
    }
    sockaddr_in6 {
        sin6_family: FAMILY,
        sin6_port: port.to_be(),
        sin6_flowinfo: 0,
        sin6_addr: in6_addr { s6_addr32: words },
        sin6_scope_id: 0,
    }
}

/// A new IPv6 datagram socket, close-on-exec, owned.
fn open() -> io::Result<OwnedFd> {
    let domain: c_int = AF_INET6;
    let kind: c_int = KIND;
    let protocol: c_int = 0;
    #[expect(unsafe_code, reason = "linux-udp6")]
    // SAFETY: `socket(2)` takes three integers and reads no memory of
    // ours; any values are answered with a new descriptor or -1 and
    // errno, never undefined behaviour. The call shares no state with
    // other threads. The declaration is in `abi`, proved by the ABI check.
    let descriptor = unsafe { abi::socket(domain, kind, protocol) };
    if descriptor < 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(abi::last_error());
    }
    Ok(adopt(descriptor))
}

/// Takes ownership of the descriptor `socket` returned.
fn adopt(descriptor: c_int) -> OwnedFd {
    #[expect(unsafe_code, reason = "linux-udp6")]
    // SAFETY: `OwnedFd::from_raw_fd` requires an open descriptor that
    // nothing else owns or will close. `open` passes only the non-negative
    // value `socket` just returned: a new descriptor of this process, which
    // no other code has seen, passed once. So the `OwnedFd` is its only
    // owner, and dropping it closes it.
    unsafe {
        OwnedFd::from_raw_fd(descriptor)
    }
}

/// Sets `IPV6_V6ONLY` to 1 on `socket`.
fn restrict(socket: BorrowedFd<'_>) -> io::Result<()> {
    let descriptor: c_int = socket.as_raw_fd();
    let level: c_int = IPPROTO_IPV6;
    let option: c_int = IPV6_V6ONLY;
    let on: c_int = ON;
    let value: *const c_void = (&raw const on).cast();
    let length: socklen_t = ON_LENGTH;
    #[expect(unsafe_code, reason = "linux-udp6")]
    // SAFETY: `setsockopt(2)` with `IPPROTO_IPV6` and `IPV6_V6ONLY` reads
    // `optlen` bytes at `optval`, an `int` (`IPV6_V6ONLY(2const)`), and
    // writes nothing of ours. `value` points to `on`, a local `c_int`
    // holding 1, live and unchanged for the whole call, and `length` is
    // `sizeof(int)`, so the call reads only memory it may read, and
    // nothing keeps the pointer afterwards. `descriptor` is borrowed from
    // the caller's `OwnedFd` for the call, so it stays open. A failure is
    // -1 and errno, never undefined behaviour. The call shares no state
    // with other threads. The declaration is in `abi`, proved by the ABI
    // check.
    let status = unsafe { abi::setsockopt(descriptor, level, option, value, length) };
    if status != 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(abi::last_error());
    }
    Ok(())
}

/// Binds `socket` to `name`.
fn bind_to(socket: BorrowedFd<'_>, name: &sockaddr_in6) -> io::Result<()> {
    let descriptor: c_int = socket.as_raw_fd();
    let address: *const sockaddr = (&raw const *name).cast();
    let length: socklen_t = NAME_LENGTH;
    #[expect(unsafe_code, reason = "linux-udp6")]
    // SAFETY: `bind(2)` reads `addrlen` bytes at `addr`, here one `struct
    // sockaddr_in6` (`ipv6(7)`), whose family field it reads first, and
    // writes nothing of ours. `address` points to `name`, borrowed from
    // the caller's local for the call, and `length` is its size, so the
    // call reads only memory it may read, and nothing keeps the pointer
    // afterwards. The cast to `struct sockaddr *` is how `bind`'s callers
    // pass every address (`bind(2)`): the kernel reads the bytes by the
    // family. `descriptor` is borrowed from the caller's `OwnedFd` for the
    // call. A failure is -1 and errno, never undefined behaviour. The call
    // shares no state with other threads. The declaration is in `abi`,
    // proved by the ABI check.
    let status = unsafe { abi::bind(descriptor, address, length) };
    if status != 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(abi::last_error());
    }
    Ok(())
}
