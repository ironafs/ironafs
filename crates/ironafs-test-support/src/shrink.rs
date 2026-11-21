// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shrinker: reduces a failing choice sequence to a simpler one.
//!
//! A candidate is accepted only if the property still fails on it and the
//! choices it recorded are strictly smaller in shortlex order (shorter, or
//! as long and lexicographically smaller), trailing zeros ignored because a
//! replay reads zero past the end. That order has no infinite descending
//! chain, so shrinking terminates on its own; the run budget also bounds
//! how many times the property is run.
//!
//! Besides blind passes over runs of choices, the shrinker deletes whole
//! elements of collections at the spans their draws recorded, so an
//! element of any number of draws is removed in one step.

use std::ops::Range;

use crate::source::Recording;

#[cfg(test)]
mod tests;

/// Chunk sizes tried by the passes that delete or zero runs of choices,
/// largest first so that big simplifications are found cheaply.
const CHUNK_SIZES: [usize; 4] = [8, 4, 2, 1];

/// How many of the smallest values are tried one by one before a binary
/// search, so that a small failing value below a larger failing threshold
/// is still found.
const SMALL_VALUES: u64 = 4;

/// The outcome of shrinking.
#[derive(Debug)]
pub(crate) struct Shrunk<P> {
    /// What the run on the simplest failing choices found returned.
    pub(crate) payload: P,
    /// The simplest failing choices, without trailing zeros.
    pub(crate) choices: Vec<u64>,
    /// How many times the property was run while shrinking.
    pub(crate) runs: u32,
    /// Whether shrinking stopped because the budget ran out.
    pub(crate) budget_exhausted: bool,
}

/// Shrinks the failing `recording`, whose run returned `payload`.
///
/// `fails` replays a candidate and returns what that run recorded and its
/// payload if the property failed, or `None` if it passed. It is called at
/// most `budget` times.
pub(crate) fn shrink<P>(
    recording: Recording,
    payload: P,
    budget: u32,
    fails: impl FnMut(&[u64]) -> Option<(Recording, P)>,
) -> Shrunk<P> {
    let mut shrinker = Shrinker {
        best: without_trailing_zeros(recording.choices),
        spans: recording.spans,
        payload,
        runs: 0,
        budget,
        budget_exhausted: false,
        fails,
    };
    loop {
        let before = shrinker.best.clone();
        shrinker.delete_spans();
        shrinker.delete_chunks();
        shrinker.zero_chunks();
        shrinker.minimise_each();
        shrinker.minimise_repeated();
        if shrinker.budget_exhausted || shrinker.best == before {
            break;
        }
    }
    Shrunk {
        payload: shrinker.payload,
        choices: shrinker.best,
        runs: shrinker.runs,
        budget_exhausted: shrinker.budget_exhausted,
    }
}

struct Shrinker<P, F> {
    best: Vec<u64>,
    /// The element spans the run on `best` recorded; they may reach past
    /// its end, where trailing zeros were removed.
    spans: Vec<Range<usize>>,
    payload: P,
    runs: u32,
    budget: u32,
    budget_exhausted: bool,
    fails: F,
}

impl<P, F: FnMut(&[u64]) -> Option<(Recording, P)>> Shrinker<P, F> {
    /// Runs `candidate` if it is simpler than the best so far and budget
    /// remains, and keeps it if it still fails. Returns whether it was kept.
    fn try_candidate(&mut self, candidate: Vec<u64>) -> bool {
        let candidate = without_trailing_zeros(candidate);
        if !shortlex_less(&candidate, &self.best) {
            return false;
        }
        if self.runs >= self.budget {
            self.budget_exhausted = true;
            return false;
        }
        self.runs += 1;
        match (self.fails)(&candidate) {
            Some((recording, payload)) => {
                let record = without_trailing_zeros(recording.choices);
                if shortlex_less(&record, &self.best) {
                    self.best = record;
                    self.spans = recording.spans;
                    self.payload = payload;
                    true
                } else {
                    false
                }
            }
            None => false,
        }
    }

    /// Tries removing each recorded element: its "one more" choice and all
    /// its draws. Deleting a span keeps the sequence well-formed: the
    /// collection reads its next continuation where the deleted one was,
    /// and every later draw reads its own choices.
    fn delete_spans(&mut self) {
        let mut index = 0;
        while !self.budget_exhausted {
            let Some(span) = self.spans.get(index).cloned() else {
                break;
            };
            let end = span.end.min(self.best.len());
            let start = span.start.min(end);
            let mut candidate = self.best.clone();
            candidate.drain(start..end);
            // A kept candidate brings new spans, and the element that now
            // sits at `index` is the next one to try.
            if !self.try_candidate(candidate) {
                index += 1;
            }
        }
    }

    /// Tries removing each run of consecutive choices. Removing the choices
    /// of one element of a collection removes that element.
    fn delete_chunks(&mut self) {
        for size in CHUNK_SIZES {
            let mut start = 0;
            while start < self.best.len() && !self.budget_exhausted {
                let end = start.saturating_add(size).min(self.best.len());
                let mut candidate = self.best.clone();
                candidate.drain(start..end);
                if !self.try_candidate(candidate) {
                    start += 1;
                }
            }
        }
    }

    /// Tries setting each run of consecutive choices to zero.
    fn zero_chunks(&mut self) {
        for size in CHUNK_SIZES {
            let mut start = 0;
            while start < self.best.len() && !self.budget_exhausted {
                let end = start.saturating_add(size).min(self.best.len());
                let mut candidate = self.best.clone();
                candidate
                    .iter_mut()
                    .take(end)
                    .skip(start)
                    .for_each(|choice| *choice = 0);
                self.try_candidate(candidate);
                start += 1;
            }
        }
    }

    /// Lowers each choice toward the smallest value that still fails,
    /// then among the values of the same parity: signed draws alternate
    /// signs (0, -1, 1, -2, …), so a failure that needs one sign fails only
    /// on every other choice.
    fn minimise_each(&mut self) {
        let mut index = 0;
        while index < self.best.len() && !self.budget_exhausted {
            let current = self.best.get(index).copied().unwrap_or(0);
            self.minimise_at(&[index], current);
            let current = self.best.get(index).copied().unwrap_or(0);
            let parity = current % 2;
            self.bisect(&[index], 0, current / 2, |step| parity + 2 * step);
            index += 1;
        }
    }

    /// Lowers together every choice that repeats a value, for failures that
    /// need two parts of the input to agree (two equal bytes, a field that
    /// matches a count), which lowering one choice at a time cannot reach.
    fn minimise_repeated(&mut self) {
        let mut seen = Vec::new();
        let mut index = 0;
        while index < self.best.len() && !self.budget_exhausted {
            let value = self.best.get(index).copied().unwrap_or(0);
            index += 1;
            if value == 0 || seen.contains(&value) {
                continue;
            }
            seen.push(value);
            let positions: Vec<usize> = self
                .best
                .iter()
                .enumerate()
                .filter(|(_, choice)| **choice == value)
                .map(|(position, _)| position)
                .collect();
            if positions.len() > 1 {
                self.minimise_at(&positions, value);
            }
        }
    }

    /// Lowers the value written at every one of `positions`, which all
    /// hold `current`. The values below [`SMALL_VALUES`] are tried first,
    /// in order, and the first that fails is kept. Otherwise a binary
    /// search between there and `current` finds a threshold: a failing
    /// value whose predecessor passed. That is the smallest failing value
    /// when failures are monotone above the small values; when they are
    /// not, the value kept still fails but a smaller one may exist.
    fn minimise_at(&mut self, positions: &[usize], current: u64) {
        let small = SMALL_VALUES.min(current);
        for probe in 0..small {
            if self.budget_exhausted || self.try_at(positions, probe) {
                return;
            }
        }
        self.bisect(positions, small, current, |probe| probe);
    }

    /// Binary-searches the steps from `low` to `high` for a threshold,
    /// writing `value(step)` at every one of `positions`; `value(high)` is
    /// the value they hold now.
    fn bisect(
        &mut self,
        positions: &[usize],
        mut low: u64,
        mut high: u64,
        value: impl Fn(u64) -> u64,
    ) {
        // Every step at or above `high` has failed; every step below `low`
        // has passed.
        while low < high && !self.budget_exhausted {
            let step = low + (high - low) / 2;
            if self.try_at(positions, value(step)) {
                high = step;
            } else {
                low = step + 1;
            }
        }
    }

    /// Tries the best sequence with `value` written at every one of
    /// `positions`. Returns whether it was kept.
    fn try_at(&mut self, positions: &[usize], value: u64) -> bool {
        let mut candidate = self.best.clone();
        for position in positions {
            if let Some(choice) = candidate.get_mut(*position) {
                *choice = value;
            }
        }
        self.try_candidate(candidate)
    }
}

/// Removes the zeros at the end, which a replay reads anyway.
fn without_trailing_zeros(mut choices: Vec<u64>) -> Vec<u64> {
    while choices.last() == Some(&0) {
        choices.pop();
    }
    choices
}

/// Whether `left` comes strictly before `right` in shortlex order.
fn shortlex_less(left: &[u64], right: &[u64]) -> bool {
    (left.len(), left) < (right.len(), right)
}
