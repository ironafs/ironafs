// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool's output written as text, in the shape cargo-mutants 27.1.0
//! writes it (its `outcome.rs`, `mutant.rs` and `process.rs` at that
//! tag), for the unit tests and `mutants --self-test`.

/// The file every planted mutant is in.
pub(super) const FILE: &str = "crates/ironafs-time/src/instant.rs";

/// A test phase's command line as the tool records it with the options
/// `xtask` passes.
pub(super) const TEST_ARGV: &str = "[\"cargo\",\"nextest\",\"run\",\"--verbose\",\
     \"--package=ironafs-time\",\"--profile=mutants\",\"--retries=0\",\"--no-tests=fail\"]";

/// A baseline whose build and tests passed.
pub(super) const BASELINE_OK: &str = "{\"scenario\":\"Baseline\",\"summary\":\"Success\",\
     \"log_path\":\"log/baseline.log\",\"diff_path\":null,\"phase_results\":[\
     {\"phase\":\"Build\",\"duration\":1.5,\"process_status\":\"Success\",\"argv\":[]},\
     {\"phase\":\"Test\",\"duration\":0.5,\"process_status\":\"Success\",\"argv\":[]}]}";

/// A baseline whose tests failed.
pub(super) fn baseline_failed() -> String {
    BASELINE_OK.replacen(
        "\"Success\",\"argv\":[]}]",
        "{\"Failure\":100},\"argv\":[]}]",
        1,
    )
}

/// One phase result.
pub(super) fn phase(name: &str, status: &str, argv: &str) -> String {
    format!(
        "{{\"phase\":\"{name}\",\"duration\":0.25,\"process_status\":{status},\"argv\":{argv}}}"
    )
}

/// The mutant object of a scenario or of the list, named `{FILE}:10:5: {change}`.
fn mutant_object(change: &str) -> String {
    format!(
        "{{\"name\":\"{FILE}:10:5: {change}\",\"package\":\"ironafs-time\",\"file\":\"{FILE}\",\
         \"function\":{{\"function_name\":\"checked_add\",\"return_type\":\"-> Option<Self>\",\
         \"span\":{{\"start\":{{\"line\":8,\"column\":5}},\"end\":{{\"line\":12,\"column\":6}}}}}},\
         \"span\":{{\"start\":{{\"line\":10,\"column\":5}},\"end\":{{\"line\":10,\"column\":9}}}},\
         \"replacement\":\"None\",\"genre\":\"FnValue\"}}"
    )
}

/// One mutant scenario with its summary and phases.
pub(super) fn mutant(change: &str, summary: &str, phases: &[String]) -> String {
    format!(
        "{{\"scenario\":{{\"Mutant\":{}}},\"summary\":\"{summary}\",\"log_path\":\"log/m.log\",\
         \"diff_path\":\"diff/m.diff\",\"phase_results\":[{}]}}",
        mutant_object(change),
        phases.join(",")
    )
}

/// A whole `outcomes.json` of cargo-mutants 27.1.0 holding `scenarios`.
pub(super) fn outcomes(scenarios: &[&str]) -> String {
    outcomes_of_version("27.1.0", scenarios)
}

/// A whole `outcomes.json` written by the given tool version.
pub(super) fn outcomes_of_version(version: &str, scenarios: &[&str]) -> String {
    format!(
        "{{\"outcomes\":[{}],\"total_mutants\":0,\"missed\":0,\"caught\":0,\"timeout\":0,\
         \"unviable\":0,\"success\":0,\"start_time\":\"2026-11-21T03:17:00Z\",\
         \"end_time\":\"2026-11-21T03:47:00Z\",\"cargo_mutants_version\":\"{version}\"}}",
        scenarios.join(",")
    )
}

/// The `--list --json` output naming one mutant per change.
pub(super) fn list(changes: &[&str]) -> String {
    let objects: Vec<String> = changes.iter().map(|change| mutant_object(change)).collect();
    format!("[{}]", objects.join(","))
}
