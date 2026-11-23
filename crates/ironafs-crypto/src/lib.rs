// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's cryptography interface (ADR 0004, C5, point 1).
//!
//! Protocol crates reach cryptography only through this crate; the
//! implementations behind it are supplied when the client is composed. It
//! is a pure crate: no I/O and no `unsafe` (C4, point 1).
//!
//! It holds the cryptographic random source: [`EntropySource`], which the
//! composition implements over the operating system's own source, and
//! [`CryptoRng`], the production [`Rng`](ironafs_time::Rng) built only
//! from one (C4, point 7). Keepalive call identities are drawn from it, so
//! an off-path sender cannot guess them (C32, point 1).

#![no_std]

extern crate alloc;

mod random;

pub use random::{CryptoRng, EntropyError, EntropySource};
