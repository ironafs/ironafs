// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Variants of a known wire message for decoder tests: single-bit flips,
//! integers near a known one, and fields set to all zeros or all ones.
//!
//! A decoder's tests start from a known message, such as a golden
//! encoding, and check that every variant is either decoded or refused
//! with an error, never a panic, and that a refused variant is refused
//! for the right reason. The variants are lists, so a test can run them
//! all, or an [`Exhaustive`](crate::Exhaustive) sweep or a
//! [`Property`](crate::Property) can draw from them with
//! [`Source::pick`](crate::Source::pick) or an index.
//!
//! ```
//! use ironafs_test_support::wire;
//!
//! // A planted decoder of a length-prefixed string: a big-endian `u32`
//! // length, then that many bytes.
//! fn decode(bytes: &[u8]) -> Result<&[u8], &'static str> {
//!     let (prefix, rest) = bytes.split_first_chunk::<4>().ok_or("short header")?;
//!     let length = usize::try_from(u32::from_be_bytes(*prefix)).map_err(|_| "length")?;
//!     rest.get(..length).ok_or("length past the end")
//! }
//!
//! let golden = [0, 0, 0, 2, b'h', b'i'];
//! for variant in wire::single_bit_flips(&golden).chain(wire::field_edges(&golden, 4)) {
//!     let _ = decode(&variant);
//! }
//! for length in wire::near(2_u32, 2).into_iter().chain(wire::edges::<u32>()) {
//!     let mut message = golden.to_vec();
//!     message.splice(..4, length.to_be_bytes());
//!     assert_eq!(decode(&message).is_ok(), length <= 2);
//! }
//! ```

#[cfg(test)]
mod tests;

/// A fixed-width integer, as wire formats carry them: implemented for
/// `u8` to `u64` and `i8` to `i64`.
pub trait FixedWidth: Copy + Into<i128> + TryFrom<i128> {
    /// The smallest value.
    const MIN: Self;
    /// The largest value.
    const MAX: Self;
    /// The value with no bit set.
    const ZERO: Self;
    /// The value with every bit set: the largest value of an unsigned
    /// type, and -1 of a signed one.
    const ALL_ONES: Self;
}

macro_rules! fixed_width {
    ($($type:ty => $ones:expr),* $(,)?) => {
        $(impl FixedWidth for $type {
            const MIN: Self = <$type>::MIN;
            const MAX: Self = <$type>::MAX;
            const ZERO: Self = 0;
            const ALL_ONES: Self = $ones;
        })*
    };
}

fixed_width!(
    u8 => u8::MAX,
    u16 => u16::MAX,
    u32 => u32::MAX,
    u64 => u64::MAX,
    i8 => -1,
    i16 => -1,
    i32 => -1,
    i64 => -1,
);

/// `message` with one bit flipped. Bits are numbered in network order:
/// bit 0 is the most significant bit of the first byte, bit 8 the most
/// significant bit of the second.
///
/// # Panics
///
/// Panics if `bit` is not below eight times the message's length.
#[must_use]
#[expect(
    clippy::panic,
    reason = "rule 1 covers untrusted input; a bit past the message is a mistake in the test, reported as `assert!` would"
)]
pub fn flip_bit(message: &[u8], bit: usize) -> Vec<u8> {
    let mut flipped = message.to_vec();
    let Some(byte) = flipped.get_mut(bit / 8) else {
        panic!(
            "flip_bit: bit {bit} is past a message of {} bytes",
            message.len()
        );
    };
    *byte ^= 0x80 >> (bit % 8);
    flipped
}

/// Every single-bit flip of `message`, bit 0 first, as [`flip_bit`]
/// numbers them: eight variants a byte.
pub fn single_bit_flips(message: &[u8]) -> impl Iterator<Item = Vec<u8>> {
    let bits = message.len().saturating_mul(8);
    (0..bits).map(move |bit| flip_bit(message, bit))
}

/// The values within `distance` of `known`, in ascending order, `known`
/// included, stopping at the type's limits: the values just below and
/// just above a cap, a length or a count.
#[must_use]
pub fn near<T: FixedWidth>(known: T, distance: u32) -> Vec<T> {
    let known: i128 = known.into();
    let distance = i128::from(distance);
    let lowest = (known - distance).max(T::MIN.into());
    let highest = (known + distance).min(T::MAX.into());
    // Every value between the type's limits converts back.
    (lowest..=highest)
        .filter_map(|value| T::try_from(value).ok())
        .collect()
}

/// A type's all-zero and all-ones values, in that order.
#[must_use]
pub fn edges<T: FixedWidth>() -> [T; 2] {
    [T::ZERO, T::ALL_ONES]
}

/// `message` with each field of `width` bytes set to all zeros, then to
/// all ones, one field at a time from the first. The fields start at
/// multiples of `width` (4 for XDR's words); a shorter last field is set
/// too.
///
/// # Panics
///
/// Panics if `width` is zero.
pub fn field_edges(message: &[u8], width: usize) -> impl Iterator<Item = Vec<u8>> {
    assert!(
        width > 0,
        "field_edges needs a field width of at least one byte"
    );
    let starts = (0..message.len()).step_by(width);
    starts.flat_map(move |start| {
        [0x00, 0xff].map(|fill| {
            let mut variant = message.to_vec();
            variant
                .iter_mut()
                .skip(start)
                .take(width)
                .for_each(|byte| *byte = fill);
            variant
        })
    })
}
