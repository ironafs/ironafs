// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of `linux-random` against the running kernel (ADR 0005, sheet 8:
//! buffers of 0, 1, 256, 257 and 4096 bytes). The fill loop's handling of
//! short counts, `EINTR`, `ENOSYS`, zero and over-long counts is
//! `random`'s, with a stand-in; the classification of the call's errno is
//! tested here.

use std::io;

use super::abi::{EINTR, ENOSYS, EPERM};
use super::{classify, fill_random, getrandom};
use crate::random::{Piece, RandomError};

#[test]
#[cfg_attr(
    miri,
    ignore = "a real system call: Miri checks the Rust around it through the stand-in tests"
)]
fn fill_random_fills_buffers_of_each_size_the_sheet_names() {
    for length in [0_usize, 1, 256, 257, 4096] {
        // Zero, then a fill: a 1-byte buffer can draw zero honestly, so
        // only the larger ones are checked for content.
        let mut buffer = vec![0_u8; length];
        fill_random(&mut buffer).unwrap();
        if length >= 256 {
            assert!(buffer.iter().any(|byte| *byte != 0), "{length}");
        }
    }
}

#[test]
#[cfg_attr(
    miri,
    ignore = "a real system call: Miri checks the Rust around it through the stand-in tests"
)]
fn fill_random_draws_different_bytes_each_time() {
    let mut first = [0_u8; 64];
    let mut second = [0_u8; 64];
    fill_random(&mut first).unwrap();
    fill_random(&mut second).unwrap();
    assert_ne!(first, second);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "a real system call: Miri checks the Rust around it through the stand-in tests"
)]
fn getrandom_reports_a_full_piece_of_at_most_256_bytes() {
    let mut piece = [0_u8; 256];
    assert!(matches!(getrandom(&mut piece), Piece::Filled(256)));
}

#[test]
fn classify_counts_a_non_negative_return_as_bytes_filled() {
    assert!(matches!(classify(0, None), Piece::Filled(0)));
    assert!(matches!(classify(256, None), Piece::Filled(256)));
}

#[test]
fn classify_retries_eintr_and_names_enosys_and_eperm_unavailable() {
    let errno = |value| Some(io::Error::from_raw_os_error(value));
    assert!(matches!(classify(-1, errno(EINTR)), Piece::Interrupted));
    for value in [ENOSYS, EPERM] {
        match classify(-1, errno(value)) {
            Piece::Unavailable(error) => assert_eq!(error.raw_os_error(), Some(value)),
            other => panic!("{value}: {other:?}"),
        }
    }
    for value in [14, 22, 11] {
        match classify(-1, errno(value)) {
            Piece::Failed(error) => assert_eq!(error.raw_os_error(), Some(value)),
            other => panic!("{value}: {other:?}"),
        }
    }
}

#[test]
fn random_errors_carry_the_real_errno() {
    let error = RandomError::Unavailable(io::Error::from_raw_os_error(ENOSYS));
    assert!(error.to_string().contains("os error 38"), "{error}");
}
