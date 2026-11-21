// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The seams can be shared between threads as trait objects, and their
//! errors travel as standard errors (coding standards rule 4).

use std::sync::Arc;
use std::thread;

use ironafs_time::{
    AwakeInstant, BootInstant, Clock, ClockError, ClockKind, Rng, RngError, WallTime,
};

/// A clock whose awake clock has failed.
struct AwakeFailed;

impl Clock for AwakeFailed {
    fn boot(&self) -> Result<BootInstant, ClockError> {
        Ok(BootInstant::from_nanos(5))
    }
    fn awake(&self) -> Result<AwakeInstant, ClockError> {
        Err(ClockError::new(ClockKind::Awake))
    }
    fn wall(&self) -> Result<WallTime, ClockError> {
        Ok(WallTime::UNIX_EPOCH)
    }
}

/// A source that writes one byte value everywhere.
struct Constant(u8);

impl Rng for Constant {
    fn fill(&self, bytes: &mut [u8]) -> Result<(), RngError> {
        bytes.fill(self.0);
        Ok(())
    }
}

#[test]
fn clock_is_shared_between_threads_as_a_trait_object() {
    let clock: Arc<dyn Clock> = Arc::new(AwakeFailed);
    let shared = Arc::clone(&clock);
    let read = thread::spawn(move || shared.awake()).join().unwrap();
    assert_eq!(read, Err(ClockError::new(ClockKind::Awake)));
    assert_eq!(clock.boot(), Ok(BootInstant::from_nanos(5)));
}

#[test]
fn rng_is_shared_between_threads_as_a_trait_object() {
    let source: Arc<dyn Rng> = Arc::new(Constant(9));
    let shared = Arc::clone(&source);
    let bytes = thread::spawn(move || {
        let mut bytes = [0_u8; 4];
        shared.fill(&mut bytes).map(|()| bytes)
    })
    .join()
    .unwrap();
    assert_eq!(bytes, Ok([9; 4]));
}

#[test]
fn errors_are_standard_errors_that_cross_threads() {
    fn boxed(
        error: impl std::error::Error + Send + Sync + 'static,
    ) -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(error)
    }
    assert_eq!(
        boxed(ClockError::new(ClockKind::Wall)).to_string(),
        "wall time could not be read"
    );
    assert_eq!(
        boxed(RngError::new()).to_string(),
        "the random source could not supply bytes"
    );
}
