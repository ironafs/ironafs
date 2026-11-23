// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The buffer's failure path and its destruction, driven through
//! `SecretBuffer` itself with the call's Miri stand-in: a failed exclusion
//! hands out no buffer, and a dropped buffer is all zero when its memory
//! is released, before it is freed. A wrapper that ignored the call's
//! status, or a destructor that did not wipe, fails here. Built only under
//! Miri, where the stand-in replaces the call.

use super::super::SecretBuffer;
use super::super::abi::stand_in::{Plan, last_release, plan};
use crate::dump::DumpError;

/// `ENOMEM`.
const ENOMEM: i32 = 12;

#[test]
fn a_failed_exclusion_hands_out_no_buffer() {
    plan(Plan::Fail(ENOMEM));
    match SecretBuffer::new() {
        Err(DumpError::Exclude(error)) => assert_eq!(error.raw_os_error(), Some(ENOMEM)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_dropped_buffer_is_all_zero_when_its_memory_is_released() {
    let mut buffer = SecretBuffer::new().unwrap();
    buffer.as_mut_slice().fill(0xA5);
    drop(buffer);
    assert_eq!(last_release(), Some(true));
}
