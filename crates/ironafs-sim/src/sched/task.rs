// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The task side of the baton: the gate, the yield, the end, and the
//! handle the code under test runs with.

use std::panic;
use std::sync::{Arc, Condvar, MutexGuard};

use ironafs_time::{Clock, Rng};

use super::{
    Action, Body, Delivery, Failure, Incoming, RunState, Shared, SignalId, SimDeadline, State,
    Wait, Waited, catch, hex,
};
use crate::trace::{ErrorCode, EventKind, TaskId, is_word};

/// The unwind payload that tears a task down at the end of a run, or at a
/// crash's cut: never a failure of the task.
struct Teardown;

/// What a task's code runs with: its seams and its yield points.
///
/// Between two yields a task runs alone, so it reads only its arguments,
/// seam results and state that tasks wrote in decided order (5.6, point
/// 11, (b)).
pub struct TaskCx {
    shared: Arc<Shared>,
    id: TaskId,
    condvar: Arc<Condvar>,
}

impl std::fmt::Debug for TaskCx {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TaskCx")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// A task thread's whole life: the gate, the body inside an unwind
/// catcher, and the end, which always hands the baton back.
pub(super) fn main(shared: Arc<Shared>, id: TaskId, condvar: Arc<Condvar>, body: Body) {
    let cx = TaskCx {
        shared,
        id,
        condvar,
    };
    let state = cx.shared.lock();
    let (state, teardown) = cx.await_grant(state);
    drop(state);
    let result = if teardown {
        Ok(())
    } else {
        catch(|| body(&cx))
    };
    let mut state = cx.shared.lock();
    match result {
        Ok(()) => {
            let decision = state.decision;
            state.emit(format!("d{decision} {id} end returned"));
        }
        Err(payload) if payload.is::<Teardown>() => {}
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            let decision = state.decision;
            state.emit(format!("d{decision} {id} end panicked"));
            state.fail(Failure::TaskPanicked { task: id, message });
        }
    }
    if let Some(slot) = state.tasks.get_mut(&id) {
        slot.run = RunState::Ended;
        slot.wake = None;
    }
    state.holder = None;
    cx.shared.scheduler.notify_all();
}

impl TaskCx {
    /// This task's id.
    #[must_use]
    pub const fn id(&self) -> TaskId {
        self.id
    }

    /// The run's clock: three virtual clocks the scheduler moves.
    #[must_use]
    pub fn clock(&self) -> &dyn Clock {
        &*self.shared.clock
    }

    /// The run's random source, seeded from (seed, `rng`, incarnation).
    #[must_use]
    pub fn rng(&self) -> &dyn Rng {
        &*self.shared.rng
    }

    /// Records an operation event: `label` happened to `object`, with an
    /// optional value. Both are words of the trace's alphabet; anything
    /// else fails the run as a misuse.
    pub fn op(&self, label: &str, object: &str, value: Option<u64>) {
        let mut state = self.shared.lock();
        if !is_word(label) || !is_word(object) {
            state.fail(Failure::Misuse("op-word"));
            return;
        }
        let decision = state.decision;
        let id = self.id;
        let line = match value {
            Some(value) => format!("d{decision} {id} op {label} {object} {value}"),
            None => format!("d{decision} {id} op {label} {object}"),
        };
        state.emit(line);
    }

    /// Records an error the task met, by its code in the closed vocabulary.
    pub fn error(&self, code: ErrorCode) {
        let mut state = self.shared.lock();
        let decision = state.decision;
        state.emit(format!("d{decision} {} error {}", self.id, code.name()));
    }

    /// A new signal, lowered.
    #[must_use]
    pub fn signal(&self) -> SignalId {
        self.shared.lock().new_signal()
    }

    /// Raises `signal`, or fires it as a cancel token. Idempotent.
    pub fn raise(&self, signal: SignalId) {
        self.shared.lock().signals.insert(signal, true);
    }

    /// Lowers `signal`.
    pub fn lower(&self, signal: SignalId) {
        self.shared.lock().signals.insert(signal, false);
    }

    /// Yields until `wait`'s condition holds, and returns how it ended
    /// (5.3, point 4). A yield point (5.6, point 3).
    #[must_use = "a wait can end without its signal: the caller decides what each outcome means"]
    pub fn wait(&self, wait: Wait) -> Waited {
        let mut state = self.shared.lock();
        if let Some(slot) = state.tasks.get_mut(&self.id) {
            slot.wake = Some(wait);
            slot.run = RunState::Parked;
        }
        state.holder = None;
        self.shared.scheduler.notify_all();
        let (state, teardown) = self.await_grant(state);
        if teardown {
            drop(state);
            panic::resume_unwind(Box::new(Teardown));
        }
        let outcome = state.outcome(&wait, &self.shared.clock);
        let mut state = state;
        let decision = state.decision;
        if let Some(outcome) = outcome {
            state.emit(format!("d{decision} {} waited {}", self.id, outcome.name()));
            return outcome;
        }
        // Granted with no wake condition holding: the scheduler broke its
        // own rule. Fail the run and end the wait as passed.
        state.fail(Failure::Misuse("grant-without-wake"));
        Waited::DeadlinePassed
    }

    /// Sends `request` to the server model and waits for its reply, by
    /// `deadline`. A yield point: the wait on a reply slot.
    ///
    /// # Errors
    ///
    /// Returns how the wait ended if no reply arrived before it did.
    pub fn call(&self, request: &[u8], deadline: SimDeadline) -> Result<Vec<u8>, Waited> {
        let (call, signal) = {
            let mut state = self.shared.lock();
            let call = state.next_call;
            state.next_call = call.saturating_add(1);
            let signal = state.new_signal();
            let decision = state.decision;
            state.emit(format!(
                "d{decision} {} call c{call} {}",
                self.id,
                hex(request)
            ));
            schedule_delivery(&mut state, &self.shared, call, signal, request);
            (call, signal)
        };
        match self.wait(Wait {
            signal: Some(signal),
            cancel: None,
            deadline,
        }) {
            Waited::Signalled => Ok(self.shared.lock().replies.remove(&call).unwrap_or_default()),
            other => Err(other),
        }
    }

    /// Takes the next world call for `endpoint`, waiting for one by
    /// `deadline`. A yield point: the wait on the endpoint's queue.
    ///
    /// # Errors
    ///
    /// Returns how the wait ended if no call arrived before it did.
    pub fn accept(&self, endpoint: u32, deadline: SimDeadline) -> Result<Incoming, Waited> {
        let signal = self.shared.lock().inbox_signal(endpoint);
        loop {
            match self.wait(Wait {
                signal: Some(signal),
                cancel: None,
                deadline,
            }) {
                Waited::Signalled => {}
                other => return Err(other),
            }
            let mut state = self.shared.lock();
            if let Some((push, request)) = state.take_incoming(endpoint) {
                let decision = state.decision;
                state.emit(format!(
                    "d{decision} {} accept p{push} {}",
                    self.id,
                    hex(&request)
                ));
                return Ok(Incoming { push, request });
            }
        }
    }

    /// Answers a world call: the answer reaches the world after a latency
    /// drawn from the network stream, unless the call has settled by then.
    /// Not a yield point.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "taken by value so each world call is answered once"
    )]
    pub fn respond(&self, incoming: Incoming, reply: &[u8]) {
        let mut state = self.shared.lock();
        let decision = state.decision;
        let push = incoming.push;
        state.emit(format!(
            "d{decision} {} respond p{push} {}",
            self.id,
            hex(reply)
        ));
        if !state.pushes.contains_key(&push) {
            return;
        }
        let now = self
            .shared
            .clock
            .boot()
            .unwrap_or(ironafs_time::BootInstant::from_nanos(0));
        let latency = state.latency();
        let due = now.checked_add(latency).unwrap_or(now);
        state.queue(
            EventKind::Answer,
            due,
            Action::Answer {
                push,
                reply: reply.to_vec(),
            },
        );
    }

    /// Waits at the gate, with the mutex released, until a new grant is
    /// posted; returns whether that grant tears the task down.
    fn await_grant<'a>(&self, mut state: MutexGuard<'a, State>) -> (MutexGuard<'a, State>, bool) {
        loop {
            let Some(slot) = state.tasks.get_mut(&self.id) else {
                return (state, true);
            };
            if slot.grants > slot.seen {
                slot.seen = slot.grants;
                slot.run = RunState::Running;
                slot.wake = None;
                let teardown = slot.teardown;
                return (state, teardown);
            }
            state = self
                .condvar
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}

/// Queues the server model's reply to `call` as a world event, after a
/// latency drawn from the network stream.
fn schedule_delivery(
    state: &mut State,
    shared: &Shared,
    call: u64,
    signal: SignalId,
    request: &[u8],
) {
    let latency = state.latency();
    let now = shared
        .clock
        .boot()
        .unwrap_or(ironafs_time::BootInstant::from_nanos(0));
    let due = now.checked_add(latency).unwrap_or(now);
    state.queue(
        EventKind::Delivery,
        due,
        Action::Deliver(Delivery {
            call,
            signal,
            request: request.to_vec(),
        }),
    );
}
