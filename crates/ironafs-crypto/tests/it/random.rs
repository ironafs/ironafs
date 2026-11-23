// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The production random source is shared between threads as the `Rng`
//! seam, and its startup error travels as a standard error (coding
//! standards rule 4).

use std::sync::Arc;
use std::thread;

use ironafs_crypto::{CryptoRng, EntropyError, EntropySource};
use ironafs_time::Rng;

/// A source that writes one byte value everywhere.
struct Constant(u8);

impl EntropySource for Constant {
    fn fill(&self, bytes: &mut [u8]) -> Result<(), EntropyError> {
        bytes.fill(self.0);
        Ok(())
    }
}

#[test]
fn crypto_rng_is_shared_between_threads_as_rng() {
    let random: Arc<dyn Rng> = Arc::new(CryptoRng::start(Constant(3)).unwrap());
    let shared = Arc::clone(&random);
    let bytes = thread::spawn(move || {
        let mut bytes = [0_u8; 4];
        shared.fill(&mut bytes).map(|()| bytes)
    })
    .join()
    .unwrap();
    assert_eq!(bytes, Ok([3; 4]));
}

#[test]
fn entropy_error_is_a_standard_error_that_crosses_threads() {
    let error: Box<dyn std::error::Error + Send + Sync> =
        Box::new(EntropyError::Unavailable { code: 1 });
    let message = thread::spawn(move || error.to_string()).join().unwrap();
    assert_eq!(
        message,
        "random source unavailable or filtered (os error 1)"
    );
}
