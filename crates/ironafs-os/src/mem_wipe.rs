// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Site `mem-wipe` (ADR 0005, row 24): `wipe`, the volatile overwrite that
//! zeroes key material and token buffers (ADR 0004, O28, point 2), the
//! same on every target.
//!
//! **Contract.** Every byte of the slice is written with zero before `wipe`
//! returns, even when the buffer is never read again. That rests on
//! `core::ptr::write_volatile` alone: the compiler does not remove these
//! stores, nor merge or reorder them with other volatile operations. The
//! `compiler_fence` after the loop is an ordering aid only; it does not
//! guarantee that every later access stays after the wipe, and no part of
//! the contract rests on it.
//!
//! **Limits** (coding standards rule 8). It wipes the addressed buffer
//! only. Copies made by moves, spills to the stack and values held in
//! registers are not covered; callers keep key material in one place and
//! pass it by reference. A buffer the operating system owns until an I/O
//! completes is wiped only after completion.
//!
//! **Evidence.** Miri runs this block over every length from 0 to 257
//! (the unit tests). `cargo xtask check-wipe` is sheet 24's release-assembly
//! gate: it builds this function into a harness under the workspace's
//! release profile and checks the emitted zero stores, for a live buffer
//! and for one wiped just before it is freed. Both functions are
//! `#[inline]` so the optimiser sees the body together with the buffer's
//! whole lifetime there, as the gate requires; the volatile writes keep
//! the stores wherever the body lands. Until the gate has run on a target,
//! the site does not ship there.
//!
//! Callers outside this crate reach it only by injection: the pure crates
//! that hold secrets cannot depend on this crate, so each takes a
//! `fn(&mut [u8])` at composition, and production passes this function.

use core::sync::atomic::{Ordering, compiler_fence};

#[cfg(test)]
mod tests;

/// Overwrites every byte of `buffer` with zero, in stores the compiler
/// keeps (see the module's contract and limits).
///
/// ```
/// let mut key = [0x5A_u8; 32];
/// ironafs_os::wipe(&mut key);
/// assert_eq!(key, [0; 32]);
/// ```
#[inline]
pub fn wipe(buffer: &mut [u8]) {
    for byte in buffer.iter_mut() {
        zero(byte);
    }
    compiler_fence(Ordering::SeqCst);
}

/// The site's one block: a volatile write of zero to one byte.
#[inline]
fn zero(byte: &mut u8) {
    let byte: *mut u8 = byte;
    #[expect(unsafe_code, reason = "mem-wipe")]
    // SAFETY: `byte` was made from an exclusive reference to one `u8`, which
    // lives for this call: it is non-null, aligned (a `u8` needs no
    // alignment), valid for one write and unaliased while the write runs.
    // A `u8` has no invalid value and no drop, so writing zero over it
    // leaves no value broken. No foreign code is involved (ADR 0005, sheet
    // 24); the thread rule is the caller's exclusive borrow.
    let () = unsafe { core::ptr::write_volatile(byte, 0) };
}
