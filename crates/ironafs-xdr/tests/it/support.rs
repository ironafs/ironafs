// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Helpers shared by the integration tests: hexadecimal input, a traced
//! decode, and seeded generators for the test types.

use ironafs_test_support::Source;
use ironafs_xdr::{Decode, Error, Message, Reservation, decode_traced, encode_into};

use crate::types::{
    Blob, Deep, Deep16, Entry, Every, Five, Ints, Label, Names, Note, Pair, Quad, SEALED, Sealed,
    Shade, Solo, Solos, Tags, Wide,
};

/// The bytes a hexadecimal string spells, ignoring whitespace.
///
/// # Panics
///
/// Panics on an odd count of digits or a character that is not one.
#[track_caller]
pub(crate) fn hex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .map(|byte| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("not a hexadecimal digit: {:?}", char::from(byte)),
        })
        .collect();
    assert!(
        digits.len().is_multiple_of(2),
        "odd count of hexadecimal digits"
    );
    digits
        .chunks(2)
        .map(|pair| pair[0] << 4 | pair[1])
        .collect()
}

/// Decodes `input` as one `message` of type `T` under the message's own
/// budget, and returns the result with every reservation recorded.
pub(crate) fn traced<'a, T: Decode<'a>>(
    message: &Message,
    input: &'a [u8],
) -> (Result<T, Error>, Vec<Reservation>) {
    traced_with_budget(message, input, message.budget)
}

/// As [`traced`], with the budget lowered to `budget`.
pub(crate) fn traced_with_budget<'a, T: Decode<'a>>(
    message: &Message,
    input: &'a [u8],
    budget: u64,
) -> (Result<T, Error>, Vec<Reservation>) {
    let mut seen = Vec::new();
    let result = decode_traced(message, input, budget, &mut |reservation| {
        seen.push(reservation);
    });
    (result, seen)
}

/// Encodes a `sealed` value into a buffer the test owns: `encode_into` is
/// the only path for a secret-bearing message.
pub(crate) fn sealed_bytes(sealed: &Sealed<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(&SEALED, sealed, &mut out).unwrap();
    out
}

// ---- generators: every value they draw is within its caps ----

/// The encoded size of a label of `n` bytes.
fn label_size(n: usize) -> usize {
    4 + n.next_multiple_of(4)
}

pub(crate) fn label(source: &mut Source) -> Label {
    Label(source.bytes(0..=8))
}

/// Labels whose encodings fit in `cap` bytes, at most `most` of them.
fn labels(source: &mut Source, most: usize, cap: usize) -> Vec<Label> {
    let mut room = cap;
    let mut labels = Vec::new();
    for candidate in source.vec_of(0..=most, label) {
        let size = label_size(candidate.0.len());
        if size <= room {
            room -= size;
            labels.push(candidate);
        }
    }
    labels
}

pub(crate) fn shade(source: &mut Source) -> Shade {
    *source.pick(&Shade::ALL)
}

pub(crate) fn entry(source: &mut Source) -> Entry {
    match source.usize_in(0..=2) {
        0 => Entry::Plain,
        1 => Entry::Linked(source.bytes(0..=16)),
        _ => Entry::Stamped(source.u64()),
    }
}

pub(crate) fn note(source: &mut Source) -> Note {
    Note {
        title: source.bytes(0..=32),
        what: entry(source),
        owner: source.bytes(0..=8),
        body: source.bytes(0..=16),
    }
}

pub(crate) fn pair(source: &mut Source) -> Pair {
    Pair([label(source), label(source)])
}

pub(crate) fn names(source: &mut Source) -> Names {
    Names(labels(source, 4, 16))
}

pub(crate) fn tags(source: &mut Source) -> Tags {
    Tags(labels(source, 3, 16))
}

pub(crate) fn solos(source: &mut Source) -> Solos {
    Solos(source.vec_of(0..=4, |_| Solo))
}

pub(crate) fn ints(source: &mut Source) -> Ints {
    Ints(source.vec_of(0..=16, Source::u32))
}

pub(crate) fn five(source: &mut Source) -> Five {
    let bytes = source.bytes(5..=5);
    Five([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4]])
}

pub(crate) fn blob(source: &mut Source) -> Blob {
    Blob(source.bytes(0..=16))
}

/// A fixed-length opaque of `N` bytes.
fn array<const N: usize>(source: &mut Source) -> [u8; N] {
    let bytes = source.bulk_bytes(N..=N);
    let mut array = [0; N];
    array.copy_from_slice(&bytes);
    array
}

pub(crate) fn wide(source: &mut Source) -> Wide {
    if source.bool() {
        Wide::Big(Quad {
            a: array(source),
            b: array(source),
            c: array(source),
            d: array(source),
        })
    } else {
        Wide::Small
    }
}

pub(crate) fn deep(source: &mut Source) -> Deep {
    let innermost = Deep16 {
        value: source.u64(),
        pad: array(source),
    };
    deep_from(innermost, || source.u64())
}

/// Builds the chain of `deep` around its innermost link.
pub(crate) fn deep_from(innermost: Deep16, mut value: impl FnMut() -> u64) -> Deep {
    use crate::types::{
        Deep2, Deep3, Deep4, Deep5, Deep6, Deep7, Deep8, Deep9, Deep10, Deep11, Deep12, Deep13,
        Deep14, Deep15,
    };
    let next = Deep15 {
        value: value(),
        next: innermost,
    };
    let next = Deep14 {
        value: value(),
        next,
    };
    let next = Deep13 {
        value: value(),
        next,
    };
    let next = Deep12 {
        value: value(),
        next,
    };
    let next = Deep11 {
        value: value(),
        next,
    };
    let next = Deep10 {
        value: value(),
        next,
    };
    let next = Deep9 {
        value: value(),
        next,
    };
    let next = Deep8 {
        value: value(),
        next,
    };
    let next = Deep7 {
        value: value(),
        next,
    };
    let next = Deep6 {
        value: value(),
        next,
    };
    let next = Deep5 {
        value: value(),
        next,
    };
    let next = Deep4 {
        value: value(),
        next,
    };
    let next = Deep3 {
        value: value(),
        next,
    };
    let next = Deep2 {
        value: value(),
        next,
    };
    Deep {
        value: value(),
        next,
    }
}

pub(crate) fn every(source: &mut Source) -> Every {
    Every {
        int: source.i32(),
        unsigned: source.u32(),
        hyper: source.i64(),
        unsigned_hyper: source.u64(),
        flag: source.bool(),
        shade: shade(source),
        big: Box::new(array(source)),
        var: source.bytes(0..=8),
        name: label(source),
        fixed: [source.u32(), source.u32()],
        list: labels(source, 4, 16),
        what: entry(source),
    }
}

/// Each test type's encoded size, from the table of `docs/specs/xdr.md`,
/// 5.1, computed apart from the codec's own `encoded_len`.
pub(crate) mod size {
    use crate::types::{
        Blob, Deep, Entry, Every, Five, Ints, Label, Names, Note, Pair, Shade, Solos, Tags, Wide,
    };

    /// A string or variable-length opaque of `n` bytes: 4 + n + pad(n).
    fn string(n: usize) -> usize {
        4 + n + (4 - n % 4) % 4
    }

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "rule 15: every size function has the property's signature"
    )]
    pub(crate) fn shade(_: &Shade) -> usize {
        4
    }

    pub(crate) fn entry(entry: &Entry) -> usize {
        match entry {
            Entry::Plain => 4,
            Entry::Linked(target) => 4 + string(target.len()),
            Entry::Stamped(_) => 4 + 8,
        }
    }

    pub(crate) fn label(label: &Label) -> usize {
        string(label.0.len())
    }

    pub(crate) fn note(note: &Note) -> usize {
        string(note.title.len())
            + entry(&note.what)
            + string(note.owner.len())
            + string(note.body.len())
    }

    pub(crate) fn pair(pair: &Pair) -> usize {
        pair.0.iter().map(label).sum()
    }

    pub(crate) fn names(names: &Names) -> usize {
        4 + names.0.iter().map(label).sum::<usize>()
    }

    pub(crate) fn tags(tags: &Tags) -> usize {
        4 + tags.0.iter().map(label).sum::<usize>()
    }

    pub(crate) fn solos(solos: &Solos) -> usize {
        4 + 4 * solos.0.len()
    }

    pub(crate) fn ints(ints: &Ints) -> usize {
        4 + 4 * ints.0.len()
    }

    pub(crate) fn five(_: &Five) -> usize {
        8
    }

    pub(crate) fn blob(blob: &Blob) -> usize {
        string(blob.0.len())
    }

    pub(crate) fn wide(wide: &Wide) -> usize {
        match wide {
            Wide::Small => 4,
            Wide::Big(_) => 4 + 4 * 60,
        }
    }

    pub(crate) fn deep(_: &Deep) -> usize {
        16 * 8 + 64
    }

    pub(crate) fn every(every: &Every) -> usize {
        4 + 4
            + 8
            + 8
            + 4
            + 4
            + 100
            + string(every.var.len())
            + label(&every.name)
            + 2 * 4
            + 4
            + every.list.iter().map(label).sum::<usize>()
            + entry(&every.what)
    }
}
