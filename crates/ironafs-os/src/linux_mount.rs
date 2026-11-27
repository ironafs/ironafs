// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Site `linux-mount` (ADR 0005, row 15 and sheet 15): the direct FUSE
//! mount used by the privileged step of ADR 0004, L7, point 1.

mod abi;

use core::ffi::{c_char, c_ulong, c_void};
use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use abi::{MS_NODEV, MS_NOSUID};

#[cfg(test)]
mod tests;

/// Mounts FUSE at `target` with the borrowed `/dev/fuse` descriptor,
/// the root's file mode including its type bits, and the mount owner's IDs.
///
/// The mount uses `nosuid`, `nodev` and `allow_other`. The caller needs
/// `CAP_SYS_ADMIN` and is responsible for selecting and securing the
/// mountpoint (ADR 0004, L7, point 1).
///
/// # Errors
///
/// [`io::ErrorKind::InvalidInput`] if the pathname contains a NUL byte,
/// before any mount call. A failed mount returns the kernel's errno.
pub fn mount_fuse(
    target: &Path,
    device: BorrowedFd<'_>,
    root_mode: u32,
    user_id: u32,
    group_id: u32,
) -> io::Result<()> {
    let target = CString::new(target.as_os_str().as_bytes())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    mount(&target, device, root_mode, user_id, group_id)
}

/// Makes one `mount` call using only the options of ADR 0005, sheet 15.
fn mount(
    target: &CStr,
    device: BorrowedFd<'_>,
    root_mode: u32,
    user_id: u32,
    group_id: u32,
) -> io::Result<()> {
    let fd = device.as_raw_fd();
    let options = CString::new(format!(
        "fd={fd},rootmode={root_mode:o},user_id={user_id},group_id={group_id},allow_other"
    ))
    .map_err(io::Error::other)?;
    let source: *const c_char = c"ironafs".as_ptr();
    let target: *const c_char = target.as_ptr();
    let filesystem_type: *const c_char = c"fuse".as_ptr();
    let flags: c_ulong = MS_NOSUID | MS_NODEV;
    let data: *const c_void = options.as_ptr().cast();
    #[expect(unsafe_code, reason = "linux-mount")]
    // SAFETY: `mount(2)` reads the source, target and filesystem type as
    // C strings. With type `fuse`, Linux reads `data` as a C string of
    // mount options (Linux FUSE documentation, Mount options). Source
    // and type are static C strings, the target is borrowed from the
    // caller's CString, and the options are kept in a local CString.
    // All four are NUL-terminated and live and immutable through the
    // call. The kernel copies the strings and keeps no userspace pointer.
    // The descriptor stays open under the borrow while the kernel looks
    // it up. An unsuitable descriptor or a caller without the required
    // privilege is a kernel error. Calls need no thread affinity, and
    // errno is read on the calling thread. The ABI check proves `abi`'s
    // declaration against <sys/mount.h>.
    let status = unsafe { abi::mount(source, target, filesystem_type, flags, data) };
    if status != 0 {
        return Err(abi::last_error());
    }
    Ok(())
}
