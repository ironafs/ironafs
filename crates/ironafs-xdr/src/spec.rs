// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a protocol specification declares for each message and each
//! variable-length item: its caps, bounds and budget
//! (`docs/specs/xdr.md`, 5.2, point 5).
//!
//! Caps are compiled in (5.2, point 8): the codec takes them as
//! `&'static` constants and never computes one from the input.

/// A variable-length item's cap and declared bound.
///
/// For opaque data and strings, `bound` and `bytes` both limit the data
/// bytes, excluding the length word and padding. For a variable-length
/// array, `bound` limits the element count and `bytes` the total encoded
/// bytes of the elements, excluding the count word: an element count alone
/// is never a cap (5.2, point 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cap {
    /// The stable cap name, `<interface>.<message>.<field path>`; errors
    /// name the field by it.
    pub name: &'static str,
    /// The declared bound `m`, or `None` where the definition states none
    /// (2³² − 1).
    pub bound: Option<u32>,
    /// The cap in bytes, the same in both directions.
    pub bytes: u32,
}

/// A message's cap on its total encoded size, and its memory budget: its
/// row of the cap table, as the generated code sets it for the message's
/// type ([`MessageType`]).
///
/// Whether the message is secret-bearing is not here. It is the one flag
/// of the message's type ([`Classified::SECRET`]), which the encoder and
/// the decoder both read, so a descriptor cannot say one thing and the
/// type another. This builds:
///
/// ```
/// use ironafs_xdr::Message;
///
/// const KEY: Message = Message { name: "demo.key", cap: 4, budget: 0 };
/// ```
///
/// and a descriptor with a secrecy of its own does not:
///
/// ```compile_fail,E0560
/// use ironafs_xdr::Message;
///
/// const KEY: Message = Message { name: "demo.key", cap: 4, budget: 0, secret: true };
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message {
    /// The message's cap name, `<interface>.<message>`.
    pub name: &'static str,
    /// The cap on the encoded message, in bytes.
    pub cap: u32,
    /// The most bytes the decoder may request from the allocator while
    /// decoding one input (5.2, point 9).
    pub budget: u64,
}

/// A type that says, once, whether a value of it can hold a secret field
/// (`docs/specs/xdr.md`, 5.1, point 8).
///
/// Every type the encoder writes has the flag ([`Encode`](crate::Encode)),
/// and so has every message type ([`MessageType`]), whichever way it
/// travels. A type has one implementation of this trait, so the flag is
/// the same wherever it is read:
///
/// - [`Encoder::put`](crate::Encoder::put) fails the build of a type that
///   declares itself plain and writes a secret component;
/// - [`encode`](crate::encode) fails the build for a secret-bearing
///   message, which goes through [`encode_into`](crate::encode_into);
/// - [`decode`](crate::decode) builds every error of a secret-bearing
///   message without an offset or a length (section 7, point 2).
///
/// # Limits
///
/// The compiler checks the flag only as far as the types carry it. These
/// rest on the generator, its tests and review:
///
/// - **A leaf's own flag.** A type with a secret field of its own, by its
///   cap table's Secret column, must say so here. Nothing checks it: the
///   primitive writes ([`Encoder::put_u32`](crate::Encoder::put_u32),
///   [`Encoder::put_opaque`](crate::Encoder::put_opaque),
///   [`Encoder::put_fixed_opaque`](crate::Encoder::put_fixed_opaque) and
///   the rest) take bytes, and bytes do not say whether they are secret.
///   That includes bytes a value reveals from a borrowed secret
///   ([`BorrowedBytes::reveal`](crate::BorrowedBytes::reveal)) and writes
///   as its own field.
/// - **Bytes copied in by hand.** Code that encodes a value apart and
///   writes its output into another message as opaque data is not
///   stopped.
/// - **The components of a decoded message.** The decoder reads the flag
///   of the message's type only. That a secret-bearing message's type
///   borrows every field from the input, and that a type holding a
///   secret component says it is secret, is checked on encode and not on
///   decode.
pub trait Classified {
    /// Whether a value of the type can hold a secret field at any depth:
    /// true if the type has a secret field of its own, or any component,
    /// element or arm whose type is secret.
    const SECRET: bool;
}

macro_rules! plain {
    ($($type:ty),*) => {$(
        impl Classified for $type {
            const SECRET: bool = false;
        }
    )*};
}

// The profile's own types hold what their parent's field holds; a parent
// with a secret integer field says so itself.
plain!(i32, u32, i64, u64, ());

/// A type that is one whole message, with the descriptor the generated code
/// sets for it. The entry points take the descriptor and the secrecy from
/// the type, so a message's type and its caps, budget and secrecy cannot
/// be paired wrongly, in either direction.
pub trait MessageType: Classified {
    /// The message's descriptor.
    const MESSAGE: Message;
}
