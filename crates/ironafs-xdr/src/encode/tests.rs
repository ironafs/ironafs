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
use core::cell::Cell;

use super::{
    Encode, Encoder, Sizer, encode_message, fixed_opaque_len, opaque_len, word_length,
    zero_to_capacity,
};
use crate as xdr;
use crate::{Cap, Classified, Error, ErrorKind, Message, MessageType};

/// One observation at the shared output path's actual phase boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Sizing,
    SizingRefused,
    Reserve,
    Write,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Observation {
    phase: Phase,
    len: usize,
    capacity: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WipeObservation {
    len_before: usize,
    zeroed_to_capacity: bool,
}

/// Carries the observations for one encode call.
struct Traced<'a, T> {
    value: &'a T,
    phases: Cell<[Option<Observation>; 3]>,
    wipes: Cell<[Option<WipeObservation>; 2]>,
}

impl<T: Classified> Classified for Traced<'_, T> {
    const SECRET: bool = T::SECRET;
}

impl<T: MessageType> MessageType for Traced<'_, T> {
    const MESSAGE: Message = T::MESSAGE;
}

impl<T: Encode> Encode for Traced<'_, T> {
    fn size(&self, sizer: &Sizer) -> Result<(), Error> {
        sizer.put(self.value)
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        encoder.put(self.value)
    }

    fn observe_phase(&self, phase: Phase, len: usize, capacity: usize) {
        let mut phases = self.phases.get();
        let slot = phases.iter_mut().find(|slot| slot.is_none()).unwrap();
        *slot = Some(Observation {
            phase,
            len,
            capacity,
        });
        self.phases.set(phases);
    }

    fn observe_wipe(&self, len_before: usize, out: &[u8], capacity: usize) {
        let mut wipes = self.wipes.get();
        let slot = wipes.iter_mut().find(|slot| slot.is_none()).unwrap();
        *slot = Some(WipeObservation {
            len_before,
            zeroed_to_capacity: zeroed_to_capacity(out, capacity),
        });
        self.wipes.set(wipes);
    }
}

/// Traces one call without requesting storage from inside the encode path.
fn trace<T, R>(
    value: &T,
    call: impl FnOnce(&Traced<'_, T>) -> R,
) -> (R, [Option<Observation>; 3], [Option<WipeObservation>; 2]) {
    let traced = Traced {
        value,
        phases: Cell::new([None; 3]),
        wipes: Cell::new([None; 2]),
    };
    let result = call(&traced);
    (
        result,
        traced.phases.into_inner(),
        traced.wipes.into_inner(),
    )
}

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
    fn size(&self, sizer: &Sizer) -> Result<(), Error> {
        sizer.opaque(&SHORT, self.0.len())
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
    fn size(&self, sizer: &Sizer) -> Result<(), Error> {
        sizer.charge(Ok(8))
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
    fn array_len<T: Encode>(cap: &Cap, elements: &[T]) -> Result<u64, Error> {
        let sizer = Sizer::new();
        sizer.array(cap, elements)?;
        Ok(sizer.total.get().unwrap())
    }

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
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.array(&SOME, &[7_u32, 8])
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
    let (result, phases, _) = trace(&Short(b"abc"), |value| {
        encode_message(&MESSAGE, value, &mut out)
    });
    result.unwrap();
    assert_sizing_reserve_write(phases, 0, out.capacity());
    assert_eq!(out, [0, 0, 0, 3, b'a', b'b', b'c', 0]);
    // A buffer that already has the room is written in place.
    let mut out = Vec::with_capacity(8);
    out.push(0xee);
    let before = (out.as_ptr(), out.capacity());
    let (result, phases, _) = trace(&Short(b"abc"), |value| {
        encode_message(&MESSAGE, value, &mut out)
    });
    result.unwrap();
    assert_sizing_reserve_write(phases, before.1, before.1);
    assert_eq!((out.as_ptr(), out.capacity()), before);
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
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.charge(Ok(8))
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
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.charge(Ok(8))
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
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.charge(Ok(12))
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
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.charge(Ok(4))
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_u64(1)
        }
    }
    /// Reports an extra word after its array, so the array's write can
    /// exceed its cap before it exhausts the message's space.
    struct Sneaky;
    impl Classified for Sneaky {
        const SECRET: bool = false;
    }

    impl Encode for Sneaky {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.array(&TIGHT, &[Wide])?;
            sizer.put(&0_u32)
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
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.charge(Ok(20))
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
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.charge(Ok(4))
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

fn zeroed_to_capacity(out: &[u8], capacity: usize) -> bool {
    out.len() == capacity && out.iter().all(|byte| *byte == 0)
}

#[test]
fn zero_to_capacity_overwrites_contents_and_spare_capacity() {
    let mut out = Vec::with_capacity(16);
    out.extend_from_slice(&[0xaa; 16]);
    out.truncate(3);
    let before = (out.as_ptr(), out.capacity());
    zero_to_capacity(&mut out);
    assert!(zeroed_to_capacity(&out, out.capacity()));
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
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.charge(Ok(8))
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
    fn size(&self, sizer: &Sizer) -> Result<(), Error> {
        sizer.put(&self.0)
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
    fn size(&self, sizer: &Sizer) -> Result<(), Error> {
        sizer.charge(Ok(4 * u64::try_from(self.0).unwrap()))
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
    fn size(&self, sizer: &Sizer) -> Result<(), Error> {
        sizer.charge(Ok(4))
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
    fn size(&self, sizer: &Sizer) -> Result<(), Error> {
        sizer.charge(Ok(4))
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
    fn size(&self, sizer: &Sizer) -> Result<(), Error> {
        sizer.put(&self.0)
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
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.charge(Ok(4))
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

/// The sizing fixtures' message rows, all plain, with the common budget.
macro_rules! sizing_message {
    ($type:ty, $name:literal, $cap:literal) => {
        impl Classified for $type {
            const SECRET: bool = false;
        }

        impl MessageType for $type {
            const MESSAGE: Message = Message {
                name: concat!("test.sizing.", $name),
                cap: $cap,
                budget: 1024,
            };
        }
    };
}

#[test]
fn declared_bound_precedes_length_word_width() {
    const TITLE: Cap = Cap {
        name: "test.sizing.repr_bound.title",
        bound: Some(4),
        bytes: 4,
    };
    struct ReprBound(usize);
    sizing_message!(ReprBound, "repr_bound", 16);

    impl Encode for ReprBound {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.opaque(&TITLE, self.0)
        }

        fn encode(&self, _: &mut Encoder<'_, Self>) -> Result<(), Error> {
            panic!("the synthetic field has no backing payload")
        }
    }

    let Ok(length) = usize::try_from(u64::from(u32::MAX) + 1) else {
        return;
    };
    let error = Error::new(ErrorKind::LengthOverBound)
        .named("test.sizing.repr_bound.title")
        .limit(4);
    assert_layout_refusal(&ReprBound(length), error);
}

#[test]
fn enclosing_cap_precedes_a_later_element_failure() {
    const ITEMS: Cap = Cap {
        name: "test.sizing.list.items",
        bound: None,
        bytes: 7,
    };
    const TEXT: Cap = Cap {
        name: "test.sizing.item_text",
        bound: Some(8),
        bytes: 8,
    };
    struct Item(&'static [u8]);
    impl Classified for Item {
        const SECRET: bool = false;
    }
    impl Encode for Item {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.opaque(&TEXT, self.0.len())
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_opaque(&TEXT, self.0)
        }
    }
    struct List([Item; 2]);
    sizing_message!(List, "list", 32);

    impl Encode for List {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.array(&ITEMS, &self.0)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_array(&ITEMS, &self.0)
        }
    }

    let list = List([Item(b"x"), Item(b"123456789")]);
    let error = Error::new(ErrorKind::CapExceeded)
        .named("test.sizing.list.items")
        .limit(7);
    assert_layout_refusal(&list, error);
}

#[expect(
    dead_code,
    reason = "rule 15: the shared integration and fuzz fixtures have additional controls"
)]
mod shaded;

const SOURCE: Error = Error::new(ErrorKind::MessageTooLarge)
    .named("test.sizing.source")
    .limit(4);
const INVALID_VISIT: Error = Error::new(ErrorKind::MessageTooLarge);
const HUGE: u64 = u64::MAX - 3;

fn sizing_phases(capacity: usize) -> [Option<Observation>; 3] {
    let entry = |phase| {
        Some(Observation {
            phase,
            len: 0,
            capacity,
        })
    };
    [entry(Phase::Sizing), entry(Phase::SizingRefused), None]
}

#[track_caller]
fn assert_caller_sizing_refusal<T: Encode + MessageType>(
    value: &T,
    expected: Error,
    out: &mut Vec<u8>,
) {
    let len_before = out.len();
    let before = (out.as_ptr(), out.capacity());
    let (result, phases, wipes) = trace(value, |value| super::encode_into(value, out));
    assert_eq!(result, Err(expected));
    assert_eq!(phases, sizing_phases(before.1));
    assert_eq!(out.as_slice(), []);
    assert_eq!((out.as_ptr(), out.capacity()), before);
    assert_wipes(wipes, [len_before, 0]);
}

#[track_caller]
fn assert_wipes(wipes: [Option<WipeObservation>; 2], lengths_before: [usize; 2]) {
    assert_eq!(
        wipes,
        lengths_before.map(|len_before| Some(WipeObservation {
            len_before,
            zeroed_to_capacity: true,
        }))
    );
}

#[track_caller]
fn assert_sizing_refusal<T: Encode + MessageType>(value: &T, expected: Error) {
    let (result, phases, _) = trace(value, |observed| super::encode(observed));
    assert_eq!(result, Err(expected));
    assert_eq!(phases, sizing_phases(0));
    assert_caller_sizing_refusal(value, expected, &mut Vec::new());
    assert_caller_sizing_refusal(value, expected, &mut vec![0xaa; 32]);
}

#[track_caller]
fn assert_layout_refusal<T: Encode + MessageType>(value: &T, expected: Error) {
    assert_eq!(super::encoded_len(value), Err(expected));
    assert_sizing_refusal(value, expected);
}

#[test]
fn declared_bound_precedes_own_byte_cap() {
    const TITLE: Cap = Cap {
        name: "test.note.title",
        bound: Some(32),
        bytes: 32,
    };

    // The first field alone decides the refusal. The P4 Note exercises
    // the same error with its complete layout.
    struct Title;
    impl Classified for Title {
        const SECRET: bool = false;
    }

    impl MessageType for Title {
        const MESSAGE: Message = Message {
            name: "test.note",
            cap: 128,
            budget: 128,
        };
    }

    impl Encode for Title {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.opaque(&TITLE, 33)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_opaque(&TITLE, &[b'x'; 33])
        }
    }

    let error = Error::new(ErrorKind::LengthOverBound)
        .named("test.note.title")
        .limit(32);
    assert_layout_refusal(&Title, error);
}

#[test]
fn shaded_default_refuses_in_sizing_for_all_buffer_capacities() {
    let value = shaded::Shaded::Default(shaded::Shade::Pale, 0x0102_0304_0506_0708);
    let error = Error::default_arm_mismatch("test.shaded.tone");
    assert_layout_refusal(&value, error);
    let mut short = vec![0xaa; 4];
    assert!((1..12).contains(&short.capacity()));
    assert_caller_sizing_refusal(&value, error, &mut short);
}

#[test]
fn default_arm_check_precedes_an_enclosing_array_charge() {
    const ITEMS: Cap = Cap {
        name: "test.sizing.shades.items",
        bound: None,
        bytes: 3,
    };

    struct Shades([shaded::Shaded; 1]);
    sizing_message!(Shades, "shades", 16);

    impl Encode for Shades {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.array(&ITEMS, &self.0)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_array(&ITEMS, &self.0)
        }
    }

    let value = Shades([shaded::Shaded::Default(
        shaded::Shade::Pale,
        0x0102_0304_0506_0708,
    )]);
    assert_layout_refusal(&value, Error::default_arm_mismatch("test.shaded.tone"));
}

#[test]
fn nested_cap_tie_prefers_the_innermost_cap() {
    const OUTER: Cap = Cap {
        name: "test.sizing.nested.outer",
        bound: None,
        bytes: 4,
    };
    const INNER: Cap = Cap {
        name: "test.sizing.nested.outer.inner",
        bound: None,
        bytes: 4,
    };

    struct Inner<'a>(&'a [u64]);
    impl Classified for Inner<'_> {
        const SECRET: bool = false;
    }

    impl Encode for Inner<'_> {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.array(&INNER, self.0)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_array(&INNER, self.0)
        }
    }

    struct Nested<'a>(&'a [Inner<'a>]);
    sizing_message!(Nested<'_>, "nested", 16);

    impl Encode for Nested<'_> {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            let depth = sizer.depth.get();
            let result = sizer.array(&OUTER, self.0);
            assert_eq!(sizer.depth.get(), depth);
            result
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_array(&OUTER, self.0)
        }
    }

    let error = Error::new(ErrorKind::CapExceeded)
        .named("test.sizing.nested.outer.inner")
        .limit(4);
    assert_layout_refusal(&Nested(&[Inner(&[1])]), error);
    let empty = Nested(&[Inner(&[])]);
    let bytes = [0, 0, 0, 1, 0, 0, 0, 0];
    assert_eq!(super::encoded_len(&empty), Ok(8));
    assert_eq!(super::encode(&empty).unwrap(), bytes);
    let mut out = Vec::new();
    super::encode_into(&empty, &mut out).unwrap();
    assert_eq!(out, bytes);

    // The first inner count consumes the outer allowance even after its
    // scope closes. The second inner count fails before opening its cap.
    let outer_error = Error::new(ErrorKind::CapExceeded)
        .named("test.sizing.nested.outer")
        .limit(4);
    assert_layout_refusal(&Nested(&[Inner(&[]), Inner(&[])]), outer_error);
}

#[test]
fn message_cap_does_not_mask_a_later_bound() {
    const PREFIX: Cap = Cap {
        name: "test.sizing.bound.prefix",
        bound: None,
        bytes: 20,
    };
    const TITLE: Cap = Cap {
        name: "test.sizing.bound.title",
        bound: Some(4),
        bytes: 4,
    };
    const ERROR: Error = Error::new(ErrorKind::LengthOverBound)
        .named("test.sizing.bound.title")
        .limit(4);

    struct Bound {
        title: &'static [u8],
        ignore_error: bool,
    }
    sizing_message!(Bound, "bound", 16);

    impl Encode for Bound {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.array(&PREFIX, &[0_u32; 5])?;
            let title = sizer.opaque(&TITLE, self.title.len());
            if self.ignore_error {
                assert_eq!(title, Err(ERROR));
                assert_eq!(sizer.put(&Unvisited), Err(ERROR));
                Ok(())
            } else {
                title
            }
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_array(&PREFIX, &[0_u32; 5])?;
            encoder.put_opaque(&TITLE, self.title)
        }
    }

    for ignore_error in [false, true] {
        assert_layout_refusal(
            &Bound {
                title: b"12345",
                ignore_error,
            },
            ERROR,
        );
    }
    let empty = Bound {
        title: b"",
        ignore_error: false,
    };
    assert_eq!(super::encoded_len(&empty), Ok(28));
    assert_sizing_refusal(&empty, super::too_large(&Bound::MESSAGE));
}

/// A component whose visit must be skipped once an error is latched.
struct Unvisited;
impl Classified for Unvisited {
    const SECRET: bool = false;
}

impl Encode for Unvisited {
    fn size(&self, _: &Sizer) -> Result<(), Error> {
        panic!("a refused context entered another size visit")
    }

    fn encode(&self, _: &mut Encoder<'_, Self>) -> Result<(), Error> {
        panic!("a sizing refusal entered the write phase")
    }
}

#[test]
fn length_word_width_is_checked_after_crossing_the_message_cap() {
    const PREFIX: Cap = Cap {
        name: "test.sizing.repr.prefix",
        bound: None,
        bytes: 20,
    };
    const DATA: Cap = Cap {
        name: "test.sizing.repr.data",
        bound: None,
        bytes: 4,
    };

    struct Repr(usize);
    sizing_message!(Repr, "repr", 16);

    impl Encode for Repr {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.array(&PREFIX, &[0_u32; 5])?;
            sizer.opaque(&DATA, self.0)
        }

        fn encode(&self, _: &mut Encoder<'_, Self>) -> Result<(), Error> {
            panic!("the synthetic field has no backing payload")
        }
    }

    let Ok(length) = usize::try_from(u64::from(u32::MAX) + 1) else {
        return;
    };
    let error = Error::new(ErrorKind::LengthNotRepresentable).named("test.sizing.repr.data");
    assert_layout_refusal(&Repr(length), error);
}

/// A handwritten unsigned integer whose size and write result are injected.
struct WordReport {
    bytes: u64,
    write_result: Result<(), Error>,
}
sizing_message!(WordReport, "write", 16);

impl Encode for WordReport {
    fn size(&self, sizer: &Sizer) -> Result<(), Error> {
        sizer.charge(Ok(self.bytes))
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        encoder.put_u32(1)?;
        self.write_result
    }
}

#[test]
fn overflow_does_not_mask_a_later_array_cap() {
    const ITEMS: Cap = Cap {
        name: "test.sizing.overflow.items",
        bound: None,
        bytes: 4,
    };

    struct Overflow<'a>(&'a [u64]);
    sizing_message!(Overflow<'_>, "overflow", 16);

    impl Encode for Overflow<'_> {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.put(&WordReport {
                bytes: HUGE,
                write_result: Ok(()),
            })?;
            sizer.put(&0_u32)?;
            sizer.array(&ITEMS, self.0)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_u32(1)?;
            encoder.put_u32(0)?;
            encoder.put_array(&ITEMS, self.0)
        }
    }

    let error = Error::new(ErrorKind::CapExceeded)
        .named("test.sizing.overflow.items")
        .limit(4);
    assert_layout_refusal(&Overflow(&[1]), error);
    let empty = Overflow(&[]);
    assert_eq!(super::encoded_len(&empty), Err(INVALID_VISIT));
    assert_sizing_refusal(&empty, super::too_large(&Overflow::MESSAGE));
}

#[test]
fn leaf_report_refusal_precedes_the_array_and_later_bound() {
    const ITEMS: Cap = Cap {
        name: "test.sizing.opaque.items",
        bound: None,
        bytes: 4,
    };
    const TITLE: Cap = Cap {
        name: "test.sizing.opaque.title",
        bound: Some(4),
        bytes: 4,
    };

    struct HyperReport(Result<u64, Error>);
    impl Classified for HyperReport {
        const SECRET: bool = false;
    }

    impl Encode for HyperReport {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.charge(self.0)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_u64(1)
        }
    }

    struct Opaque(HyperReport);
    sizing_message!(Opaque, "opaque", 16);

    impl Encode for Opaque {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.array(&ITEMS, core::slice::from_ref(&self.0))?;
            sizer.opaque(&TITLE, 5)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_array(&ITEMS, core::slice::from_ref(&self.0))?;
            encoder.put_opaque(&TITLE, b"12345")
        }
    }

    let leaf_error = Opaque(HyperReport(Err(SOURCE)));
    assert_eq!(super::encoded_len(&leaf_error), Err(SOURCE));
    assert_sizing_refusal(&leaf_error, super::too_large(&Opaque::MESSAGE));
    let cap_error = Error::new(ErrorKind::CapExceeded)
        .named("test.sizing.opaque.items")
        .limit(4);
    assert_layout_refusal(&Opaque(HyperReport(Ok(8))), cap_error);
}

#[track_caller]
fn assert_sizing_reserve_write(
    phases: [Option<Observation>; 3],
    initial_capacity: usize,
    written_capacity: usize,
) {
    let [sizing, reserve, write] = phases.map(Option::unwrap);
    assert_eq!(
        (sizing.phase, sizing.len, sizing.capacity),
        (Phase::Sizing, 0, initial_capacity)
    );
    assert_eq!(
        (reserve.phase, reserve.len, reserve.capacity),
        (Phase::Reserve, 0, initial_capacity)
    );
    assert_eq!(
        (write.phase, write.len, write.capacity),
        (Phase::Write, 0, written_capacity)
    );
}

#[track_caller]
fn assert_word_report<T: Encode + MessageType>(value: &T, bytes: u64) {
    let length = if bytes == 1 {
        Err(INVALID_VISIT)
    } else {
        Ok(bytes)
    };
    assert_eq!(super::encoded_len(value), length);
    if bytes == 4 {
        let (result, phases, _) = trace(value, |observed| super::encode(observed));
        assert_eq!(result, Err(SOURCE));
        let capacity = phases[2].unwrap().capacity;
        assert!(capacity >= 4);
        assert_sizing_reserve_write(phases, 0, capacity);

        let mut out = Vec::new();
        let (result, phases, wipes) = trace(value, |value| super::encode_into(value, &mut out));
        assert_eq!(result, Err(SOURCE));
        assert!(out.capacity() >= 4);
        assert_sizing_reserve_write(phases, 0, out.capacity());
        assert_eq!(out.as_slice(), []);
        assert_wipes(wipes, [0, 4]);
    } else {
        assert_sizing_refusal(value, super::too_large(&T::MESSAGE));
    }
}

#[test]
fn handwritten_leaf_and_wrapper_share_sizing_and_write_error_rules() {
    struct Wrapper(WordReport);
    sizing_message!(Wrapper, "write", 16);

    impl Encode for Wrapper {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.put(&self.0)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put(&self.0)
        }
    }

    for bytes in [4, 1, HUGE] {
        let wrapper = Wrapper(WordReport {
            bytes,
            write_result: Err(SOURCE),
        });
        assert_word_report(&wrapper.0, bytes);
        assert_word_report(&wrapper, bytes);
    }
}

#[test]
fn full_cap_stack_refuses_and_restores_each_scope() {
    const ITEMS: Cap = Cap {
        name: "test.sizing.stack.injected",
        bound: None,
        bytes: 128,
    };

    struct Stack(u32);
    sizing_message!(Stack, "stack", 128);

    impl Encode for Stack {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            if self.0 == 0 {
                return sizer.put(&0_u32);
            }
            let entry = sizer.depth.get();
            let result = sizer.array(&ITEMS, &[Self(self.0 - 1)]);
            assert_eq!(sizer.depth.get(), entry);
            if entry == super::DEPTH_MAX as usize {
                assert_eq!(result, Err(INVALID_VISIT));
            }
            result
        }

        fn encode(&self, _: &mut Encoder<'_, Self>) -> Result<(), Error> {
            panic!("the stack fixture exercises sizing")
        }
    }

    assert_eq!(super::encoded_len(&Stack(super::DEPTH_MAX)), Ok(68));
    let full = Stack(super::DEPTH_MAX + 1);
    assert_eq!(super::encoded_len(&full), Err(INVALID_VISIT));
    assert_sizing_refusal(&full, super::too_large(&Stack::MESSAGE));
}

#[test]
fn caught_sizing_unwind_restores_depth_and_latches_refusal() {
    use ironafs_test_support::Property;

    const PANIC: &str = "the sizing visit was interrupted";

    const ITEMS: Cap = Cap {
        name: "test.sizing.interrupted.items",
        bound: None,
        bytes: 8,
    };

    struct Pair([u32; 2]);
    impl Classified for Pair {
        const SECRET: bool = false;
    }

    impl Encode for Pair {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.put(&self.0[0])?;
            panic!("{PANIC}")
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put(&self.0[0])?;
            encoder.put(&self.0[1])
        }
    }

    struct Interrupted;
    sizing_message!(Interrupted, "interrupted", 16);

    impl Encode for Interrupted {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            let depth = sizer.depth.get();
            // Replaying after the unwind would reuse a refused context.
            let caught = Property::new("a sizing array visit unwinds")
                .cases(1)
                .shrink_budget(0)
                .run(
                    |_| [Pair([1, 2])],
                    |items| {
                        let _ = sizer.array(&ITEMS, items);
                    },
                )
                .unwrap_err();
            assert_eq!(caught.message(), PANIC);
            assert_eq!(sizer.depth.get(), depth);
            assert_eq!(sizer.put(&Unvisited), Err(INVALID_VISIT));
            Ok(())
        }

        fn encode(&self, _: &mut Encoder<'_, Self>) -> Result<(), Error> {
            panic!("an interrupted visit must refuse")
        }
    }

    assert_eq!(super::encoded_len(&Interrupted), Err(INVALID_VISIT));
    assert_sizing_refusal(&Interrupted, super::too_large(&Interrupted::MESSAGE));
}

#[test]
fn arrays_require_each_element_to_charge_positive_whole_units() {
    const ITEMS: Cap = Cap {
        name: "test.sizing.width.items",
        bound: Some(1),
        bytes: 4,
    };
    const CAP_ERROR: Error = Error::new(ErrorKind::CapExceeded)
        .named("test.sizing.width.items")
        .limit(4);

    enum Element {
        Leaf(WordReport),
        EmptyStructure,
        IgnoreInnerRefusal(Result<(), Error>),
    }
    impl Classified for Element {
        const SECRET: bool = false;
    }

    impl Encode for Element {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            match self {
                Self::Leaf(leaf) => sizer.put(leaf),
                Self::EmptyStructure => Ok(()),
                Self::IgnoreInnerRefusal(result) => {
                    let leaf = WordReport {
                        bytes: 8,
                        write_result: Ok(()),
                    };
                    assert_eq!(sizer.put(&leaf), Err(CAP_ERROR));
                    *result
                }
            }
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_u32(1)
        }
    }

    struct Width([Element; 1]);
    sizing_message!(Width, "width", 16);

    impl Encode for Width {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.array(&ITEMS, &self.0)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_array(&ITEMS, &self.0)
        }
    }

    struct FixedWidth([Element; 1]);
    sizing_message!(FixedWidth, "fixed_width", 16);

    impl Encode for FixedWidth {
        fn size(&self, sizer: &Sizer) -> Result<(), Error> {
            sizer.fixed_array(&self.0)
        }

        fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
            encoder.put_fixed_array(&self.0)
        }
    }

    for element in [
        Element::Leaf(WordReport {
            bytes: 0,
            write_result: Ok(()),
        }),
        Element::Leaf(WordReport {
            bytes: 1,
            write_result: Ok(()),
        }),
        Element::EmptyStructure,
    ] {
        let variable = Width([element]);
        assert_eq!(super::encoded_len(&variable), Err(INVALID_VISIT));
        assert_sizing_refusal(&variable, super::too_large(&Width::MESSAGE));
        let fixed = FixedWidth(variable.0);
        assert_eq!(super::encoded_len(&fixed), Err(INVALID_VISIT));
        assert_sizing_refusal(&fixed, super::too_large(&FixedWidth::MESSAGE));
    }

    for result in [Ok(()), Err(SOURCE)] {
        assert_layout_refusal(&Width([Element::IgnoreInnerRefusal(result)]), CAP_ERROR);
    }

    let variable = Width([Element::Leaf(WordReport {
        bytes: 4,
        write_result: Ok(()),
    })]);
    assert_eq!(super::encoded_len(&variable), Ok(8));
    let bytes = super::encode(&variable).unwrap();
    assert_eq!(bytes, [0, 0, 0, 1, 0, 0, 0, 1]);
    let mut out = Vec::new();
    super::encode_into(&variable, &mut out).unwrap();
    assert_eq!(out, bytes);

    let fixed = FixedWidth(variable.0);
    assert_eq!(super::encoded_len(&fixed), Ok(4));
    let bytes = super::encode(&fixed).unwrap();
    assert_eq!(bytes, [0, 0, 0, 1]);
    super::encode_into(&fixed, &mut out).unwrap();
    assert_eq!(out, bytes);
    assert_eq!(super::encoded_len(&()), Ok(0));
}
