// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the seam checks on sources held in memory, and for the
//! self-test's planted cases.

use super::{Sampling, SourceFile, Violation, check, self_test, source_file};

const SAMPLING: &[Sampling] = &[Sampling {
    krate: "core-crate",
    path: "core_crate::time::deadline_after",
}];

fn core_file(source: &str) -> SourceFile {
    source_file("core-crate", "src/time.rs", false, false, source).unwrap()
}

const CONTROL: &str = "
#[expect(clippy::disallowed_methods, reason = \"the one sampling function\")]
pub fn deadline_after(clock: &dyn Clock, from: &Stamp, d: Duration) -> Result<Deadline, DeadlineError> {
    let now = clock.sample().map_err(DeadlineError::Clock)?;
    Deadline::boot_after(&now, from, d)
}
pub fn serve(clock: &dyn Clock, request: &Request, d: Duration) -> Result<Deadline, DeadlineError> {
    deadline_after(clock, &request.arrival, d)
}
";

#[test]
fn deadline_check_passes_the_control() {
    assert_eq!(check(&[core_file(CONTROL)], SAMPLING), []);
}

#[test]
fn deadline_check_refuses_a_constructor_outside_the_sampling_function() {
    let source = format!(
        "{CONTROL}\npub fn other(now: &Stamp, from: &Stamp, d: Duration) -> Result<Deadline, DeadlineError> {{ Deadline::awake_after(now, from, d) }}\n"
    );
    let found = check(&[core_file(&source)], SAMPLING);
    assert!(
        matches!(
            found.as_slice(),
            [Violation::Outside {
                name: "awake_after",
                ..
            }]
        ),
        "{found:?}"
    );
}

#[test]
fn deadline_check_refuses_a_kept_now() {
    let source = CONTROL.replace(
        "Deadline::boot_after(&now, from, d)",
        "let kept = now; Deadline::boot_after(&kept, from, d)",
    );
    let found = check(&[core_file(&source)], SAMPLING);
    assert!(
        matches!(found.as_slice(), [Violation::NowNotFresh { .. }]),
        "{found:?}"
    );
}

#[test]
fn deadline_check_refuses_a_fresh_from() {
    let source = CONTROL.replace("&request.arrival", "&clock.sample()?");
    let found = check(&[core_file(&source)], SAMPLING);
    assert!(
        matches!(found.as_slice(), [Violation::BadFrom { .. }]),
        "{found:?}"
    );
}

#[test]
fn deadline_check_ignores_cfg_test_items() {
    let source = format!(
        "{CONTROL}\n#[cfg(test)]\nfn planted(now: &Stamp) {{ let _ = Deadline::boot_after(now, now, Duration::ZERO); }}\n"
    );
    assert_eq!(check(&[core_file(&source)], SAMPLING), []);
}

fn pure_file(krate: &str, path: &str, source: &str) -> SourceFile {
    source_file(krate, path, true, path == "src/lib.rs", source).unwrap()
}

#[test]
fn pure_check_passes_definitions_and_root_reexports() {
    let files = [
        pure_file(
            "ironafs-time",
            "src/lib.rs",
            "mod seam;\npub use seam::{\n    Clock, ClockError,\n    Rng,\n};\n",
        ),
        pure_file(
            "ironafs-time",
            "src/seam.rs",
            "pub trait Clock: Send + Sync {\n    fn boot(&self) -> u64;\n}\npub trait Rng { fn fill(&self); }\n",
        ),
    ];
    assert_eq!(check(&files, SAMPLING), []);
}

#[test]
fn pure_check_refuses_a_seam_held_in_a_pure_crate() {
    let files = [pure_file(
        "ironafs-rx",
        "src/endpoint.rs",
        "pub fn tick(clock: &dyn ironafs_time::Clock) -> u64 { 0 }\n",
    )];
    let found = check(&files, SAMPLING);
    assert!(
        matches!(
            found.as_slice(),
            [Violation::SeamInPure { name: "Clock", .. }]
        ),
        "{found:?}"
    );
}

#[test]
fn pure_check_refuses_a_provided_method_on_a_seam() {
    let files = [pure_file(
        "ironafs-time",
        "src/seam.rs",
        "pub trait Clock {\n    fn boot(&self) -> u64;\n    fn twice(&self) -> u64 { self.boot() }\n}\n",
    )];
    let found = check(&files, SAMPLING);
    assert!(
        matches!(
            found.as_slice(),
            [Violation::ProvidedMethod { name: "Clock", .. }]
        ),
        "{found:?}"
    );
}

#[test]
fn self_test_catches_every_planted_case() {
    let mut output = Vec::new();
    self_test::run(&mut output).unwrap();
    let report = String::from_utf8(output).unwrap();
    assert!(report.contains("check-seams --self-test:"), "{report}");
}
