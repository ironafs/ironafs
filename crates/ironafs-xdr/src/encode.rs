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

use crate::{Cap, Error, ErrorKind, Message, MessageType};

#[cfg(test)]
mod tests;

/// A type the codec can encode, as a protocol's generated code defines it.
pub trait Encode {
    /// The value's encoded size in bytes.
    ///
    /// # Errors
    ///
    /// `LengthNotRepresentable`, `LengthOverBound` or `CapExceeded` for an
    /// item the decoder would reject; `MessageTooLarge` if the sum
    /// overflows.
    fn encoded_len(&self) -> Result<u64, Error>;

    /// Writes the value.
    ///
    /// # Errors
    ///
    /// As [`Encode::encoded_len`], and `MessageTooLarge` if the value
    /// writes more bytes than its size.
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error>;
}

/// The output of one message: its buffer, and the bytes it may still take.
///
/// Its `Debug` shows the message's name only: neither the output, which
/// may hold a secret-bearing message, nor the bytes still to write, which
/// would tell a secret field's length.
pub struct Encoder<'o> {
    out: &'o mut Vec<u8>,
    remaining: u64,
    message: &'static str,
    cap: u32,
}

impl fmt::Debug for Encoder<'_> {
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
/// descriptor says it holds a secret does not build here: [`encode_into`],
/// with a buffer the caller wipes, is its only path (`docs/specs/xdr.md`,
/// 5.1, point 8):
///
/// ```compile_fail
/// use ironafs_xdr::{Encode, Encoder, Error, Message, MessageType, encode};
///
/// /// A secret-bearing message: a 4-byte key.
/// struct Key([u8; 4]);
///
/// impl MessageType for Key {
///     const MESSAGE: Message = Message { name: "demo.key", cap: 4, budget: 0, secret: true };
/// }
///
/// impl Encode for Key {
///     fn encoded_len(&self) -> Result<u64, Error> {
///         Ok(4)
///     }
///
///     fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
///         encoder.put_fixed_opaque(&self.0)
///     }
/// }
///
/// let _ = encode(&Key([1, 2, 3, 4]));
/// ```
///
/// # Errors
///
/// As [`encode_into`].
pub fn encode<T: Encode + MessageType>(value: &T) -> Result<Vec<u8>, Error> {
    const {
        assert!(
            !T::MESSAGE.secret,
            "a secret-bearing message goes through encode_into"
        );
    }
    let mut out = Vec::new();
    encode_into(&T::MESSAGE, value, &mut out)?;
    Ok(out)
}

/// Encodes `value` as one `message` into `out`, the caller's buffer.
///
/// For a secret-bearing message (`docs/specs/xdr.md`, 5.1, point 8, and
/// 5.5, point 3) the secret bytes never move:
///
/// - **No reallocation.** The whole size is requested once, before
///   anything is written, and only where that moves nothing: `out` has no
///   allocation yet, or already has room for the whole message. A buffer
///   that has an allocation too small for the message is refused with
///   `AllocationFailed` and left as it was, since growing it would move its
///   bytes and release the old block unwiped. The output is written by
///   `push` alone, so the buffer never grows.
/// - **Wiping is the caller's.** Before writing, and on any error, the
///   encoder overwrites the buffer and its spare capacity with zeros and
///   leaves it empty. That is best effort only: ordinary writes to memory
///   that is about to be cleared may be removed by the compiler, and a
///   pure crate cannot write any other way. The caller wipes `out` with
///   the project's approved wiper (ADR 0005, `mem-wipe`) after use, on
///   error and on drop.
///
/// For any other message the buffer may also grow, once, when it is too
/// small.
///
/// # Errors
///
/// Any error of [`Encode::encoded_len`]; `MessageTooLarge` naming the
/// message if the size is above its cap, before any output is reserved;
/// `AllocationFailed` if the reservation fails, or for a secret-bearing
/// message would move the buffer.
pub fn encode_into<T: Encode + ?Sized>(
    message: &Message,
    value: &T,
    out: &mut Vec<u8>,
) -> Result<(), Error> {
    let size = size_within_cap(message, value)?;
    let length = usize::try_from(size).map_err(|_| too_large(message))?;
    if message.secret && out.capacity() != 0 && out.capacity() < length {
        return Err(Error::new(ErrorKind::AllocationFailed));
    }
    wipe(out);
    let written = write(message, value, out, size, length);
    if written.is_err() {
        wipe(out);
    }
    written
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

/// Requests the output once and writes `value`, of `size` bytes, into
/// `out`, which is empty.
fn write<T: Encode + ?Sized>(
    message: &Message,
    value: &T,
    out: &mut Vec<u8>,
    size: u64,
    length: usize,
) -> Result<(), Error> {
    #[expect(
        clippy::disallowed_methods,
        reason = "rule 3: the encoder requests its output once, after the size check (docs/specs/xdr.md, 5.5, point 3)"
    )]
    out.try_reserve_exact(length)
        .map_err(|_| Error::new(ErrorKind::AllocationFailed))?;
    let mut encoder = Encoder {
        out,
        remaining: size,
        message: message.name,
        cap: message.cap,
    };
    value.encode(&mut encoder)?;
    if encoder.remaining == 0 {
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
fn zero_to_capacity(out: &mut Vec<u8>) {
    out.fill(0);
    let capacity = out.capacity();
    #[expect(
        clippy::disallowed_methods,
        reason = "rule 8: resizing within the capacity writes zeros over the spare bytes and never reallocates"
    )]
    out.resize(capacity, 0);
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

impl Encoder<'_> {
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
    /// length does not build:
    ///
    /// ```compile_fail
    /// # use ironafs_xdr::{Encoder, Error};
    /// fn write(encoder: &mut Encoder<'_>, short: &[u8; 96]) -> Result<(), Error> {
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

    /// Writes a variable-length array: its count word, then each element.
    ///
    /// # Errors
    ///
    /// As [`array_len`], then any element's error.
    pub fn put_array<T: Encode>(&mut self, cap: &Cap, elements: &[T]) -> Result<(), Error> {
        array_len(cap, elements)?;
        self.put_u32(word_length(cap, elements.len())?)?;
        self.put_fixed_array(elements)
    }

    /// Writes a fixed-length array: each element in order.
    ///
    /// # Errors
    ///
    /// Any element's error.
    pub fn put_fixed_array<T: Encode>(&mut self, elements: &[T]) -> Result<(), Error> {
        elements.iter().try_for_each(|element| element.encode(self))
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

    /// Pushes `bytes`, within the size requested for the output, so the
    /// buffer never grows.
    fn put_bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let length = u64::try_from(bytes.len()).map_err(|_| self.overrun())?;
        self.remaining = self
            .remaining
            .checked_sub(length)
            .ok_or_else(|| self.overrun())?;
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

            fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
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

    fn encode(&self, _: &mut Encoder<'_>) -> Result<(), Error> {
        Ok(())
    }
}
