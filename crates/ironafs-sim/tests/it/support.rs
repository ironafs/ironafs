// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Running the simulator's child process under the supervisor.

use std::path::Path;
use std::time::Duration;

use ironafs_sim::child::Exit;
use ironafs_sim::supervisor::{self, Ended, Supervised};

/// The outer limit for one child: the margin plus ten seconds of run.
const LIMIT: Duration = Duration::from_secs(10).saturating_add(supervisor::LIMIT_MARGIN);

/// Runs the child with `args` and `stdin`.
pub(crate) fn child(args: &[&str], stdin: &[u8]) -> Supervised {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    supervisor::run(
        Path::new(env!("CARGO_BIN_EXE_ironafs-sim")),
        &args,
        stdin,
        LIMIT,
    )
    .unwrap()
}

/// Runs `scenario` at `seed` with `plants` and returns its trace, after
/// checking the child ended as `expected`.
#[track_caller]
pub(crate) fn trace(scenario: &str, seed: u64, plants: &[&str], expected: Exit) -> String {
    let seed = seed.to_string();
    let mut args = vec!["run", scenario, seed.as_str()];
    for plant in plants {
        args.push("--plant");
        args.push(plant);
    }
    let run = child(&args, b"");
    assert_eq!(
        run.ended,
        Ended::Exited(expected),
        "{}",
        String::from_utf8_lossy(&run.report)
    );
    String::from_utf8(run.trace).unwrap()
}
