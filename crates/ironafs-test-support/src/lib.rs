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
//! printed when the first case fails, before shrinking starts. The report
//! also prints a `#[test]` that replays the minimal input through
//! [`Property::regression`]; failures are kept as such Rust tests in the
//! owning crate's `tests/it/regressions.rs`, never as data files the
//! runner writes.
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
//! For a domain small enough to enumerate, an [`Exhaustive`] sweep runs a
//! test on every input the same generators can give, up to a bound it
//! prints, and refuses a larger domain rather than sampling it. The
//! [`wire`] module gives variants of a known wire message for decoder
//! tests: single-bit flips, integers near a known one, and fields at all
//! zeros and all ones.
//!
//! [`SeededClock`] and [`SeededRng`] implement the `Clock` and `Rng`
//! interfaces of `ironafs-time` for tests and the simulator: virtual clocks
//! that move only when the test moves them, and random bytes from a seed.
//! Neither is ever a production source (coding standards rule 7).
//!
//! [`ScriptedCore`] is a core behind the `ironafs-vfs` contract whose
//! state a test scripts, so a layer is tested without the client chain
//! (ADR 0004, C7).
//!
//! This crate is a dev-dependency only: no shipped crate depends on it.

mod capture;
mod clock;
mod property;
mod rng;
mod scripted;
mod shrink;
mod source;
mod splitmix64;
mod sweep;
pub mod wire;

pub use clock::{SeededClock, TimeOverflow};
pub use property::{Counterexample, Property, Verdict};
pub use rng::SeededRng;
pub use scripted::{Evidence, ScriptedCore};
pub use source::Source;
pub use splitmix64::SplitMix64;
pub use sweep::{Exhaustive, FailedInput, SweepFailure};
