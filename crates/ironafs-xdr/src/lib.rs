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
//! each of its types, from the decoder's and encoder's primitives. Each
//! type says once whether it can hold a secret ([`Classified`]), and each
//! message's type carries its descriptor ([`MessageType`]), which the
//! entry points take from the type:
//!
//! ```
//! use ironafs_xdr::{
//!     Cap, Classified, Decode, Decoder, Encode, Encoder, Error, Message, MessageType, Sizer,
//!     decode, encode,
//! };
//!
//! const NAME: Cap = Cap { name: "demo.greeting.name", bound: Some(16), bytes: 16 };
//! const GREETING: Message = Message { name: "demo.greeting", cap: 64, budget: 64 };
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
//! impl Classified for Greeting {
//!     const SECRET: bool = false;
//! }
//!
//! impl MessageType for Greeting {
//!     const MESSAGE: Message = GREETING;
//! }
//!
//! impl Encode for Greeting {
//!     fn size(&self, sizer: &Sizer) -> Result<(), Error> {
//!         sizer.put(&self.count)?;
//!         sizer.opaque(&NAME, self.name.len())
//!     }
//!
//!     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
//!         encoder.put_u32(self.count)?;
//!         encoder.put_opaque(&NAME, &self.name)
//!     }
//! }
//!
//! let greeting = Greeting { count: 2, name: b"hi".to_vec() };
//! let bytes = encode(&greeting).unwrap();
//! assert_eq!(bytes, [0, 0, 0, 2, 0, 0, 0, 2, b'h', b'i', 0, 0]);
//! assert_eq!(decode::<Greeting>(&bytes).unwrap(), greeting);
//! ```
//!
//! A message nested deeper than [`limits::DEPTH_MAX`] does not build: the
//! entry points check each type's depth when they are compiled for it, so
//! no input is ever recursed into past the limit (5.2, point 10.3). Sixteen
//! levels build:
//!
//! ```
//! # use ironafs_xdr::{Classified, Decode, Decoder, Error, Message, MessageType, decode};
//! # struct Level<T>(T);
//! # impl<T> Classified for Level<T> {
//! #     const SECRET: bool = false;
//! # }
//! # impl<T> MessageType for Level<T> {
//! #     const MESSAGE: Message = Message { name: "demo.deep", cap: 4, budget: 0 };
//! # }
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
//! type Sixteen = Four<Four<Four<Four<u32>>>>;
//! assert!(decode::<Sixteen>(&[0, 0, 0, 7]).is_ok());
//! ```
//!
//! and seventeen do not:
//!
//! ```compile_fail,E0080
//! # use ironafs_xdr::{Classified, Decode, Decoder, Error, Message, MessageType, decode};
//! # struct Level<T>(T);
//! # impl<T> Classified for Level<T> {
//! #     const SECRET: bool = false;
//! # }
//! # impl<T> MessageType for Level<T> {
//! #     const MESSAGE: Message = Message { name: "demo.deep", cap: 4, budget: 0 };
//! # }
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
//! type Seventeen = Level<Four<Four<Four<Four<u32>>>>>;
//! let _ = decode::<Seventeen>(&[0, 0, 0, 7]);
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
    Decode, Decoder, Mark, OutOfLine, Reservation, array_heap_ratio, array_reservations, decode,
    decode_traced, fixed_array_heap_ratio, fixed_array_reservations, max, out_of_line_heap_ratio,
    out_of_line_reservations, proven,
};
pub use encode::{Encode, Encoder, Sizer, encode, encode_into, encoded_len};
pub use error::{Error, ErrorKind};
pub use spec::{Cap, Classified, Message, MessageType};
