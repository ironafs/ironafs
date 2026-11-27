// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stable replay of the tracked seeds through the fuzz target's checks.

use super::decode_invariants::check_input;
use super::support::read_decode_seed;

#[test]
fn replay_empty() {
    check_input(&read_decode_seed("empty"));
}

#[test]
fn replay_every_message_cap() {
    check_input(&read_decode_seed("every-message-cap"));
}

#[test]
fn replay_every_message_cap_plus_one() {
    check_input(&read_decode_seed("every-message-cap-plus-one"));
}

#[test]
fn replay_g14_shaded_explicit() {
    check_input(&read_decode_seed("g14-shaded-explicit"));
}

#[test]
fn replay_g15_shaded_default() {
    check_input(&read_decode_seed("g15-shaded-default"));
}

#[test]
fn replay_one_word() {
    check_input(&read_decode_seed("one-word"));
}

#[test]
fn replay_r09b_shaded_undeclared() {
    check_input(&read_decode_seed("r09b-shaded-undeclared"));
}

#[test]
fn replay_zero_word() {
    check_input(&read_decode_seed("zero-word"));
}
