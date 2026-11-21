// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cryptographic random source and the production [`Rng`] built on it
//! (ADR 0004, C4, point 7; C5, point 1).
//!
//! This crate is pure and cannot call the operating system, so the source
//! itself is supplied when the client is composed: the operating system's
//! own source, reached through the safe sites of the operating-system
//! bindings. Entropy failure fails closed. There is no fallback source: a
//! source that fails at startup refuses the start ([`CryptoRng::start`]),
//! and one that fails later fails the operation that asked
//! ([`CryptoRng`]'s [`Rng::fill`]).

use alloc::boxed::Box;
use core::error::Error;
use core::fmt;

use ironafs_time::{Rng, RngError};

#[cfg(test)]
mod tests;

/// How many bytes [`CryptoRng::start`] draws to prove the source works.
///
/// The bytes are discarded. Any non-zero length makes a source that blocks
/// until it is ready (as the Linux system call does early in boot) block
/// here, before the process announces that it has started.
const STARTUP_PROBE_LEN: usize = 16;

/// A source of cryptographically secure random bytes: in production the
/// operating system's own source, supplied when the client is composed.
///
/// Only an `EntropySource` becomes the production [`Rng`]. A seeded
/// generator implements [`Rng`] for tests and the simulator and never
/// implements this trait, so it cannot be passed to [`CryptoRng::start`];
/// the test-only crates that hold such generators do not depend on this
/// crate, and `cargo xtask check-deps` keeps every shipped crate off them.
pub trait EntropySource: Send + Sync {
    /// Fills all of `bytes` with random bytes, or fails.
    ///
    /// A source that can return fewer bytes than asked loops until the
    /// buffer is full; it never reports a partly filled buffer as filled.
    ///
    /// # Errors
    ///
    /// Returns an [`EntropyError`] if the source cannot fill all of
    /// `bytes`. The contents of `bytes` are then unspecified and must not
    /// be used.
    fn fill(&self, bytes: &mut [u8]) -> Result<(), EntropyError>;
}

/// The production [`Rng`]: the cryptographic random source, over an
/// [`EntropySource`] injected when the client is composed.
///
/// It is built only from an [`EntropySource`], never from another [`Rng`],
/// so a seeded generator cannot take its place:
///
/// ```compile_fail,E0277
/// use ironafs_crypto::CryptoRng;
/// use ironafs_time::{Rng, RngError};
///
/// /// A seeded generator, as the simulator has.
/// struct Seeded;
///
/// impl Rng for Seeded {
///     fn fill(&self, bytes: &mut [u8]) -> Result<(), RngError> {
///         bytes.fill(7);
///         Ok(())
///     }
/// }
///
/// let _ = CryptoRng::start(Seeded);
/// ```
pub struct CryptoRng {
    source: Box<dyn EntropySource>,
}

impl CryptoRng {
    /// Proves that `source` supplies bytes and returns the [`Rng`] over it.
    ///
    /// It draws one probe from `source` and discards it. The process calls
    /// this before it announces that it has started, so a source that is
    /// missing, filtered or failing refuses the start. A source that blocks
    /// until it is ready blocks this call for as long; the caller bounds
    /// that wait.
    ///
    /// ```
    /// use ironafs_crypto::{CryptoRng, EntropyError, EntropySource};
    ///
    /// /// A source whose system call is missing.
    /// struct Missing;
    ///
    /// impl EntropySource for Missing {
    ///     fn fill(&self, _bytes: &mut [u8]) -> Result<(), EntropyError> {
    ///         Err(EntropyError::Unavailable { code: 38 })
    ///     }
    /// }
    ///
    /// let refused = CryptoRng::start(Missing).map(|_| ());
    /// assert_eq!(refused, Err(EntropyError::Unavailable { code: 38 }));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the source's [`EntropyError`], unchanged, if the probe
    /// fails.
    pub fn start(source: impl EntropySource + 'static) -> Result<Self, EntropyError> {
        let mut probe = [0_u8; STARTUP_PROBE_LEN];
        source.fill(&mut probe)?;
        Ok(Self {
            source: Box::new(source),
        })
    }
}

impl Rng for CryptoRng {
    /// Fills all of `bytes` from the source. An empty buffer succeeds
    /// without calling the source.
    ///
    /// # Errors
    ///
    /// Returns an [`RngError`] if the source fails. Nothing else is tried.
    fn fill(&self, bytes: &mut [u8]) -> Result<(), RngError> {
        if bytes.is_empty() {
            return Ok(());
        }
        self.source.fill(bytes).map_err(|_| RngError::new())
    }
}

impl fmt::Debug for CryptoRng {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("CryptoRng").finish_non_exhaustive()
    }
}

/// Why the random source could not supply bytes.
///
/// The codes are the operating system's own: an `errno` value on Linux and
/// macOS, the status `SecRandomCopyBytes` returned, or an `NTSTATUS` as a
/// signed 32-bit integer on Windows. They reach the report unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EntropyError {
    /// The source is missing, or a sandbox filter refused it. A filter can
    /// answer with the same code a missing call gives, so the two are not
    /// told apart.
    Unavailable {
        /// The operating system's error code.
        code: i32,
    },
    /// The source reported a failure.
    Failed {
        /// The operating system's error code.
        code: i32,
    },
    /// The source returned no bytes, or more than were asked for.
    BadCount,
}

impl fmt::Display for EntropyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { code } => write!(
                formatter,
                "random source unavailable or filtered (os error {code})"
            ),
            Self::Failed { code } => write!(formatter, "random source failed (os error {code})"),
            Self::BadCount => {
                formatter.write_str("random source returned a byte count it was not asked for")
            }
        }
    }
}

impl Error for EntropyError {}
