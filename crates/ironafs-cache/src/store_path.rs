// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Paths in the state directory, relative to the root the `Disk` driver
//! was opened on (seams and scheduling specification, 5.5, point 2).
//!
//! A name is one component from a restricted character set with one case,
//! never `.` or `..`, so on-disk names are case-independent (ADR 0004,
//! O41, point 4). A path has a bounded number of components. Validation
//! checks only the names: the driver resolves every component relative to
//! its root handle and refuses a link at each one.

use alloc::string::String;
use alloc::vec::Vec;

#[cfg(test)]
mod tests;

/// The most components a [`StorePath`] has, its name included.
pub const MAX_COMPONENTS: usize = 8;

/// The longest [`StoreName`], in bytes.
pub const MAX_NAME_BYTES: usize = 255;

/// One component of a path in the state directory: one to
/// [`MAX_NAME_BYTES`] bytes of `a` to `z`, `0` to `9`, `.`, `_` and `-`,
/// and never `.` or `..`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StoreName(String);

impl StoreName {
    /// The name `name`, or `None` if it is not a valid component.
    #[must_use]
    pub fn new(name: &str) -> Option<Self> {
        let allowed = |byte: u8| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        };
        let valid = !name.is_empty()
            && name.len() <= MAX_NAME_BYTES
            && name != "."
            && name != ".."
            && name.bytes().all(allowed);
        valid.then(|| Self(String::from(name)))
    }

    /// The name as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A directory below the state directory's root: one or more
/// [`StoreName`]s, at most [`MAX_COMPONENTS`] less one, so a file name
/// still fits. The root itself is outside the `Disk` interface (5.5, point
/// 6), so no `StoreDir` names it or its parent.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StoreDir(Vec<StoreName>);

impl StoreDir {
    /// The directory `dir`, its components separated by `/`, or `None` if
    /// a component is not a valid [`StoreName`] or there are too many.
    #[must_use]
    pub fn parse(dir: &str) -> Option<Self> {
        let names = dir
            .split('/')
            .map(StoreName::new)
            .collect::<Option<Vec<_>>>()?;
        (names.len() < MAX_COMPONENTS).then_some(Self(names))
    }

    /// The components, from the root down.
    #[must_use]
    pub fn names(&self) -> &[StoreName] {
        &self.0
    }
}

/// A file in the state directory: a [`StoreDir`] and a [`StoreName`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StorePath {
    dir: StoreDir,
    name: StoreName,
}

impl StorePath {
    /// The file `name` in `dir`, or `None` if the path would have more
    /// than [`MAX_COMPONENTS`] components.
    #[must_use]
    pub fn new(dir: StoreDir, name: StoreName) -> Option<Self> {
        (dir.0.len() < MAX_COMPONENTS).then_some(Self { dir, name })
    }

    /// The directory that holds the file.
    #[must_use]
    pub fn dir(&self) -> &StoreDir {
        &self.dir
    }

    /// The file's name in its directory.
    #[must_use]
    pub fn name(&self) -> &StoreName {
        &self.name
    }
}
