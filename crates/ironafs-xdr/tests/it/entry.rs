// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The decode entry points take the cap, the memory budget and the secrecy
//! from the decoded type (`docs/specs/xdr.md`, 5.1, point 8, and 5.2,
//! point 9): no caller supplies a descriptor.

use ironafs_xdr::{
    Cap, Classified, Decode, Decoder, Error, ErrorKind, Message, MessageType, array_heap_ratio,
    array_reservations, decode, decode_traced, proven,
};

const WORDS: Cap = Cap {
    name: "test.tight.words",
    bound: None,
    bytes: 16,
};

/// A list of words whose flag is `SECRET`, under a cap of 12 bytes and a
/// budget of 8: room for the count word and two words.
#[derive(Debug, PartialEq)]
struct Tight<const SECRET: bool>(Vec<u32>);

impl<const SECRET: bool> Classified for Tight<SECRET> {
    const SECRET: bool = SECRET;
}

impl<const SECRET: bool> MessageType for Tight<SECRET> {
    const MESSAGE: Message = Message {
        name: "test.tight",
        cap: 12,
        budget: 8,
    };
}

impl<'a, const SECRET: bool> Decode<'a> for Tight<SECRET> {
    const S_MIN: u64 = 4;
    const DEPTH: u32 = 1;
    const HEAP_RATIO: Option<u64> = array_heap_ratio::<u32>();
    const RESERVATIONS: Option<u64> = array_reservations::<u32>();

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.array(&WORDS).map(Self)
    }
}

type Plain = Tight<false>;
type Sealed = Tight<true>;

// Its worst case is one byte requested per byte of input: 12 over a budget
// of 8, so the type is not proven against its own row.
const _: () = assert!(!proven::<Plain>());

const TWO: [u8; 12] = [0, 0, 0, 2, 0, 0, 0, 7, 0, 0, 0, 8];
const THREE: [u8; 16] = [0, 0, 0, 3, 0, 0, 0, 7, 0, 0, 0, 8, 0, 0, 0, 9];

#[test]
fn decode_takes_the_cap_from_the_type() {
    assert_eq!(decode::<Plain>(&TWO), Ok(Tight(vec![7, 8])));
    let error = decode::<Plain>(&THREE).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MessageTooLarge);
    assert_eq!(error.name(), Some("test.tight"));
    assert_eq!(error.limit_value(), Some(12));
    assert_eq!((error.offset(), error.extent()), (Some(0), Some(16)));
}

#[test]
fn decode_takes_the_budget_from_the_type() {
    // Two words are 8 bytes of slots, the type's whole budget; one byte
    // less is refused before the request is made.
    let mut seen = Vec::new();
    let result = decode_traced::<Plain>(&TWO, 7, &mut |reservation| seen.push(reservation));
    let error = result.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::BudgetExceeded);
    assert_eq!(error.name(), Some("test.tight"));
    assert_eq!(seen, []);
}

#[test]
fn decode_traced_never_raises_the_budget() {
    /// As `Tight`, with a budget one byte short of two words.
    #[derive(Debug)]
    struct Short;

    impl Classified for Short {
        const SECRET: bool = false;
    }

    impl MessageType for Short {
        const MESSAGE: Message = Message {
            name: "test.short",
            cap: 12,
            budget: 7,
        };
    }

    impl<'a> Decode<'a> for Short {
        const S_MIN: u64 = 4;
        const DEPTH: u32 = 1;
        const HEAP_RATIO: Option<u64> = array_heap_ratio::<u32>();
        const RESERVATIONS: Option<u64> = array_reservations::<u32>();

        fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
            decoder.array::<u32>(&WORDS).map(|_| Self)
        }
    }

    for budget in [u64::MAX, 8, 7, 0] {
        let error = decode_traced::<Short>(&TWO, budget, &mut |_| {}).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::BudgetExceeded, "budget {budget}");
    }
    assert_eq!(
        decode::<Short>(&TWO).unwrap_err().kind(),
        ErrorKind::BudgetExceeded
    );
}

#[test]
fn decode_takes_the_secrecy_from_the_type() {
    // The same bytes and the same decoding, apart from the type's flag.
    let cut = &TWO[..10];
    let plain = decode::<Plain>(cut).unwrap_err();
    assert_eq!(plain.kind(), ErrorKind::Truncated);
    assert!(plain.offset().is_some());
    let sealed = decode::<Sealed>(cut).unwrap_err();
    assert_eq!(sealed.kind(), plain.kind());
    assert_eq!((sealed.offset(), sealed.extent()), (None, None));
    // The entry point's own errors too.
    let sealed = decode::<Sealed>(&THREE).unwrap_err();
    assert_eq!(sealed.kind(), ErrorKind::MessageTooLarge);
    assert_eq!((sealed.offset(), sealed.extent()), (None, None));
    let mut trailing = TWO[..8].to_vec();
    trailing[3] = 0;
    let sealed = decode::<Sealed>(&trailing).unwrap_err();
    assert_eq!(sealed.kind(), ErrorKind::TrailingBytes);
    assert_eq!((sealed.offset(), sealed.extent()), (None, None));
    let traced = decode_traced::<Sealed>(cut, u64::MAX, &mut |_| {}).unwrap_err();
    assert_eq!((traced.offset(), traced.extent()), (None, None));
}

/// A handwritten secret-bearing message whose decoder decodes a plain
/// message from its own bytes on the side, and passes that decode's error
/// on as its own: an error that already carries a position.
#[derive(Debug)]
struct Forwarding;

impl Classified for Forwarding {
    const SECRET: bool = true;
}

impl MessageType for Forwarding {
    const MESSAGE: Message = Message {
        name: "test.forwarding",
        cap: 12,
        budget: 8,
    };
}

impl<'a> Decode<'a> for Forwarding {
    const S_MIN: u64 = 12;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: Option<u64> = Some(0);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        let inner = decoder.fixed_opaque::<12>()?;
        decode::<Plain>(&inner[..10]).map(|_| Self)
    }
}

#[test]
fn decode_strips_the_position_from_an_error_a_secret_decoder_passes_on() {
    let forwarded = decode::<Plain>(&TWO[..10]).unwrap_err();
    assert!(
        forwarded.offset().is_some(),
        "the inner error is positioned"
    );
    for error in [
        decode::<Forwarding>(&TWO).unwrap_err(),
        decode_traced::<Forwarding>(&TWO, u64::MAX, &mut |_| {}).unwrap_err(),
    ] {
        assert_eq!(error.kind(), ErrorKind::Truncated);
        assert_eq!((error.offset(), error.extent()), (None, None));
    }
}
