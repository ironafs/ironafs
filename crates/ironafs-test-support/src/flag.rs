// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A watched flag a test sets by hand: a cancel token or signal for tests
//! that never block on it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ironafs_time::{Registration, Waker, Watch, WatchFull};

#[cfg(test)]
mod tests;

/// A [`Watch`] that is set or not, as the test built it. It never wakes a
/// registered waker, so it suits only waits that never block, such as the
/// seeded clock's, and values that only report whether they are set.
#[derive(Debug, Default)]
pub struct Flag(AtomicBool);

impl Flag {
    /// A flag that is already set.
    #[must_use]
    pub fn set() -> Self {
        Self(AtomicBool::new(true))
    }
}

impl Watch for Flag {
    fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    fn register(&self, _waker: Arc<dyn Waker>) -> Result<Registration, WatchFull> {
        Ok(Registration::new(0))
    }

    fn deregister(&self, _registration: Registration) {}
}
