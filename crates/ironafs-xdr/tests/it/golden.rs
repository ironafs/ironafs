// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden vectors of `docs/specs/xdr.md`, 10.1. Each expected byte
//! string is derived by hand from the profile and the cited section of RFC
//! 4506, with the derivation beside it, and is checked both ways: encoding
//! the value gives exactly the bytes, and decoding the bytes gives exactly
//! the value.

use std::fmt::Debug;

use ironafs_xdr::{
    Classified, Decode, Decoder, Encode, Encoder, Error, ErrorKind, Message, MessageType, Sizer,
    decode, encode, encode_into, encoded_len,
};

use crate::support::heap::Owned;
use crate::support::{encoded, hex};
use crate::types::{
    BLOB, Blob, ENTRY, Entry, FIVE, FLAG, Five, Flag, INTS, Ints, LABEL, Label, NOTE, Note, PAIR,
    Pair, SHADE, SHADED, Shade, Shaded,
};

const PRIMITIVE: Message = Message {
    name: "test.primitive",
    cap: 128,
    budget: 1024,
};

/// A primitive value as a whole message under `PRIMITIVE`.
#[derive(Debug, PartialEq)]
struct Primitive<T>(T);

impl<T: Encode> MessageType for Primitive<T> {
    const MESSAGE: Message = PRIMITIVE;
}

impl<T: Encode> Classified for Primitive<T> {
    const SECRET: bool = T::SECRET;
}

impl<T: Encode> Encode for Primitive<T> {
    fn size(&self, sizer: &Sizer) -> Result<(), Error> {
        sizer.put(&self.0)
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        encoder.put(&self.0)
    }
}

impl<'a, T: Decode<'a>> Decode<'a> for Primitive<T> {
    const S_MIN: u64 = T::S_MIN;
    const DEPTH: u32 = T::DEPTH;
    const HEAP_RATIO: Option<u64> = T::HEAP_RATIO;
    const RESERVATIONS: Option<u64> = T::RESERVATIONS;

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        T::decode(decoder).map(Self)
    }
}

/// Checks `value` against `expected` in both directions.
#[track_caller]
fn both_ways<T>(message: &Message, value: &T, expected: &str)
where
    T: for<'a> Decode<'a> + Encode + MessageType + PartialEq + Debug,
{
    assert_eq!(*message, T::MESSAGE);
    let bytes = hex(expected);
    assert_eq!(encoded(value).unwrap(), bytes, "encoding {value:?}");
    assert_eq!(&decode::<T>(&bytes).unwrap(), value, "decoding {expected}");
}

/// G1, §4.1: two's complement, most significant byte first.
#[test]
fn golden_g1_int() {
    both_ways(&PRIMITIVE, &Primitive(0_i32), "00000000");
    // −1 is all ones.
    both_ways(&PRIMITIVE, &Primitive(-1_i32), "ffffffff");
    // −2³¹: only the sign bit.
    both_ways(&PRIMITIVE, &Primitive(i32::MIN), "80000000");
    // 2³¹ − 1: all but the sign bit.
    both_ways(&PRIMITIVE, &Primitive(i32::MAX), "7fffffff");
}

/// G2, §4.2.
#[test]
fn golden_g2_unsigned_int() {
    both_ways(&PRIMITIVE, &Primitive(1_u32), "00000001");
    both_ways(&PRIMITIVE, &Primitive(u32::MAX), "ffffffff");
}

/// G3, §4.5: eight bytes, most significant first.
#[test]
fn golden_g3_hyper() {
    both_ways(&PRIMITIVE, &Primitive(-1_i64), "ffffffff ffffffff");
    both_ways(
        &PRIMITIVE,
        &Primitive(0x0102_0304_0506_0708_u64),
        "01020304 05060708",
    );
}

/// G4, §4.4: the enumeration FALSE = 0, TRUE = 1.
#[test]
fn golden_g4_bool() {
    both_ways(&FLAG, &Flag(false), "00000000");
    both_ways(&FLAG, &Flag(true), "00000001");
}

/// G5, §4.3: an enumeration is its declared integer; VIVID is 9.
#[test]
fn golden_g5_shade() {
    both_ways(&SHADE, &Shade::Vivid, "00000009");
}

/// G6, §4.9: five bytes, then three zero bytes of padding to the unit.
#[test]
fn golden_g6_fixed_opaque() {
    both_ways(&FIVE, &Five(*b"abcde"), "61626364 65000000");
}

/// G7, §4.10 and §3: the length word, the data, then padding of 0, 3, 2,
/// 1, 0 and 3 bytes for lengths 0 to 5.
#[test]
fn golden_g7_variable_opaque() {
    both_ways(&BLOB, &Blob(vec![]), "00000000");
    both_ways(&BLOB, &Blob(vec![1]), "00000001 01000000");
    both_ways(&BLOB, &Blob(vec![1, 2]), "00000002 01020000");
    both_ways(&BLOB, &Blob(vec![1, 2, 3]), "00000003 01020300");
    both_ways(&BLOB, &Blob(vec![1, 2, 3, 4]), "00000004 01020304");
    both_ways(
        &BLOB,
        &Blob(vec![1, 2, 3, 4, 5]),
        "00000005 01020304 05000000",
    );
}

/// G8, §4.11: a string is encoded as opaque data; "files" is 5 bytes,
/// 66 69 6c 65 73, padded by 3.
#[test]
fn golden_g8_label() {
    both_ways(
        &LABEL,
        &Label(b"files".owned()),
        "00000005 66696c65 73000000",
    );
}

/// G9, §4.13: the element count, then each element.
#[test]
fn golden_g9_unsigned_int_array() {
    both_ways(&INTS, &Ints(vec![7, 8]), "00000002 00000007 00000008");
}

/// G10, §4.12: no count; "a" is 8 bytes, "" is 4.
#[test]
fn golden_g10_pair() {
    both_ways(
        &PAIR,
        &Pair([Label(b"a".owned()), Label(vec![])]),
        "00000001 61000000 00000000",
    );
}

/// G11, §4.15 and §4.16: the discriminant, then the arm; PLAIN's arm is
/// void, LINKED's a string ("x" = 78), STAMPED's a hyper.
#[test]
fn golden_g11_entry() {
    both_ways(&ENTRY, &Entry::Plain, "00000000");
    both_ways(
        &ENTRY,
        &Entry::Linked(b"x".owned()),
        "00000001 00000001 78000000",
    );
    both_ways(
        &ENTRY,
        &Entry::Stamped(0x0102_0304_0506_0708),
        "00000002 01020304 05060708",
    );
}

/// G12, §4.14 with §4.10, §4.11 and §4.15 nested: the components in order.
/// title "readme" (72 65 61 64 6d 65): 6, the bytes, 2 of padding, 12
/// bytes; what LINKED "docs" (64 6f 63 73): 1, 4, the bytes, 12 bytes;
/// owner "root" (72 6f 6f 74): 4, the bytes, 8 bytes; body 01 to 05: 5,
/// the bytes, 3 of padding, 12 bytes. 44 in all.
#[test]
fn golden_g12_note() {
    let note = Note {
        title: b"readme".owned(),
        what: Entry::Linked(b"docs".owned()),
        owner: b"root".owned(),
        body: vec![1, 2, 3, 4, 5],
    };
    let expected = "00000006 72656164 6d650000 00000001 00000004 646f6373 \
                    00000004 726f6f74 00000005 01020304 05000000";
    assert_eq!(hex(expected).len(), 44);
    both_ways(&NOTE, &note, expected);
}

/// G13, §4.16: void is nothing.
#[test]
fn golden_g13_void() {
    both_ways(&PRIMITIVE, &Primitive(()), "");
}

/// Checks the shared width and the value and byte round trips through
/// both public encode entry points.
#[track_caller]
fn shaded_both_ways(value: Shaded, expected: &str) {
    both_ways(&SHADED, &value, expected);
    let bytes = hex(expected);
    assert_eq!(bytes.len(), 12);
    assert_eq!(encoded_len(&value), Ok(12));
    assert_eq!(encode(&decode::<Shaded>(&bytes).unwrap()).unwrap(), bytes);
    assert_eq!(decode::<Shaded>(&encode(&value).unwrap()).unwrap(), value);
}

/// G14, §§4.3, 4.5 and 4.15: PALE (1), then its explicit unsigned hyper.
#[test]
fn golden_g14_shaded_explicit_arm() {
    shaded_both_ways(
        Shaded::Pale(0x0102_0304_0506_0708),
        "00000001 01020304 05060708",
    );
}

/// G15, §§4.3, 4.5 and 4.15: VIVID (9), then the default unsigned hyper.
#[test]
fn golden_g15_shaded_default_arm() {
    shaded_both_ways(
        Shaded::Default(Shade::Vivid, 0xfedc_ba98_7654_3210),
        "00000009 fedcba98 76543210",
    );
}

/// A default arm with G14's discriminant and payload must be refused.
#[test]
fn shaded_refuses_default_with_an_explicit_discriminant() {
    let value = Shaded::Default(Shade::Pale, 0x0102_0304_0506_0708);
    let check = |error: Error| {
        assert_eq!(error.kind(), ErrorKind::DefaultArmMismatch);
        assert_eq!(error.name(), Some("test.shaded.tone"));
        assert_eq!(error.limit_value(), None);
        assert_eq!(error.offset(), None);
        assert_eq!(error.extent(), None);
        assert_eq!(
            format!("{error}"),
            "default arm discriminant selects an explicit arm: test.shaded.tone"
        );
    };
    check(encode(&value).unwrap_err());
    check(encoded_len(&value).unwrap_err());

    let short = vec![0xaa; 4];
    let roomy = vec![0xaa; 12];
    assert!((1..12).contains(&short.capacity()));
    assert!(roomy.capacity() >= 12);
    for mut out in [Vec::new(), short, roomy] {
        let before = (out.as_ptr(), out.capacity());
        check(encode_into(&value, &mut out).unwrap_err());
        assert_eq!(out, []);
        assert_eq!((out.as_ptr(), out.capacity()), before);
    }
}
