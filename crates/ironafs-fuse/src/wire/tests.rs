// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the little-endian reader and writer.

use super::{Reader, put_u16, put_u32, put_u64};

#[test]
fn reader_reads_little_endian() {
    let bytes = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
    ];
    let mut reader = Reader::new(&bytes);
    assert_eq!(reader.u16(), Some(0x0201));
    assert_eq!(reader.u32(), Some(0x0605_0403));
    assert_eq!(reader.u64(), Some(0x0e0d_0c0b_0a09_0807));
    assert_eq!(reader.rest(), b"");
}

#[test]
fn reader_refuses_past_end_and_keeps_its_place() {
    let bytes = [1, 2, 3];
    let mut reader = Reader::new(&bytes);
    assert_eq!(reader.u32(), None);
    assert_eq!(reader.u16(), Some(0x0201));
    assert_eq!(reader.skip(2), None);
    assert_eq!(reader.skip(1), Some(()));
    assert_eq!(reader.u16(), None);
}

#[test]
fn writer_writes_little_endian() {
    let mut out = Vec::new();
    put_u16(&mut out, 0x0201);
    put_u32(&mut out, 0x0605_0403);
    put_u64(&mut out, 0x0e0d_0c0b_0a09_0807);
    assert_eq!(out, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
}
