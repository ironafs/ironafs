// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]

//! IronAFS's XDR codec, with size caps (`docs/specs/xdr.md`).

extern crate alloc;

mod borrowed;
mod decode;
mod encode;
mod error;
pub mod limits;
mod spec;

pub use borrowed::{BorrowedBytes, BorrowedHyper, BorrowedWord};
pub use decode::{
    Decode, Decoder, Mark, Reservation, array_heap_ratio, array_reservations, decode,
    decode_traced, max, proven,
};
pub use encode::{
    Encode, Encoder, array_len, encode, encode_into, fixed_array_len, fixed_opaque_len, opaque_len,
    sum,
};
pub use error::{Error, ErrorKind};
pub use spec::{Cap, Message};
