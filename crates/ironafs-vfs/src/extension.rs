// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Extension arms (ADR 0004, N41): a bounded, tagged, length-delimited
//! list of extensions, each marked critical or not.
//!
//! The adapter contract uses the same shapes as control IPC, under its
//! exact-version rule: it needs room to grow, not compatibility (N41,
//! point 7). The list is bounded in count and in bytes when it is built,
//! so a list received from a layer cannot be a memory attack (N41, "Why";
//! coding standards rule 3). A receiver refuses a request whole if it
//! holds an unknown critical extension ([`Extensions::first_unknown_critical`],
//! N41, point 4), and names the unknown non-critical ones it ignored
//! ([`Extensions::unknown_tags`], N41, point 9).

use alloc::vec::Vec;

#[cfg(test)]
mod tests;

/// The most extensions one list holds.
pub const MAX_EXTENSIONS: usize = 16;

/// The most payload bytes one list holds, over all its extensions; so
/// also the most one extension holds.
pub const MAX_EXTENSION_BYTES: usize = 4096;

/// One extension: a tag, whether it is critical, and its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extension {
    tag: u32,
    critical: bool,
    bytes: Vec<u8>,
}

impl Extension {
    /// An extension with `tag`, critical or not, holding `bytes`.
    ///
    /// # Errors
    ///
    /// [`ExtensionError::TooLarge`] if `bytes` is longer than
    /// [`MAX_EXTENSION_BYTES`].
    pub fn new(tag: u32, critical: bool, bytes: Vec<u8>) -> Result<Self, ExtensionError> {
        if bytes.len() > MAX_EXTENSION_BYTES {
            return Err(ExtensionError::TooLarge);
        }
        Ok(Self {
            tag,
            critical,
            bytes,
        })
    }

    /// The extension's tag.
    #[must_use]
    pub const fn tag(&self) -> u32 {
        self.tag
    }

    /// Whether a receiver that does not know the tag must refuse the
    /// whole request (N41, points 2 and 4).
    #[must_use]
    pub const fn critical(&self) -> bool {
        self.critical
    }

    /// The extension's payload.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// A bounded list of extensions, each tag at most once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extensions {
    items: Vec<Extension>,
    bytes: usize,
}

impl Extensions {
    /// An empty list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            items: Vec::new(),
            bytes: 0,
        }
    }

    /// Adds `extension` to the list.
    ///
    /// # Errors
    ///
    /// [`ExtensionError::TooMany`] if the list already holds
    /// [`MAX_EXTENSIONS`]; [`ExtensionError::TooLarge`] if the payloads
    /// would pass [`MAX_EXTENSION_BYTES`] together;
    /// [`ExtensionError::Duplicate`] if the tag is already in the list.
    /// The list is unchanged on an error.
    pub fn push(&mut self, extension: Extension) -> Result<(), ExtensionError> {
        if self.items.len() >= MAX_EXTENSIONS {
            return Err(ExtensionError::TooMany);
        }
        let bytes = self
            .bytes
            .checked_add(extension.bytes.len())
            .filter(|total| *total <= MAX_EXTENSION_BYTES)
            .ok_or(ExtensionError::TooLarge)?;
        if self.items.iter().any(|held| held.tag == extension.tag) {
            return Err(ExtensionError::Duplicate);
        }
        self.items.push(extension);
        self.bytes = bytes;
        Ok(())
    }

    /// The extensions, in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = &Extension> {
        self.items.iter()
    }

    /// The tag of the first critical extension not in `known`, if any: a
    /// request holding one is refused whole, as not supported, and never
    /// applied in part (N41, point 4).
    #[must_use]
    pub fn first_unknown_critical(&self, known: &[u32]) -> Option<u32> {
        self.items
            .iter()
            .find(|extension| extension.critical && !known.contains(&extension.tag))
            .map(Extension::tag)
    }

    /// The tags not in `known`, critical or not, in list order: what a
    /// receiver ignored and names as ignored (N41, points 3 and 9).
    pub fn unknown_tags<'a>(&'a self, known: &'a [u32]) -> impl Iterator<Item = u32> + 'a {
        self.items
            .iter()
            .map(Extension::tag)
            .filter(move |tag| !known.contains(tag))
    }
}

/// Why an extension or a list was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExtensionError {
    /// The list already holds its most extensions.
    TooMany,
    /// The payload passes the list's byte cap.
    TooLarge,
    /// The list already holds an extension with this tag.
    Duplicate,
}

impl core::fmt::Display for ExtensionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::TooMany => "too many extensions",
            Self::TooLarge => "the extensions' payload is too large",
            Self::Duplicate => "an extension's tag appears twice",
        })
    }
}

impl core::error::Error for ExtensionError {}
