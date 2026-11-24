// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of `linux-rtnetlink` (ADR 0005, sheet 23) that Miri runs too,
//! through the calls' stand-ins: the group set, both sockets and a wake
//! made. Under Miri, `stand_in` also checks each call's arguments, every
//! reply the sheet names (a short sender address, a wrong family, another
//! port, a truncation, an empty datagram) and every wait outcome
//! ("interrupted", `POLLNVAL`, `POLLERR` on the wake, both entries ready).
//! The checks of each outcome are `route`'s; the tests that need the
//! running kernel are in `kernel`.

use super::abi::{
    RTMGRP_IPV4_IFADDR, RTMGRP_IPV4_ROUTE, RTMGRP_IPV4_RULE, RTMGRP_IPV6_IFADDR, RTMGRP_IPV6_ROUTE,
    RTMGRP_LINK, RTNLGRP_IPV6_RULE, RTNLGRP_NEXTHOP,
};
use super::{GROUPS, RouteSocket, Wake};

#[cfg(not(miri))]
mod kernel;
#[cfg(miri)]
mod stand_in;

#[test]
fn the_groups_are_links_both_families_addresses_routes_and_rules_and_nexthops() {
    // One bit per group, group 1 the lowest (`netlink(7)`).
    assert_eq!(RTMGRP_LINK, 1 << (1 - 1));
    assert_eq!(RTMGRP_IPV4_IFADDR, 1 << (5 - 1));
    assert_eq!(RTMGRP_IPV4_RULE, 1 << (8 - 1));
    assert_eq!(
        GROUPS,
        RTMGRP_LINK
            | RTMGRP_IPV4_IFADDR
            | RTMGRP_IPV4_ROUTE
            | RTMGRP_IPV4_RULE
            | RTMGRP_IPV6_IFADDR
            | RTMGRP_IPV6_ROUTE
            | (1 << (RTNLGRP_IPV6_RULE - 1))
            | (1 << (RTNLGRP_NEXTHOP - 1))
    );
    // The nexthop group is the last a 32-bit mask can carry.
    assert_eq!(GROUPS & (1 << 31), 1 << 31);
    assert_eq!(GROUPS.count_ones(), 8);
}

#[test]
fn a_notify_and_a_request_socket_are_made() {
    let notify = RouteSocket::notify().unwrap();
    let request = RouteSocket::request().unwrap();
    drop((notify, request));
}

#[test]
fn a_wake_is_made() {
    drop(Wake::new().unwrap());
}
