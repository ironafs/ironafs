// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Decoding: the bounded cursor, the checks of `docs/specs/xdr.md`, 5.4,
//! in their order, and the entry points.
//!
//! Every byte the decoder reads or copies is taken through the cursor,
//! whose two ends are the tightest limit of the variable-length arrays
//! enclosing the position and the input's end (5.4, point 5). Every length
//! and count is checked against its declared bound, its cap and those ends
//! before anything is reserved, and every reservation goes through the
//! ledger (5.2, point 9). Array elements are decoded by a loop, never by
//! recursion, so a count never sets the stack's depth (5.1, point 4).

use alloc::vec::Vec;
use core::fmt;
use core::mem::size_of;

use crate::limits::{DEPTH_MAX, INLINE_MAX, INLINE_OPAQUE_MAX};
use crate::{Cap, Error, ErrorKind, Message};

mod ledger;

#[cfg(test)]
mod tests;

use ledger::Ledger;
pub use ledger::Reservation;

/// The size of one XDR unit, in bytes.
const UNIT: u64 = 4;

/// A type the codec can decode, as a protocol's generated code defines it.
///
/// The constants are what the generator computes from the definitions
/// (`docs/specs/xdr.md`, 5.1, point 5, and 5.2, points 9 and 10); the
/// entry points and [`proven`] check them at compile time.
pub trait Decode<'a>: Sized {
    /// `s_min`: the fewest bytes any value of the type encodes to.
    const S_MIN: u64;
    /// The nesting depth, counted in structures, unions and arrays.
    const DEPTH: u32;
    /// `r(T)`: an upper bound on the bytes requested from the allocator
    /// per byte of encoding.
    const HEAP_RATIO: u64;
    /// `q(T)`: an upper bound on the reservations made per unit of
    /// encoding.
    const RESERVATIONS: u64;

    /// Decodes one value at the decoder's position.
    ///
    /// # Errors
    ///
    /// Any [`Error`] the profile's checks raise; the first one stops the
    /// decode.
    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error>;
}

/// The end of the input that the elements of a variable-length array may
/// not pass, and the cap it comes from.
#[derive(Clone, Copy, Debug)]
struct Limit {
    end: usize,
    cap: &'static Cap,
}

/// A position in the input, for an error the generated code raises after
/// reading a word, such as a discriminant with no arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark(usize);

/// The decoder's state for one input: the bounded cursor and the ledger.
///
/// In a secret-bearing message the generated code borrows every opaque,
/// string and integer from the input ([`Decoder::opaque`],
/// [`Decoder::fixed_opaque`], [`Decoder::word`], [`Decoder::hyper_word`])
/// and copies nothing (5.1, point 8); the decoder then builds every error
/// without a position.
pub struct Decoder<'a, 'h> {
    input: &'a [u8],
    offset: usize,
    limit: Option<Limit>,
    ledger: Ledger<'h>,
    secret: bool,
}

impl fmt::Debug for Decoder<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Decoder")
            .field("ledger", &self.ledger)
            .finish_non_exhaustive()
    }
}

/// Decodes `input` as one `message` of type `T`.
///
/// A type nested deeper than [`DEPTH_MAX`], or larger inline than
/// [`INLINE_MAX`], does not build here.
///
/// # Errors
///
/// `MessageTooLarge` if the input is longer than the message's cap, before
/// anything is read; `TrailingBytes` if bytes remain after the value; and
/// any error of [`Decode::decode`]. No value, not even part of one, is
/// returned with an error.
pub fn decode<'a, T: Decode<'a>>(message: &Message, input: &'a [u8]) -> Result<T, Error> {
    within_limits::<T>();
    run(
        message,
        input,
        Ledger::new(message.name, message.budget, None),
        T::decode,
    )
}

/// Decodes as [`decode`] does, with the memory budget lowered to `budget`
/// if that is below the message's, and calling `hook` with every
/// reservation the ledger lets through, before it is made.
///
/// This is the test and fuzzing entry point (`docs/specs/xdr.md`, 5.2,
/// point 9.2).
///
/// # Errors
///
/// As [`decode`].
pub fn decode_traced<'a, T: Decode<'a>>(
    message: &Message,
    input: &'a [u8],
    budget: u64,
    hook: &mut dyn FnMut(Reservation),
) -> Result<T, Error> {
    within_limits::<T>();
    let ledger = Ledger::new(message.name, budget.min(message.budget), Some(hook));
    run(message, input, ledger, T::decode)
}

/// Fails the build of an entry point for a type nested deeper than
/// [`DEPTH_MAX`] or larger inline than [`INLINE_MAX`] (5.2, point 10), so
/// no input is recursed into past the depth limit.
const fn within_limits<'a, T: Decode<'a>>() {
    const {
        assert!(T::DEPTH <= DEPTH_MAX, "nested deeper than xdr.depth_max");
        assert!(
            size_of::<T>() <= INLINE_MAX,
            "larger inline than xdr.inline_max"
        );
    }
}

/// Whether `T`'s worst case fits `message`: its nesting depth and inline
/// size are within the profile's limits, and its heap ratio times the
/// message cap is within the memory budget (5.2, points 9.3 and 10). The
/// generated code asserts it in a constant for each message, so a message
/// that does not fit does not build on that target.
///
/// The product saturates, and a saturated product is above any budget, so
/// an overflow fails the assertion too.
#[must_use]
pub const fn proven<'a, T: Decode<'a>>(message: &Message) -> bool {
    T::DEPTH <= DEPTH_MAX
        && size_of::<T>() <= INLINE_MAX
        && T::HEAP_RATIO.saturating_mul(message.cap as u64) <= message.budget
}

/// `r` of a variable-length array of `T`: its element slots, each
/// `size_of::<T>()`, per byte of the at least `T::S_MIN` bytes each element
/// encodes to, plus `r(T)` (5.2, point 9.3). Saturates, failing
/// [`proven`], on overflow.
#[must_use]
pub const fn array_heap_ratio<'a, T: Decode<'a>>() -> u64 {
    let slot = size_of::<T>() as u64;
    if T::S_MIN == 0 {
        return u64::MAX;
    }
    slot.div_ceil(T::S_MIN).saturating_add(T::HEAP_RATIO)
}

/// `q` of a variable-length array of `T`: one reservation for its buffer,
/// or its elements' (5.2, point 9.5).
#[must_use]
pub const fn array_reservations<'a, T: Decode<'a>>() -> u64 {
    max(1, T::RESERVATIONS)
}

/// The larger of two constants, for the generated code's `r` and `q` of
/// structures and unions.
#[must_use]
pub const fn max(a: u64, b: u64) -> u64 {
    if a > b { a } else { b }
}

/// Runs `decode_value` over `input` as one message.
pub(crate) fn run<'a, 'h, T>(
    message: &Message,
    input: &'a [u8],
    ledger: Ledger<'h>,
    decode_value: impl FnOnce(&mut Decoder<'a, 'h>) -> Result<T, Error>,
) -> Result<T, Error> {
    let mut decoder = Decoder {
        input,
        offset: 0,
        limit: None,
        ledger,
        secret: message.secret,
    };
    if u64::try_from(input.len()).map_or(true, |length| length > u64::from(message.cap)) {
        let error = Error::new(ErrorKind::MessageTooLarge)
            .named(message.name)
            .limit(u64::from(message.cap));
        return Err(decoder.locate(error, 0, Some(input.len())));
    }
    let value = decode_value(&mut decoder)?;
    let remaining = decoder.remaining();
    if remaining > 0 {
        let error = Error::new(ErrorKind::TrailingBytes);
        return Err(decoder.locate(error, decoder.offset, Some(remaining)));
    }
    Ok(value)
}

impl<'a> Decoder<'a, '_> {
    /// The current position, for an error raised after reading.
    #[must_use]
    pub const fn mark(&self) -> Mark {
        Mark(self.offset)
    }

    /// The error for a union whose discriminant, read at `at`, selects no
    /// arm and has no default arm (5.4, point 7, step 2).
    #[must_use]
    pub fn unknown_discriminant(&self, field: &'static str, at: Mark) -> Error {
        let error = Error::new(ErrorKind::UnknownDiscriminant).named(field);
        self.locate(error, at.0, None)
    }

    /// Reads a signed integer (5.1, point 1).
    ///
    /// # Errors
    ///
    /// `CapExceeded` if its bytes would pass an enclosing array's limit,
    /// else `Truncated` if they would pass the input's end.
    pub fn read_i32(&mut self) -> Result<i32, Error> {
        self.word().map(|word| i32::from_be_bytes(*word))
    }

    /// Reads an unsigned integer.
    ///
    /// # Errors
    ///
    /// As [`Decoder::read_i32`].
    pub fn read_u32(&mut self) -> Result<u32, Error> {
        self.word().map(|word| u32::from_be_bytes(*word))
    }

    /// Reads a hyper.
    ///
    /// # Errors
    ///
    /// As [`Decoder::read_i32`].
    pub fn read_i64(&mut self) -> Result<i64, Error> {
        self.hyper_word().map(|word| i64::from_be_bytes(*word))
    }

    /// Reads an unsigned hyper.
    ///
    /// # Errors
    ///
    /// As [`Decoder::read_i32`].
    pub fn read_u64(&mut self) -> Result<u64, Error> {
        self.hyper_word().map(|word| u64::from_be_bytes(*word))
    }

    /// Borrows the 4 bytes of an integer or enumeration from the input,
    /// for a secret-bearing message (5.1, point 8).
    ///
    /// # Errors
    ///
    /// As [`Decoder::read_i32`].
    pub fn word(&mut self) -> Result<&'a [u8; 4], Error> {
        self.take_array()
    }

    /// Borrows the 8 bytes of a hyper from the input.
    ///
    /// # Errors
    ///
    /// As [`Decoder::read_i32`].
    pub fn hyper_word(&mut self) -> Result<&'a [u8; 8], Error> {
        self.take_array()
    }

    /// Reads a boolean: 0 is false and 1 is true (5.4, point 9).
    ///
    /// # Errors
    ///
    /// As [`Decoder::read_i32`], and `InvalidBool` naming `field` for any
    /// other value.
    pub fn read_bool(&mut self, field: &'static str) -> Result<bool, Error> {
        let at = self.offset;
        match self.read_u32()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.locate(Error::new(ErrorKind::InvalidBool).named(field), at, None)),
        }
    }

    /// Reads an enumeration and maps it through `declared`, which returns
    /// `None` for a value the enumeration does not declare (5.4, point 9).
    /// An open enumeration's `declared` keeps every value.
    ///
    /// # Errors
    ///
    /// As [`Decoder::read_i32`], and `UnknownEnumValue` naming `field` when
    /// `declared` returns `None`.
    pub fn read_enum<E>(
        &mut self,
        field: &'static str,
        declared: impl FnOnce(i32) -> Option<E>,
    ) -> Result<E, Error> {
        let at = self.offset;
        let value = self.read_i32()?;
        declared(value).ok_or_else(|| {
            self.locate(
                Error::new(ErrorKind::UnknownEnumValue).named(field),
                at,
                None,
            )
        })
    }

    /// Borrows a fixed-length opaque of `n` bytes and checks its padding.
    ///
    /// # Errors
    ///
    /// `CapExceeded` or `Truncated` if the item, padding included, would
    /// pass an end of the cursor, before anything is read; `NonZeroPadding`
    /// at the first padding byte that is not zero.
    pub fn fixed_opaque(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let start = self.offset;
        let padded = padded_len(n).ok_or_else(|| self.truncated(start))?;
        self.check_end(start, start.checked_add(padded))?;
        let data = self.take(n)?;
        self.padding(n)?;
        Ok(data)
    }

    /// Copies a fixed-length opaque of `N` bytes, held inline, which the
    /// profile allows only up to
    /// [`INLINE_OPAQUE_MAX`](crate::limits::INLINE_OPAQUE_MAX) bytes
    /// (5.2, point 10.2); a longer `N` does not build.
    ///
    /// # Errors
    ///
    /// As [`Decoder::fixed_opaque`].
    pub fn fixed_opaque_inline<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        const { assert!(N <= INLINE_OPAQUE_MAX) };
        let start = self.offset;
        let data = self.fixed_opaque(N)?;
        data.first_chunk::<N>()
            .copied()
            .ok_or_else(|| self.truncated(start))
    }

    /// Copies a fixed-length opaque of `n` bytes into a buffer reserved
    /// through the ledger: how a fixed-length opaque longer than
    /// [`INLINE_OPAQUE_MAX`](crate::limits::INLINE_OPAQUE_MAX) bytes is
    /// held out of line (5.2, point 10.2). `field` names the request.
    ///
    /// # Errors
    ///
    /// `CapExceeded` or `Truncated` if the item would pass an end of the
    /// cursor, checked before the buffer is reserved (5.4, point 2);
    /// `BudgetExceeded` or `AllocationFailed` from the reservation;
    /// `NonZeroPadding` as [`Decoder::fixed_opaque`].
    pub fn fixed_opaque_owned(&mut self, n: usize, field: &'static str) -> Result<Vec<u8>, Error> {
        let start = self.offset;
        let padded = padded_len(n).ok_or_else(|| self.truncated(start))?;
        self.check_end(start, start.checked_add(padded))?;
        let buffer = self.reserve(n, field, start)?;
        self.copy_into(buffer, n)
    }

    /// Borrows variable-length opaque data or a string from the input,
    /// after its checks in order (5.4, point 3).
    ///
    /// # Errors
    ///
    /// In this order: `CapExceeded` or `Truncated` for the length word
    /// itself; `LengthOverBound` above the declared bound; `CapExceeded`
    /// above the cap, naming it, or past an enclosing array's limit,
    /// naming that array's cap; `Truncated` if the input ends first;
    /// `NonZeroPadding`.
    pub fn opaque(&mut self, cap: &'static Cap) -> Result<&'a [u8], Error> {
        let n = self.length(cap)?;
        let data = self.take(n)?;
        self.padding(n)?;
        Ok(data)
    }

    /// Copies variable-length opaque data or a string into a buffer of
    /// exactly its length, reserved through the ledger once every check
    /// before it has passed (5.4, point 3, step 5).
    ///
    /// # Errors
    ///
    /// As [`Decoder::opaque`], with `BudgetExceeded` or `AllocationFailed`
    /// from the reservation before `NonZeroPadding`.
    pub fn opaque_owned(&mut self, cap: &'static Cap) -> Result<Vec<u8>, Error> {
        let start = self.offset;
        let n = self.length(cap)?;
        let buffer = self.reserve(n, cap.name, start)?;
        self.copy_into(buffer, n)
    }

    /// Decodes a variable-length array of `T`, after its checks in order
    /// (5.4, point 4), then its elements by a loop within the array's
    /// limit (5.4, point 5).
    ///
    /// A `T` that encodes to no bytes may not be an array element (5.1,
    /// point 5); such an array does not build.
    ///
    /// # Errors
    ///
    /// In this order: `CapExceeded` or `Truncated` for the count word;
    /// `LengthOverBound` above the declared bound; `CapExceeded` if the
    /// count times `T::S_MIN` is above the cap or would pass an enclosing
    /// limit; `Truncated` if it is above the bytes remaining;
    /// `BudgetExceeded` or `AllocationFailed` from the reservation of the
    /// element slots; then any element's error.
    pub fn array<T: Decode<'a>>(&mut self, cap: &'static Cap) -> Result<Vec<T>, Error> {
        const { assert!(T::S_MIN >= UNIT) };
        let start = self.offset;
        let count = self.read_u32()?;
        if cap.bound.is_some_and(|bound| count > bound) {
            return Err(self.over_bound(cap, start));
        }
        let at_least = u64::from(count)
            .checked_mul(T::S_MIN)
            .filter(|bytes| *bytes <= u64::from(cap.bytes))
            .ok_or_else(|| self.cap_exceeded(cap, start))?;
        self.check_item_end(start, at_least)?;
        let count = usize::try_from(count).map_err(|_| self.cap_exceeded(cap, start))?;
        let mut elements = self.reserve(count, cap.name, start)?;
        let array_end = usize::try_from(cap.bytes)
            .ok()
            .and_then(|bytes| self.offset.checked_add(bytes))
            .ok_or_else(|| self.cap_exceeded(cap, start))?;
        let enclosing = self.limit;
        if enclosing.is_none_or(|limit| array_end <= limit.end) {
            self.limit = Some(Limit {
                end: array_end,
                cap,
            });
        }
        let mut result = Ok(());
        for _ in 0..count {
            match T::decode(self) {
                Ok(element) => elements.push(element),
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        self.limit = enclosing;
        result.map(|()| elements)
    }

    /// Decodes a fixed-length array of `N` elements of `T`, held inline.
    /// On entry it checks that `N` times `T::S_MIN` bytes fit before an
    /// end of the cursor (5.4, point 2); the elements are then decoded by
    /// a loop.
    ///
    /// # Errors
    ///
    /// `CapExceeded` or `Truncated` on entry, then the first element's
    /// error.
    pub fn fixed_array<T: Decode<'a> + Default, const N: usize>(
        &mut self,
    ) -> Result<[T; N], Error> {
        const { assert!(T::S_MIN >= UNIT) };
        let start = self.offset;
        let at_least = u64::try_from(N)
            .ok()
            .and_then(|n| n.checked_mul(T::S_MIN))
            .ok_or_else(|| self.truncated(start))?;
        self.check_item_end(start, at_least)?;
        let mut failure = None;
        let elements = core::array::from_fn(|_| {
            if failure.is_some() {
                return T::default();
            }
            T::decode(self).unwrap_or_else(|error| {
                failure = Some(error);
                T::default()
            })
        });
        failure.map_or(Ok(elements), Err)
    }

    /// Reads a length word and runs the checks of 5.4, point 3, steps 1 to
    /// 4, returning the data length.
    fn length(&mut self, cap: &'static Cap) -> Result<usize, Error> {
        let start = self.offset;
        let n = self.read_u32()?;
        if cap.bound.is_some_and(|bound| n > bound) {
            return Err(self.over_bound(cap, start));
        }
        if n > cap.bytes {
            return Err(self.cap_exceeded(cap, start));
        }
        let n = usize::try_from(n).map_err(|_| self.cap_exceeded(cap, start))?;
        let padded = padded_len(n).ok_or_else(|| self.cap_exceeded(cap, start))?;
        let end = self.offset.checked_add(padded);
        self.check_end(start, end)?;
        Ok(n)
    }

    /// Checks that `at_least` bytes from the current offset fit before both
    /// ends of the cursor, for an item that starts at `start`.
    fn check_item_end(&self, start: usize, at_least: u64) -> Result<(), Error> {
        let end = usize::try_from(at_least)
            .ok()
            .and_then(|bytes| self.offset.checked_add(bytes));
        self.check_end(start, end)
    }

    /// Checks an item that starts at `start` and would end at `end` (`None`
    /// past `usize::MAX`) against the cursor's two ends: the tightest
    /// array limit first, then the input's end (5.4, point 5).
    fn check_end(&self, start: usize, end: Option<usize>) -> Result<(), Error> {
        if let Some(limit) = self.limit
            && end.is_none_or(|end| end > limit.end)
        {
            return Err(self.cap_exceeded(limit.cap, start));
        }
        if end.is_none_or(|end| end > self.input.len()) {
            return Err(self.truncated(start));
        }
        Ok(())
    }

    /// Takes `n` bytes through the cursor.
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let start = self.offset;
        let end = start.checked_add(n);
        self.check_end(start, end)?;
        let bytes = end
            .and_then(|end| self.input.get(start..end))
            .ok_or_else(|| self.truncated(start))?;
        self.offset = start.saturating_add(n);
        Ok(bytes)
    }

    /// Takes `N` bytes through the cursor, as an array.
    fn take_array<const N: usize>(&mut self) -> Result<&'a [u8; N], Error> {
        let start = self.offset;
        self.take(N)?
            .first_chunk::<N>()
            .ok_or_else(|| self.truncated(start))
    }

    /// Takes the padding after `n` data bytes and checks it is zero
    /// (5.4, point 8).
    fn padding(&mut self, n: usize) -> Result<(), Error> {
        let start = self.offset;
        let padding = self.take(pad(n))?;
        match padding.iter().position(|byte| *byte != 0) {
            Some(index) => {
                let at = start.saturating_add(index);
                Err(self.locate(Error::new(ErrorKind::NonZeroPadding), at, None))
            }
            None => Ok(()),
        }
    }

    /// Fills `buffer`, reserved for `n` bytes, by `push` from the next `n`
    /// input bytes, then checks the padding.
    fn copy_into(&mut self, mut buffer: Vec<u8>, n: usize) -> Result<Vec<u8>, Error> {
        for byte in self.take(n)? {
            buffer.push(*byte);
        }
        self.padding(n)?;
        Ok(buffer)
    }

    /// Reserves `count` slots of `T` through the ledger, for the item whose
    /// length or count word is at `at`.
    fn reserve<T>(&mut self, count: usize, name: &'static str, at: usize) -> Result<Vec<T>, Error> {
        self.ledger
            .reserve(count, name)
            .map_err(|error| self.locate(error, at, None))
    }

    fn remaining(&self) -> usize {
        self.input.len().saturating_sub(self.offset)
    }

    fn over_bound(&self, cap: &'static Cap, at: usize) -> Error {
        let bound = cap.bound.map_or(u64::from(u32::MAX), u64::from);
        let error = Error::new(ErrorKind::LengthOverBound)
            .named(cap.name)
            .limit(bound);
        self.locate(error, at, None)
    }

    fn cap_exceeded(&self, cap: &'static Cap, at: usize) -> Error {
        let error = Error::new(ErrorKind::CapExceeded)
            .named(cap.name)
            .limit(u64::from(cap.bytes));
        self.locate(error, at, None)
    }

    fn truncated(&self, at: usize) -> Error {
        let available = self.input.len().saturating_sub(at);
        self.locate(Error::new(ErrorKind::Truncated), at, Some(available))
    }

    /// Adds the position to `error`, except in a secret-bearing message,
    /// where an offset or a length could reflect a secret's bytes (section
    /// 7, point 2).
    fn locate(&self, error: Error, offset: usize, extent: Option<usize>) -> Error {
        if self.secret {
            error
        } else {
            error.at(offset, extent)
        }
    }
}

/// The padding after `n` data bytes: 0 to 3 (section 6, A1).
const fn pad(n: usize) -> usize {
    (4 - n % 4) % 4
}

/// `n` plus its padding, or `None` on overflow (section 6, A2).
const fn padded_len(n: usize) -> Option<usize> {
    n.checked_add(pad(n))
}

macro_rules! decode_number {
    ($type:ty, $size:expr, $read:ident) => {
        impl<'a> Decode<'a> for $type {
            const S_MIN: u64 = $size;
            const DEPTH: u32 = 0;
            const HEAP_RATIO: u64 = 0;
            const RESERVATIONS: u64 = 0;

            fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error> {
                decoder.$read()
            }
        }
    };
}

decode_number!(i32, 4, read_i32);
decode_number!(u32, 4, read_u32);
decode_number!(i64, 8, read_i64);
decode_number!(u64, 8, read_u64);

/// Void: nothing on the wire (5.1, point 1).
impl<'a> Decode<'a> for () {
    const S_MIN: u64 = 0;
    const DEPTH: u32 = 0;
    const HEAP_RATIO: u64 = 0;
    const RESERVATIONS: u64 = 0;

    fn decode(_: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        Ok(())
    }
}
