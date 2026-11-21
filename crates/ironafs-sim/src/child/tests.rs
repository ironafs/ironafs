// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The child's command line and exit statuses, in one process.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use super::{Exit, main};

/// A sink the test can read after the child is done with it.
#[derive(Clone, Default)]
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn child(args: &[&str], stdin: &str) -> (Exit, String, String) {
    let out = Shared::default();
    let mut err = Vec::new();
    let exit = main(
        args.iter().map(|arg| (*arg).to_owned()),
        &mut stdin.as_bytes(),
        Box::new(out.clone()),
        &mut err,
    );
    let out = String::from_utf8(out.0.lock().unwrap().clone()).unwrap();
    (exit, out, String::from_utf8(err).unwrap())
}

#[test]
fn run_streams_the_trace_and_passes() {
    let (exit, out, err) = child(&["run", "sample", "1"], "");
    assert_eq!(exit, Exit::Passed, "{err}");
    assert!(out.starts_with("ironafs-sim trace 1\n"));
    assert!(out.ends_with(" sched end ok\n"));
}

#[test]
fn replay_of_its_own_trace_passes_and_of_another_diverges() {
    let (_, original, _) = child(&["run", "sample", "2"], "");
    let (exit, replayed, _) = child(&["replay", "sample", "2"], &original);
    assert_eq!(exit, Exit::Passed);
    assert_eq!(replayed, original);
    let (exit, _, err) = child(&["replay", "sample", "3"], &original);
    assert_eq!(exit, Exit::Diverged);
    assert!(
        err.contains("replay: ironafs-sim replay sample 3 < trace"),
        "{err}"
    );
}

#[test]
fn a_failing_run_prints_its_replay_command() {
    let failing = (0..64)
        .find(|seed| {
            child(
                &["run", "sample", &seed.to_string(), "--plant", "lost-update"],
                "",
            )
            .0 == Exit::Failed
        })
        .unwrap();
    let seed = failing.to_string();
    let (exit, out, err) = child(&["run", "sample", &seed, "--plant", "lost-update"], "");
    assert_eq!(exit, Exit::Failed);
    assert!(out.ends_with(" sched end failed invariant:every-write-applied\n"));
    assert_eq!(
        err,
        format!(
            "invariant:every-write-applied\n\
             replay: ironafs-sim replay sample {seed} --plant lost-update < trace\n"
        )
    );
}

#[test]
fn bad_command_lines_are_usage_errors() {
    for args in [
        &[][..],
        &["run", "sample"][..],
        &["run", "nosuch", "1"][..],
        &["run", "sample", "x"][..],
        &["walk", "sample", "1"][..],
        &["run", "sample", "1", "--plant"][..],
        &["run", "sample", "1", "--plant", "nosuch"][..],
    ] {
        assert_eq!(child(args, "").0, Exit::Usage, "{args:?}");
    }
}

#[test]
fn exit_codes_read_back() {
    for exit in [
        Exit::Passed,
        Exit::Failed,
        Exit::Diverged,
        Exit::SimulatorError,
        Exit::Usage,
    ] {
        assert_eq!(Exit::from_code(exit as i32), Some(exit));
    }
    assert_eq!(Exit::from_code(101), None);
}
