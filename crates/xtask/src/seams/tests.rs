// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the seam checks on sources held in memory, and for the
//! self-test's planted cases.

use std::path::Path;

use super::{Sampling, SourceFile, Violation, check, renamed_dependencies, self_test, source_file};

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
        found
            .iter()
            .any(|found| matches!(found, Violation::NowNotFresh { .. })),
        "{found:?}"
    );
    assert!(
        found.iter().all(|found| matches!(
            found,
            Violation::NowNotFresh { .. } | Violation::Reassigned { .. }
        )),
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

#[test]
fn pure_check_refuses_a_seam_through_an_uppercase_crate_alias() {
    let files = [pure_file(
        "ironafs-rx",
        "src/endpoint.rs",
        "use ironafs_time as Time;\npub fn tick(clock: &dyn Time::Clock) -> u64 { 0 }\n",
    )];
    let found = check(&files, SAMPLING);
    assert!(
        found
            .iter()
            .any(|found| matches!(found, Violation::SeamInPure { name: "Clock", .. })),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|found| matches!(found, Violation::HomeImport { .. })),
        "{found:?}"
    );
}

#[test]
fn pure_check_passes_a_declared_variant_through_its_enum_and_self() {
    let files = [pure_file(
        "ironafs-time",
        "src/wait.rs",
        "pub enum DeadlineError { Clock(u8), Overflow }\n\
         pub fn failed() -> DeadlineError { DeadlineError::Clock(0) }\n\
         impl DeadlineError { fn code(&self) -> u8 { match self { Self::Clock(code) => *code, Self::Overflow => 0 } } }\n",
    )];
    assert_eq!(check(&files, SAMPLING), []);
}

#[test]
fn pure_check_refuses_a_variant_path_through_an_enum_without_that_variant() {
    let files = [pure_file(
        "ironafs-rx",
        "src/endpoint.rs",
        "pub enum Time { Other }\npub fn tick(clock: &dyn Time::Clock) {}\n",
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
fn deadline_check_refuses_a_parameter_named_now() {
    let source = CONTROL
        .replace(
            "pub fn deadline_after(clock: &dyn Clock, from",
            "pub fn deadline_after(clock: &dyn Clock, now: &Stamp, from",
        )
        .replace(
            "    let now = clock.sample().map_err(DeadlineError::Clock)?;\n    Deadline::boot_after(&now, from, d)",
            "    let built = Deadline::boot_after(&now, from, d);\n    let now = clock.sample().map_err(DeadlineError::Clock)?;\n    built",
       )
        .replace("deadline_after(clock, &request.arrival, d)", "deadline_after(clock, &kept, &request.arrival, d)");
    let found = check(&[core_file(&source)], SAMPLING);
    // The constructor runs before the fresh sample is bound.
    assert!(
        found
            .iter()
            .any(|found| matches!(found, Violation::NowNotFresh { .. })),
        "{found:?}"
    );
    assert!(
        found.iter().all(|found| matches!(
            found,
            Violation::Reassigned { .. } | Violation::NowNotFresh { .. }
        )),
        "{found:?}"
    );
}

#[test]
fn deadline_check_refuses_a_shadowing_pattern_binding() {
    let source = CONTROL.replace(
        "    Deadline::boot_after(&now, from, d)",
        "    if let Some(now) = kept { return Deadline::boot_after(&now, from, d); }\n    Deadline::boot_after(&now, from, d)",
    );
    let found = check(&[core_file(&source)], SAMPLING);
    assert!(
        matches!(found.as_slice(), [Violation::Reassigned { .. }, ..]),
        "{found:?}"
    );
}

#[test]
fn deadline_check_refuses_a_from_other_than_the_parameter() {
    let source = CONTROL
        .replace(
            "from: &Stamp, d: Duration)",
            "from: &Stamp, d: Duration, forged: &Stamp)",
        )
        .replace(
            "Deadline::boot_after(&now, from, d)",
            "Deadline::boot_after(&now, forged, d)",
        )
        .replace(
            "&request.arrival, d)",
            "&request.arrival, d, &request.arrival)",
        );
    let found = check(&[core_file(&source)], SAMPLING);
    assert!(
        matches!(found.as_slice(), [Violation::NotFromParameter { .. }]),
        "{found:?}"
    );
}

#[test]
fn deadline_check_covers_ironafs_time_outside_the_two_definitions() {
    let definitions = "impl Deadline {\n    pub fn boot_after(now: &Stamp, from: &Stamp, d: Duration) -> Result<Self, DeadlineError> { todo() }\n    pub fn awake_after(now: &Stamp, from: &Stamp, d: Duration) -> Result<Self, DeadlineError> { todo() }\n}\n";
    let time =
        |source: &str| source_file("ironafs-time", "src/wait.rs", true, false, source).unwrap();
    assert_eq!(check(&[time(definitions)], SAMPLING), []);
    let helper = format!(
        "{definitions}pub fn deadline(now: &Stamp, from: &Stamp, d: Duration) -> Result<Deadline, DeadlineError> {{ Deadline::boot_after(now, from, d) }}\n"
    );
    let found = check(&[time(&helper)], SAMPLING);
    assert!(
        matches!(
            found.as_slice(),
            [Violation::Outside {
                name: "boot_after",
                ..
            }]
        ),
        "{found:?}"
    );
}

#[test]
fn seams_check_refuses_a_path_attribute() {
    let source = format!("{CONTROL}\n#[path = \"../elsewhere.rs\"]\nmod elsewhere;\n");
    let found = check(&[core_file(&source)], SAMPLING);
    assert!(
        matches!(found.as_slice(), [Violation::PathAttribute { .. }]),
        "{found:?}"
    );
}

fn time_error_enum() -> SourceFile {
    pure_file(
        "ironafs-time",
        "src/wait.rs",
        "pub enum DeadlineError { Clock(u8), Overflow }\n",
    )
}

#[test]
fn pure_check_refuses_a_rooted_path_through_an_enum_named_like_a_home_crate() {
    let files = [pure_file(
        "ironafs-rx",
        "src/endpoint.rs",
        "pub enum ironafs_time { Clock }\npub fn tick(clock: &dyn ::ironafs_time::Clock) {}\n",
    )];
    let found = check(&files, SAMPLING);
    assert!(
        found.iter().any(|found| matches!(
            found,
            Violation::SeamInPure {
                name: "Clock",
                line: 2,
                ..
            }
        )),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|found| matches!(found, Violation::HomeShadow { line: 1, .. })),
        "{found:?}"
    );
}

#[test]
fn pure_check_refuses_a_home_enum_path_that_is_not_imported() {
    // The name could be a renamed dependency, not the enum.
    let files = [
        time_error_enum(),
        pure_file(
            "ironafs-rx",
            "src/endpoint.rs",
            "pub fn tick(clock: &dyn DeadlineError::Clock) {}\n",
        ),
    ];
    let found = check(&files, SAMPLING);
    assert!(
        matches!(
            found.as_slice(),
            [Violation::SeamInPure {
                name: "Clock",
                line: 1,
                ..
            }]
        ),
        "{found:?}"
    );
}

#[test]
fn pure_check_passes_a_home_enum_variant_imported_or_by_full_path() {
    let files = [
        time_error_enum(),
        pure_file(
            "ironafs-rx",
            "src/endpoint.rs",
            "use ironafs_time::{DeadlineError, Stamp};\n\
             pub fn failed() -> DeadlineError { DeadlineError::Clock(0) }\n\
             pub fn also() -> ironafs_time::DeadlineError { ironafs_time::DeadlineError::Clock(1) }\n",
        ),
    ];
    assert_eq!(check(&files, SAMPLING), []);
}

#[test]
fn renamed_dependency_in_a_pure_manifest_is_refused() {
    let manifest = "[package]\nname = \"ironafs-rx\"\n\n[dependencies]\n\
                    DeadlineError = { package = \"ironafs-time\", path = \"../ironafs-time\" }\n\
                    ironafs-time.workspace = true\n\n[dependencies.Other]\npackage = \"ironafs-time\"\n";
    let found = renamed_dependencies(Path::new("crates/ironafs-rx/Cargo.toml"), manifest);
    assert!(
        matches!(
            found.as_slice(),
            [
                Violation::RenamedDependency { line: 5, .. },
                Violation::RenamedDependency { line: 9, .. }
            ]
        ),
        "{found:?}"
    );
    assert_eq!(
        renamed_dependencies(
            Path::new("Cargo.toml"),
            "[package]\nname = \"a\" # package = no\n"
        ),
        []
    );
}

#[test]
fn seams_check_refuses_an_include_macro() {
    let source = format!("{CONTROL}\ninclude!(\"../generated.rs\");\n");
    let found = check(&[core_file(&source)], SAMPLING);
    assert!(
        matches!(found.as_slice(), [Violation::IncludeMacro { .. }]),
        "{found:?}"
    );
}

#[test]
fn deadline_check_refuses_a_sample_from_a_parameter_that_is_not_the_clock() {
    let source = CONTROL
        .replace(
            "pub fn deadline_after(clock: &dyn Clock, from",
            "pub fn deadline_after(clock: &dyn Clock, fake: &Fake, from",
        )
        .replace("let now = clock.sample()", "let now = fake.sample()")
        .replace(
            "deadline_after(clock, &request.arrival",
            "deadline_after(clock, &fake, &request.arrival",
        );
    let found = check(&[core_file(&source)], SAMPLING);
    assert!(
        found
            .iter()
            .any(|found| matches!(found, Violation::NoFreshSample { .. })),
        "{found:?}"
    );
}
