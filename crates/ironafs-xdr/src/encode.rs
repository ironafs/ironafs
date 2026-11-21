// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Encoding: the canonical form (`docs/specs/xdr.md`, 5.5).
//!
//! The encoder works in two passes. [`Encode::encoded_len`] computes the
//! message's size with checked arithmetic and refuses every value the
//! decoder would reject, by declared bound and by cap; only then is the
//! output requested, once, and written by `push` alone, so the buffer
//! never grows (5.5, point 3). Every check runs again as the value is
//! written, so the write pass cannot emit an item the size pass would
//! refuse.

use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;

use crate::{Cap, Classified, Error, ErrorKind, Message, MessageType};

#[cfg(test)]
mod tests;

/// A type the codec can encode, as a protocol's generated code defines it.
///
/// Whether a value of it can hold a secret is its one flag
/// ([`Classified::SECRET`]), which [`Encoder::put`] checks against each
/// component it writes.
pub trait Encode: Classified {
    /// The value's encoded size in bytes.
    ///
    /// # Errors
    ///
    /// `LengthNotRepresentable`, `LengthOverBound` or `CapExceeded` for an
    /// item the decoder would reject; `MessageTooLarge` if the sum
    /// overflows.
    fn encoded_len(&self) -> Result<u64, Error>;

    /// Writes the value, with an encoder typed for its own type: a
    /// component of another type is written through [`Encoder::put`],
    /// which checks its secrecy. So this builds:
    ///
    /// ```
    /// # use ironafs_xdr::{Classified, Encode, Encoder, Error, Message, MessageType, encode, encode_into};
    /// # /// A secret-bearing message: a 4-byte key.
    /// # struct Key([u8; 4]);
    /// # impl Classified for Key {
    /// #     const SECRET: bool = true;
    /// # }
    /// # impl MessageType for Key {
    /// #     const MESSAGE: Message = Message { name: "demo.key", cap: 4, budget: 0 };
    /// # }
    /// # impl Encode for Key {
    /// #     fn encoded_len(&self) -> Result<u64, Error> {
    /// #         Ok(4)
    /// #     }
    /// #     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
    /// #         encoder.put_fixed_opaque(&self.0)
    /// #     }
    /// # }
    ///
    /// /// A message that holds one value of `C`.
    /// struct Wrapper<C>(C);
    ///
    /// impl<C: Encode> Classified for Wrapper<C> {
    ///     const SECRET: bool = C::SECRET;
    /// }
    ///
    /// impl<C: Encode> MessageType for Wrapper<C> {
    ///     const MESSAGE: Message = Message { name: "demo.wrapper", cap: 4, budget: 0 };
    /// }
    ///
    /// impl<C: Encode> Encode for Wrapper<C> {
    ///     fn encoded_len(&self) -> Result<u64, Error> {
    ///         self.0.encoded_len()
    ///     }
    ///
    ///     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
    ///         encoder.put(&self.0)
    ///     }
    /// }
    ///
    /// let _ = encode(&Wrapper(7_u32));
    /// ```
    ///
    /// and the same with the one line changed, to write the component with
    /// its parent's encoder, does not:
    ///
    /// ```compile_fail,E0308
    /// # use ironafs_xdr::{Classified, Encode, Encoder, Error, Message, MessageType, encode, encode_into};
    /// # /// A secret-bearing message: a 4-byte key.
    /// # struct Key([u8; 4]);
    /// # impl Classified for Key {
    /// #     const SECRET: bool = true;
    /// # }
    /// # impl MessageType for Key {
    /// #     const MESSAGE: Message = Message { name: "demo.key", cap: 4, budget: 0 };
    /// # }
    /// # impl Encode for Key {
    /// #     fn encoded_len(&self) -> Result<u64, Error> {
    /// #         Ok(4)
    /// #     }
    /// #     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
    /// #         encoder.put_fixed_opaque(&self.0)
    /// #     }
    /// # }
    ///
    /// /// A message that holds one value of `C`.
    /// struct Wrapper<C>(C);
    ///
    /// impl<C: Encode> Classified for Wrapper<C> {
    ///     const SECRET: bool = C::SECRET;
    /// }
    ///
    /// impl<C: Encode> MessageType for Wrapper<C> {
    ///     const MESSAGE: Message = Message { name: "demo.wrapper", cap: 4, budget: 0 };
    /// }
    ///
    /// impl<C: Encode> Encode for Wrapper<C> {
    ///     fn encoded_len(&self) -> Result<u64, Error> {
    ///         self.0.encoded_len()
    ///     }
    ///
    ///     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
    ///         self.0.encode(encoder)
    ///     }
    /// }
    ///
    /// let _ = encode(&Wrapper(7_u32));
    /// ```
    ///
    /// # Errors
    ///
    /// As [`Encode::encoded_len`], and `MessageTooLarge` if the value
    /// writes more bytes than its size.
    fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error>;
}

/// The output of one message, as the value of type `T` being written sees
/// it: its buffer, and the bytes the message may still take.
///
/// Every encoder of one message shares the one count of the bytes left, by
/// reference: a component's encoder ([`Encoder::put`]) takes each byte it
/// writes from the same count as its parent's, as it writes it. So the
/// count is right at every moment, also when a component's `encode`
/// unwinds and its parent catches the unwind, and no write passes the
/// size requested for the output.
///
/// Its `Debug` shows the message's name only: neither the output, which
/// may hold a secret-bearing message, nor the bytes still to write, which
/// would tell a secret field's length.
pub struct Encoder<'o, T: ?Sized> {
    out: &'o mut Vec<u8>,
    remaining: &'o mut u64,
    message: &'static str,
    cap: u32,
    value: PhantomData<fn(&T)>,
}

impl<T: ?Sized> fmt::Debug for Encoder<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Encoder")
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

/// Encodes `value`, one message of type `T`, into a new buffer, under the
/// descriptor its type carries.
///
/// A buffer this crate returns is never zeroed on drop, so a message whose
/// type says it can hold a secret ([`Classified::SECRET`]) does not build
/// here: [`encode_into`], with a buffer the caller wipes, is its only path
/// (`docs/specs/xdr.md`, 5.1, point 8). A type holding a secret component
/// is secret ([`Encoder::put`]), so this refuses every message that can
/// hold a secret, directly or through a generic caller. A plain message
/// builds:
///
/// ```
/// # use ironafs_xdr::{Classified, Encode, Encoder, Error, Message, MessageType, encode, encode_into};
/// # /// A secret-bearing message: a 4-byte key.
/// # struct Key([u8; 4]);
/// # impl Classified for Key {
/// #     const SECRET: bool = true;
/// # }
/// # impl MessageType for Key {
/// #     const MESSAGE: Message = Message { name: "demo.key", cap: 4, budget: 0 };
/// # }
/// # impl Encode for Key {
/// #     fn encoded_len(&self) -> Result<u64, Error> {
/// #         Ok(4)
/// #     }
/// #     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
/// #         encoder.put_fixed_opaque(&self.0)
/// #     }
/// # }
/// # /// A plain message: one word.
/// # struct Word(u32);
/// # impl Classified for Word {
/// #     const SECRET: bool = false;
/// # }
/// # impl MessageType for Word {
/// #     const MESSAGE: Message = Message { name: "demo.word", cap: 4, budget: 0 };
/// # }
/// # impl Encode for Word {
/// #     fn encoded_len(&self) -> Result<u64, Error> {
/// #         Ok(4)
/// #     }
/// #     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
/// #         encoder.put_u32(self.0)
/// #     }
/// # }
///
/// fn send<T: Encode + MessageType>(value: &T) -> Result<Vec<u8>, Error> {
///     encode(value)
/// }
///
/// let _ = send(&Word(7));
/// ```
///
/// and the same with the one line changed, to send the key, does not:
///
/// ```compile_fail,E0080
/// # use ironafs_xdr::{Classified, Encode, Encoder, Error, Message, MessageType, encode, encode_into};
/// # /// A secret-bearing message: a 4-byte key.
/// # struct Key([u8; 4]);
/// # impl Classified for Key {
/// #     const SECRET: bool = true;
/// # }
/// # impl MessageType for Key {
/// #     const MESSAGE: Message = Message { name: "demo.key", cap: 4, budget: 0 };
/// # }
/// # impl Encode for Key {
/// #     fn encoded_len(&self) -> Result<u64, Error> {
/// #         Ok(4)
/// #     }
/// #     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
/// #         encoder.put_fixed_opaque(&self.0)
/// #     }
/// # }
/// # /// A plain message: one word.
/// # struct Word(u32);
/// # impl Classified for Word {
/// #     const SECRET: bool = false;
/// # }
/// # impl MessageType for Word {
/// #     const MESSAGE: Message = Message { name: "demo.word", cap: 4, budget: 0 };
/// # }
/// # impl Encode for Word {
/// #     fn encoded_len(&self) -> Result<u64, Error> {
/// #         Ok(4)
/// #     }
/// #     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
/// #         encoder.put_u32(self.0)
/// #     }
/// # }
///
/// fn send<T: Encode + MessageType>(value: &T) -> Result<Vec<u8>, Error> {
///     encode(value)
/// }
///
/// let _ = send(&Key([1, 2, 3, 4]));
/// ```
///
/// # Errors
///
/// As [`encode_into`].
pub fn encode<T: Encode + MessageType + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
    const {
        assert!(
            !T::SECRET,
            "a secret-bearing message goes through encode_into"
        );
    }
    let mut out = Vec::new();
    encode_into(value, &mut out)?;
    Ok(out)
}

/// Encodes `value`, one message of type `T`, into `out`, the caller's
/// buffer, under the descriptor its type carries.
///
/// The contract holds for every message, so that a buffer which once held
/// a secret-bearing message is safe to pass for any other
/// (`docs/specs/xdr.md`, 5.1, point 8, and 5.5, point 3):
///
/// - **The allocation never moves.** The encoder never grows, shrinks,
///   moves or frees the allocation `out` holds. Its one heap request is
///   the whole size, made once before anything is written, and only when
///   `out` has no allocation yet; an allocated `out` too small for the
///   message is refused with `AllocationFailed`. The output is written by
///   `push` alone, within that size. So every byte the encoder writes
///   stays in the one allocation the caller holds.
/// - **Every call starts with a wipe, and so ends unless it succeeds.**
///   Before anything else, the encoder overwrites `out` and its spare
///   capacity with zeros and leaves it empty; on any error, whatever it
///   raises, and when a value's `encode` unwinds, it does so again. On
///   success `out` holds exactly the message.
/// - **A value's unwind moves nothing.** Each byte is counted against
///   the message's size as it is written, in the one count every
///   component's encoder shares, and no write passes the buffer's
///   capacity. So a value that catches a component's unwind and writes
///   on is held to the same size, and the allocation stays in place.
/// - **The wipe that holds is the caller's.** The encoder's zeroing is
///   best effort: ordinary writes to memory that is about to be cleared
///   may be removed by the compiler, and a pure crate cannot write any
///   other way. Since the allocation never moves, the caller's wipe of
///   it with the project's approved wiper (ADR 0005, `mem-wipe`), after
///   use, on error, before reuse and on drop, reaches every copy.
///
/// # Errors
///
/// Any error of [`Encode::encoded_len`]; `MessageTooLarge` naming the
/// message if the size is above its cap, before any output is reserved;
/// `AllocationFailed` if the reservation fails, or `out` has an
/// allocation too small for the message.
pub fn encode_into<T: Encode + MessageType + ?Sized>(
    value: &T,
    out: &mut Vec<u8>,
) -> Result<(), Error> {
    encode_message(&T::MESSAGE, value, out)
}

/// [`encode_into`] under `message`, whatever the value's type.
fn encode_message<T: Encode + ?Sized>(
    message: &Message,
    value: &T,
    out: &mut Vec<u8>,
) -> Result<(), Error> {
    wipe(out);
    let mut output = Output { out, keep: false };
    write(message, value, output.out)?;
    output.keep = true;
    Ok(())
}

/// The caller's buffer while a message is written into it. Unless the
/// message was written whole, leaving it wipes the buffer: on an error,
/// and when a value's `encode` unwinds.
struct Output<'o> {
    out: &'o mut Vec<u8>,
    keep: bool,
}

impl Drop for Output<'_> {
    fn drop(&mut self) {
        if !self.keep {
            wipe(self.out);
        }
    }
}

/// `MessageTooLarge` naming `message`.
fn too_large(message: &Message) -> Error {
    Error::new(ErrorKind::MessageTooLarge)
        .named(message.name)
        .limit(u64::from(message.cap))
}

/// `value`'s encoded size, checked against `message`'s cap.
fn size_within_cap<T: Encode + ?Sized>(message: &Message, value: &T) -> Result<u64, Error> {
    let size = value.encoded_len().map_err(|error| {
        if error.kind() == ErrorKind::MessageTooLarge {
            too_large(message)
        } else {
            error
        }
    })?;
    if size > u64::from(message.cap) {
        return Err(too_large(message));
    }
    Ok(size)
}

/// Sizes `value`, requests the output once, and writes `value` into
/// `out`, which is empty.
fn write<T: Encode + ?Sized>(message: &Message, value: &T, out: &mut Vec<u8>) -> Result<(), Error> {
    let size = size_within_cap(message, value)?;
    let length = usize::try_from(size).map_err(|_| too_large(message))?;
    if out.capacity() != 0 && out.capacity() < length {
        return Err(Error::new(ErrorKind::AllocationFailed));
    }
    #[expect(
        clippy::disallowed_methods,
        reason = "rule 3: the encoder requests its output once, after the size check, into a buffer with no allocation or with room already (docs/specs/xdr.md, 5.5, point 3)"
    )]
    out.try_reserve_exact(length)
        .map_err(|_| Error::new(ErrorKind::AllocationFailed))?;
    let mut remaining = size;
    let mut encoder = Encoder {
        out,
        remaining: &mut remaining,
        message: message.name,
        cap: message.cap,
        value: PhantomData,
    };
    value.encode(&mut encoder)?;
    if *encoder.remaining == 0 {
        Ok(())
    } else {
        Err(encoder.overrun())
    }
}

/// Overwrites every byte `out` holds, and its spare capacity, with zeros,
/// and leaves it empty with the same capacity. Best effort: the compiler
/// may remove these writes, so the caller's wiper is what holds
/// ([`encode_into`]).
fn wipe(out: &mut Vec<u8>) {
    zero_to_capacity(out);
    out.clear();
}

/// Overwrites every byte of `out`'s capacity with zero, leaving it full.
/// The spare bytes are written by `push` up to the capacity captured
/// first: the one way of filling a buffer that Rust documents as never
/// reallocating.
fn zero_to_capacity(out: &mut Vec<u8>) {
    out.fill(0);
    for _ in out.len()..out.capacity() {
        out.push(0);
    }
}

/// The encoded size of a fixed-length opaque of `n` bytes: `n` plus its
/// padding.
///
/// # Errors
///
/// `MessageTooLarge` if the size does not fit in 64 bits.
pub fn fixed_opaque_len(n: usize) -> Result<u64, Error> {
    let n = u64::try_from(n).map_err(|_| sum_overflow())?;
    sum(n, pad(n))
}

/// The encoded size of variable-length opaque data or a string of `n`
/// bytes, after its checks: 4 for the length word, the data and padding.
///
/// # Errors
///
/// `LengthNotRepresentable` if `n` does not fit in 32 bits;
/// `LengthOverBound` above the declared bound; `CapExceeded` above the cap
/// (5.5, point 2).
pub fn opaque_len(cap: &Cap, n: usize) -> Result<u64, Error> {
    let length = word_length(cap, n)?;
    if cap.bound.is_some_and(|bound| length > bound) {
        return Err(over_bound(cap));
    }
    if length > cap.bytes {
        return Err(cap_exceeded(cap));
    }
    let n = u64::from(length);
    sum(4, sum(n, pad(n))?)
}

/// The encoded size of a variable-length array: 4 for the count word and
/// the elements, after its checks. The count is compared with the declared
/// bound, in elements, and the elements' encoded bytes with the cap, in
/// bytes (5.5, point 2).
///
/// # Errors
///
/// `LengthNotRepresentable` if the count does not fit in 32 bits;
/// `LengthOverBound` above the declared bound; any element's error;
/// `CapExceeded` if the elements' bytes are above the cap.
pub fn array_len<T: Encode>(cap: &Cap, elements: &[T]) -> Result<u64, Error> {
    let count = word_length(cap, elements.len())?;
    if cap.bound.is_some_and(|bound| count > bound) {
        return Err(over_bound(cap));
    }
    let bytes = fixed_array_len(elements)?;
    if bytes > u64::from(cap.bytes) {
        return Err(cap_exceeded(cap));
    }
    sum(4, bytes)
}

/// The encoded size of a fixed-length array: the sum of its elements.
///
/// # Errors
///
/// Any element's error; `MessageTooLarge` if the sum overflows.
pub fn fixed_array_len<T: Encode>(elements: &[T]) -> Result<u64, Error> {
    elements
        .iter()
        .try_fold(0, |total, element| sum(total, element.encoded_len()?))
}

/// Adds two encoded sizes (section 6, A10).
///
/// # Errors
///
/// `MessageTooLarge` on overflow; [`encode_into`] names the message.
pub fn sum(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_add(b).ok_or_else(sum_overflow)
}

impl<T: Encode + ?Sized> Encoder<'_, T> {
    /// Writes a signed integer.
    ///
    /// # Errors
    ///
    /// `MessageTooLarge` if the value writes past its computed size.
    pub fn put_i32(&mut self, value: i32) -> Result<(), Error> {
        self.put_bytes(&value.to_be_bytes())
    }

    /// Writes an unsigned integer.
    ///
    /// # Errors
    ///
    /// As [`Encoder::put_i32`].
    pub fn put_u32(&mut self, value: u32) -> Result<(), Error> {
        self.put_bytes(&value.to_be_bytes())
    }

    /// Writes a hyper.
    ///
    /// # Errors
    ///
    /// As [`Encoder::put_i32`].
    pub fn put_i64(&mut self, value: i64) -> Result<(), Error> {
        self.put_bytes(&value.to_be_bytes())
    }

    /// Writes an unsigned hyper.
    ///
    /// # Errors
    ///
    /// As [`Encoder::put_i32`].
    pub fn put_u64(&mut self, value: u64) -> Result<(), Error> {
        self.put_bytes(&value.to_be_bytes())
    }

    /// Writes a boolean as 0 or 1.
    ///
    /// # Errors
    ///
    /// As [`Encoder::put_i32`].
    pub fn put_bool(&mut self, value: bool) -> Result<(), Error> {
        self.put_u32(u32::from(value))
    }

    /// Writes a fixed-length opaque of its declared width `N`: its bytes,
    /// then zero padding. The width is in the type, so data of any other
    /// length does not build. This builds:
    ///
    /// ```
    /// # use ironafs_xdr::{Encode, Encoder, Error};
    /// fn write<T: Encode>(encoder: &mut Encoder<'_, T>, short: &[u8; 96]) -> Result<(), Error> {
    ///     encoder.put_fixed_opaque::<96>(short)
    /// }
    /// ```
    ///
    /// and the same with the one line changed, to a width of 100, does
    /// not:
    ///
    /// ```compile_fail,E0308
    /// # use ironafs_xdr::{Encode, Encoder, Error};
    /// fn write<T: Encode>(encoder: &mut Encoder<'_, T>, short: &[u8; 96]) -> Result<(), Error> {
    ///     encoder.put_fixed_opaque::<100>(short)
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// As [`Encoder::put_i32`].
    pub fn put_fixed_opaque<const N: usize>(&mut self, data: &[u8; N]) -> Result<(), Error> {
        self.put_data(data)
    }

    /// Writes variable-length opaque data or a string: its length word, its
    /// bytes, then zero padding.
    ///
    /// # Errors
    ///
    /// As [`opaque_len`], then as [`Encoder::put_i32`].
    pub fn put_opaque(&mut self, cap: &Cap, data: &[u8]) -> Result<(), Error> {
        opaque_len(cap, data.len())?;
        self.put_u32(word_length(cap, data.len())?)?;
        self.put_data(data)
    }

    /// Writes a component, element or arm of type `C` (its value's
    /// [`Encode::encode`], with an encoder typed for `C`, sharing this
    /// encoder's count of the bytes left).
    ///
    /// A type's flag is at least that of each component it writes
    /// ([`Classified::SECRET`]): this is checked here, when the build
    /// reaches a type that writes a `C`, not trusted. So a wrapper, generic
    /// or not, is secret whenever what it holds is, whatever it declares.
    /// One that takes its flag from what it holds builds:
    ///
    /// ```
    /// # use ironafs_xdr::{Classified, Encode, Encoder, Error, Message, MessageType, encode, encode_into};
    /// # /// A secret-bearing message: a 4-byte key.
    /// # struct Key([u8; 4]);
    /// # impl Classified for Key {
    /// #     const SECRET: bool = true;
    /// # }
    /// # impl MessageType for Key {
    /// #     const MESSAGE: Message = Message { name: "demo.key", cap: 4, budget: 0 };
    /// # }
    /// # impl Encode for Key {
    /// #     fn encoded_len(&self) -> Result<u64, Error> {
    /// #         Ok(4)
    /// #     }
    /// #     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
    /// #         encoder.put_fixed_opaque(&self.0)
    /// #     }
    /// # }
    ///
    /// /// A message that holds one value of `C`.
    /// struct Wrapper<C>(C);
    ///
    /// impl<C: Encode> Classified for Wrapper<C> {
    ///     const SECRET: bool = C::SECRET;
    /// }
    ///
    /// impl<C: Encode> MessageType for Wrapper<C> {
    ///     const MESSAGE: Message = Message { name: "demo.wrapper", cap: 4, budget: 0 };
    /// }
    ///
    /// impl<C: Encode> Encode for Wrapper<C> {
    ///     fn encoded_len(&self) -> Result<u64, Error> {
    ///         self.0.encoded_len()
    ///     }
    ///
    ///     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
    ///         encoder.put(&self.0)
    ///     }
    /// }
    ///
    /// let mut out = Vec::new();
    /// let _ = encode_into(&Wrapper(Key([1, 2, 3, 4])), &mut out);
    /// ```
    ///
    /// and the same with the one line changed, to declare itself plain,
    /// does not:
    ///
    /// ```compile_fail,E0080
    /// # use ironafs_xdr::{Classified, Encode, Encoder, Error, Message, MessageType, encode, encode_into};
    /// # /// A secret-bearing message: a 4-byte key.
    /// # struct Key([u8; 4]);
    /// # impl Classified for Key {
    /// #     const SECRET: bool = true;
    /// # }
    /// # impl MessageType for Key {
    /// #     const MESSAGE: Message = Message { name: "demo.key", cap: 4, budget: 0 };
    /// # }
    /// # impl Encode for Key {
    /// #     fn encoded_len(&self) -> Result<u64, Error> {
    /// #         Ok(4)
    /// #     }
    /// #     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
    /// #         encoder.put_fixed_opaque(&self.0)
    /// #     }
    /// # }
    ///
    /// /// A message that holds one value of `C`.
    /// struct Wrapper<C>(C);
    ///
    /// impl<C: Encode> Classified for Wrapper<C> {
    ///     const SECRET: bool = false;
    /// }
    ///
    /// impl<C: Encode> MessageType for Wrapper<C> {
    ///     const MESSAGE: Message = Message { name: "demo.wrapper", cap: 4, budget: 0 };
    /// }
    ///
    /// impl<C: Encode> Encode for Wrapper<C> {
    ///     fn encoded_len(&self) -> Result<u64, Error> {
    ///         self.0.encoded_len()
    ///     }
    ///
    ///     fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
    ///         encoder.put(&self.0)
    ///     }
    /// }
    ///
    /// let mut out = Vec::new();
    /// let _ = encode_into(&Wrapper(Key([1, 2, 3, 4])), &mut out);
    /// ```
    ///
    /// # Errors
    ///
    /// The component's error.
    pub fn put<C: Encode + ?Sized>(&mut self, component: &C) -> Result<(), Error> {
        const {
            assert!(
                T::SECRET || !C::SECRET,
                "a type that writes a secret component must declare SECRET"
            );
        }
        component.encode(&mut Encoder {
            out: &mut *self.out,
            remaining: &mut *self.remaining,
            message: self.message,
            cap: self.cap,
            value: PhantomData,
        })
    }

    /// Writes a variable-length array: its count word, then each element.
    ///
    /// # Errors
    ///
    /// As [`array_len`], then any element's error.
    pub fn put_array<C: Encode>(&mut self, cap: &Cap, elements: &[C]) -> Result<(), Error> {
        array_len(cap, elements)?;
        self.put_u32(word_length(cap, elements.len())?)?;
        self.put_fixed_array(elements)
    }

    /// Writes a fixed-length array: each element in order, through
    /// [`Encoder::put`].
    ///
    /// # Errors
    ///
    /// Any element's error.
    pub fn put_fixed_array<C: Encode>(&mut self, elements: &[C]) -> Result<(), Error> {
        elements.iter().try_for_each(|element| self.put(element))
    }

    /// Writes `data`, then its zero padding.
    fn put_data(&mut self, data: &[u8]) -> Result<(), Error> {
        self.put_bytes(data)?;
        self.put_padding(data.len())
    }

    fn put_padding(&mut self, n: usize) -> Result<(), Error> {
        let n = u64::try_from(n).map_err(|_| self.overrun())?;
        for _ in 0..pad(n) {
            self.put_bytes(&[0])?;
        }
        Ok(())
    }

    /// Pushes `bytes`, within the size requested for the output and
    /// within the buffer's capacity, so the buffer never grows: `push`
    /// within the capacity never reallocates. The count alone keeps every
    /// write within the size; the capacity is checked as well, so that
    /// the allocation's staying in place does not rest on the count.
    fn put_bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let length = u64::try_from(bytes.len()).map_err(|_| self.overrun())?;
        let left = self
            .remaining
            .checked_sub(length)
            .ok_or_else(|| self.overrun())?;
        let room = self.out.capacity().saturating_sub(self.out.len());
        if bytes.len() > room {
            return Err(self.overrun());
        }
        *self.remaining = left;
        for byte in bytes {
            self.out.push(*byte);
        }
        Ok(())
    }

    /// The error for a value that writes more, or fewer, bytes than its
    /// computed size: a fault in its `Encode` implementation.
    fn overrun(&self) -> Error {
        Error::new(ErrorKind::MessageTooLarge)
            .named(self.message)
            .limit(u64::from(self.cap))
    }
}

/// The padding after `n` data bytes: 0 to 3 (section 6, A1).
const fn pad(n: u64) -> u64 {
    (4 - n % 4) % 4
}

/// A length or count as its 32-bit word (section 6, A7).
fn word_length(cap: &Cap, n: usize) -> Result<u32, Error> {
    u32::try_from(n).map_err(|_| Error::new(ErrorKind::LengthNotRepresentable).named(cap.name))
}

fn over_bound(cap: &Cap) -> Error {
    let bound = cap.bound.map_or(u64::from(u32::MAX), u64::from);
    Error::new(ErrorKind::LengthOverBound)
        .named(cap.name)
        .limit(bound)
}

fn cap_exceeded(cap: &Cap) -> Error {
    Error::new(ErrorKind::CapExceeded)
        .named(cap.name)
        .limit(u64::from(cap.bytes))
}

fn sum_overflow() -> Error {
    Error::new(ErrorKind::MessageTooLarge)
}

macro_rules! encode_number {
    ($type:ty, $size:expr, $put:ident) => {
        impl Encode for $type {
            fn encoded_len(&self) -> Result<u64, Error> {
                Ok($size)
            }

            fn encode(&self, encoder: &mut Encoder<'_, Self>) -> Result<(), Error> {
                encoder.$put(*self)
            }
        }
    };
}

encode_number!(i32, 4, put_i32);
encode_number!(u32, 4, put_u32);
encode_number!(i64, 8, put_i64);
encode_number!(u64, 8, put_u64);

/// Void: nothing on the wire.
impl Encode for () {
    fn encoded_len(&self) -> Result<u64, Error> {
        Ok(0)
    }

    fn encode(&self, _: &mut Encoder<'_, Self>) -> Result<(), Error> {
        Ok(())
    }
}
