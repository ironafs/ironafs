// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_macros,
    reason = "the tests build their values and buffers outside the encoder"
)]

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use super::{
    Encode, Encoder, Plain, array_len, encode, encode_into, fixed_opaque_len, opaque_len,
    word_length, zero_to_capacity,
};
use crate::{Cap, Error, ErrorKind, Message};

const MESSAGE: Message = Message {
    name: "test.m",
    cap: 32,
    budget: 1024,
    secret: false,
};

/// Bound 8 bytes, cap 4: below its bound.
const SHORT: Cap = Cap {
    name: "test.m.short",
    bound: Some(8),
    bytes: 4,
};

/// Bound 3 elements, cap 12 bytes.
const FEW: Cap = Cap {
    name: "test.m.few",
    bound: Some(3),
    bytes: 12,
};

/// No bound, cap 8 bytes.
const SOME: Cap = Cap {
    name: "test.m.some",
    bound: None,
    bytes: 8,
};

/// Variable-length opaque under `SHORT`, as a message.
struct Short<'a>(&'a [u8]);

impl Plain for Short<'_> {}

impl Encode for Short<'_> {
    fn encoded_len(&self) -> Result<u64, Error> {
        opaque_len(&SHORT, self.0.len())
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_opaque(&SHORT, self.0)
    }
}

/// A value whose size understates what it writes.
struct Liar;

impl Plain for Liar {}

impl Encode for Liar {
    fn encoded_len(&self) -> Result<u64, Error> {
        Ok(8)
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        encoder.put_u32(1)?;
        encoder.put_u64(1)
    }
}

#[track_caller]
fn assert_error(error: &Error, kind: ErrorKind, name: Option<&str>, limit: Option<u64>) {
    assert_eq!(error.kind(), kind, "{error}");
    assert_eq!(error.name(), name, "{error}");
    assert_eq!(error.limit_value(), limit, "{error}");
    assert_eq!(error.offset(), None, "{error}");
}

#[test]
fn numbers_encode_most_significant_byte_first() {
    assert_eq!(encode(&MESSAGE, &-2_i32).unwrap(), [0xff, 0xff, 0xff, 0xfe]);
    assert_eq!(encode(&MESSAGE, &7_u32).unwrap(), [0, 0, 0, 7]);
    assert_eq!(
        encode(&MESSAGE, &0x0102_0304_0506_0708_u64).unwrap(),
        [1, 2, 3, 4, 5, 6, 7, 8]
    );
    assert_eq!(
        encode(&MESSAGE, &i64::MIN).unwrap(),
        [0x80, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(encode(&MESSAGE, &()).unwrap(), []);
}

#[test]
fn opaque_is_length_data_and_zero_padding() {
    assert_eq!(encode(&MESSAGE, &Short(b"")).unwrap(), [0, 0, 0, 0]);
    assert_eq!(
        encode(&MESSAGE, &Short(b"ab")).unwrap(),
        [0, 0, 0, 2, b'a', b'b', 0, 0]
    );
    assert_eq!(
        encode(&MESSAGE, &Short(b"abcd")).unwrap(),
        [0, 0, 0, 4, b'a', b'b', b'c', b'd']
    );
}

#[test]
fn fixed_opaque_len_adds_padding() {
    let lengths: Vec<u64> = (0..6).map(|n| fixed_opaque_len(n).unwrap()).collect();
    assert_eq!(lengths, [0, 4, 4, 4, 4, 8]);
}

#[test]
fn opaque_len_checks_bound_before_cap() {
    assert_error(
        &opaque_len(&SHORT, 9).unwrap_err(),
        ErrorKind::LengthOverBound,
        Some("test.m.short"),
        Some(8),
    );
    assert_error(
        &opaque_len(&SHORT, 5).unwrap_err(),
        ErrorKind::CapExceeded,
        Some("test.m.short"),
        Some(4),
    );
    assert_eq!(opaque_len(&SHORT, 4).unwrap(), 8);
}

#[test]
fn array_len_compares_count_with_bound_and_bytes_with_cap() {
    // Four elements of 4 bytes: 16 bytes, but the count is checked first.
    assert_error(
        &array_len(&FEW, &[0_u32; 4]).unwrap_err(),
        ErrorKind::LengthOverBound,
        Some("test.m.few"),
        Some(3),
    );
    // Three elements of 8 bytes: within the bound, 24 bytes over the cap.
    assert_error(
        &array_len(&FEW, &[0_u64; 3]).unwrap_err(),
        ErrorKind::CapExceeded,
        Some("test.m.few"),
        Some(12),
    );
    assert_eq!(array_len(&FEW, &[0_u32; 3]).unwrap(), 16);
    assert_eq!(array_len(&SOME, &[0_u32; 2]).unwrap(), 12);
}

#[test]
fn put_array_writes_count_then_elements() {
    struct Pair;
    impl Plain for Pair {}

    impl Encode for Pair {
        fn encoded_len(&self) -> Result<u64, Error> {
            array_len(&SOME, &[7_u32, 8])
        }

        fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
            encoder.put_array(&SOME, &[7_u32, 8])
        }
    }
    assert_eq!(
        encode(&MESSAGE, &Pair).unwrap(),
        [0, 0, 0, 2, 0, 0, 0, 7, 0, 0, 0, 8]
    );
}

#[test]
fn word_length_refuses_lengths_past_32_bits() {
    let Ok(too_long) = usize::try_from(u64::from(u32::MAX) + 1) else {
        return;
    };
    assert_error(
        &word_length(&SOME, too_long).unwrap_err(),
        ErrorKind::LengthNotRepresentable,
        Some("test.m.some"),
        None,
    );
    assert_eq!(word_length(&SOME, 5).unwrap(), 5);
}

#[test]
fn encode_over_message_cap_reserves_nothing() {
    let message = Message { cap: 7, ..MESSAGE };
    let mut out = Vec::new();
    let error = encode_into(&message, &Short(b"abcd"), &mut out).unwrap_err();
    assert_error(&error, ErrorKind::MessageTooLarge, Some("test.m"), Some(7));
    assert_eq!(out.capacity(), 0);
}

#[test]
fn encode_into_requests_once_and_never_grows() {
    let mut out = Vec::new();
    encode_into(&MESSAGE, &Short(b"abc"), &mut out).unwrap();
    assert_eq!(out, [0, 0, 0, 3, b'a', b'b', b'c', 0]);
    // A buffer that already has the room is written in place.
    let mut out = Vec::with_capacity(8);
    out.push(0xee);
    let before = out.as_ptr();
    encode_into(&MESSAGE, &Short(b"abc"), &mut out).unwrap();
    assert_eq!(out.as_ptr(), before);
    assert_eq!(out, [0, 0, 0, 3, b'a', b'b', b'c', 0]);
}

#[test]
fn value_writing_past_its_size_fails_and_leaves_nothing() {
    let mut out = vec![1, 2, 3];
    let error = encode_into(&MESSAGE, &Liar, &mut out).unwrap_err();
    assert_error(&error, ErrorKind::MessageTooLarge, Some("test.m"), Some(32));
    assert_eq!(out, []);
}

#[test]
fn value_writing_short_of_its_size_fails() {
    struct Short8;
    impl Plain for Short8 {}

    impl Encode for Short8 {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(8)
        }

        fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
            encoder.put_u32(1)
        }
    }
    let error = encode(&MESSAGE, &Short8).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MessageTooLarge);
}

#[test]
fn bool_encodes_as_zero_or_one() {
    struct Flags;
    impl Plain for Flags {}

    impl Encode for Flags {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(8)
        }

        fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
            encoder.put_bool(false)?;
            encoder.put_bool(true)
        }
    }
    assert_eq!(encode(&MESSAGE, &Flags).unwrap(), [0, 0, 0, 0, 0, 0, 0, 1]);
}

#[test]
fn put_opaque_refuses_over_cap_even_after_a_size_pass_that_passed() {
    /// Sizes itself as 12 bytes, then writes 5 bytes under a cap of 4.
    struct Sneaky;
    impl Plain for Sneaky {}

    impl Encode for Sneaky {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(12)
        }

        fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
            encoder.put_opaque(&SHORT, b"abcde")
        }
    }
    let error = encode(&MESSAGE, &Sneaky).unwrap_err();
    assert_error(
        &error,
        ErrorKind::CapExceeded,
        Some("test.m.short"),
        Some(4),
    );
}

#[test]
fn encoder_debug_shows_no_output_bytes() {
    /// Writes a key-like word, then formats the encoder.
    struct Peek;
    impl Plain for Peek {}

    impl Encode for Peek {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(4)
        }

        fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
            encoder.put_u32(0x5ec2_e7ed)?;
            let plain = format!("{encoder:?}");
            let alternate = format!("{encoder:#?}");
            for text in [plain, alternate] {
                assert!(!text.contains("94"), "{text}");
                assert!(!text.contains("237"), "{text}");
                assert!(!text.contains("out"), "{text}");
            }
            Ok(())
        }
    }
    encode(&MESSAGE, &Peek).unwrap();
}

#[test]
fn zero_to_capacity_overwrites_contents_and_spare_capacity() {
    let mut out = Vec::with_capacity(16);
    out.extend_from_slice(&[0xaa; 16]);
    out.truncate(3);
    zero_to_capacity(&mut out);
    assert_eq!(out, [0; 16]);
}

#[test]
fn encode_into_failure_leaves_buffer_empty_in_place() {
    let mut out = Vec::with_capacity(32);
    out.extend_from_slice(&[0xaa; 32]);
    let before = (out.as_ptr(), out.capacity());
    let error = encode_into(&MESSAGE, &Liar, &mut out).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MessageTooLarge);
    assert_eq!(out, []);
    assert_eq!((out.as_ptr(), out.capacity()), before);
}

#[test]
fn encode_into_reuses_a_buffer_holding_old_bytes() {
    let mut out = vec![0xaa; 32];
    let before = out.as_ptr();
    encode_into(&MESSAGE, &Short(b"ab"), &mut out).unwrap();
    assert_eq!(out, [0, 0, 0, 2, b'a', b'b', 0, 0]);
    assert_eq!(out.as_ptr(), before);
}
