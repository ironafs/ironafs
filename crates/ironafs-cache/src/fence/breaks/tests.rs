// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

use ironafs_vfs::{ContextId, CredentialGeneration};

use super::{BreakCounters, BreakLimits, FenceError};
use crate::fence::{Clocks, PauseGeneration, ResumeGeneration, ServerValues, Stamp, Suspicion};
use crate::{CellIndex, ContextKey, FileId, Uniquifier, Vnode, VolumeId};

fn context(id: u64) -> ContextKey {
    ContextKey {
        id: ContextId(id),
        generation: CredentialGeneration(1),
    }
}

fn file(volume: u32, vnode: u32) -> FileId {
    FileId {
        cell: CellIndex(0),
        volume: VolumeId(volume),
        vnode: Vnode(vnode),
        uniquifier: Uniquifier(1),
    }
}

const SERVER: ServerValues = ServerValues {
    suspect_epoch: Stamp(1),
    pause: PauseGeneration(0),
};

const ROOMY: BreakLimits = BreakLimits {
    objects: 16,
    volumes: 16,
};

const OBJECT: Suspicion = Suspicion {
    object: true,
    volume: false,
    suspect_epoch: false,
    pause: false,
    resume: false,
};

const VOLUME: Suspicion = Suspicion {
    object: false,
    volume: true,
    suspect_epoch: false,
    pause: false,
    resume: false,
};

fn check(
    table: &BreakCounters,
    resume: &ResumeGeneration,
    ctx: u64,
    target: FileId,
    recorded: &crate::fence::FenceTuple,
) -> Suspicion {
    table.check(
        resume,
        Clocks::Steady,
        context(ctx),
        target,
        SERVER,
        recorded,
    )
}

#[test]
fn record_keeps_value_until_a_break() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(ROOMY).unwrap();
    let ticket = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    let again = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    assert_eq!(ticket, again);
    assert!(check(&table, &resume, 1, file(1, 1), &ticket).is_clear());
}

#[test]
fn break_file_fences_every_context_holding_it() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(ROOMY).unwrap();
    let first = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    let second = table
        .record(context(2), file(1, 1), SERVER, &resume)
        .unwrap();
    assert_eq!(table.break_file(&file(1, 1)), 2);
    assert_eq!(check(&table, &resume, 1, file(1, 1), &first), OBJECT);
    assert_eq!(check(&table, &resume, 2, file(1, 1), &second), OBJECT);
}

#[test]
fn break_file_leaves_other_files() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(ROOMY).unwrap();
    let other = table
        .record(context(1), file(1, 2), SERVER, &resume)
        .unwrap();
    table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    assert_eq!(table.break_file(&file(1, 1)), 1);
    assert!(check(&table, &resume, 1, file(1, 2), &other).is_clear());
}

#[test]
fn break_volume_fences_every_file_of_it_in_every_context() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(ROOMY).unwrap();
    let a = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    let b = table
        .record(context(2), file(1, 2), SERVER, &resume)
        .unwrap();
    let other = table
        .record(context(1), file(2, 1), SERVER, &resume)
        .unwrap();
    assert_eq!(table.break_volume(CellIndex(0), VolumeId(1)), 2);
    assert_eq!(check(&table, &resume, 1, file(1, 1), &a), VOLUME);
    assert_eq!(check(&table, &resume, 2, file(1, 2), &b), VOLUME);
    assert!(check(&table, &resume, 1, file(2, 1), &other).is_clear());
    // The same volume id in another cell is another volume.
    assert_eq!(table.break_volume(CellIndex(1), VolumeId(2)), 0);
    assert!(check(&table, &resume, 1, file(2, 1), &other).is_clear());
}

#[test]
fn break_unknown_file_changes_nothing() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(ROOMY).unwrap();
    let ticket = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    assert_eq!(table.break_file(&file(1, 9)), 0);
    assert_eq!(table.break_volume(CellIndex(0), VolumeId(9)), 0);
    assert_eq!((table.objects(), table.volumes()), (1, 1));
    assert!(check(&table, &resume, 1, file(1, 1), &ticket).is_clear());
}

#[test]
fn break_raises_a_fresh_value_each_time() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(ROOMY).unwrap();
    table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    table.break_file(&file(1, 1));
    let between = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    table.break_file(&file(1, 1));
    assert_eq!(check(&table, &resume, 1, file(1, 1), &between), OBJECT);
}

#[test]
fn check_after_eviction_fails_object() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(BreakLimits {
        objects: 1,
        volumes: 16,
    })
    .unwrap();
    let evicted = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    table
        .record(context(1), file(1, 2), SERVER, &resume)
        .unwrap();
    assert_eq!(table.objects(), 1);
    assert_eq!(check(&table, &resume, 1, file(1, 1), &evicted), OBJECT);
    // Made again, the record takes a value no earlier record held.
    table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    assert_eq!(check(&table, &resume, 1, file(1, 1), &evicted), OBJECT);
}

#[test]
fn eviction_takes_the_oldest_value() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(BreakLimits {
        objects: 2,
        volumes: 16,
    })
    .unwrap();
    let old = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    let young = table
        .record(context(1), file(1, 2), SERVER, &resume)
        .unwrap();
    // A break gives file 1 a newer value than file 2's.
    table.break_file(&file(1, 1));
    let renewed = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    table
        .record(context(1), file(1, 3), SERVER, &resume)
        .unwrap();
    assert_eq!(check(&table, &resume, 1, file(1, 2), &young), OBJECT);
    assert!(check(&table, &resume, 1, file(1, 1), &renewed).is_clear());
    assert_eq!(check(&table, &resume, 1, file(1, 1), &old), OBJECT);
}

#[test]
fn record_within_limits_evicts_nothing() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(BreakLimits {
        objects: 2,
        volumes: 2,
    })
    .unwrap();
    let a = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    let b = table
        .record(context(2), file(1, 1), SERVER, &resume)
        .unwrap();
    assert_eq!((table.objects(), table.volumes()), (2, 2));
    assert!(check(&table, &resume, 1, file(1, 1), &a).is_clear());
    assert!(check(&table, &resume, 2, file(1, 1), &b).is_clear());
}

#[test]
fn volume_records_are_bounded() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(BreakLimits {
        objects: 16,
        volumes: 1,
    })
    .unwrap();
    let first = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    table
        .record(context(1), file(2, 1), SERVER, &resume)
        .unwrap();
    assert_eq!(table.volumes(), 1);
    assert_eq!(check(&table, &resume, 1, file(1, 1), &first), VOLUME);
}

#[test]
fn exhausted_table_fails_every_check_and_refuses_records() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(ROOMY).unwrap();
    let ticket = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    table.next = Some(u64::MAX);
    // The last value is taken; the next raise finds none.
    assert_eq!(table.break_file(&file(1, 1)), 1);
    assert!(!table.is_exhausted());
    let last = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    assert_eq!(table.break_volume(CellIndex(0), VolumeId(1)), 1);
    assert!(table.is_exhausted());
    let all = Suspicion {
        object: true,
        volume: true,
        ..Suspicion::default()
    };
    assert_eq!(check(&table, &resume, 1, file(1, 1), &last), all);
    assert_eq!(check(&table, &resume, 1, file(1, 1), &ticket), all);
    assert_eq!(
        table.record(context(1), file(1, 1), SERVER, &resume),
        Err(FenceError::Exhausted)
    );
    assert_eq!(
        table.record(context(3), file(5, 5), SERVER, &resume),
        Err(FenceError::Exhausted)
    );
}

#[test]
fn check_compares_the_server_values() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(ROOMY).unwrap();
    let ticket = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    let advanced = ServerValues {
        suspect_epoch: Stamp(2),
        ..SERVER
    };
    let paused = ServerValues {
        pause: PauseGeneration(1),
        ..SERVER
    };
    let at = |server| {
        table.check(
            &resume,
            Clocks::Steady,
            context(1),
            file(1, 1),
            server,
            &ticket,
        )
    };
    assert_eq!(
        at(advanced),
        Suspicion {
            suspect_epoch: true,
            ..Suspicion::default()
        }
    );
    assert_eq!(
        at(paused),
        Suspicion {
            pause: true,
            ..Suspicion::default()
        }
    );
}

#[test]
fn check_moved_clocks_raises_resume_once() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(ROOMY).unwrap();
    let ticket = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    let moved = table.check(
        &resume,
        Clocks::Moved,
        context(1),
        file(1, 1),
        SERVER,
        &ticket,
    );
    assert_eq!(
        moved,
        Suspicion {
            resume: true,
            ..Suspicion::default()
        }
    );
    let after = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    assert!(check(&table, &resume, 1, file(1, 1), &after).is_clear());
}

#[test]
fn check_of_another_context_never_matches() {
    let resume = ResumeGeneration::new();
    let mut table = BreakCounters::new(ROOMY).unwrap();
    let ticket = table
        .record(context(1), file(1, 1), SERVER, &resume)
        .unwrap();
    table
        .record(context(2), file(1, 1), SERVER, &resume)
        .unwrap();
    assert!(!check(&table, &resume, 2, file(1, 1), &ticket).is_clear());
    assert!(!check(&table, &resume, 3, file(1, 1), &ticket).is_clear());
}

#[test]
fn new_rejects_limits_out_of_range() {
    for (objects, volumes) in [
        (0, 1),
        (1, 0),
        (crate::MAX_NODES + 1, 1),
        (1, crate::MAX_NODES + 1),
    ] {
        assert_eq!(
            BreakCounters::new(BreakLimits { objects, volumes }).map(|_| ()),
            Err(FenceError::InvalidLimits)
        );
    }
    assert!(
        BreakCounters::new(BreakLimits {
            objects: crate::MAX_NODES,
            volumes: 1,
        })
        .is_ok()
    );
}
