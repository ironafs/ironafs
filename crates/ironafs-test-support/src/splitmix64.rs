// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `SplitMix64` pseudo-random generator.
//!
//! Written from its published description: Guy L. Steele Jr., Doug Lea and
//! Christine H. Flood, "Fast Splittable Pseudorandom Number Generators",
//! OOPSLA 2014 (<https://doi.org/10.1145/2714064.2660195>), as the
//! fixed-increment form in Sebastiano Vigna's public domain reference
//! `splitmix.c` (<https://prng.di.unimi.it/splitmix.c>), whose increment,
//! multipliers and shifts this file uses.
//!
//! The known-answer test checks the first five outputs for seed zero.
//! A published list of those outputs: UNVERIFIED. The reference file
//! prints none, and the paper has not been checked for any. The expected
//! values were checked by compiling the reference `splitmix.c`
//! unchanged and running it from seed zero (Apple clang 21, macOS,
//! 2026-11-21).

#[cfg(test)]
mod tests;

/// The increment added to the state before each output: the odd integer
/// nearest to 2^64 divided by the golden ratio.
const GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// A seeded `SplitMix64` generator: deterministic, fast, and not
/// cryptographic.
///
/// The whole sequence is fixed by the seed, so a test that prints its seed
/// can be replayed exactly.
///
/// ```
/// use ironafs_test_support::SplitMix64;
///
/// let mut generator = SplitMix64::new(0);
/// assert_eq!(generator.next_u64(), 0xe220_a839_7b1d_cdaf);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Creates a generator whose sequence is fixed by `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next 64-bit output.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GAMMA);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}
