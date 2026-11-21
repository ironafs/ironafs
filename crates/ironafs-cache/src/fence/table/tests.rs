// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

use alloc::rc::Rc;
use core::cell::Cell;
use core::sync::atomic::{AtomicU64, Ordering};

use super::{FenceError, FenceLimits, FenceTable, MAX_FENCE_BYTES, MIN_FENCE_BYTES, SLOT_BYTES};
use crate::fence::{
    ClockWatch, FenceTuple, PauseGeneration, Server, Stamp, Suspicion, VolumeEpoch,
};
use crate::identity::fixture::{CELL, context, file};
use crate::{FileId, VolumeId};

/// A clock watch that reports each move the test makes, once.
#[derive(Debug)]
struct Watch(Rc<Cell<u32>>);

impl ClockWatch for Watch {
    fn moved(&mut self) -> bool {
        self.0.replace(0) > 0
    }
}

/// A pause generation no test raises.
static IDLE: AtomicU64 = AtomicU64::new(0);

/// A pause generation of 1.
static PAUSED: AtomicU64 = AtomicU64::new(1);

const SERVER: Server<'static> = Server {
    suspect_epoch: Stamp(1),
    pause: &IDLE,
};

const ROOMY: usize = 64 * MIN_FENCE_BYTES;

const V1: VolumeId = VolumeId(1);
const V2: VolumeId = VolumeId(2);

fn table(bytes: usize) -> (FenceTable<Watch>, Rc<Cell<u32>>) {
    let moves = Rc::new(Cell::new(0));
    let table = FenceTable::new(FenceLimits { bytes }, Watch(Rc::clone(&moves))).unwrap();
    (table, moves)
}

fn only(change: impl FnOnce(&mut Suspicion)) -> Suspicion {
    let mut suspicion = Suspicion::default();
    change(&mut suspicion);
    suspicion
}

fn check(
    table: &mut FenceTable<Watch>,
    ctx: u64,
    target: FileId,
    recorded: &FenceTuple,
) -> Suspicion {
    table.check(context(ctx), target, SERVER, recorded)
}

/// The epoch the incarnation code writes for `ctx`'s view of `volume` in
/// these tests.
fn epoch_of(ctx: u64, volume: VolumeId) -> VolumeEpoch {
    VolumeEpoch(ctx * 1_000 + u64::from(volume.0))
}

/// Records a tuple, after writing the context's epoch for the volume, as
/// the incarnation code does when it first sees the volume.
fn record(table: &mut FenceTable<Watch>, ctx: u64, target: FileId) -> FenceTuple {
    table
        .set_volume_epoch(
            context(ctx),
            CELL,
            target.volume,
            epoch_of(ctx, target.volume),
        )
        .unwrap();
    record_bare(table, ctx, target)
}

/// Records a tuple without writing an epoch.
fn record_bare(table: &mut FenceTable<Watch>, ctx: u64, target: FileId) -> FenceTuple {
    table.record(context(ctx), target, SERVER).unwrap()
}

#[test]
fn record_keeps_value_until_a_break() {
    let (mut table, _) = table(ROOMY);
    let ticket = record(&mut table, 1, file(V1, 1));
    let again = record(&mut table, 1, file(V1, 1));
    assert_eq!(ticket, again);
    assert!(check(&mut table, 1, file(V1, 1), &ticket).is_clear());
}

#[test]
fn break_file_fences_every_context_holding_it() {
    let (mut table, _) = table(ROOMY);
    let first = record(&mut table, 1, file(V1, 1));
    let second = record(&mut table, 2, file(V1, 1));
    assert_eq!(table.break_file(&file(V1, 1)), 2);
    let object = only(|s| s.object = true);
    assert_eq!(check(&mut table, 1, file(V1, 1), &first), object);
    assert_eq!(check(&mut table, 2, file(V1, 1), &second), object);
}

#[test]
fn break_file_leaves_other_files() {
    let (mut table, _) = table(ROOMY);
    let other = record(&mut table, 1, file(V1, 2));
    record(&mut table, 1, file(V1, 1));
    assert_eq!(table.break_file(&file(V1, 1)), 1);
    assert!(check(&mut table, 1, file(V1, 2), &other).is_clear());
}

#[test]
fn break_volume_fences_every_file_of_it_in_every_context() {
    let (mut table, _) = table(ROOMY);
    let a = record(&mut table, 1, file(V1, 1));
    let b = record(&mut table, 2, file(V1, 2));
    let other = record(&mut table, 1, file(V2, 1));
    assert_eq!(table.break_volume(CELL, V1), 2);
    let volume = only(|s| s.volume = true);
    assert_eq!(check(&mut table, 1, file(V1, 1), &a), volume);
    assert_eq!(check(&mut table, 2, file(V1, 2), &b), volume);
    assert!(check(&mut table, 1, file(V2, 1), &other).is_clear());
    // The same volume id in another cell is another volume.
    assert_eq!(table.break_volume(crate::CellIndex(9), V2), 0);
    assert!(check(&mut table, 1, file(V2, 1), &other).is_clear());
}

#[test]
fn break_unknown_file_changes_nothing() {
    let (mut table, _) = table(ROOMY);
    let ticket = record(&mut table, 1, file(V1, 1));
    assert_eq!(table.break_file(&file(V1, 9)), 0);
    assert_eq!(table.break_volume(CELL, VolumeId(9)), 0);
    assert_eq!((table.objects(), table.volumes()), (1, 1));
    assert!(check(&mut table, 1, file(V1, 1), &ticket).is_clear());
}

#[test]
fn break_raises_a_fresh_value_each_time() {
    let (mut table, _) = table(ROOMY);
    record(&mut table, 1, file(V1, 1));
    table.break_file(&file(V1, 1));
    let between = record(&mut table, 1, file(V1, 1));
    table.break_file(&file(V1, 1));
    assert_eq!(
        check(&mut table, 1, file(V1, 1), &between),
        only(|s| s.object = true)
    );
}

#[test]
fn check_after_eviction_fails_object() {
    let (mut table, _) = table(MIN_FENCE_BYTES);
    let evicted = record(&mut table, 1, file(V1, 1));
    record(&mut table, 1, file(V1, 2));
    assert_eq!((table.objects(), table.volumes()), (1, 1));
    assert!(!table.holds_object(context(1), file(V1, 1)));
    let object = only(|s| s.object = true);
    assert_eq!(check(&mut table, 1, file(V1, 1), &evicted), object);
    // Made again, the record takes a value no earlier record held.
    record(&mut table, 1, file(V1, 1));
    assert_eq!(check(&mut table, 1, file(V1, 1), &evicted), object);
}

#[test]
fn eviction_takes_the_oldest_value_and_never_a_record_in_use() {
    let (mut table, _) = table(3 * SLOT_BYTES);
    let old = record(&mut table, 1, file(V1, 1));
    let young = record(&mut table, 1, file(V1, 2));
    // A break gives file 1 a newer value than file 2's.
    table.break_file(&file(V1, 1));
    let renewed = record(&mut table, 1, file(V1, 1));
    // The volume record is the oldest, but this recording uses it.
    record(&mut table, 1, file(V1, 3));
    assert!(table.holds_volume(context(1), CELL, V1));
    assert!(!table.holds_object(context(1), file(V1, 2)));
    assert_eq!(
        check(&mut table, 1, file(V1, 2), &young),
        only(|s| s.object = true)
    );
    assert!(check(&mut table, 1, file(V1, 1), &renewed).is_clear());
    assert_eq!(
        check(&mut table, 1, file(V1, 1), &old),
        only(|s| s.object = true)
    );
}

#[test]
fn records_stay_within_the_byte_budget() {
    let budget = 3 * MIN_FENCE_BYTES;
    let (mut table, _) = table(budget);
    // What the table reserved at the start is what it holds throughout.
    let reserved = table.bytes();
    let capacities = (
        table.records.capacity(),
        table.objects.capacity(),
        table.volumes.capacity(),
    );
    assert_eq!(table.capacity(), budget / SLOT_BYTES);
    for ctx in 1..=4 {
        for volume in 1..=4 {
            for vnode in 1..=4 {
                record(&mut table, ctx, file(VolumeId(volume), vnode));
                assert!(table.bytes() <= budget);
                assert_eq!(table.bytes(), reserved);
                assert!(table.objects() + table.volumes() <= table.capacity());
                assert_eq!(
                    (
                        table.records.capacity(),
                        table.objects.capacity(),
                        table.volumes.capacity()
                    ),
                    capacities
                );
            }
        }
    }
    assert!(table.objects() >= 1 && table.volumes() >= 1);
}

#[test]
fn record_within_budget_evicts_nothing() {
    let (mut table, _) = table(2 * MIN_FENCE_BYTES);
    let a = record(&mut table, 1, file(V1, 1));
    let b = record(&mut table, 2, file(V1, 1));
    assert_eq!((table.objects(), table.volumes()), (2, 2));
    assert!(check(&mut table, 1, file(V1, 1), &a).is_clear());
    assert!(check(&mut table, 2, file(V1, 1), &b).is_clear());
}

#[test]
fn volume_records_are_evicted_too() {
    let (mut table, _) = table(MIN_FENCE_BYTES);
    let first = record(&mut table, 1, file(V1, 1));
    record(&mut table, 1, file(V2, 1));
    assert_eq!((table.objects(), table.volumes()), (1, 1));
    assert_eq!(
        check(&mut table, 1, file(V1, 1), &first),
        only(|s| {
            s.object = true;
            s.volume = true;
            s.volume_epoch = true;
        })
    );
}

#[test]
fn exhausted_table_fails_every_check_and_refuses_records() {
    let (mut table, _) = table(ROOMY);
    let ticket = record(&mut table, 1, file(V1, 1));
    table.next = Some(u64::MAX);
    // The last value is taken; the next raise finds none.
    assert_eq!(table.break_file(&file(V1, 1)), 1);
    assert!(!table.is_exhausted());
    let last = record(&mut table, 1, file(V1, 1));
    assert_eq!(table.break_volume(CELL, V1), 1);
    assert!(table.is_exhausted());
    let both = only(|s| {
        s.object = true;
        s.volume = true;
    });
    assert_eq!(check(&mut table, 1, file(V1, 1), &last), both);
    assert_eq!(check(&mut table, 1, file(V1, 1), &ticket), both);
    assert_eq!(
        table.record(context(1), file(V1, 1), SERVER),
        Err(FenceError::Exhausted)
    );
    assert_eq!(
        table.record(context(3), file(VolumeId(5), 5), SERVER),
        Err(FenceError::Exhausted)
    );
}

#[test]
fn record_refused_changes_nothing_but_the_latch() {
    let (mut table, _) = table(MIN_FENCE_BYTES);
    let held = record(&mut table, 1, file(V1, 1));
    // One value left; a recording in a new volume needs two, and would
    // evict both records held.
    table.next = Some(u64::MAX);
    let bytes = table.bytes();
    assert_eq!(
        table.record(context(2), file(V2, 1), SERVER),
        Err(FenceError::Exhausted)
    );
    assert_eq!(
        (table.objects(), table.volumes(), table.bytes()),
        (1, 1, bytes)
    );
    assert!(table.holds_object(context(1), file(V1, 1)));
    assert!(table.holds_volume(context(1), CELL, V1));
    assert_eq!(table.next, Some(u64::MAX));
    // The latch fails every check.
    assert!(table.is_exhausted());
    assert!(!check(&mut table, 1, file(V1, 1), &held).is_clear());
}

#[test]
fn record_refuses_when_the_resume_generation_is_exhausted() {
    let (mut table, moves) = table(ROOMY);
    let ticket = record(&mut table, 1, file(V1, 1));
    table.resume = Some(u64::MAX);
    moves.set(1);
    assert_eq!(
        table.record(context(2), file(V2, 1), SERVER),
        Err(FenceError::Exhausted)
    );
    assert!(table.is_exhausted());
    assert_eq!((table.objects(), table.volumes()), (1, 1));
    assert_eq!(
        check(&mut table, 1, file(V1, 1), &ticket),
        only(|s| s.resume = true)
    );
}

#[test]
fn check_compares_the_server_values() {
    let (mut table, _) = table(ROOMY);
    let ticket = record(&mut table, 1, file(V1, 1));
    let advanced = Server {
        suspect_epoch: Stamp(2),
        ..SERVER
    };
    let paused = Server {
        pause: &PAUSED,
        ..SERVER
    };
    assert_eq!(
        table.check(context(1), file(V1, 1), advanced, &ticket),
        only(|s| s.suspect_epoch = true)
    );
    assert_eq!(
        table.check(context(1), file(V1, 1), paused, &ticket),
        only(|s| s.pause = true)
    );
}

#[test]
fn check_raises_the_resume_generation_once_per_move() {
    let (mut table, moves) = table(ROOMY);
    let ticket = record(&mut table, 1, file(V1, 1));
    moves.set(3);
    assert_eq!(
        check(&mut table, 1, file(V1, 1), &ticket),
        only(|s| s.resume = true)
    );
    let after = record(&mut table, 1, file(V1, 1));
    assert!(check(&mut table, 1, file(V1, 1), &after).is_clear());
    assert_eq!(after.resume, Stamp(ticket.resume.get() + 1));
}

#[test]
fn record_consults_the_clocks_first() {
    let (mut table, moves) = table(ROOMY);
    let before = record(&mut table, 1, file(V1, 1));
    moves.set(1);
    let after = record(&mut table, 1, file(V1, 2));
    assert!(check(&mut table, 1, file(V1, 2), &after).is_clear());
    assert_eq!(
        check(&mut table, 1, file(V1, 1), &before),
        only(|s| s.resume = true)
    );
}

#[test]
fn check_of_another_context_never_matches() {
    let (mut table, _) = table(ROOMY);
    let ticket = record(&mut table, 1, file(V1, 1));
    record(&mut table, 2, file(V1, 1));
    assert!(!check(&mut table, 2, file(V1, 1), &ticket).is_clear());
    assert!(!check(&mut table, 3, file(V1, 1), &ticket).is_clear());
}

#[test]
fn a_changed_volume_epoch_fails_tuples_recorded_under_the_old_one() {
    let (mut table, _) = table(ROOMY);
    table
        .set_volume_epoch(context(1), CELL, V1, VolumeEpoch(10))
        .unwrap();
    let ticket = record_bare(&mut table, 1, file(V1, 1));
    assert_eq!(ticket.volume_epoch, Some(VolumeEpoch(10)));
    assert!(check(&mut table, 1, file(V1, 1), &ticket).is_clear());
    table
        .set_volume_epoch(context(1), CELL, V1, VolumeEpoch(11))
        .unwrap();
    assert_eq!(
        check(&mut table, 1, file(V1, 1), &ticket),
        only(|s| s.volume_epoch = true)
    );
    // Another context's epoch is its own.
    table
        .set_volume_epoch(context(2), CELL, V1, VolumeEpoch(12))
        .unwrap();
    let fresh = record_bare(&mut table, 1, file(V1, 1));
    assert!(check(&mut table, 1, file(V1, 1), &fresh).is_clear());
}

#[test]
fn a_volume_epoch_evicted_fails_tuples_recorded_under_it() {
    let (mut table, _) = table(MIN_FENCE_BYTES);
    table
        .set_volume_epoch(context(1), CELL, V1, VolumeEpoch(10))
        .unwrap();
    let ticket = record_bare(&mut table, 1, file(V1, 1));
    record_bare(&mut table, 1, file(V2, 1));
    assert!(!table.holds_volume(context(1), CELL, V1));
    // Made again, with the same epoch written back: the counter still
    // fails the old tuple.
    record_bare(&mut table, 1, file(V1, 1));
    table
        .set_volume_epoch(context(1), CELL, V1, VolumeEpoch(10))
        .unwrap();
    assert_eq!(
        check(&mut table, 1, file(V1, 1), &ticket),
        only(|s| {
            s.object = true;
            s.volume = true;
        })
    );
}

#[test]
fn a_tuple_recorded_with_no_epoch_fails_until_adopted() {
    let (mut table, _) = table(ROOMY);
    let mut ticket = record_bare(&mut table, 1, file(V1, 1));
    assert_eq!(ticket.volume_epoch, None);
    assert_eq!(
        check(&mut table, 1, file(V1, 1), &ticket),
        only(|s| s.volume_epoch = true)
    );
    // The install's own observation writes the epoch, then adopts it.
    table
        .set_volume_epoch(context(1), CELL, V1, VolumeEpoch(10))
        .unwrap();
    assert!(table.adopt_volume_epoch(context(1), file(V1, 1), &mut ticket));
    assert_eq!(ticket.volume_epoch, Some(VolumeEpoch(10)));
    assert!(check(&mut table, 1, file(V1, 1), &ticket).is_clear());
}

#[test]
fn adoption_is_refused_after_the_volume_record_changed() {
    let (mut table, _) = table(ROOMY);
    let mut broken = record_bare(&mut table, 1, file(V1, 1));
    let mut no_epoch = record_bare(&mut table, 2, file(V1, 1));
    table.break_volume(CELL, V1);
    table
        .set_volume_epoch(context(1), CELL, V1, VolumeEpoch(10))
        .unwrap();
    assert!(!table.adopt_volume_epoch(context(1), file(V1, 1), &mut broken));
    assert_eq!(broken.volume_epoch, None);
    // No epoch written in context 2: nothing to adopt.
    let mut unbroken = record_bare(&mut table, 2, file(V1, 2));
    assert!(!table.adopt_volume_epoch(context(2), file(V1, 2), &mut unbroken));
    assert!(!table.adopt_volume_epoch(context(2), file(V1, 1), &mut no_epoch));
    assert!(!check(&mut table, 2, file(V1, 2), &unbroken).is_clear());
}

#[test]
fn adoption_is_refused_for_a_tuple_with_an_epoch() {
    let (mut table, _) = table(ROOMY);
    table
        .set_volume_epoch(context(1), CELL, V1, VolumeEpoch(10))
        .unwrap();
    let mut ticket = record_bare(&mut table, 1, file(V1, 1));
    table
        .set_volume_epoch(context(1), CELL, V1, VolumeEpoch(11))
        .unwrap();
    assert!(!table.adopt_volume_epoch(context(1), file(V1, 1), &mut ticket));
    assert_eq!(ticket.volume_epoch, Some(VolumeEpoch(10)));
}

#[test]
fn set_volume_epoch_refused_changes_nothing_but_the_latch() {
    let (mut table, _) = table(MIN_FENCE_BYTES);
    record_bare(&mut table, 1, file(V1, 1));
    table.next = None;
    assert_eq!(
        table.set_volume_epoch(context(1), CELL, V2, VolumeEpoch(10)),
        Err(FenceError::Exhausted)
    );
    assert!(table.holds_volume(context(1), CELL, V1));
    assert!(table.holds_object(context(1), file(V1, 1)));
    assert_eq!(table.volumes(), 1);
    // A present record's epoch needs no value.
    assert_eq!(
        table.set_volume_epoch(context(1), CELL, V1, VolumeEpoch(10)),
        Ok(())
    );
}

#[test]
fn new_rejects_limits_out_of_range() {
    for bytes in [0, MIN_FENCE_BYTES - 1, MAX_FENCE_BYTES + 1] {
        let watch = Watch(Rc::new(Cell::new(0)));
        assert_eq!(
            FenceTable::new(FenceLimits { bytes }, watch).map(|_| ()),
            Err(FenceError::InvalidLimits)
        );
    }
    for bytes in [MIN_FENCE_BYTES, MAX_FENCE_BYTES] {
        let watch = Watch(Rc::new(Cell::new(0)));
        assert!(FenceTable::new(FenceLimits { bytes }, watch).is_ok());
    }
}

#[test]
fn adoption_is_refused_once_an_epoch_copy_was_evicted() {
    // The context's copy of epoch 10 is evicted while its evidence record
    // lives on; a fetch then records no epoch, the evidence record is
    // reclaimed, and the fetch's own reply writes a new epoch.
    let (mut table, _) = table(MIN_FENCE_BYTES);
    table
        .set_volume_epoch(context(1), CELL, V1, VolumeEpoch(10))
        .unwrap();
    record_bare(&mut table, 1, file(V2, 1));
    assert!(!table.holds_volume(context(1), CELL, V1));
    let mut ticket = record_bare(&mut table, 1, file(V1, 1));
    assert_eq!(ticket.volume_epoch, None);
    table
        .set_volume_epoch(context(1), CELL, V1, VolumeEpoch(11))
        .unwrap();
    assert!(!table.adopt_volume_epoch(context(1), file(V1, 1), &mut ticket));
    assert!(!check(&mut table, 1, file(V1, 1), &ticket).is_clear());
}

/// A clock watch whose read races the driver: while armed, the pause
/// generation rises during the clock comparison, which finds no move.
struct Racing {
    pause: Rc<AtomicU64>,
    armed: Rc<Cell<bool>>,
}

impl ClockWatch for Racing {
    fn moved(&mut self) -> bool {
        if self.armed.get() {
            self.pause.fetch_add(1, Ordering::SeqCst);
        }
        false
    }
}

#[test]
fn record_and_check_load_the_pause_generation_after_the_clocks() {
    let pause = Rc::new(AtomicU64::new(0));
    let armed = Rc::new(Cell::new(false));
    let watch = Racing {
        pause: Rc::clone(&pause),
        armed: Rc::clone(&armed),
    };
    let mut table = FenceTable::new(FenceLimits { bytes: ROOMY }, watch).unwrap();
    let server = Server {
        suspect_epoch: Stamp(1),
        pause: &pause,
    };
    table
        .set_volume_epoch(context(1), CELL, V1, VolumeEpoch(10))
        .unwrap();
    let steady = table.record(context(1), file(V1, 1), server).unwrap();
    assert_eq!(steady.pause, PauseGeneration(0));
    // The driver raises the pause generation while the check compares the
    // clocks: the check sees the raise.
    armed.set(true);
    assert_eq!(
        table.check(context(1), file(V1, 1), server, &steady),
        only(|s| s.pause = true)
    );
    // And a recording made during a raise records the raised value.
    let raced = table.record(context(1), file(V1, 2), server).unwrap();
    assert_eq!(raced.pause, PauseGeneration(2));
}
