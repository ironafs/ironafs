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
    Encode, Encoder, array_len, encode_message, fixed_opaque_len, opaque_len, word_length,
    zero_to_capacity,
};
use crate::{Cap, Classified, Error, ErrorKind, Message, MessageType};

const MESSAGE: Message = Message {
    name: "test.m",
    cap: 32,
    budget: 1024,
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

impl Classified for Short<'_> {
    const SECRET: bool = false;
}

impl Encode for Short<'_> {
    fn encoded_len(&self) -> Result<u64, Error> {
        opaque_len(&SHORT, self.0.len())
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        encoder.put_opaque(&SHORT, self.0)
    }
}

/// A value whose size understates what it writes.
struct Liar;

impl Classified for Liar {
    const SECRET: bool = false;
}

impl Encode for Liar {
    fn encoded_len(&self) -> Result<u64, Error> {
        Ok(8)
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        encoder.put_u32(1)?;
        encoder.put_u64(1)
    }
}

/// Encodes `value` as one `message` into a new buffer, by the path every
/// message may take.
fn encode_with<T: Encode>(message: &Message, value: &T) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    encode_message(message, value, &mut out)?;
    Ok(out)
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
    assert_eq!(
        encode_with(&MESSAGE, &-2_i32).unwrap(),
        [0xff, 0xff, 0xff, 0xfe]
    );
    assert_eq!(encode_with(&MESSAGE, &7_u32).unwrap(), [0, 0, 0, 7]);
    assert_eq!(
        encode_with(&MESSAGE, &0x0102_0304_0506_0708_u64).unwrap(),
        [1, 2, 3, 4, 5, 6, 7, 8]
    );
    assert_eq!(
        encode_with(&MESSAGE, &i64::MIN).unwrap(),
        [0x80, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(encode_with(&MESSAGE, &()).unwrap(), []);
}

#[test]
fn opaque_is_length_data_and_zero_padding() {
    assert_eq!(encode_with(&MESSAGE, &Short(b"")).unwrap(), [0, 0, 0, 0]);
    assert_eq!(
        encode_with(&MESSAGE, &Short(b"ab")).unwrap(),
        [0, 0, 0, 2, b'a', b'b', 0, 0]
    );
    assert_eq!(
        encode_with(&MESSAGE, &Short(b"abcd")).unwrap(),
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
    impl Classified for Pair {
        const SECRET: bool = false;
    }

    impl Encode for Pair {
        fn encoded_len(&self) -> Result<u64, Error> {
            array_len(&SOME, &[7_u32, 8])
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_array(&SOME, &[7_u32, 8])
        }
    }
    assert_eq!(
        encode_with(&MESSAGE, &Pair).unwrap(),
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
    let error = encode_message(&message, &Short(b"abcd"), &mut out).unwrap_err();
    assert_error(&error, ErrorKind::MessageTooLarge, Some("test.m"), Some(7));
    assert_eq!(out.capacity(), 0);
}

#[test]
fn encode_into_requests_once_and_never_grows() {
    let mut out = Vec::new();
    encode_message(&MESSAGE, &Short(b"abc"), &mut out).unwrap();
    assert_eq!(out, [0, 0, 0, 3, b'a', b'b', b'c', 0]);
    // A buffer that already has the room is written in place.
    let mut out = Vec::with_capacity(8);
    out.push(0xee);
    let before = out.as_ptr();
    encode_message(&MESSAGE, &Short(b"abc"), &mut out).unwrap();
    assert_eq!(out.as_ptr(), before);
    assert_eq!(out, [0, 0, 0, 3, b'a', b'b', b'c', 0]);
}

#[test]
fn value_writing_past_its_size_fails_and_leaves_nothing() {
    let mut out = Vec::new();
    let error = encode_message(&MESSAGE, &Liar, &mut out).unwrap_err();
    assert_error(&error, ErrorKind::MessageTooLarge, Some("test.m"), Some(32));
    assert_eq!(out, []);
}

#[test]
fn value_writing_short_of_its_size_fails() {
    struct Short8;
    impl Classified for Short8 {
        const SECRET: bool = false;
    }

    impl Encode for Short8 {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(8)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_u32(1)
        }
    }
    let error = encode_with(&MESSAGE, &Short8).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MessageTooLarge);
}

#[test]
fn bool_encodes_as_zero_or_one() {
    struct Flags;
    impl Classified for Flags {
        const SECRET: bool = false;
    }

    impl Encode for Flags {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(8)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_bool(false)?;
            encoder.put_bool(true)
        }
    }
    assert_eq!(
        encode_with(&MESSAGE, &Flags).unwrap(),
        [0, 0, 0, 0, 0, 0, 0, 1]
    );
}

#[test]
fn put_opaque_refuses_over_cap_even_after_a_size_pass_that_passed() {
    /// Sizes itself as 12 bytes, then writes 5 bytes under a cap of 4.
    struct Sneaky;
    impl Classified for Sneaky {
        const SECRET: bool = false;
    }

    impl Encode for Sneaky {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(12)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_opaque(&SHORT, b"abcde")
        }
    }
    let error = encode_with(&MESSAGE, &Sneaky).unwrap_err();
    assert_error(
        &error,
        ErrorKind::CapExceeded,
        Some("test.m.short"),
        Some(4),
    );
}

#[test]
fn put_array_refuses_over_cap_even_after_a_size_pass_that_passed() {
    /// No bound, cap 4 bytes.
    const TIGHT: Cap = Cap {
        name: "test.m.tight",
        bound: None,
        bytes: 4,
    };
    /// Sizes itself as one word, then writes two.
    struct Wide;
    impl Classified for Wide {
        const SECRET: bool = false;
    }

    impl Encode for Wide {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(4)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_u64(1)
        }
    }
    /// Sizes itself as 12 bytes, room for what its array writes, then
    /// writes one element of 8 bytes under a cap of 4.
    struct Sneaky;
    impl Classified for Sneaky {
        const SECRET: bool = false;
    }

    impl Encode for Sneaky {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(12)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_array(&TIGHT, &[Wide])
        }
    }
    let mut out = Vec::new();
    let error = encode_message(&MESSAGE, &Sneaky, &mut out).unwrap_err();
    assert_error(
        &error,
        ErrorKind::CapExceeded,
        Some("test.m.tight"),
        Some(4),
    );
    assert_eq!(out, []);
}

#[test]
fn put_array_refuses_a_count_over_its_bound() {
    /// Sizes itself as the array, without its checks.
    struct Four;
    impl Classified for Four {
        const SECRET: bool = false;
    }

    impl Encode for Four {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(20)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_array(&FEW, &[0_u32; 4])
        }
    }
    let error = encode_with(&MESSAGE, &Four).unwrap_err();
    assert_error(
        &error,
        ErrorKind::LengthOverBound,
        Some("test.m.few"),
        Some(3),
    );
}

#[test]
fn encoder_debug_shows_no_output_bytes() {
    /// Writes a key-like word, then formats the encoder.
    struct Peek;
    impl Classified for Peek {
        const SECRET: bool = false;
    }

    impl Encode for Peek {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(4)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
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
    encode_with(&MESSAGE, &Peek).unwrap();
}

#[test]
fn zero_to_capacity_overwrites_contents_and_spare_capacity() {
    let mut out = Vec::with_capacity(16);
    out.extend_from_slice(&[0xaa; 16]);
    out.truncate(3);
    let before = (out.as_ptr(), out.capacity());
    zero_to_capacity(&mut out);
    assert_eq!(out.len(), out.capacity());
    assert!(out.iter().all(|byte| *byte == 0));
    assert_eq!((out.as_ptr(), out.capacity()), before);
}

#[test]
fn encode_into_failure_leaves_buffer_empty_in_place() {
    let mut out = Vec::with_capacity(32);
    out.extend_from_slice(&[0xaa; 32]);
    let before = (out.as_ptr(), out.capacity());
    let error = encode_message(&MESSAGE, &Liar, &mut out).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MessageTooLarge);
    assert_eq!(out, []);
    assert_eq!((out.as_ptr(), out.capacity()), before);
}

#[test]
fn encode_into_reuses_a_buffer_holding_old_bytes() {
    let mut out = vec![0xaa; 32];
    let before = out.as_ptr();
    encode_message(&MESSAGE, &Short(b"ab"), &mut out).unwrap();
    assert_eq!(out, [0, 0, 0, 2, b'a', b'b', 0, 0]);
    assert_eq!(out.as_ptr(), before);
}

#[test]
fn encoder_debug_shows_the_message_name_only() {
    /// Formats the encoder before and after writing.
    struct Twice;
    impl Classified for Twice {
        const SECRET: bool = false;
    }

    impl Encode for Twice {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(8)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            let before = format!("{encoder:?}");
            encoder.put_u32(1)?;
            let after = format!("{encoder:#?}");
            encoder.put_u32(2)?;
            assert_eq!(before, "Encoder { message: \"test.m\", .. }");
            assert_eq!(after, "Encoder {\n    message: \"test.m\",\n    ..\n}");
            Ok(())
        }
    }
    let mut out = Vec::new();
    encode_message(&MESSAGE, &Twice, &mut out).unwrap();
}

#[test]
fn encode_into_never_grows_an_allocated_buffer_for_any_message() {
    // Growing would move the bytes and release the old block, which may
    // hold an earlier secret encoding, without the caller's wipe.
    let mut out = Vec::with_capacity(4);
    out.extend_from_slice(&[0xaa; 4]);
    let before = (out.as_ptr(), out.capacity());
    let error = encode_message(&ROOMY, &Past::new(&out), &mut out).unwrap_err();
    assert_error(&error, ErrorKind::AllocationFailed, None, None);
    assert_eq!(out, []);
    assert_eq!((out.as_ptr(), out.capacity()), before);
    // The same for a secret-bearing message, through its entry point.
    let mut out = Vec::with_capacity(4);
    out.extend_from_slice(&[0xaa; 4]);
    let before = (out.as_ptr(), out.capacity());
    let long = Holder(Holder(Long(Past::new(&out))));
    let error = super::encode_into(&long, &mut out).unwrap_err();
    assert_error(&error, ErrorKind::AllocationFailed, None, None);
    assert_eq!(out, []);
    assert_eq!((out.as_ptr(), out.capacity()), before);
}

/// A secret leaf of any whole number of words.
struct Long(Past);

impl Classified for Long {
    const SECRET: bool = true;
}

impl Encode for Long {
    fn encoded_len(&self) -> Result<u64, Error> {
        self.0.encoded_len()
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        (0..(self.0).0).try_for_each(|_| encoder.put_u32(1))
    }
}

/// A message with room for any test buffer's capacity and more.
const ROOMY: Message = Message {
    cap: 4096,
    ..MESSAGE
};

/// Whole words, one more than fit in the capacity of the buffer it is
/// built from: whatever capacity the allocator gave, too big for it.
struct Past(usize);

impl Past {
    fn new(out: &Vec<u8>) -> Self {
        Self(out.capacity() / 4 + 1)
    }
}

impl Classified for Past {
    const SECRET: bool = false;
}

impl Encode for Past {
    fn encoded_len(&self) -> Result<u64, Error> {
        Ok(4 * u64::try_from(self.0).unwrap())
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        (0..self.0).try_for_each(|_| encoder.put_u32(1))
    }
}

/// A message, a way to encode into a buffer under it, and the error kind it
/// must raise.
type Case<'c> = (
    &'c Message,
    &'c dyn Fn(&Message, &mut Vec<u8>) -> Result<(), Error>,
    ErrorKind,
);

#[test]
fn encode_into_empties_the_buffer_on_every_error() {
    let small = Message { cap: 7, ..MESSAGE };
    let cases: [Case<'_>; 4] = [
        // The value's own size check refuses it.
        (
            &MESSAGE,
            &|m, out| encode_message(m, &Short(b"abcdefghi"), out),
            ErrorKind::LengthOverBound,
        ),
        // Above the message cap, before anything is reserved.
        (
            &small,
            &|m, out| encode_message(m, &Short(b"abcd"), out),
            ErrorKind::MessageTooLarge,
        ),
        // An allocated buffer too small for the message.
        (
            &MESSAGE,
            &|_, out| encode_message(&ROOMY, &Past::new(out), out),
            ErrorKind::AllocationFailed,
        ),
        // A value that writes past its size.
        (
            &MESSAGE,
            &|m, out| encode_message(m, &Liar, out),
            ErrorKind::MessageTooLarge,
        ),
    ];
    for (message, run, kind) in cases {
        let mut out = vec![0xaa; 8];
        let before = (out.as_ptr(), out.capacity());
        let error = run(message, &mut out).unwrap_err();
        assert_eq!(error.kind(), kind, "{error}");
        assert_eq!(out, [], "{error}");
        assert_eq!((out.as_ptr(), out.capacity()), before, "{error}");
    }
}

#[test]
fn secret_encode_into_writes_in_place_or_into_a_fresh_buffer() {
    let mut out = Vec::with_capacity(8);
    out.push(0xaa);
    let before = out.as_ptr();
    super::encode_into(&Holder(Key), &mut out).unwrap();
    assert_eq!(out, [0x5e, 0xc2, 0xe7, 0xed]);
    assert_eq!(out.as_ptr(), before);
    let mut fresh = Vec::new();
    super::encode_into(&Holder(Key), &mut fresh).unwrap();
    assert_eq!(fresh, [0x5e, 0xc2, 0xe7, 0xed]);
}

/// A plain message type with its descriptor.
struct Greeting;

impl MessageType for Greeting {
    const MESSAGE: Message = MESSAGE;
}

impl Classified for Greeting {
    const SECRET: bool = false;
}

impl Encode for Greeting {
    fn encoded_len(&self) -> Result<u64, Error> {
        Ok(4)
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        encoder.put_u32(9)
    }
}

#[test]
fn encode_uses_the_descriptor_its_type_carries() {
    assert_eq!(super::encode(&Greeting).unwrap(), [0, 0, 0, 9]);
}

#[test]
fn encode_into_uses_the_descriptor_its_type_carries() {
    let mut out = Vec::new();
    super::encode_into(&Greeting, &mut out).unwrap();
    assert_eq!(out, [0, 0, 0, 9]);
}

/// A secret leaf: its own field is secret.
struct Key;

impl Classified for Key {
    const SECRET: bool = true;
}

impl Encode for Key {
    fn encoded_len(&self) -> Result<u64, Error> {
        Ok(4)
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        encoder.put_u32(0x5ec2_e7ed)
    }
}

/// Holds a value of `C`, secret exactly when `C` is: how generated code
/// takes its flag from its components.
struct Holder<C>(C);

impl<C: Encode> Classified for Holder<C> {
    const SECRET: bool = C::SECRET;
}

impl<C: Encode> Encode for Holder<C> {
    fn encoded_len(&self) -> Result<u64, Error> {
        self.0.encoded_len()
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        encoder.put(&self.0)
    }
}

impl<C: Encode> MessageType for Holder<C> {
    const MESSAGE: Message = ROOMY;
}

// Secrecy carries up through any depth of nesting, at compile time, and
// a message type has the one flag, whichever of its traits reads it.
const _: () = assert!(<Holder<Holder<Key>> as Classified>::SECRET);
const _: () = assert!(!<Holder<Holder<u32>> as Classified>::SECRET);

/// The flag a generic caller reads through a type's `Encode`.
const fn flag_as_encode<T: Encode>() -> bool {
    T::SECRET
}

/// The flag a generic caller reads through a type's `MessageType`.
const fn flag_as_message<T: MessageType>() -> bool {
    T::SECRET
}

const _: () = assert!(flag_as_encode::<Holder<Key>>() && flag_as_message::<Holder<Key>>());
const _: () = assert!(!flag_as_encode::<Holder<u32>>() && !flag_as_message::<Holder<u32>>());

#[test]
fn a_holder_of_a_secret_encodes_only_into_the_callers_buffer() {
    let mut out = Vec::new();
    super::encode_into(&Holder(Holder(Key)), &mut out).unwrap();
    assert_eq!(out, [0x5e, 0xc2, 0xe7, 0xed]);
    assert_eq!(super::encode(&Holder(7_u32)).unwrap(), [0, 0, 0, 7]);
}

#[test]
fn put_carries_the_bytes_left_back_to_its_parent() {
    /// Sizes itself as one word, then writes two through its components.
    struct Two;
    impl Classified for Two {
        const SECRET: bool = false;
    }

    impl Encode for Two {
        fn encoded_len(&self) -> Result<u64, Error> {
            Ok(4)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put(&1_u32)?;
            encoder.put(&2_u32)
        }
    }
    let error = encode_message(&MESSAGE, &Two, &mut Vec::new()).unwrap_err();
    assert_error(&error, ErrorKind::MessageTooLarge, Some("test.m"), Some(32));
}

#[test]
fn a_write_past_the_capacity_is_refused_whatever_the_count() {
    // The count is right by construction (`Encoder`); the capacity check
    // holds the buffer in place even if it were not.
    let mut out: Vec<u8> = Vec::with_capacity(4);
    let before = (out.as_ptr(), out.capacity());
    let room = u64::try_from(out.capacity()).unwrap();
    let mut remaining = room + 8;
    let mut encoder: Encoder<'_, u32> = Encoder {
        out: &mut out,
        remaining: &mut remaining,
        message: MESSAGE.name,
        cap: MESSAGE.cap,
        value: core::marker::PhantomData,
    };
    let filled = (0..room).try_for_each(|_| encoder.put_bytes(&[0xaa]));
    assert_eq!(filled, Ok(()));
    let error = encoder.put_u32(1).unwrap_err();
    assert_error(&error, ErrorKind::MessageTooLarge, Some("test.m"), Some(32));
    assert_eq!(remaining, 8, "a refused write takes nothing from the count");
    assert_eq!((out.as_ptr(), out.capacity()), before);
    assert_eq!(u64::try_from(out.len()).unwrap(), room);
}
