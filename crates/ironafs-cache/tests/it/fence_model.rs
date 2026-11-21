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
//! when the volume record is evicted. After every operation it reads which
//! records the table still holds, so an eviction is an event like any
//! other. Every check must report exactly what the model expects: a reply
//! recorded before any change of its object, volume, server, socket,
//! clocks or epoch is refused after it (ADR 0004, C16, point 2; C25,
//! point 3), and nothing else fails a fence. An adoption must succeed
//! exactly when the tuple has no epoch, its volume record is unchanged
//! since, and an epoch has been written.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use ironafs_cache::{
    ClockWatch, EpochSource, FenceLimits, FenceTable, FenceTuple, FileId, MIN_FENCE_BYTES,
    OBJECT_RECORD_BYTES, PauseGeneration, ServerValues, Stamp, Suspicion, VOLUME_RECORD_BYTES,
    VolumeEpoch, VolumeId,
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
    pause: [u64; SERVERS],
    jumped: bool,
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
            pause: [0; SERVERS],
            jumped: false,
            tickets: Vec::new(),
            written: BTreeMap::new(),
            last_epoch: 0,
        })
    }

    fn server(&self, volume: VolumeId) -> ServerValues {
        let server = server_of(volume);
        ServerValues {
            suspect_epoch: self.suspect[server],
            pause: PauseGeneration(self.pause[server]),
        }
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

    /// Reads which records the table still holds: an eviction changes the
    /// tuples recorded under the record, and forgets the context's epoch.
    fn evictions_observed(&mut self, coverage: &mut Coverage) {
        let table = &self.table;
        for ticket in &mut self.tickets {
            if !ticket.changed.object && !table.holds_object(context(ticket.context), ticket.file) {
                ticket.changed.object = true;
                coverage.evictions += 1;
            }
            if !ticket.changed.volume
                && !table.holds_volume(context(ticket.context), CELL, ticket.file.volume)
            {
                ticket.changed.volume = true;
                coverage.evictions += 1;
            }
        }
        self.written
            .retain(|&(ctx, volume), _| table.holds_volume(context(ctx), CELL, volume));
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
                let tuple = self
                    .table
                    .record(context(ctx), target, self.server(target.volume))
                    .map_err(|error| format!("record refused: {error}"))?;
                // A record evicts only other records, so a context's epoch
                // is still known if its volume record was kept.
                if self.table.holds_volume(context(ctx), CELL, target.volume)
                    && tuple.volume_epoch != self.epoch(ctx, target.volume)
                {
                    return Err(format!(
                        "recorded {tuple:?}, epoch written {:?}",
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
                self.mark(
                    |ticket| ticket.file == target,
                    |changed| changed.object = true,
                );
            }
            Op::BreakVolume { volume } => {
                self.table.break_volume(CELL, VolumeId(volume));
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
                self.pause[server] += 1;
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
                let want = ticket.tuple.volume_epoch.is_none()
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
                    self.server(ticket.file.volume),
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
        world.evictions_observed(coverage);
        coverage.ran[op.index()] += 1;
        let charged = world.table.objects() * OBJECT_RECORD_BYTES
            + world.table.volumes() * VOLUME_RECORD_BYTES;
        if charged != world.table.bytes() || charged > case.bytes {
            return Err(format!(
                "step {step}: records charge {charged}, table says {}, budget {}",
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
                bytes: MIN_FENCE_BYTES + source.usize_in(0..=3) * OBJECT_RECORD_BYTES,
                ops: source.vec_of(0..=48, |source| op(source, DOMAIN)),
            },
            |case| run(case, &mut coverage),
        );
    assert_covered(&coverage, true);
}

#[test]
fn fence_matches_its_reference_model_on_every_short_sequence() {
    // Every sequence of four operations over two contexts, two volumes on
    // two servers, one vnode and four picks, in a table with room for two
    // object records and one volume record, so that recordings evict; the
    // sweep prints how many it ran and the bound.
    let small = Domain {
        contexts: 2,
        volumes: 2,
        vnodes: 1,
        picks: 3,
    };
    let mut coverage = Coverage::default();
    Exhaustive::new("fence, every four-operation sequence", 500_000).check(
        |source| Case {
            bytes: MIN_FENCE_BYTES + OBJECT_RECORD_BYTES,
            ops: (0..4).map(|_| op(source, small)).collect(),
        },
        |case| run(case, &mut coverage),
    );
    assert_covered(&coverage, true);
}
