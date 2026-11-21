// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the replies: byte vectors written by hand from fuse-codec.md
//! 5.1, points 3 and 5, and the rules of 5.5, points 5 to 11.

use super::{Attr, EncodeError, InitAnswer, InitReply, Kind, ReplyTo, StatFs, Timestamp, shape};
use crate::Errno;
use crate::init::{InitConfig, InitFlags, InitRequest};

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

fn to<S>() -> ReplyTo<S> {
    ReplyTo::new(0, 7, 0)
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

fn mode_of_attr_reply(reply: &[u8]) -> u32 {
    u32::from_le_bytes(reply[92..96].try_into().unwrap())
}

#[test]
fn error_reply_is_negative_and_bare() {
    assert_eq!(
        ReplyTo::<shape::Entry>::new(0, 0x1122, 0)
            .error(Errno::ENOENT)
            .as_bytes(),
        [
            16, 0, 0, 0, 0xfe, 0xff, 0xff, 0xff, 0x22, 0x11, 0, 0, 0, 0, 0, 0
        ]
    );
}

#[test]
fn empty_reply_is_the_header_alone() {
    assert_eq!(
        to::<shape::Empty>().ok().as_bytes(),
        [16, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0]
    );
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
    assert_eq!(
        to::<shape::Entry>()
            .entry(0x55, 3, &sample_attr())
            .as_bytes(),
        expected
    );
}

#[test]
fn attr_reply_layout() {
    let expected = le(&[&head(120, 0, 7), &[0; 16], &sample_attr_bytes(0o100_644)]);
    assert_eq!(
        to::<shape::Attr>().attr(&sample_attr()).as_bytes(),
        expected
    );
}

#[test]
fn entry_and_attr_timeouts_are_zero() {
    let reply = to::<shape::Entry>().entry(7, 1, &sample_attr());
    assert_eq!(reply.as_bytes()[32..56], [0; 24]);
    let reply = to::<shape::Attr>().attr(&sample_attr());
    assert_eq!(reply.as_bytes()[16..32], [0; 16]);
}

#[test]
fn attr_clears_set_id_bits() {
    let mut value = sample_attr();
    for permissions in [0o7777, 0xffff] {
        value.permissions = permissions;
        let reply = to::<shape::Attr>().attr(&value);
        assert_eq!(
            mode_of_attr_reply(reply.as_bytes()),
            0o101_777,
            "{permissions:o}"
        );
    }
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
        let reply = to::<shape::Attr>().attr(&value);
        assert_eq!(
            mode_of_attr_reply(reply.as_bytes()),
            bits | 0o755,
            "{kind:?}"
        );
    }
}

#[test]
fn timestamp_refuses_out_of_range() {
    assert!(Timestamp::new(0, 999_999_999).is_some());
    assert!(Timestamp::new(0, 1_000_000_000).is_none());
    assert!(Timestamp::new(u64::try_from(i64::MAX).unwrap(), 0).is_some());
    assert!(Timestamp::new(u64::try_from(i64::MAX).unwrap() + 1, 0).is_none());
}

/// `struct fuse_sx_time` (16 bytes): seconds, nanoseconds, reserved.
fn sx_time(seconds: u64, nanos: u32) -> Vec<u8> {
    le(&[&seconds.to_le_bytes(), &nanos.to_le_bytes(), &[0; 4]])
}

/// `struct fuse_statx_out` for [`sample_attr`], field by field (288
/// bytes): the reply's own fields, then `struct fuse_statx` (256).
fn sample_statx_bytes(mask: u32, birth: &[u8]) -> Vec<u8> {
    le(&[
        &[0; 8],  // attr_valid
        &[0; 4],  // attr_valid_nsec
        &[0; 4],  // flags
        &[0; 16], // spare
        &mask.to_le_bytes(),
        &4096u32.to_le_bytes(), // blksize
        &[0; 8],                // attributes
        &1u32.to_le_bytes(),    // nlink
        &1000u32.to_le_bytes(),
        &100u32.to_le_bytes(),
        &0o100_644u16.to_le_bytes(), // mode
        &[0; 2],
        &0x0102u64.to_le_bytes(), // ino
        &5000u64.to_le_bytes(),
        &10u64.to_le_bytes(),
        &[0; 8], // attributes_mask
        &sx_time(1_700_000_000, 1),
        birth,
        &sx_time(1_700_000_002, 999_999_999),
        &sx_time(1_700_000_001, 2),
        &[0; 16],  // rdev and dev, major and minor
        &[0; 112], // spare
    ])
}

#[test]
fn statx_reply_layout_without_birth_time() {
    let expected = le(&[&head(304, 0, 7), &sample_statx_bytes(0x7ff, &[0; 16])]);
    assert_eq!(
        to::<shape::Statx>().statx(&sample_attr(), None).as_bytes(),
        expected
    );
}

#[test]
fn statx_reply_reports_a_birth_time_it_has() {
    let birth = Timestamp::new(1_600_000_000, 5).unwrap();
    let expected = le(&[
        &head(304, 0, 7),
        &sample_statx_bytes(0xfff, &sx_time(1_600_000_000, 5)),
    ]);
    assert_eq!(
        to::<shape::Statx>()
            .statx(&sample_attr(), Some(birth))
            .as_bytes(),
        expected
    );
}

#[test]
fn statx_clears_set_id_bits() {
    let mut value = sample_attr();
    value.permissions = 0o7777;
    let reply = to::<shape::Statx>().statx(&value, None);
    let mode = u16::from_le_bytes(reply.as_bytes()[76..78].try_into().unwrap());
    assert_eq!(mode, 0o101_777);
}

#[test]
fn open_reply_is_direct_io_only() {
    let expected = le(&[
        &head(32, 0, 7),
        &9u64.to_le_bytes(),
        &1u32.to_le_bytes(),
        &[0; 4],
    ]);
    assert_eq!(to::<shape::Open>().open(9).as_bytes(), expected);
}

#[test]
fn opendir_reply_sets_no_cache_flag() {
    let expected = le(&[&head(32, 0, 7), &9u64.to_le_bytes(), &[0; 8]]);
    assert_eq!(to::<shape::Opendir>().opendir(9).as_bytes(), expected);
}

#[test]
fn data_reply_carries_the_bytes() {
    let reply = ReplyTo::<shape::Data>::new(0, 7, 3).data(b"abc").unwrap();
    assert_eq!(reply.as_bytes(), le(&[&head(19, 0, 7), b"abc"]));
    let reply = ReplyTo::<shape::Data>::new(0, 7, 3).data(b"").unwrap();
    assert_eq!(reply.as_bytes(), head(16, 0, 7));
}

#[test]
fn data_over_the_request_size_refused() {
    let refused = ReplyTo::<shape::Data>::new(0, 7, 3)
        .data(b"abcd")
        .unwrap_err();
    assert_eq!(refused.error, EncodeError::DataTooLong);
    // The reply is still owed, and can be an error.
    assert_eq!(refused.reply.error(Errno::EIO).as_bytes().len(), 16);
    let full = ReplyTo::<shape::Data>::new(0, 7, 1_048_576).data(&vec![0; 1_048_576]);
    assert_eq!(full.unwrap().as_bytes().len(), 16 + 1_048_576);
}

#[test]
fn link_target_caps() {
    assert_eq!(
        to::<shape::Readlink>()
            .readlink(b"../x")
            .unwrap()
            .as_bytes(),
        le(&[&head(20, 0, 7), b"../x"])
    );
    assert_eq!(
        to::<shape::Readlink>()
            .readlink(&[b'a'; 4095])
            .unwrap()
            .as_bytes()
            .len(),
        16 + 4095
    );
    for target in [&[b'a'; 4096][..], b"", b"a\0b"] {
        assert_eq!(
            to::<shape::Readlink>().readlink(target).unwrap_err().error,
            EncodeError::LinkTarget
        );
    }
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
    assert_eq!(to::<shape::Statfs>().statfs(&value).as_bytes(), expected);
}

#[test]
fn init_reply_splits_flags() {
    let request = InitRequest::seven(46, 131_072, InitFlags::from_bits(u64::MAX));
    let config = InitConfig::new(65_536, 1_048_576, 12, 9, 1_000_000_000).unwrap();
    let (reply, answer) = InitReply::new(0, 7, request).answer(&config);
    assert!(matches!(answer, InitAnswer::Ready(_)));
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
    assert_eq!(reply.as_bytes(), expected);
}

#[test]
fn init_major_only_reply_for_a_newer_major() {
    let request = crate::init::decode_init(&8u32.to_le_bytes()).unwrap();
    let config = InitConfig::new(0, 4096, 1, 1, 1).unwrap();
    let (reply, answer) = InitReply::new(0, 7, request).answer(&config);
    assert_eq!(answer, InitAnswer::NewInitAwaited);
    assert_eq!(
        reply.as_bytes(),
        le(&[&head(20, 0, 7), &7u32.to_le_bytes()])
    );
}

#[test]
fn init_refusal_is_eproto() {
    let request = InitRequest::seven(22, 0, InitFlags::from_bits(0));
    let config = InitConfig::new(0, 4096, 1, 1, 1).unwrap();
    let (reply, answer) = InitReply::new(0, 7, request).answer(&config);
    assert!(matches!(answer, InitAnswer::Refused(_)));
    assert_eq!(reply.as_bytes(), head(16, -71, 7));
}
