// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

use core::sync::atomic::AtomicU64;

use super::{EpochSource, FenceTuple, PauseGeneration, Stamp, Suspicion, VolumeEpoch};

fn tuple(values: [u64; 6]) -> FenceTuple {
    let [object, volume, suspect_epoch, pause, resume, epoch] = values;
    FenceTuple {
        object: Stamp(object),
        volume: Stamp(volume),
        suspect_epoch: Stamp(suspect_epoch),
        pause: PauseGeneration(pause),
        resume: Stamp(resume),
        volume_epoch: Some(VolumeEpoch(epoch)),
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
fn compare_names_each_changed_value() {
    let recorded = tuple([1, 2, 3, 4, 5, 6]);
    assert!(recorded.compare(&recorded).is_clear());
    let flag = |index: usize| {
        let mut flags = [false; 6];
        if let Some(slot) = flags.get_mut(index) {
            *slot = true;
        }
        let [object, volume, suspect_epoch, pause, resume, volume_epoch] = flags;
        Suspicion {
            object,
            volume,
            suspect_epoch,
            pause,
            resume,
            volume_epoch,
        }
    };
    for index in 0..6 {
        let mut values = [1, 2, 3, 4, 5, 6];
        values[index] = 9;
        let current = tuple(values);
        assert_eq!(recorded.compare(&current), flag(index), "{current:?}");
    }
}

#[test]
fn compare_fails_on_absent_values_even_when_equal() {
    let mut recorded = tuple([0, 2, 0, 0, 5, 6]);
    recorded.volume_epoch = None;
    assert_eq!(
        recorded.compare(&recorded),
        Suspicion {
            object: true,
            suspect_epoch: true,
            volume_epoch: true,
            ..Suspicion::default()
        }
    );
}

#[test]
fn compare_narrow_only_for_object() {
    let recorded = tuple([1, 2, 3, 4, 5, 6]);
    assert!(!recorded.compare(&recorded).is_narrow());
    assert!(recorded.compare(&tuple([9, 2, 3, 4, 5, 6])).is_narrow());
    assert!(!recorded.compare(&tuple([9, 9, 3, 4, 5, 6])).is_narrow());
    assert!(!recorded.compare(&tuple([1, 9, 3, 4, 5, 6])).is_narrow());
    assert!(!recorded.compare(&tuple([9, 2, 3, 4, 9, 6])).is_narrow());
    assert!(!recorded.compare(&tuple([9, 2, 3, 4, 5, 9])).is_narrow());
}
