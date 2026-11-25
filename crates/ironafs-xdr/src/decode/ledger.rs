// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ledger: the one routine through which the decoder requests heap
//! memory, and its running total against the message's memory budget
//! (`docs/specs/xdr.md`, 5.2, point 9).
//!
//! It counts requested bytes, the element count times `size_of` the
//! element type, from its own request, never from the capacity the
//! allocator returns: a reservation may give more than it was asked for,
//! and a vector of a zero-sized type reports a capacity of `usize::MAX`.

use alloc::vec::Vec;
use core::fmt;
use core::mem::size_of;

use crate::{Error, ErrorKind};

#[cfg(test)]
mod tests;

/// One heap request the decoder made, as the test hook of
/// [`decode_traced`](crate::decode_traced) records it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reservation {
    /// The requested bytes: element count times element size.
    pub bytes: u64,
    /// The cap name, or for a fixed-length opaque held out of line its
    /// field name, of the item that made the request.
    pub name: &'static str,
}

/// The running total of one decode's requested bytes.
pub(crate) struct Ledger<'h> {
    /// The message's cap name, which `BudgetExceeded` names.
    message: &'static str,
    budget: u64,
    total: u64,
    hook: Option<&'h mut dyn FnMut(Reservation)>,
}

impl fmt::Debug for Ledger<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Ledger")
            .field("message", &self.message)
            .field("budget", &self.budget)
            .finish_non_exhaustive()
    }
}

impl<'h> Ledger<'h> {
    pub(crate) fn new(
        message: &'static str,
        budget: u64,
        hook: Option<&'h mut dyn FnMut(Reservation)>,
    ) -> Self {
        Self {
            message,
            budget,
            total: 0,
            hook,
        }
    }

    /// Reserves room for exactly `count` elements of `T`, fallibly, after
    /// checking that the request keeps the total within the budget.
    ///
    /// The returned buffer has capacity for at least `count` elements; it
    /// is filled only by `push`, so it never grows (5.2, point 9.4). The
    /// hook sees the request before the allocator does, so a request that
    /// fails, or whose buffer is dropped by a later error, is recorded.
    ///
    /// # Errors
    ///
    /// `BudgetExceeded` if the request would carry the total past the
    /// budget, or its size overflows; `AllocationFailed` if the allocator
    /// refuses it. Neither carries a position: the decoder adds it.
    pub(crate) fn reserve<T>(&mut self, count: usize, name: &'static str) -> Result<Vec<T>, Error> {
        let over_budget = Error::new(ErrorKind::BudgetExceeded)
            .named(self.message)
            .limit(self.budget);
        let bytes = u64::try_from(count)
            .ok()
            .zip(u64::try_from(size_of::<T>()).ok())
            .and_then(|(count, size)| count.checked_mul(size))
            .ok_or(over_budget)?;
        let total = self
            .total
            .checked_add(bytes)
            .filter(|total| *total <= self.budget)
            .ok_or(over_budget)?;
        if let Some(hook) = self.hook.as_mut() {
            hook(Reservation { bytes, name });
        }
        self.total = total;
        let mut buffer = Vec::new();
        #[expect(
            clippy::disallowed_methods,
            reason = "rule 3: this is the decoder's one reservation routine (docs/specs/xdr.md, 5.2, point 9.2)"
        )]
        buffer
            .try_reserve_exact(count)
            .map_err(|_| Error::new(ErrorKind::AllocationFailed))?;
        Ok(buffer)
    }
}
