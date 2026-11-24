// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of which addresses an IPv6-only UDP socket takes and how its
//! errors are told apart (ADR 0005, sheet 21).

use std::io;
use std::net::Ipv6Addr;

use super::{Rules, Udp6Error, bind_error, needs_zone, socket_error};

const RULES: Rules = Rules {
    unsupported: 97,
    in_use: 98,
};

#[test]
fn needs_zone_names_link_local_unicast_and_link_scoped_multicast() {
    for scoped in [
        "fe80::1",
        "fe80::",
        "febf:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
        "ff01::1",
        "ff02::1",
        "ff12::1",
        "ff32::fb",
    ] {
        assert!(needs_zone(&scoped.parse().unwrap()), "{scoped}");
    }
    for unscoped in [
        "::",
        "::1",
        "2001:db8::1",
        "fec0::1",
        "fc00::1",
        "fe7f:ffff::1",
        "ff05::1",
        "ff0e::1",
        "::ffff:127.0.0.1",
    ] {
        assert!(!needs_zone(&unscoped.parse().unwrap()), "{unscoped}");
    }
    assert!(!needs_zone(&Ipv6Addr::UNSPECIFIED));
}

#[test]
fn socket_errors_tell_a_missing_family_apart() {
    assert!(matches!(
        socket_error(RULES, io::Error::from_raw_os_error(97)),
        Udp6Error::Unsupported(error) if error.raw_os_error() == Some(97)
    ));
    assert!(matches!(
        socket_error(RULES, io::Error::from_raw_os_error(24)),
        Udp6Error::Socket(error) if error.raw_os_error() == Some(24)
    ));
}

#[test]
fn bind_errors_tell_an_address_in_use_apart() {
    assert!(matches!(
        bind_error(RULES, io::Error::from_raw_os_error(98)),
        Udp6Error::InUse(error) if error.raw_os_error() == Some(98)
    ));
    assert!(matches!(
        bind_error(RULES, io::Error::from_raw_os_error(99)),
        Udp6Error::Bind(error) if error.raw_os_error() == Some(99)
    ));
}
