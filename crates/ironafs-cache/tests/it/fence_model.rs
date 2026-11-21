// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fence against a reference model, over random operation sequences
//! and over every short sequence of a small domain.
//!
//! The model keeps every tuple recorded and, for each, which of its five
//! values an event has changed since: a break naming its file, a break of
//! its volume, an advance or a new record of its server, a pause or resume
//! of its server's socket, and a suspend or clock jump. After every check
//! it asserts:
//! - every value the model says changed is reported changed, so a reply
//!   recorded before any break on its object, volume or server is refused
//!   after it (ADR 0004, C16, point 2), with or without evictions;
//! - when the table is large enough never to evict, the result is exactly
//!   the model's: nothing else fails a fence.

use ironafs_cache::{
    BreakCounters, BreakLimits, CellIndex, Clocks, ContextKey, EpochSource, FenceTuple, FileId,
    PauseGeneration, ResumeGeneration, ServerValues, Stamp, Suspicion, Uniquifier, Vnode, VolumeId,
};
use ironafs_test_support::{Exhaustive, Property, Source};
use ironafs_vfs::{ContextId, CredentialGeneration};

/// Servers in the domain; volume `v` is on server `v % SERVERS`, and each
/// server's key is on its own socket.
const SERVERS: usize = 2;

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
    Check {
        pick: usize,
    },
}

impl Op {
    fn index(self) -> usize {
        match self {
            Self::Record { .. } => 0,
            Self::BreakFile { .. } => 1,
            Self::BreakVolume { .. } => 2,
            Self::Advance { .. } => 3,
            Self::Pause { .. } => 4,
            Self::Jump => 5,
            Self::Check { .. } => 6,
        }
    }
}

#[derive(Debug)]
struct Case {
    limits: BreakLimits,
    /// Whether the limits are large enough that nothing is ever evicted.
    roomy: bool,
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

impl Domain {
    fn roomy(self) -> BreakLimits {
        let volumes = u32::try_from(self.contexts).unwrap() * self.volumes;
        BreakLimits {
            objects: volumes * self.vnodes,
            volumes,
        }
    }
}

fn op(source: &mut Source, domain: Domain) -> Op {
    let volume =
        |source: &mut Source| u32::try_from(source.u64_in(1..=u64::from(domain.volumes))).unwrap();
    let vnode =
        |source: &mut Source| u32::try_from(source.u64_in(1..=u64::from(domain.vnodes))).unwrap();
    match source.usize_in(0..=6) {
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
        _ => Op::Check {
            pick: source.usize_in(0..=domain.picks),
        },
    }
}

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

fn server_of(volume: u32) -> usize {
    usize::try_from(volume).unwrap() % SERVERS
}

/// One recorded tuple and what the model says has changed since.
#[derive(Debug)]
struct Ticket {
    context: u64,
    file: FileId,
    tuple: FenceTuple,
    changed: Suspicion,
}

/// The world the table lives in: the callback state records' epochs, the
/// sockets' pause generations, and the client-wide resume generation.
struct World {
    table: BreakCounters,
    epochs: EpochSource,
    resume: ResumeGeneration,
    suspect: [Stamp; SERVERS],
    pause: [u64; SERVERS],
    jumped: bool,
    tickets: Vec<Ticket>,
}

impl World {
    fn new(limits: BreakLimits) -> Result<Self, String> {
        let epochs = EpochSource::new();
        let mut suspect = [Stamp::NONE; SERVERS];
        for value in &mut suspect {
            *value = epochs.take().ok_or("epochs exhausted")?;
        }
        Ok(Self {
            table: BreakCounters::new(limits).map_err(|error| error.to_string())?,
            epochs,
            resume: ResumeGeneration::new(),
            suspect,
            pause: [0; SERVERS],
            jumped: false,
            tickets: Vec::new(),
        })
    }

    fn server(&self, volume: u32) -> ServerValues {
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

    /// Applies `op`; returns whether a check found the fence failed.
    fn apply(&mut self, op: Op, roomy: bool) -> Result<Option<bool>, String> {
        match op {
            Op::Record {
                context: ctx,
                volume,
                vnode,
            } => {
                let target = file(volume, vnode);
                let tuple = self
                    .table
                    .record(context(ctx), target, self.server(volume), &self.resume)
                    .map_err(|error| format!("record refused: {error}"))?;
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
                self.table.break_volume(CellIndex(0), VolumeId(volume));
                self.mark(
                    |ticket| ticket.file.volume == VolumeId(volume),
                    |changed| changed.volume = true,
                );
            }
            Op::Advance { server } => {
                self.suspect[server] = self.epochs.take().ok_or("epochs exhausted")?;
                self.mark(
                    |ticket| server_of(ticket.file.volume.0) == server,
                    |changed| changed.suspect_epoch = true,
                );
            }
            Op::Pause { server } => {
                self.pause[server] += 1;
                self.mark(
                    |ticket| server_of(ticket.file.volume.0) == server,
                    |changed| changed.pause = true,
                );
            }
            Op::Jump => self.jumped = true,
            Op::Check { pick } => {
                if self.tickets.is_empty() {
                    return Ok(None);
                }
                let clocks = if std::mem::take(&mut self.jumped) {
                    // The check raises the resume generation before it
                    // compares: every tuple recorded so far fails.
                    self.mark(|_| true, |changed| changed.resume = true);
                    Clocks::Moved
                } else {
                    Clocks::Steady
                };
                let ticket = &self.tickets[pick % self.tickets.len()];
                let got = self.table.check(
                    &self.resume,
                    clocks,
                    context(ticket.context),
                    ticket.file,
                    self.server(ticket.file.volume.0),
                    &ticket.tuple,
                );
                let want = ticket.changed;
                let missed = [
                    (want.object, got.object, "object"),
                    (want.volume, got.volume, "volume"),
                    (want.suspect_epoch, got.suspect_epoch, "suspect epoch"),
                    (want.pause, got.pause, "pause"),
                    (want.resume, got.resume, "resume"),
                ]
                .into_iter()
                .find(|&(want, got, _)| want && !got);
                if let Some((_, _, value)) = missed {
                    return Err(format!(
                        "{ticket:?}: {value} changed, check reported {got:?}"
                    ));
                }
                if roomy && got != want {
                    return Err(format!("{ticket:?}: no eviction, check reported {got:?}"));
                }
                return Ok(Some(!got.is_clear()));
            }
        }
        Ok(None)
    }
}

/// Runs `case`, counting the operations of each kind and the checks that
/// failed and held.
fn run(case: &Case, ran: &mut [u64; 7], verdicts: &mut [u64; 2]) -> Result<(), String> {
    let mut world = World::new(case.limits)?;
    for (step, &op) in case.ops.iter().enumerate() {
        let verdict = world
            .apply(op, case.roomy)
            .map_err(|error| format!("step {step} {op:?}: {error}"))?;
        ran[op.index()] += 1;
        if let Some(failed) = verdict {
            verdicts[usize::from(failed)] += 1;
        }
        if world.table.objects() > case.limits.objects as usize
            || world.table.volumes() > case.limits.volumes as usize
        {
            return Err(format!("step {step}: over the limits {:?}", case.limits));
        }
    }
    Ok(())
}

#[track_caller]
fn assert_covered(ran: &[u64; 7], verdicts: &[u64; 2]) {
    assert!(
        ran.iter().all(|&count| count > 0),
        "an operation never ran: {ran:?}"
    );
    assert!(
        verdicts.iter().all(|&count| count > 0),
        "checks never both held and failed: {verdicts:?}"
    );
}

const DOMAIN: Domain = Domain {
    contexts: 3,
    volumes: 3,
    vnodes: 3,
    picks: 15,
};

#[test]
fn fence_refuses_every_reply_recorded_before_a_break() {
    let mut ran = [0; 7];
    let mut verdicts = [0; 2];
    Property::new("fence refuses every reply recorded before a break")
        .cases(512)
        .check(
            |source| Case {
                limits: DOMAIN.roomy(),
                roomy: true,
                ops: source.vec_of(0..=48, |source| op(source, DOMAIN)),
            },
            |case| run(case, &mut ran, &mut verdicts),
        );
    assert_covered(&ran, &verdicts);
}

#[test]
fn fence_refuses_every_reply_recorded_before_a_break_under_eviction() {
    let mut ran = [0; 7];
    let mut verdicts = [0; 2];
    Property::new("fence refuses every reply recorded before a break, under eviction")
        .cases(512)
        .check(
            |source| Case {
                limits: BreakLimits {
                    objects: u32::try_from(source.u64_in(1..=4)).unwrap(),
                    volumes: u32::try_from(source.u64_in(1..=3)).unwrap(),
                },
                roomy: false,
                ops: source.vec_of(0..=48, |source| op(source, DOMAIN)),
            },
            |case| run(case, &mut ran, &mut verdicts),
        );
    assert_covered(&ran, &verdicts);
}

#[test]
fn fence_matches_its_reference_model_on_every_short_sequence() {
    // Every sequence of four operations over two contexts, two volumes on
    // two servers, one vnode and two picks, in a table that never evicts;
    // the sweep prints how many it ran and the bound.
    let small = Domain {
        contexts: 2,
        volumes: 2,
        vnodes: 1,
        picks: 1,
    };
    let mut ran = [0; 7];
    let mut verdicts = [0; 2];
    Exhaustive::new("fence, every four-operation sequence", 500_000).check(
        |source| Case {
            limits: small.roomy(),
            roomy: true,
            ops: (0..4).map(|_| op(source, small)).collect(),
        },
        |case| run(case, &mut ran, &mut verdicts),
    );
    assert_covered(&ran, &verdicts);
}
