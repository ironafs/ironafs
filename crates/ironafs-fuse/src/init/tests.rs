// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for INIT: the reviewed flag constants, the selection function,
//! version negotiation and the INIT body (fuse-codec.md 5.3, 5.5).

use ironafs_test_support::{Property, Source};

use super::{
    ALLOWED, ConfigError, DIRECT_IO_ALLOW_MMAP, INIT_EXT, InitConfig, InitDefect, InitFlags,
    InitOutcome, InitRefused, InitRequest, MAX_MINOR, MIN_MINOR, NEVER, NO_EXPORT_SUPPORT,
    PASSTHROUGH, POSIX_ACL, WRITEBACK_CACHE, decode_init, negotiate, select_flags,
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
    let offer = flags(!NO_EXPORT_SUPPORT.bits());
    assert_eq!(select_flags(offer), INIT_EXT);
    let offer = flags(!INIT_EXT.bits());
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

/// A major-7 request offering every flag the given minor may offer, built
/// the way the decoder would accept it.
fn request(minor: u32) -> InitRequest {
    InitRequest::seven(minor, 131_072, flags(u64::MAX))
}

fn config() -> InitConfig {
    InitConfig::new(65_536, 1_048_576, 12, 9, 1).unwrap()
}

#[test]
fn negotiate_takes_the_smaller_minor() {
    for (kernel, expected) in [(23, 23), (40, 40), (46, 46), (50, 46)] {
        match negotiate(&request(kernel), &config()) {
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
        negotiate(&request(22), &config()).unwrap_err(),
        InitRefused::MinorBelowFloor { minor: 22 }
    );
}

#[test]
fn negotiate_offers_major_7_to_a_newer_major() {
    let init = decode_init(&[8, 0, 0, 0]).unwrap();
    assert_eq!(init.major(), 8);
    assert_eq!(init.minor(), None);
    assert!(matches!(
        negotiate(&init, &config()),
        Ok(InitOutcome::OfferMajor)
    ));
}

#[test]
fn negotiate_refuses_older_major() {
    assert_eq!(
        negotiate(&decode_init(&[6, 0, 0, 0]).unwrap(), &config()).unwrap_err(),
        InitRefused::MajorTooOld { major: 6 }
    );
}

#[test]
fn readahead_is_the_smaller() {
    let Ok(InitOutcome::Ready(negotiated)) = negotiate(&request(40), &config()) else {
        panic!("not negotiated");
    };
    assert_eq!(negotiated.max_readahead, 65_536);
    let small = InitRequest::seven(40, 4_096, flags(0));
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

/// An INIT body at protocol 7.23, by hand: `major`, `minor`,
/// `max_readahead`, `flags`, little-endian.
const INIT_7_23: [u8; 16] = [
    0x07, 0, 0, 0, // major 7
    0x17, 0, 0, 0, // minor 23
    0x00, 0x00, 0x02, 0, // max_readahead 131,072
    0x01, 0x00, 0x00, 0x00, // flags: ASYNC_READ (bit 0)
];

/// The extended INIT body of 7.36 and later (64 bytes): the 16 above with
/// this minor and `flags`, then `flags2` and eleven unused words.
fn extended(minor: u8, flags: u32, flags2: u32) -> [u8; 64] {
    let mut body = [0u8; 64];
    body[..16].copy_from_slice(&INIT_7_23);
    body[4] = minor;
    body[12..16].copy_from_slice(&flags.to_le_bytes());
    body[16..20].copy_from_slice(&flags2.to_le_bytes());
    body
}

#[test]
fn decode_init_16_bytes_at_7_23() {
    let init = decode_init(&INIT_7_23).unwrap();
    assert_eq!(init.major(), 7);
    assert_eq!(init.minor(), Some(23));
    assert_eq!(init.offer(), Some(flags(1)));
    assert_eq!(init, InitRequest::seven(23, 131_072, flags(1)));
}

#[test]
fn decode_init_other_major_reads_only_the_major() {
    // A newer major's body is not 7.x's layout; only the major is read.
    for body in [&[9u8, 0, 0, 0][..], &[9, 0, 0, 0, 1, 2], &[5, 0, 0, 0]] {
        let init = decode_init(body).unwrap();
        assert_eq!(init.minor(), None, "{body:?}");
        assert_eq!(init.offer(), None, "{body:?}");
    }
    assert_eq!(decode_init(&[9, 0, 0]), Err(InitDefect::Body));
}

#[test]
fn decode_init_ext_reads_the_64_byte_body() {
    let init = decode_init(&extended(46, 1 << 30, 1 << 6)).unwrap();
    assert_eq!(init.minor(), Some(46));
    assert_eq!(init.offer(), Some(flags((1 << 30) | (1 << 38))));
}

#[test]
fn decode_init_refuses_bodies_of_other_sizes() {
    // INIT_EXT set: exactly 64 bytes.
    let body = extended(46, 1 << 30, 0);
    for size in [16, 20, 63] {
        assert_eq!(decode_init(&body[..size]), Err(InitDefect::Body), "{size}");
    }
    let mut longer = body.to_vec();
    longer.push(0);
    assert_eq!(decode_init(&longer), Err(InitDefect::Body));
    // INIT_EXT clear: exactly 16 bytes.
    assert_eq!(decode_init(&INIT_7_23[..15]), Err(InitDefect::Body));
    let mut longer = INIT_7_23.to_vec();
    longer.push(0);
    assert_eq!(decode_init(&longer), Err(InitDefect::Body));
    assert_eq!(decode_init(&extended(46, 0, 0)), Err(InitDefect::Body));
}

#[test]
fn decode_init_refuses_flags_before_their_minor() {
    // FUSE_INIT_EXT is 7.36's, FUSE_NO_EXPORT_SUPPORT 7.40's (E70).
    assert_eq!(
        decode_init(&extended(35, 1 << 30, 0)),
        Err(InitDefect::Flags)
    );
    assert!(decode_init(&extended(36, 1 << 30, 0)).is_ok());
    assert_eq!(
        decode_init(&extended(39, 1 << 30, 1 << 6)),
        Err(InitDefect::Flags)
    );
    assert!(decode_init(&extended(40, 1 << 30, 1 << 6)).is_ok());
    let mut body = INIT_7_23;
    body[15] = 0x40; // INIT_EXT at 7.23, in a 16-byte body
    assert_eq!(decode_init(&body), Err(InitDefect::Body));
}

#[test]
fn negotiated_flags_are_defined_at_the_negotiated_minor() {
    // Whatever a valid offer holds, each selected flag exists at the
    // negotiated minor: the offer was checked against the kernel's minor,
    // and the negotiated minor is never below it while it is at most 46.
    for minor in [36u8, 39, 40, 46] {
        let flags2 = if minor >= 40 { 1 << 6 } else { 0 };
        let init = decode_init(&extended(minor, 1 << 30, flags2)).unwrap();
        let Ok(InitOutcome::Ready(negotiated)) = negotiate(&init, &config()) else {
            panic!("{minor}");
        };
        let selected = negotiated.flags();
        assert!(!selected.contains(NO_EXPORT_SUPPORT) || negotiated.minor() >= 40);
        assert!(!selected.contains(INIT_EXT) || negotiated.minor() >= 36);
    }
}
