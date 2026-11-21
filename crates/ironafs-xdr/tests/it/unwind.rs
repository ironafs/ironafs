// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The encoder when a value's `encode` unwinds (`docs/specs/xdr.md`, 5.5,
//! point 3): the bytes a component wrote before it unwound count against
//! the message's size whoever catches the unwind, so the caller's
//! allocation never grows or moves, and an unwind that leaves the encoder
//! leaves the buffer wiped and empty.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use ironafs_xdr::{Encode, Encoder, Error, ErrorKind, Message, MessageType, encode_into};

/// Room for any test buffer's capacity and more.
const ROOMY: Message = Message {
    name: "test.unwind",
    cap: 4096,
    budget: 0,
    secret: false,
};

/// Unwinds without a panic message, so the test's output stays clean.
fn unwind() -> ! {
    resume_unwind(Box::new("a component's encode unwinds"))
}

/// Writes its words, then unwinds.
struct Bomb(usize);

impl Encode for Bomb {
    const SECRET: bool = false;

    fn encoded_len(&self) -> Result<u64, Error> {
        Ok(4 * u64::try_from(self.0).unwrap())
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        (0..self.0).try_for_each(|_| encoder.put_u32(0x5ec2_e7ed))?;
        unwind()
    }
}

/// Sizes itself as its component alone, catches the component's unwind,
/// and then writes one word more: what a stale count would let through.
struct Catcher(Bomb);

impl MessageType for Catcher {
    const MESSAGE: Message = ROOMY;
}

impl Encode for Catcher {
    const SECRET: bool = false;

    fn encoded_len(&self) -> Result<u64, Error> {
        self.0.encoded_len()
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        let caught = catch_unwind(AssertUnwindSafe(|| encoder.put(&self.0)));
        assert!(caught.is_err(), "the component unwinds");
        encoder.put_u32(1)
    }
}

/// A message that is one [`Bomb`]: its unwind leaves the encoder.
struct Lone(Bomb);

impl MessageType for Lone {
    const MESSAGE: Message = ROOMY;
}

impl Encode for Lone {
    const SECRET: bool = false;

    fn encoded_len(&self) -> Result<u64, Error> {
        self.0.encoded_len()
    }

    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
        encoder.put(&self.0)
    }
}

/// A buffer with an allocation, and as many whole words as fill it.
fn full_buffer() -> (Vec<u8>, usize) {
    let out: Vec<u8> = Vec::with_capacity(8);
    let words = out.capacity() / 4;
    (out, words)
}

#[test]
fn a_caught_unwind_leaves_the_count_of_what_the_component_wrote() {
    let (mut out, words) = full_buffer();
    let before = (out.as_ptr(), out.capacity());
    // The component fills the whole size before it unwinds, so the word
    // its parent writes afterwards is past the size.
    let error = encode_into(&Catcher(Bomb(words)), &mut out).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MessageTooLarge, "{error}");
    assert_eq!(error.name(), Some("test.unwind"));
    assert_eq!(out, []);
    assert_eq!((out.as_ptr(), out.capacity()), before);
}

#[test]
fn an_unwind_out_of_the_encoder_leaves_the_buffer_wiped_in_place() {
    let (mut out, words) = full_buffer();
    out.push(0xaa);
    let before = (out.as_ptr(), out.capacity());
    let caught = catch_unwind(AssertUnwindSafe(|| {
        encode_into(&Lone(Bomb(words)), &mut out)
    }));
    assert!(caught.is_err(), "the unwind reaches the caller");
    assert_eq!(out, []);
    assert_eq!((out.as_ptr(), out.capacity()), before);
}
