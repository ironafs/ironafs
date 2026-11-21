// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A seeded [`Rng`] for tests and the simulator (coding standards rule 7;
//! ADR 0004, C5, point 1: "the simulator seeds its own").
//!
//! It is not cryptographic and lives only in this dev-only crate, so no
//! shipped build can reach it: `cargo xtask check-deps` refuses a shipped
//! crate that depends on this one.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};

use ironafs_time::{Rng, RngError};

use crate::SplitMix64;

#[cfg(test)]
mod tests;

/// Random bytes from a seed: the little-endian bytes of successive
/// [`SplitMix64`] outputs, as one stream.
///
/// The stream does not depend on how it is split between calls: two
/// fills of four bytes give the same eight bytes as one fill of eight. A
/// test can make the source fail, to check that its caller fails closed.
///
/// ```
/// use ironafs_test_support::SeededRng;
/// use ironafs_time::Rng;
///
/// let mut first = [0_u8; 16];
/// let mut second = [0_u8; 16];
/// SeededRng::new(7).fill(&mut first).unwrap();
/// SeededRng::new(7).fill(&mut second).unwrap();
/// assert_eq!(first, second);
/// ```
#[derive(Debug)]
pub struct SeededRng {
    state: Mutex<RngState>,
}

#[derive(Debug)]
struct RngState {
    generator: SplitMix64,
    /// Bytes of the last output not yet handed out, first byte first.
    pending: VecDeque<u8>,
    failing: bool,
}

impl SeededRng {
    /// A source whose stream is fixed by `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            state: Mutex::new(RngState {
                generator: SplitMix64::new(seed),
                pending: VecDeque::new(),
                failing: false,
            }),
        }
    }

    /// Makes every later [`Rng::fill`] fail while `failing` is true. A
    /// failed fill draws nothing, so the stream resumes where it stopped.
    pub fn set_failing(&self, failing: bool) {
        self.state().failing = failing;
    }

    fn state(&self) -> MutexGuard<'_, RngState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Rng for SeededRng {
    fn fill(&self, bytes: &mut [u8]) -> Result<(), RngError> {
        let mut state = self.state();
        if state.failing {
            return Err(RngError::new());
        }
        for byte in bytes {
            *byte = state.next_byte();
        }
        Ok(())
    }
}

impl RngState {
    fn next_byte(&mut self) -> u8 {
        if self.pending.is_empty() {
            let output = self.generator.next_u64();
            self.pending.extend(output.to_le_bytes());
        }
        // Never empty here: eight bytes were just added if it was.
        self.pending.pop_front().unwrap_or_default()
    }
}
