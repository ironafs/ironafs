// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The child process's work: run or replay one scenario, writing each
//! trace line to standard output before acting on it, and exit with a
//! status that says how the run ended (seams and scheduling
//! specification, 5.6, point 10).
//!
//! ```text
//! ironafs-sim run <scenario> <seed> [--plant <name>]...
//! ironafs-sim replay <scenario> <seed> [--plant <name>]...   (the original trace on stdin)
//! ```

use std::io::{Read, Write};

use crate::replay::Replay;
use crate::scenario::{Plant, find};
use crate::sched::Failure;
use crate::trace::check_vocabulary;

#[cfg(test)]
mod tests;

/// How the child ended, as its exit status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// The run passed.
    Passed = 0,
    /// The run failed: an invariant, the budget, a stuck run or a task's
    /// panic.
    Failed = 1,
    /// A replay diverged from its original.
    Diverged = 2,
    /// A simulator error: a host failure, a misuse, an unconfirmed lost
    /// baton or an error outside the vocabulary. Never a pass, and never a
    /// breach of a client invariant.
    SimulatorError = 3,
    /// The command line was wrong.
    Usage = 64,
}

impl Exit {
    /// The status for `code` as the supervisor reads it back.
    #[must_use]
    pub fn from_code(code: i32) -> Option<Self> {
        [
            Self::Passed,
            Self::Failed,
            Self::Diverged,
            Self::SimulatorError,
            Self::Usage,
        ]
        .into_iter()
        .find(|exit| *exit as i32 == code)
    }
}

const USAGE: &str = "usage: ironafs-sim run|replay <scenario> <seed> [--plant <name>]...\n";

/// Runs the child with its arguments (without the program name).
pub fn main(
    args: impl IntoIterator<Item = String>,
    stdin: &mut dyn Read,
    stdout: Box<dyn Write + Send>,
    stderr: &mut dyn Write,
) -> Exit {
    let mut args = args.into_iter();
    let (Some(mode), Some(name), Some(seed)) = (args.next(), args.next(), args.next()) else {
        let _ = stderr.write_all(USAGE.as_bytes());
        return Exit::Usage;
    };
    let (Some(scenario), Ok(seed)) = (find(&name), seed.parse::<u64>()) else {
        let _ = stderr.write_all(USAGE.as_bytes());
        return Exit::Usage;
    };
    let mut plants = Vec::new();
    while let Some(flag) = args.next() {
        let plant = args.next().as_deref().and_then(Plant::parse);
        let (true, Some(plant)) = (flag == "--plant", plant) else {
            let _ = stderr.write_all(USAGE.as_bytes());
            return Exit::Usage;
        };
        plants.push(plant);
    }
    let replay = match mode.as_str() {
        "run" => None,
        "replay" => {
            let mut original = String::new();
            if stdin.read_to_string(&mut original).is_err() {
                let _ = writeln!(stderr, "the original trace on standard input is not text");
                return Exit::Usage;
            }
            Some(Replay::from_trace(&original))
        }
        _ => {
            let _ = stderr.write_all(USAGE.as_bytes());
            return Exit::Usage;
        }
    };
    let outcome = scenario.run(seed, &plants, Some(stdout), replay);
    if let Err(error) = check_vocabulary(&outcome.trace) {
        let _ = writeln!(stderr, "simulator error: {error}");
        return Exit::SimulatorError;
    }
    let Some(failure) = outcome.failure else {
        return Exit::Passed;
    };
    let mut command = format!("ironafs-sim replay {} {seed}", scenario.name);
    for plant in &plants {
        command.push_str(" --plant ");
        command.push_str(plant.name());
    }
    let _ = writeln!(stderr, "{failure}\nreplay: {command} < trace");
    match failure {
        Failure::Diverged { .. } => Exit::Diverged,
        Failure::Host(_) | Failure::Misuse(_) | Failure::LostBaton { .. } => Exit::SimulatorError,
        Failure::Invariant(_)
        | Failure::Stuck
        | Failure::Budget
        | Failure::TooManyTasks
        | Failure::TaskPanicked { .. } => Exit::Failed,
    }
}
