// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]

//! IronAFS's XDR codec, with size caps (`docs/specs/xdr.md`).

extern crate alloc;

mod decode;
mod error;
pub mod limits;
mod spec;

pub use decode::{
    Decode, Decoder, Mark, Reservation, array_heap_ratio, array_reservations, decode,
    decode_traced, max, proven,
};
pub use error::{Error, ErrorKind};
pub use spec::{Cap, Message};
