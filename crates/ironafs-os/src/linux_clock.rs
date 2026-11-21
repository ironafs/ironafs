// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Site `linux-clock` (ADR 0005, row 1 and sheet 1): the boot and awake
//! clocks on Linux, `CLOCK_BOOTTIME` and `CLOCK_MONOTONIC` through
//! `clock_gettime`, glibc and musl alike.
//!
//! `clock_gettime(2)`: `CLOCK_MONOTONIC` "does not count time that the
//! system is suspended"; `CLOCK_BOOTTIME` (since Linux 2.6.39) is
//! "identical to `CLOCK_MONOTONIC`, except that it also includes any time
//! that the system is suspended". It is in the C library itself from
//! glibc 2.17. Each clock is read on its own: a failed `CLOCK_BOOTTIME`
//! read is an error, never answered from `CLOCK_MONOTONIC`.

mod abi;

use std::io;

use crate::clock::{ClockError, OsClock, nanos};
use abi::{CLOCK_BOOTTIME, CLOCK_MONOTONIC, timespec};

#[cfg(test)]
mod tests;

/// Reads one of the two monotonic clocks as nanoseconds since an
/// unspecified start (the same start for every read of that clock until
/// the machine restarts).
///
/// # Errors
///
/// Returns [`ClockError::Failed`] with the operating system's error if the
/// read fails, [`ClockError::Invalid`] if it returns a negative second
/// count or a nanosecond field outside one second, and
/// [`ClockError::Overflow`] if the time does not fit 64-bit nanoseconds.
///
/// ```
/// use ironafs_os::{OsClock, read_clock};
///
/// let awake = read_clock(OsClock::Awake)?;
/// let boot = read_clock(OsClock::Boot)?;
/// // The boot clock counts everything the awake clock does, and suspend.
/// assert!(boot >= awake);
/// # Ok::<(), ironafs_os::ClockError>(())
/// ```
pub fn read_clock(clock: OsClock) -> Result<u64, ClockError> {
    nanos(clock, read(clock_id(clock)))
}

/// The clock id each clock is read from.
fn clock_id(clock: OsClock) -> core::ffi::c_int {
    match clock {
        OsClock::Boot => CLOCK_BOOTTIME,
        OsClock::Awake => CLOCK_MONOTONIC,
    }
}

/// Reads the clock `id`, one of the two constants, as its seconds and
/// nanoseconds, or the error the call set.
fn read(id: core::ffi::c_int) -> io::Result<(i64, i64)> {
    let mut time = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let out: *mut timespec = &raw mut time;
    #[expect(unsafe_code, reason = "linux-clock")]
    // SAFETY: `clock_gettime(2)` writes one `struct timespec` through `tp`
    // and reads nothing else of ours. `out` points to `time`, a local this
    // function owns, zero-initialised, valid and unaliased for the whole
    // call; nothing keeps the pointer afterwards. `id` is one of the two
    // constants `read_clock` passes, so it names a valid clock on every
    // supported kernel (2.6.39 and later); an invalid one would be
    // answered with `EINVAL`, not undefined behaviour. The call shares no
    // state with other threads. The declaration is in `abi`, written from
    // the manual page and `<time.h>`, and proved by the ABI check.
    let status = unsafe { abi::clock_gettime(id, out) };
    if status != 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(io::Error::last_os_error());
    }
    // `long` is `i64` on every allowlisted Linux target.
    Ok((time.tv_sec, time.tv_nsec))
}
