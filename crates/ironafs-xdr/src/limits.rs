// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The profile-wide limits on inline storage and nesting
//! (`docs/specs/xdr.md`, 5.2, point 10, and section 13, Q7).
//!
//! They hold for every message of every protocol. Changing one moves
//! these constants only; the code that checks them reads them from here.

/// `xdr.inline_max`: the most bytes a decoded type may occupy inline
/// (`size_of` on the target), on the stack or inside its parent.
pub const INLINE_MAX: usize = 256;

/// The longest fixed-length opaque held inline. Anything longer is held
/// out of line, in a buffer reserved through the ledger, or borrowed from
/// the input in a secret-bearing message.
pub const INLINE_OPAQUE_MAX: usize = 64;

/// `xdr.depth_max`: the deepest nesting of a message, counted in
/// structures, unions and arrays from the message down.
pub const DEPTH_MAX: u32 = 16;

/// The stack, in bytes, on which the largest valid input of every type
/// must decode (the test `decode_fits_small_stack`).
pub const SMALL_STACK: usize = 64 * 1024;
