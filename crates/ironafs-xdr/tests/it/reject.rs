// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The reject corpus of `docs/specs/xdr.md`, 10.3: one file per case under
//! `tests/fixtures/reject/`, each naming its type, the error it expects,
//! the section it breaks and the exact reservations made before the error.
//!
//! A fixture is lines of `key value`, after `#` comment lines:
//!
//! - `type`: the test type the input is decoded as;
//! - `breaks`: the section of the specification it breaks;
//! - `expect`: the error kind, then the field or cap name, or `-`;
//! - `offset`, `extent`: the error's position, or `none`;
//! - `budget`: optional, a memory budget below the message's;
//! - `requests`: `none`, or each reservation as `name:bytes`, or
//!   `name:slotsN` for N slots of the type's element;
//! - `input`: the bytes, in hexadecimal grouped by unit.

use std::fs;
use std::mem::size_of;
use std::path::{Path, PathBuf};

use ironafs_xdr::{
    Classified, Decode, Decoder, Error, ErrorKind, Message, MessageType, Reservation,
};

use crate::support::heap::{Append, Gather, Owned, Text};
use crate::support::{hex, traced_with_budget};
use crate::types::{
    Blob, Entry, Every, Flag, Ints, Label, Names, Note, Pair, Sealed, Shade, Shaded, Tags,
};

/// The message an `int` decoded on its own belongs to.
const INT: Message = Message {
    name: "test.int",
    cap: 128,
    budget: 1024,
};

/// One case of the corpus.
#[derive(Debug, Default)]
struct Case {
    file: PathBuf,
    type_name: String,
    breaks: String,
    kind: String,
    name: Option<String>,
    offset: Option<usize>,
    extent: Option<usize>,
    budget: Option<u64>,
    requests: Vec<(String, String)>,
    input: Vec<u8>,
}

fn optional_number<T: std::str::FromStr>(value: &str) -> Option<T> {
    (value != "none").then(|| {
        value
            .parse()
            .unwrap_or_else(|_| panic!("not a number: {value}"))
    })
}

fn parse(file: &Path) -> Case {
    let text = fs::read_to_string(file).unwrap();
    let mut case = Case {
        file: file.to_path_buf(),
        ..Case::default()
    };
    let mut seen_input = false;
    for line in text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
    {
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        match key {
            "type" => case.type_name = value.owned(),
            "breaks" => case.breaks = value.owned(),
            "expect" => {
                let (kind, name) = value.split_once(' ').unwrap();
                case.kind = kind.owned();
                case.name = (name != "-").then(|| name.owned());
            }
            "offset" => case.offset = optional_number(value),
            "extent" => case.extent = optional_number(value),
            "budget" => case.budget = optional_number(value),
            "requests" => {
                if value != "none" {
                    case.requests = value
                        .split_whitespace()
                        .map(|request| {
                            let (name, bytes) = request.split_once(':').unwrap();
                            (name.owned(), bytes.owned())
                        })
                        .gather();
                }
            }
            "input" => {
                case.input = hex(value);
                seen_input = true;
            }
            _ => panic!("{}: unknown key {key}", file.display()),
        }
    }
    assert!(seen_input, "{}: no input", file.display());
    assert!(!case.breaks.is_empty(), "{}: no section", file.display());
    case
}

/// An `int` decoded on its own, as a message.
struct Int;

impl Classified for Int {
    const SECRET: bool = false;
}

impl MessageType for Int {
    const MESSAGE: Message = INT;
}

impl<'a> Decode<'a> for Int {
    const S_MIN: u64 = i32::S_MIN;
    const DEPTH: u32 = i32::DEPTH;
    const HEAP_RATIO: Option<u64> = i32::HEAP_RATIO;
    const RESERVATIONS: Option<u64> = i32::RESERVATIONS;

    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        decoder.read_i32().map(|_| Self)
    }
}

/// Decodes `input` as `T`, keeping the outcome and the ledger's requests.
fn reject<'a, T: Decode<'a> + MessageType>(
    input: &'a [u8],
    budget: Option<u64>,
) -> (Result<(), Error>, Vec<Reservation>) {
    let (result, seen) = traced_with_budget::<T>(input, budget.unwrap_or(T::MESSAGE.budget));
    (result.map(|_| ()), seen)
}

/// The outcome and reservations of decoding the case's input, and the size
/// of its type's array element, if it has one.
fn run(case: &Case) -> (Result<(), Error>, Vec<Reservation>, Option<usize>) {
    let input = &case.input;
    let budget = case.budget;
    let ((result, seen), element) = match case.type_name.as_str() {
        "int" => (reject::<Int>(input, budget), None),
        "flag" => (reject::<Flag>(input, budget), None),
        "shade" => (reject::<Shade>(input, budget), None),
        "shaded" => (reject::<Shaded>(input, budget), None),
        "entry" => (reject::<Entry>(input, budget), None),
        "label" => (reject::<Label>(input, budget), None),
        "blob" => (reject::<Blob>(input, budget), None),
        "note" => (reject::<Note>(input, budget), None),
        "pair" => (reject::<Pair>(input, budget), None),
        "names" => (reject::<Names>(input, budget), Some(size_of::<Label>())),
        "tags" => (reject::<Tags>(input, budget), Some(size_of::<Label>())),
        "ints" => (reject::<Ints>(input, budget), Some(size_of::<u32>())),
        "sealed" => (reject::<Sealed<'_>>(input, budget), None),
        "every" => (reject::<Every>(input, budget), None),
        other => panic!("unknown type {other}"),
    };
    (result, seen, element)
}

fn kind_name(kind: ErrorKind) -> String {
    format!("{kind:?}")
}

fn fixtures() -> Vec<PathBuf> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reject");
    let mut files: Vec<PathBuf> = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
        .gather();
    files.sort();
    files
}

#[test]
fn reject_corpus_fails_with_exact_kind_offset_and_reservations() {
    let files = fixtures();
    assert_eq!(
        files.len(),
        30,
        "the corpus of section 10.3, R6 as one file per padding byte"
    );
    for file in files {
        let case = parse(&file);
        let label = case.file.display();
        let (result, seen, element) = run(&case);
        let kind_and_offset = result
            .as_ref()
            .err()
            .map(|error| (kind_name(error.kind()), error.offset()));
        assert_eq!(
            kind_and_offset,
            Some((case.kind, case.offset)),
            "{label}: {result:?}"
        );
        let error = result.unwrap_err();
        assert_eq!(error.name(), case.name.as_deref(), "{label}: {error}");
        assert_eq!(error.extent(), case.extent, "{label}: {error}");
        let expected: Vec<Reservation> = case
            .requests
            .iter()
            .map(|(name, bytes)| {
                let bytes = match bytes.strip_prefix("slots") {
                    Some(count) => count.parse::<usize>().unwrap() * element.unwrap(),
                    None => bytes.parse().unwrap(),
                };
                let name = seen
                    .iter()
                    .map(|reservation| reservation.name)
                    .find(|seen_name| seen_name == name)
                    .unwrap_or_else(|| panic!("{label}: no request named {name}: {seen:?}"));
                Reservation {
                    bytes: u64::try_from(bytes).unwrap(),
                    name,
                }
            })
            .gather();
        assert_eq!(seen, expected, "{label}");
    }
}

/// R15: errors from the secret-bearing message carry no offset or length,
/// and their text names only the kind, the field and the specification's
/// constant.
#[test]
fn sealed_errors_name_kind_and_field_only() {
    let mut over = hex("00000011");
    over.append_all([0x5a; 20]);
    over.append_all(hex("00000001 00000000"));
    let error = reject::<Sealed<'_>>(&over, None).0.unwrap_err();
    assert_eq!(
        error.text(),
        "length over declared bound: test.sealed.key, limit 16"
    );
    assert_eq!(
        format!("{error:?}"),
        "Error { kind: LengthOverBound, name: Some(\"test.sealed.key\"), limit: Some(16), position: None }"
    );
    let cut = hex("00000000 0000");
    let error = reject::<Sealed<'_>>(&cut, None).0.unwrap_err();
    assert_eq!(error.text(), "input truncated");
    assert_eq!(
        format!("{error:?}"),
        "Error { kind: Truncated, name: None, limit: None, position: None }"
    );
}
