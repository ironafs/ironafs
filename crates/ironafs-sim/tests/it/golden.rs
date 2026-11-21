// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Golden traces (seams and scheduling specification, 5.6, point 11): the
//! whole trace of each golden seed of each scenario is committed, per
//! target, and every run compares it exactly. The committed set for this
//! target must be exactly the scenarios' golden seeds.
//!
//! A difference is a defect, or a deliberate change: then regenerate each
//! fixture in the same commit as the change, with `VERSION_LINE` moved if
//! the format changed:
//!
//! ```text
//! cargo run -q -p ironafs-sim -- run <scenario> <seed> \
//!     > crates/ironafs-sim/tests/fixtures/golden/<target>/<scenario>-<seed>.trace
//! ```

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use ironafs_sim::child::Exit;
use ironafs_sim::scenario::SCENARIOS;
use ironafs_sim::trace::{FIXTURE_CAP, VERSION_LINE, terminal};

use crate::support::trace;

/// This target's fixture directory name: architecture, system and, where
/// there is one, the C environment.
fn target() -> String {
    let env = if cfg!(target_env = "gnu") {
        "-gnu"
    } else if cfg!(target_env = "musl") {
        "-musl"
    } else if cfg!(target_env = "msvc") {
        "-msvc"
    } else {
        ""
    };
    format!("{}-{}{env}", std::env::consts::ARCH, std::env::consts::OS)
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/golden")
        .join(target())
}

#[test]
fn golden_traces_match_the_committed_fixtures() {
    let dir = fixtures();
    let committed: BTreeSet<String> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("no golden traces for {}: {error}", target()))
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    let expected: BTreeSet<String> = SCENARIOS
        .iter()
        .flat_map(|scenario| {
            scenario
                .golden_seeds
                .iter()
                .map(|seed| format!("{}-{seed}.trace", scenario.name))
        })
        .collect();
    assert!(!expected.is_empty());
    assert_eq!(committed, expected, "the committed set for {}", target());
    for scenario in SCENARIOS {
        for seed in scenario.golden_seeds {
            let path = dir.join(format!("{}-{seed}.trace", scenario.name));
            let fixture = fs::read_to_string(&path).unwrap();
            assert!(
                fixture.len() <= FIXTURE_CAP,
                "{} is over the cap",
                path.display()
            );
            assert!(fixture.starts_with(VERSION_LINE), "{}", path.display());
            assert!(
                terminal(&fixture).is_some(),
                "{} is partial",
                path.display()
            );
            let run = trace(scenario.name, *seed, &[], Exit::Passed);
            assert!(run == fixture, "{} differs from its run", path.display());
        }
    }
}
