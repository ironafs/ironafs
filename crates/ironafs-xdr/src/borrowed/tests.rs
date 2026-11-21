// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![expect(
    clippy::disallowed_macros,
    reason = "the tests format the wrappers to check their text"
)]

use alloc::format;

use super::{BorrowedBytes, BorrowedHyper, BorrowedWord};

#[test]
fn debug_prints_the_type_name_only() {
    let bytes = [0x6b, 0x65, 0x79, 0x21, 0, 0, 0, 7];
    let word = [0, 0, 0, 7];
    assert_eq!(format!("{:?}", BorrowedBytes::new(&bytes)), "BorrowedBytes");
    assert_eq!(
        format!("{:#?}", BorrowedBytes::new(&bytes[..3])),
        "BorrowedBytes"
    );
    assert_eq!(format!("{:?}", BorrowedWord::new(&word)), "BorrowedWord");
    assert_eq!(format!("{:?}", BorrowedHyper::new(&bytes)), "BorrowedHyper");
}

#[test]
fn reveal_returns_the_input_where_it_lies() {
    let bytes = [0xff, 0xff, 0xff, 0xfe, 1, 2, 3, 4];
    let borrowed = BorrowedBytes::new(&bytes);
    assert_eq!(borrowed.reveal().as_ptr(), bytes.as_ptr());
    let word = bytes.first_chunk::<4>().unwrap();
    let borrowed = BorrowedWord::new(word);
    assert_eq!(borrowed.reveal_i32(), -2);
    assert_eq!(borrowed.reveal_u32(), 0xffff_fffe);
    assert_eq!(borrowed.reveal_bytes().as_ptr(), bytes.as_ptr());
    let hyper = BorrowedHyper::new(&bytes);
    assert_eq!(hyper.reveal_u64(), 0xffff_fffe_0102_0304);
    assert_eq!(hyper.reveal_i64(), -0x0000_0001_fefd_fcfc);
}
