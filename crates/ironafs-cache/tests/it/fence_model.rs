// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fence against a reference model, over random operation sequences
//! and over every short sequence of a small domain, with and without
//! eviction.
//!
//! The model keeps every tuple recorded and, for each, which of its values
//! an event has changed since: a break naming its file, a break of its
//! volume, the eviction of its object or volume record, an advance of its
//! server, a pause of its server's socket, and a suspend or clock jump. It
//! keeps each context's local volume epoch as last written, and forgets it
//! when the volume record is evicted. It decides evictions itself: it
//! mirrors the table's sequence of counter values and its rule (when full,
//! the smallest value goes, never a record the operation uses), predicts
//! every value a tuple records, and after every operation asserts that the
//! table holds exactly the records it expects, no more and no fewer. Every
//! check must report exactly what the model expects: a reply
//! recorded before any change of its object, volume, server, socket,
//! clocks or epoch is refused after it (ADR 0004, C16, point 2; C25,
//! point 3), and nothing else fails a fence. An adoption must succeed
//! exactly when no epoch copy has ever been evicted, the tuple has no
//! epoch, its volume record is unchanged since, and an epoch has been
//! written.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use ironafs_cache::{
    ClockWatch, EpochSource, FenceLimits, FenceTable, FenceTuple, FileId, MIN_FENCE_BYTES,
    SLOT_BYTES, Server, Stamp, Suspicion, VolumeEpoch, VolumeId,
};
use ironafs_test_support::{Exhaustive, Property, Source};

use crate::fixture::{CELL, context, file};

/// Servers in the domain; volume `v` is on server `v % SERVERS`, and each
/// server's key is on its own socket.
const SERVERS: usize = 2;

/// A clock watch that reports the moves the model makes, once.
struct Watch(Rc<Cell<u32>>);

impl ClockWatch for Watch {
    fn moved(&mut self) -> bool {
        self.0.replace(0) > 0
    }
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Record {
        context: u64,
        volume: u32,
        vnode: u32,
    },
    BreakFile {
        volume: u32,
        vnode: u32,
    },
    BreakVolume {
        volume: u32,
    },
    Advance {
        server: usize,
    },
    Pause {
        server: usize,
    },
    Jump,
    SetEpoch {
        context: u64,
        volume: u32,
    },
    Adopt {
        pick: usize,
    },
    Check {
        pick: usize,
    },
}

const KINDS: usize = 9;

impl Op {
    fn index(self) -> usize {
        match self {
            Self::Record { .. } => 0,
            Self::BreakFile { .. } => 1,
            Self::BreakVolume { .. } => 2,
            Self::Advance { .. } => 3,
            Self::Pause { .. } => 4,
            Self::Jump => 5,
            Self::SetEpoch { .. } => 6,
            Self::Adopt { .. } => 7,
            Self::Check { .. } => 8,
        }
    }
}

#[derive(Debug)]
struct Case {
    bytes: usize,
    ops: Vec<Op>,
}

/// The domain's sizes.
#[derive(Clone, Copy)]
struct Domain {
    contexts: u64,
    volumes: u32,
    vnodes: u32,
    picks: usize,
}

fn op(source: &mut Source, domain: Domain) -> Op {
    let volume =
        |source: &mut Source| u32::try_from(source.u64_in(1..=u64::from(domain.volumes))).unwrap();
    let vnode =
        |source: &mut Source| u32::try_from(source.u64_in(1..=u64::from(domain.vnodes))).unwrap();
    match source.usize_in(0..=KINDS - 1) {
        0 => Op::Record {
            context: source.u64_in(1..=domain.contexts),
            volume: volume(source),
            vnode: vnode(source),
        },
        1 => Op::BreakFile {
            volume: volume(source),
            vnode: vnode(source),
        },
        2 => Op::BreakVolume {
            volume: volume(source),
        },
        3 => Op::Advance {
            server: source.usize_in(0..=SERVERS - 1),
        },
        4 => Op::Pause {
            server: source.usize_in(0..=SERVERS - 1),
        },
        5 => Op::Jump,
        6 => Op::SetEpoch {
            context: source.u64_in(1..=domain.contexts),
            volume: volume(source),
        },
        7 => Op::Adopt {
            pick: source.usize_in(0..=domain.picks),
        },
        _ => Op::Check {
            pick: source.usize_in(0..=domain.picks),
        },
    }
}

/// What a tuple takes from the server of `volume`.
fn server<'a>(
    suspect: &[Stamp; SERVERS],
    pause: &'a [AtomicU64; SERVERS],
    volume: VolumeId,
) -> Server<'a> {
    let index = server_of(volume);
    Server {
        suspect_epoch: suspect[index],
        pause: &pause[index],
    }
}

fn server_of(volume: VolumeId) -> usize {
    usize::try_from(volume.0).unwrap() % SERVERS
}

/// One recorded tuple and what the model says has changed since. The
/// local volume epoch is compared with the model's at each check instead.
#[derive(Debug)]
struct Ticket {
    context: u64,
    file: FileId,
    tuple: FenceTuple,
    changed: Suspicion,
}

/// A record, as the model names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Object(u64, FileId),
    Volume(u64, VolumeId),
}

/// The model's own store: which records exist and their values, decided
/// without asking the table.
struct Store {
    capacity: usize,
    next: u64,
    values: BTreeMap<Key, u64>,
}

impl Store {
    /// Evicts the smallest values, other than `keep`, until `needed` more
    /// fit; returns what it evicted.
    fn make_room(&mut self, needed: usize, keep: &[Key]) -> Vec<Key> {
        let mut evicted = Vec::new();
        while self.values.len() + needed > self.capacity {
            let Some(victim) = self
                .values
                .iter()
                .filter(|(key, _)| !keep.contains(key))
                .min_by_key(|(_, value)| **value)
                .map(|(key, _)| *key)
            else {
                break;
            };
            self.values.remove(&victim);
            evicted.push(victim);
        }
        evicted
    }

    /// Gives `key` the next value, made or raised.
    fn stamp(&mut self, key: Key) -> u64 {
        let value = self.next;
        self.next += 1;
        self.values.insert(key, value);
        value
    }

    /// Raises every record of `key`'s file or volume, in context order,
    /// as the table's indexes order them.
    fn raise_all(&mut self, matches: impl Fn(&Key) -> bool) {
        let keys: Vec<Key> = self
            .values
            .keys()
            .copied()
            .filter(|key| matches(key))
            .collect();
        for key in keys {
            self.stamp(key);
        }
    }
}

/// What a run exercised.
#[derive(Default)]
struct Coverage {
    ran: [u64; KINDS],
    /// Checks that held and that failed.
    verdicts: [u64; 2],
    /// Adoptions refused and made.
    adoptions: [u64; 2],
    evictions: u64,
}

/// The world the table lives in: the callback state records' epochs, the
/// sockets' pause generations, the clocks, and the incarnation code's
/// epochs.
struct World {
    table: FenceTable<Watch>,
    moves: Rc<Cell<u32>>,
    epochs: EpochSource,
    suspect: [Stamp; SERVERS],
    pause: [AtomicU64; SERVERS],
    jumped: bool,
    store: Store,
    /// Whether the model has evicted a volume record with an epoch copy.
    copy_lost: bool,
    tickets: Vec<Ticket>,
    /// Each context's local volume epoch, as last written.
    written: BTreeMap<(u64, VolumeId), VolumeEpoch>,
    last_epoch: u64,
}

impl World {
    fn new(bytes: usize) -> Result<Self, String> {
        let epochs = EpochSource::new();
        let mut suspect = [Stamp::NONE; SERVERS];
        for value in &mut suspect {
            *value = epochs.take().ok_or("epochs exhausted")?;
        }
        let moves = Rc::new(Cell::new(0));
        Ok(Self {
            table: FenceTable::new(FenceLimits { bytes }, Watch(Rc::clone(&moves)))
                .map_err(|error| error.to_string())?,
            moves,
            epochs,
            suspect,
            pause: [AtomicU64::new(0), AtomicU64::new(0)],
            jumped: false,
            store: Store {
                capacity: bytes / SLOT_BYTES,
                next: 1,
                values: BTreeMap::new(),
            },
            copy_lost: false,
            tickets: Vec::new(),
            written: BTreeMap::new(),
            last_epoch: 0,
        })
    }

    fn mark(&mut self, applies: impl Fn(&Ticket) -> bool, change: impl Fn(&mut Suspicion)) {
        for ticket in &mut self.tickets {
            if applies(ticket) {
                change(&mut ticket.changed);
            }
        }
    }

    /// The table consults the clocks before a recording and a check: a
    /// jump since the last fails every tuple recorded before it.
    fn clocks_observed(&mut self) {
        if std::mem::take(&mut self.jumped) {
            self.mark(|_| true, |changed| changed.resume = true);
        }
    }

    /// Applies the model's own evictions: each changes the tuples recorded
    /// under the record, and an evicted volume record forgets its epoch.
    fn evicted(&mut self, keys: Vec<Key>, coverage: &mut Coverage) {
        for key in keys {
            coverage.evictions += 1;
            match key {
                Key::Object(ctx, target) => self.mark(
                    |ticket| ticket.context == ctx && ticket.file == target,
                    |changed| changed.object = true,
                ),
                Key::Volume(ctx, volume) => {
                    self.mark(
                        |ticket| ticket.context == ctx && ticket.file.volume == volume,
                        |changed| changed.volume = true,
                    );
                    self.copy_lost |= self.written.remove(&(ctx, volume)).is_some();
                }
            }
        }
    }

    /// Asserts that the table holds exactly the model's records.
    fn holds_what_the_model_holds(&self) -> Result<(), String> {
        let (mut objects, mut volumes) = (0, 0);
        for key in self.store.values.keys() {
            let held = match *key {
                Key::Object(ctx, target) => {
                    objects += 1;
                    self.table.holds_object(context(ctx), target)
                }
                Key::Volume(ctx, volume) => {
                    volumes += 1;
                    self.table.holds_volume(context(ctx), CELL, volume)
                }
            };
            if !held {
                return Err(format!("the table lost {key:?}"));
            }
        }
        if (self.table.objects(), self.table.volumes()) != (objects, volumes) {
            return Err(format!(
                "the table holds {} objects and {} volumes, the model {objects} and {volumes}",
                self.table.objects(),
                self.table.volumes()
            ));
        }
        Ok(())
    }

    fn epoch(&self, ctx: u64, volume: VolumeId) -> Option<VolumeEpoch> {
        self.written.get(&(ctx, volume)).copied()
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per operation, each applying it to the table and the model"
    )]
    fn apply(&mut self, op: Op, coverage: &mut Coverage) -> Result<(), String> {
        match op {
            Op::Record {
                context: ctx,
                volume,
                vnode,
            } => {
                self.clocks_observed();
                let target = file(volume, vnode);
                let object_key = Key::Object(ctx, target);
                let volume_key = Key::Volume(ctx, target.volume);
                let keep: Vec<Key> = [object_key, volume_key]
                    .into_iter()
                    .filter(|key| self.store.values.contains_key(key))
                    .collect();
                let evicted = self.store.make_room(2 - keep.len(), &keep);
                self.evicted(evicted, coverage);
                let object_value = match self.store.values.get(&object_key) {
                    Some(&value) => value,
                    None => self.store.stamp(object_key),
                };
                let volume_value = match self.store.values.get(&volume_key) {
                    Some(&value) => value,
                    None => self.store.stamp(volume_key),
                };
                let tuple = self
                    .table
                    .record(
                        context(ctx),
                        target,
                        server(&self.suspect, &self.pause, target.volume),
                    )
                    .map_err(|error| format!("record refused: {error}"))?;
                if (tuple.object.get(), tuple.volume.get()) != (object_value, volume_value)
                    || tuple.volume_epoch != self.epoch(ctx, target.volume)
                {
                    return Err(format!(
                        "recorded {tuple:?}, expected values {object_value} and {volume_value}, epoch {:?}",
                        self.epoch(ctx, target.volume)
                    ));
                }
                self.tickets.push(Ticket {
                    context: ctx,
                    file: target,
                    tuple,
                    changed: Suspicion::default(),
                });
            }
            Op::BreakFile { volume, vnode } => {
                let target = file(volume, vnode);
                self.table.break_file(&target);
                self.store
                    .raise_all(|key| matches!(*key, Key::Object(_, held) if held == target));
                self.mark(
                    |ticket| ticket.file == target,
                    |changed| changed.object = true,
                );
            }
            Op::BreakVolume { volume } => {
                self.table.break_volume(CELL, VolumeId(volume));
                self.store.raise_all(
                    |key| matches!(*key, Key::Volume(_, held) if held == VolumeId(volume)),
                );
                self.mark(
                    |ticket| ticket.file.volume == VolumeId(volume),
                    |changed| changed.volume = true,
                );
            }
            Op::Advance { server } => {
                self.suspect[server] = self.epochs.take().ok_or("epochs exhausted")?;
                self.mark(
                    |ticket| server_of(ticket.file.volume) == server,
                    |changed| changed.suspect_epoch = true,
                );
            }
            Op::Pause { server } => {
                self.pause[server].fetch_add(1, Ordering::SeqCst);
                self.mark(
                    |ticket| server_of(ticket.file.volume) == server,
                    |changed| changed.pause = true,
                );
            }
            Op::Jump => {
                self.moves.set(self.moves.get() + 1);
                self.jumped = true;
            }
            Op::SetEpoch {
                context: ctx,
                volume,
            } => {
                self.last_epoch += 1;
                let epoch = VolumeEpoch(self.last_epoch);
                let key = Key::Volume(ctx, VolumeId(volume));
                if !self.store.values.contains_key(&key) {
                    let evicted = self.store.make_room(1, &[]);
                    self.evicted(evicted, coverage);
                    self.store.stamp(key);
                }
                self.table
                    .set_volume_epoch(context(ctx), CELL, VolumeId(volume), epoch)
                    .map_err(|error| format!("epoch refused: {error}"))?;
                self.written.insert((ctx, VolumeId(volume)), epoch);
            }
            Op::Adopt { pick } => {
                if self.tickets.is_empty() {
                    return Ok(());
                }
                let index = pick % self.tickets.len();
                let ticket = &self.tickets[index];
                let written = self.epoch(ticket.context, ticket.file.volume);
                let want = !self.copy_lost
                    && ticket.tuple.volume_epoch.is_none()
                    && !ticket.changed.volume
                    && written.is_some();
                let mut tuple = ticket.tuple;
                let got =
                    self.table
                        .adopt_volume_epoch(context(ticket.context), ticket.file, &mut tuple);
                if got != want {
                    return Err(format!("{ticket:?}: adoption {got}, expected {want}"));
                }
                let expected = if want {
                    written
                } else {
                    ticket.tuple.volume_epoch
                };
                if tuple.volume_epoch != expected {
                    return Err(format!("{ticket:?}: adopted {:?}", tuple.volume_epoch));
                }
                self.tickets[index].tuple = tuple;
                coverage.adoptions[usize::from(got)] += 1;
            }
            Op::Check { pick } => {
                self.clocks_observed();
                if self.tickets.is_empty() {
                    return Ok(());
                }
                let ticket = &self.tickets[pick % self.tickets.len()];
                let got = self.table.check(
                    context(ticket.context),
                    ticket.file,
                    server(&self.suspect, &self.pause, ticket.file.volume),
                    &ticket.tuple,
                );
                let recorded = ticket.tuple.volume_epoch;
                let want = Suspicion {
                    volume_epoch: recorded.is_none()
                        || recorded != self.epoch(ticket.context, ticket.file.volume),
                    ..ticket.changed
                };
                if got != want {
                    return Err(format!(
                        "{ticket:?}: check reported {got:?}, expected {want:?}"
                    ));
                }
                coverage.verdicts[usize::from(!got.is_clear())] += 1;
            }
        }
        Ok(())
    }
}

/// Runs `case`, counting what it exercised.
fn run(case: &Case, coverage: &mut Coverage) -> Result<(), String> {
    let mut world = World::new(case.bytes)?;
    for (step, &op) in case.ops.iter().enumerate() {
        world
            .apply(op, coverage)
            .map_err(|error| format!("step {step} {op:?}: {error}"))?;
        world
            .holds_what_the_model_holds()
            .map_err(|error| format!("step {step} {op:?}: {error}"))?;
        coverage.ran[op.index()] += 1;
        if world.table.bytes() > case.bytes
            || world.table.objects() + world.table.volumes() > world.table.capacity()
        {
            return Err(format!(
                "step {step}: {} bytes for a budget of {}",
                world.table.bytes(),
                case.bytes
            ));
        }
    }
    Ok(())
}

#[track_caller]
fn assert_covered(coverage: &Coverage, evicting: bool) {
    assert!(
        coverage.ran.iter().all(|&count| count > 0),
        "an operation never ran: {:?}",
        coverage.ran
    );
    assert!(
        coverage.verdicts.iter().all(|&count| count > 0),
        "checks never both held and failed: {:?}",
        coverage.verdicts
    );
    assert!(
        coverage.adoptions.iter().all(|&count| count > 0),
        "adoptions never both made and refused: {:?}",
        coverage.adoptions
    );
    assert_eq!(
        coverage.evictions > 0,
        evicting,
        "evictions: {}",
        coverage.evictions
    );
}

const DOMAIN: Domain = Domain {
    contexts: 3,
    volumes: 3,
    vnodes: 3,
    picks: 15,
};

#[test]
fn fence_refuses_every_reply_recorded_before_a_change() {
    let mut coverage = Coverage::default();
    Property::new("fence refuses every reply recorded before a change")
        .cases(512)
        .check(
            |source| Case {
                bytes: 64 * MIN_FENCE_BYTES,
                ops: source.vec_of(0..=48, |source| op(source, DOMAIN)),
            },
            |case| run(case, &mut coverage),
        );
    assert_covered(&coverage, false);
}

#[test]
fn fence_refuses_every_reply_recorded_before_a_change_under_eviction() {
    let mut coverage = Coverage::default();
    Property::new("fence refuses every reply recorded before a change, under eviction")
        .cases(512)
        .check(
            |source| Case {
                bytes: (2 + source.usize_in(0..=3)) * SLOT_BYTES,
                ops: source.vec_of(0..=48, |source| op(source, DOMAIN)),
            },
            |case| run(case, &mut coverage),
        );
    assert_covered(&coverage, true);
}

#[test]
fn fence_matches_its_reference_model_on_every_short_sequence() {
    // Every sequence of four operations over two contexts, two volumes on
    // two servers, one vnode and three picks (four operations leave at most
    // three tuples to pick; the random runs pick among more), in a table
    // with room for three records, so that recordings evict; the sweep
    // prints how many it ran and the bound.
    let small = Domain {
        contexts: 2,
        volumes: 2,
        vnodes: 1,
        picks: 2,
    };
    let mut coverage = Coverage::default();
    Exhaustive::new("fence, every four-operation sequence", 500_000).check(
        |source| Case {
            bytes: 3 * SLOT_BYTES,
            ops: (0..4).map(|_| op(source, small)).collect(),
        },
        |case| run(case, &mut coverage),
    );
    assert_covered(&coverage, true);
}
