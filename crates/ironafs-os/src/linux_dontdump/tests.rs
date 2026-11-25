// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of `linux-dontdump` (ADR 0005, sheet 12) that Miri runs too: with
//! a stand-in for the exclusion, the whole 64 KiB-aligned allocation is
//! advised, a failure hands out no buffer, and the buffer clears with
//! `wipe`; and `SecretBuffer::new` through the site's call, which under
//! Miri reaches the call's stand-in. The tests that need the running kernel
//! are in `kernel`.

use std::cell::Cell;
use std::io;

use super::{BLOCK, SecretBuffer, exclude_with};
use crate::dump::DumpError;

#[cfg(not(miri))]
mod kernel;
#[cfg(miri)]
mod stand_in;

const ENOMEM: i32 = 12;

#[test]
fn secret_buffer_new_advises_its_whole_block_through_the_call() {
    let mut buffer = SecretBuffer::new().unwrap();
    assert_eq!(buffer.as_slice().as_ptr().addr() % (64 * 1024), 0);
    assert!(buffer.as_slice().iter().all(|byte| *byte == 0));
    buffer.as_mut_slice().fill(0x5A);
    drop(buffer);
}

#[test]
fn exclude_with_advises_the_whole_64_kib_aligned_allocation() {
    let seen = Cell::new((1_usize, 0_usize));
    let buffer = exclude_with(|memory| {
        seen.set((memory.as_ptr().addr(), memory.len()));
        Ok(())
    })
    .unwrap();
    let (address, length) = seen.get();
    assert_eq!(address % (64 * 1024), 0, "{address:#x}");
    assert_eq!(length, 64 * 1024);
    assert_eq!(address, buffer.as_slice().as_ptr().addr());
    assert_eq!(SecretBuffer::SIZE, BLOCK);
}

#[test]
fn exclude_with_hands_out_no_buffer_when_the_exclusion_fails() {
    match exclude_with(|_| Err(io::Error::from_raw_os_error(ENOMEM))) {
        Err(DumpError::Exclude(error)) => assert_eq!(error.raw_os_error(), Some(ENOMEM)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn secret_buffer_starts_zeroed_and_clear_zeroes_every_byte() {
    let mut buffer = exclude_with(|_| Ok(())).unwrap();
    assert!(buffer.as_slice().iter().all(|byte| *byte == 0));
    buffer.as_mut_slice().fill(0xA5);
    buffer.clear();
    assert_eq!(buffer.as_slice().len(), SecretBuffer::SIZE);
    assert!(buffer.as_slice().iter().all(|byte| *byte == 0));
}

#[test]
fn secret_buffer_debug_shows_its_size_and_none_of_its_bytes() {
    let mut buffer = exclude_with(|_| Ok(())).unwrap();
    buffer.as_mut_slice().fill(0xA5);
    assert_eq!(format!("{buffer:?}"), "SecretBuffer { size: 65536, .. }");
}
