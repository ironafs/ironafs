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

use crate::{Cap, Error, ErrorKind, Message};

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
#[derive(Debug)]
pub struct Encoder<'o> {
    out: &'o mut Vec<u8>,
    remaining: u64,
    message: &'static str,
    cap: u32,
}

/// Encodes `value` as one `message` into a new buffer.
///
/// # Errors
///
/// As [`encode_into`].
pub fn encode<T: Encode + ?Sized>(message: &Message, value: &T) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    encode_into(message, value, &mut out)?;
    Ok(out)
}

/// Encodes `value` as one `message` into `out`, which is cleared first.
///
/// The whole size is requested once, before anything is written, so a
/// buffer the caller zeroes on drop holds the only copy of a secret-bearing
/// message (`docs/specs/xdr.md`, 5.1, point 8). On error `out` is left
/// empty, and nothing from it may be sent.
///
/// # Errors
///
/// Any error of [`Encode::encoded_len`]; `MessageTooLarge` naming the
/// message if the size is above its cap, before any output is reserved;
/// `AllocationFailed` if the reservation fails.
pub fn encode_into<T: Encode + ?Sized>(
    message: &Message,
    value: &T,
    out: &mut Vec<u8>,
) -> Result<(), Error> {
    out.clear();
    let too_large = Error::new(ErrorKind::MessageTooLarge)
        .named(message.name)
        .limit(u64::from(message.cap));
    let size = value.encoded_len().map_err(|error| {
        if error.kind() == ErrorKind::MessageTooLarge {
            too_large
        } else {
            error
        }
    })?;
    if size > u64::from(message.cap) {
        return Err(too_large);
    }
    let length = usize::try_from(size).map_err(|_| too_large)?;
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
    let written = value.encode(&mut encoder).and_then(|()| {
        if encoder.remaining == 0 {
            Ok(())
        } else {
            Err(encoder.overrun())
        }
    });
    if written.is_err() {
        encoder.out.clear();
    }
    written
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

    /// Writes a fixed-length opaque: its bytes, then zero padding.
    ///
    /// # Errors
    ///
    /// As [`Encoder::put_i32`].
    pub fn put_fixed_opaque(&mut self, data: &[u8]) -> Result<(), Error> {
        self.put_bytes(data)?;
        self.put_padding(data.len())
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
        self.put_fixed_opaque(data)
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
