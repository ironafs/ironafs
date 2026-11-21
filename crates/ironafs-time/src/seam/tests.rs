// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the seam errors' messages, which are part of their contract
//! (coding standards rule 4: lowercase, no trailing period).

use alloc::string::ToString;

use super::{ClockError, ClockKind, RngError};

#[test]
fn clock_error_names_the_clock() {
    assert_eq!(
        ClockError::new(ClockKind::Boot).to_string(),
        "the boot clock could not be read"
    );
    assert_eq!(
        ClockError::new(ClockKind::Awake).to_string(),
        "the awake clock could not be read"
    );
    assert_eq!(
        ClockError::new(ClockKind::Wall).to_string(),
        "wall time could not be read"
    );
}

#[test]
fn clock_error_keeps_its_clock() {
    assert_eq!(ClockError::new(ClockKind::Awake).clock(), ClockKind::Awake);
}

#[test]
fn rng_error_says_the_source_failed() {
    assert_eq!(
        RngError::new().to_string(),
        "the random source could not supply bytes"
    );
}
