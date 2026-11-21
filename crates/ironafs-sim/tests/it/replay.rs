// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Replay across processes (seams and scheduling specification, 5.6,
//! point 11; 10.5, point 5): the same seed in two separate processes
//! gives byte-identical, non-empty traces that render the fields a leak
//! would change; strict replay in a third gives the same bytes; planted
//! leaks turn the comparison red; and two pinned seeds choose different
//! identities.

use ironafs_sim::child::Exit;
use ironafs_sim::scenario::{Plant, find};
use ironafs_sim::supervisor::Ended;
use ironafs_sim::trace::{VERSION_LINE, chosen_identities, decisions, terminal};

use crate::support::{child, trace};

#[test]
fn sample_read_replays_byte_identically() {
    for seed in 1..=4 {
        let first = trace("sample", seed, &[], Exit::Passed);
        let second = trace("sample", seed, &[], Exit::Passed);
        assert_eq!(first, second, "seed {seed}");
        // Not vacuous: the trace is whole, and renders task ids, draw
        // counts, replies and the code under test's own events.
        assert!(first.starts_with(VERSION_LINE));
        assert_eq!(terminal(&first), Some("end ok"));
        assert!(decisions(&first).len() >= 16, "seed {seed}");
        assert!(
            decisions(&first)
                .iter()
                .any(|decision| decision.enabled.len() > 1)
        );
        for field in [
            " draws=",
            "decide [t0",
            " world reply c",
            " op fetched file:f ",
        ] {
            assert!(first.contains(field), "seed {seed} lacks {field:?}");
        }
        let seed_text = seed.to_string();
        let replayed = child(&["replay", "sample", &seed_text], first.as_bytes());
        assert_eq!(replayed.ended, Ended::Exited(Exit::Passed), "seed {seed}");
        assert_eq!(replayed.trace, first.as_bytes(), "seed {seed}");
    }
}

#[test]
fn planted_leaks_singly_and_combined_turn_replay_red() {
    for plants in [
        &["leak-pid"][..],
        &["leak-address"][..],
        &["leak-pid", "leak-address"][..],
    ] {
        let first = trace("sample", 1, plants, Exit::Passed);
        let second = trace("sample", 1, plants, Exit::Passed);
        // The plant fired: the code under test recorded its host value.
        assert!(first.contains(" op host "), "{plants:?} never fired");
        assert_ne!(first, second, "{plants:?} was not caught");
        // And the replay of one process's trace in another diverges.
        let replayed = child(
            &["replay", "sample", "1", "--plant", plants[0]],
            first.as_bytes(),
        );
        if plants.len() == 1 {
            assert_eq!(replayed.ended, Ended::Exited(Exit::Diverged), "{plants:?}");
        }
    }
}

#[test]
fn two_pinned_seeds_choose_different_identities() {
    let one = trace("sample", 1, &[], Exit::Passed);
    let two = trace("sample", 2, &[], Exit::Passed);
    assert_ne!(chosen_identities(&one), chosen_identities(&two));

    // A planted change to a printed field alone: every stream ignores the
    // seed, so the traces differ (the header prints the seed) while the
    // identities are the same, and the check fails.
    let one = trace("sample", 1, &["fixed-schedule"], Exit::Passed);
    let two = trace("sample", 2, &["fixed-schedule"], Exit::Passed);
    assert_ne!(one, two);
    assert_eq!(chosen_identities(&one), chosen_identities(&two));
}

#[test]
fn a_recorded_failing_seed_replays_in_another_process() {
    let sample = find("sample").unwrap();
    let seed = (0..64)
        .find(|seed| {
            sample
                .run(*seed, &[Plant::LostUpdate], None, None)
                .failure
                .is_some()
        })
        .unwrap();
    let original = trace("sample", seed, &["lost-update"], Exit::Failed);
    assert_eq!(
        terminal(&original),
        Some("end failed invariant:every-write-applied")
    );
    let seed_text = seed.to_string();
    let replayed = child(
        &["replay", "sample", &seed_text, "--plant", "lost-update"],
        original.as_bytes(),
    );
    assert_eq!(replayed.ended, Ended::Exited(Exit::Failed));
    assert_eq!(replayed.trace, original.as_bytes());
}

#[test]
fn a_replay_of_an_edited_log_diverges_in_another_process() {
    let original = trace("sample", 2, &[], Exit::Passed);
    let decision = decisions(&original)
        .into_iter()
        .find(|decision| decision.enabled.len() > 1)
        .unwrap();
    let mut changed = decision.clone();
    changed.pick = (changed.pick + 1) % changed.enabled.len();
    let edited = original.replacen(&decision.render(), &changed.render(), 1);
    let replayed = child(&["replay", "sample", "2"], edited.as_bytes());
    assert_eq!(replayed.ended, Ended::Exited(Exit::Diverged));
    let report = String::from_utf8(replayed.report).unwrap();
    assert!(
        report.starts_with(&format!("diverged:d{}\n", decision.number)),
        "{report}"
    );
}
