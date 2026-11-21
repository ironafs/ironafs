// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tests of `linux-udp6` (ADR 0005, sheet 21) that need the running
//! kernel, so Miri does not build them. Each runs in whatever network
//! namespace the test runs in, under its `net.ipv6.bindv6only` (the guest
//! runs them at 0 and at 1): an IPv4 socket and this socket share a port,
//! in either order, which only `IPV6_V6ONLY` allows at 0; a port held by
//! another IPv6 socket is `InUse`; a scoped address opens no descriptor;
//! the socket is close-on-exec and carries datagrams. An address family
//! denied by a sandbox is an ignored test, run under a filter that denies
//! `AF_INET6`.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::os::fd::AsRawFd;

use super::super::udp6_socket;
use crate::udp6::Udp6Error;

/// `O_CLOEXEC` in `/proc/self/fdinfo`'s octal `flags`.
const O_CLOEXEC: u32 = 0o2_000_000;

/// The number of this process's open descriptors.
fn open_descriptors() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

/// The port `socket` is bound to.
fn port(socket: &UdpSocket) -> u16 {
    socket.local_addr().unwrap().port()
}

/// The value of `net.ipv6.bindv6only` here, for the failure message.
fn bindv6only() -> String {
    std::fs::read_to_string("/proc/sys/net/ipv6/bindv6only").unwrap_or_default()
}

#[test]
fn an_ipv4_socket_bound_first_shares_its_port_with_this_one() {
    let v4 = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    let shared = port(&v4);
    let v6 = udp6_socket(Ipv6Addr::UNSPECIFIED, shared);
    assert!(v6.is_ok(), "bindv6only {}: {v6:?}", bindv6only());
}

#[test]
fn this_socket_bound_first_shares_its_port_with_an_ipv4_socket() {
    let v6 = udp6_socket(Ipv6Addr::UNSPECIFIED, 0).unwrap();
    let v4 = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port(&v6)));
    assert!(v4.is_ok(), "bindv6only {}: {v4:?}", bindv6only());
}

#[test]
fn a_port_held_by_another_ipv6_socket_is_in_use_and_nothing_stays_open() {
    let held = UdpSocket::bind((Ipv6Addr::UNSPECIFIED, 0)).unwrap();
    let before = open_descriptors();
    match udp6_socket(Ipv6Addr::UNSPECIFIED, port(&held)) {
        Err(Udp6Error::InUse(error)) => assert_eq!(error.raw_os_error(), Some(98)),
        other => panic!("{other:?}"),
    }
    assert_eq!(open_descriptors(), before);
}

#[test]
fn a_scoped_address_opens_no_descriptor() {
    let before = open_descriptors();
    assert!(matches!(
        udp6_socket("fe80::1".parse().unwrap(), 0),
        Err(Udp6Error::Scoped(_))
    ));
    assert_eq!(open_descriptors(), before);
}

#[test]
fn the_socket_is_close_on_exec_and_carries_datagrams() {
    let receiver = udp6_socket(Ipv6Addr::LOCALHOST, 0).unwrap();
    let sender = udp6_socket(Ipv6Addr::LOCALHOST, 0).unwrap();
    for socket in [&receiver, &sender] {
        let info =
            std::fs::read_to_string(format!("/proc/self/fdinfo/{}", socket.as_raw_fd())).unwrap();
        let flags = info
            .lines()
            .find_map(|line| line.strip_prefix("flags:"))
            .map(|flags| u32::from_str_radix(flags.trim(), 8).unwrap())
            .unwrap();
        assert_eq!(flags & O_CLOEXEC, O_CLOEXEC, "{info}");
    }
    sender
        .send_to(b"ping", receiver.local_addr().unwrap())
        .unwrap();
    let mut buffer = [0_u8; 8];
    let (count, from) = receiver.recv_from(&mut buffer).unwrap();
    assert_eq!(&buffer[..count], b"ping");
    assert_eq!(from, sender.local_addr().unwrap());
}

#[test]
fn the_socket_takes_no_ipv4_traffic() {
    let socket = udp6_socket(Ipv6Addr::UNSPECIFIED, 0).unwrap();
    let mapped = SocketAddr::from((Ipv4Addr::LOCALHOST.to_ipv6_mapped(), 9));
    assert!(socket.send_to(b"x", mapped).is_err());
}

#[test]
#[ignore = "needs a sandbox that denies AF_INET6 sockets with EAFNOSUPPORT: run under such a filter"]
fn a_sandbox_that_denies_the_family_is_unsupported() {
    match udp6_socket(Ipv6Addr::LOCALHOST, 0) {
        Err(Udp6Error::Unsupported(error)) => assert_eq!(error.raw_os_error(), Some(97)),
        other => panic!("{other:?}"),
    }
}
