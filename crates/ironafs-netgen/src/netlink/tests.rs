// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The route-netlink decoder against datagrams written from the layout of
//! docs/specs/network-generation.md, 5.4: golden datagrams, the reject
//! corpus of 5.6, and a seeded sweep of truncations and changed bytes.

use alloc::vec::Vec;

use super::{Address, AddressKey, Dump, Event, Link, Malformed, dump_request, event, messages};
use crate::netlink::build::{self, AF_INET, AF_INET6};

/// Every event of a datagram, or the first error.
fn events(datagram: &[u8]) -> Result<Vec<Event>, Malformed> {
    let mut found = Vec::new();
    for message in messages(datagram) {
        found.push(event(&message?)?);
    }
    Ok(found)
}

// Golden bytes, written by hand from 5.4 (little-endian hosts: the
// x86_64 and aarch64 targets).

/// `RTM_NEWADDR`, 48 bytes: 192.0.2.1/24 on link 2, flags 0x80
/// (permanent), scope 0, with `IFA_ADDRESS` and `IFA_LOCAL`.
#[cfg(target_endian = "little")]
const NEW_ADDRESS_V4: [u8; 40] = [
    40, 0, 0, 0, // length
    20, 0, // RTM_NEWADDR
    0, 0, // flags
    7, 0, 0, 0, // sequence
    0, 0, 0, 0, // port id
    2, 24, 0x80, 0, // family AF_INET, prefix 24, flags, scope
    2, 0, 0, 0, // index 2
    8, 0, 1, 0, 192, 0, 2, 1, // IFA_ADDRESS
    8, 0, 2, 0, 192, 0, 2, 1, // IFA_LOCAL
];

/// `RTM_NEWLINK`, 40 bytes: link 3, flags `UP`, `RUNNING`, `LOWER_UP` plus
/// BROADCAST, `IFLA_OPERSTATE` 6 (up) padded to 8, and an unknown
/// attribute 3 (the name) skipped by length.
#[cfg(target_endian = "little")]
const NEW_LINK: [u8; 48] = [
    48, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // header
    0, 0, 1, 0, // family, pad, type 1 (ether)
    3, 0, 0, 0, // index 3
    0x43, 0x00, 0x01, 0x00, // flags: UP | BROADCAST | RUNNING | LOWER_UP
    0, 0, 0, 0, // change mask
    5, 0, 16, 0, 6, 0, 0, 0, // IFLA_OPERSTATE = 6, three bytes of padding
    8, 0, 3, 0, b'e', b't', b'h', 0, // IFLA_IFNAME "eth"
];

/// `NLMSG_DONE` with `NLM_F_MULTI | NLM_F_DUMP_INTR`, and a 4-byte body.
#[cfg(target_endian = "little")]
const DONE_INTERRUPTED: [u8; 20] = [
    20, 0, 0, 0, 3, 0, 0x12, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[test]
#[cfg(target_endian = "little")]
fn golden_address_decodes_its_local_address_and_flags() {
    let mut local = [0_u8; 16];
    local[..4].copy_from_slice(&[192, 0, 2, 1]);
    assert_eq!(
        events(&NEW_ADDRESS_V4),
        Ok(alloc::vec![Event::Address(Address {
            key: AddressKey {
                family: AF_INET,
                index: 2,
                prefix_length: 24,
                address: local,
            },
            flags: 0x80,
            scope: 0,
        })])
    );
    let message = messages(&NEW_ADDRESS_V4).next().unwrap().unwrap();
    assert_eq!(message.sequence, 7);
    assert!(!message.interrupted());
}

#[test]
#[cfg(target_endian = "little")]
fn golden_link_keeps_only_its_usability_and_skips_unknown_attributes() {
    assert_eq!(
        events(&NEW_LINK),
        Ok(alloc::vec![Event::Link(Link {
            index: 3,
            usability: 0x0001_0041,
            operstate: Some(6),
        })])
    );
}

#[test]
#[cfg(target_endian = "little")]
fn golden_done_reports_an_interrupted_dump() {
    let message = messages(&DONE_INTERRUPTED).next().unwrap().unwrap();
    assert!(message.interrupted());
    assert_eq!(message.sequence, 9);
    assert_eq!(event(&message), Ok(Event::Done));
}

#[test]
#[cfg(target_endian = "little")]
fn dump_request_is_the_header_and_a_padded_rtgenmsg() {
    assert_eq!(
        dump_request(Dump::Links, 5),
        [
            20, 0, 0, 0, 18, 0, 0x01, 0x03, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]
    );
    assert_eq!(
        dump_request(Dump::Addresses, 0x0102_0304),
        [
            20, 0, 0, 0, 22, 0, 0x01, 0x03, 4, 3, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0
        ]
    );
}

#[test]
fn ifa_flags_replaces_the_one_byte_flags_and_ifa_local_wins() {
    let datagram = build::address(20, AF_INET6, 3, 64, 0x20, |body| {
        build::attribute(body, 1, &[0xfe; 16]);
        build::attribute(body, 8, &0x0000_0220_u32.to_ne_bytes());
        build::attribute(
            body,
            2,
            &[0x20, 1, 0xd, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        );
    });
    let found = events(&datagram).unwrap();
    let [Event::Address(address)] = found.as_slice() else {
        panic!("{found:?}");
    };
    assert_eq!(address.flags, 0x220);
    assert_eq!(address.key.address[..2], [0x20, 1]);
    assert_eq!(address.key.family, AF_INET6);
}

#[test]
fn removed_link_and_address_route_rule_nexthop_and_control_types_are_told_apart() {
    let mut datagram = Vec::new();
    datagram.extend(build::link(17, 4, 0, |_| {}));
    datagram.extend(build::address(21, AF_INET, 4, 8, 0, |body| {
        build::attribute(body, 1, &[10, 0, 0, 1]);
    }));
    for kind in [24, 25, 32, 33, 104, 105] {
        datagram.extend(build::message(kind, 0, 0, &[0; 12]));
    }
    datagram.extend(build::message(2, 0, 0, &(-105_i32).to_ne_bytes()));
    datagram.extend(build::message(4, 0, 0, &[]));
    datagram.extend(build::message(1, 0, 0, &[]));
    datagram.extend(build::message(99, 0, 0, &[1, 2, 3]));
    let found = events(&datagram).unwrap();
    assert_eq!(found.len(), 12);
    assert_eq!(found[0], Event::LinkGone(4));
    assert!(matches!(
        found[1],
        Event::AddressGone(Address {
            key: AddressKey {
                index: 4,
                prefix_length: 8,
                ..
            },
            ..
        })
    ));
    assert_eq!(
        found[2..],
        [
            Event::Route,
            Event::Route,
            Event::Rule,
            Event::Rule,
            Event::Nexthop,
            Event::Nexthop,
            Event::Error(-105),
            Event::Overrun,
            Event::Skipped,
            Event::Skipped,
        ]
    );
}

#[test]
fn the_last_message_s_padding_may_be_absent() {
    // A 17-byte message: its aligned length (20) passes the datagram's end.
    let datagram = build::message(99, 0, 0, &[7]);
    assert_eq!(datagram.len(), 17);
    assert_eq!(events(&datagram), Ok(alloc::vec![Event::Skipped]));
}

#[test]
fn an_empty_datagram_has_no_message() {
    assert_eq!(events(&[]), Ok(alloc::vec![]));
}

// The reject corpus: each check of 5.6, one input each.

#[test]
fn fewer_than_sixteen_bytes_left_is_a_short_header() {
    let mut datagram = build::message(99, 0, 0, &[]);
    datagram.extend([0; 15]);
    assert_eq!(events(&datagram), Err(Malformed::ShortHeader));
}

#[test]
fn a_length_below_the_header_is_refused() {
    let mut datagram = build::message(99, 0, 0, &[]);
    datagram[..4].copy_from_slice(&15_u32.to_ne_bytes());
    assert_eq!(events(&datagram), Err(Malformed::BadLength));
}

#[test]
fn a_length_past_the_datagram_is_refused() {
    let mut datagram = build::message(99, 0, 0, &[0; 4]);
    datagram[..4].copy_from_slice(&21_u32.to_ne_bytes());
    assert_eq!(events(&datagram), Err(Malformed::BadLength));
    datagram[..4].copy_from_slice(&u32::MAX.to_ne_bytes());
    assert_eq!(events(&datagram), Err(Malformed::BadLength));
}

#[test]
fn a_short_link_or_address_body_is_refused() {
    assert_eq!(
        events(&build::message(16, 0, 0, &[0; 15])),
        Err(Malformed::ShortBody)
    );
    assert_eq!(
        events(&build::message(20, 0, 0, &[0; 7])),
        Err(Malformed::ShortBody)
    );
    assert_eq!(
        events(&build::message(2, 0, 0, &[0; 3])),
        Err(Malformed::ShortBody)
    );
}

#[test]
fn an_attribute_shorter_than_its_header_or_past_the_body_is_refused() {
    let datagram = build::link(16, 1, 0, |body| body.extend([3, 0, 16, 0]));
    assert_eq!(events(&datagram), Err(Malformed::BadAttribute));
    let datagram = build::link(16, 1, 0, |body| body.extend([9, 0, 16, 0, 6, 0, 0, 0]));
    assert_eq!(events(&datagram), Err(Malformed::BadAttribute));
    let datagram = build::link(16, 1, 0, |body| body.extend([5, 0]));
    assert_eq!(events(&datagram), Err(Malformed::BadAttribute));
}

#[test]
fn a_value_of_the_wrong_size_is_refused() {
    let datagram = build::link(16, 1, 0, |body| build::attribute(body, 16, &[6, 0]));
    assert_eq!(events(&datagram), Err(Malformed::BadValue));
    let datagram = build::address(20, AF_INET, 1, 8, 0, |body| {
        build::attribute(body, 1, &[10, 0, 0, 1]);
        build::attribute(body, 8, &[0, 0]);
    });
    assert_eq!(events(&datagram), Err(Malformed::BadValue));
    let datagram = build::address(20, AF_INET, 1, 8, 0, |body| {
        build::attribute(body, 1, &[0; 16]);
    });
    assert_eq!(events(&datagram), Err(Malformed::BadValue));
    let datagram = build::address(20, AF_INET6, 1, 8, 0, |body| {
        build::attribute(body, 2, &[0; 4]);
    });
    assert_eq!(events(&datagram), Err(Malformed::BadValue));
}

#[test]
fn an_address_of_another_family_or_with_none_is_refused() {
    let datagram = build::address(20, 7, 1, 8, 0, |body| {
        build::attribute(body, 1, &[10, 0, 0, 1]);
    });
    assert_eq!(events(&datagram), Err(Malformed::BadValue));
    let datagram = build::address(20, AF_INET, 1, 8, 0, |_| {});
    assert_eq!(events(&datagram), Err(Malformed::NoAddress));
}

#[test]
fn messages_stop_after_the_first_error() {
    let mut datagram = build::pad(build::message(16, 0, 0, &[0; 3]));
    datagram.extend(build::message(24, 0, 0, &[]));
    let mut iterator = messages(&datagram);
    let first = iterator.next().unwrap().unwrap();
    assert_eq!(event(&first), Err(Malformed::ShortBody));
    // The header walk itself is sound, so the next message is still read.
    assert!(iterator.next().unwrap().is_ok());
    let mut broken = build::message(24, 0, 0, &[]);
    broken[..4].copy_from_slice(&3_u32.to_ne_bytes());
    broken.extend(build::message(24, 0, 0, &[]));
    let mut iterator = messages(&broken);
    assert_eq!(
        iterator.next().map(Result::err),
        Some(Some(Malformed::BadLength))
    );
    assert!(iterator.next().is_none());
}

/// A splitmix64 step: the sweep's seeded source.
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[test]
fn every_truncation_and_changed_byte_decodes_or_is_refused_without_panic() {
    let mut whole = Vec::new();
    whole.extend(build::link(16, 3, 0x41, |body| {
        build::attribute(body, 16, &[6]);
        build::attribute(body, 3, b"eth0\0");
    }));
    whole.extend(build::address(20, AF_INET6, 3, 64, 0, |body| {
        build::attribute(body, 1, &[0x20; 16]);
        build::attribute(body, 6, &[0; 16]);
        build::attribute(body, 8, &[0; 4]);
    }));
    whole.extend(build::message(24, 0, 0, &[0; 28]));
    whole.extend(build::message(3, 2, 0, &[0; 4]));
    let count = events(&whole).unwrap().len();
    assert_eq!(count, 4);
    // Every prefix: whole messages only, or refused.
    for end in 0..=whole.len() {
        let _ = events(whole.get(..end).unwrap());
    }
    let seed = 0x6e65_7467_656e_0001_u64;
    let mut state = seed;
    for round in 0..20_000 {
        let mut changed = whole.clone();
        for _ in 0..=next(&mut state) % 4 {
            let at = usize::try_from(next(&mut state) % changed.len() as u64).unwrap();
            changed[at] = u8::try_from(next(&mut state) & 0xff).unwrap();
        }
        let cut = usize::try_from(next(&mut state) % (changed.len() as u64 + 1)).unwrap();
        // A panic here fails the test with the round, from this seed.
        let _ = (round, seed, events(&changed[..cut]));
    }
}
