// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

use core::sync::atomic::{AtomicU64, Ordering};

use super::{EpochSource, FenceTuple, PauseGeneration, ResumeGeneration, Stamp, Suspicion};

fn tuple(values: [u64; 5]) -> FenceTuple {
    let [object, volume, suspect_epoch, pause, resume] = values;
    FenceTuple {
        object: Stamp(object),
        volume: Stamp(volume),
        suspect_epoch: Stamp(suspect_epoch),
        pause: PauseGeneration(pause),
        resume: Stamp(resume),
    }
}

#[test]
fn stamp_none_matches_nothing() {
    assert!(!Stamp::NONE.matches(Stamp::NONE));
    assert!(!Stamp::NONE.matches(Stamp(1)));
    assert!(!Stamp(1).matches(Stamp::NONE));
    assert!(Stamp(7).matches(Stamp(7)));
    assert!(!Stamp(7).matches(Stamp(8)));
}

#[test]
fn epoch_source_never_repeats() {
    let source = EpochSource::new();
    let taken: [Option<Stamp>; 4] = core::array::from_fn(|_| source.take());
    assert_eq!(
        taken,
        [
            Some(Stamp(1)),
            Some(Stamp(2)),
            Some(Stamp(3)),
            Some(Stamp(4))
        ]
    );
}

#[test]
fn epoch_source_exhausted_returns_none() {
    let source = EpochSource {
        next: AtomicU64::new(u64::MAX - 1),
    };
    assert_eq!(source.take(), Some(Stamp(u64::MAX - 1)));
    assert_eq!(source.take(), Some(Stamp(u64::MAX)));
    assert_eq!(source.take(), None);
    assert_eq!(source.take(), None);
}

#[test]
fn resume_generation_raise_moves_current() {
    let resume = ResumeGeneration::new();
    let before = resume.current();
    resume.raise();
    let after = resume.current();
    assert!(!after.matches(before));
    assert!(after.matches(resume.current()));
}

#[test]
fn resume_generation_exhausted_matches_nothing() {
    let resume = ResumeGeneration {
        value: AtomicU64::new(u64::MAX),
    };
    let before = resume.current();
    assert!(before.matches(before));
    resume.raise();
    assert_eq!(resume.current(), Stamp::NONE);
    assert!(!resume.current().matches(before));
    assert!(!resume.current().matches(resume.current()));
    resume.raise();
    assert_eq!(resume.value.load(Ordering::SeqCst), 0);
}

#[test]
fn compare_names_each_changed_value() {
    let recorded = tuple([1, 2, 3, 4, 5]);
    assert!(recorded.compare(&recorded).is_clear());
    let cases = [
        (
            tuple([9, 2, 3, 4, 5]),
            Suspicion {
                object: true,
                ..Suspicion::default()
            },
        ),
        (
            tuple([1, 9, 3, 4, 5]),
            Suspicion {
                volume: true,
                ..Suspicion::default()
            },
        ),
        (
            tuple([1, 2, 9, 4, 5]),
            Suspicion {
                suspect_epoch: true,
                ..Suspicion::default()
            },
        ),
        (
            tuple([1, 2, 3, 9, 5]),
            Suspicion {
                pause: true,
                ..Suspicion::default()
            },
        ),
        (
            tuple([1, 2, 3, 4, 9]),
            Suspicion {
                resume: true,
                ..Suspicion::default()
            },
        ),
    ];
    for (current, expected) in cases {
        assert_eq!(recorded.compare(&current), expected, "{current:?}");
    }
}

#[test]
fn compare_fails_on_absent_values_even_when_equal() {
    let recorded = tuple([0, 2, 0, 0, 5]);
    assert_eq!(
        recorded.compare(&recorded),
        Suspicion {
            object: true,
            suspect_epoch: true,
            ..Suspicion::default()
        }
    );
}

#[test]
fn compare_narrow_only_for_object() {
    let recorded = tuple([1, 2, 3, 4, 5]);
    assert!(!recorded.compare(&recorded).is_narrow());
    assert!(recorded.compare(&tuple([9, 2, 3, 4, 5])).is_narrow());
    assert!(!recorded.compare(&tuple([9, 9, 3, 4, 5])).is_narrow());
    assert!(!recorded.compare(&tuple([1, 9, 3, 4, 5])).is_narrow());
    assert!(!recorded.compare(&tuple([9, 2, 3, 4, 9])).is_narrow());
}
