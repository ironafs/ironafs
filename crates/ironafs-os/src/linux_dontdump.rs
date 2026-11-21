// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Site `linux-dontdump` (ADR 0005, row 12 and sheet 12): a secret buffer
//! whose pages are excluded from core dumps with `madvise(MADV_DONTDUMP)`.
//!
//! The memory is one boxed 64 KiB block that is also 64 KiB aligned,
//! allocated in safe Rust (sheet 12 says a boxed slice of such blocks; a
//! buffer is exactly one, so the box holds the block itself and its bytes
//! are one slice without any unsafe flattening), so its start and length are multiples
//! of every page size the allowlisted Linux targets use (4, 16 and 64 KiB),
//! as `madvise(2)` requires ("addr must be page-aligned"), without asking
//! the system for the page size and without an unsafe allocation. The
//! advice covers the whole allocation and changes no content. A failed
//! exclusion fails closed: the buffer is not handed out.
//!
//! **Limits.** It is an additional layer, never the only one: the process's
//! dump controls (`unix-nodump`) and the marker tests stay (ADR 0004, O25,
//! points 2 and 3). The advice is not undone when the buffer is freed, so
//! later non-secret data in the same pages may also be left out of a dump.
//! Each buffer costs 64 KiB, whatever it holds: callers budget concurrent
//! secret buffers by that full size (owner, 2026-11-20). Each advised range
//! is a mapping of its own to the kernel, so very many live buffers count
//! against the process's limit on mappings.

mod abi;

use core::ffi::{c_int, c_void};
use std::fmt;
use std::io;

use crate::dump::DumpError;
use crate::mem_wipe::wipe;
use abi::MADV_DONTDUMP;

#[cfg(test)]
mod tests;

/// The size and alignment of a secret buffer: 64 KiB.
const BLOCK: usize = 64 * 1024;

/// One block: 64 KiB in size and in alignment.
#[repr(C, align(65536))]
struct Block([u8; BLOCK]);

/// A 64 KiB buffer for secrets, excluded from core dumps and zeroed with
/// [`wipe`](crate::wipe) when dropped.
///
/// Keep key material in it and pass it by reference: moves of the bytes
/// out of it are not covered (see `wipe`'s limits).
///
/// ```
/// let mut buffer = ironafs_os::SecretBuffer::new()?;
/// assert_eq!(buffer.as_mut_slice().len(), ironafs_os::SecretBuffer::SIZE);
/// # Ok::<(), ironafs_os::DumpError>(())
/// ```
pub struct SecretBuffer {
    block: Box<Block>,
}

impl SecretBuffer {
    /// The buffer's size in bytes, which is also what it costs.
    pub const SIZE: usize = BLOCK;

    /// Allocates a zeroed buffer and excludes it from core dumps.
    ///
    /// # Errors
    ///
    /// Returns [`DumpError::Exclude`] with the system's error if the
    /// exclusion fails; no buffer is handed out then.
    pub fn new() -> Result<Self, DumpError> {
        exclude_with(dont_dump)
    }

    /// The buffer's bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.block.0
    }

    /// The buffer's bytes, to write.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.block.0
    }

    /// Zeroes every byte with `wipe`.
    fn clear(&mut self) {
        wipe(self.as_mut_slice());
    }
}

impl Drop for SecretBuffer {
    fn drop(&mut self) {
        self.clear();
    }
}

impl fmt::Debug for SecretBuffer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretBuffer")
            .field("size", &Self::SIZE)
            .finish_non_exhaustive()
    }
}

/// Allocates one zeroed block and passes all of it to `exclude`; hands it
/// out as a buffer only if `exclude` succeeds.
fn exclude_with(
    exclude: impl FnOnce(&mut [u8]) -> io::Result<()>,
) -> Result<SecretBuffer, DumpError> {
    #[expect(
        clippy::large_stack_arrays,
        reason = "the zeroed block may pass through the stack once on its way to the heap; it \
                  holds no secret yet, and the safe alternatives all build it the same way"
    )]
    let block = Box::new(Block([0; BLOCK]));
    let mut buffer = SecretBuffer { block };
    exclude(buffer.as_mut_slice()).map_err(DumpError::Exclude)?;
    Ok(buffer)
}

/// Excludes `memory`, a whole 64 KiB block, from core dumps.
fn dont_dump(memory: &mut [u8]) -> io::Result<()> {
    let address: *mut c_void = memory.as_mut_ptr().cast();
    let length: usize = memory.len();
    let advice: c_int = MADV_DONTDUMP;
    #[expect(unsafe_code, reason = "linux-dontdump")]
    // SAFETY: `madvise(2)` with `MADV_DONTDUMP` marks the pages of the range
    // to be left out of a core dump; it reads and writes no byte of them,
    // so it cannot change memory Rust sees. `address` and `length` come
    // from `memory`, an exclusive borrow of a whole 64 KiB block
    // (`exclude_with` passes nothing else), live for the call:
    // the start is 64 KiB aligned and the length a multiple of 64 KiB, so
    // the range is page-aligned for every page size of the allowlisted
    // targets and lies inside mapped memory this process owns. A bad range
    // or advice is answered with -1 and errno (EINVAL, ENOMEM), never
    // undefined behaviour. Nothing keeps the pointer afterwards. The
    // advice is per mapping and the call is safe from any thread. The
    // declaration is in `abi`, proved by the ABI check.
    let status = unsafe { abi::madvise(address, length, advice) };
    if status != 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
