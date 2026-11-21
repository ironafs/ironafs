// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Linux network-change monitor's threads
//! (docs/specs/network-generation.md, 5.7 and section 7): they own the
//! two route sockets of `ironafs-os` and feed `ironafs-netgen`'s
//! [`Monitor`], which decides every move.
//!
//! - The **notify thread** owns the notify socket. It is made, and joins
//!   its groups, before the monitor may ask for a snapshot; after a loss
//!   on it (an overflow, a truncated or malformed datagram, a failed
//!   receive) it is made again before the next snapshot, so no record
//!   queued before the loss is applied to fresh tables. A datagram from a
//!   sender other than the kernel is ignored: it moves nothing.
//! - The **snapshot thread** owns the request socket. It waits for the
//!   monitor to ask for a dump, then sends and receives without holding
//!   the lock, which it takes only to hand each datagram over. A dump that
//!   does not end in order (links, then addresses, then nothing) may still
//!   run in the kernel on that socket, so the socket is made again.
//!
//! The lock is held only while the monitor applies a datagram or a loss,
//! hooks included, so neither a pending dump nor a quiet notify socket
//! keeps [`MonitorHandle::resumed`] waiting. While a dump is outstanding
//! the generation is unavailable, so a kernel that never answered would
//! leave it unavailable, never stale.
//!
//! When either thread ends, or the handle is dropped, the source stops.
//! A thread unwinding from a hook's panic, or finding the lock poisoned
//! by one, halts it instead: the hooks are not called again.

use std::error::Error;
use std::fmt;
use std::io;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
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

/// The monitor, and the signal that wakes the snapshot thread.
#[derive(Debug)]
struct Shared {
    monitor: Mutex<Monitor>,
    wake: Condvar,
}

impl Shared {
    /// The monitor. If a hook panicked while it was held, the source is
    /// halted first: its state can no longer be trusted, and its hooks are
    /// not called again.
    fn lock(&self) -> MutexGuard<'_, Monitor> {
        self.monitor.lock().unwrap_or_else(|poisoned| {
            let mut monitor = poisoned.into_inner();
            monitor.halt();
            monitor
        })
    }
}

/// The running monitor. Dropping it stops the source.
#[derive(Debug)]
pub struct MonitorHandle {
    shared: Arc<Shared>,
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
        let result = self.shared.lock().lost(Loss::Resumed);
        self.shared.wake.notify_all();
        result
    }
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        stop(&self.shared);
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
    let shared = Arc::new(Shared {
        monitor: Mutex::new(monitor),
        wake: Condvar::new(),
    });
    let handle = MonitorHandle {
        shared: Arc::clone(&shared),
    };
    // On an error below, dropping the handle stops the source.
    start("network-notify", &shared, move |shared| {
        follow(shared, notify)
    })
    .map_err(MonitorError::Thread)?;
    start("network-snapshot", &shared, move |shared| {
        snapshots(shared, request)
    })
    .map_err(MonitorError::Thread)?;
    Ok((reader, handle))
}

/// Starts one of the monitor's threads, which stops the source when it
/// ends.
fn start(
    name: &str,
    shared: &Arc<Shared>,
    body: impl FnOnce(&Shared) -> Result<(), Stopped> + Send + 'static,
) -> io::Result<()> {
    let shared = Arc::clone(shared);
    thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let guard = StopOnExit(shared);
            let _ = body(&guard.0);
        })
        .map(drop)
}

/// Stops the source when a thread ends, however it ends.
struct StopOnExit(Arc<Shared>);

impl Drop for StopOnExit {
    fn drop(&mut self) {
        if thread::panicking() {
            self.0.lock().halt();
            self.0.wake.notify_all();
        } else {
            stop(&self.0);
        }
    }
}

/// Stops the source and wakes the snapshot thread, which then ends.
fn stop(shared: &Shared) {
    shared.lock().stop();
    shared.wake.notify_all();
}

/// The notify thread: one notification at a time, and a fresh
/// subscription after any loss on its socket. Returns only when the source
/// stops.
fn follow(shared: &Shared, mut notify: RouteSocket) -> Result<(), Stopped> {
    let mut buffer = vec![0_u8; BUFFER];
    loop {
        let received = notify.receive(&mut buffer);
        let mut monitor = shared.lock();
        match received {
            Ok(count) => monitor.notification(buffer.get(..count).unwrap_or_default())?,
            Err(RouteError::Overflow(_)) => monitor.lost(Loss::Overflow)?,
            Err(RouteError::Truncated) => monitor.lost(Loss::Truncated)?,
            Err(RouteError::BadSender | RouteError::NotKernel(_)) => {}
            Err(_) => monitor.lost(Loss::Failed)?,
        }
        if monitor.needs_subscription() {
            let Ok(fresh) = RouteSocket::notify() else {
                monitor.stop();
                return Err(Stopped);
            };
            notify = fresh;
            monitor.subscribed();
        }
        drop(monitor);
        shared.wake.notify_all();
    }
}

/// The snapshot thread: each dump the monitor asks for, sent and received
/// without the lock. Returns only when the source stops.
fn snapshots(shared: &Shared, mut request: RouteSocket) -> Result<(), Stopped> {
    let mut buffer = vec![0_u8; BUFFER];
    loop {
        let asked = wait_for_request(shared)?;
        if request.send(&asked.bytes()).is_err() {
            shared.lock().dump_failed(Loss::Failed)?;
            renew(shared, &mut request)?;
            continue;
        }
        let next = loop {
            let received = request.receive(&mut buffer);
            let mut monitor = shared.lock();
            match received {
                Ok(count) => monitor.reply(buffer.get(..count).unwrap_or_default())?,
                Err(RouteError::BadSender | RouteError::NotKernel(_)) => {}
                Err(RouteError::Overflow(_)) => monitor.dump_failed(Loss::Overflow)?,
                Err(RouteError::Truncated) => monitor.dump_failed(Loss::Truncated)?,
                Err(_) => monitor.dump_failed(Loss::Failed)?,
            }
            let next = monitor.request();
            if next != Some(asked) {
                break next;
            }
        };
        if !follows(asked, next) {
            renew(shared, &mut request)?;
        }
    }
}

/// Waits until the monitor asks for a dump, or the source stops.
fn wait_for_request(shared: &Shared) -> Result<Request, Stopped> {
    let mut monitor = shared.lock();
    loop {
        if monitor.is_stopped() {
            return Err(Stopped);
        }
        if let Some(asked) = monitor.request() {
            return Ok(asked);
        }
        monitor = shared.wake.wait(monitor).unwrap_or_else(|poisoned| {
            let mut monitor = poisoned.into_inner();
            monitor.halt();
            monitor
        });
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

/// Makes the request socket again, so no abandoned dump answers on it.
fn renew(shared: &Shared, request: &mut RouteSocket) -> Result<(), Stopped> {
    match RouteSocket::request() {
        Ok(fresh) => {
            *request = fresh;
            Ok(())
        }
        Err(_) => shared.lock().dump_failed(Loss::Failed),
    }
}
