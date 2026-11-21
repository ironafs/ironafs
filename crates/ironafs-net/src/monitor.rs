// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Linux network-change monitor's thread
//! (docs/specs/network-generation.md, 5.7 and section 7): it owns the two
//! route sockets of `ironafs-os` and feeds `ironafs-netgen`'s
//! [`Monitor`], which decides every move.
//!
//! The notify socket is made, and joins its groups, before the request
//! socket sends anything, so no change falls between the subscription and
//! the snapshot. A datagram from a sender other than the kernel is
//! ignored: it moves nothing. A failed notify socket is made again, after
//! the move its loss causes. A dump that does not end in the order the
//! monitor asks for (links, then addresses, then nothing) leaves the
//! kernel's dump running on that socket, so the request socket is made
//! again before the next one.
//!
//! When the thread ends, for any reason, a panicking hook included, the
//! source is stopped, so readers never keep a generation nobody moves.

use std::error::Error;
use std::fmt;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use ironafs_netgen::{Consumers, Loss, Monitor, Reader, Request, Stopped};
use ironafs_os::{RouteError, RouteSocket};

/// The receive buffer, in bytes (docs/specs/network-generation.md, 5.5,
/// point 1).
const BUFFER: usize = 65_536;

/// Why the monitor did not start.
#[derive(Debug)]
pub enum MonitorError {
    /// A route socket could not be made.
    Socket(RouteError),
    /// The thread could not be started.
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

/// The monitor with its request socket and buffer: a snapshot runs whole
/// under one lock, from the thread or from [`MonitorHandle::resumed`].
#[derive(Debug)]
struct State {
    monitor: Monitor,
    request: RouteSocket,
    buffer: Vec<u8>,
}

/// The running monitor. Dropping it leaves the thread running.
#[derive(Debug)]
pub struct MonitorHandle {
    state: Arc<Mutex<State>>,
}

impl MonitorHandle {
    /// The machine resumed from suspend: the generation moves at once,
    /// and a fresh snapshot is taken before this returns. Whoever detects
    /// a resume (the clock pair) calls it.
    ///
    /// # Errors
    ///
    /// [`Stopped`] once the source has stopped.
    pub fn resumed(&self) -> Result<(), Stopped> {
        let mut state = lock(&self.state);
        state.monitor.lost(Loss::Resumed)?;
        state.snapshot()
    }
}

/// Starts the monitor: subscribes, then starts the thread that takes the
/// first snapshot and follows every change. `make` builds the consumers'
/// hooks, which run on that thread or in [`MonitorHandle::resumed`]'s
/// caller. The reader is unavailable until the first snapshot completes.
///
/// # Errors
///
/// [`MonitorError`] if a socket cannot be made or the thread cannot start;
/// the consumers are then told the source stopped.
pub fn spawn(
    make: impl FnOnce(&Reader) -> Consumers,
) -> Result<(Reader, MonitorHandle), MonitorError> {
    let (mut monitor, reader) = Monitor::new(make);
    // The notify socket joins its groups before the request socket is
    // made, and so before anything is sent on it.
    let sockets = RouteSocket::notify().and_then(|notify| Ok((notify, RouteSocket::request()?)));
    let (notify, request) = match sockets {
        Ok(sockets) => sockets,
        Err(error) => {
            monitor.stop();
            return Err(MonitorError::Socket(error));
        }
    };
    let state = Arc::new(Mutex::new(State {
        monitor,
        request,
        buffer: vec![0_u8; BUFFER],
    }));
    let shared = Arc::clone(&state);
    let started = thread::Builder::new()
        .name("network-monitor".into())
        .spawn(move || {
            let guard = StopOnExit(shared);
            let _ = follow(&guard.0, notify);
        });
    if let Err(error) = started {
        lock(&state).monitor.stop();
        return Err(MonitorError::Thread(error));
    }
    Ok((reader, MonitorHandle { state }))
}

/// Stops the source when the thread ends, however it ends.
struct StopOnExit(Arc<Mutex<State>>);

impl Drop for StopOnExit {
    fn drop(&mut self) {
        lock(&self.0).monitor.stop();
    }
}

/// The state, even if a hook panicked while it was held: the thread then
/// ends, and the state is only ever stopped.
fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The thread's loop: the snapshot the monitor asks for, if any, then one
/// notification. Returns only when the source stops.
fn follow(state: &Mutex<State>, mut notify: RouteSocket) -> Result<(), Stopped> {
    let mut buffer = vec![0_u8; BUFFER];
    loop {
        lock(state).snapshot()?;
        let received = notify.receive(&mut buffer);
        let mut held = lock(state);
        let monitor = &mut held.monitor;
        match received {
            Ok(count) => monitor.notification(buffer.get(..count).unwrap_or_default())?,
            Err(RouteError::Overflow(_)) => monitor.lost(Loss::Overflow)?,
            Err(RouteError::Truncated) => monitor.lost(Loss::Truncated)?,
            Err(RouteError::BadSender | RouteError::NotKernel(_)) => {}
            Err(_) => {
                monitor.lost(Loss::Failed)?;
                let Ok(fresh) = RouteSocket::notify() else {
                    monitor.stop();
                    return Err(Stopped);
                };
                notify = fresh;
            }
        }
    }
}

impl State {
    /// Runs every dump the monitor asks for, until it asks for none.
    fn snapshot(&mut self) -> Result<(), Stopped> {
        while let Some(asked) = self.monitor.request() {
            if self.request.send(&asked.bytes()).is_err() {
                self.monitor.dump_failed(Loss::Failed)?;
                self.renew()?;
                continue;
            }
            let next = loop {
                match self.request.receive(&mut self.buffer) {
                    Ok(count) => self
                        .monitor
                        .reply(self.buffer.get(..count).unwrap_or_default())?,
                    Err(RouteError::BadSender | RouteError::NotKernel(_)) => {}
                    Err(RouteError::Overflow(_)) => self.monitor.dump_failed(Loss::Overflow)?,
                    Err(RouteError::Truncated) => self.monitor.dump_failed(Loss::Truncated)?,
                    Err(_) => self.monitor.dump_failed(Loss::Failed)?,
                }
                let next = self.monitor.request();
                if next != Some(asked) {
                    break next;
                }
            };
            if !follows(asked, next) {
                self.renew()?;
            }
        }
        Ok(())
    }

    /// Makes the request socket again, so no abandoned dump answers on it.
    fn renew(&mut self) -> Result<(), Stopped> {
        match RouteSocket::request() {
            Ok(fresh) => {
                self.request = fresh;
                Ok(())
            }
            Err(_) => self.monitor.dump_failed(Loss::Failed),
        }
    }
}

/// Whether `next` is what a dump that ended in order asks for after
/// `asked`: the address dump after the link dump, nothing after that.
fn follows(asked: Request, next: Option<Request>) -> bool {
    match next {
        None => !asked.is_links(),
        Some(next) => asked.is_links() && !next.is_links(),
    }
}
