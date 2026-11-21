// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]

//! IronAFS's XDR codec, with size caps (`docs/specs/xdr.md`).
//!
//! The codec of the profile's types: integers, hypers, enumerations,
//! booleans, opaque data, strings, arrays, structures, discriminated unions
//! and void, encoded as RFC 4506 sets out. Every variable-length item has a
//! cap in bytes, checked before anything is reserved; every heap request
//! goes through one ledger, held to the message's memory budget; and the
//! decoder is strict: non-zero padding, a boolean other than 0 or 1, an
//! undeclared enumeration value, a discriminant with no arm and trailing
//! bytes are each rejected. It is pure: bytes in, values or errors out.
//!
//! A protocol's generated code implements [`Decode`] and [`Encode`] for
//! each of its types, from the decoder's and encoder's primitives:
//!
//! ```
//! use ironafs_xdr::{
//!     Cap, Decode, Decoder, Encode, Encoder, Error, Message, decode, encode, opaque_len, sum,
//! };
//!
//! const NAME: Cap = Cap { name: "demo.greeting.name", bound: Some(16), bytes: 16 };
//! const GREETING: Message = Message { name: "demo.greeting", cap: 64, budget: 64, secret: false };
//!
//! #[derive(Debug, PartialEq)]
//! struct Greeting {
//!     count: u32,
//!     name: Vec<u8>,
//! }
//!
//! impl<'a> Decode<'a> for Greeting {
//!     const S_MIN: u64 = 8;
//!     const DEPTH: u32 = 1;
//!     const HEAP_RATIO: Option<u64> = Some(1);
//!     const RESERVATIONS: Option<u64> = Some(1);
//!
//!     fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
//!         Ok(Self { count: decoder.read_u32()?, name: decoder.opaque_owned(&NAME)? })
//!     }
//! }
//!
//! impl ironafs_xdr::Plain for Greeting {}
//!
//! impl Encode for Greeting {
//!     fn encoded_len(&self) -> Result<u64, Error> {
//!         sum(4, opaque_len(&NAME, self.name.len())?)
//!     }
//!
//!     fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
//!         encoder.put_u32(self.count)?;
//!         encoder.put_opaque(&NAME, &self.name)
//!     }
//! }
//!
//! let greeting = Greeting { count: 2, name: b"hi".to_vec() };
//! let bytes = encode(&GREETING, &greeting).unwrap();
//! assert_eq!(bytes, [0, 0, 0, 2, 0, 0, 0, 2, b'h', b'i', 0, 0]);
//! assert_eq!(decode::<Greeting>(&GREETING, &bytes).unwrap(), greeting);
//! ```
//!
//! A message nested deeper than [`limits::DEPTH_MAX`] does not build: the
//! entry points check each type's depth when they are compiled for it, so
//! no input is ever recursed into past the limit (5.2, point 10.3). Sixteen
//! levels build:
//!
//! ```
//! # use ironafs_xdr::{Decode, Decoder, Error, Message, decode};
//! # struct Level<T>(T);
//! # impl<'a, T: Decode<'a>> Decode<'a> for Level<T> {
//! #     const S_MIN: u64 = T::S_MIN;
//! #     const DEPTH: u32 = T::DEPTH + 1;
//! #     const HEAP_RATIO: Option<u64> = T::HEAP_RATIO;
//! #     const RESERVATIONS: Option<u64> = T::RESERVATIONS;
//! #     fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
//! #         T::decode(decoder).map(Self)
//! #     }
//! # }
//! # type Four<T> = Level<Level<Level<Level<T>>>>;
//! # const M: Message = Message { name: "demo.deep", cap: 4, budget: 0, secret: false };
//! type Sixteen = Four<Four<Four<Four<u32>>>>;
//! assert!(decode::<Sixteen>(&M, &[0, 0, 0, 7]).is_ok());
//! ```
//!
//! and seventeen do not:
//!
//! ```compile_fail
//! # use ironafs_xdr::{Decode, Decoder, Error, Message, decode};
//! # struct Level<T>(T);
//! # impl<'a, T: Decode<'a>> Decode<'a> for Level<T> {
//! #     const S_MIN: u64 = T::S_MIN;
//! #     const DEPTH: u32 = T::DEPTH + 1;
//! #     const HEAP_RATIO: Option<u64> = T::HEAP_RATIO;
//! #     const RESERVATIONS: Option<u64> = T::RESERVATIONS;
//! #     fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
//! #         T::decode(decoder).map(Self)
//! #     }
//! # }
//! # type Four<T> = Level<Level<Level<Level<T>>>>;
//! # const M: Message = Message { name: "demo.deep", cap: 4, budget: 0, secret: false };
//! type Seventeen = Level<Four<Four<Four<Four<u32>>>>>;
//! let _ = decode::<Seventeen>(&M, &[0, 0, 0, 7]);
//! ```

extern crate alloc;

mod borrowed;
mod decode;
mod encode;
mod error;
pub mod limits;
mod spec;

pub use borrowed::{BorrowedBytes, BorrowedHyper, BorrowedWord};
pub use decode::{
    Decode, Decoder, Mark, OutOfLine, Reservation, Vacant, array_heap_ratio, array_reservations,
    decode, decode_traced, max, out_of_line_heap_ratio, out_of_line_reservations, proven,
};
pub use encode::{
    Encode, Encoder, Plain, array_len, encode, encode_into, fixed_array_len, fixed_opaque_len,
    opaque_len, sum,
};
pub use error::{Error, ErrorKind};
pub use spec::{Cap, Message};
