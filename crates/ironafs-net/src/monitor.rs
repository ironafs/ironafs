// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Linux network-change monitor's threads
//! (docs/specs/network-generation.md, 5.7 and section 7): they own the
//! two route sockets of `ironafs-os` and feed `ironafs-netgen`'s
//! [`Monitor`], which decides every move.
//!
//! - The **notify thread** receives on the notify socket. The socket is
//!   made, and joins its groups, before the monitor may ask for a
//!   snapshot; after a loss on it (an overflow, a truncated or malformed
//!   datagram, a failed receive) it is made again before the next
//!   snapshot, so no record queued before the loss is applied to fresh
//!   tables. A datagram from a sender other than the kernel is ignored:
//!   it moves nothing. The snapshot is published by this thread too, when
//!   it reads the acknowledgement of the snapshot's barrier, which the
//!   kernel queues after every notification before it.
//! - The **snapshot thread** receives on the request socket. It waits for
//!   the monitor to ask for a dump, then sends and receives without
//!   holding the lock, which it takes only to hand each datagram over;
//!   when the dumps are complete it sends the barrier on the notify
//!   socket. A dump that does not end in order (links, then addresses,
//!   then the barrier) may still run in the kernel on that socket, so the
//!   socket is made again.
//!
//! Both sockets live in the locked state, so the thread that changes the
//! monitor can reach the other thread's socket. **No thread waits for
//! ever:** each has its own [`Wake`], and waits on its socket and that
//! wake together for at most [`ROUTE_WAIT_HORIZON`] before it receives,
//! which never waits (ADR 0005, sheet 23). Whoever needs a thread to look
//! again (a stop, a loss that abandons a dump, a resume) decides it under
//! the lock and then signals that thread's wake. Each turn of a thread
//! clears its wake before it reads the state, so a signal made after the
//! read ends the wait. A failed signal or clear stops the source.
//!
//! TODO(#97): this driver is interim. It keeps 5.7's two threads and only
//! puts sheet 23's wait and wakes under them; section 5.8 of the
//! specification (attempts and their deadlines, the stop model, the
//! startup handoff) replaces it.
//!
//! When either thread ends, or the handle is dropped, the source stops;
//! the handle's drop joins both threads. A hook that drops the handle runs
//! on one of the threads, under the lock: the drop only asks the source to
//! stop ([`StopRequest`]), which the monitor does once that move has been
//! told to every consumer, before publishing it or applying anything more;
//! the thread then ends. A thread unwinding from a
//! hook's panic, or finding the lock poisoned by one, halts it instead: the
//! hooks are not called again.

use std::error::Error;
use std::fmt;
use std::io;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle, ThreadId};
use std::time::Duration;

use ironafs_netgen::{Consumers, Loss, Monitor, Reader, Request, StopRequest, Stopped};
use ironafs_os::{Ready, Receipt, RouteError, RouteSocket, Sent, Waited, Wake};

#[cfg(test)]
mod tests;

/// The receive buffer, in bytes (docs/specs/network-generation.md, 5.5,
/// point 1).
const BUFFER: usize = 65_536;

/// The longest wait on a route socket and its wake (ADR 0005, sheet 23:
/// 1 second, accepted by the owner, 2026-11-22). With nothing to do, a
/// thread looks again once per horizon.
const ROUTE_WAIT_HORIZON: Duration = Duration::from_secs(1);

/// Why the monitor did not start.
#[derive(Debug)]
pub enum MonitorError {
    /// A route socket or a wake could not be made.
    Socket(RouteError),
    /// A thread could not be started.
    Thread(io::Error),
}

impl fmt::Display for MonitorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Socket(error) => {
                write!(
                    formatter,
                    "the network-change monitor has no socket: {error}"
                )
            }
            Self::Thread(error) => {
                write!(
                    formatter,
                    "the network-change monitor did not start: {error}"
                )
            }
        }
    }
}

impl Error for MonitorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Socket(error) => Some(error),
            Self::Thread(error) => Some(error),
        }
    }
}

/// The monitor and both sockets, under one lock.
#[derive(Debug)]
struct State {
    monitor: Monitor,
    notify: Arc<RouteSocket>,
    request: Arc<RouteSocket>,
}

/// The state, the signal that wakes the snapshot thread while it waits
/// for work, each thread's wake, and the request with which a hook that
/// drops the handle stops the source.
#[derive(Debug)]
struct Shared {
    state: Mutex<State>,
    work: Condvar,
    notify_wake: Wake,
    request_wake: Wake,
    stop: StopRequest,
}

impl Shared {
    /// The state. If a hook panicked while it was held, the source is
    /// halted first: its state can no longer be trusted, and its hooks are
    /// not called again.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|poisoned| {
            let mut state = poisoned.into_inner();
            state.monitor.halt();
            state
        })
    }

    /// Wakes both threads: the snapshot thread's wait for work, and each
    /// thread's wait on its socket. Called after the change it signals,
    /// with the lock released.
    fn wake_all(&self) {
        self.work.notify_all();
        // Both are signalled, so one failed wake does not hold up the
        // other thread.
        let notify = self.notify_wake.signal();
        let request = self.request_wake.signal();
        if notify.is_err() || request.is_err() {
            self.failed_wake();
        }
    }

    /// A wake that cannot be signalled or cleared is not recoverable (ADR
    /// 0005, sheet 23): the source stops, and each thread finds the stop
    /// at its next look, at most one horizon later.
    fn failed_wake(&self) {
        self.lock().monitor.stop();
        self.work.notify_all();
    }
}

/// The running monitor. Dropping it stops the source and joins both
/// threads. Dropped by a hook, on one of those threads, it only asks the
/// source to stop and joins nothing: the stop happens once the hook's move
/// has been told to every consumer, before it is published, so the source
/// may still read as live when the drop returns.
#[derive(Debug)]
pub struct MonitorHandle {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
}

impl MonitorHandle {
    /// The machine resumed from suspend: every consumer is told at once,
    /// the generation is unavailable until a fresh snapshot completes, and
    /// the snapshot thread is woken to take it. Whoever detects a resume
    /// (the clock pair) calls it. The consumers' hooks run in the caller.
    ///
    /// # Errors
    ///
    /// [`Stopped`] once the source has stopped.
    pub fn resumed(&self) -> Result<(), Stopped> {
        // The wake is sent even if a hook unwinds.
        let _wake = WakeOnDrop(&self.shared);
        self.shared.lock().monitor.lost(Loss::Resumed)
    }
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        let ids: Vec<ThreadId> = self
            .threads
            .iter()
            .map(|thread| thread.thread().id())
            .collect();
        // A hook that drops the handle runs on one of the threads, which
        // holds the lock and cannot join itself: the stop is left to that
        // thread, which ends once the hook returns, and stops the source.
        if ids.contains(&thread::current().id()) {
            self.shared.stop.ask();
            return;
        }
        stop(&self.shared);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

/// Wakes both threads when dropped, unwinding included.
struct WakeOnDrop<'a>(&'a Shared);

impl Drop for WakeOnDrop<'_> {
    fn drop(&mut self) {
        self.0.wake_all();
    }
}

/// Starts the monitor: subscribes, then starts the threads that take the
/// first snapshot and follow every change. `make` builds the consumers'
/// hooks. The reader is unavailable until the first snapshot completes.
///
/// # Errors
///
/// [`MonitorError`] if a socket cannot be made or a thread cannot start;
/// the consumers are then told the source stopped.
pub fn spawn(
    make: impl FnOnce(&Reader) -> Consumers,
) -> Result<(Reader, MonitorHandle), MonitorError> {
    let (mut monitor, reader) = Monitor::new(make);
    // The notify socket joins its groups before the monitor may ask for
    // anything, so no change falls between the subscription and the
    // snapshot.
    let sockets = RouteSocket::notify()
        .and_then(|notify| Ok((notify, RouteSocket::request()?, Wake::new()?, Wake::new()?)));
    let (notify, request, notify_wake, request_wake) = match sockets {
        Ok(sockets) => sockets,
        Err(error) => {
            monitor.stop();
            return Err(MonitorError::Socket(error));
        }
    };
    monitor.subscribed();
    let stop_request = monitor.stop_request();
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            monitor,
            notify: Arc::new(notify),
            request: Arc::new(request),
        }),
        work: Condvar::new(),
        notify_wake,
        request_wake,
        stop: stop_request,
    });
    let mut handle = MonitorHandle {
        shared: Arc::clone(&shared),
        threads: Vec::new(),
    };
    // On an error below, dropping the handle stops the source and joins
    // what started.
    handle
        .threads
        .push(start("network-notify", &shared, follow).map_err(MonitorError::Thread)?);
    handle
        .threads
        .push(start("network-snapshot", &shared, snapshots).map_err(MonitorError::Thread)?);
    Ok((reader, handle))
}

/// Starts one of the monitor's threads, which stops the source when it
/// ends.
fn start(
    name: &str,
    shared: &Arc<Shared>,
    body: fn(&Shared) -> Result<(), Stopped>,
) -> io::Result<JoinHandle<()>> {
    let shared = Arc::clone(shared);
    thread::Builder::new().name(name.into()).spawn(move || {
        let guard = StopOnExit(shared);
        let _ = body(&guard.0);
    })
}

/// Stops the source when a thread ends, however it ends, and wakes the
/// other thread so it ends too.
struct StopOnExit(Arc<Shared>);

impl Drop for StopOnExit {
    fn drop(&mut self) {
        if thread::panicking() {
            self.0.lock().monitor.halt();
            self.0.wake_all();
        } else {
            stop(&self.0);
        }
    }
}

/// Stops the source, then wakes both threads, which then end; the wakes
/// are sent even if a hook told of the stop unwinds.
fn stop(shared: &Shared) {
    let _wake = WakeOnDrop(shared);
    shared.lock().monitor.stop();
}

/// The notify thread: one datagram at a time, a fresh subscription after
/// any loss on its socket, and a wake for the snapshot thread whenever the
/// monitor's request changed. Returns only when the source stops.
fn follow(shared: &Shared) -> Result<(), Stopped> {
    let mut buffer = vec![0_u8; BUFFER];
    loop {
        cleared(shared, &shared.notify_wake)?;
        let notify = {
            let state = shared.lock();
            if state.monitor.is_stopped() {
                return Err(Stopped);
            }
            Arc::clone(&state.notify)
        };
        let received = match turn(&notify, &shared.notify_wake, &mut buffer) {
            Turn::Idle => continue,
            Turn::Datagram(count) => Ok(count),
            Turn::Failed(error) => Err(Some(error)),
            Turn::Unexplained => Err(None),
        };
        let mut state = shared.lock();
        if state.monitor.is_stopped() {
            return Err(Stopped);
        }
        let before = (state.monitor.request(), state.monitor.barrier());
        let monitor = &mut state.monitor;
        match received {
            Ok(count) => monitor.notification(buffer.get(..count).unwrap_or_default())?,
            Err(Some(RouteError::Overflow(_))) => monitor.lost(Loss::Overflow)?,
            Err(Some(RouteError::Truncated)) => monitor.lost(Loss::Truncated)?,
            Err(Some(RouteError::BadSender | RouteError::NotKernel(_))) => {}
            Err(_) => monitor.lost(Loss::Failed)?,
        }
        if state.monitor.needs_subscription() {
            let Ok(fresh) = RouteSocket::notify() else {
                state.monitor.stop();
                return Err(Stopped);
            };
            state.notify = Arc::new(fresh);
            state.monitor.subscribed();
        }
        let changed = (state.monitor.request(), state.monitor.barrier()) != before;
        drop(state);
        if changed {
            shared.work.notify_all();
            // The snapshot thread may be waiting on a dump just abandoned.
            if shared.request_wake.signal().is_err() {
                shared.failed_wake();
                return Err(Stopped);
            }
        }
    }
}

/// The snapshot thread: each dump the monitor asks for, sent and received
/// without the lock, then the barrier on the notify socket. Returns only
/// when the source stops.
fn snapshots(shared: &Shared) -> Result<(), Stopped> {
    let mut buffer = vec![0_u8; BUFFER];
    let mut barrier_sent = None;
    loop {
        let (asked, request) = {
            let mut state = shared.lock();
            loop {
                if state.monitor.is_stopped() {
                    return Err(Stopped);
                }
                if let Some(asked) = state.monitor.request() {
                    break (asked, Arc::clone(&state.request));
                }
                if let Some(barrier) = state.monitor.barrier()
                    && barrier_sent != Some(barrier)
                {
                    barrier_sent = Some(barrier);
                    // Sent with the lock released (ADR 0005, sheet 23).
                    let notify = Arc::clone(&state.notify);
                    drop(state);
                    let went = sent(|| notify.send(&barrier.bytes()));
                    state = shared.lock();
                    if !went {
                        state.monitor.dump_failed(Loss::Failed)?;
                    }
                    continue;
                }
                state = shared.work.wait(state).unwrap_or_else(|poisoned| {
                    let mut state = poisoned.into_inner();
                    state.monitor.halt();
                    state
                });
            }
        };
        if !sent(|| request.send(&asked.bytes())) {
            shared.lock().monitor.dump_failed(Loss::Failed)?;
            renew(shared)?;
            continue;
        }
        let (next, barrier) = loop {
            cleared(shared, &shared.request_wake)?;
            {
                let state = shared.lock();
                if state.monitor.is_stopped() {
                    return Err(Stopped);
                }
                let next = state.monitor.request();
                if next != Some(asked) {
                    break (next, state.monitor.barrier().is_some());
                }
            }
            let received = turn(&request, &shared.request_wake, &mut buffer);
            let mut state = shared.lock();
            if state.monitor.is_stopped() {
                return Err(Stopped);
            }
            let monitor = &mut state.monitor;
            match received {
                Turn::Idle | Turn::Failed(RouteError::BadSender | RouteError::NotKernel(_)) => {}
                Turn::Datagram(count) => monitor.reply(buffer.get(..count).unwrap_or_default())?,
                Turn::Failed(RouteError::Overflow(_)) => monitor.dump_failed(Loss::Overflow)?,
                Turn::Failed(RouteError::Truncated) => monitor.dump_failed(Loss::Truncated)?,
                Turn::Failed(_) => monitor.dump_failed(Loss::Failed)?,
                Turn::Unexplained => {
                    // The socket stays in that state: its dump fails, and
                    // it is made again (`follows` is false with no request).
                    monitor.dump_failed(Loss::Failed)?;
                    break (None, false);
                }
            }
            if state.monitor.is_stopped() {
                return Err(Stopped);
            }
            let next = state.monitor.request();
            if next != Some(asked) {
                break (next, state.monitor.barrier().is_some());
            }
        };
        if !follows(asked, next, barrier) {
            renew(shared)?;
        }
    }
}

/// Whether `next` is what a dump that ended in order asks for after
/// `asked`: the address dump after the link dump, and after that no dump
/// but the barrier. No dump and no barrier means it was abandoned (a loss
/// that awaits a fresh subscription), so its socket is made again.
fn follows(asked: Request, next: Option<Request>, barrier: bool) -> bool {
    match next {
        None => !asked.is_links() && barrier,
        Some(next) => asked.is_links() && !next.is_links(),
    }
}

/// Makes the request socket again, so no abandoned dump answers on it.
fn renew(shared: &Shared) -> Result<(), Stopped> {
    match RouteSocket::request() {
        Ok(fresh) => {
            shared.lock().request = Arc::new(fresh);
            Ok(())
        }
        Err(_) => shared.lock().monitor.dump_failed(Loss::Failed),
    }
}

/// Clears `wake` at the start of a turn, before the state is read, so a
/// signal made after the read ends the turn's wait. A failed clear stops
/// the source.
fn cleared(shared: &Shared, wake: &Wake) -> Result<(), Stopped> {
    if wake.clear().is_err() {
        shared.failed_wake();
        return Err(Stopped);
    }
    Ok(())
}

/// What one turn of a thread hands over.
#[derive(Debug)]
enum Turn {
    /// Nothing: a timeout, an interruption, the wake alone, or nothing
    /// queued after the socket was reported readable only. The caller
    /// looks at the state again.
    Idle,
    /// A datagram of this length, in the buffer.
    Datagram(usize),
    /// The wait or the receive failed.
    Failed(RouteError),
    /// The wait reported an error or a hangup on the socket that the
    /// receive did not explain (nothing queued): a failed receive on that
    /// socket (ADR 0005, sheet 23), so the socket is not waited on again.
    Unexplained,
}

/// One wait on `socket` and `wake`, to [`ROUTE_WAIT_HORIZON`], then, if
/// the socket was reported, one receive. A failed wait is a failed
/// receive.
fn turn(socket: &RouteSocket, wake: &Wake, buffer: &mut [u8]) -> Turn {
    match socket.wait(wake, ROUTE_WAIT_HORIZON) {
        Ok(Waited::Ready(ready)) if ready.socket() => received(ready, socket.receive(buffer)),
        Ok(Waited::Ready(_) | Waited::TimedOut | Waited::Interrupted) => Turn::Idle,
        Err(error) => Turn::Failed(error),
    }
}

/// What a receive made after the wait reported `ready` hands over.
fn received(ready: Ready, receipt: Result<Receipt, RouteError>) -> Turn {
    match receipt {
        Ok(Receipt::Datagram(count)) => Turn::Datagram(count),
        Ok(Receipt::NothingQueued) if ready.error || ready.hangup => Turn::Unexplained,
        Ok(Receipt::NothingQueued | Receipt::Interrupted) => Turn::Idle,
        Err(error) => Turn::Failed(error),
    }
}

/// How many times a send is made: an interrupted one once more, as a
/// wake's transfer is (ADR 0005, sheet 23), and no more.
const SEND_ATTEMPTS: usize = 2;

/// Sends with `send`, made again once if a signal interrupted it:
/// whether every byte went. A second interruption is a failed send.
fn sent(mut send: impl FnMut() -> Result<Sent, RouteError>) -> bool {
    for _ in 0..SEND_ATTEMPTS {
        match send() {
            Ok(Sent::All) => return true,
            Ok(Sent::Interrupted) => {}
            Err(_) => return false,
        }
    }
    false
}
