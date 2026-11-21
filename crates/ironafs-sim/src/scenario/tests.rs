// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The scenarios in one process: the sample runs clean, its planted lost
//! update fails on some seeds and replays from its seed, and strict
//! replay catches each planted divergence at its decision.

use super::{Plant, SCENARIOS, find};
use crate::replay::Replay;
use crate::sched::Failure;
use crate::trace::{Decision, decisions, terminal};

fn sample() -> &'static super::Scenario {
    find("sample").unwrap()
}

#[test]
fn every_plant_name_parses_back_and_scenarios_are_unique() {
    for plant in Plant::ALL {
        assert_eq!(Plant::parse(plant.name()), Some(plant));
    }
    assert_eq!(Plant::parse("none"), None);
    let mut names: Vec<&str> = SCENARIOS.iter().map(|scenario| scenario.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), SCENARIOS.len());
}

#[test]
fn sample_runs_clean_and_replies_arrive() {
    for seed in 0..32 {
        let outcome = sample().run(seed, &[], None, None);
        assert_eq!(outcome.failure, None, "seed {seed}\n{}", outcome.trace);
        assert!(outcome.trace.contains(" world reply c"), "seed {seed}");
        assert!(outcome.trace.contains(" op fetched file:f "), "seed {seed}");
        assert_eq!(terminal(&outcome.trace), Some("end ok"));
    }
}

/// The seeds below 64 on which the planted lost update fails.
fn lost_update_failures() -> Vec<u64> {
    (0..64)
        .filter(|seed| {
            sample()
                .run(*seed, &[Plant::LostUpdate], None, None)
                .failure
                == Some(Failure::Invariant("every-write-applied"))
        })
        .collect()
}

#[test]
fn planted_failing_schedule_fails_on_some_seeds_only() {
    let failing = lost_update_failures();
    assert!(!failing.is_empty(), "the planted lost update never failed");
    assert!(
        failing.len() < 64,
        "the planted lost update failed on every seed"
    );
}

#[test]
fn planted_failing_schedule_reproduces_from_its_seed() {
    let seed = lost_update_failures()[0];
    let first = sample().run(seed, &[Plant::LostUpdate], None, None);
    let again = sample().run(seed, &[Plant::LostUpdate], None, None);
    assert_eq!(again.trace, first.trace);
    assert_eq!(again.failure, first.failure);
    let replayed = sample().run(
        seed,
        &[Plant::LostUpdate],
        None,
        Some(Replay::from_trace(&first.trace)),
    );
    assert_eq!(replayed.trace, first.trace);
    assert_eq!(
        replayed.failure,
        Some(Failure::Invariant("every-write-applied"))
    );
}

/// Replays `edited` and returns the decision at which it diverged.
fn diverges_at(seed: u64, edited: &str) -> Option<u64> {
    match sample()
        .run(seed, &[], None, Some(Replay::from_trace(edited)))
        .failure
    {
        Some(Failure::Diverged { decision }) => Some(decision),
        _ => None,
    }
}

/// `trace` with line `index` replaced by `line`.
fn with_line(trace: &str, index: usize, line: Option<&str>) -> String {
    let mut lines: Vec<&str> = trace.lines().collect();
    match line {
        Some(line) => lines[index] = line,
        None => {
            lines.remove(index);
        }
    }
    join(&lines)
}

fn join(lines: &[&str]) -> String {
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text
}

fn line_index(trace: &str, decision: &Decision) -> usize {
    let rendered = decision.render();
    trace.lines().position(|line| line == rendered).unwrap()
}

#[test]
fn strict_replay_of_a_clean_run_gives_the_same_bytes() {
    let seed = 3;
    let original = sample().run(seed, &[], None, None);
    let replayed = sample().run(seed, &[], None, Some(Replay::from_trace(&original.trace)));
    assert_eq!(replayed.failure, None);
    assert_eq!(replayed.trace, original.trace);
}

#[test]
fn planted_divergence_is_caught_at_its_decision() {
    let seed = 3;
    let trace = sample().run(seed, &[], None, None).trace;
    let all = decisions(&trace);
    let contested = all
        .iter()
        .find(|decision| decision.enabled.len() > 1)
        .unwrap();

    // A changed chosen entry.
    let mut changed = contested.clone();
    changed.pick = (changed.pick + 1) % changed.enabled.len();
    let edited = with_line(
        &trace,
        line_index(&trace, contested),
        Some(&changed.render()),
    );
    assert_eq!(diverges_at(seed, &edited), Some(contested.number));

    // A changed unchosen entry, same length and same choice.
    let mut changed = contested.clone();
    let other = (changed.pick + 1) % changed.enabled.len();
    changed.enabled[other] = crate::trace::Entry::Task(crate::trace::TaskId(99));
    let edited = with_line(
        &trace,
        line_index(&trace, contested),
        Some(&changed.render()),
    );
    assert_eq!(diverges_at(seed, &edited), Some(contested.number));

    // A changed reply byte, at the first decision that emits reply bytes.
    let (index, reply) = trace
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(" world reply "))
        .unwrap();
    let decision: u64 = reply
        .strip_prefix('d')
        .and_then(|rest| rest.split(' ').next())
        .unwrap()
        .parse()
        .unwrap();
    let flipped = match reply.strip_suffix('0') {
        Some(head) => format!("{head}1"),
        None => format!("{}0", reply.get(..reply.len() - 1).unwrap()),
    };
    let edited = with_line(&trace, index, Some(&flipped));
    assert_eq!(diverges_at(seed, &edited), Some(decision));

    // A log one record short.
    let last = all.last().unwrap();
    let edited = with_line(&trace, line_index(&trace, last), None);
    assert_eq!(diverges_at(seed, &edited), Some(last.number));

    // A log with one record extra.
    let index = line_index(&trace, contested);
    let mut lines: Vec<&str> = trace.lines().collect();
    let rendered = contested.render();
    lines.insert(index + 1, &rendered);
    let edited = join(&lines);
    assert_eq!(diverges_at(seed, &edited), Some(contested.number));

    // A log with a record extra past the end.
    let edited = format!("{trace}d9999 sched decide [t0] pick=0 drawn draws=1 at=0\n");
    assert_eq!(diverges_at(seed, &edited), Some(all.len() as u64));
}

#[test]
fn a_replay_that_differs_only_in_its_terminal_line_diverges() {
    let seed = lost_update_failures()[0];
    let original = sample().run(seed, &[Plant::LostUpdate], None, None).trace;
    let edited = original.replace(
        "end failed invariant:every-write-applied",
        "end failed invariant:reply-matches-its-version",
    );
    assert_ne!(edited, original);
    let replayed = sample().run(
        seed,
        &[Plant::LostUpdate],
        None,
        Some(Replay::from_trace(&edited)),
    );
    let last = decisions(&original).last().unwrap().number;
    assert_eq!(
        replayed.failure,
        Some(Failure::Diverged { decision: last + 1 })
    );
}
