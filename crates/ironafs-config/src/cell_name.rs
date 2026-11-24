// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canonical cell name: one rule for every cell name the client takes
//! in, from local files, policy, the classic files, discovery or a mount
//! point (`docs/specs/config-syntax.md`, section 5.7).
//!
//! A [`CellName`] is made only by [`CellName::new`]; its field is private
//! and it has no other constructor, so holding one proves the rule was
//! applied. Equal names are equal byte strings: the rule removes the two
//! differences it allows (ASCII case and one final dot).

use alloc::string::String;
use core::error::Error;
use core::fmt;

#[cfg(test)]
mod tests;

/// The longest cell name, in bytes, after its final dot is removed.
const MAX_NAME_BYTES: usize = 253;

/// The longest label, in bytes.
const MAX_LABEL_BYTES: usize = 63;

/// A cell's name, in its canonical form: ASCII lower case, no final dot, 1
/// to 253 bytes, labels of 1 to 63 bytes of letters, digits and inner
/// hyphens.
///
/// No other way to make one exists:
///
/// ```compile_fail,E0451
/// use ironafs_config::CellName;
///
/// let name = CellName { name: String::from("Not_Canonical") };
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellName {
    name: String,
}

impl CellName {
    /// Applies the cell-name rule to `input`, octets from any source, and
    /// returns the canonical name.
    ///
    /// Upper-case ASCII is lower-cased and one final dot is removed; an
    /// internationalised name is accepted only in its ASCII form, which is
    /// not validated further.
    ///
    /// ```
    /// use ironafs_config::{CellName, CellNameError};
    ///
    /// assert_eq!(CellName::new(b"Example.ORG.")?.as_str(), "example.org");
    /// assert_eq!(
    ///     CellName::new(b"_afs3-vlserver._udp.example.org"),
    ///     Err(CellNameError::Underscore { offset: 0 })
    /// );
    /// # Ok::<(), CellNameError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`CellNameError`] for the first rule `input` breaks, in
    /// the order of the specification's section 5.7: empty, two final
    /// dots, too long, then each label from the first.
    pub fn new(input: &[u8]) -> Result<Self, CellNameError> {
        if input.is_empty() {
            return Err(CellNameError::Empty { offset: 0 });
        }
        if input.ends_with(b"..") {
            // The input ends with two dots, so it holds at least two bytes.
            return Err(CellNameError::TwoFinalDots {
                offset: input.len() - 2,
            });
        }
        let body = input.strip_suffix(b".").unwrap_or(input);
        if body.is_empty() {
            return Err(CellNameError::Empty { offset: 0 });
        }
        if body.len() > MAX_NAME_BYTES {
            return Err(CellNameError::TooLong {
                offset: MAX_NAME_BYTES,
            });
        }
        let mut name = String::with_capacity(body.len());
        let mut start = 0;
        for label in body.split(|octet| *octet == b'.') {
            check_label(label, start)?;
            if !name.is_empty() {
                name.push('.');
            }
            // `check_label` let only ASCII through.
            name.extend(
                label
                    .iter()
                    .map(|octet| char::from(octet.to_ascii_lowercase())),
            );
            // `start` stays within `body`, at most 253 bytes.
            start += label.len() + 1;
        }
        Ok(Self { name })
    }

    /// The name, in its canonical form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for CellName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.name)
    }
}

/// Checks one label that starts at `start` in the input.
fn check_label(label: &[u8], start: usize) -> Result<(), CellNameError> {
    if label.is_empty() {
        return Err(CellNameError::EmptyLabel { offset: start });
    }
    if label.len() > MAX_LABEL_BYTES {
        return Err(CellNameError::LabelTooLong { offset: start });
    }
    let last = label.len() - 1;
    for (index, &octet) in label.iter().enumerate() {
        let offset = start + index;
        match octet {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => {}
            b'-' if index != 0 && index != last => {}
            b'-' => return Err(CellNameError::HyphenAtLabelEdge { offset }),
            b'_' => return Err(CellNameError::Underscore { offset }),
            0x80.. => return Err(CellNameError::NotAscii { offset }),
            _ => return Err(CellNameError::InvalidOctet { offset, octet }),
        }
    }
    Ok(())
}

/// Why a cell name was refused. Each offset is in bytes from the start of
/// the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CellNameError {
    /// Nothing, or only a dot.
    Empty {
        /// Always 0: where the name should start.
        offset: usize,
    },
    /// The input ends with two dots.
    TwoFinalDots {
        /// Where the first of the two final dots is.
        offset: usize,
    },
    /// Longer than 253 bytes after one final dot is removed.
    TooLong {
        /// Always 253: the first byte over the limit.
        offset: usize,
    },
    /// An empty label: a leading dot, or two dots together.
    EmptyLabel {
        /// Where the empty label is.
        offset: usize,
    },
    /// A label longer than 63 bytes.
    LabelTooLong {
        /// Where the label starts.
        offset: usize,
    },
    /// An underscore.
    Underscore {
        /// Where it is.
        offset: usize,
    },
    /// An octet that is not ASCII: an internationalised name is accepted
    /// only in its ASCII form.
    NotAscii {
        /// Where it is.
        offset: usize,
    },
    /// An ASCII octet other than a letter, a digit, a hyphen or a dot.
    InvalidOctet {
        /// Where it is.
        offset: usize,
        /// The octet.
        octet: u8,
    },
    /// A hyphen at the start or end of a label.
    HyphenAtLabelEdge {
        /// Where it is.
        offset: usize,
    },
}

impl fmt::Display for CellNameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Empty { offset } => write!(formatter, "cell name is empty at byte {offset}"),
            Self::TwoFinalDots { offset } => {
                write!(formatter, "cell name ends with two dots at byte {offset}")
            }
            Self::TooLong { offset } => write!(
                formatter,
                "cell name is longer than {MAX_NAME_BYTES} bytes: byte {offset} is over the limit"
            ),
            Self::EmptyLabel { offset } => {
                write!(formatter, "cell name has an empty label at byte {offset}")
            }
            Self::LabelTooLong { offset } => write!(
                formatter,
                "cell name has a label longer than {MAX_LABEL_BYTES} bytes at byte {offset}"
            ),
            Self::Underscore { offset } => {
                write!(formatter, "cell name has an underscore at byte {offset}")
            }
            Self::NotAscii { offset } => write!(
                formatter,
                "cell name has a non-ASCII byte at byte {offset}; write an international name in its ASCII form"
            ),
            Self::InvalidOctet { offset, octet } => write!(
                formatter,
                "cell name has byte 0x{octet:02x} at byte {offset}; only letters, digits, hyphens and dots are allowed"
            ),
            Self::HyphenAtLabelEdge { offset } => write!(
                formatter,
                "cell name has a hyphen at the start or end of a label at byte {offset}"
            ),
        }
    }
}

impl Error for CellNameError {}
