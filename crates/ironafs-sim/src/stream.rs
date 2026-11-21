// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Seeded streams: every random choice of a run comes from one stream per
//! label (the schedule, the network, the code under test's `Rng`), each
//! fixed by the run's seed, the label and the incarnation (seams and
//! scheduling specification, 5.6, point 6). One crash point therefore
//! reproduces alone, and a change to how one stream is used leaves the
//! others' values where they were.

use std::ops::RangeInclusive;

use ironafs_test_support::{Source, SplitMix64};

#[cfg(test)]
mod tests;

/// The 64-bit FNV-1a offset basis and prime, from the published
/// description of the hash (Fowler, Noll and Vo; IETF draft
/// draft-eastlake-fnv).
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The seed of the stream for `label` in `incarnation` of the run seeded
/// by `seed`.
///
/// The label is hashed with 64-bit FNV-1a; the run's seed is mixed once
/// through [`SplitMix64`], combined with the label's hash and the
/// incarnation, and mixed again. Every step is a fixed integer function,
/// so the value is the same on every target.
#[must_use]
pub fn stream_seed(seed: u64, label: &str, incarnation: u64) -> u64 {
    let label_hash = label.bytes().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    });
    let run = SplitMix64::new(seed).next_u64();
    SplitMix64::new(run ^ label_hash ^ incarnation.rotate_left(32)).next_u64()
}

/// One seeded stream of choices.
///
/// Its draws use `ironafs-test-support`'s [`Source`], whose range
/// reduction is IronAFS's own and value-stable, so a seed gives the same
/// choices on every target and release.
#[derive(Debug)]
pub struct Stream {
    source: Source,
    draws: u64,
}

impl Stream {
    /// The stream for `label` in `incarnation` of the run seeded by `seed`.
    #[must_use]
    pub fn new(seed: u64, label: &str, incarnation: u64) -> Self {
        Self {
            source: Source::from_seed(stream_seed(seed, label, incarnation)),
            draws: 0,
        }
    }

    /// Draws an index below `len`, or `None` when `len` is zero. A
    /// one-entry draw is still a draw, and is counted.
    pub fn index(&mut self, len: usize) -> Option<usize> {
        let last = len.checked_sub(1)?;
        self.draws = self.draws.saturating_add(1);
        Some(self.source.usize_in(0..=last))
    }

    /// Draws an integer in `range`, or `None` when it is empty.
    pub fn u64_in(&mut self, range: RangeInclusive<u64>) -> Option<u64> {
        if range.is_empty() {
            return None;
        }
        self.draws = self.draws.saturating_add(1);
        Some(self.source.u64_in(range))
    }

    /// How many draws the stream has given.
    #[must_use]
    pub const fn draws(&self) -> u64 {
        self.draws
    }
}
