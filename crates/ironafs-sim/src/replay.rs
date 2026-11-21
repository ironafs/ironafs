// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Strict replay (seams and scheduling specification, 5.6, point 11): the
//! logged indices are fed back to the scheduler, and every line the
//! replay appends is compared with the original's, in order. The first
//! difference stops the run as a divergence at that decision; a decision
//! past the end of the log, or records left unused at the end, are
//! divergences too. There is no lenient mode here: minimisation's lenient
//! replay is never evidence of determinism.

use std::collections::BTreeMap;

use crate::sched::Failure;
use crate::trace::decisions;

#[cfg(test)]
mod tests;

/// A recorded trace, fed back to a run.
#[derive(Debug)]
pub struct Replay {
    expected: Vec<String>,
    next_line: usize,
    picks: BTreeMap<u64, usize>,
}

impl Replay {
    /// A replay of `trace`: its lines, and the index of each of its
    /// decisions. A decision number logged twice keeps its first index;
    /// the line comparison catches the extra record.
    #[must_use]
    pub fn from_trace(trace: &str) -> Self {
        let mut picks = BTreeMap::new();
        for decision in decisions(trace) {
            picks.entry(decision.number).or_insert(decision.pick);
        }
        Self {
            expected: trace.lines().map(str::to_owned).collect(),
            next_line: 0,
            picks,
        }
    }

    /// Whether `line` is the original's next line; moves past it.
    pub(crate) fn matches_next(&mut self, line: &str) -> bool {
        let expected = self.expected.get(self.next_line);
        self.next_line = self.next_line.saturating_add(1);
        expected.is_some_and(|expected| expected == line)
    }

    /// The logged index of `decision`.
    pub(crate) fn pick(&self, decision: u64) -> Result<usize, Failure> {
        self.picks
            .get(&decision)
            .copied()
            .ok_or(Failure::Diverged { decision })
    }

    /// Whether every recorded line has been compared.
    pub(crate) fn is_exhausted(&self) -> bool {
        self.next_line >= self.expected.len()
    }
}
