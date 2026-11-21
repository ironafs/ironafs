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
//! monitor can reach the other thread's socket. **No receive blocks for
//! ever:** whoever needs a blocked thread to look again (a stop, a loss
//! that abandons a dump, a resume) sends that thread's socket a wake
//! ([`ironafs_netgen::wake`]), which the kernel answers on it. A socket
//! is replaced only under the lock, and a stop is decided under the lock
//! before the wakes are sent to the sockets then in place, so no thread
//! can start a receive that the stop's wake misses. A wake that cannot be
//! sent, or whose acknowledgement the kernel drops, is the residual the
//! specification names (5.7, point 7; section 13, Q6).
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

use ironafs_netgen::{Consumers, Loss, Monitor, Reader, Request, StopRequest, Stopped, wake};
use ironafs_os::{RouteError, RouteSocket};

/// The receive buffer, in bytes (docs/specs/network-generation.md, 5.5,
/// point 1).
const BUFFER: usize = 65_536;

/// Why the monitor did not start.
#[derive(Debug)]
pub enum MonitorError {
    /// A route socket could not be made.
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
/// for work, and the request with which a hook that drops the handle stops
/// the source.
#[derive(Debug)]
struct Shared {
    state: Mutex<State>,
    work: Condvar,
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

    /// Wakes both threads: the snapshot thread's wait, and each receive on
    /// the sockets now in place. Called after the change it signals, with
    /// the lock released.
    fn wake_all(&self) {
        let (notify, request) = {
            let state = self.lock();
            (Arc::clone(&state.notify), Arc::clone(&state.request))
        };
        self.work.notify_all();
        // A failed wake leaves that thread to its next datagram; the
        // source's state is already decided.
        let _ = notify.send(&wake());
        let _ = request.send(&wake());
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
    let sockets = RouteSocket::notify().and_then(|notify| Ok((notify, RouteSocket::request()?)));
    let (notify, request) = match sockets {
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
        let notify = {
            let state = shared.lock();
            if state.monitor.is_stopped() {
                return Err(Stopped);
            }
            Arc::clone(&state.notify)
        };
        let received = notify.receive(&mut buffer);
        let mut state = shared.lock();
        if state.monitor.is_stopped() {
            return Err(Stopped);
        }
        let before = (state.monitor.request(), state.monitor.barrier());
        let monitor = &mut state.monitor;
        match received {
            Ok(count) => monitor.notification(buffer.get(..count).unwrap_or_default())?,
            Err(RouteError::Overflow(_)) => monitor.lost(Loss::Overflow)?,
            Err(RouteError::Truncated) => monitor.lost(Loss::Truncated)?,
            Err(RouteError::BadSender | RouteError::NotKernel(_)) => {}
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
        let request = Arc::clone(&state.request);
        drop(state);
        if changed {
            shared.work.notify_all();
            // The snapshot thread may be blocked on a dump just abandoned.
            let _ = request.send(&wake());
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
                    if state.notify.send(&barrier.bytes()).is_err() {
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
        if request.send(&asked.bytes()).is_err() {
            shared.lock().monitor.dump_failed(Loss::Failed)?;
            renew(shared)?;
            continue;
        }
        let (next, barrier) = loop {
            let received = request.receive(&mut buffer);
            let mut state = shared.lock();
            if state.monitor.is_stopped() {
                return Err(Stopped);
            }
            let monitor = &mut state.monitor;
            match received {
                Ok(count) => monitor.reply(buffer.get(..count).unwrap_or_default())?,
                Err(RouteError::BadSender | RouteError::NotKernel(_)) => {}
                Err(RouteError::Overflow(_)) => monitor.dump_failed(Loss::Overflow)?,
                Err(RouteError::Truncated) => monitor.dump_failed(Loss::Truncated)?,
                Err(_) => monitor.dump_failed(Loss::Failed)?,
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
