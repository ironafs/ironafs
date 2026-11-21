// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The codec's one closed error set (`docs/specs/xdr.md`, section 7).
//!
//! An [`Error`] holds its kind, a field or cap name, a constant of the
//! specification (a cap, a bound or a memory budget) and, outside a
//! secret-bearing message, where it happened. It never holds a value read
//! from the input: no byte, no invalid word and no claimed length or count.
//! The decoder builds an error from a secret-bearing message without a
//! position at all, and the entry points strip one from any error a
//! secret-bearing message's decoder passes on from a decode of its own,
//! so no reporting boundary needs a redaction rule of its own.

use core::fmt;

#[cfg(test)]
mod tests;

/// What went wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The message is longer than its cap.
    MessageTooLarge,
    /// The input ends inside an item.
    Truncated,
    /// A length or count is above the declared bound.
    LengthOverBound,
    /// A length, count or running total is above its cap, or a read or an
    /// item passes an enclosing array's limit.
    CapExceeded,
    /// A padding byte is not zero.
    NonZeroPadding,
    /// A boolean is neither 0 nor 1.
    InvalidBool,
    /// A closed enumeration's value is not declared.
    UnknownEnumValue,
    /// A union's discriminant has no arm and there is no default arm.
    UnknownDiscriminant,
    /// Bytes remain after the message.
    TrailingBytes,
    /// A reservation would carry the requested bytes past the message's
    /// memory budget.
    BudgetExceeded,
    /// A fallible reservation failed.
    AllocationFailed,
    /// A length or count does not fit in 32 bits.
    LengthNotRepresentable,
}

impl ErrorKind {
    /// Every kind, in the order of the specification's table.
    pub const ALL: [Self; 12] = [
        Self::MessageTooLarge,
        Self::Truncated,
        Self::LengthOverBound,
        Self::CapExceeded,
        Self::NonZeroPadding,
        Self::InvalidBool,
        Self::UnknownEnumValue,
        Self::UnknownDiscriminant,
        Self::TrailingBytes,
        Self::BudgetExceeded,
        Self::AllocationFailed,
        Self::LengthNotRepresentable,
    ];

    /// The kind's display text.
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::MessageTooLarge => "message too large",
            Self::Truncated => "input truncated",
            Self::LengthOverBound => "length over declared bound",
            Self::CapExceeded => "cap exceeded",
            Self::NonZeroPadding => "non-zero padding",
            Self::InvalidBool => "invalid boolean",
            Self::UnknownEnumValue => "unknown enumeration value",
            Self::UnknownDiscriminant => "unknown union discriminant",
            Self::TrailingBytes => "trailing bytes",
            Self::BudgetExceeded => "memory budget exceeded",
            Self::AllocationFailed => "allocation failed",
            Self::LengthNotRepresentable => "length not representable in 32 bits",
        }
    }
}

/// Where a decode error happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Position {
    /// Byte offset from the start of the input.
    offset: usize,
    /// The input's length, the bytes available or the trailing count,
    /// as the kind defines; all come from the buffer's size, not its
    /// content.
    extent: Option<usize>,
}

/// A codec error: its kind, and only constants and positions besides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Error {
    kind: ErrorKind,
    name: Option<&'static str>,
    limit: Option<u64>,
    position: Option<Position>,
}

impl Error {
    pub(crate) const fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            name: None,
            limit: None,
            position: None,
        }
    }

    pub(crate) const fn named(self, name: &'static str) -> Self {
        Self {
            name: Some(name),
            ..self
        }
    }

    pub(crate) const fn limit(self, limit: u64) -> Self {
        Self {
            limit: Some(limit),
            ..self
        }
    }

    /// Adds where the error happened. The decoder never calls this for a
    /// secret-bearing message (`docs/specs/xdr.md`, section 7, point 2).
    pub(crate) const fn at(self, offset: usize, extent: Option<usize>) -> Self {
        Self {
            position: Some(Position { offset, extent }),
            ..self
        }
    }

    /// The error without where it happened, for a secret-bearing message.
    pub(crate) const fn unlocated(self) -> Self {
        Self {
            position: None,
            ..self
        }
    }

    /// The error's kind.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The field or cap name the error concerns, a constant of the codec.
    #[must_use]
    pub const fn name(&self) -> Option<&'static str> {
        self.name
    }

    /// The specification's constant the error concerns: a cap, a declared
    /// bound or a memory budget.
    #[must_use]
    pub const fn limit_value(&self) -> Option<u64> {
        self.limit
    }

    /// The byte offset, from the start of the input, of the item that
    /// failed. `None` for an encode error and for any error from a
    /// secret-bearing message.
    #[must_use]
    pub const fn offset(&self) -> Option<usize> {
        match self.position {
            Some(position) => Some(position.offset),
            None => None,
        }
    }

    /// For `MessageTooLarge`, the input's length; for `Truncated`, the
    /// bytes available from the offset; for `TrailingBytes`, their count.
    /// `None` otherwise, and for any error from a secret-bearing message.
    #[must_use]
    pub const fn extent(&self) -> Option<usize> {
        match self.position {
            Some(position) => position.extent,
            None => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.kind.text())?;
        if let Some(name) = self.name {
            write!(formatter, ": {name}")?;
        }
        if let Some(limit) = self.limit {
            write!(formatter, ", limit {limit}")?;
        }
        if let Some(position) = self.position {
            write!(formatter, ", at offset {}", position.offset)?;
            if let Some(extent) = position.extent {
                write!(formatter, ", {extent} bytes there")?;
            }
        }
        Ok(())
    }
}

impl core::error::Error for Error {}
