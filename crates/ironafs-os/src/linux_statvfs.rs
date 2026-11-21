// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Site `linux-statvfs` (ADR 0005, row 22 and sheet 22): the bytes free
//! to an unprivileged user on the filesystem of a borrowed directory
//! descriptor, through `fstatvfs`.
//!
//! It is the chunk store's own directory that is measured: a descriptor of
//! the state directory can be on another filesystem. So the descriptor is
//! first checked to be a directory's (through `std`, on a duplicate), and
//! anything else is refused before the call. The result is `f_bavail ×
//! f_frsize`, checked for overflow. A call interrupted by a signal is
//! retried. What a failed query means (clean-cache admission suspended
//! until one succeeds, ADR 0004, C27, point 3) is the caller's.

mod abi;

use core::ffi::c_int;
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};

use crate::space::{SpaceError, available};
use abi::statvfs;

#[cfg(test)]
mod tests;

// The size glibc 2.28 and 2.41 and musl give the struct on these targets
// (measured in the guest; the ABI check proves it on each).
const _: () = assert!(size_of::<statvfs>() == 112 && align_of::<statvfs>() == 8);

/// The bytes free to an unprivileged user on the filesystem that holds
/// `directory`.
///
/// # Errors
///
/// [`SpaceError::Inspect`] if the descriptor cannot be inspected,
/// [`SpaceError::NotDirectory`] if it is not a directory's,
/// [`SpaceError::Failed`] with the errno if the query fails, and
/// [`SpaceError::Overflow`] if the product does not fit 64 bits.
///
/// ```
/// use std::os::fd::AsFd;
/// let directory = std::fs::File::open(std::env::temp_dir())?;
/// let free = ironafs_os::available_space(directory.as_fd())?;
/// # let _ = free;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn available_space(directory: BorrowedFd<'_>) -> Result<u64, SpaceError> {
    let inspected = File::from(
        directory
            .try_clone_to_owned()
            .map_err(SpaceError::Inspect)?,
    )
    .metadata()
    .map_err(SpaceError::Inspect)?;
    if !inspected.is_dir() {
        return Err(SpaceError::NotDirectory);
    }
    measure(directory)
}

/// The free space of the filesystem of `descriptor`, of any kind: the
/// query, retried while a signal interrupts it, and the arithmetic, after
/// the caller's check.
fn measure(descriptor: BorrowedFd<'_>) -> Result<u64, SpaceError> {
    let mut figures = statvfs::zeroed();
    loop {
        match query(descriptor, &mut figures) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(SpaceError::Failed(error)),
            Ok(()) => return available(figures.f_bavail, figures.f_frsize),
        }
    }
}

/// One `fstatvfs` call on `descriptor`, into `figures`.
fn query(descriptor: BorrowedFd<'_>, figures: &mut statvfs) -> io::Result<()> {
    let fd: c_int = descriptor.as_raw_fd();
    let buffer: *mut statvfs = figures;
    #[expect(unsafe_code, reason = "linux-statvfs")]
    // SAFETY: `fstatvfs(3)` writes one `struct statvfs` through `buf` and
    // reads nothing of ours. `buffer` points to `figures`, an exclusive
    // borrow of the caller's zero-initialised local, live and unused for
    // the whole call, of the C struct's full size and alignment (the ABI
    // check proves both on each C library, the opaque tail included), so
    // the call writes only memory this call may write, and nothing keeps
    // the pointer afterwards. `fd` is borrowed from the caller for the
    // call, so it stays open; a descriptor the call cannot query is
    // answered with -1 and errno (EBADF, ENOSYS, EOVERFLOW), never
    // undefined behaviour. The call shares no state with other threads.
    // The declaration is in `abi`, proved by the ABI check.
    let status = unsafe { abi::fstatvfs(fd, buffer) };
    if status != 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
