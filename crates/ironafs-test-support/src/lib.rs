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
//! This crate is a dev-dependency only: no shipped crate depends on it.

mod capture;
mod property;
mod shrink;
mod source;
mod splitmix64;

pub use property::{Counterexample, Property, Verdict};
pub use source::Source;
pub use splitmix64::SplitMix64;
