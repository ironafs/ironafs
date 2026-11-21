// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The source of test inputs: draws recorded as a choice sequence.
//!
//! Every draw reads one choice, an integer from zero up to a bound, and
//! maps it to a value so that a smaller choice gives a simpler value: a
//! smaller integer, `false`, a shorter length, the first item of a list.
//! The shrinker works on the recorded choices, never on the values, so
//! any generator built from these draws shrinks without code of its own.

use std::ops::RangeInclusive;

use crate::SplitMix64;

#[cfg(test)]
mod tests;

/// Where a [`Source`] takes its choices from.
#[derive(Debug)]
enum Origin {
    /// New choices from a seeded generator.
    Fresh(SplitMix64),
    /// Choices given by the shrinker; zero once they run out.
    Replay { choices: Vec<u64>, position: usize },
}

/// Draws test inputs for a property and records each choice it made.
///
/// A property's generator receives a `Source` and builds its input from
/// the draws below. The same seed, or the same recorded choices, always
/// give the same input.
///
/// ```
/// use ironafs_test_support::Property;
///
/// Property::new("sorting keeps the length").check(
///     |source| source.vec_of(0..=16, |source| source.u32()),
///     |values| {
///         let mut sorted = values.clone();
///         sorted.sort_unstable();
///         assert_eq!(sorted.len(), values.len());
///     },
/// );
/// ```
#[derive(Debug)]
pub struct Source {
    origin: Origin,
    record: Vec<u64>,
}

impl Source {
    pub(crate) fn fresh(generator: SplitMix64) -> Self {
        Self {
            origin: Origin::Fresh(generator),
            record: Vec::new(),
        }
    }

    pub(crate) fn replay(choices: Vec<u64>) -> Self {
        Self {
            origin: Origin::Replay {
                choices,
                position: 0,
            },
            record: Vec::new(),
        }
    }

    /// The choices drawn so far, in order.
    pub(crate) fn into_record(self) -> Vec<u64> {
        self.record
    }

    /// Draws one choice in `0..=max` and records it.
    fn draw(&mut self, max: u64) -> u64 {
        let choice = match &mut self.origin {
            Origin::Fresh(generator) => {
                // Scales a full-width output onto 0..=max (the high half of
                // the 128-bit product); max == u64::MAX gives the output
                // unchanged.
                let wide = u128::from(generator.next_u64()) * (u128::from(max) + 1);
                u64::try_from(wide >> 64).unwrap_or(max)
            }
            Origin::Replay { choices, position } => {
                let choice = choices.get(*position).copied().unwrap_or(0);
                *position = position.saturating_add(1);
                // A shrunk sequence can put a choice where a smaller bound
                // is drawn; reduce it rather than reject the sequence.
                if choice <= max {
                    choice
                } else {
                    choice % (max + 1)
                }
            }
        };
        self.record.push(choice);
        choice
    }

    /// Draws an integer in `range`, shrinking toward its start.
    ///
    /// # Panics
    ///
    /// Panics if `range` is empty.
    pub fn u64_in(&mut self, range: RangeInclusive<u64>) -> u64 {
        let (start, end) = range.into_inner();
        assert!(start <= end, "u64_in needs a non-empty range");
        start + self.draw(end - start)
    }

    /// Draws a length or index in `range`, shrinking toward its start.
    ///
    /// # Panics
    ///
    /// Panics if `range` is empty.
    pub fn usize_in(&mut self, range: RangeInclusive<usize>) -> usize {
        let (start, end) = range.into_inner();
        assert!(start <= end, "usize_in needs a non-empty range");
        let span = u64::try_from(end - start).unwrap_or(u64::MAX);
        // The draw is at most `end - start`, so it fits in a usize.
        start + usize::try_from(self.draw(span)).unwrap_or(0)
    }

    /// Draws a boolean, shrinking toward `false`.
    pub fn bool(&mut self) -> bool {
        self.draw(1) == 1
    }

    /// Draws any `u8`, shrinking toward zero.
    pub fn u8(&mut self) -> u8 {
        u8::try_from(self.draw(u64::from(u8::MAX))).unwrap_or(0)
    }

    /// Draws any `u16`, shrinking toward zero.
    pub fn u16(&mut self) -> u16 {
        u16::try_from(self.draw(u64::from(u16::MAX))).unwrap_or(0)
    }

    /// Draws any `u32`, shrinking toward zero.
    pub fn u32(&mut self) -> u32 {
        u32::try_from(self.draw(u64::from(u32::MAX))).unwrap_or(0)
    }

    /// Draws any `u64`, shrinking toward zero.
    pub fn u64(&mut self) -> u64 {
        self.draw(u64::MAX)
    }

    /// Draws any `i32`, shrinking toward zero by magnitude.
    pub fn i32(&mut self) -> i32 {
        let choice = u32::try_from(self.draw(u64::from(u32::MAX))).unwrap_or(0);
        // Zigzag order: 0, -1, 1, -2, 2, … A halved u32 always fits.
        i32::try_from(choice >> 1).unwrap_or(0) ^ -i32::from(choice & 1 == 1)
    }

    /// Draws any `i64`, shrinking toward zero by magnitude.
    pub fn i64(&mut self) -> i64 {
        let choice = self.draw(u64::MAX);
        // Zigzag order: 0, -1, 1, -2, 2, … A halved u64 always fits.
        i64::try_from(choice >> 1).unwrap_or(0) ^ -i64::from(choice & 1 == 1)
    }

    /// Draws a byte string whose length is in `len`, shrinking toward the
    /// shortest length and zero bytes.
    ///
    /// # Panics
    ///
    /// Panics if `len` is empty.
    pub fn bytes(&mut self, len: RangeInclusive<usize>) -> Vec<u8> {
        self.vec_of(len, Self::u8)
    }

    /// Draws a vector whose length is in `len`, each element from
    /// `element`, shrinking toward fewer and simpler elements.
    ///
    /// # Panics
    ///
    /// Panics if `len` is empty.
    pub fn vec_of<T>(
        &mut self,
        len: RangeInclusive<usize>,
        mut element: impl FnMut(&mut Self) -> T,
    ) -> Vec<T> {
        let count = self.usize_in(len);
        (0..count).map(|_| element(self)).collect()
    }

    /// Picks one of `items`, shrinking toward the first.
    ///
    /// To choose between generators, pick an index or an enum and match
    /// on it, with the simplest variant first.
    ///
    /// # Panics
    ///
    /// Panics if `items` is empty.
    #[expect(
        clippy::panic,
        reason = "rule 1 covers untrusted input; an empty list is a mistake in the test, reported as `assert!` would"
    )]
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let Some((first, _)) = items.split_first() else {
            panic!("pick needs at least one item");
        };
        let index = self.usize_in(0..=items.len() - 1);
        // The index is below the length, so `get` always finds an item.
        items.get(index).unwrap_or(first)
    }
}
