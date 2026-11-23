// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the reading and judging of the target gate.

use super::{ACCEPTED, Expected, GATE_ERROR, Run, cargo_refusal, judge, list_problems, listed};

const SCRIPT: &str = "//! Docs.\nconst TARGETS: [&str; 2] = [\n    \"x86_64-unknown-linux-gnu\",\n    \
                      \"aarch64-apple-darwin\",\n];\nfn main() {}\n";

#[test]
fn listed_reads_the_targets_constant() {
    assert_eq!(
        listed(SCRIPT).unwrap(),
        ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
    );
}

#[test]
fn listed_refuses_a_script_without_a_plain_list() {
    assert!(listed("fn main() {}\n").is_err());
    assert!(listed(&SCRIPT.replace("\"aarch64-apple-darwin\"", "OTHER")).is_err());
    assert!(listed(&SCRIPT.replace("aarch64-apple-darwin", "aarch64-apple-darwin\\n")).is_err());
    assert!(listed(&SCRIPT.replace("= [", "= &[")).is_err());
}

#[test]
fn list_problems_names_each_missing_extra_and_repeated_triple() {
    let reference = ["a-b-c", "d-e-f"];
    let owned = |items: &[&str]| {
        items
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        list_problems(&owned(&["d-e-f", "a-b-c"]), &reference),
        Vec::<String>::new()
    );
    let found = list_problems(&owned(&["a-b-c", "x-y-z", "a-b-c"]), &reference);
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(
        found
            .iter()
            .any(|problem| problem.starts_with("d-e-f: in ADR"))
    );
    assert!(
        found
            .iter()
            .any(|problem| problem.starts_with("x-y-z: in the build"))
    );
    assert!(found.iter().any(|problem| problem.contains("named twice")));
}

fn refused(line: &str) -> Run {
    let mut stdout = ACCEPTED.to_vec();
    stdout.extend_from_slice(line.as_bytes());
    Run {
        success: false,
        stdout,
    }
}

#[test]
fn judge_accepts_only_success_with_the_rerun_rule_alone() {
    let ok = Run {
        success: true,
        stdout: ACCEPTED.to_vec(),
    };
    assert_eq!(judge(&ok, &Expected::Accept), None);
    let more = Run {
        success: true,
        stdout: b"cargo::rerun-if-changed=build.rs\ncargo::rerun-if-changed=src\n".to_vec(),
    };
    assert!(judge(&more, &Expected::Accept).is_some());
    let none = Run {
        success: true,
        stdout: Vec::new(),
    };
    assert!(judge(&none, &Expected::Accept).is_some());
    let failed = Run {
        success: false,
        stdout: ACCEPTED.to_vec(),
    };
    assert!(judge(&failed, &Expected::Accept).is_some());
}

#[test]
fn judge_refusal_needs_one_escaped_line_naming_the_value_and_the_point() {
    let shown = "\"x\\ny\"";
    let expected = Expected::Refuse {
        shown,
        hidden: b"\ny",
    };
    let good = format!("cargo::error={GATE_ERROR} {shown}: ADR 0005, point 5.3\n");
    assert_eq!(judge(&refused(&good), &expected), None);
    // Printed raw, the line break splits the refusal.
    let raw = format!("cargo::error={GATE_ERROR} \"x\ny\": ADR 0005, point 5.3\n");
    assert!(judge(&refused(&raw), &expected).is_some());
    // Two refusal lines, or none.
    assert!(judge(&refused(&format!("{good}{good}")), &expected).is_some());
    assert!(judge(&refused(""), &expected).is_some());
    // Another message, or no point named.
    assert!(
        judge(
            &refused(&format!("cargo::error=no {shown} point 5.3\n")),
            &expected
        )
        .is_some()
    );
    assert!(
        judge(
            &refused(&format!("cargo::error={GATE_ERROR} {shown}\n")),
            &expected
        )
        .is_some()
    );
    // A refusal with a successful exit is an acceptance.
    let mut accepted = refused(&good);
    accepted.success = true;
    assert!(judge(&accepted, &expected).is_some());
}

#[test]
fn judge_refusal_refuses_output_holding_the_hidden_bytes() {
    let expected = Expected::Refuse {
        shown: "not UTF-8",
        hidden: b"\xff",
    };
    let good = format!("cargo::error={GATE_ERROR} not UTF-8: ADR 0005, point 5.3\n");
    assert_eq!(judge(&refused(&good), &expected), None);
    let mut leaked = refused(&good);
    leaked.stdout.insert(ACCEPTED.len() + 3, 0xff);
    assert!(judge(&leaked, &expected).is_some());
}

#[test]
fn cargo_refusal_needs_the_build_script_and_the_gate_s_message() {
    let target = "arm64e-apple-darwin";
    let refusal = format!(
        "   Compiling ironafs-os v0.0.0 (/w/crates/ironafs-os)\n\
         error: ironafs-os@0.0.0: {GATE_ERROR} \"{target}\": ADR 0005, point 5.3\n\
         error: failed to run custom build command for `ironafs-os v0.0.0 (/w/crates/ironafs-os)`\n\
         \nCaused by:\n  process didn't exit successfully: `build-script-build` (exit status: 1)\n\
         \x20 --- stdout\n  cargo::rerun-if-changed=build.rs\n\
         \x20 cargo::error={GATE_ERROR} \"{target}\": ADR 0005, point 5.3\n"
    );
    assert_eq!(cargo_refusal(&refusal, target), Ok(()));
    assert!(cargo_refusal(&refusal, "aarch64-oe-linux-gnu").is_err());
    let later = "error[E0463]: can't find crate for `core`\n";
    assert!(cargo_refusal(later, target).is_err());
    assert!(cargo_refusal(&format!("{refusal}{later}"), target).is_err());
    let guard = "error: ironafs-os builds only for the targets of ADR 0005, point 5.3\n";
    assert!(cargo_refusal(guard, target).is_err());
}
