// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Values a secret-bearing message borrows from its input
//! (`docs/specs/xdr.md`, 5.1, point 8).
//!
//! A misframed input can put a key's bytes in any field, so in such a
//! message every opaque, string and integer is held as a reference into the
//! input, which the caller owns and zeroes on drop, and no copy of an input
//! byte outlives a failed decode. Each wrapper's `Debug` prints its type
//! name only, never a value, a length or a count, and none has `Display`:
//! the raw value is reached only through a method named `reveal`, so
//! printing it takes a deliberate call that review sees.
//!
//! Not zeroed, because zeroing needs code this crate may not contain: the
//! words read into registers and stack temporaries during a decode, the
//! lengths of the borrowed slices, already checked against their caps, and
//! a revealed value once the caller holds it.

use core::fmt;

#[cfg(test)]
mod tests;

/// Opaque data or a string borrowed from the input.
#[derive(Clone, Copy)]
pub struct BorrowedBytes<'a>(&'a [u8]);

/// The 4 bytes of an integer or an open enumeration, borrowed from the
/// input.
#[derive(Clone, Copy)]
pub struct BorrowedWord<'a>(&'a [u8; 4]);

/// The 8 bytes of a hyper, borrowed from the input.
#[derive(Clone, Copy)]
pub struct BorrowedHyper<'a>(&'a [u8; 8]);

impl<'a> BorrowedBytes<'a> {
    /// Wraps bytes borrowed from the input.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }

    /// The bytes, where they lie in the input.
    #[must_use]
    pub const fn reveal(&self) -> &'a [u8] {
        self.0
    }
}

impl<'a> BorrowedWord<'a> {
    /// Wraps a word borrowed from the input.
    #[must_use]
    pub const fn new(word: &'a [u8; 4]) -> Self {
        Self(word)
    }

    /// The word as an unsigned integer.
    #[must_use]
    pub const fn reveal_u32(&self) -> u32 {
        u32::from_be_bytes(*self.0)
    }

    /// The word as a signed integer.
    #[must_use]
    pub const fn reveal_i32(&self) -> i32 {
        i32::from_be_bytes(*self.0)
    }

    /// The word's bytes, where they lie in the input.
    #[must_use]
    pub const fn reveal_bytes(&self) -> &'a [u8; 4] {
        self.0
    }
}

impl<'a> BorrowedHyper<'a> {
    /// Wraps a hyper borrowed from the input.
    #[must_use]
    pub const fn new(word: &'a [u8; 8]) -> Self {
        Self(word)
    }

    /// The hyper as an unsigned value.
    #[must_use]
    pub const fn reveal_u64(&self) -> u64 {
        u64::from_be_bytes(*self.0)
    }

    /// The hyper as a signed value.
    #[must_use]
    pub const fn reveal_i64(&self) -> i64 {
        i64::from_be_bytes(*self.0)
    }

    /// The hyper's bytes, where they lie in the input.
    #[must_use]
    pub const fn reveal_bytes(&self) -> &'a [u8; 8] {
        self.0
    }
}

impl fmt::Debug for BorrowedBytes<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BorrowedBytes")
    }
}

impl fmt::Debug for BorrowedWord<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BorrowedWord")
    }
}

impl fmt::Debug for BorrowedHyper<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BorrowedHyper")
    }
}
