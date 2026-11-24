// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of `linux-udp6` (ADR 0005, sheet 21) that Miri runs too, through
//! the calls' stand-ins: the address the wrapper builds, a socket made
//! and returned, and a scoped address refused. Under Miri, `stand_in`
//! also checks the calls' order and arguments and each failure path. The
//! tests that need the running kernel are in `kernel`.

use std::net::Ipv6Addr;

use super::abi::{AF_INET6, in6_addr};
use super::{name, udp6_socket};
use crate::udp6::Udp6Error;

#[cfg(not(miri))]
mod kernel;
#[cfg(miri)]
mod stand_in;

#[test]
fn the_address_carries_the_family_the_port_in_network_order_and_no_scope() {
    let address: Ipv6Addr = "2001:db8::102:304".parse().unwrap();
    let built = name(address, 0x1234);
    assert_eq!(i32::from(built.sin6_family), AF_INET6);
    assert_eq!(built.sin6_port.to_ne_bytes(), [0x12, 0x34]);
    assert_eq!(built.sin6_flowinfo, 0);
    assert_eq!(built.sin6_scope_id, 0);
    let in6_addr { s6_addr32: words } = built.sin6_addr;
    let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_ne_bytes()).collect();
    assert_eq!(bytes, address.octets());
}

#[test]
fn a_socket_is_made_bound_and_returned() {
    let socket = udp6_socket(Ipv6Addr::LOCALHOST, 0).unwrap();
    drop(socket);
}

#[test]
fn an_address_that_needs_a_zone_is_refused() {
    for scoped in ["fe80::1", "ff02::1"] {
        let address: Ipv6Addr = scoped.parse().unwrap();
        assert!(
            matches!(udp6_socket(address, 0), Err(Udp6Error::Scoped(refused)) if refused == address),
            "{scoped}"
        );
    }
}
