// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stable replay of the tracked seeds through the fuzz target's checks.

use std::fs;
use std::path::Path;

use super::invariants::check_input;

/// Reads the named syntax seed from the tracked corpus and checks it.
///
/// # Panics
///
/// Panics if the seed cannot be read or the parser violates an invariant.
#[track_caller]
fn replay_seed(name: &str) {
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/seeds/ironafs-config_syntax");
    let input = fs::read(directory.join(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
    check_input(&input);
}

#[test]
fn replay_accepted() {
    replay_seed("accepted");
}

#[test]
fn replay_bidi_control() {
    replay_seed("bidi-control");
}

#[test]
fn replay_empty() {
    replay_seed("empty");
}

#[test]
fn replay_float() {
    replay_seed("float");
}

#[test]
fn replay_line_at_cap() {
    replay_seed("line-at-cap");
}

#[test]
fn replay_line_over_cap() {
    replay_seed("line-over-cap");
}

#[test]
fn replay_not_a_table() {
    replay_seed("not-a-table");
}

#[test]
fn replay_too_deep() {
    replay_seed("too-deep");
}
