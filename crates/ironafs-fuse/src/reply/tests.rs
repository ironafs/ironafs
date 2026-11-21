// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the replies: byte vectors written by hand from fuse-codec.md
//! 5.1, points 3 and 5, and the rules of 5.5, points 5 to 10.

use super::{
    Attr, EncodeError, Kind, StatFs, Timestamp, attr, data, empty, entry, error, init,
    init_major_only, open, opendir, readlink, statfs,
};
use crate::Errno;
use crate::init::{InitConfig, InitFlags, InitOutcome, InitRequest, negotiate};

fn le(fields: &[&[u8]]) -> Vec<u8> {
    fields.concat()
}

/// A reply header: `len`, `error`, `unique`.
fn head(len: u32, error: i32, unique: u64) -> Vec<u8> {
    le(&[
        &len.to_le_bytes(),
        &error.to_le_bytes(),
        &unique.to_le_bytes(),
    ])
}

fn sample_attr() -> Attr {
    Attr {
        ino: 0x0102,
        size: 5000,
        blocks: 10,
        atime: Timestamp::new(1_700_000_000, 1).unwrap(),
        mtime: Timestamp::new(1_700_000_001, 2).unwrap(),
        ctime: Timestamp::new(1_700_000_002, 999_999_999).unwrap(),
        kind: Kind::File,
        permissions: 0o644,
        nlink: 1,
        uid: 1000,
        gid: 100,
        blksize: 4096,
    }
}

/// `fuse_attr` for [`sample_attr`] with `mode`, field by field (88 bytes).
fn sample_attr_bytes(mode: u32) -> Vec<u8> {
    le(&[
        &0x0102u64.to_le_bytes(),
        &5000u64.to_le_bytes(),
        &10u64.to_le_bytes(),
        &1_700_000_000u64.to_le_bytes(),
        &1_700_000_001u64.to_le_bytes(),
        &1_700_000_002u64.to_le_bytes(),
        &1u32.to_le_bytes(),
        &2u32.to_le_bytes(),
        &999_999_999u32.to_le_bytes(),
        &mode.to_le_bytes(),
        &1u32.to_le_bytes(),
        &1000u32.to_le_bytes(),
        &100u32.to_le_bytes(),
        &0u32.to_le_bytes(), // rdev
        &4096u32.to_le_bytes(),
        &0u32.to_le_bytes(), // flags
    ])
}

#[test]
fn error_reply_is_negative_and_bare() {
    assert_eq!(
        error(0x1122, Errno::ENOENT),
        [
            16, 0, 0, 0, 0xfe, 0xff, 0xff, 0xff, 0x22, 0x11, 0, 0, 0, 0, 0, 0
        ]
    );
}

#[test]
fn empty_reply_is_the_header_alone() {
    assert_eq!(empty(7), [16, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn entry_reply_layout() {
    let expected = le(&[
        &head(144, 0, 7),
        &0x55u64.to_le_bytes(),
        &3u64.to_le_bytes(),
        &[0; 24], // entry_valid, attr_valid, both nsec fields
        &sample_attr_bytes(0o100_644),
    ]);
    assert_eq!(entry(7, 0x55, 3, &sample_attr()), expected);
}

#[test]
fn attr_reply_layout() {
    let expected = le(&[&head(120, 0, 7), &[0; 16], &sample_attr_bytes(0o100_644)]);
    assert_eq!(attr(7, &sample_attr()), expected);
}

#[test]
fn entry_and_attr_timeouts_are_zero() {
    let reply = entry(7, 1, 1, &sample_attr());
    assert_eq!(reply[32..56], [0; 24]);
    let reply = attr(7, &sample_attr());
    assert_eq!(reply[16..32], [0; 16]);
}

#[test]
fn attr_clears_set_id_bits() {
    let mut value = sample_attr();
    value.permissions = 0o7777;
    let reply = attr(7, &value);
    let mode = u32::from_le_bytes(reply[16 + 16 + 60..16 + 16 + 64].try_into().unwrap());
    assert_eq!(mode, 0o101_777);
    value.permissions = 0xffff;
    let reply = attr(7, &value);
    let mode = u32::from_le_bytes(reply[16 + 16 + 60..16 + 16 + 64].try_into().unwrap());
    assert_eq!(mode, 0o101_777);
}

#[test]
fn kinds_have_their_type_bits() {
    for (kind, bits) in [
        (Kind::Directory, 0o040_000),
        (Kind::File, 0o100_000),
        (Kind::Symlink, 0o120_000),
    ] {
        let mut value = sample_attr();
        value.kind = kind;
        value.permissions = 0o755;
        let reply = attr(7, &value);
        let mode = u32::from_le_bytes(reply[92..96].try_into().unwrap());
        assert_eq!(mode, bits | 0o755, "{kind:?}");
    }
}

#[test]
fn timestamp_refuses_out_of_range() {
    assert!(Timestamp::new(0, 999_999_999).is_some());
    assert!(Timestamp::new(0, 1_000_000_000).is_none());
    assert!(Timestamp::new(u64::try_from(i64::MAX).unwrap(), 0).is_some());
    assert!(Timestamp::new(u64::try_from(i64::MAX).unwrap() + 1, 0).is_none());
}

#[test]
fn open_reply_is_direct_io_only() {
    let expected = le(&[
        &head(32, 0, 7),
        &9u64.to_le_bytes(),
        &1u32.to_le_bytes(),
        &[0; 4],
    ]);
    assert_eq!(open(7, 9), expected);
}

#[test]
fn opendir_reply_sets_no_cache_flag() {
    let expected = le(&[&head(32, 0, 7), &9u64.to_le_bytes(), &[0; 8]]);
    assert_eq!(opendir(7, 9), expected);
}

#[test]
fn data_reply_carries_the_bytes() {
    assert_eq!(data(7, b"abc").unwrap(), le(&[&head(19, 0, 7), b"abc"]));
    assert_eq!(data(7, b"").unwrap(), head(16, 0, 7));
}

#[test]
fn data_over_cap_refused() {
    assert_eq!(data(7, &vec![0; 1_048_576]).unwrap().len(), 16 + 1_048_576);
    assert_eq!(
        data(7, &vec![0; 1_048_577]).unwrap_err(),
        EncodeError::DataTooLong
    );
}

#[test]
fn link_target_caps() {
    assert_eq!(
        readlink(7, b"../x").unwrap(),
        le(&[&head(20, 0, 7), b"../x"])
    );
    assert_eq!(readlink(7, &[b'a'; 4095]).unwrap().len(), 16 + 4095);
    assert_eq!(
        readlink(7, &[b'a'; 4096]).unwrap_err(),
        EncodeError::LinkTarget
    );
    assert_eq!(readlink(7, b"").unwrap_err(), EncodeError::LinkTarget);
    assert_eq!(readlink(7, b"a\0b").unwrap_err(), EncodeError::LinkTarget);
}

#[test]
fn statfs_reply_layout() {
    let value = StatFs {
        blocks: 1,
        bfree: 2,
        bavail: 3,
        files: 4,
        ffree: 5,
        bsize: 4096,
        frsize: 1024,
    };
    let expected = le(&[
        &head(96, 0, 7),
        &1u64.to_le_bytes(),
        &2u64.to_le_bytes(),
        &3u64.to_le_bytes(),
        &4u64.to_le_bytes(),
        &5u64.to_le_bytes(),
        &4096u32.to_le_bytes(),
        &255u32.to_le_bytes(),
        &1024u32.to_le_bytes(),
        &[0; 28],
    ]);
    assert_eq!(statfs(7, &value), expected);
}

#[test]
fn init_reply_splits_flags() {
    let request = InitRequest {
        major: 7,
        minor: 46,
        max_readahead: 131_072,
        offer: InitFlags::from_bits(u64::MAX),
    };
    let config = InitConfig::new(65_536, 1_048_576, 12, 9, 1_000_000_000).unwrap();
    let Ok(InitOutcome::Ready(negotiated)) = negotiate(&request, &config) else {
        panic!("not negotiated");
    };
    let mut expected = le(&[
        &head(80, 0, 7),
        &7u32.to_le_bytes(),
        &46u32.to_le_bytes(),
        &65_536u32.to_le_bytes(),
        &(1u32 << 30).to_le_bytes(), // INIT_EXT
        &12u16.to_le_bytes(),
        &9u16.to_le_bytes(),
        &1_048_576u32.to_le_bytes(),
        &1_000_000_000u32.to_le_bytes(),
        &[0; 4],                    // max_pages, map_alignment
        &(1u32 << 6).to_le_bytes(), // flags2: NO_EXPORT_SUPPORT
    ]);
    expected.resize(80, 0);
    assert_eq!(init(7, &negotiated), expected);
}

#[test]
fn init_major_only_reply() {
    assert_eq!(
        init_major_only(7),
        le(&[&head(20, 0, 7), &7u32.to_le_bytes()])
    );
}
