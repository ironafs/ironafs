// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Site `unix-nodump` (ADR 0005, row 11 and sheet 11), its Linux part: the
//! core-file limit set to zero with `setrlimit(RLIMIT_CORE, {0, 0})`, the
//! process made non-dumpable with `prctl(PR_SET_DUMPABLE, 0)`, and the
//! attribute read back with `prctl(PR_GET_DUMPABLE)`, which must then
//! return exactly 0 (ADR 0004, O26, point 2).
//!
//! The limit alone is not enough: it "will be ignored if the system is
//! configured to pipe core dumps to a program" (core(5)). The dumpable
//! attribute is reset to `/proc/sys/fs/suid_dumpable` when the effective
//! or filesystem user or group id changes, and on an exec of a set-id
//! program or of one whose file capabilities raise its permitted set
//! (`PR_SET_DUMPABLE(2const)`); a plain exec makes it 1 again. So every
//! process O25, point 2, names calls [`disable_dumps`] first in `main`,
//! and again after any such transition it makes; no site on ADR 0005's list
//! changes an id.
//!
//! A non-dumpable process also cannot be attached with `PTRACE_ATTACH`,
//! and its `/proc/<pid>` files belong to root (`PR_SET_DUMPABLE(2const)`).

mod abi;

use core::ffi::{c_int, c_ulong};
use std::io;

use crate::dump::{DumpError, disable_with};
use abi::{PR_GET_DUMPABLE, PR_SET_DUMPABLE, RLIMIT_CORE, rlimit};

#[cfg(test)]
mod tests;

/// The value of `prctl`'s variadic arguments the operation does not use.
const UNUSED: c_ulong = 0;

/// The dumpable value set: `SUID_DUMP_DISABLE`, not dumpable.
const NOT_DUMPABLE: c_ulong = 0;

/// Disables core dumps for this process: the core-file limit set to zero,
/// the process made non-dumpable, and that read back.
///
/// Every step is taken even after one fails; the first failure is
/// returned. Call it first in `main`, before any credential is read, and
/// again after any change of user or group id.
///
/// # Errors
///
/// Returns [`DumpError::Limit`], [`DumpError::Set`] or
/// [`DumpError::ReadBack`] with the system's error if a call fails, and
/// [`DumpError::StillDumpable`] if the attribute reads anything but 0
/// afterwards.
///
/// ```
/// ironafs_os::disable_dumps()?;
/// # Ok::<(), ironafs_os::DumpError>(())
/// ```
pub fn disable_dumps() -> Result<(), DumpError> {
    disable_with(zero_core_limit, set_not_dumpable, dumpable)
}

/// Sets this process's soft and hard core-file limits to zero.
fn zero_core_limit() -> io::Result<()> {
    let limit = rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let pointer: *const rlimit = &raw const limit;
    let resource: c_int = RLIMIT_CORE;
    #[expect(unsafe_code, reason = "unix-nodump")]
    // SAFETY: `setrlimit(2)` reads one `struct rlimit` through `rlim` and
    // writes nothing of ours. `pointer` points to `limit`, a local this
    // function owns, initialised to {0, 0} and valid and unchanged for the
    // whole call; nothing keeps the pointer afterwards. `resource` is
    // `RLIMIT_CORE`, a valid resource on every supported kernel. Lowering
    // a limit needs no privilege; a failure is returned as -1 with errno
    // (EFAULT, EINVAL, EPERM), never undefined behaviour. The limits are
    // per process and the call is safe from any thread. The declaration
    // is in `abi`, proved by the ABI check.
    let status = unsafe { abi::setrlimit(resource, pointer) };
    if status != 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Makes this process non-dumpable.
fn set_not_dumpable() -> io::Result<()> {
    let option: c_int = PR_SET_DUMPABLE;
    #[expect(unsafe_code, reason = "unix-nodump")]
    // SAFETY: `prctl(2)` reads its arguments after the operation as
    // `unsigned long` values, and `PR_SET_DUMPABLE(2const)` reads only the
    // second, which must be 0 or 1. Every argument after the operation is
    // passed as `c_ulong`, at register width, as the variadic C library
    // function reads them (ADR 0005, "Variadic calls"): `NOT_DUMPABLE` (0),
    // then 0 for the three the operation does not read, so the C library
    // reads no argument that was not passed. No pointer is passed and
    // nothing of ours is read or written. The attribute is per process,
    // and the call is safe from any thread. The declaration is in `abi`,
    // proved by the ABI check.
    let status = unsafe { abi::prctl(option, NOT_DUMPABLE, UNUSED, UNUSED, UNUSED) };
    if status != 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Reads this process's dumpable attribute: 0, 1 or 2 (the kernel's
/// root-only dump mode, from `/proc/sys/fs/suid_dumpable`).
fn dumpable() -> io::Result<i32> {
    let option: c_int = PR_GET_DUMPABLE;
    #[expect(unsafe_code, reason = "unix-nodump")]
    // SAFETY: `PR_GET_DUMPABLE(2const)` reads no argument after the
    // operation and returns the attribute, or -1 with errno. The four
    // `unsigned long` arguments `prctl(2)` names are passed as 0, at
    // register width, so the variadic C library function reads no
    // argument that was not passed. No pointer is passed and nothing of
    // ours is read or written; the call is safe from any thread. The
    // declaration is in `abi`, proved by the ABI check.
    let value = unsafe { abi::prctl(option, UNUSED, UNUSED, UNUSED, UNUSED) };
    if value < 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(io::Error::last_os_error());
    }
    Ok(value)
}
