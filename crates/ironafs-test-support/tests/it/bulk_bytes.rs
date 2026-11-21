// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Multi-MiB properties through `Source::bulk_bytes`: they fit the test
//! runner's per-test limit, and a failure shrinks by length, not byte by
//! byte.

use ironafs_test_support::Property;

/// The largest payload these properties draw.
const MAX_LEN: usize = 8 << 20;

/// A length-prefixed frame: a big-endian `u32` length, then the payload.
fn frame(payload: &[u8]) -> Vec<u8> {
    let mut framed = u32::try_from(payload.len()).unwrap().to_be_bytes().to_vec();
    framed.extend_from_slice(payload);
    framed
}

/// The payload of a frame, if its length prefix matches.
fn unframe(framed: &[u8]) -> Option<&[u8]> {
    let (prefix, payload) = framed.split_first_chunk::<4>()?;
    (usize::try_from(u32::from_be_bytes(*prefix)).ok()? == payload.len()).then_some(payload)
}

/// The multi-MiB acceptance property: 32 cases of up to 8 MiB each.
///
/// Sized to the test runner's limit of 60 seconds a test
/// (`.config/nextest.toml`): in a debug build on an Apple M4 Max (16
/// cores, 2026-11-20) it took 2.27 to 2.28 s in three runs, and 2.26 to
/// 2.50 s with `IRONAFS_TEST_SEED` set to 5 and to 0x1234, which leaves
/// room for a runner ten times slower. Time grows with the bytes drawn
/// (about 17 ms a MiB here: 64 cases of up to 16 MiB took 9.1 s, 48 of up
/// to 32 MiB 14.2 s). For comparison, a failing property over
/// `Source::bytes` of up to 1 MiB spent its whole 20-second shrink time
/// limit in 360 runs without shortening the input much, where
/// `bulk_bytes` reached the exact minimum in 100 runs and 40 ms.
#[test]
fn bulk_frame_round_trips_up_to_eight_mib() {
    Property::new("bulk frame round trip")
        .cases(32)
        .check_described(
            |source| source.bulk_bytes(0..=MAX_LEN),
            |payload| assert_eq!(unframe(&frame(payload)), Some(payload.as_slice())),
            |payload| format!("{} bytes", payload.len()),
        );
}

#[test]
fn bulk_planted_length_bug_shrinks_to_shortest_failing_length() {
    let failure = Property::new("bulk planted length bug")
        .run(
            |source| source.bulk_bytes(0..=MAX_LEN),
            |payload| assert!(payload.len() <= 3 << 20),
        )
        .unwrap_err();
    assert_eq!(failure.value().len(), (3 << 20) + 1);
    assert_eq!(failure.choices(), [(3 << 20) + 1]);
    assert!(!failure.shrink_budget_exhausted() && !failure.shrink_timed_out());
}

#[test]
fn bulk_planted_content_bug_shrinks_length_and_keeps_a_failing_seed() {
    let failure = Property::new("bulk planted content bug")
        .run(
            |source| source.bulk_bytes(0..=MAX_LEN),
            |payload| assert!(payload.len() < 1 << 20 || payload[0] % 2 == 0),
        )
        .unwrap_err();
    assert_eq!(failure.value().len(), 1 << 20);
    let seed = failure.choices()[1];
    assert!(seed != 0 && failure.value()[0] % 2 == 1, "seed {seed}");
    assert!(!failure.shrink_budget_exhausted() && !failure.shrink_timed_out());
}
