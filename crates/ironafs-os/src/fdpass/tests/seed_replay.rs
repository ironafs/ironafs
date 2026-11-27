// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stable replay of the tracked seeds through the fuzz target's checks.

use std::fs;
use std::path::Path;

use super::invariants::check_input;

/// Reads the named receive model seed from the tracked corpus and checks it.
///
/// # Panics
///
/// Panics if the seed cannot be read or the model violates an invariant.
#[track_caller]
fn replay_seed(name: &str) {
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/seeds/ironafs-os_fdpass");
    let input = fs::read(directory.join(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
    check_input(&input);
}

#[test]
fn replay_control_past_buffer() {
    replay_seed("control-past-buffer");
}

#[test]
fn replay_control_short() {
    replay_seed("control-short");
}

#[test]
fn replay_empty() {
    replay_seed("empty");
}

#[test]
fn replay_end_of_stream() {
    replay_seed("end-of-stream");
}

#[test]
fn replay_header_past_control() {
    replay_seed("header-past-control");
}

#[test]
fn replay_negative_descriptor() {
    replay_seed("negative-descriptor");
}

#[test]
fn replay_one_descriptor() {
    replay_seed("one-descriptor");
}

#[test]
fn replay_peer_pidfd() {
    replay_seed("peer-pidfd");
}

#[test]
fn replay_picked_rules() {
    replay_seed("picked-rules");
}

#[test]
fn replay_truncated() {
    replay_seed("truncated");
}

#[test]
fn replay_two_descriptors() {
    replay_seed("two-descriptors");
}

#[test]
fn replay_wrong_data() {
    replay_seed("wrong-data");
}

#[test]
fn replay_wrong_level() {
    replay_seed("wrong-level");
}

#[test]
fn replay_wrong_type() {
    replay_seed("wrong-type");
}
