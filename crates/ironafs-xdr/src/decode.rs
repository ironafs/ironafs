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

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use core::mem::size_of;
use core::ops::Deref;

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
    /// per byte of encoding, or `None` where computing it overflowed,
    /// which [`proven`] refuses (section 6, A8).
    const HEAP_RATIO: Option<u64>;
    /// `q(T)`: an upper bound on the reservations made per unit of
    /// encoding, or `None` where computing it overflowed.
    const RESERVATIONS: Option<u64>;

    /// Decodes one value at the decoder's position.
    ///
    /// # Errors
    ///
    /// Any [`Error`] the profile's checks raise; the first one stops the
    /// decode.
    fn decode(decoder: &mut Decoder<'a, '_>) -> Result<Self, Error>;
}

/// A value held out of line, in a one-element buffer reserved through the
/// ledger (`docs/specs/xdr.md`, 5.2, point 10.2), so that its parent's
/// inline size stays small whichever arm or component it is.
#[derive(Clone, PartialEq, Eq)]
pub struct OutOfLine<T>(Box<[T; 1]>);

impl<T> Deref for OutOfLine<T> {
    type Target = T;

    fn deref(&self) -> &T {
        let [value] = &*self.0;
        value
    }
}

impl<T: fmt::Debug> fmt::Debug for OutOfLine<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, formatter)
    }
}

/// The end of the input that the elements of a variable-length array may
/// not pass, and the cap it comes from.
#[derive(Clone, Copy, Debug)]
struct Limit {
    /// An absolute offset, in 64 bits so that a cap up to 2³² − 1 adds to
    /// any offset on every target (section 6, A5).
    end: u64,
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
/// size are within the profile's limits, its heap ratio times the message
/// cap is within the memory budget, and its reservation count over the
/// message cap is computable (5.2, points 9.3, 9.5 and 10). The generated
/// code asserts it in a constant for each message, so a message that does
/// not fit does not build on that target. A ratio whose computation
/// overflowed, or a product that overflows, is not proven (section 6, A8).
#[must_use]
pub const fn proven<'a, T: Decode<'a>>(message: &Message) -> bool {
    let cap = message.cap as u64;
    let Some(heap_ratio) = T::HEAP_RATIO else {
        return false;
    };
    let Some(reservations) = T::RESERVATIONS else {
        return false;
    };
    let heap_fits = match heap_ratio.checked_mul(cap) {
        Some(worst) => worst <= message.budget,
        None => false,
    };
    T::DEPTH <= DEPTH_MAX
        && size_of::<T>() <= INLINE_MAX
        && heap_fits
        && reservations.checked_mul(cap).is_some()
}

/// `r` of a variable-length array of `T`: its element slots, each
/// `size_of::<T>()`, per byte of the at least `T::S_MIN` bytes each element
/// encodes to, plus `r(T)` (5.2, point 9.3). `None` on overflow, or for an
/// element that encodes to no bytes.
#[must_use]
pub const fn array_heap_ratio<'a, T: Decode<'a>>() -> Option<u64> {
    slot_ratio::<T>(T::HEAP_RATIO)
}

/// `q` of a variable-length array of `T`: one reservation for its buffer,
/// or its elements' (5.2, point 9.5).
#[must_use]
pub const fn array_reservations<'a, T: Decode<'a>>() -> Option<u64> {
    max(Some(1), T::RESERVATIONS)
}

/// `r` of a fixed-length array of `T`: its element slots, reserved through
/// the ledger while it is decoded, as for a variable-length array.
#[must_use]
pub const fn fixed_array_heap_ratio<'a, T: Decode<'a>>() -> Option<u64> {
    slot_ratio::<T>(T::HEAP_RATIO)
}

/// `q` of a fixed-length array of `T`: one reservation for its slots, which
/// no count word backs, plus its elements'. Its elements are at least one
/// unit each, so per unit that is at most 1 + `q(T)`.
#[must_use]
pub const fn fixed_array_reservations<'a, T: Decode<'a>>() -> Option<u64> {
    out_of_line_reservations::<T>()
}

/// `r` of a value of `T` held out of line: a one-element array
/// (5.2, point 9.3).
#[must_use]
pub const fn out_of_line_heap_ratio<'a, T: Decode<'a>>() -> Option<u64> {
    slot_ratio::<T>(T::HEAP_RATIO)
}

/// `q` of a value of `T` held out of line: its own slot, and its
/// reservations (5.2, point 9.5).
#[must_use]
pub const fn out_of_line_reservations<'a, T: Decode<'a>>() -> Option<u64> {
    match T::RESERVATIONS {
        Some(reservations) => reservations.checked_add(1),
        None => None,
    }
}

/// ⌈`size_of::<T>()` ÷ `T::S_MIN`⌉ + `inner`, checked.
const fn slot_ratio<'a, T: Decode<'a>>(inner: Option<u64>) -> Option<u64> {
    let slot = size_of::<T>() as u64;
    match (inner, slot.checked_div(T::S_MIN)) {
        (Some(inner), Some(_)) => slot.div_ceil(T::S_MIN).checked_add(inner),
        _ => None,
    }
}

/// The larger of two ratios, for the generated code's `r` and `q` of
/// structures and unions; `None` if either is.
#[must_use]
pub const fn max(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if a > b { a } else { b }),
        _ => None,
    }
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

    /// Borrows a fixed-length opaque of `N` bytes and checks its padding.
    ///
    /// # Errors
    ///
    /// `CapExceeded` or `Truncated` if the item, padding included, would
    /// pass an end of the cursor, before anything is read; `NonZeroPadding`
    /// at the first padding byte that is not zero.
    pub fn fixed_opaque<const N: usize>(&mut self) -> Result<&'a [u8; N], Error> {
        let start = self.offset;
        self.check_item_end(start, padded_len(offset64(N)))?;
        let data = self.take_array::<N>()?;
        self.padding(N)?;
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
        self.fixed_opaque::<N>().copied()
    }

    /// Copies a fixed-length opaque of `N` bytes into a buffer reserved
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
    pub fn fixed_opaque_owned<const N: usize>(
        &mut self,
        field: &'static str,
    ) -> Result<Box<[u8; N]>, Error> {
        let start = self.offset;
        self.check_item_end(start, padded_len(offset64(N)))?;
        let buffer = self.reserve(N, field, start)?;
        let buffer = self.copy_into(buffer, N)?;
        self.boxed(buffer, start)
    }

    /// Decodes a value of `T` held out of line (5.2, point 10.2): it
    /// checks that `T::S_MIN` bytes fit before both ends of the cursor,
    /// then reserves one slot through the ledger, named `field`, then
    /// decodes the value into it (5.4, point 2). No count word is read.
    ///
    /// A `T` that encodes to no bytes may not be held out of line; such a
    /// call does not build.
    ///
    /// # Errors
    ///
    /// `CapExceeded` or `Truncated` on entry; `BudgetExceeded` or
    /// `AllocationFailed` from the reservation; then the value's error.
    pub fn out_of_line<T: Decode<'a>>(
        &mut self,
        field: &'static str,
    ) -> Result<OutOfLine<T>, Error> {
        const { assert!(T::S_MIN >= UNIT) };
        let start = self.offset;
        self.check_item_end(start, Some(T::S_MIN))?;
        let mut slot = self.reserve(1, field, start)?;
        slot.push(T::decode(self)?);
        self.boxed(slot, start).map(OutOfLine)
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
        self.check_item_end(start, Some(at_least))?;
        let count = usize::try_from(count).map_err(|_| self.cap_exceeded(cap, start))?;
        let mut elements = self.reserve(count, cap.name, start)?;
        let array_end = offset64(self.offset)
            .checked_add(u64::from(cap.bytes))
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

    /// Decodes a fixed-length array of `N` elements of `T`. On entry it
    /// checks that `N` times `T::S_MIN` bytes fit before an end of the
    /// cursor (5.4, point 2); it then reserves `N` slots through the
    /// ledger, named `field`, decodes the elements into them by a loop that
    /// stops at the first error, and moves them into the array, which is
    /// held inline. No element is ever made up, so any element type will
    /// do. The slots count against the memory budget, so the generated `r`
    /// and `q` of a fixed-length array come from [`fixed_array_heap_ratio`]
    /// and [`fixed_array_reservations`]. An array of no elements reserves
    /// nothing.
    ///
    /// # Errors
    ///
    /// `CapExceeded` or `Truncated` on entry; `BudgetExceeded` or
    /// `AllocationFailed` from the reservation; then the first element's
    /// error.
    pub fn fixed_array<T: Decode<'a>, const N: usize>(
        &mut self,
        field: &'static str,
    ) -> Result<[T; N], Error> {
        const { assert!(T::S_MIN >= UNIT) };
        let start = self.offset;
        let at_least = offset64(N).checked_mul(T::S_MIN);
        self.check_item_end(start, at_least)?;
        if N == 0 {
            return <[T; N]>::try_from(Vec::new())
                .map_err(|_| self.locate(Error::new(ErrorKind::AllocationFailed), start, None));
        }
        let mut elements = self.reserve(N, field, start)?;
        for _ in 0..N {
            elements.push(T::decode(self)?);
        }
        <[T; N]>::try_from(elements)
            .map_err(|_| self.locate(Error::new(ErrorKind::AllocationFailed), start, None))
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
        self.check_item_end(start, padded_len(u64::from(n)))?;
        usize::try_from(n).map_err(|_| self.cap_exceeded(cap, start))
    }

    /// Checks that `at_least` bytes (`None` past `u64::MAX`) from the
    /// current offset fit before both ends of the cursor, for an item that
    /// starts at `start`.
    fn check_item_end(&self, start: usize, at_least: Option<u64>) -> Result<(), Error> {
        let end = at_least.and_then(|bytes| offset64(self.offset).checked_add(bytes));
        self.check_end(start, end)
    }

    /// Checks an item that starts at `start` and would end at the absolute
    /// offset `end` (`None` past `u64::MAX`) against the cursor's two
    /// ends: the tightest array limit first, then the input's end (5.4,
    /// point 5). The arithmetic is in 64 bits on every target (section 6,
    /// A3 and A5).
    fn check_end(&self, start: usize, end: Option<u64>) -> Result<(), Error> {
        if let Some(limit) = self.limit
            && end.is_none_or(|end| end > limit.end)
        {
            return Err(self.cap_exceeded(limit.cap, start));
        }
        if end.is_none_or(|end| end > offset64(self.input.len())) {
            return Err(self.truncated(start));
        }
        Ok(())
    }

    /// Takes `n` bytes through the cursor.
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let start = self.offset;
        self.check_item_end(start, Some(offset64(n)))?;
        let bytes = start
            .checked_add(n)
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
        let padding = self.take(n.wrapping_neg() % 4)?;
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

    /// Turns a buffer holding exactly `N` elements into a box, in place.
    /// The conversion keeps the block only when the capacity equals the
    /// length; otherwise it shrinks it, an allocator request the ledger
    /// never saw (5.2, point 9.4). So a buffer whose reservation gave more
    /// capacity than it asked for is refused with `AllocationFailed`
    /// instead. A zero-sized type has no block, whatever its capacity.
    fn boxed<T, const N: usize>(&self, buffer: Vec<T>, at: usize) -> Result<Box<[T; N]>, Error> {
        let failed = || self.locate(Error::new(ErrorKind::AllocationFailed), at, None);
        if size_of::<T>() != 0 && buffer.capacity() != N {
            return Err(failed());
        }
        Box::<[T; N]>::try_from(buffer).map_err(|_| failed())
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

/// An offset or length in 64 bits. No supported target has a `usize`
/// wider than 64 bits; on one that had, the value would saturate, which
/// only makes it fail a check.
fn offset64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// `n` plus its padding to the unit, or `None` on overflow (section 6, A1
/// and A2).
const fn padded_len(n: u64) -> Option<u64> {
    n.checked_add(n.wrapping_neg() % 4)
}

macro_rules! decode_number {
    ($type:ty, $size:expr, $read:ident) => {
        impl<'a> Decode<'a> for $type {
            const S_MIN: u64 = $size;
            const DEPTH: u32 = 0;
            const HEAP_RATIO: Option<u64> = Some(0);
            const RESERVATIONS: Option<u64> = Some(0);

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
    const HEAP_RATIO: Option<u64> = Some(0);
    const RESERVATIONS: Option<u64> = Some(0);

    fn decode(_: &mut Decoder<'a, '_>) -> Result<Self, Error> {
        Ok(())
    }
}
