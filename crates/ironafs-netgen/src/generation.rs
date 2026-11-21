// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The network generation's counter: one publisher, any number of readers
//! (docs/specs/network-generation.md, 5.2 and 5.3).
//!
//! The value `u64::MAX` is "unavailable": before the first move and after
//! the source stops. A move tells every consumer first and only then
//! stores the new value (release), so a reader (acquire) never sees a
//! generation whose consumers have not been told of it.

use alloc::sync::Arc;
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::consumer::{Cause, Consumers, Move};

#[cfg(test)]
mod tests;

/// The published value that means no generation is current.
const UNAVAILABLE: u64 = u64::MAX;

/// One network generation. Later generations compare greater.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetworkGeneration(u64);

impl NetworkGeneration {
    /// The counter's value, from 1.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// No generation is current: the source has not completed its first
/// snapshot, or it stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unavailable;

impl fmt::Display for Unavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the network generation is unavailable")
    }
}

impl core::error::Error for Unavailable {}

/// Reads the current generation. Cheap to clone; any thread.
#[derive(Clone, Debug)]
pub struct Reader {
    cell: Arc<AtomicU64>,
}

impl Reader {
    /// The current generation.
    ///
    /// # Errors
    ///
    /// [`Unavailable`] before the first move and after the source stops.
    pub fn current(&self) -> Result<NetworkGeneration, Unavailable> {
        match self.cell.load(Ordering::Acquire) {
            UNAVAILABLE => Err(Unavailable),
            value => Ok(NetworkGeneration(value)),
        }
    }
}

/// The source stopped: the counter is exhausted or the source was told to
/// stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stopped;

/// The one writer of the counter. Dropping it stops the source.
#[derive(Debug)]
pub(crate) struct Publisher {
    cell: Arc<AtomicU64>,
    /// The last generation published, 0 before the first.
    value: u64,
    consumers: Consumers,
    stopped: bool,
}

impl Publisher {
    /// A publisher with nothing published yet, and its reader. `make`
    /// builds the consumers' hooks, and may give each a reader of its own.
    pub(crate) fn new(make: impl FnOnce(&Reader) -> Consumers) -> (Self, Reader) {
        let cell = Arc::new(AtomicU64::new(UNAVAILABLE));
        let reader = Reader {
            cell: Arc::clone(&cell),
        };
        let consumers = make(&reader);
        let publisher = Self {
            cell,
            value: 0,
            consumers,
            stopped: false,
        };
        (publisher, reader)
    }

    /// Moves the generation by one: every consumer is told, then the new
    /// value is published.
    ///
    /// # Errors
    ///
    /// [`Stopped`] if the source has stopped, or if the next value would
    /// be the unavailable marker, which stops it.
    pub(crate) fn advance(&mut self, cause: Cause) -> Result<NetworkGeneration, Stopped> {
        if self.stopped {
            return Err(Stopped);
        }
        let next = match self.value.checked_add(1) {
            Some(next) if next != UNAVAILABLE => next,
            _ => {
                self.stop();
                return Err(Stopped);
            }
        };
        let generation = NetworkGeneration(next);
        self.consumers.tell(Move::To(generation, cause));
        self.value = next;
        self.cell.store(next, Ordering::Release);
        Ok(generation)
    }

    /// Stops the source: every consumer is told once, and reads fail from
    /// now on.
    pub(crate) fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        self.cell.store(UNAVAILABLE, Ordering::Release);
        self.consumers.tell(Move::Stopped);
    }
}

impl Drop for Publisher {
    fn drop(&mut self) {
        self.stop();
    }
}
