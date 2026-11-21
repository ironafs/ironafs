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

/// A message's cap on its total encoded size, and its memory budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message {
    /// The message's cap name, `<interface>.<message>`.
    pub name: &'static str,
    /// The cap on the encoded message, in bytes.
    pub cap: u32,
    /// The most bytes the decoder may request from the allocator while
    /// decoding one input (5.2, point 9).
    pub budget: u64,
    /// Whether the message holds a secret field at any depth. Errors from
    /// decoding it carry no offset or length (section 7, point 2). A
    /// message type that can hold a secret by its encoding
    /// ([`Encode::SECRET`](crate::Encode::SECRET)) and says otherwise here
    /// does not build in [`encode_into`](crate::encode_into).
    pub secret: bool,
}

/// A type that is one whole message, with the descriptor the generated code
/// sets for it, so that a message's type and its caps, budget and secrecy
/// cannot be paired wrongly.
pub trait MessageType {
    /// The message's descriptor.
    const MESSAGE: Message;
}
