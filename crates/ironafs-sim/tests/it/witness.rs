// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The interleaving witness over seed ranges (seams and scheduling
//! specification, 5.6, point 6; 10.5, point 6).

use ironafs_sim::child::Exit;
use ironafs_sim::scenario::find;
use ironafs_sim::trace::{Entry, chosen_identities, ops};
use ironafs_sim::witness::{Order, Race, Report};

use crate::support::trace;

/// The race fixture's window and meeting events, with the proposed floor
/// of 8 seeds per order.
const RACE: Race = Race {
    open: "open",
    close: "close",
    first: "a",
    second: "b",
    floor: 8,
};

/// The pull-request seed range of a race scenario.
const SEEDS: std::ops::Range<u64> = 0..256;

fn sweep(plants: &[&str]) -> Report {
    let traces: Vec<String> = SEEDS
        .map(|seed| trace("race", seed, plants, Exit::Passed))
        .collect();
    RACE.sweep(traces.iter().map(String::as_str))
}

#[test]
fn race_both_orders_witnessed_over_the_seed_range() {
    let report = sweep(&[]);
    assert_eq!(
        report.check(&RACE, &[Order::FirstThenSecond, Order::SecondThenFirst]),
        Ok(()),
        "{report:?}"
    );
    assert!(report.not_exercised > 0, "{report:?}");
}

#[test]
fn race_without_interleaving_witness_fails() {
    // The scheduler planted to grant each task its whole critical window.
    let report = sweep(&["segregate"]);
    let failed = report
        .check(&RACE, &[Order::FirstThenSecond, Order::SecondThenFirst])
        .unwrap_err();
    assert_eq!(failed.witnessed, 0, "{report:?}");
    assert_eq!(report.not_exercised, 256, "{report:?}");
}

#[test]
fn meeting_events_on_different_objects_are_reported_not_exercised() {
    let report = sweep(&["other-object"]);
    assert_eq!(
        report,
        Report {
            first_then_second: 0,
            second_then_first: 0,
            not_exercised: 256,
        }
    );
}

/// Whether the trace's task steps are fully segregated: each task's
/// operation events form one unbroken run.
fn segregated(trace: &str) -> bool {
    let mut finished = std::collections::BTreeSet::new();
    let mut current = None;
    for op in ops(trace) {
        if current != Some(op.task) {
            if let Some(previous) = current {
                finished.insert(previous);
            }
            if finished.contains(&op.task) {
                return false;
            }
            current = Some(op.task);
        }
    }
    true
}

#[test]
fn segregated_share_is_measured() {
    // Three tasks of three steps, a uniform draw among enabled entries.
    // An independent estimate: after the first pick, staying on one task
    // twice is 1/3 * 1/3, then on the next twice 1/2 * 1/2, so about 1/36
    // (2.8%) of schedules are fully segregated, when every task is enabled
    // at every decision. Measured: 54 of 2048 (2.6%), pinned, so a change
    // moves it deliberately. (An earlier simulator reported about 7%.)
    let scenario = find("segregation").unwrap();
    let mut count = 0;
    for seed in 0..2048 {
        let outcome = scenario.run(seed, &[], None, None);
        assert_eq!(outcome.failure, None, "seed {seed}");
        assert!(
            chosen_identities(&outcome.trace)
                .iter()
                .all(|entry| matches!(entry, Entry::Task(_)))
        );
        if segregated(&outcome.trace) {
            count += 1;
        }
    }
    assert_eq!(count, MEASURED, "segregated schedules in 2048 seeds");
}

const MEASURED: usize = 54;
