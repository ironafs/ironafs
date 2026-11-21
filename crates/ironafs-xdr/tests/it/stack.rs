// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The stack bound of `docs/specs/xdr.md`, 5.2, point 10.4: the largest
//! valid input of every test type decodes on a thread with a 64 KiB stack.
//! A stack overflow aborts the test process, which fails the run.

use std::fmt::Debug;
use std::thread;

use ironafs_xdr::limits::SMALL_STACK;
use ironafs_xdr::{BorrowedBytes, BorrowedWord, Decode, Message, Plain, decode, encode};

use crate::support::{deep_from, sealed_bytes};
use crate::types::{
    BLOB, Blob, DEEP, Deep16, ENTRY, EVERY, Entry, Every, FIVE, FLAG, Five, Flag, INTS, Ints,
    LABEL, Label, NAMES, NOTE, Names, Note, PAIR, Pair, Quad, SEALED, SHADE, SOLOS, Sealed, Shade,
    Solo, Solos, TAGS, Tags, WIDE, Wide,
};

/// Encodes `value`, checks the encoding is as long as `size`, and decodes
/// it back on a thread with the small stack, expecting `value`.
#[track_caller]
fn fits<T>(message: &'static Message, value: &T, size: usize)
where
    T: for<'a> Decode<'a> + Plain + PartialEq + Debug + Send + 'static,
{
    let bytes = encode(message, value).unwrap();
    assert_eq!(bytes.len(), size, "{value:?}");
    let decoded = thread::Builder::new()
        .stack_size(SMALL_STACK)
        .spawn(move || decode::<T>(message, &bytes))
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(&decoded.unwrap(), value);
}

fn bytes(n: usize) -> Vec<u8> {
    (1..=n)
        .map(|byte| u8::try_from(byte % 251).unwrap())
        .collect()
}

fn label(n: usize) -> Label {
    Label(bytes(n))
}

#[test]
fn decode_fits_small_stack() {
    let mut values = 0..;
    let innermost = Deep16 {
        value: u64::MAX,
        pad: [0xa5; 64],
    };
    let deep = deep_from(innermost, || values.next().unwrap());
    fits(&DEEP, &deep, 192);
    let quad = Quad {
        a: [1; 60],
        b: [2; 60],
        c: [3; 60],
        d: [4; 60],
    };
    fits(&WIDE, &Wide::Big(quad), 244);
    let every = Every {
        int: i32::MIN,
        unsigned: u32::MAX,
        hyper: i64::MIN,
        unsigned_hyper: u64::MAX,
        flag: true,
        shade: Shade::Vivid,
        big: Box::new(std::array::from_fn(|index| {
            u8::try_from(index % 251).unwrap()
        })),
        var: bytes(8),
        name: label(8),
        fixed: [7, 8],
        list: vec![label(0), label(0), label(0), label(0)],
        what: Entry::Linked(bytes(16)),
    };
    fits(&EVERY, &every, 32 + 100 + 12 + 12 + 8 + 20 + 24);
    let note = Note {
        title: bytes(32),
        what: Entry::Linked(bytes(16)),
        owner: bytes(8),
        body: bytes(16),
    };
    fits(&NOTE, &note, 36 + 24 + 12 + 20);
    fits(&NAMES, &Names(vec![label(4), label(4)]), 20);
    fits(&TAGS, &Tags(vec![label(4), label(0), label(0)]), 20);
    fits(&PAIR, &Pair([label(8), label(8)]), 24);
    fits(&SOLOS, &Solos(vec![Solo; 4]), 20);
    fits(&INTS, &Ints((0..16).collect()), 68);
    fits(&BLOB, &Blob(bytes(16)), 20);
    fits(&FIVE, &Five(*b"abcde"), 8);
    fits(&LABEL, &label(8), 12);
    fits(&ENTRY, &Entry::Linked(bytes(16)), 24);
    fits(&SHADE, &Shade::Deep, 4);
    fits(&FLAG, &Flag(true), 4);
}

#[test]
fn sealed_decode_fits_small_stack() {
    let key = bytes(16);
    let serial = [0, 0, 0, 9];
    let tag = bytes(8);
    let sealed = Sealed {
        key: BorrowedBytes::new(&key),
        serial: BorrowedWord::new(&serial),
        tag: BorrowedBytes::new(&tag),
    };
    let bytes = sealed_bytes(&sealed);
    assert_eq!(bytes.len(), 20 + 4 + 12);
    let expected = bytes.clone();
    let decoded = thread::Builder::new()
        .stack_size(SMALL_STACK)
        .spawn(move || {
            let decoded = decode::<Sealed<'_>>(&SEALED, &bytes).unwrap();
            (
                decoded.key.reveal().to_vec(),
                decoded.serial.reveal_u32(),
                decoded.tag.reveal().to_vec(),
                sealed_bytes(&decoded),
            )
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(decoded, (key, 9, tag, expected));
}
