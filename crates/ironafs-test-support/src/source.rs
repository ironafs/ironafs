// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The source of test inputs: draws recorded as a choice sequence.
//!
//! Every draw reads one choice, an integer from zero up to a bound, and
//! maps it to a value so that a smaller choice gives a simpler value: a
//! smaller integer, `false`, a shorter length, the first item of a list.
//! The shrinker works on the recorded choices, never on the values, so
//! any generator built from these draws shrinks without code of its own.

use std::ops::{Range, RangeInclusive};

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
    /// Choices read from bytes, as [`Source::from_bytes`] describes; zero
    /// once they run out.
    Bytes { bytes: Vec<u8>, position: usize },
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
    /// How many bytes each recorded choice takes in the layout that
    /// [`Source::from_bytes`] reads.
    widths: Vec<u8>,
    spans: Vec<Range<usize>>,
}

/// The choices a run drew, and where in them each deletable element of a
/// collection lies.
#[derive(Debug)]
pub(crate) struct Recording {
    /// The choices drawn, in order.
    pub(crate) choices: Vec<u64>,
    /// The range of `choices` each deletable element took, outer elements
    /// before the elements inside them. Removing the choices in one span
    /// removes that element and leaves every other draw reading its own
    /// choices.
    pub(crate) spans: Vec<Range<usize>>,
}

impl From<Vec<u64>> for Recording {
    /// A recording with no known elements.
    fn from(choices: Vec<u64>) -> Self {
        Self {
            choices,
            spans: Vec::new(),
        }
    }
}

impl Source {
    /// A source of fresh draws whose sequence is fixed by `seed`, for
    /// seeded inputs outside a [`Property`](crate::Property), such as a
    /// simulator's schedule.
    ///
    /// ```
    /// use ironafs_test_support::Source;
    ///
    /// let mut first = Source::from_seed(7);
    /// let mut second = Source::from_seed(7);
    /// assert_eq!(first.u64(), second.u64());
    /// assert_eq!(first.choices(), second.choices());
    /// ```
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        Self {
            origin: Origin::Fresh(SplitMix64::new(seed)),
            record: Vec::new(),
            widths: Vec::new(),
            spans: Vec::new(),
        }
    }

    /// A source that replays `choices`, such as a counterexample's minimal
    /// choices, and draws zero choices once they run out.
    #[must_use]
    pub fn from_choices(choices: Vec<u64>) -> Self {
        Self {
            origin: Origin::Replay {
                choices,
                position: 0,
            },
            record: Vec::new(),
            widths: Vec::new(),
            spans: Vec::new(),
        }
    }

    /// A source that reads its choices from `bytes`, such as a fuzzer's
    /// input, so that a fuzz target drives the same generators as a
    /// property. Any byte string gives valid values.
    ///
    /// Each draw reads its choice as a big-endian integer of as many bytes
    /// as its largest choice needs (none for a draw with one possible
    /// value), and reduces a choice past that largest one modulo the
    /// number of possible choices. Near the end it reads only the bytes
    /// left, and past the end it reads zero. [`Source::choice_bytes`]
    /// writes a run's choices in this layout, so the bytes it returns
    /// replay that run.
    ///
    /// ```
    /// use ironafs_test_support::Source;
    ///
    /// let mut source = Source::from_bytes(&[0x01, 0x02, 0x03]);
    /// assert_eq!(source.u16(), 0x0102);
    /// assert_eq!(source.u8(), 0x03);
    /// assert_eq!(source.u8(), 0);
    /// ```
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            origin: Origin::Bytes {
                bytes: bytes.to_vec(),
                position: 0,
            },
            record: Vec::new(),
            widths: Vec::new(),
            spans: Vec::new(),
        }
    }

    /// The choices drawn so far in the byte layout that
    /// [`Source::from_bytes`] reads, for seeding a fuzzer's corpus with
    /// the inputs of a property.
    ///
    /// ```
    /// use ironafs_test_support::Source;
    ///
    /// let mut seeded = Source::from_seed(9);
    /// let value = (seeded.u32(), seeded.string(8));
    /// let mut driven = Source::from_bytes(&seeded.choice_bytes());
    /// assert_eq!((driven.u32(), driven.string(8)), value);
    /// ```
    #[must_use]
    pub fn choice_bytes(&self) -> Vec<u8> {
        self.record
            .iter()
            .zip(&self.widths)
            .flat_map(|(choice, width)| {
                choice
                    .to_be_bytes()
                    .into_iter()
                    .skip(8 - usize::from(*width))
            })
            .collect()
    }

    /// The choices drawn so far, in order.
    #[must_use]
    pub fn choices(&self) -> &[u64] {
        &self.record
    }

    /// The choices drawn so far, in order.
    #[cfg(test)]
    pub(crate) fn into_record(self) -> Vec<u64> {
        self.record
    }

    /// The choices drawn so far and the elements' spans in them.
    pub(crate) fn into_recording(self) -> Recording {
        Recording {
            choices: self.record,
            spans: self.spans,
        }
    }

    /// Draws one optional element: a recorded "one more" choice, which a
    /// fresh run takes from `more`, then the element from `element` if
    /// that choice is one and `room` allows it. The continuation choice and
    /// the element's draws are recorded as one span, so that the shrinker
    /// can delete the element whole, however many draws it took.
    fn more_element<T>(
        &mut self,
        more: bool,
        room: bool,
        element: impl FnOnce(&mut Self) -> T,
    ) -> Option<T> {
        let start = self.record.len();
        if self.draw_with(1, |_| u64::from(more)) == 0 || !room {
            return None;
        }
        // Reserve the span's place first, so that it comes before the spans
        // of any elements drawn inside this one.
        let slot = self.spans.len();
        self.spans.push(start..start);
        let value = element(self);
        let end = self.record.len();
        if let Some(span) = self.spans.get_mut(slot) {
            span.end = end;
        }
        Some(value)
    }

    /// Draws one choice in `0..=max` and records it.
    fn draw(&mut self, max: u64) -> u64 {
        self.draw_with(max, |generator| fresh_choice(generator, max))
    }

    /// Draws one choice in `0..=max` and records it, taking it from `fresh`
    /// in a fresh run. A replayed run reads the given choices instead, so
    /// `fresh` decides only how often each choice is drawn, never what the
    /// choice means.
    fn draw_with(&mut self, max: u64, fresh: impl FnOnce(&mut SplitMix64) -> u64) -> u64 {
        let width = byte_width(max);
        let choice = match &mut self.origin {
            Origin::Fresh(generator) => fresh(generator).min(max),
            Origin::Replay { choices, position } => {
                let choice = choices.get(*position).copied().unwrap_or(0);
                *position = position.saturating_add(1);
                // A shrunk sequence can put a choice where a smaller bound
                // is drawn; reduce it rather than reject the sequence.
                reduce(choice, max)
            }
            Origin::Bytes { bytes, position } => {
                let read = bytes.iter().skip(*position).take(usize::from(width));
                let choice = read.fold(0, |choice, byte| (choice << 8) | u64::from(*byte));
                *position = position.saturating_add(usize::from(width));
                reduce(choice, max)
            }
        };
        self.record.push(choice);
        self.widths.push(width);
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

    /// Draws an integer in `range`, shrinking toward zero by magnitude
    /// (negative first at equal magnitude) when the range holds zero, and
    /// toward the end nearer zero otherwise.
    ///
    /// # Panics
    ///
    /// Panics if `range` is empty.
    pub fn i64_in(&mut self, range: RangeInclusive<i64>) -> i64 {
        let (start, end) = range.into_inner();
        assert!(start <= end, "i64_in needs a non-empty range");
        let (start, end) = (i128::from(start), i128::from(end));
        let choice = i128::from(self.draw(u64::try_from(end - start).unwrap_or(u64::MAX)));
        let value = if start >= 0 {
            start + choice
        } else if end <= 0 {
            end - choice
        } else {
            // Zigzag (0, -1, 1, -2, 2, …) while both sides have values,
            // then the rest of the longer side in order.
            let pairs = (-start).min(end);
            if choice <= 2 * pairs {
                if choice % 2 == 1 {
                    -(choice + 1) / 2
                } else {
                    choice / 2
                }
            } else if -start > end {
                pairs - choice
            } else {
                choice - pairs
            }
        };
        // The value lies in the range, so it fits in an i64.
        i64::try_from(value).unwrap_or(0)
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

    /// Draws a large byte string whose length is in `len`, recorded as two
    /// choices only: the length, then a seed that fills the bytes.
    ///
    /// Seed zero gives all zero bytes; any other seed `n` gives the outputs
    /// of [`SplitMix64::new(n)`](SplitMix64::new), each written
    /// little-endian, the last one cut to the length. Shrinking lowers the
    /// length first, then the seed toward zero, and never works byte by
    /// byte, so a multi-MiB input costs two choices and a fill.
    ///
    /// Use [`Source::bytes`] when the property depends on particular byte
    /// values (a parser's tags, lengths and delimiters), so that each byte
    /// shrinks on its own; its cost grows with the length (two choices a
    /// byte), so keep it to inputs of a few KiB. Use `bulk_bytes` for large
    /// payloads whose content the code under test only carries or checks
    /// as a whole (opaque XDR data, file contents, Rx jumbogram payloads):
    /// a failure that depends on the length shrinks to the shortest
    /// length, but one that depends on particular bytes shrinks only to
    /// the lowest seed that still fails.
    ///
    /// ```
    /// use ironafs_test_support::Source;
    ///
    /// let mut source = Source::from_seed(5);
    /// let payload = source.bulk_bytes(0..=4 << 20);
    /// assert!(payload.len() <= 4 << 20);
    /// assert_eq!(source.choices().len(), 2);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `len` is empty.
    pub fn bulk_bytes(&mut self, len: RangeInclusive<usize>) -> Vec<u8> {
        let (min, max) = len.into_inner();
        assert!(min <= max, "bulk_bytes needs a non-empty length range");
        let length = self.usize_in(min..=max);
        let seed = self.u64();
        let mut bytes = vec![0; length];
        if seed != 0 {
            let mut generator = SplitMix64::new(seed);
            for chunk in bytes.chunks_mut(8) {
                let output = generator.next_u64().to_le_bytes();
                chunk
                    .iter_mut()
                    .zip(output)
                    .for_each(|(byte, value)| *byte = value);
            }
        }
        bytes
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
        let (min, max) = len.into_inner();
        assert!(min <= max, "vec_of needs a non-empty length range");
        // A fresh run picks the length up front, without recording it. Each
        // element past `min` is then preceded by a recorded "one more"
        // choice, and the vector ends with a recorded "no more" even at
        // `max`, where it is read and ignored. Deleting an element's
        // choices together with its continuation then removes exactly that
        // element, and the draws after the vector keep their own choices.
        let target = match &mut self.origin {
            Origin::Fresh(generator) => {
                let span = u64::try_from(max - min).unwrap_or(u64::MAX);
                min.saturating_add(usize::try_from(fresh_choice(generator, span)).unwrap_or(0))
            }
            Origin::Replay { .. } | Origin::Bytes { .. } => max,
        };
        let mut values: Vec<T> = (0..min).map(|_| element(self)).collect();
        loop {
            let (more, room) = (values.len() < target, values.len() < max);
            match self.more_element(more, room, &mut element) {
                Some(value) => values.push(value),
                None => return values,
            }
        }
    }

    /// Draws a string of at most `max_bytes` bytes of UTF-8, shrinking
    /// toward fewer characters and lower code points. A fresh run aims for
    /// a length in bytes drawn up to `max_bytes`, and draws three
    /// characters in four from ASCII, one in eight from the rest of the
    /// Basic Multilingual Plane and one in eight from any scalar value.
    ///
    /// ```
    /// use ironafs_test_support::Source;
    ///
    /// let text = Source::from_seed(3).string(16);
    /// assert!(text.len() <= 16);
    /// ```
    pub fn string(&mut self, max_bytes: usize) -> String {
        self.string_below(max_bytes, char::MAX)
    }

    /// Draws a string of at most `max_bytes` ASCII characters, shrinking
    /// toward fewer characters and lower code points.
    pub fn ascii_string(&mut self, max_bytes: usize) -> String {
        self.string_below(max_bytes, '\u{7f}')
    }

    /// Draws a string of at most `max_bytes` bytes, each character at most
    /// `highest`. Like [`Source::vec_of`], every character is preceded by
    /// a "one more" choice and the string ends with a "no more".
    fn string_below(&mut self, max_bytes: usize, highest: char) -> String {
        let target = match &mut self.origin {
            Origin::Fresh(generator) => {
                let span = u64::try_from(max_bytes).unwrap_or(u64::MAX);
                usize::try_from(fresh_choice(generator, span)).unwrap_or(0)
            }
            Origin::Replay { .. } | Origin::Bytes { .. } => max_bytes,
        };
        let mut text = String::new();
        loop {
            let room = max_bytes.saturating_sub(text.len());
            let more = text.len() < target && room > 0;
            let drawn =
                self.more_element(more, room > 0, |source| source.char_within(room, highest));
            match drawn {
                Some(character) => text.push(character),
                None => return text,
            }
        }
    }

    /// Draws a character at most `highest` whose UTF-8 encoding fits in
    /// `room` bytes (at least one), shrinking toward U+0000.
    fn char_within(&mut self, room: usize, highest: char) -> char {
        let widest = match room {
            0 | 1 => '\u{7f}',
            2 => '\u{7ff}',
            3 => '\u{ffff}',
            _ => char::MAX,
        };
        let last = scalar_index(widest.min(highest));
        let choice = self.draw_with(last, |generator| {
            // The tiers pick how often each range is drawn, not what a
            // choice means: index n is always the nth scalar value.
            let tier = generator.next_u64() % 8;
            let bound = match tier {
                0..=5 => scalar_index('\u{7f}'),
                6 => scalar_index('\u{ffff}'),
                _ => last,
            };
            fresh_choice(generator, bound.min(last))
        });
        scalar_at(choice)
    }

    /// Draws `None` or `Some` value from `element`, shrinking toward
    /// `None`.
    pub fn option_of<T>(&mut self, element: impl FnOnce(&mut Self) -> T) -> Option<T> {
        if self.bool() {
            Some(element(self))
        } else {
            None
        }
    }

    /// Draws an index into `weights`, each index as often as its weight in
    /// a fresh run, shrinking toward the first index.
    ///
    /// # Panics
    ///
    /// Panics if `weights` is empty or any weight is zero.
    pub fn weighted(&mut self, weights: &[u32]) -> usize {
        assert!(
            !weights.is_empty() && !weights.contains(&0),
            "weighted needs at least one weight, and no zero weight"
        );
        let last = u64::try_from(weights.len() - 1).unwrap_or(u64::MAX);
        let total: u64 = weights.iter().copied().map(u64::from).sum();
        let choice = self.draw_with(last, |generator| {
            let mut point = scale(generator.next_u64(), total - 1);
            for (index, weight) in (0..).zip(weights) {
                let weight = u64::from(*weight);
                if point < weight {
                    return index;
                }
                point -= weight;
            }
            last
        });
        // The choice is at most the last index, so it fits in a usize.
        usize::try_from(choice).unwrap_or(0)
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

/// The first surrogate code point and how many there are: no `char` has
/// one, so scalar values are numbered without them.
const SURROGATES: (u32, u32) = (0xd800, 0x800);

/// The position of `character` among the Unicode scalar values.
fn scalar_index(character: char) -> u64 {
    let code = u32::from(character);
    let (first, count) = SURROGATES;
    u64::from(if code >= first { code - count } else { code })
}

/// The scalar value at `index`, or U+0000 past the last one.
fn scalar_at(index: u64) -> char {
    let (first, count) = SURROGATES;
    let code = u32::try_from(index).unwrap_or(u32::MAX);
    let code = if code >= first {
        code.saturating_add(count)
    } else {
        code
    };
    char::from_u32(code).unwrap_or('\0')
}

/// Reduces a replayed `choice` into `0..=max`.
fn reduce(choice: u64, max: u64) -> u64 {
    if choice <= max {
        choice
    } else {
        choice % (max + 1)
    }
}

/// How many bytes a choice in `0..=max` takes, big-endian: none when `max`
/// is zero.
fn byte_width(max: u64) -> u8 {
    // At most 64 bits, so at most 8 bytes.
    u8::try_from(max.bit_width().div_ceil(8)).unwrap_or(8)
}

/// One fresh draw in this many is a boundary value.
const BOUNDARY_ODDS: u64 = 16;

/// A fresh choice in `0..=max` from one generator output: usually uniform,
/// but one time in [`BOUNDARY_ODDS`] one of the boundaries 0, 1, `max - 1`
/// and `max`, where off-by-one and overflow bugs live. A choice of 0 or
/// `max` is a range's start or end; for the signed draws, 1 is -1 and the
/// top two choices are the extremes.
fn fresh_choice(generator: &mut SplitMix64, max: u64) -> u64 {
    let output = generator.next_u64();
    if !output.is_multiple_of(BOUNDARY_ODDS) {
        return scale(output, max);
    }
    // The bits above the ones tested pick the boundary.
    match (output >> 4) % 4 {
        0 => 0,
        1 => max.min(1),
        2 => max.saturating_sub(1),
        _ => max,
    }
}

/// Scales a full-width `output` onto `0..=max` (the high half of the
/// 128-bit product); `max == u64::MAX` gives the output unchanged.
fn scale(output: u64, max: u64) -> u64 {
    let wide = u128::from(output) * (u128::from(max) + 1);
    u64::try_from(wide >> 64).unwrap_or(max)
}
