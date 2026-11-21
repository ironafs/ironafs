// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The scenarios, in the library, so the child process, the replay
//! command and the tests call the same function (test strategy
//! specification, 5.1, point 7).
//!
//! The `Rpc` and `Disk` seams do not exist yet, so the sample operation is
//! the minimal scripted operation of the seams and scheduling
//! specification's 10.5, point 5, and section 13, Q11: it reads the real
//! `Clock` and draws from the real `Rng` of `ironafs-time`, and yields at
//! the simulator's own reply-slot and deadline waits, which the `Rpc` and
//! `Disk` implementations will use.

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use ironafs_time::{BootDeadline, ClockKind};

use crate::replay::Replay;
use crate::sched::{Config, Outcome, SchedPlants, Setup, SimDeadline, TaskCx, Wait, Waited, run};
use crate::trace::ErrorCode;

#[cfg(test)]
mod tests;

/// A plant: a deliberate defect a test switches on to show a check fails.
/// Every evidence run has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Plant {
    /// The writers read the version, yield, then write it plus one: a
    /// lost update on some schedules (a planted failing schedule).
    LostUpdate,
    /// A reader records the process id: a host value that differs between
    /// two processes.
    LeakPid,
    /// A reader records a heap address: a host value that differs between
    /// two processes.
    LeakAddress,
    /// Every stream ignores the seed: two seeds choose the same identities
    /// while the printed seed differs.
    FixedSchedule,
    /// The scheduler keeps running the task that ran last: each task runs
    /// its whole critical window alone.
    Segregate,
    /// The race fixture's second meeting event names another object.
    OtherObject,
}

impl Plant {
    const ALL: [Self; 6] = [
        Self::LostUpdate,
        Self::LeakPid,
        Self::LeakAddress,
        Self::FixedSchedule,
        Self::Segregate,
        Self::OtherObject,
    ];

    /// The plant's name on the command line and in a trace's header.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::LostUpdate => "lost-update",
            Self::LeakPid => "leak-pid",
            Self::LeakAddress => "leak-address",
            Self::FixedSchedule => "fixed-schedule",
            Self::Segregate => "segregate",
            Self::OtherObject => "other-object",
        }
    }

    /// The plant named `text`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|plant| plant.name() == text)
    }
}

/// A scenario: its name, its golden seeds, and how it sets a run up.
#[derive(Clone, Copy)]
pub struct Scenario {
    /// The scenario's name.
    pub name: &'static str,
    /// The seeds whose whole traces are committed as golden fixtures.
    pub golden_seeds: &'static [u64],
    build: fn(&mut Setup, &[Plant]),
}

impl fmt::Debug for Scenario {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Scenario")
            .field("name", &self.name)
            .field("golden_seeds", &self.golden_seeds)
            .finish_non_exhaustive()
    }
}

/// Every scenario.
pub const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "sample",
        golden_seeds: &[1, 2, 3, 4],
        build: sample,
    },
    Scenario {
        name: "race",
        golden_seeds: &[],
        build: race,
    },
    Scenario {
        name: "segregation",
        golden_seeds: &[],
        build: segregation,
    },
];

/// The scenario named `name`.
#[must_use]
pub fn find(name: &str) -> Option<&'static Scenario> {
    SCENARIOS.iter().find(|scenario| scenario.name == name)
}

impl Scenario {
    /// The run's configuration for `seed` with `plants`, which the
    /// header names.
    #[must_use]
    pub fn config(&self, seed: u64, plants: &[Plant]) -> Config {
        let mut config = Config::new(self.name, seed);
        let mut names: Vec<&str> = plants.iter().map(|plant| plant.name()).collect();
        names.sort_unstable();
        names.dedup();
        config.label = names.join(",");
        config.plants = SchedPlants {
            segregate: plants.contains(&Plant::Segregate),
            fixed_seed: plants.contains(&Plant::FixedSchedule).then_some(0),
            ..SchedPlants::default()
        };
        config
    }

    /// Runs the scenario once, in this process.
    #[must_use]
    pub fn run(
        &self,
        seed: u64,
        plants: &[Plant],
        sink: Option<Box<dyn std::io::Write + Send>>,
        replay: Option<Replay>,
    ) -> Outcome {
        let build = self.build;
        run(&self.config(seed, plants), sink, replay, |setup| {
            build(setup, plants);
        })
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A deadline `after` from now on the boot clock, or the error code of
/// why none could be built.
fn boot_deadline(cx: &TaskCx, after: Duration) -> Result<SimDeadline, ErrorCode> {
    let now = cx.clock().boot().map_err(|error| match error.clock() {
        ClockKind::Boot => ErrorCode::ClockBoot,
        ClockKind::Awake => ErrorCode::ClockAwake,
        ClockKind::Wall => ErrorCode::ClockWall,
    })?;
    BootDeadline::after(now, after)
        .map(SimDeadline::Boot)
        .ok_or(ErrorCode::DeadlineOverflow)
}

fn waited_code(waited: Waited) -> ErrorCode {
    match waited {
        Waited::Cancelled | Waited::Signalled => ErrorCode::Cancelled,
        Waited::ClockFailed => ErrorCode::ClockBoot,
        Waited::DeadlinePassed => ErrorCode::DeadlinePassed,
    }
}

/// Yields to a deadline `after` from now.
fn pause(cx: &TaskCx, after: Duration) -> Result<(), ErrorCode> {
    let deadline = boot_deadline(cx, after)?;
    match cx.wait(Wait {
        signal: None,
        cancel: None,
        deadline,
    }) {
        Waited::DeadlinePassed => Ok(()),
        other => Err(waited_code(other)),
    }
}

/// The server model's file: its version and the bytes of that version.
#[derive(Debug)]
struct File {
    version: u64,
}

/// The bytes of version `version` of the file: eight bytes, so a reply
/// that mixed two versions is seen.
fn contents(version: u64) -> [u8; 8] {
    version.wrapping_mul(0x9e37_79b9_7f4a_7c15).to_be_bytes()
}

const FETCH: u8 = 0;
const BUMP: u8 = 1;
const GET: u8 = 2;
const SET: u8 = 3;

/// The server model of the sample scenario. A fetch returns the version
/// and its contents; a bump moves the version by one; a get returns the
/// version; a set writes the version it is given.
fn serve(file: &Mutex<File>, request: &[u8]) -> Vec<u8> {
    let mut file = lock(file);
    match request.split_first() {
        Some((&FETCH, _)) => {
            let mut reply = file.version.to_be_bytes().to_vec();
            reply.extend_from_slice(&contents(file.version));
            reply
        }
        Some((&BUMP, _)) => {
            file.version = file.version.wrapping_add(1);
            file.version.to_be_bytes().to_vec()
        }
        Some((&GET, _)) => file.version.to_be_bytes().to_vec(),
        Some((&SET, rest)) => {
            if let Some(version) = be_u64(rest) {
                file.version = version;
            }
            file.version.to_be_bytes().to_vec()
        }
        _ => Vec::new(),
    }
}

fn be_u64(bytes: &[u8]) -> Option<u64> {
    let head: [u8; 8] = bytes.get(..8)?.try_into().ok()?;
    Some(u64::from_be_bytes(head))
}

/// What the sample's readers saw that they should not have.
#[derive(Debug, Default)]
struct Seen {
    mixed_replies: u64,
}

/// The sample scenario: one server; two readers fetching a file, each
/// with a fresh nonce from `Rng` and a deadline from `Clock`; and two
/// writers changing it. Its invariants: no reader takes a reply whose
/// contents are not its version's, and at the end the version is the
/// start plus one per writer.
fn sample(setup: &mut Setup, plants: &[Plant]) {
    const START: u64 = 7;
    let file = Arc::new(Mutex::new(File { version: START }));
    let server_file = Arc::clone(&file);
    setup.server(move |request: &[u8]| serve(&server_file, request));
    let seen = Arc::new(Mutex::new(Seen::default()));
    for _ in 0..2 {
        let seen = Arc::clone(&seen);
        let plants = plants.to_vec();
        setup.task("reader", move |cx: &TaskCx| {
            if let Err(code) = reader(cx, &seen, &plants) {
                cx.error(code);
            }
        });
    }
    let lost_update = plants.contains(&Plant::LostUpdate);
    for _ in 0..2 {
        setup.task("writer", move |cx: &TaskCx| {
            if let Err(code) = writer(cx, lost_update) {
                cx.error(code);
            }
        });
    }
    let writes = Arc::clone(&file);
    setup.check_at_end(move || {
        if lock(&writes).version == START + 2 {
            Ok(())
        } else {
            Err("every-write-applied")
        }
    });
    setup.check_each_step(move || {
        if lock(&seen).mixed_replies == 0 {
            Ok(())
        } else {
            Err("reply-matches-its-version")
        }
    });
}

fn reader(cx: &TaskCx, seen: &Mutex<Seen>, plants: &[Plant]) -> Result<(), ErrorCode> {
    if plants.contains(&Plant::LeakPid) {
        cx.op("host", "pid", Some(u64::from(std::process::id())));
    }
    if plants.contains(&Plant::LeakAddress) {
        let boxed = Box::new(0_u8);
        let address = (&raw const *boxed).addr();
        cx.op("host", "address", u64::try_from(address).ok());
    }
    for _ in 0..2 {
        let mut nonce = [0_u8; 4];
        cx.rng().fill(&mut nonce).map_err(|_| ErrorCode::Rng)?;
        let mut request = vec![FETCH];
        request.extend_from_slice(&nonce);
        let deadline = boot_deadline(cx, Duration::from_millis(50))?;
        cx.op("fetch", "file:f", None);
        match cx.call(&request, deadline) {
            Ok(reply) => {
                let version = be_u64(&reply).unwrap_or(u64::MAX);
                if reply.get(8..) != Some(contents(version).as_slice()) {
                    lock(seen).mixed_replies += 1;
                }
                cx.op("fetched", "file:f", Some(version));
            }
            Err(waited) => cx.error(waited_code(waited)),
        }
        let mut think = [0_u8; 1];
        cx.rng().fill(&mut think).map_err(|_| ErrorCode::Rng)?;
        let [think] = think;
        pause(cx, Duration::from_micros(u64::from(think) * 20))?;
    }
    Ok(())
}

fn writer(cx: &TaskCx, lost_update: bool) -> Result<(), ErrorCode> {
    let deadline = boot_deadline(cx, Duration::from_millis(50))?;
    if !lost_update {
        cx.op("bump", "file:f", None);
        let reply = cx.call(&[BUMP], deadline).map_err(waited_code)?;
        cx.op("bumped", "file:f", be_u64(&reply));
        return Ok(());
    }
    cx.op("get", "file:f", None);
    let reply = cx.call(&[GET], deadline).map_err(waited_code)?;
    let version = be_u64(&reply).unwrap_or_default();
    cx.op("got", "file:f", Some(version));
    let mut request = vec![SET];
    request.extend_from_slice(&version.wrapping_add(1).to_be_bytes());
    cx.op("set", "file:f", Some(version.wrapping_add(1)));
    cx.call(&request, deadline).map_err(waited_code)?;
    Ok(())
}

/// The race fixture: task `a` opens a window on object `x`, takes two
/// steps with its meeting event `a` between them, and closes it; task `b`
/// takes a seeded number of steps, then its meeting event `b` on `x` (on
/// `y` with [`Plant::OtherObject`]).
fn race(setup: &mut Setup, plants: &[Plant]) {
    setup.task("opener", |cx: &TaskCx| {
        let steps = || -> Result<(), ErrorCode> {
            cx.op("open", "x", None);
            pause(cx, Duration::ZERO)?;
            cx.op("a", "x", None);
            pause(cx, Duration::ZERO)?;
            cx.op("close", "x", None);
            Ok(())
        };
        if let Err(code) = steps() {
            cx.error(code);
        }
    });
    let object = if plants.contains(&Plant::OtherObject) {
        "y"
    } else {
        "x"
    };
    setup.task("meeter", move |cx: &TaskCx| {
        let steps = || -> Result<(), ErrorCode> {
            let mut count = [0_u8; 1];
            cx.rng().fill(&mut count).map_err(|_| ErrorCode::Rng)?;
            let [count] = count;
            for _ in 0..count % 4 {
                pause(cx, Duration::ZERO)?;
            }
            cx.op("b", object, None);
            Ok(())
        };
        if let Err(code) = steps() {
            cx.error(code);
        }
    });
}

/// Three tasks of three steps each, every step an operation event naming
/// its task: the fixture whose share of fully segregated schedules is
/// measured.
fn segregation(setup: &mut Setup, _plants: &[Plant]) {
    for name in ["a", "b", "c"] {
        setup.task("worker", move |cx: &TaskCx| {
            for round in 0..3_u64 {
                cx.op("work", name, Some(round));
                if round < 2
                    && let Err(code) = pause(cx, Duration::ZERO)
                {
                    cx.error(code);
                    return;
                }
            }
        });
    }
}
