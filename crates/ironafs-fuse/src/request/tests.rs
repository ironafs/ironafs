// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for request decoding: golden vectors written by hand from
//! fuse-codec.md 5.1, and its reject corpus (10.3).

use ironafs_vfs::NameError;

use super::{
    DecodeError, Forget, FrameError, Header, Malformed, Open, Phase, Read, Release, Request, decode,
};
use crate::init::{INIT_EXT, InitFlags, InitRequest, NO_EXPORT_SUPPORT};
use crate::reply::{InitReply, ReplyTo};
use crate::{Errno, Opcode};

/// The reply token the decoder gives a request of [`request`]'s unique.
fn to<S>() -> ReplyTo<S> {
    ReplyTo::new(0, 7, 0)
}

const READY: Phase = Phase::Negotiated { minor: 46 };

/// A LOOKUP of `b"cell"` under node 1, by hand: the 40-byte header
/// (`len` 45, opcode 1, unique 0x1122, nodeid 1, uid 1000, gid 100, pid
/// 4242, then four zero bytes) and the name with its NUL.
const LOOKUP_CELL: [u8; 45] = [
    45, 0, 0, 0, // len
    1, 0, 0, 0, // opcode: LOOKUP
    0x22, 0x11, 0, 0, 0, 0, 0, 0, // unique
    1, 0, 0, 0, 0, 0, 0, 0, // nodeid
    0xe8, 0x03, 0, 0, // uid 1000
    100, 0, 0, 0, // gid 100
    0x92, 0x10, 0, 0, // pid 4242
    0, 0, 0, 0, // total_extlen and padding
    b'c', b'e', b'l', b'l', 0,
];

/// A request with the given opcode and body: the header written field by
/// field, unique 7, nodeid 9, uid 1, gid 2, pid 3.
fn request(opcode: u32, body: &[u8]) -> Vec<u8> {
    let len = u32::try_from(40 + body.len()).unwrap();
    let mut out = Vec::new();
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&opcode.to_le_bytes());
    out.extend_from_slice(&7u64.to_le_bytes());
    out.extend_from_slice(&9u64.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&3u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(body);
    out
}

fn header(opcode: u32) -> Header {
    Header {
        opcode,
        unique: 7,
        nodeid: 9,
        uid: 1,
        gid: 2,
        pid: 3,
    }
}

#[track_caller]
fn decoded(bytes: &[u8], phase: Phase) -> Request<'_> {
    match decode(bytes, phase, 0) {
        Ok((_, request)) => request,
        Err(error) => panic!("refused: {error:?}"),
    }
}

#[track_caller]
fn malformed(bytes: &[u8], phase: Phase) -> Malformed {
    match decode(bytes, phase, 0) {
        Err(DecodeError::Request(error)) => error.malformed,
        other => panic!("not a request error: {other:?}"),
    }
}

fn le(fields: &[&[u8]]) -> Vec<u8> {
    fields.concat()
}

#[test]
fn lookup_golden_vector_decodes() {
    let (header, request) = decode(&LOOKUP_CELL, READY, 0).unwrap();
    assert_eq!(
        header,
        Header {
            opcode: 1,
            unique: 0x1122,
            nodeid: 1,
            uid: 1000,
            gid: 100,
            pid: 4242,
        }
    );
    match request {
        Request::Lookup { name, reply } => {
            assert_eq!(name.as_bytes(), b"cell");
            assert_eq!(reply.unique(), 0x1122);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn values_pass_through_unjudged() {
    let mut bytes = request(3, &[0; 16]);
    bytes[8..16].copy_from_slice(&0u64.to_le_bytes());
    bytes[16..24].copy_from_slice(&0u64.to_le_bytes());
    bytes[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
    let (header, _) = decode(&bytes, READY, 0).unwrap();
    assert_eq!((header.unique, header.nodeid, header.uid), (0, 0, u32::MAX));
}

#[test]
fn getattr_with_and_without_fh() {
    let body = le(&[
        &0u32.to_le_bytes(),
        &0u32.to_le_bytes(),
        &77u64.to_le_bytes(),
    ]);
    assert_eq!(
        decoded(&request(3, &body), READY),
        Request::Getattr {
            fh: None,
            reply: to()
        }
    );
    let body = le(&[
        &1u32.to_le_bytes(),
        &0u32.to_le_bytes(),
        &77u64.to_le_bytes(),
    ]);
    assert_eq!(
        decoded(&request(3, &body), READY),
        Request::Getattr {
            fh: Some(77),
            reply: to()
        }
    );
}

fn read_body(offset: u64, size: u32, read_flags: u32) -> Vec<u8> {
    le(&[
        &5u64.to_le_bytes(),
        &offset.to_le_bytes(),
        &size.to_le_bytes(),
        &read_flags.to_le_bytes(),
        &66u64.to_le_bytes(),
        &0o2u32.to_le_bytes(),
        &[0; 4],
    ])
}

#[test]
fn read_of_4096_at_8192() {
    let expected = Read {
        fh: 5,
        offset: 8192,
        size: 4096,
        lock_owner: None,
        flags: 0o2,
    };
    assert_eq!(
        decoded(&request(15, &read_body(8192, 4096, 0)), READY),
        Request::Read {
            read: expected,
            reply: ReplyTo::new(0, 7, 4096)
        }
    );
    assert_eq!(
        decoded(&request(28, &read_body(8192, 4096, 0)), READY),
        Request::Readdir {
            read: expected,
            reply: to()
        }
    );
    assert_eq!(
        decoded(&request(15, &read_body(8192, 4096, 2)), READY),
        Request::Read {
            read: Read {
                lock_owner: Some(66),
                ..expected
            },
            reply: ReplyTo::new(0, 7, 4096)
        }
    );
}

#[test]
fn open_release_flush_access_interrupt_forget() {
    let open = le(&[&0o100_000u32.to_le_bytes(), &0u32.to_le_bytes()]);
    assert_eq!(
        decoded(&request(14, &open), READY),
        Request::Open {
            open: Open { flags: 0o100_000 },
            reply: to()
        }
    );
    assert_eq!(
        decoded(&request(27, &open), READY),
        Request::Opendir {
            open: Open { flags: 0o100_000 },
            reply: to()
        }
    );
    let release = le(&[
        &5u64.to_le_bytes(),
        &2u32.to_le_bytes(),
        &3u32.to_le_bytes(),
        &8u64.to_le_bytes(),
    ]);
    let expected = Release {
        fh: 5,
        flags: 2,
        flush: true,
        flock_unlock: true,
        lock_owner: 8,
    };
    assert_eq!(
        decoded(&request(18, &release), READY),
        Request::Release {
            release: expected,
            reply: to()
        }
    );
    assert_eq!(
        decoded(&request(29, &release), READY),
        Request::Releasedir {
            release: expected,
            reply: to()
        }
    );
    let flush = le(&[&5u64.to_le_bytes(), &[0xee; 8], &8u64.to_le_bytes()]);
    assert_eq!(
        decoded(&request(25, &flush), READY),
        Request::Flush {
            fh: 5,
            lock_owner: 8,
            reply: to()
        }
    );
    let access = le(&[&4u32.to_le_bytes(), &[0xee; 4]]);
    assert_eq!(
        decoded(&request(34, &access), READY),
        Request::Access {
            mask: 4,
            reply: to()
        }
    );
    assert_eq!(
        decoded(&request(36, &0x55u64.to_le_bytes()), READY),
        Request::Interrupt { target: 0x55 }
    );
    assert_eq!(
        decoded(&request(2, &3u64.to_le_bytes()), READY),
        Request::Forget { nlookup: 3 }
    );
}

#[test]
fn empty_bodies() {
    assert_eq!(
        decoded(&request(5, &[]), READY),
        Request::Readlink { reply: to() }
    );
    assert_eq!(
        decoded(&request(17, &[]), READY),
        Request::Statfs { reply: to() }
    );
    assert_eq!(
        decoded(&request(38, &[]), READY),
        Request::Destroy { reply: to() }
    );
    assert_eq!(
        malformed(&request(17, &[0]), READY),
        Malformed::BodyLength {
            expected: 0,
            found: 1
        }
    );
}

#[test]
fn batch_forget_of_two_records() {
    let body = le(&[
        &2u32.to_le_bytes(),
        &[0xee; 4],
        &10u64.to_le_bytes(),
        &1u64.to_le_bytes(),
        &11u64.to_le_bytes(),
        &u64::MAX.to_le_bytes(),
    ]);
    let bytes = request(42, &body);
    let Request::BatchForget(batch) = decoded(&bytes, READY) else {
        panic!("not a batch");
    };
    assert_eq!(batch.len(), 2);
    assert_eq!(
        batch.iter().collect::<Vec<_>>(),
        [
            Forget {
                node: 10,
                nlookup: 1
            },
            Forget {
                node: 11,
                nlookup: u64::MAX
            }
        ]
    );
}

#[test]
fn batch_forget_count_must_match() {
    let body = le(&[&3u32.to_le_bytes(), &[0; 4], &[0; 32]]);
    assert_eq!(
        malformed(&request(42, &body), READY),
        Malformed::ForgetCount
    );
    let body = le(&[&u32::MAX.to_le_bytes(), &[0; 4]]);
    assert_eq!(
        malformed(&request(42, &body), READY),
        Malformed::ForgetCount
    );
    assert_eq!(
        malformed(&request(42, &[0; 7]), READY),
        Malformed::BodyLength {
            expected: 8,
            found: 7
        }
    );
}

#[test]
fn init_at_7_23_and_7_46() {
    let body = le(&[
        &7u32.to_le_bytes(),
        &23u32.to_le_bytes(),
        &4096u32.to_le_bytes(),
        &1u32.to_le_bytes(),
    ]);
    assert_eq!(
        decoded(&request(26, &body), Phase::AwaitingInit),
        Request::Init {
            reply: InitReply::new(0, 7, InitRequest::seven(23, 4096, InitFlags::from_bits(1)))
        }
    );
    let mut body = le(&[
        &7u32.to_le_bytes(),
        &46u32.to_le_bytes(),
        &4096u32.to_le_bytes(),
        &INIT_EXT.low().to_le_bytes(),
        &NO_EXPORT_SUPPORT.high().to_le_bytes(),
    ]);
    body.resize(64, 0);
    assert_eq!(
        decoded(&request(26, &body), Phase::AwaitingInit),
        Request::Init {
            reply: InitReply::new(
                0,
                7,
                InitRequest::seven(
                    46,
                    4096,
                    InitFlags::from_bits(INIT_EXT.bits() | NO_EXPORT_SUPPORT.bits()),
                ),
            ),
        }
    );
    assert_eq!(
        malformed(&request(26, &body[..20]), Phase::AwaitingInit),
        Malformed::InitBody
    );
    let mut early = body.clone();
    early[4] = 39; // NO_EXPORT_SUPPORT offered at 7.39
    assert_eq!(
        malformed(&request(26, &early), Phase::AwaitingInit),
        Malformed::InitFlags
    );
}

#[test]
fn opcodes_not_decoded_and_unknown() {
    assert_eq!(
        decoded(&request(16, &[1, 2, 3]), READY),
        Request::NotDecoded {
            opcode: Opcode::Write,
            reply: to()
        }
    );
    for opcode in [0, 7, 19, 54, 4096, 436_207_616] {
        assert_eq!(
            decoded(&request(opcode, &[1, 2, 3]), READY),
            Request::Unknown {
                opcode,
                reply: to()
            }
        );
    }
}

#[test]
fn frame_errors_have_no_reply() {
    assert_eq!(
        decode(&LOOKUP_CELL[..39], READY, 0).unwrap_err(),
        DecodeError::Frame(FrameError::Short { read: 39 })
    );
    let mut longer = LOOKUP_CELL;
    longer[0] = 46;
    assert_eq!(
        decode(&longer, READY, 0).unwrap_err(),
        DecodeError::Frame(FrameError::LengthMismatch {
            declared: 46,
            read: 45
        })
    );
    longer[0] = 44;
    assert_eq!(
        decode(&longer, READY, 0).unwrap_err(),
        DecodeError::Frame(FrameError::LengthMismatch {
            declared: 44,
            read: 45
        })
    );
}

#[test]
fn phase_is_enforced() {
    assert_eq!(
        malformed(&request(3, &[0; 16]), Phase::AwaitingInit),
        Malformed::BeforeInit
    );
    let init = le(&[
        &7u32.to_le_bytes(),
        &40u32.to_le_bytes(),
        &0u32.to_le_bytes(),
        &0u32.to_le_bytes(),
    ]);
    assert_eq!(malformed(&request(26, &init), READY), Malformed::InitAgain);
}

#[test]
fn extensions_refused_from_minor_38() {
    let mut bytes = request(3, &[0; 16]);
    bytes[36] = 1;
    assert_eq!(
        malformed(&bytes, Phase::Negotiated { minor: 38 }),
        Malformed::Extensions
    );
    assert_eq!(
        decoded(&bytes, Phase::Negotiated { minor: 37 }),
        Request::Getattr {
            fh: None,
            reply: to()
        }
    );
}

#[test]
fn fixed_bodies_are_exact() {
    for (opcode, size) in [
        (2, 8),
        (3, 16),
        (14, 8),
        (15, 40),
        (18, 24),
        (25, 24),
        (27, 8),
        (28, 40),
        (29, 24),
        (34, 8),
        (36, 8),
    ] {
        for found in [size - 1, size + 1] {
            assert_eq!(
                malformed(&request(opcode, &vec![0; found]), READY),
                Malformed::BodyLength {
                    expected: size,
                    found
                },
                "opcode {opcode}"
            );
        }
    }
}

#[test]
fn lookup_names_are_checked() {
    let cases: [(&[u8], Malformed); 7] = [
        (b"cell", Malformed::NameUnterminated),
        (b"cell\0x", Malformed::NameUnterminated),
        (b"a/b\0", Malformed::Name(NameError::Slash)),
        (b".\0", Malformed::Name(NameError::Dot)),
        (b"..\0", Malformed::Name(NameError::DotDot)),
        (b"\0", Malformed::Name(NameError::Empty)),
        (b"a\0\0", Malformed::NameUnterminated),
    ];
    for (body, expected) in cases {
        assert_eq!(malformed(&request(1, body), READY), expected, "{body:?}");
    }
    let mut long = vec![b'x'; 256];
    long.push(0);
    assert_eq!(
        malformed(&request(1, &long), READY),
        Malformed::Name(NameError::TooLong)
    );
    let mut longest = vec![b'x'; 255];
    longest.push(0);
    assert!(matches!(
        decoded(&request(1, &longest), READY),
        Request::Lookup { .. }
    ));
}

#[test]
fn unknown_flag_bits_are_refused() {
    let getattr = le(&[&2u32.to_le_bytes(), &[0; 12]]);
    assert_eq!(
        malformed(&request(3, &getattr), READY),
        Malformed::UnknownFlags
    );
    assert_eq!(
        malformed(&request(15, &read_body(0, 1, 1)), READY),
        Malformed::UnknownFlags
    );
    let release = le(&[
        &5u64.to_le_bytes(),
        &0u32.to_le_bytes(),
        &4u32.to_le_bytes(),
        &0u64.to_le_bytes(),
    ]);
    assert_eq!(
        malformed(&request(18, &release), READY),
        Malformed::UnknownFlags
    );
}

#[test]
fn read_ranges_are_capped() {
    assert_eq!(
        malformed(&request(15, &read_body(0, 1_048_577, 0)), READY),
        Malformed::OutOfRange
    );
    assert_eq!(
        malformed(&request(28, &read_body(0, 1_048_577, 0)), READY),
        Malformed::OutOfRange
    );
    assert!(matches!(
        decoded(&request(15, &read_body(0, 1_048_576, 0)), READY),
        Request::Read { .. }
    ));
    let max = u64::try_from(i64::MAX).unwrap();
    assert_eq!(
        malformed(&request(15, &read_body(max, 1, 0)), READY),
        Malformed::OutOfRange
    );
    assert_eq!(
        malformed(&request(15, &read_body(u64::MAX, 4096, 0)), READY),
        Malformed::OutOfRange
    );
    assert!(matches!(
        decoded(&request(15, &read_body(max - 1, 1, 0)), READY),
        Request::Read { .. }
    ));
    // A directory offset is the layer's cookie, not a byte offset.
    assert!(matches!(
        decoded(&request(28, &read_body(u64::MAX, 4096, 0)), READY),
        Request::Readdir { .. }
    ));
}

#[test]
fn errors_reply_as_the_spec_says() {
    let reply = |bytes: &[u8], phase| match decode(bytes, phase, 0) {
        Err(DecodeError::Request(error)) => {
            assert_eq!(
                error.header,
                header(u32::from_le_bytes(bytes[4..8].try_into().unwrap()))
            );
            error.errno()
        }
        other => panic!("{other:?}"),
    };
    let mut long = vec![b'x'; 256];
    long.push(0);
    assert_eq!(reply(&request(1, &long), READY), Some(Errno::ENAMETOOLONG));
    assert_eq!(reply(&request(1, b"a/b\0"), READY), Some(Errno::EINVAL));
    assert_eq!(
        reply(&request(15, &read_body(0, 1_048_577, 0)), READY),
        Some(Errno::EINVAL)
    );
    assert_eq!(reply(&request(3, &[0; 15]), READY), Some(Errno::EIO));
    assert_eq!(
        reply(&request(3, &[0; 16]), Phase::AwaitingInit),
        Some(Errno::EIO)
    );
    // No reply to the requests that take none, even when malformed.
    assert_eq!(reply(&request(2, &[0; 7]), READY), None);
    assert_eq!(reply(&request(36, &[0; 7]), READY), None);
    assert_eq!(reply(&request(42, &[0; 7]), READY), None);
}

#[test]
fn statx_decodes() {
    let body = le(&[
        &1u32.to_le_bytes(),
        &[0xee; 4],
        &77u64.to_le_bytes(),
        &0x800u32.to_le_bytes(),
        &0xfffu32.to_le_bytes(),
    ]);
    assert_eq!(
        decoded(&request(52, &body), READY),
        Request::Statx {
            fh: Some(77),
            flags: 0x800,
            mask: 0xfff,
            reply: to()
        }
    );
    let mut without_fh = body.clone();
    without_fh[0] = 0;
    assert!(matches!(
        decoded(&request(52, &without_fh), READY),
        Request::Statx { fh: None, .. }
    ));
    let mut unknown = body.clone();
    unknown[0] = 2;
    assert_eq!(
        malformed(&request(52, &unknown), READY),
        Malformed::UnknownFlags
    );
    for found in [23, 25] {
        assert_eq!(
            malformed(&request(52, &vec![0; found]), READY),
            Malformed::BodyLength {
                expected: 24,
                found
            }
        );
    }
}

#[test]
fn open_flags_word_is_checked_from_7_33() {
    // Reserved below 7.33; from it, `open_flags`, whose one defined bit
    // needs a flag never selected (fuse-codec.md 5.4, point 6).
    let open = le(&[&0u32.to_le_bytes(), &1u32.to_le_bytes()]);
    let zero = le(&[&0u32.to_le_bytes(), &0u32.to_le_bytes()]);
    for opcode in [14, 27] {
        assert!(decode(&request(opcode, &open), Phase::Negotiated { minor: 32 }, 0).is_ok());
        assert_eq!(
            malformed(&request(opcode, &open), Phase::Negotiated { minor: 33 }),
            Malformed::UnknownFlags
        );
        assert!(decode(&request(opcode, &zero), Phase::Negotiated { minor: 33 }, 0).is_ok());
    }
}

#[test]
fn refused_requests_reply_only_when_owed() {
    let Err(DecodeError::Request(error)) = decode(&request(3, &[0; 15]), READY, 0) else {
        panic!("not refused");
    };
    let reply = error.into_reply().unwrap();
    assert_eq!(
        reply.as_bytes(),
        [16, 0, 0, 0, 0xfb, 0xff, 0xff, 0xff, 7, 0, 0, 0, 0, 0, 0, 0]
    );
    let Err(DecodeError::Request(error)) = decode(&request(2, &[0; 7]), READY, 0) else {
        panic!("not refused");
    };
    assert_eq!(error.into_reply(), None);
}
