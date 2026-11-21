// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Linux name codec: Linux shows the bytes (ADR 0004, L29, point 2).
//!
//! A name crosses unchanged in both directions; neither side normalises
//! (L29, point 1). Every [`Name`] is a valid Linux path component, so
//! encoding cannot fail. Bytes from the FUSE layer are untrusted input,
//! and [`decode`] refuses any that are not one component.

use crate::{Name, NameError};

#[cfg(test)]
mod tests;

/// The name for the bytes of one component the Linux layer received.
///
/// # Errors
///
/// A [`NameError`] if the bytes are not one path component, as
/// [`Name::new`] states.
pub fn decode(bytes: &[u8]) -> Result<Name, NameError> {
    Name::new(bytes)
}

/// The bytes the Linux layer shows for `name`: its own.
#[must_use]
pub fn encode(name: &Name) -> &[u8] {
    name.as_bytes()
}
