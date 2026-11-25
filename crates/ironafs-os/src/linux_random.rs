// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Site `linux-random` (ADR 0005, row 8 and sheet 8): the kernel's random
//! source, through the raw `getrandom` system call.
//!
//! glibc's own `getrandom` wrapper exists only from glibc 2.25, and a build
//! that runs on glibc 2.17 must not need it, so the call goes through
//! `syscall(SYS_getrandom, ...)`. Flags are 0: the call draws from the
//! urandom source and, until the kernel's pool is initialised, blocks
//! (`getrandom(2)`); this site sets no time limit on that wait. A caller
//! that must stay responsive early in boot makes its first fill on a
//! separate worker (ADR 0005, sheet 8, "What the daemon does").
//!
//! A kernel without the call answers `ENOSYS`, and a sandbox's filter can
//! answer `ENOSYS` or `EPERM`; both are [`RandomError::Unavailable`], with
//! the real errno. There is no `/dev/urandom` fallback: it can answer
//! before the pool is initialised.

mod abi;
mod number;

use core::ffi::c_long;
use std::io;

use crate::random::{Piece, RandomError, fill_with};
use abi::{EINTR, ENOSYS, EPERM};
use number::SYS_getrandom;

#[cfg(test)]
mod tests;

/// The flags passed: none, so the call blocks until the pool is
/// initialised and draws from the urandom source. `getrandom`'s parameter
/// is an `unsigned int`; it is passed as a `long` like every argument of
/// `syscall` (see the declaration in `abi`).
const FLAGS: c_long = 0;

/// Fills `buffer` with random bytes from the kernel.
///
/// After an error the buffer's content is unspecified: never treat it as
/// filled. An empty buffer makes no call.
///
/// # Errors
///
/// Returns [`RandomError::Unavailable`] if the kernel lacks the call or a
/// sandbox filters it (`ENOSYS`, `EPERM`), [`RandomError::Failed`] for any
/// other error, and [`RandomError::NoBytes`] or [`RandomError::OverLong`]
/// if the call reports zero bytes or more than it was asked for. A call
/// interrupted by a signal is retried.
///
/// ```
/// let mut seed = [0_u8; 32];
/// ironafs_os::fill_random(&mut seed)?;
/// # Ok::<(), ironafs_os::RandomError>(())
/// ```
pub fn fill_random(buffer: &mut [u8]) -> Result<(), RandomError> {
    fill_with(buffer, getrandom)
}

/// One `getrandom` call over `piece`, at most `MAX_PIECE` bytes, its
/// result classified.
fn getrandom(piece: &mut [u8]) -> Piece {
    let Some([address, length, flags, unused3, unused4, unused5]) = arguments(piece) else {
        return Piece::Failed(io::Error::other("a getrandom piece longer than a long"));
    };
    let number: c_long = SYS_getrandom;
    #[expect(unsafe_code, reason = "linux-random")]
    // SAFETY: `getrandom(2)` writes at most `buflen` bytes at `buf` and
    // reads nothing of ours. `address` is the exposed address of `piece`,
    // an exclusive borrow of the caller's buffer that lives, unused, for
    // the whole call, and `length` is its length, at most `MAX_PIECE`
    // (256), which `fill_with` guarantees; so the kernel writes only memory
    // this call may write, and nothing keeps the address afterwards. The
    // address was exposed, so the bytes the kernel writes are this
    // buffer's to Rust. `syscall` is variadic: every argument after the
    // number is a `long`, six of them, the last three zero (the reason is
    // at the declaration, in `abi`). The call shares no state with other
    // threads. The declaration is in `abi`, and `SYS_getrandom` per
    // architecture in `number`; the ABI check proves both.
    let returned =
        unsafe { abi::syscall(number, address, length, flags, unused3, unused4, unused5) };
    // At once, with no call in between, on the thread that made it.
    let error = (returned < 0).then(io::Error::last_os_error);
    classify(returned, error)
}

/// The six variadic arguments of one `getrandom` call over `piece`, each a
/// `long` (see the declaration in `abi`): the buffer's exposed address,
/// its length, the flags (0) and three zeros. `None` if the length does not
/// fit a `long`, which a piece of at most `MAX_PIECE` bytes always does.
fn arguments(piece: &mut [u8]) -> Option<[c_long; 6]> {
    let address = c_long::from_ne_bytes(piece.as_mut_ptr().expose_provenance().to_ne_bytes());
    let length = c_long::try_from(piece.len()).ok()?;
    Some([address, length, FLAGS, 0, 0, 0])
}

/// What a `getrandom` call that returned `returned` did; `error` is the
/// errno it set, read when it returned a negative value.
fn classify(returned: c_long, error: Option<io::Error>) -> Piece {
    if let Ok(count) = usize::try_from(returned) {
        return Piece::Filled(count);
    }
    let error = error.unwrap_or_else(|| io::Error::other("getrandom failed with no error"));
    match error.raw_os_error() {
        Some(EINTR) => Piece::Interrupted,
        Some(ENOSYS | EPERM) => Piece::Unavailable(error),
        _ => Piece::Failed(error),
    }
}
