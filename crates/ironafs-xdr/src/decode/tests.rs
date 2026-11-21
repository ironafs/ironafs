// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_macros,
    reason = "the tests build their inputs and records outside the ledger"
)]

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use super::{Decoder, Ledger, Reservation, run};
use crate::{Cap, Error, ErrorKind, Message};

const MESSAGE: Message = Message {
    name: "test.m",
    cap: 128,
    budget: 1024,
    secret: false,
};

const SECRET: Message = Message {
    secret: true,
    ..MESSAGE
};

/// A string with a declared bound of 32 bytes and a cap of 16.
const NAME: Cap = Cap {
    name: "test.m.name",
    bound: Some(32),
    bytes: 16,
};

/// A string whose cap is its bound.
const TITLE: Cap = Cap {
    name: "test.m.title",
    bound: Some(8),
    bytes: 8,
};

/// An array of strings, 16 bytes of elements.
const NAMES: Cap = Cap {
    name: "test.m.names",
    bound: None,
    bytes: 16,
};

/// An array of at most 3 strings.
const FEW: Cap = Cap {
    name: "test.m.few",
    bound: Some(3),
    bytes: 64,
};

/// A string label, as an array element.
#[derive(Debug, PartialEq, Eq)]
struct Label(Vec<u8>);

const LABEL: Cap = Cap {
    name: "test.m.label",
    bound: Some(8),
    bytes: 8,
};

impl<'a> super::Decode<'a> for Label {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(1);
    const RESERVATIONS: Option<u64> = Some(1);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.opaque_owned(&LABEL).map(Label)
    }
}

/// Decodes `input` as one message with a decoding closure, recording
/// every reservation: `(result, reservations)`. A macro, so that the
/// closure's lifetimes are inferred at each call.
macro_rules! decode_with {
    ($message:expr, $input:expr, $decode_value:expr) => {{
        let message: &Message = $message;
        let mut seen: Vec<Reservation> = Vec::new();
        let result = {
            let mut hook = |reservation| seen.push(reservation);
            let ledger = Ledger::new(message.name, message.budget, Some(&mut hook));
            run(message, $input, ledger, $decode_value)
        };
        (result, seen)
    }};
}

#[track_caller]
fn assert_error(error: &Error, kind: ErrorKind, name: Option<&str>, offset: Option<usize>) {
    assert_eq!(error.kind(), kind, "{error}");
    assert_eq!(error.name(), name, "{error}");
    assert_eq!(error.offset(), offset, "{error}");
}

#[test]
fn read_numbers_are_most_significant_byte_first() {
    let input = [
        0xff, 0xff, 0xff, 0xfe, 0x00, 0x00, 0x00, 0x07, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x80, 0, 0, 0, 0, 0, 0, 0,
    ];
    let (result, _) = decode_with!(&MESSAGE, &input, |decoder| {
        Ok((
            decoder.read_i32()?,
            decoder.read_u32()?,
            decoder.read_u64()?,
            decoder.read_i64()?,
        ))
    });
    assert_eq!(result.unwrap(), (-2, 7, 0x0102_0304_0506_0708, i64::MIN));
}

#[test]
fn decode_rejects_empty_input_for_int_as_truncated() {
    let (result, _) = decode_with!(&MESSAGE, &[], Decoder::read_i32);
    let error = result.unwrap_err();
    assert_error(&error, ErrorKind::Truncated, None, Some(0));
    assert_eq!(error.extent(), Some(0));
}

#[test]
fn decode_rejects_input_over_message_cap_before_reading() {
    let input = vec![0; 129];
    let mut read = false;
    let (result, _) = decode_with!(&MESSAGE, &input, |_| {
        read = true;
        Ok(())
    });
    let error = result.unwrap_err();
    assert_error(&error, ErrorKind::MessageTooLarge, Some("test.m"), Some(0));
    assert_eq!(error.limit_value(), Some(128));
    assert_eq!(error.extent(), Some(129));
    assert!(!read);
}

#[test]
fn decode_accepts_input_of_exactly_message_cap() {
    let input = vec![0; 128];
    let (result, _) = decode_with!(&MESSAGE, &input, |decoder| {
        decoder.fixed_opaque::<128>().map(|data| data.len())
    });
    assert_eq!(result.unwrap(), 128);
}

#[test]
fn decode_rejects_trailing_word_and_trailing_byte() {
    let (result, _) = decode_with!(&MESSAGE, &[0, 0, 0, 1, 0, 0, 0, 0], Decoder::read_u32);
    let error = result.unwrap_err();
    assert_error(&error, ErrorKind::TrailingBytes, None, Some(4));
    assert_eq!(error.extent(), Some(4));
    let (result, _) = decode_with!(&MESSAGE, &[0, 0, 0, 1, 0], Decoder::read_u32);
    let error = result.unwrap_err();
    assert_error(&error, ErrorKind::TrailingBytes, None, Some(4));
    assert_eq!(error.extent(), Some(1));
}

#[test]
fn read_bool_accepts_only_zero_and_one() {
    let (result, _) = decode_with!(&MESSAGE, &[0, 0, 0, 0, 0, 0, 0, 1], |decoder| {
        Ok((
            decoder.read_bool("test.m.b")?,
            decoder.read_bool("test.m.b")?,
        ))
    });
    assert_eq!(result.unwrap(), (false, true));
    for word in [[0, 0, 0, 2], [0xff, 0xff, 0xff, 0xff], [1, 0, 0, 0]] {
        let input = [[0, 0, 0, 0], word].concat();
        let (result, _) = decode_with!(&MESSAGE, &input, |decoder| {
            decoder.read_u32()?;
            decoder.read_bool("test.m.b")
        });
        assert_error(
            &result.unwrap_err(),
            ErrorKind::InvalidBool,
            Some("test.m.b"),
            Some(4),
        );
    }
}

#[test]
fn read_enum_rejects_an_undeclared_value_at_its_word() {
    let declared = |value| (value == 4).then_some("deep");
    let (result, _) = decode_with!(&MESSAGE, &[0, 0, 0, 4], |decoder| {
        decoder.read_enum("test.m.e", declared)
    });
    assert_eq!(result.unwrap(), "deep");
    let (result, _) = decode_with!(&MESSAGE, &[0, 0, 0, 2], |decoder| {
        decoder.read_enum("test.m.e", declared)
    });
    assert_error(
        &result.unwrap_err(),
        ErrorKind::UnknownEnumValue,
        Some("test.m.e"),
        Some(0),
    );
}

#[test]
fn fixed_opaque_rejects_each_non_zero_padding_byte_at_its_offset() {
    let valid = [0x61, 0x62, 0x63, 0x64, 0x65, 0, 0, 0];
    let (result, _) = decode_with!(&MESSAGE, &valid, |decoder| {
        decoder.fixed_opaque::<5>().map(|data| data.to_vec())
    });
    assert_eq!(result.unwrap(), b"abcde");
    for index in 5..8 {
        let mut input = valid;
        input[index] = 1;
        let (result, _) = decode_with!(&MESSAGE, &input, |decoder| {
            decoder.fixed_opaque::<5>().map(|data| data.len())
        });
        assert_error(
            &result.unwrap_err(),
            ErrorKind::NonZeroPadding,
            None,
            Some(index),
        );
    }
}

#[test]
fn fixed_opaque_short_of_its_padding_is_truncated_at_its_start() {
    let (result, _) = decode_with!(&MESSAGE, &[0, 0, 0, 0, 1, 2, 3, 4, 5], |decoder| {
        decoder.read_u32()?;
        decoder.fixed_opaque::<5>().map(|data| data.len())
    });
    assert_error(&result.unwrap_err(), ErrorKind::Truncated, None, Some(4));
}

#[test]
fn fixed_opaque_inline_copies_its_bytes() {
    let (result, _) = decode_with!(&MESSAGE, &[1, 2, 3, 0], Decoder::fixed_opaque_inline::<3>);
    assert_eq!(result.unwrap(), [1, 2, 3]);
}

#[test]
fn fixed_opaque_owned_checks_its_whole_size_before_reserving() {
    let input = [7; 8];
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| {
        decoder.fixed_opaque_owned::<100>("test.m.big")
    });
    assert_error(&result.unwrap_err(), ErrorKind::Truncated, None, Some(0));
    assert_eq!(seen, []);
    let input = [7; 100];
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| {
        decoder.fixed_opaque_owned::<100>("test.m.big")
    });
    assert_eq!(*result.unwrap(), input);
    assert_eq!(
        seen,
        [Reservation {
            bytes: 100,
            name: "test.m.big"
        }]
    );
}

#[test]
fn opaque_borrows_its_data_from_the_input() {
    let input = [0, 0, 0, 3, b'a', b'b', b'c', 0];
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| {
        decoder.opaque(&NAME).map(<[u8]>::as_ptr)
    });
    assert_eq!(result.unwrap(), input[4..].as_ptr());
    assert_eq!(seen, []);
}

#[test]
fn opaque_checks_bound_before_cap() {
    // 33 is above both the bound (32) and the cap (16).
    let mut input = vec![0, 0, 0, 33];
    input.resize(4 + 36, 0);
    let (result, _) = decode_with!(&MESSAGE, &input, |decoder| {
        decoder.opaque(&NAME).map(<[u8]>::len)
    });
    let error = result.unwrap_err();
    assert_error(
        &error,
        ErrorKind::LengthOverBound,
        Some("test.m.name"),
        Some(0),
    );
    assert_eq!(error.limit_value(), Some(32));
}

#[test]
fn opaque_over_cap_within_bound_is_cap_exceeded() {
    let mut input = vec![0, 0, 0, 17];
    input.resize(4 + 20, 0);
    let (result, _) = decode_with!(&MESSAGE, &input, |decoder| {
        decoder.opaque(&NAME).map(<[u8]>::len)
    });
    let error = result.unwrap_err();
    assert_error(&error, ErrorKind::CapExceeded, Some("test.m.name"), Some(0));
    assert_eq!(error.limit_value(), Some(16));
}

#[test]
fn opaque_owned_with_huge_length_reserves_nothing() {
    let unbounded = &Cap {
        name: "test.m.blob",
        bound: None,
        bytes: 16,
    };
    let (result, seen) = decode_with!(&MESSAGE, &[0xff; 4], |decoder| {
        decoder.opaque_owned(unbounded)
    });
    assert_error(
        &result.unwrap_err(),
        ErrorKind::CapExceeded,
        Some("test.m.blob"),
        Some(0),
    );
    assert_eq!(seen, []);
}

#[test]
fn opaque_owned_short_of_its_data_is_truncated_before_reserving() {
    let (result, seen) = decode_with!(&MESSAGE, &[0, 0, 0, 5, 1, 2], |decoder| {
        decoder.opaque_owned(&TITLE)
    });
    let error = result.unwrap_err();
    assert_error(&error, ErrorKind::Truncated, None, Some(0));
    assert_eq!(error.extent(), Some(6));
    assert_eq!(seen, []);
}

#[test]
fn opaque_owned_copies_exactly_its_length_and_checks_padding() {
    let input = [0, 0, 0, 2, 9, 8, 0, 0];
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| decoder.opaque_owned(&TITLE));
    assert_eq!(result.unwrap(), [9, 8]);
    assert_eq!(
        seen,
        [Reservation {
            bytes: 2,
            name: "test.m.title"
        }]
    );
    let input = [0, 0, 0, 2, 9, 8, 0, 1];
    let (result, _) = decode_with!(&MESSAGE, &input, |decoder| decoder.opaque_owned(&TITLE));
    assert_error(
        &result.unwrap_err(),
        ErrorKind::NonZeroPadding,
        None,
        Some(7),
    );
}

#[test]
fn opaque_owned_over_budget_names_message_at_length_word() {
    let message = Message {
        budget: 1,
        ..MESSAGE
    };
    let (result, seen) = decode_with!(&message, &[0, 0, 0, 2, 9, 8, 0, 0], |decoder| {
        decoder.opaque_owned(&TITLE)
    });
    let error = result.unwrap_err();
    assert_error(&error, ErrorKind::BudgetExceeded, Some("test.m"), Some(0));
    assert_eq!(error.limit_value(), Some(1));
    assert_eq!(seen, []);
}

#[test]
fn array_decodes_its_elements_in_order() {
    let input = [0, 0, 0, 2, 0, 0, 0, 1, b'a', 0, 0, 0, 0, 0, 0, 0];
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| decoder.array::<Label>(&NAMES));
    assert_eq!(result.unwrap(), [Label(vec![b'a']), Label(vec![])]);
    let slot = u64::try_from(core::mem::size_of::<Label>()).unwrap();
    assert_eq!(
        seen,
        [
            Reservation {
                bytes: 2 * slot,
                name: "test.m.names"
            },
            Reservation {
                bytes: 1,
                name: "test.m.label"
            },
            Reservation {
                bytes: 0,
                name: "test.m.label"
            },
        ]
    );
}

#[test]
fn array_count_over_bound_reserves_nothing() {
    let input = [[0, 0, 0, 4], [0; 4], [0; 4], [0; 4], [0; 4]].concat();
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| decoder.array::<Label>(&FEW));
    let error = result.unwrap_err();
    assert_error(
        &error,
        ErrorKind::LengthOverBound,
        Some("test.m.few"),
        Some(0),
    );
    assert_eq!(error.limit_value(), Some(3));
    assert_eq!(seen, []);
}

#[test]
fn array_count_whose_minimum_passes_cap_reserves_nothing() {
    let input = [0x40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| decoder.array::<u32>(&NAMES));
    assert_error(
        &result.unwrap_err(),
        ErrorKind::CapExceeded,
        Some("test.m.names"),
        Some(0),
    );
    assert_eq!(seen, []);
}

#[test]
fn array_count_not_backed_by_input_reserves_nothing() {
    let input = [0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0];
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| decoder.array::<u32>(&NAMES));
    assert_error(&result.unwrap_err(), ErrorKind::Truncated, None, Some(0));
    assert_eq!(seen, []);
}

#[test]
fn array_element_ending_past_the_limit_is_cap_exceeded_naming_the_array() {
    // Limit at offset 20; the second label's length word at 12 is within
    // its own cap, but its 8 data bytes would end at 24.
    let input = [
        &[0, 0, 0, 2][..],
        &[0, 0, 0, 4],
        b"abcd",
        &[0, 0, 0, 8],
        b"efghijkl",
    ]
    .concat();
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| decoder.array::<Label>(&NAMES));
    let error = result.unwrap_err();
    assert_error(
        &error,
        ErrorKind::CapExceeded,
        Some("test.m.names"),
        Some(12),
    );
    assert_eq!(error.limit_value(), Some(16));
    let slot = u64::try_from(core::mem::size_of::<Label>()).unwrap();
    assert_eq!(
        seen,
        [
            Reservation {
                bytes: 2 * slot,
                name: "test.m.names"
            },
            Reservation {
                bytes: 4,
                name: "test.m.label"
            },
        ]
    );
}

#[test]
fn array_element_word_at_the_limit_is_cap_exceeded_though_input_holds_it() {
    let input = [
        &[0, 0, 0, 3][..],
        &[0, 0, 0, 4],
        b"abcd",
        &[0, 0, 0, 4],
        b"abcd",
        &[0, 0, 0, 0],
    ]
    .concat();
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| decoder.array::<Label>(&NAMES));
    assert_error(
        &result.unwrap_err(),
        ErrorKind::CapExceeded,
        Some("test.m.names"),
        Some(20),
    );
    assert_eq!(seen.len(), 3);
}

/// A string of up to 12 bytes, borrowed.
#[derive(Debug, Default)]
struct Long;

const LONG: Cap = Cap {
    name: "test.m.long",
    bound: None,
    bytes: 12,
};

const WIDE: Cap = Cap {
    name: "test.m.wide",
    bound: None,
    bytes: 16,
};

impl<'a> super::Decode<'a> for Long {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(0);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.opaque(&LONG).map(|_| Long)
    }
}

#[test]
fn array_limit_ends_with_the_array() {
    // The array's one element reaches its limit at 20; the word after it
    // is outside the array and reads on.
    let input = [
        &[0, 0, 0, 1][..],
        &[0, 0, 0, 12],
        b"abcdefghijkl",
        &[0, 0, 0, 9],
    ]
    .concat();
    let (result, _) = decode_with!(&MESSAGE, &input, |decoder| {
        let elements = decoder.array::<Long>(&WIDE)?;
        Ok((elements.len(), decoder.read_u32()?))
    });
    assert_eq!(result.unwrap(), (1, 9));
}

#[test]
fn nested_array_keeps_the_tighter_enclosing_limit() {
    // An outer array of 8 bytes, limit 12, holds inner arrays whose own
    // cap is 16. An inner count of 2 needs 8 bytes from offset 8, past the
    // outer limit; after one inner array of one element, the second inner
    // count word at 12 passes it.
    const OUTER: Cap = Cap {
        name: "test.m.outer",
        bound: None,
        bytes: 8,
    };
    #[derive(Debug, Default)]
    struct Inner;
    impl<'a> super::Decode<'a> for Inner {
        const S_MIN: u64 = 4;
        const DEPTH: u32 = 1;
        const HEAP_RATIO: Option<u64> = Some(0);
        const RESERVATIONS: Option<u64> = Some(1);

        fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
            decoder.array::<u32>(&WIDE).map(|_| Inner)
        }
    }
    for (input, offset) in [
        (
            [[0, 0, 0, 1], [0, 0, 0, 2], [0, 0, 0, 1], [0, 0, 0, 2]].concat(),
            4,
        ),
        (
            [[0, 0, 0, 2], [0, 0, 0, 1], [0, 0, 0, 7], [0, 0, 0, 1]].concat(),
            12,
        ),
    ] {
        let (result, _) = decode_with!(&MESSAGE, &input, |decoder| {
            decoder.array::<Inner>(&OUTER).map(|v| v.len())
        });
        assert_error(
            &result.unwrap_err(),
            ErrorKind::CapExceeded,
            Some("test.m.outer"),
            Some(offset),
        );
    }
}

#[test]
fn fixed_array_checks_its_minimum_on_entry() {
    let (result, seen) = decode_with!(&MESSAGE, &[0, 0, 0, 0], |decoder| decoder
        .fixed_array::<Label, 2>("test.m.array"));
    assert_error(&result.unwrap_err(), ErrorKind::Truncated, None, Some(0));
    assert_eq!(seen, []);
}

#[test]
fn fixed_array_decodes_each_element() {
    let input = [&[0, 0, 0, 1][..], b"a\0\0\0", &[0, 0, 0, 0]].concat();
    let (result, _) = decode_with!(&MESSAGE, &input, |decoder| decoder
        .fixed_array::<Label, 2>("test.m.array"));
    assert_eq!(result.unwrap(), [Label(vec![b'a']), Label(vec![])]);
}

#[test]
fn fixed_array_stops_at_its_first_element_error() {
    let input = [&[0, 0, 0, 9][..], &[0; 8]].concat();
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| decoder
        .fixed_array::<Label, 2>("test.m.array"));
    assert_error(
        &result.unwrap_err(),
        ErrorKind::LengthOverBound,
        Some("test.m.label"),
        Some(0),
    );
    // The slots, then no request for the label that failed.
    let names: Vec<&str> = seen.iter().map(|reservation| reservation.name).collect();
    assert_eq!(names, ["test.m.array"]);
}

#[test]
fn secret_message_errors_carry_no_position() {
    let (result, _) = decode_with!(&SECRET, &[0, 0, 0, 9, 0, 0], |decoder| {
        decoder.opaque(&NAME).map(<[u8]>::len)
    });
    let error = result.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Truncated);
    assert_eq!(error.offset(), None);
    assert_eq!(error.extent(), None);
    let (result, _) = decode_with!(&SECRET, &[0; 8], Decoder::read_u32);
    let error = result.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::TrailingBytes);
    assert_eq!((error.offset(), error.extent()), (None, None));
    let input = vec![0; 129];
    let (result, _) = decode_with!(&SECRET, &input, Decoder::read_u32);
    let error = result.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MessageTooLarge);
    assert_eq!((error.offset(), error.extent()), (None, None));
}

#[test]
fn word_borrows_from_the_input() {
    let input = [0, 0, 0, 7, 1, 2, 3, 4, 5, 6, 7, 8];
    let (result, _) = decode_with!(&MESSAGE, &input, |decoder| {
        Ok((decoder.word()?.as_ptr(), decoder.hyper_word()?.as_ptr()))
    });
    let (word, hyper) = result.unwrap();
    assert_eq!(word, input.as_ptr());
    assert_eq!(hyper, input[4..].as_ptr());
}

#[test]
fn unknown_discriminant_is_located_at_its_mark() {
    let (result, _) = decode_with!(&MESSAGE, &[0, 0, 0, 3], |decoder| -> Result<(), Error> {
        let at = decoder.mark();
        decoder.read_u32()?;
        Err(decoder.unknown_discriminant("test.m.u", at))
    });
    assert_error(
        &result.unwrap_err(),
        ErrorKind::UnknownDiscriminant,
        Some("test.m.u"),
        Some(0),
    );
}

/// A type whose heap ratio is the largest a constant can hold.
struct Greedy;

impl<'a> super::Decode<'a> for Greedy {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(u64::MAX);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.read_u32().map(|_| Greedy)
    }
}

#[test]
fn proven_refuses_a_worst_case_that_overflows() {
    let everything = Message {
        name: "test.m",
        cap: 1,
        budget: u64::MAX,
        secret: false,
    };
    // u64::MAX × 1 is exactly the budget: proven.
    assert!(super::proven::<Greedy>(&everything));
    // u64::MAX × 2 overflows: not proven, however large the budget.
    let twice = Message {
        cap: 2,
        ..everything
    };
    assert!(!super::proven::<Greedy>(&twice));
}

/// A label whose `Default` would panic, as one that allocates or fails
/// might: a fixed array must never call it.
#[derive(Debug, PartialEq, Eq)]
struct Loud(Vec<u8>);

impl Default for Loud {
    fn default() -> Self {
        panic!("Default called on a fixed-array element")
    }
}

impl<'a> super::Decode<'a> for Loud {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(1);
    const RESERVATIONS: Option<u64> = Some(1);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.opaque_owned(&LABEL).map(Loud)
    }
}

#[test]
fn fixed_array_after_an_element_error_builds_no_element() {
    let input = [&[0, 0, 0, 9][..], &[0; 8]].concat();
    let (result, _) = decode_with!(&MESSAGE, &input, |decoder| decoder
        .fixed_array::<Loud, 2>("test.m.array"));
    assert_error(
        &result.unwrap_err(),
        ErrorKind::LengthOverBound,
        Some("test.m.label"),
        Some(0),
    );
}

#[test]
fn nested_array_child_element_past_parent_limit_is_parent_cap_exceeded() {
    // The outer array (cap 8, limit 12) holds one inner array of labels
    // whose own cap (16, limit 24) is wider. The inner count of 1 needs 4
    // bytes from 8, within the outer limit; its label's length word at 8
    // says 4 bytes, which would end at 16, past the outer limit.
    const OUTER: Cap = Cap {
        name: "test.m.outer",
        bound: None,
        bytes: 8,
    };
    #[derive(Debug)]
    struct Inner;
    impl<'a> super::Decode<'a> for Inner {
        const S_MIN: u64 = 4;
        const DEPTH: u32 = 1;
        const HEAP_RATIO: Option<u64> = Some(7);
        const RESERVATIONS: Option<u64> = Some(1);

        fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
            decoder.array::<Label>(&WIDE).map(|_| Inner)
        }
    }
    let input = [&[0, 0, 0, 1][..], &[0, 0, 0, 1], &[0, 0, 0, 4], b"abcd"].concat();
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| {
        decoder.array::<Inner>(&OUTER).map(|v| v.len())
    });
    assert_error(
        &result.unwrap_err(),
        ErrorKind::CapExceeded,
        Some("test.m.outer"),
        Some(8),
    );
    let names: Vec<&str> = seen.iter().map(|reservation| reservation.name).collect();
    assert_eq!(names, ["test.m.outer", "test.m.wide"]);
}

/// A four-byte type whose own ratios are the largest a constant holds.
struct GreedyWord(#[expect(dead_code, reason = "only its size matters")] u32);

impl<'a> super::Decode<'a> for GreedyWord {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(u64::MAX);
    const RESERVATIONS: Option<u64> = Some(u64::MAX);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.read_u32().map(GreedyWord)
    }
}

#[test]
fn slot_ratios_are_checked_not_saturated() {
    // A zero-sized slot adds nothing: exactly u64::MAX.
    assert_eq!(super::array_heap_ratio::<Greedy>(), Some(u64::MAX));
    // A four-byte slot over four bytes adds 1: overflow.
    assert_eq!(super::array_heap_ratio::<GreedyWord>(), None);
    assert_eq!(super::out_of_line_heap_ratio::<GreedyWord>(), None);
    assert_eq!(super::out_of_line_reservations::<GreedyWord>(), None);
    assert_eq!(super::array_reservations::<Label>(), Some(1));
    assert_eq!(super::max(Some(3), None), None);
    assert_eq!(super::max(Some(3), Some(7)), Some(7));
    let message = Message {
        name: "test.m",
        cap: 2,
        budget: u64::MAX,
        secret: false,
    };
    assert!(!super::proven::<GreedyWord>(&message));
}

#[test]
fn out_of_line_checks_its_minimum_before_reserving_its_slot() {
    let (result, seen) = decode_with!(&MESSAGE, &[0, 0, 0], |decoder| {
        decoder.out_of_line::<u32>("test.m.held").map(|held| *held)
    });
    assert_error(&result.unwrap_err(), ErrorKind::Truncated, None, Some(0));
    assert_eq!(seen, []);
    let (result, seen) = decode_with!(&MESSAGE, &[0, 0, 0, 7], |decoder| {
        decoder.out_of_line::<u32>("test.m.held").map(|held| *held)
    });
    assert_eq!(result.unwrap(), 7);
    assert_eq!(
        seen,
        [Reservation {
            bytes: 4,
            name: "test.m.held"
        }]
    );
}

#[test]
fn out_of_line_inside_an_array_is_held_to_its_limit() {
    // An array of 4 bytes (limit 8) whose one element reads a word, then
    // holds a u32 out of line from offset 8: its minimum passes the limit.
    const SMALL: Cap = Cap {
        name: "test.m.small",
        bound: None,
        bytes: 4,
    };
    #[derive(Debug)]
    struct Held;
    impl<'a> super::Decode<'a> for Held {
        const S_MIN: u64 = 4;
        const DEPTH: u32 = 1;
        const HEAP_RATIO: Option<u64> = Some(8);
        const RESERVATIONS: Option<u64> = Some(2);

        fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
            decoder.read_u32()?;
            decoder.out_of_line::<u32>("test.m.held").map(|_| Held)
        }
    }
    let input = [[0, 0, 0, 1], [0, 0, 0, 0], [0, 0, 0, 7]].concat();
    let (result, seen) = decode_with!(&MESSAGE, &input, |decoder| {
        decoder.array::<Held>(&SMALL).map(|v| v.len())
    });
    assert_error(
        &result.unwrap_err(),
        ErrorKind::CapExceeded,
        Some("test.m.small"),
        Some(8),
    );
    assert_eq!(seen.len(), 1);
}

#[test]
fn array_whose_cap_is_the_largest_word_has_a_limit_on_every_target() {
    // The limit is the count word's end plus 2³² − 1, held in 64 bits.
    const HUGE: Cap = Cap {
        name: "test.m.huge",
        bound: None,
        bytes: u32::MAX,
    };
    let (result, seen) = decode_with!(&MESSAGE, &[0, 0, 0, 0], |decoder| {
        decoder.array::<u32>(&HUGE).map(|v| v.len())
    });
    assert_eq!(result.unwrap(), 0);
    assert_eq!(
        seen,
        [Reservation {
            bytes: 0,
            name: "test.m.huge"
        }]
    );
}

#[test]
fn boxed_refuses_a_buffer_with_excess_capacity() {
    // Converting it would shrink the block, a request the ledger never saw.
    let mut buffer: Vec<u32> = Vec::with_capacity(8);
    buffer.push(7);
    let input = [0; 4];
    let (result, _) = decode_with!(&MESSAGE, &input, |decoder| {
        let held = decoder.boxed::<u32, 1>(buffer, 0)?;
        decoder.read_u32()?;
        Ok(held)
    });
    assert_error(
        &result.unwrap_err(),
        ErrorKind::AllocationFailed,
        None,
        Some(0),
    );
}

#[test]
fn boxed_keeps_the_reserved_block() {
    let mut buffer: Vec<u32> = Vec::new();
    buffer.try_reserve_exact(2).unwrap();
    buffer.extend([7, 8]);
    let before = buffer.as_ptr();
    let input = [0; 4];
    let (result, _) = decode_with!(&MESSAGE, &input, |decoder| {
        let held = decoder.boxed::<u32, 2>(buffer, 0)?;
        decoder.read_u32()?;
        Ok(held)
    });
    let held = result.unwrap();
    assert_eq!(*held, [7, 8]);
    assert_eq!(held.as_ptr(), before);
}

#[test]
fn boxed_takes_a_zero_sized_type_whatever_its_capacity() {
    let mut buffer: Vec<()> = Vec::new();
    buffer.push(());
    let input = [0; 4];
    let (result, _) = decode_with!(&MESSAGE, &input, |decoder| {
        let held = decoder.boxed::<(), 1>(buffer, 0)?;
        decoder.read_u32()?;
        Ok(held)
    });
    assert_eq!(*result.unwrap(), [()]);
}

/// An element that owns a 100-byte fixed-length opaque out of line, and has
/// no constant empty value.
#[derive(Debug, PartialEq, Eq)]
struct Big(Box<[u8; 100]>);

impl<'a> super::Decode<'a> for Big {
    const S_MIN: u64 = 100;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(1);
    const RESERVATIONS: Option<u64> = Some(1);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.fixed_opaque_owned("test.m.big").map(Big)
    }
}

#[test]
fn fixed_array_of_elements_without_an_empty_value_decodes() {
    let roomy = Message {
        cap: 200,
        ..MESSAGE
    };
    let input = [[1; 100], [2; 100]].concat();
    let (result, seen) = decode_with!(&roomy, &input, |decoder| decoder
        .fixed_array::<Big, 2>("test.m.array"));
    assert_eq!(
        result.unwrap(),
        [Big(Box::new([1; 100])), Big(Box::new([2; 100]))]
    );
    let slot = u64::try_from(core::mem::size_of::<Big>()).unwrap();
    assert_eq!(
        seen,
        [
            Reservation {
                bytes: 2 * slot,
                name: "test.m.array"
            },
            Reservation {
                bytes: 100,
                name: "test.m.big"
            },
            Reservation {
                bytes: 100,
                name: "test.m.big"
            },
        ]
    );
}
