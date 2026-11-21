// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The operating system's random source: the fill loop every random site
//! shares, and its error (ADR 0005, sheets 8 to 10).
//!
//! A fill writes the caller's whole buffer or returns an error. After an
//! error the buffer's content is unspecified and is never to be treated
//! as filled. There is no fallback to another source, a file or a
//! time-seeded generator: entropy failure fails closed, and
//! `ironafs-crypto` turns the error into a refusal (ADR 0004, C5, point 1).

use std::error::Error;
use std::fmt;
use std::io;

#[cfg(test)]
mod tests;

/// The most bytes asked of the operating system in one call. Linux's
/// `getrandom` returns every byte of a request this small once its pool is
/// initialised (getrandom(2)); the loop still accepts a short count.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(
        dead_code,
        reason = "used by the Linux sites; the other systems' sites join with their parts"
    )
)]
pub(crate) const MAX_PIECE: usize = 256;

/// What one call of the operating system's source did, as its site
/// classifies the call's result.
#[derive(Debug)]
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(
        dead_code,
        reason = "used by the Linux sites; the other systems' sites join with their parts"
    )
)]
pub(crate) enum Piece {
    /// It reports this many bytes written at the start of the piece.
    Filled(usize),
    /// A signal interrupted it before it wrote anything; it is retried.
    Interrupted,
    /// The source is absent or a sandbox filters it.
    Unavailable(io::Error),
    /// Any other failure.
    Failed(io::Error),
}

/// Why the random source did not fill the buffer.
#[derive(Debug)]
pub enum RandomError {
    /// The source is absent (the kernel lacks the call) or filtered by a
    /// sandbox; the error carries the real errno, which alone does not tell
    /// the two apart, since a filter can return either (ADR 0005, sheet 8).
    Unavailable(io::Error),
    /// The source failed in any other way.
    Failed(io::Error),
    /// A call reported no bytes written.
    NoBytes,
    /// A call reported more bytes than it was asked for.
    OverLong { asked: usize, returned: usize },
}

impl fmt::Display for RandomError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(error) => {
                write!(formatter, "random source unavailable or filtered: {error}")
            }
            Self::Failed(error) => write!(formatter, "random source failed: {error}"),
            Self::NoBytes => write!(formatter, "random source returned no bytes"),
            Self::OverLong { asked, returned } => write!(
                formatter,
                "random source returned {returned} bytes for a request of {asked}"
            ),
        }
    }
}

impl Error for RandomError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unavailable(error) | Self::Failed(error) => Some(error),
            Self::NoBytes | Self::OverLong { .. } => None,
        }
    }
}

/// Fills `buffer` through `call`, at most `MAX_PIECE` bytes a call, until
/// every byte is written. An empty buffer makes no call. An interrupted
/// call is retried; every other failure, a call that reports no bytes and
/// one that reports more than it was asked end the fill with an error.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(
        dead_code,
        reason = "used by the Linux sites; the other systems' sites join with their parts"
    )
)]
pub(crate) fn fill_with(
    buffer: &mut [u8],
    mut call: impl FnMut(&mut [u8]) -> Piece,
) -> Result<(), RandomError> {
    let mut rest = buffer;
    while !rest.is_empty() {
        let asked = rest.len().min(MAX_PIECE);
        let (piece, _) = rest.split_at_mut(asked);
        let returned = match call(piece) {
            Piece::Filled(0) => return Err(RandomError::NoBytes),
            Piece::Filled(returned) => returned,
            Piece::Interrupted => continue,
            Piece::Unavailable(error) => return Err(RandomError::Unavailable(error)),
            Piece::Failed(error) => return Err(RandomError::Failed(error)),
        };
        let over_long = RandomError::OverLong { asked, returned };
        if returned > asked {
            return Err(over_long);
        }
        rest = std::mem::take(&mut rest)
            .get_mut(returned..)
            .ok_or(over_long)?;
    }
    Ok(())
}
