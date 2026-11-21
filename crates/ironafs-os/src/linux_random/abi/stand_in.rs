// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Miri stand-in for `syscall` (ADR 0005, point 6.5), built only under
//! Miri, which cannot make a system call. It takes what the wrapper passes
//! (the number and six `long`s) and honours `getrandom`'s contract for
//! them: given `SYS_getrandom`, a buffer address, a length of 1 to 256 and
//! flags 0, it writes that many bytes at the address and returns the
//! length; anything else returns -1 and writes nothing. The bytes are one
//! value per call, never zero, different from the last call's. So Miri
//! checks the wrapper's address, its pairing with the length, and the
//! classification of the result.

use core::ffi::c_long;
use core::sync::atomic::{AtomicU8, Ordering};

use crate::linux_random::number::SYS_getrandom;

/// The byte the next call writes.
static NEXT: AtomicU8 = AtomicU8::new(1);

/// Fills the buffer at `address` as `getrandom` would.
///
/// # Safety
///
/// As the foreign call's: `address` is the exposed address of memory valid
/// for `length` bytes of writes.
#[expect(unsafe_code, reason = "linux-random: Miri stand-in")]
pub(in crate::linux_random) unsafe fn syscall(
    number: c_long,
    address: c_long,
    length: c_long,
    flags: c_long,
    unused3: c_long,
    unused4: c_long,
    unused5: c_long,
) -> c_long {
    let Ok(count) = usize::try_from(length) else {
        return -1;
    };
    let shape = number == SYS_getrandom
        && (1..=256).contains(&count)
        && flags == 0
        && [unused3, unused4, unused5] == [0, 0, 0];
    if !shape {
        return -1;
    }
    let value = NEXT.fetch_add(1, Ordering::Relaxed).max(1);
    let pointer: *mut u8 =
        core::ptr::with_exposed_provenance_mut(usize::from_ne_bytes(address.to_ne_bytes()));
    #[expect(unsafe_code, reason = "linux-random: Miri stand-in")]
    // SAFETY: the caller promises `address` is the exposed address of
    // memory valid for `length` bytes of writes, as `getrandom` requires
    // of its buffer; Miri checks that promise and the provenance.
    let () = unsafe { core::ptr::write_bytes(pointer, value, count) };
    length
}
