// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stub.

/// Stub.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellName(alloc::string::String);

impl CellName {
    /// Stub.
    ///
    /// # Errors
    ///
    /// Stub.
    pub fn new(input: &[u8]) -> Result<Self, CellNameError> {
        let _ = input;
        Err(CellNameError::Empty)
    }

    /// Stub.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stub.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CellNameError {
    /// Stub.
    Empty,
}

impl core::fmt::Display for CellNameError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("stub")
    }
}

impl core::error::Error for CellNameError {}
