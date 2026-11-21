// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The supervisor: every run whose result is evidence runs in a child
//! process under it (seams and scheduling specification, 5.6, point 10).
//!
//! The child writes each trace line before acting on it, so the
//! supervisor always holds the committed prefix. It enforces an outer
//! real-time limit, the run's budget plus the watchdog interval plus a
//! reporting margin, so the child's own watchdog can always report first.
//! A child that aborts, crashes or passes the limit is a simulator error,
//! reported with what it wrote: never a pass.
//!
//! Two named departures from coding standards rule 7, the simulator's
//! only ones besides the watchdog: the child is a subprocess, and the
//! outer limit reads real time (in [`run`]'s wait for the child's output).

use std::io::{self, Read, Write};
use std::path::Path;
use std::process::Stdio;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use crate::child::Exit;

#[cfg(test)]
mod tests;

/// The subprocess builder for the supervisor alone.
#[expect(
    clippy::disallowed_types,
    reason = "rule 7: the simulator's supervisor runs each evidence run in a child process \
              (seams and scheduling specification, 5.6, point 10); the child's inputs are its \
              command line and standard input only"
)]
type Command = std::process::Command;

/// The watchdog interval plus the reporting margin, before the run's own
/// budget is added (5.6, points 9 and 10).
pub const LIMIT_MARGIN: Duration = Duration::from_secs(30 + 10);

/// How a child ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    /// It exited with a status the child defines.
    Exited(Exit),
    /// It exited with a status the child never gives: it aborted or
    /// crashed.
    Unknown(Option<i32>),
    /// It passed the outer limit and was killed.
    OverLimit,
}

/// What a supervised child left.
#[derive(Clone, Debug)]
pub struct Supervised {
    /// How it ended.
    pub ended: Ended,
    /// Everything it wrote to standard output: the trace, or its
    /// committed prefix.
    pub trace: Vec<u8>,
    /// Everything it wrote to standard error.
    pub report: Vec<u8>,
}

/// Runs `program` with `args`, writes `stdin` to it, and waits at most
/// `limit` for it to finish writing its trace.
///
/// # Errors
///
/// Returns the error if the child cannot be started or waited for.
pub fn run(
    program: &Path,
    args: &[String],
    stdin: &[u8],
    limit: Duration,
) -> io::Result<Supervised> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let input = child.stdin.take();
    let bytes = stdin.to_vec();
    let writer = thread::spawn(move || {
        if let Some(mut input) = input {
            // A child that exits without reading its input closes the
            // pipe; that is its own failure, seen in its status.
            let _ = input.write_all(&bytes);
        }
    });
    let trace = Arc::new(Mutex::new(Vec::new()));
    let (done, finished) = mpsc::channel();
    let out = child.stdout.take();
    let sink = Arc::clone(&trace);
    let reader = thread::spawn(move || {
        if let Some(mut out) = out {
            let mut chunk = [0_u8; 8192];
            while let Ok(read) = out.read(&mut chunk) {
                let Some(bytes) = chunk.get(..read).filter(|bytes| !bytes.is_empty()) else {
                    break;
                };
                sink.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .extend_from_slice(bytes);
            }
        }
        let _ = done.send(());
    });
    let err = child.stderr.take();
    let reporter = thread::spawn(move || {
        let mut report = Vec::new();
        if let Some(mut err) = err {
            let _ = err.read_to_end(&mut report);
        }
        report
    });
    // The outer limit: a real-time wait, a named departure (see above).
    let over = finished.recv_timeout(limit).is_err();
    if over {
        child.kill()?;
    }
    let status = child.wait()?;
    let _ = reader.join();
    let _ = writer.join();
    let report = reporter.join().unwrap_or_default();
    let trace = std::mem::take(&mut *trace.lock().unwrap_or_else(PoisonError::into_inner));
    let ended = if over {
        Ended::OverLimit
    } else {
        status
            .code()
            .and_then(Exit::from_code)
            .map_or(Ended::Unknown(status.code()), Ended::Exited)
    };
    Ok(Supervised {
        ended,
        trace,
        report,
    })
}
