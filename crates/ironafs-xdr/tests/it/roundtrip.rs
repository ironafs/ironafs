// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The round-trip properties of `docs/specs/xdr.md`, 10.2, with the
//! workspace's own seeded generator and shrinker.

use std::fmt::Debug;

use ironafs_test_support::{Property, Source};
use ironafs_xdr::{
    BorrowedBytes, BorrowedWord, Decode, Encode, ErrorKind, Message, MessageType, Reservation,
    decode, encode,
};

use crate::support::heap::{Append, Gather, Owned};
use crate::support::{self, hex, sealed_bytes, size, traced};
use crate::types::{
    BLOB, Blob, DEEP, Deep, ENTRY, EVERY, Entry, Every, FIVE, Five, INTS, Ints, LABEL, Label,
    NAMES, NOTE, Names, Note, PAIR, Pair, SHADE, SHADED, SOLOS, Sealed, Shade, Shaded, Solos, TAGS,
    Tags, WIDE, Wide,
};

/// A type the properties run over.
trait Tested: for<'a> Decode<'a> + Encode + MessageType + PartialEq + Debug {}

impl<T: for<'a> Decode<'a> + Encode + MessageType + PartialEq + Debug> Tested for T {}

/// The heap ratio and reservation ratio of `T`.
fn ratios<T: Tested>() -> (u64, u64) {
    (
        <T as Decode<'static>>::HEAP_RATIO.unwrap(),
        <T as Decode<'static>>::RESERVATIONS.unwrap(),
    )
}

/// How an input is made from a valid encoding.
#[derive(Debug)]
enum Mutation {
    None,
    /// Replace the byte at this index (modulo the length) with this value.
    Byte(usize, u8),
    /// Keep this many bytes.
    Prefix(usize),
    /// Append these bytes.
    Append(Vec<u8>),
    /// Ignore the encoding: these bytes instead.
    Random(Vec<u8>),
}

fn mutation(source: &mut Source) -> Mutation {
    match source.weighted(&[2, 4, 2, 1, 2]) {
        0 => Mutation::None,
        1 => Mutation::Byte(source.usize_in(0..=511), source.u8()),
        2 => Mutation::Prefix(source.usize_in(0..=511)),
        3 => Mutation::Append(source.bytes(1..=8)),
        _ => Mutation::Random(source.bytes(0..=300)),
    }
}

fn mutate(mut bytes: Vec<u8>, mutation: &Mutation) -> Vec<u8> {
    match mutation {
        Mutation::None => {}
        Mutation::Byte(index, value) => {
            if !bytes.is_empty() {
                let index = index % bytes.len();
                bytes[index] = *value;
            }
        }
        Mutation::Prefix(length) => bytes.truncate(*length),
        Mutation::Append(extra) => bytes.append_all(extra),
        Mutation::Random(random) => return random.clone(),
    }
    bytes
}

/// P6's checks on one traced decode of `length` bytes.
#[track_caller]
fn check_ledger<T: Tested>(message: &Message, length: usize, accepted: bool, seen: &[Reservation]) {
    assert_eq!(*message, T::MESSAGE, "the row is the type's own");
    let (heap_ratio, reservations) = ratios::<T>();
    let length = u64::try_from(length).unwrap();
    let total: u64 = seen.iter().map(|reservation| reservation.bytes).sum();
    assert!(total <= message.budget, "requested {total} over budget");
    if accepted {
        assert!(
            total <= heap_ratio * length,
            "requested {total} over r × {length}"
        );
    }
    let count = u64::try_from(seen.len()).unwrap();
    assert!(
        count <= reservations * length / 4,
        "{count} reservations over q × {length} ÷ 4"
    );
}

/// P1 and P3: every generated value encodes to whole units, to exactly
/// the size the profile's table gives (`size`, computed apart from the
/// codec), and decodes back to itself.
fn encodes_and_decodes_back<T: Tested>(
    name: &str,
    message: &'static Message,
    generate: fn(&mut Source) -> T,
    size: fn(&T) -> usize,
) {
    Property::new(format!("{name}: decode(encode(x)) = x")).check(generate, |value| {
        let bytes = encode(value).unwrap();
        assert_eq!(bytes.len() % 4, 0, "not whole units");
        assert_eq!(bytes.len(), size(value), "not the profile's size");
        let (decoded, seen) = traced::<T>(&bytes);
        assert_eq!(&decoded.unwrap(), value);
        check_ledger::<T>(message, bytes.len(), true, &seen);
    });
}

/// P2 and P6: every accepted input re-encodes to itself, and every input,
/// accepted or not, stays within the ledger's bounds.
fn accepted_inputs_are_canonical<T: Tested>(
    name: &str,
    message: &'static Message,
    generate: fn(&mut Source) -> T,
) {
    Property::new(format!("{name}: encode(decode(b)) = b"))
        .cases(1024)
        .check(
            |source| {
                let value = generate(source);
                (value, mutation(source))
            },
            |(value, mutation)| {
                let input = mutate(encode(value).unwrap(), mutation);
                let (decoded, seen) = traced::<T>(&input);
                check_ledger::<T>(message, input.len(), decoded.is_ok(), &seen);
                match decoded {
                    Ok(decoded) => assert_eq!(encode(&decoded).unwrap(), input),
                    Err(error) => {
                        if let Some(offset) = error.offset() {
                            assert!(offset <= input.len(), "{error}");
                        }
                    }
                }
            },
        );
}

/// P5 and P6 on truncated input: every proper prefix of a valid encoding
/// is `Truncated`.
fn prefixes_are_truncated<T: Tested>(
    name: &str,
    message: &'static Message,
    generate: fn(&mut Source) -> T,
) {
    Property::new(format!("{name}: proper prefixes are truncated"))
        .cases(64)
        .check(generate, |value| {
            let bytes = encode(value).unwrap();
            for length in 0..bytes.len() {
                let (decoded, seen) = traced::<T>(&bytes[..length]);
                let error = decoded.unwrap_err();
                assert_eq!(
                    error.kind(),
                    ErrorKind::Truncated,
                    "prefix of {length}: {error}"
                );
                check_ledger::<T>(message, length, false, &seen);
            }
        });
}

fn all_properties<T: Tested>(
    name: &str,
    message: &'static Message,
    generate: fn(&mut Source) -> T,
    size: fn(&T) -> usize,
) {
    encodes_and_decodes_back(name, message, generate, size);
    accepted_inputs_are_canonical(name, message, generate);
    prefixes_are_truncated(name, message, generate);
}

#[test]
fn shade_properties_hold() {
    all_properties::<Shade>("shade", &SHADE, support::shade, size::shade);
}

#[test]
fn shaded_properties_hold() {
    all_properties::<Shaded>("shaded", &SHADED, support::shaded, size::shaded);
}

#[test]
fn entry_properties_hold() {
    all_properties::<Entry>("entry", &ENTRY, support::entry, size::entry);
}

#[test]
fn label_properties_hold() {
    all_properties::<Label>("label", &LABEL, support::label, size::label);
}

#[test]
fn note_properties_hold() {
    all_properties::<Note>("note", &NOTE, support::note, size::note);
}

#[test]
fn pair_properties_hold() {
    all_properties::<Pair>("pair", &PAIR, support::pair, size::pair);
}

#[test]
fn names_properties_hold() {
    all_properties::<Names>("names", &NAMES, support::names, size::names);
}

#[test]
fn tags_properties_hold() {
    all_properties::<Tags>("tags", &TAGS, support::tags, size::tags);
}

#[test]
fn solos_properties_hold() {
    all_properties::<Solos>("solos", &SOLOS, support::solos, size::solos);
}

#[test]
fn ints_properties_hold() {
    all_properties::<Ints>("ints", &INTS, support::ints, size::ints);
}

#[test]
fn five_properties_hold() {
    all_properties::<Five>("five", &FIVE, support::five, size::five);
}

#[test]
fn blob_properties_hold() {
    all_properties::<Blob>("blob", &BLOB, support::blob, size::blob);
}

#[test]
fn wide_properties_hold() {
    all_properties::<Wide>("wide", &WIDE, support::wide, size::wide);
}

#[test]
fn deep_properties_hold() {
    all_properties::<Deep>("deep", &DEEP, support::deep, size::deep);
}

#[test]
fn every_properties_hold() {
    all_properties::<Every>("every", &EVERY, support::every, size::every);
}

/// The kind and cap or field name of an error.
fn named(error: &ironafs_xdr::Error) -> (ErrorKind, Option<&'static str>) {
    (error.kind(), error.name())
}

/// P4: the encoder refuses exactly what the decoder rejects, with the same
/// kind and the same cap or field name.
#[test]
fn encoder_refuses_what_decoder_rejects() {
    // note.body: cap 16 below its bound of 64; 17 bytes is CapExceeded.
    let over_cap = Note {
        title: vec![],
        what: Entry::Plain,
        owner: vec![],
        body: vec![0; 17],
    };
    let refused = encode(&over_cap).unwrap_err();
    let bytes =
        hex("00000000 00000000 00000000 00000011 00000000 00000000 00000000 00000000 00000000");
    let rejected = decode::<Note>(&bytes).unwrap_err();
    assert_eq!(
        named(&refused),
        (ErrorKind::CapExceeded, Some("test.note.body"))
    );
    assert_eq!(named(&refused), named(&rejected));

    // note.title: cap equal to its bound of 32; 33 bytes is above both,
    // and the bound is checked first.
    let over_bound = Note {
        title: vec![0; 33],
        ..over_cap.clone()
    };
    let refused = encode(&over_bound).unwrap_err();
    let mut bytes = hex("00000021");
    bytes.append_all([0; 36]);
    let rejected = decode::<Note>(&bytes).unwrap_err();
    assert_eq!(
        named(&refused),
        (ErrorKind::LengthOverBound, Some("test.note.title"))
    );
    assert_eq!(named(&refused), named(&rejected));

    // tags: four empty labels, 16 bytes within the cap, one over the bound.
    let four = Tags(vec![Label(vec![]); 4]);
    let refused = encode(&four).unwrap_err();
    let bytes = hex("00000004 00000000 00000000 00000000 00000000");
    let rejected = decode::<Tags>(&bytes).unwrap_err();
    assert_eq!(
        named(&refused),
        (ErrorKind::LengthOverBound, Some("test.tags"))
    );
    assert_eq!(named(&refused), named(&rejected));

    // names: labels of 4 and 8 bytes, 20 bytes over the cap of 16.
    let over = Names(vec![Label(b"abcd".owned()), Label(b"abcdefgh".owned())]);
    let refused = encode(&over).unwrap_err();
    let bytes = hex("00000002 00000004 61626364 00000008 61626364 65666768");
    let rejected = decode::<Names>(&bytes).unwrap_err();
    assert_eq!(
        named(&refused),
        (ErrorKind::CapExceeded, Some("test.names"))
    );
    assert_eq!(named(&refused), named(&rejected));
}

/// Sets each byte in `padding` of `valid` in turn to 01 and expects
/// `NonZeroPadding` at that byte's offset.
#[track_caller]
fn padding_is_checked<T: Tested>(message: &Message, valid: &[u8], padding: &[usize]) {
    assert_eq!(*message, T::MESSAGE, "the row is the type's own");
    assert!(decode::<T>(valid).is_ok());
    for &index in padding {
        let mut input = valid.owned();
        assert_eq!(input[index], 0);
        input[index] = 1;
        let error = decode::<T>(&input).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NonZeroPadding, "byte {index}");
        assert_eq!(error.offset(), Some(index));
    }
}

/// Adds the offsets of the padding of a string of `length` bytes at
/// `offset` to `padding`, and returns the offset after it.
fn string_padding(offset: usize, length: usize, padding: &mut Vec<usize>) -> usize {
    let data_end = offset + 4 + length;
    let end = offset + 4 + length.next_multiple_of(4);
    padding.append_all(data_end..end);
    end
}

/// P7: setting any one padding byte of a valid encoding to a non-zero
/// value gives `NonZeroPadding` at that byte's offset.
#[test]
fn non_zero_padding_is_rejected_at_its_offset() {
    Property::new("blob: padding is checked").check(support::blob, |blob| {
        let bytes = encode(blob).unwrap();
        let data_end = 4 + blob.0.len();
        let padding: Vec<usize> = (data_end..bytes.len()).gather();
        padding_is_checked::<Blob>(&BLOB, &bytes, &padding);
    });
    Property::new("five: padding is checked").check(support::five, |five| {
        let bytes = encode(five).unwrap();
        padding_is_checked::<Five>(&FIVE, &bytes, &[5, 6, 7]);
    });
    Property::new("note: padding is checked").check(support::note, |note| {
        let bytes = encode(note).unwrap();
        let mut padding = Vec::new();
        let mut offset = 0;
        offset = string_padding(offset, note.title.len(), &mut padding);
        offset = match &note.what {
            Entry::Plain => offset + 4,
            Entry::Linked(target) => string_padding(offset + 4, target.len(), &mut padding),
            Entry::Stamped(_) => offset + 12,
        };
        offset = string_padding(offset, note.owner.len(), &mut padding);
        offset = string_padding(offset, note.body.len(), &mut padding);
        assert_eq!(offset, bytes.len());
        padding_is_checked::<Note>(&NOTE, &bytes, &padding);
    });
}

/// The parts of a `sealed` value, owned by the test.
#[derive(Debug)]
struct SealedParts {
    key: Vec<u8>,
    serial: [u8; 4],
    tag: Vec<u8>,
}

impl SealedParts {
    fn sealed(&self) -> Sealed<'_> {
        Sealed {
            key: BorrowedBytes::new(&self.key),
            serial: BorrowedWord::new(&self.serial),
            tag: BorrowedBytes::new(&self.tag),
        }
    }
}

fn sealed_parts(source: &mut Source) -> SealedParts {
    SealedParts {
        key: source.bytes(0..=16),
        serial: source.u32().to_be_bytes(),
        tag: source.bytes(0..=8),
    }
}

/// Whether `inner` lies within `outer`.
fn within(outer: &[u8], inner: &[u8]) -> bool {
    let outer = outer.as_ptr_range();
    let inner = inner.as_ptr_range();
    outer.start <= inner.start && inner.end <= outer.end
}

/// P1 and P8 for `sealed`: it round-trips, its decode copies nothing from
/// the input and requests nothing, and its `Debug` text is the same for
/// any two accepted inputs. P2 is `sealed_accepted_inputs_are_canonical`.
#[test]
fn sealed_borrows_everything_and_shows_nothing() {
    let reference = {
        let parts = SealedParts {
            key: vec![],
            serial: [0; 4],
            tag: vec![],
        };
        let bytes = sealed_bytes(&parts.sealed());
        format!("{:?}", decode::<Sealed<'_>>(&bytes).unwrap())
    };
    assert_eq!(
        reference,
        "Sealed { key: BorrowedBytes, serial: BorrowedWord, tag: BorrowedBytes }"
    );
    Property::new("sealed: borrowed, silent and round-trips").check(sealed_parts, |parts| {
        let bytes = sealed_bytes(&parts.sealed());
        let (decoded, seen) = traced::<Sealed<'_>>(&bytes);
        let decoded = decoded.unwrap();
        assert_eq!(seen, []);
        assert!(within(&bytes, decoded.key.reveal()));
        assert!(within(&bytes, decoded.serial.reveal_bytes()));
        assert!(within(&bytes, decoded.tag.reveal()));
        assert_eq!(decoded.key.reveal(), parts.key);
        assert_eq!(decoded.serial.reveal_bytes(), &parts.serial);
        assert_eq!(decoded.tag.reveal(), parts.tag);
        assert_eq!(format!("{decoded:?}"), reference);
        assert_eq!(format!("{:?}", decoded.key), "BorrowedBytes");
        assert_eq!(format!("{:?}", decoded.serial), "BorrowedWord");
        assert_eq!(sealed_bytes(&decoded), bytes);
    });
}

/// P2 and P6 for `sealed`, on inputs made from its valid encodings as for
/// the other types: every accepted input re-encodes to itself and borrows
/// from it, no input, accepted or not, requests anything, and no error
/// carries a position (section 7, point 2).
#[test]
fn sealed_accepted_inputs_are_canonical() {
    Property::new("sealed: encode(decode(b)) = b")
        .cases(1024)
        .check(
            |source| (sealed_parts(source), mutation(source)),
            |(parts, mutation)| {
                let input = mutate(sealed_bytes(&parts.sealed()), mutation);
                let (decoded, seen) = traced::<Sealed<'_>>(&input);
                assert_eq!(seen, []);
                match decoded {
                    Ok(decoded) => {
                        assert!(within(&input, decoded.key.reveal()));
                        assert!(within(&input, decoded.serial.reveal_bytes()));
                        assert!(within(&input, decoded.tag.reveal()));
                        assert_eq!(sealed_bytes(&decoded), input);
                    }
                    Err(error) => {
                        assert_eq!((error.offset(), error.extent()), (None, None), "{error}");
                    }
                }
            },
        );
}
