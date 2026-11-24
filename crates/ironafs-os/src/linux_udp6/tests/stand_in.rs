// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tests of `linux-udp6` (ADR 0005, sheet 21) that only the Miri
//! stand-ins can run: they record each call, so these check that the
//! option is set to 1 before `bind`, that a failed option leaves the
//! socket unbound, and that a scoped address makes no call.

use std::net::Ipv6Addr;

use super::super::abi::stand_in::{Call, Fail, calls, fail};
use super::super::abi::{AF_INET6, IPPROTO_IPV6, IPV6_V6ONLY, SOCK_CLOEXEC, SOCK_DGRAM};
use super::super::{name, udp6_socket};
use crate::udp6::Udp6Error;

#[test]
fn the_calls_are_socket_then_ipv6_only_set_to_one_then_bind() {
    let address: Ipv6Addr = "2001:db8::7".parse().unwrap();
    let _ = calls();
    udp6_socket(address, 853).unwrap();
    assert_eq!(
        calls(),
        [
            Call::Socket(AF_INET6, SOCK_DGRAM | SOCK_CLOEXEC, 0),
            Call::Option(IPPROTO_IPV6, IPV6_V6ONLY, 1, 4),
            Call::Bind(name(address, 853), 28),
        ]
    );
}

#[test]
fn a_failed_option_leaves_the_socket_unbound() {
    let _ = calls();
    fail(Fail::Option, 22);
    assert!(matches!(
        udp6_socket(Ipv6Addr::LOCALHOST, 0),
        Err(Udp6Error::IpV6Only(_))
    ));
    assert!(!calls().iter().any(|call| matches!(call, Call::Bind(..))));
}

#[test]
fn a_planned_failure_fails_one_call_and_the_next_succeeds() {
    let _ = calls();
    fail(Fail::Bind, 98);
    assert!(matches!(
        udp6_socket(Ipv6Addr::LOCALHOST, 0),
        Err(Udp6Error::InUse(_))
    ));
    udp6_socket(Ipv6Addr::LOCALHOST, 0).unwrap();
}

#[test]
fn a_missing_family_and_an_address_in_use_are_told_apart_through_the_calls() {
    let _ = calls();
    fail(Fail::Socket, 97);
    assert!(matches!(
        udp6_socket(Ipv6Addr::LOCALHOST, 0),
        Err(Udp6Error::Unsupported(error)) if error.raw_os_error() == Some(97)
    ));
    assert_eq!(calls().len(), 1);
    fail(Fail::Socket, 24);
    assert!(matches!(
        udp6_socket(Ipv6Addr::LOCALHOST, 0),
        Err(Udp6Error::Socket(error)) if error.raw_os_error() == Some(24)
    ));
    let _ = calls();
    fail(Fail::Bind, 98);
    assert!(matches!(
        udp6_socket(Ipv6Addr::LOCALHOST, 0),
        Err(Udp6Error::InUse(error)) if error.raw_os_error() == Some(98)
    ));
    assert_eq!(calls().len(), 3);
    fail(Fail::Bind, 99);
    assert!(matches!(
        udp6_socket(Ipv6Addr::LOCALHOST, 0),
        Err(Udp6Error::Bind(error)) if error.raw_os_error() == Some(99)
    ));
}

#[test]
fn a_scoped_address_makes_no_call() {
    let _ = calls();
    assert!(udp6_socket("fe80::1".parse().unwrap(), 0).is_err());
    assert_eq!(calls(), []);
}
