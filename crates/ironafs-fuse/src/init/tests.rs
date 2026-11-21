// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for INIT: the reviewed flag constants, the selection function,
//! version negotiation and the INIT body (fuse-codec.md 5.3, 5.5).

use ironafs_test_support::{Property, Source};

use super::{
    ALLOWED, ConfigError, DIRECT_IO_ALLOW_MMAP, INIT_EXT, InitConfig, InitFlags, InitOutcome,
    InitRefused, InitRequest, MAX_MINOR, MIN_MINOR, NEVER, NO_EXPORT_SUPPORT, PASSTHROUGH,
    POSIX_ACL, WRITEBACK_CACHE, decode_init, negotiate, select_flags,
};

fn flags(bits: u64) -> InitFlags {
    InitFlags::from_bits(bits)
}

#[test]
fn reviewed_constants_have_their_reviewed_bits() {
    assert_eq!(INIT_EXT.bits(), 1 << 30);
    assert_eq!(NO_EXPORT_SUPPORT.bits(), 1 << 38);
    assert_eq!(WRITEBACK_CACHE.bits(), 1 << 16);
    assert_eq!(POSIX_ACL.bits(), 1 << 20);
    assert_eq!(DIRECT_IO_ALLOW_MMAP.bits(), 1 << 36);
    assert_eq!(PASSTHROUGH.bits(), 1 << 37);
    assert_eq!(ALLOWED.bits(), (1 << 30) | (1 << 38));
    assert_eq!(NEVER.bits(), (1 << 16) | (1 << 20) | (1 << 36) | (1 << 37));
}

#[test]
fn allowed_holds_no_never_flag() {
    assert_eq!(ALLOWED.bits() & NEVER.bits(), 0);
}

#[test]
fn select_full_offer_gives_exactly_allowed() {
    assert_eq!(select_flags(flags(u64::MAX)), ALLOWED);
}

#[test]
fn select_without_an_allowed_bit_omits_it() {
    let offer = flags(u64::MAX & !NO_EXPORT_SUPPORT.bits());
    assert_eq!(select_flags(offer), INIT_EXT);
    let offer = flags(u64::MAX & !INIT_EXT.bits());
    assert_eq!(select_flags(offer), NO_EXPORT_SUPPORT);
}

#[test]
fn select_allowed_bit_alone_gives_it_alone() {
    assert_eq!(select_flags(INIT_EXT), INIT_EXT);
    assert_eq!(select_flags(NO_EXPORT_SUPPORT), NO_EXPORT_SUPPORT);
    assert_eq!(select_flags(flags(0)), flags(0));
}

#[test]
fn never_list_flags_are_never_selected() {
    for never in [
        WRITEBACK_CACHE,
        POSIX_ACL,
        DIRECT_IO_ALLOW_MMAP,
        PASSTHROUGH,
    ] {
        assert_eq!(select_flags(never), flags(0), "{never:?}");
        assert_eq!(
            select_flags(flags(never.bits() | INIT_EXT.bits())),
            INIT_EXT,
            "{never:?}"
        );
    }
    assert_eq!(select_flags(NEVER), flags(0));
}

#[test]
fn select_is_offer_and_allowed() {
    Property::new("select_flags is the offer intersected with ALLOWED")
        .cases(4096)
        .check(Source::u64, |&offer| {
            let selected = select_flags(flags(offer));
            assert_eq!(selected.bits(), offer & ALLOWED.bits());
            assert_eq!(selected.bits() & NEVER.bits(), 0);
        });
}

fn request(major: u32, minor: u32) -> InitRequest {
    InitRequest {
        major,
        minor,
        max_readahead: 131_072,
        offer: flags(u64::MAX),
    }
}

fn config() -> InitConfig {
    InitConfig::new(65_536, 1_048_576, 12, 9, 1).unwrap()
}

#[test]
fn negotiate_takes_the_smaller_minor() {
    for (kernel, expected) in [(23, 23), (40, 40), (46, 46), (50, 46)] {
        match negotiate(&request(7, kernel), &config()) {
            Ok(InitOutcome::Ready(negotiated)) => {
                assert_eq!(negotiated.minor(), expected, "{kernel}");
                assert_eq!(negotiated.flags(), ALLOWED, "{kernel}");
            }
            other => panic!("minor {kernel}: {other:?}"),
        }
    }
    assert_eq!(MIN_MINOR, 23);
    assert_eq!(MAX_MINOR, 46);
}

#[test]
fn negotiate_refuses_minor_below_floor() {
    assert_eq!(
        negotiate(&request(7, 22), &config()).unwrap_err(),
        InitRefused::MinorBelowFloor { minor: 22 }
    );
}

#[test]
fn negotiate_offers_major_7_to_a_newer_major() {
    assert!(matches!(
        negotiate(&request(8, 0), &config()),
        Ok(InitOutcome::OfferMajor)
    ));
}

#[test]
fn negotiate_refuses_older_major() {
    assert_eq!(
        negotiate(&request(6, 40), &config()).unwrap_err(),
        InitRefused::MajorTooOld { major: 6 }
    );
}

#[test]
fn readahead_is_the_smaller() {
    let Ok(InitOutcome::Ready(negotiated)) = negotiate(&request(7, 40), &config()) else {
        panic!("not negotiated");
    };
    assert_eq!(negotiated.max_readahead, 65_536);
    let mut small = request(7, 40);
    small.max_readahead = 4_096;
    let Ok(InitOutcome::Ready(negotiated)) = negotiate(&small, &config()) else {
        panic!("not negotiated");
    };
    assert_eq!(negotiated.max_readahead, 4_096);
}

#[test]
fn config_refuses_max_write_over_the_data_cap() {
    assert_eq!(
        InitConfig::new(0, 1_048_577, 12, 9, 1).unwrap_err(),
        ConfigError::MaxWrite
    );
}

/// An INIT body at protocol 7.23: `major`, `minor`, `max_readahead`,
/// `flags`, little-endian.
const INIT_7_23: [u8; 16] = [
    0x07, 0, 0, 0, // major 7
    0x17, 0, 0, 0, // minor 23
    0x00, 0x00, 0x02, 0, // max_readahead 131,072
    0x00, 0x00, 0x00, 0x40, // flags: INIT_EXT (bit 30), which 7.23 never sends
];

#[test]
fn decode_init_16_bytes_reads_no_flags2() {
    let mut body = INIT_7_23;
    body[15] = 0x00;
    body[12] = 0x01; // ASYNC_READ only
    assert_eq!(
        decode_init(&body),
        Some(InitRequest {
            major: 7,
            minor: 23,
            max_readahead: 131_072,
            offer: flags(1),
        })
    );
}

#[test]
fn decode_init_ext_without_flags2_is_refused() {
    assert_eq!(decode_init(&INIT_7_23), None);
}

#[test]
fn decode_init_64_bytes_with_init_ext_reads_flags2_shifted() {
    let mut body = [0u8; 64];
    body[..16].copy_from_slice(&INIT_7_23);
    body[4] = 46;
    body[16..20].copy_from_slice(&[0x40, 0x00, 0x00, 0x00]); // flags2 bit 6 = bit 38
    let decoded = decode_init(&body).unwrap();
    assert_eq!(decoded.minor, 46);
    assert_eq!(decoded.offer, flags((1 << 30) | (1 << 38)));
}

#[test]
fn decode_init_64_bytes_without_init_ext_ignores_flags2() {
    let mut body = [0u8; 64];
    body[..16].copy_from_slice(&INIT_7_23);
    body[15] = 0;
    body[16..20].copy_from_slice(&[0xff; 4]);
    assert_eq!(decode_init(&body).unwrap().offer, flags(0));
}

#[test]
fn decode_init_short_body_is_refused() {
    assert_eq!(decode_init(&INIT_7_23[..15]), None);
}
