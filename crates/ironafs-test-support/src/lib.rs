// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's own seeded generator and shrinker for property tests
//! (coding standards rules 15 and 21).
//!
//! A [`Property`] pairs a generator, which builds an input from the draws
//! of a [`Source`], with a test of one input. Inputs come from a seed that
//! every failure report prints; setting `IRONAFS_TEST_SEED` to it replays
//! the run. A failing input is shrunk through the choices its draws
//! recorded, so generators need no shrinking code of their own, and
//! shrinking always stops within its run budget and time limit. The seed is
//! printed when the first case fails, before shrinking starts.
//!
//! A [`Source`] can also read its choices from a byte string
//! ([`Source::from_bytes`]), so a fuzz target drives the same generators
//! from the fuzzer's input. That byte layout is a contract the fuzz
//! targets and their corpora depend on: each draw reads its choice as a
//! big-endian integer of as many bytes as its largest choice needs (none
//! when it has one possible value), reduces it modulo the number of
//! possible choices, and reads zero past the end of the input.
//! [`Source::choice_bytes`] writes a run's choices in that layout.
//!
//! This crate is a dev-dependency only: no shipped crate depends on it.

mod capture;
mod property;
mod shrink;
mod source;
mod splitmix64;

pub use property::{Counterexample, Property, Verdict};
pub use source::Source;
pub use splitmix64::SplitMix64;
