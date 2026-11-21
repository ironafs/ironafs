// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The baton scheduler (seams and scheduling specification, 5.6).
//!
//! Each task is a standard-library thread, and a baton lets exactly one
//! run. A task gives the baton back only at a yield point or at its end.
//! The scheduler alone chooses the next step: a task whose wake condition
//! holds, or a world event due now, drawn from the seed's schedule stream
//! and logged with the whole enabled list. Every step advances the
//! virtual clocks by the step cost; with nothing enabled, time jumps to
//! the earliest pending deadline or event.
//!
//! All scheduling state is under one mutex. A task waits on its own
//! condition variable until its grant count passes its seen count, so a
//! grant posted before the thread reaches its gate is never lost, and a
//! spurious wake only re-checks the counts.
//!
//! The one real-time read is the watchdog on the baton (5.6, point 9): a
//! named departure from coding standards rule 7, in [`Run::grant`]. It
//! never feeds a decision, so it cannot change a trace.

mod task;

use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ironafs_test_support::{SeededClock, SeededRng};
use ironafs_time::{AwakeDeadline, BootDeadline, BootInstant, Clock};

use crate::replay::Replay;
use crate::stream::{Stream, stream_seed};
use crate::trace::{Decision, Entry, EventKind, How, TaskId, VERSION_LINE};

pub use task::TaskCx;

#[cfg(test)]
mod tests;

/// A task's code: runs on its own thread, and yields only through its
/// [`TaskCx`].
pub type Body = Box<dyn FnOnce(&TaskCx) + Send>;

/// A check of the world, after every step or at the end of a run: `Err`
/// names the invariant that broke.
pub type Check = Box<dyn FnMut() -> Result<(), &'static str> + Send>;

/// A server model: the reply to one request's bytes.
pub type Server = Box<dyn FnMut(&[u8]) -> Vec<u8> + Send>;

/// A deadline on one of the two monotonic clocks. No wait counts on wall
/// time (ADR 0004, C37, point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimDeadline {
    /// On the boot clock, which counts suspend.
    Boot(BootDeadline),
    /// On the awake clock, which does not.
    Awake(AwakeDeadline),
}

/// How a wait ended, in the order of precedence of the specification's
/// 5.3, point 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waited {
    /// The cancel token fired.
    Cancelled,
    /// The deadline's clock could not be read.
    ClockFailed,
    /// The deadline's clock reads at or past it.
    DeadlinePassed,
    /// The signal is raised.
    Signalled,
}

impl Waited {
    /// The outcome's name in a trace.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::ClockFailed => "clock-failed",
            Self::DeadlinePassed => "deadline-passed",
            Self::Signalled => "signalled",
        }
    }
}

/// A flag a task waits on: a signal a producer raises, or a cancel token
/// an owner fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SignalId(u64);

/// What a waiting task waits for (5.6, point 5): its wake condition holds
/// when any of these is true.
#[derive(Clone, Copy, Debug)]
pub struct Wait {
    /// The signal that ends the wait when raised.
    pub signal: Option<SignalId>,
    /// The cancel token that ends the wait when fired.
    pub cancel: Option<SignalId>,
    /// The deadline: every wait has one.
    pub deadline: SimDeadline,
}

/// Test plants in the scheduler itself (specification, 10.5, point 6).
/// Each is off in every evidence run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SchedPlants {
    /// Post each task's first grant before its thread exists.
    pub grant_before_spawn: bool,
    /// Notify every task's condition variable at every decision.
    pub spurious_wakes: bool,
    /// Keep running the task that ran last while it is enabled, so each
    /// task runs its whole critical window alone.
    pub segregate: bool,
    /// Seed every stream with this value instead of the run's seed, which
    /// the header still prints: a change to a printed field alone.
    pub fixed_seed: Option<u64>,
}

/// A run's parameters.
#[derive(Clone, Debug)]
pub struct Config {
    /// The scenario's name, printed in the header.
    pub scenario: &'static str,
    /// The run's seed.
    pub seed: u64,
    /// The incarnation, from 0.
    pub incarnation: u64,
    /// What each step costs in virtual time (5.6, point 7).
    pub step_cost: Duration,
    /// The most decisions a run may take ("budget reached").
    pub budget: u64,
    /// The age at which an enabled entry is overdue (5.6, point 6).
    pub fairness: u64,
    /// The most live tasks (5.6, point 2).
    pub live_cap: usize,
    /// The real-time limit on one step (5.6, point 9).
    pub watchdog: Duration,
    /// Each task thread's stack size, in bytes.
    pub stack_size: usize,
    /// The range of a delivery's latency, in nanoseconds.
    pub latency: (u64, u64),
    /// Plants, all off in an evidence run.
    pub plants: SchedPlants,
    /// Words printed in the header after the seed: the plants of the
    /// scenario, so a replay names its inputs (5.6, point 11, (d)).
    pub label: String,
}

impl Config {
    /// The defaults for a scenario and seed.
    #[must_use]
    pub fn new(scenario: &'static str, seed: u64) -> Self {
        Self {
            scenario,
            seed,
            incarnation: 0,
            step_cost: Duration::from_micros(10),
            budget: 100_000,
            fairness: 64,
            live_cap: 64,
            watchdog: Duration::from_secs(30),
            stack_size: 256 * 1024,
            latency: (100_000, 5_000_000),
            plants: SchedPlants::default(),
            label: String::new(),
        }
    }
}

/// Why a run failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// A scenario's invariant broke.
    Invariant(&'static str),
    /// Work was open and nothing was pending.
    Stuck,
    /// The decision budget ran out.
    Budget,
    /// More live tasks than the cap.
    TooManyTasks,
    /// A task panicked; the message is kept for the report, never put in
    /// the trace.
    TaskPanicked {
        /// The task.
        task: TaskId,
        /// Its panic message.
        message: String,
    },
    /// The holder kept the baton past the watchdog: unconfirmed.
    LostBaton {
        /// The task.
        task: TaskId,
        /// The decision whose step it was running.
        decision: u64,
    },
    /// A replay's trace differed from the original's at this decision.
    Diverged {
        /// The decision.
        decision: u64,
    },
    /// The code under test misused the simulator, such as an operation
    /// label outside the trace's alphabet.
    Misuse(&'static str),
    /// The host failed the run: a thread that could not be spawned, or
    /// the trace's output could not be written. Never a pass, never a
    /// breach of a client invariant (5.6, point 10).
    Host(&'static str),
}

impl Failure {
    /// The failure's code in the trace's terminal line.
    #[must_use]
    pub fn code(&self) -> String {
        match self {
            Self::Invariant(name) => format!("invariant:{name}"),
            Self::Stuck => "stuck".to_owned(),
            Self::Budget => "budget".to_owned(),
            Self::TooManyTasks => "too-many-tasks".to_owned(),
            Self::TaskPanicked { task, .. } => format!("task-panicked:{task}"),
            Self::LostBaton { task, .. } => format!("lost-baton:{task}"),
            Self::Diverged { decision } => format!("diverged:d{decision}"),
            Self::Misuse(what) => format!("misuse:{what}"),
            Self::Host(what) => format!("host:{what}"),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskPanicked { task, message } => {
                write!(formatter, "task {task} panicked: {message}")
            }
            Self::LostBaton { task, decision } => write!(
                formatter,
                "unconfirmed lost baton: task {task} ran past the watchdog in the step of \
                 decision {decision}"
            ),
            other => formatter.write_str(&other.code()),
        }
    }
}

/// The result of a run.
#[derive(Debug)]
pub struct Outcome {
    /// The whole trace, through its terminal line.
    pub trace: String,
    /// `None` if the run passed.
    pub failure: Option<Failure>,
}

/// What a scenario sets up before the first decision.
pub struct Setup {
    tasks: Vec<(&'static str, Body)>,
    server: Option<Server>,
    each_step: Vec<Check>,
    at_end: Vec<Check>,
}

impl fmt::Debug for Setup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Setup")
            .field("tasks", &self.tasks.len())
            .field("server", &self.server.is_some())
            .finish_non_exhaustive()
    }
}

impl Setup {
    /// Adds a task, named for the trace. Task ids follow the order of
    /// these calls.
    pub fn task(&mut self, name: &'static str, body: impl FnOnce(&TaskCx) + Send + 'static) {
        self.tasks.push((name, Box::new(body)));
    }

    /// Sets the server model that answers every [`TaskCx::call`].
    pub fn server(&mut self, server: impl FnMut(&[u8]) -> Vec<u8> + Send + 'static) {
        self.server = Some(Box::new(server));
    }

    /// Adds a check run after every step (ADR 0004, O45, point 3).
    pub fn check_each_step(
        &mut self,
        check: impl FnMut() -> Result<(), &'static str> + Send + 'static,
    ) {
        self.each_step.push(Box::new(check));
    }

    /// Adds a check run once every task has ended.
    pub fn check_at_end(
        &mut self,
        check: impl FnMut() -> Result<(), &'static str> + Send + 'static,
    ) {
        self.at_end.push(Box::new(check));
    }
}

/// Runs one incarnation of a scenario to its end and returns its trace.
///
/// `sink`, if given, receives every trace line as it is appended, flushed
/// before the scheduler acts on it, so a supervising parent always holds
/// the committed prefix (5.6, point 10). `replay`, if given, feeds the
/// recorded decisions back and stops the run at the first line that
/// differs (5.6, point 11).
pub fn run(
    config: &Config,
    sink: Option<Box<dyn Write + Send>>,
    replay: Option<Replay>,
    scenario: impl FnOnce(&mut Setup),
) -> Outcome {
    let mut setup = Setup {
        tasks: Vec::new(),
        server: None,
        each_step: Vec::new(),
        at_end: Vec::new(),
    };
    scenario(&mut setup);
    let mut run = Run::new(config, sink, replay, setup.server.take());
    run.start(setup.tasks);
    let failure = run.drive(&mut setup.each_step, &mut setup.at_end);
    run.finish(failure)
}

/// The state every task and the scheduler share, under one mutex.
struct Shared {
    state: Mutex<State>,
    /// The scheduler waits here until the holder is none.
    scheduler: Condvar,
    clock: Arc<SeededClock>,
    rng: Arc<SeededRng>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunState {
    Starting,
    Running,
    Parked,
    Ended,
}

struct Slot {
    grants: u64,
    seen: u64,
    run: RunState,
    wake: Option<Wait>,
    condvar: Arc<Condvar>,
    /// The decision at which the task last became enabled or last ran.
    since: Option<u64>,
    /// Set while the run tears the task down: its next grant unwinds it.
    teardown: bool,
    thread: Option<JoinHandle<()>>,
    /// The body of a task whose thread is spawned at its first grant.
    unspawned: Option<Body>,
}

struct Pending {
    due: BootInstant,
    since: Option<u64>,
    action: Delivery,
}

/// A delivery: the server model's reply to a call, put in its slot.
struct Delivery {
    call: u64,
    signal: SignalId,
    request: Vec<u8>,
}

struct State {
    holder: Option<TaskId>,
    tasks: BTreeMap<TaskId, Slot>,
    signals: BTreeMap<SignalId, bool>,
    next_signal: u64,
    replies: BTreeMap<u64, Vec<u8>>,
    next_call: u64,
    events: BTreeMap<(EventKind, u64), Pending>,
    next_event: u64,
    server: Option<Server>,
    network: Stream,
    latency: (u64, u64),
    /// The number of the current decision: lines a step writes carry it.
    decision: u64,
    lines: Vec<String>,
    sink: Option<Box<dyn Write + Send>>,
    replay: Option<Replay>,
    /// Set from the cut on: lines are counted, not appended (5.6, point 8).
    cut: bool,
    dropped_lines: u64,
    failure: Option<Failure>,
}

impl State {
    /// Appends one line to the trace, writes it to the sink, and checks
    /// it against the replay.
    fn emit(&mut self, line: String) {
        if self.cut {
            self.dropped_lines = self.dropped_lines.saturating_add(1);
            return;
        }
        if let Some(sink) = &mut self.sink {
            let written = sink
                .write_all(line.as_bytes())
                .and_then(|()| sink.write_all(b"\n"))
                .and_then(|()| sink.flush());
            if written.is_err() {
                self.fail(Failure::Host("trace-output"));
            }
        }
        if let Some(replay) = &mut self.replay
            && !replay.matches_next(&line)
        {
            let decision = self.decision;
            self.fail(Failure::Diverged { decision });
        }
        self.lines.push(line);
    }

    /// Records the run's first failure; later ones are consequences.
    fn fail(&mut self, failure: Failure) {
        if self.failure.is_none() {
            self.failure = Some(failure);
        }
    }

    fn new_signal(&mut self) -> SignalId {
        let id = SignalId(self.next_signal);
        self.next_signal = self.next_signal.saturating_add(1);
        self.signals.insert(id, false);
        id
    }

    fn is_set(&self, signal: Option<SignalId>) -> bool {
        signal.is_some_and(|id| self.signals.get(&id).copied().unwrap_or(false))
    }

    /// How a wait on `wait` ends now, if it does, by the precedence of
    /// 5.3, point 4.
    fn outcome(&self, wait: &Wait, clock: &SeededClock) -> Option<Waited> {
        if self.is_set(wait.cancel) {
            return Some(Waited::Cancelled);
        }
        let passed = match wait.deadline {
            SimDeadline::Boot(deadline) => clock.boot().map(|now| deadline.has_passed(now)),
            SimDeadline::Awake(deadline) => clock.awake().map(|now| deadline.has_passed(now)),
        };
        match passed {
            Err(_) => Some(Waited::ClockFailed),
            Ok(true) => Some(Waited::DeadlinePassed),
            Ok(false) => self.is_set(wait.signal).then_some(Waited::Signalled),
        }
    }
}

struct Run {
    shared: Arc<Shared>,
    config: Config,
    schedule: Stream,
    /// The task that ran last, for the segregating plant.
    last: Option<TaskId>,
}

impl Run {
    fn new(
        config: &Config,
        sink: Option<Box<dyn Write + Send>>,
        replay: Option<Replay>,
        server: Option<Server>,
    ) -> Self {
        let seed = config.plants.fixed_seed.unwrap_or(config.seed);
        let incarnation = config.incarnation;
        let state = State {
            holder: None,
            tasks: BTreeMap::new(),
            signals: BTreeMap::new(),
            next_signal: 0,
            replies: BTreeMap::new(),
            next_call: 0,
            events: BTreeMap::new(),
            next_event: 0,
            server,
            network: Stream::new(seed, "network", incarnation),
            latency: config.latency,
            decision: 0,
            lines: Vec::new(),
            sink,
            replay,
            cut: false,
            dropped_lines: 0,
            failure: None,
        };
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(state),
                scheduler: Condvar::new(),
                clock: Arc::new(SeededClock::from_seed(stream_seed(
                    seed,
                    "clock",
                    incarnation,
                ))),
                rng: Arc::new(SeededRng::new(stream_seed(seed, "rng", incarnation))),
            }),
            config: config.clone(),
            schedule: Stream::new(seed, "schedule", incarnation),
            last: None,
        }
    }

    /// Writes the header and creates every task: ids first, then threads.
    fn start(&mut self, tasks: Vec<(&'static str, Body)>) {
        let mut state = self.shared.lock();
        state.emit(VERSION_LINE.to_owned());
        let label = if self.config.label.is_empty() {
            "-"
        } else {
            self.config.label.as_str()
        };
        state.emit(format!(
            "d0 sched run {} seed={} incarnation={} plants={label}",
            self.config.scenario, self.config.seed, self.config.incarnation
        ));
        if tasks.len() > self.config.live_cap {
            state.fail(Failure::TooManyTasks);
        }
        for (index, (name, body)) in tasks.into_iter().enumerate() {
            let id = TaskId(u64::try_from(index).unwrap_or(u64::MAX));
            state.emit(format!("d0 sched spawn {id} {name}"));
            state.tasks.insert(
                id,
                Slot {
                    grants: 0,
                    seen: 0,
                    run: RunState::Starting,
                    wake: None,
                    condvar: Arc::new(Condvar::new()),
                    since: None,
                    teardown: false,
                    thread: None,
                    unspawned: Some(body),
                },
            );
        }
        if !self.config.plants.grant_before_spawn {
            let ids: Vec<TaskId> = state.tasks.keys().copied().collect();
            for id in ids {
                self.spawn(&mut state, id);
            }
        }
    }

    /// Spawns the thread of task `id`, whose first act is to wait at its
    /// gate for a grant.
    fn spawn(&self, state: &mut State, id: TaskId) {
        let Some(slot) = state.tasks.get_mut(&id) else {
            return;
        };
        let Some(body) = slot.unspawned.take() else {
            return;
        };
        let shared = Arc::clone(&self.shared);
        let condvar = Arc::clone(&slot.condvar);
        let spawned = thread::Builder::new()
            .stack_size(self.config.stack_size)
            .spawn(move || task::main(shared, id, condvar, body));
        if let Ok(handle) = spawned {
            slot.thread = Some(handle);
        } else {
            slot.run = RunState::Ended;
            state.fail(Failure::Host("thread-spawn"));
        }
    }

    /// Runs decisions until every task has ended or the run fails.
    fn drive(&mut self, each_step: &mut [Check], at_end: &mut [Check]) -> Option<Failure> {
        let shared = Arc::clone(&self.shared);
        loop {
            let mut state = shared.lock();
            if let Some(failure) = state.failure.clone() {
                return Some(failure);
            }
            if state.tasks.values().all(|slot| slot.run == RunState::Ended) {
                drop(state);
                return at_end
                    .iter_mut()
                    .find_map(|check| check().err())
                    .map(Failure::Invariant);
            }
            if state.decision >= self.config.budget {
                return Some(Failure::Budget);
            }
            let enabled = self.enabled(&mut state);
            if enabled.is_empty() {
                if let Err(failure) = self.jump(&mut state) {
                    return Some(failure);
                }
                continue;
            }
            let (pick, how) = match self.choose(&mut state, &enabled) {
                Ok(choice) => choice,
                Err(failure) => return Some(failure),
            };
            let Some(chosen) = enabled.get(pick).copied() else {
                return Some(Failure::Diverged {
                    decision: state.decision,
                });
            };
            let at = self.shared.clock.boot().map_or(0, BootInstant::as_nanos);
            let line = Decision {
                number: state.decision,
                enabled,
                pick,
                how,
                draws: self.schedule.draws(),
                at,
            }
            .render();
            state.emit(line);
            if self.config.plants.spurious_wakes {
                for slot in state.tasks.values() {
                    slot.condvar.notify_all();
                }
            }
            let state = match chosen {
                Entry::Task(id) => {
                    self.last = Some(id);
                    match self.grant(state, id) {
                        Ok(state) => state,
                        Err(failure) => return Some(failure),
                    }
                }
                Entry::Event { kind, creation } => {
                    self.last = None;
                    Self::run_event(&mut state, kind, creation);
                    state
                }
            };
            let decision = state.decision;
            drop(state);
            if self.shared.clock.advance(self.config.step_cost).is_err() {
                return Some(Failure::Host("virtual-time-overflow"));
            }
            if let Some(name) = each_step.iter_mut().find_map(|check| check().err()) {
                return Some(Failure::Invariant(name));
            }
            self.shared.lock().decision = decision.saturating_add(1);
        }
    }

    /// The enabled list, in its fixed order (5.6, point 5): tasks whose
    /// wake condition holds, by id; then world events due now, by kind
    /// and creation. Read while no task runs.
    fn enabled(&self, state: &mut State) -> Vec<Entry> {
        let decision = state.decision;
        let mut entries = Vec::new();
        let ready: Vec<(TaskId, bool)> = state
            .tasks
            .iter()
            .map(|(id, slot)| {
                let ready = match slot.run {
                    RunState::Starting => true,
                    RunState::Parked => slot
                        .wake
                        .is_some_and(|wait| state.outcome(&wait, &self.shared.clock).is_some()),
                    RunState::Running | RunState::Ended => false,
                };
                (*id, ready)
            })
            .collect();
        for (id, ready) in ready {
            if let Some(slot) = state.tasks.get_mut(&id) {
                if ready {
                    slot.since.get_or_insert(decision);
                    entries.push(Entry::Task(id));
                } else {
                    slot.since = None;
                }
            }
        }
        let now = self.shared.clock.boot().ok();
        for ((kind, creation), pending) in &mut state.events {
            if now.is_some_and(|now| pending.due <= now) {
                pending.since.get_or_insert(decision);
                entries.push(Entry::Event {
                    kind: *kind,
                    creation: *creation,
                });
            }
        }
        entries
    }

    /// The index of the entry to run, and how it was chosen (5.6, point
    /// 6): the overdue entry of greatest age if any, else a draw.
    fn choose(&mut self, state: &mut State, enabled: &[Entry]) -> Result<(usize, How), Failure> {
        let decision = state.decision;
        let age = |entry: &Entry| -> u64 {
            let since = match entry {
                Entry::Task(id) => state.tasks.get(id).and_then(|slot| slot.since),
                Entry::Event { kind, creation } => state
                    .events
                    .get(&(*kind, *creation))
                    .and_then(|pending| pending.since),
            };
            since.map_or(0, |since| decision.saturating_sub(since))
        };
        let mut overdue: Option<(usize, u64)> = None;
        for (index, entry) in enabled.iter().enumerate() {
            let entry_age = age(entry);
            if entry_age >= self.config.fairness
                && overdue.is_none_or(|(_, oldest)| entry_age > oldest)
            {
                overdue = Some((index, entry_age));
            }
        }
        if let Some((index, _)) = overdue {
            return Ok((index, How::Overdue));
        }
        if self.config.plants.segregate
            && let Some(index) = self
                .last
                .and_then(|last| enabled.iter().position(|entry| *entry == Entry::Task(last)))
        {
            return Ok((index, How::Drawn));
        }
        let drawn = self.schedule.index(enabled.len());
        let pick = match &mut state.replay {
            Some(replay) => replay.pick(decision)?,
            None => drawn.unwrap_or(0),
        };
        Ok((pick, How::Drawn))
    }

    /// Moves virtual time to the earliest pending deadline or event.
    fn jump(&self, state: &mut State) -> Result<(), Failure> {
        let clock = &self.shared.clock;
        let mut earliest: Option<Duration> = None;
        let mut consider = |left: Duration| {
            earliest = Some(earliest.map_or(left, |known| known.min(left)));
        };
        for slot in state.tasks.values() {
            if let (RunState::Parked, Some(wait)) = (slot.run, slot.wake) {
                let left = match wait.deadline {
                    SimDeadline::Boot(deadline) => clock.boot().map(|now| deadline.remaining(now)),
                    SimDeadline::Awake(deadline) => {
                        clock.awake().map(|now| deadline.remaining(now))
                    }
                };
                if let Ok(left) = left {
                    consider(left);
                }
            }
        }
        if let Ok(now) = clock.boot() {
            for pending in state.events.values() {
                consider(
                    pending
                        .due
                        .checked_duration_since(now)
                        .unwrap_or(Duration::ZERO),
                );
            }
        }
        let Some(left) = earliest else {
            return Err(Failure::Stuck);
        };
        if left.is_zero() {
            // Something is due yet nothing is enabled: a simulator defect.
            return Err(Failure::Stuck);
        }
        clock
            .advance(left)
            .map_err(|_| Failure::Host("virtual-time-overflow"))?;
        let decision = state.decision;
        state.emit(format!("d{decision} sched time +{}", left.as_nanos()));
        Ok(())
    }

    /// Grants the baton to task `id` and waits until it hands it back.
    fn grant<'a>(
        &self,
        mut state: MutexGuard<'a, State>,
        id: TaskId,
    ) -> Result<MutexGuard<'a, State>, Failure> {
        let Some(slot) = state.tasks.get_mut(&id) else {
            return Err(Failure::Misuse("grant-unknown-task"));
        };
        slot.grants = slot.grants.saturating_add(1);
        slot.since = None;
        slot.run = RunState::Running;
        slot.condvar.notify_all();
        state.holder = Some(id);
        self.spawn(&mut state, id);
        // The watchdog: the simulator's one real-time read on the baton,
        // a named departure from coding standards rule 7 (5.6, point 9).
        // It never feeds a decision.
        let (state, waited) = self
            .shared
            .scheduler
            .wait_timeout_while(state, self.config.watchdog, |state| state.holder.is_some())
            .unwrap_or_else(PoisonError::into_inner);
        if waited.timed_out() && state.holder.is_some() {
            return Err(Failure::LostBaton {
                task: id,
                decision: state.decision,
            });
        }
        Ok(state)
    }

    /// Runs a world event on the scheduler's thread.
    fn run_event(state: &mut State, kind: EventKind, creation: u64) {
        let Some(pending) = state.events.remove(&(kind, creation)) else {
            return;
        };
        let Delivery {
            call,
            signal,
            request,
        } = pending.action;
        let reply = state
            .server
            .as_mut()
            .map_or_else(Vec::new, |server| server(&request));
        let decision = state.decision;
        state.emit(format!("d{decision} world reply c{call} {}", hex(&reply)));
        state.replies.insert(call, reply);
        state.signals.insert(signal, true);
    }

    /// Ends the run: tears down every task still alive, joins every
    /// thread, and writes the terminal line.
    fn finish(self, failure: Option<Failure>) -> Outcome {
        let mut state = self.shared.lock();
        let decision = state.decision;
        let end = failure.as_ref().map_or_else(
            || "end ok".to_owned(),
            |failure| format!("end failed {}", failure.code()),
        );
        state.emit(format!("d{decision} sched {end}"));
        let unused = state
            .replay
            .as_ref()
            .is_some_and(|replay| !replay.is_exhausted());
        state.cut = true;
        let lost = matches!(failure, Some(Failure::LostBaton { .. }));
        let alive: Vec<TaskId> = state
            .tasks
            .iter()
            .filter(|(_, slot)| slot.run != RunState::Ended)
            .map(|(id, _)| *id)
            .collect();
        for id in alive {
            if lost && state.holder == Some(id) {
                continue;
            }
            let Some(slot) = state.tasks.get_mut(&id) else {
                continue;
            };
            if slot.unspawned.take().is_some() {
                slot.run = RunState::Ended;
                continue;
            }
            slot.teardown = true;
            match self.grant(state, id) {
                Ok(next) => state = next,
                Err(_) => return Self::outcome_without_join(&self.shared, failure),
            }
        }
        let threads: Vec<JoinHandle<()>> = state
            .tasks
            .values_mut()
            .filter(|slot| slot.run == RunState::Ended)
            .filter_map(|slot| slot.thread.take())
            .collect();
        drop(state);
        for thread in threads {
            // A task's panic was caught inside the thread; joining cannot
            // carry one.
            let _ = thread.join();
        }
        let mut state = self.shared.lock();
        let trace = take_trace(&mut state);
        let failure = match failure {
            None if unused => Some(Failure::Diverged { decision }),
            other => other,
        };
        Outcome { trace, failure }
    }

    fn outcome_without_join(shared: &Shared, failure: Option<Failure>) -> Outcome {
        let mut state = shared.lock();
        Outcome {
            trace: take_trace(&mut state),
            failure,
        }
    }
}

fn take_trace(state: &mut State) -> String {
    let mut trace = String::new();
    for line in state.lines.drain(..) {
        trace.push_str(&line);
        trace.push('\n');
    }
    trace
}

/// Lower-case hexadecimal, two digits a byte; `-` for no bytes.
fn hex(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "-".to_owned();
    }
    let mut text = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _ = write_hex(&mut text, *byte);
    }
    text
}

fn write_hex(text: &mut String, byte: u8) -> fmt::Result {
    use std::fmt::Write as _;
    write!(text, "{byte:02x}")
}

/// Catches a task body's unwind; used by [`task::main`].
fn catch(body: impl FnOnce()) -> Result<(), Box<dyn std::any::Any + Send>> {
    panic::catch_unwind(AssertUnwindSafe(body))
}
