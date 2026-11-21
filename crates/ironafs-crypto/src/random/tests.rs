// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the random source: its error messages, which carry the
//! operating system's code into the startup report (coding standards
//! rule 4), and the production `Rng` over an injected source.

use alloc::format;
use alloc::string::ToString;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

use ironafs_time::{Rng, RngError};

use super::{CryptoRng, EntropyError, EntropySource};

#[test]
fn entropy_error_messages_name_the_failure_and_code() {
    assert_eq!(
        EntropyError::Unavailable { code: 38 }.to_string(),
        "random source unavailable or filtered (os error 38)"
    );
    assert_eq!(
        EntropyError::Failed {
            code: -1_073_741_811
        }
        .to_string(),
        "random source failed (os error -1073741811)"
    );
    assert_eq!(
        EntropyError::BadCount.to_string(),
        "random source returned a byte count it was not asked for"
    );
}

/// A source that succeeds for its first `good_calls` calls, writing
/// `GOOD`, and then fails with `error` after writing `PARTIAL` over the
/// first half of the buffer.
struct Scripted {
    calls: AtomicUsize,
    good_calls: usize,
    error: EntropyError,
}

const GOOD: u8 = 0xA5;
const PARTIAL: u8 = 0x5A;

impl Scripted {
    fn new(good_calls: usize, error: EntropyError) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            good_calls,
            error,
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

/// Shares a [`Scripted`] source with the test that counts its calls.
struct Shared(Arc<Scripted>);

impl EntropySource for Shared {
    fn fill(&self, bytes: &mut [u8]) -> Result<(), EntropyError> {
        let call = self.0.calls.fetch_add(1, Ordering::SeqCst);
        if call < self.0.good_calls {
            bytes.fill(GOOD);
            Ok(())
        } else {
            let half = bytes.len() / 2;
            bytes[..half].fill(PARTIAL);
            Err(self.0.error)
        }
    }
}

#[track_caller]
fn started(source: &Arc<Scripted>) -> CryptoRng {
    CryptoRng::start(Shared(Arc::clone(source))).unwrap()
}

#[test]
fn start_probes_the_source_once() {
    let source = Scripted::new(usize::MAX, EntropyError::BadCount);
    let _rng = started(&source);
    assert_eq!(source.calls(), 1);
}

#[test]
fn start_refuses_unavailable_source_with_its_code() {
    let source = Scripted::new(0, EntropyError::Unavailable { code: 38 });
    let refused = CryptoRng::start(Shared(Arc::clone(&source))).map(|_| ());
    assert_eq!(refused, Err(EntropyError::Unavailable { code: 38 }));
    assert_eq!(source.calls(), 1);
}

#[test]
fn start_refuses_failed_source_with_its_code() {
    let source = Scripted::new(0, EntropyError::Failed { code: -50 });
    let refused = CryptoRng::start(Shared(source)).map(|_| ());
    assert_eq!(refused, Err(EntropyError::Failed { code: -50 }));
}

#[test]
fn fill_passes_source_bytes_through() {
    let source = Scripted::new(usize::MAX, EntropyError::BadCount);
    let rng = started(&source);
    let mut bytes = [0_u8; 32];
    assert_eq!(rng.fill(&mut bytes), Ok(()));
    assert_eq!(bytes, [GOOD; 32]);
    assert_eq!(source.calls(), 2);
}

#[test]
fn fill_fails_when_source_fails_after_start() {
    let source = Scripted::new(1, EntropyError::Unavailable { code: 1 });
    let rng = started(&source);
    let mut bytes = [0_u8; 8];
    assert_eq!(rng.fill(&mut bytes), Err(RngError::new()));
    // No fallback: the one failing call is the only one after the probe.
    assert_eq!(source.calls(), 2);
}

#[test]
fn fill_reports_failure_after_partial_write() {
    let source = Scripted::new(1, EntropyError::BadCount);
    let rng = started(&source);
    let mut bytes = [0_u8; 8];
    assert_eq!(rng.fill(&mut bytes), Err(RngError::new()));
    assert_eq!(bytes[..4], [PARTIAL; 4]);
}

#[test]
fn fill_of_empty_buffer_does_not_call_source() {
    let source = Scripted::new(1, EntropyError::BadCount);
    let rng = started(&source);
    assert_eq!(rng.fill(&mut []), Ok(()));
    assert_eq!(source.calls(), 1);
}

#[test]
fn debug_names_the_type_only() {
    let source = Scripted::new(usize::MAX, EntropyError::BadCount);
    assert_eq!(format!("{:?}", started(&source)), "CryptoRng { .. }");
}
