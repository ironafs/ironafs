// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The seeded `Clock` and `Rng` reproduce the same sequence from one seed,
//! used as the code under test uses them: shared trait objects.

use std::sync::Arc;
use std::time::Duration;

use ironafs_test_support::{SeededClock, SeededRng};
use ironafs_time::{Clock, ClockKind, Rng};

/// One run of a small scripted scenario: clock moves, reads and draws
/// interleaved, recorded as text.
fn run(seed: u64) -> Vec<String> {
    let clock = Arc::new(SeededClock::from_seed(seed));
    let source = Arc::new(SeededRng::new(seed));
    let reader: Arc<dyn Clock> = Arc::clone(&clock) as Arc<dyn Clock>;
    let random: Arc<dyn Rng> = Arc::clone(&source) as Arc<dyn Rng>;
    let mut record = Vec::new();
    for step in 0..6_u64 {
        let mut bytes = vec![0_u8; usize::try_from(step).unwrap() * 3 + 1];
        record.push(format!("{:?}", random.fill(&mut bytes).map(|()| bytes)));
        record.push(format!(
            "{:?} {:?} {:?}",
            reader.boot(),
            reader.awake(),
            reader.wall()
        ));
        if step % 2 == 0 {
            clock.advance(Duration::from_millis(step * 250)).unwrap();
        } else {
            clock.suspend(Duration::from_secs(step * 60)).unwrap();
        }
    }
    clock.set_failing(ClockKind::Boot, true);
    source.set_failing(true);
    let mut bytes = [0_u8; 4];
    record.push(format!("{:?} {:?}", reader.boot(), random.fill(&mut bytes)));
    record
}

#[test]
fn seeded_clock_and_rng_reproduce_the_same_sequence_from_one_seed() {
    assert_eq!(run(0x5eed), run(0x5eed));
}

#[test]
fn seeded_clock_and_rng_sequences_differ_between_seeds() {
    assert_ne!(run(1), run(2));
}
