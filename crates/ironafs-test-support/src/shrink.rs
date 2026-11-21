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

#[cfg(test)]
mod tests;

/// Chunk sizes tried by the passes that delete or zero runs of choices,
/// largest first so that big simplifications are found cheaply.
const CHUNK_SIZES: [usize; 4] = [8, 4, 2, 1];

/// The outcome of shrinking.
#[derive(Debug)]
pub(crate) struct Shrunk<P> {
    /// What the run on the simplest failing choices found returned.
    pub(crate) payload: P,
    /// How many times the property was run while shrinking.
    pub(crate) runs: u32,
    /// Whether shrinking stopped because the budget ran out.
    pub(crate) budget_exhausted: bool,
}

/// Shrinks the failing `choices`, whose run returned `payload`.
///
/// `fails` replays a candidate and returns the choices that run recorded
/// and its payload if the property failed, or `None` if it passed. It is
/// called at most `budget` times.
pub(crate) fn shrink<P>(
    choices: Vec<u64>,
    payload: P,
    budget: u32,
    fails: impl FnMut(&[u64]) -> Option<(Vec<u64>, P)>,
) -> Shrunk<P> {
    let mut shrinker = Shrinker {
        best: without_trailing_zeros(choices),
        payload,
        runs: 0,
        budget,
        budget_exhausted: false,
        fails,
    };
    loop {
        let before = shrinker.best.clone();
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
        runs: shrinker.runs,
        budget_exhausted: shrinker.budget_exhausted,
    }
}

struct Shrinker<P, F> {
    best: Vec<u64>,
    payload: P,
    runs: u32,
    budget: u32,
    budget_exhausted: bool,
    fails: F,
}

impl<P, F: FnMut(&[u64]) -> Option<(Vec<u64>, P)>> Shrinker<P, F> {
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
            Some((record, payload)) => {
                let record = without_trailing_zeros(record);
                if shortlex_less(&record, &self.best) {
                    self.best = record;
                    self.payload = payload;
                    true
                } else {
                    false
                }
            }
            None => false,
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

    /// Lowers each choice to the smallest value that still fails.
    fn minimise_each(&mut self) {
        let mut index = 0;
        while index < self.best.len() && !self.budget_exhausted {
            let current = self.best.get(index).copied().unwrap_or(0);
            self.minimise_at(&[index], current);
            index += 1;
        }
    }

    /// Lowers together every choice that repeats a value, for failures that
    /// need two parts of the input to agree (two equal bytes, a length that
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

    /// Finds, by binary search between zero and `current`, the smallest
    /// value that still fails when written at every one of `positions`.
    fn minimise_at(&mut self, positions: &[usize], current: u64) {
        // Every probe at or above `high` has failed; every probe below
        // `low` has passed. Zero is probed first.
        let mut low = 0;
        let mut high = current;
        let mut probe = 0;
        while low < high && !self.budget_exhausted {
            let mut candidate = self.best.clone();
            for position in positions {
                if let Some(choice) = candidate.get_mut(*position) {
                    *choice = probe;
                }
            }
            if self.try_candidate(candidate) {
                high = probe;
            } else {
                low = probe + 1;
            }
            probe = low + (high - low) / 2;
        }
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
