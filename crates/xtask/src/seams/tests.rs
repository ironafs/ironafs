// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the seam checks on sources held in memory, and for the
//! self-test's planted cases.

use std::path::PathBuf;

use super::{Sampling, SourceFile, Violation, check, renamed_dependencies, self_test, source_file};
use crate::metadata::Member;

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
fn pure_check_passes_a_declared_variant_and_self() {
    let files = [pure_file(
        "ironafs-time",
        "src/wait.rs",
        "pub enum DeadlineError { Clock(u8), Overflow }\n\
         impl DeadlineError {\n\
             fn failed() -> Self { Self::Clock(0) }\n\
             fn code(&self) -> u8 { match self { Self::Clock(code) => *code, Self::Overflow => 0 } }\n\
         }\n",
    )];
    assert_eq!(check(&files, SAMPLING), []);
}

#[test]
fn pure_check_refuses_a_variant_path_other_than_self_even_through_its_enum() {
    // A path is not resolved by name: only the declaration and `Self::`
    // are exempt.
    let files = [pure_file(
        "ironafs-time",
        "src/wait.rs",
        "pub enum DeadlineError { Clock(u8), Overflow }\n\
         pub fn failed() -> DeadlineError { DeadlineError::Clock(0) }\n",
    )];
    let found = check(&files, SAMPLING);
    assert!(
        matches!(
            found.as_slice(),
            [Violation::SeamInPure {
                name: "Clock",
                line: 2,
                ..
            }]
        ),
        "{found:?}"
    );
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
fn pure_check_refuses_a_home_enum_variant_imported_or_by_full_path() {
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
    let found = check(&files, SAMPLING);
    let lines: Vec<usize> = found
        .iter()
        .map(|found| match found {
            Violation::SeamInPure {
                name: "Clock",
                line,
                ..
            } => *line,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(lines, [2, 3]);
}

fn member(name: &str, renamed: &[&str]) -> Member {
    Member {
        name: name.into(),
        id: name.into(),
        dir: PathBuf::from(format!("/w/crates/{name}")),
        lib_root: None,
        targets: Vec::new(),
        target_kinds: vec!["lib".into()],
        editions: vec!["2024".into()],
        build_script: None,
        declared_dependencies: Vec::new(),
        renamed: renamed.iter().map(|name| (*name).into()).collect(),
    }
}

#[test]
fn renamed_dependency_of_a_pure_crate_is_refused_from_the_resolved_graph() {
    // Whatever the manifest's spelling (inline, quoted or dotted key, or
    // inherited from the workspace table), cargo resolves the rename.
    let found = renamed_dependencies(&[
        member("ironafs-rx", &["DeadlineError"]),
        member("xtask", &["renamed_but_not_pure"]),
        member("ironafs-cache", &[]),
    ]);
    assert_eq!(
        found,
        [Violation::RenamedDependency {
            krate: "ironafs-rx".into(),
            name: "DeadlineError".into()
        }]
    );
}

#[test]
fn seams_check_refuses_a_path_inside_cfg_attr() {
    let source = format!("{CONTROL}\n#[cfg_attr(all(), path = \"tests.rs\")]\nmod hidden;\n");
    let found = check(&[core_file(&source)], SAMPLING);
    assert!(
        matches!(found.as_slice(), [Violation::PathAttribute { .. }]),
        "{found:?}"
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

#[test]
fn pure_check_passes_the_real_production_rng() {
    // The production `Rng` of 5.2, point 3, as `ironafs-crypto` writes it.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let read = |path: &str| std::fs::read_to_string(dir.join(path)).unwrap();
    let files = [
        pure_file(
            "ironafs-crypto",
            "src/lib.rs",
            &read("ironafs-crypto/src/lib.rs"),
        ),
        pure_file(
            "ironafs-crypto",
            "src/random.rs",
            &read("ironafs-crypto/src/random.rs"),
        ),
    ];
    assert_eq!(check(&files, SAMPLING), []);
}

#[test]
fn workspace_check_reports_exact_written_implementations() {
    let mut output = Vec::new();
    super::census::self_test::run(&mut output).unwrap();
}

use super::census::self_test::{census_fixture, census_output, implementation_rows};

#[test]
fn census_distinguishes_written_rows_from_unresolved_source() {
    let dir = crate::fixture::fresh_dir(&std::env::temp_dir(), "seam-census-scopes").unwrap();
    let result = (|| -> Result<_, crate::Error> {
        let mut fixture = census_fixture();
        fixture.file(
            "outside-role",
            "src/lib.rs",
            Some(
                "#[cfg(test)] mod unit;\n\
                 mod inline { #![cfg(test)]\n\
                 struct Wrap<T>(T);\n\
                 impl<T> ::ironafs_time::Clock for Wrap<T> where T: Copy {}\n\
                 }\n\
                 mod local {\n\
                 trait Clock {}\n\
                 struct Wrong;\n\
                 impl Clock for Wrong {}\n\
                 impl Wrong {}\n\
                 }\n\
                 /// impl Clock for Documented {}\n\
                 const TEXT: &str = \"impl Clock for StringOnly {}\";\n\
                 macro_rules! make { ($seam:path, $name:ident) => {\n\
                 struct $name; impl $seam for $name {}\n\
                 }; }\n\
                 use ironafs_time::Clock as Timing;\n\
                 make!(Timing, Made);\n\
                 struct Renamed; impl Timing for Renamed {}\n\
                 #[cfg(test)] fn block() { if !(false) {} }\n\
                 mod helper { macro_rules! custom { ($seam:path, $name:ident) => { struct $name; impl $seam for $name {} }; } pub(crate) use custom as assert; }\n\
                 fn custom() { helper::assert!(Timing, Generated); }\n",
            ),
        );
        fixture.file(
            "outside-role",
            "src/unit.rs",
            Some("use ironafs_time::Clock;\nstruct Unit;\nimpl Clock for Unit {}\n"),
        );
        fixture.file(
            "outside-role",
            "src/orphan.rs",
            Some("use ironafs_time::Clock;\nstruct Orphan;\nimpl Clock for Orphan {}\n"),
        );
        fixture.file(
            "outside-role",
            "tests/root.rs",
            Some("#[path = \"support/actual.rs\"] mod redirected;\n"),
        );
        fixture.file(
            "outside-role",
            "tests/support/actual.rs",
            Some("struct Redirect;\nimpl ironafs_time::Clock for Redirect {}\n"),
        );
        let workspace = fixture.load(&dir, super::CHECK, "written and unresolved source")?;
        census_output(&workspace)
    })();
    crate::fixture::remove_dir(&dir).unwrap();
    let output = result.unwrap();
    assert_eq!(
        implementation_rows(&output),
        [
            "check-seams: implementation ironafs-time::Clock | ironafs-time | test:double | crate | impl Clock for Double | crates/ironafs-time/tests/double.rs:3",
            "check-seams: implementation ironafs-time::Clock | outside-role | lib:outside_role | crate::inline | impl<T> ::ironafs_time::Clock for Wrap<T> where T: Copy | crates/outside-role/src/lib.rs:4 | cfg(test)",
            "check-seams: implementation ironafs-time::Clock | outside-role | lib:outside_role | crate::unit | impl Clock for Unit | crates/outside-role/src/unit.rs:3 | cfg(test)",
            "check-seams: implementation ironafs-time::Clock | outside-role | test:root | crate::redirected | impl ironafs_time::Clock for Redirect | crates/outside-role/tests/support/actual.rs:2",
        ]
    );
    assert!(output.contains("check-seams: written-source census incomplete"));
    assert!(output.contains("crates/outside-role/src/orphan.rs:1 | unlinked"));
    assert!(output.contains("crates/outside-role/src/lib.rs:18 | unexpanded item macro make!"));
    assert!(output.contains(
        "crates/outside-role/src/lib.rs:19 | cannot bind trait in impl Timing for Renamed"
    ));
    assert!(!output.contains("if!"));
    assert!(
        output.contains("crates/outside-role/src/lib.rs:22 | unexpanded macro in a block assert!")
    );
}

#[test]
fn census_keeps_cfg_on_qualified_functions() {
    let output = census_with_library(
        "seam-census-qualified-functions",
        "#[cfg(all())]\n\
         pub(crate) const unsafe extern \"C\" fn combined() {\n\
         struct Combined;\n\
         impl ironafs_time::Clock for Combined {}\n\
         }\n\
         #[cfg_attr(test, allow(dead_code))]\n\
         pub async fn asynchronous() {\n\
         struct Async;\n\
         impl ironafs_time::Clock for Async {}\n\
         }\n\
         #[cfg(all())]\n\
         extern fn implicit_abi() {\n\
         struct Implicit;\n\
         impl ironafs_time::Clock for Implicit {}\n\
         }\n",
    );
    assert_eq!(
        implementation_rows(&output)
            .into_iter()
            .filter(|row| row.contains(" | outside-role | "))
            .collect::<Vec<_>>(),
        [
            "check-seams: implementation ironafs-time::Clock | outside-role | lib:outside_role | crate::{line 12} | impl ironafs_time::Clock for Implicit | crates/outside-role/src/lib.rs:14 | cfg(all())",
            "check-seams: implementation ironafs-time::Clock | outside-role | lib:outside_role | crate::{line 2} | impl ironafs_time::Clock for Combined | crates/outside-role/src/lib.rs:4 | cfg(all())",
            "check-seams: implementation ironafs-time::Clock | outside-role | lib:outside_role | crate::{line 7} | impl ironafs_time::Clock for Async | crates/outside-role/src/lib.rs:9 | cfg_attr(test,allow(dead_code))",
        ],
        "{output}"
    );
    assert!(output.contains("crates/outside-role/src/lib.rs:6 | unresolved attribute cfg_attr"));
}

#[test]
fn census_resolves_super_from_a_module_in_nested_blocks() {
    let output = census_with_library(
        "seam-census-local-module",
        "mod outer {\n\
         use ironafs_time::Clock;\n\
         fn define() {\n\
         {\n\
         mod local {\n\
         struct Local;\n\
         impl super::Clock for Local {}\n\
         }\n\
         }\n\
         }\n\
         }\n",
    );
    assert!(
        implementation_rows(&output).contains(
            &"check-seams: implementation ironafs-time::Clock | outside-role | lib:outside_role | crate::outer::local::{line 3}::{line 4} | impl super::Clock for Local | crates/outside-role/src/lib.rs:7"
        ),
        "{output}"
    );
}

fn census_with_library(case: &'static str, source: &str) -> String {
    let dir = crate::fixture::fresh_dir(&std::env::temp_dir(), case).unwrap();
    let mut fixture = census_fixture();
    fixture.file("outside-role", "src/lib.rs", Some(source));
    let result = fixture
        .load(&dir, super::CHECK, case)
        .and_then(|workspace| census_output(&workspace));
    crate::fixture::remove_dir(&dir).unwrap();
    result.unwrap()
}

#[test]
fn census_reports_malformed_needed_source_without_a_partial_inventory() {
    let dir = crate::fixture::fresh_dir(&std::env::temp_dir(), "seam-census-inputs").unwrap();
    let result = (|| -> Result<_, crate::Error> {
        let mut fixture = census_fixture();
        fixture.file("outside-role", "tests/another.rs", Some("mod sibling;\n"));
        fixture.file("outside-role", "tests/sibling.rs", Some("struct Good;\n"));
        let workspace = fixture.load(&dir, super::CHECK, "malformed source")?;
        let file = workspace.root.join("crates/outside-role/tests/sibling.rs");
        let original = census_output(&workspace)?;
        std::fs::write(&file, [0xff]).unwrap();
        let mut output = Vec::new();
        let invalid = super::run_workspace(&workspace, &mut output).unwrap_err();
        assert!(
            invalid
                .to_string()
                .contains("tests/sibling.rs: not readable as Rust: not UTF-8")
        );
        assert_eq!(output, [] as [u8; 0]);

        std::fs::write(&file, "fn wrong() { let broken = (1]; }\n").unwrap();
        let unbalanced = census_output(&workspace).unwrap_err();
        assert!(
            unbalanced
                .to_string()
                .contains("tests/sibling.rs:1: mismatched delimiter")
        );

        std::fs::remove_file(&file).unwrap();
        let absent = census_output(&workspace).unwrap_err();
        assert!(absent.to_string().contains("tests/sibling.rs"));
        crate::fixture::write_file(&file, "struct Good;\n")?;
        assert_eq!(census_output(&workspace)?, original);
        Ok(())
    })();
    crate::fixture::remove_dir(&dir).unwrap();
    result.unwrap();
}
