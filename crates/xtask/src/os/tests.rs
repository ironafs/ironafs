// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the comparison of the crate's sites with the copy.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::inventory::{Block, BlockKind, Code, ExternBlock, Inventory, Row, Status};
use super::scan::{ExternItem, scan};
use super::{CrateFile, Violation, compare};
use crate::rust_source::tokens_with_strings;

const LINUX: &str = "x86_64-unknown-linux-gnu";
const WINDOWS: &str = "x86_64-pc-windows-msvc";

fn row(id: &str, implemented: bool, code: Option<Code>) -> Row {
    let status = if implemented {
        Status::Implemented
    } else {
        Status::Approved
    };
    Row {
        number: "1".into(),
        id: id.into(),
        columns: Default::default(),
        open: false,
        status: BTreeMap::from([(LINUX.to_owned(), status)]),
        code,
    }
}

fn clock_code() -> Code {
    Code {
        module: "linux_clock".into(),
        headers: vec!["time.h".into()],
        extern_blocks: vec![ExternBlock {
            link: None,
            items: vec![ExternItem {
                name: "clock_gettime".into(),
                safe: false,
            }],
        }],
        blocks: vec![Block {
            kind: BlockKind::Call,
            operation: "clock_gettime".into(),
            arguments: vec!["id".into(), "tp".into()],
        }],
        impls: 0,
        structs: vec!["timespec".into()],
        constants: Vec::new(),
        stand_in: None,
    }
}

fn inventory() -> Inventory {
    Inventory {
        rows: vec![
            row("linux-clock", true, Some(clock_code())),
            row("linux-peercred", false, None),
        ],
    }
}

const CLOCK: &str = "#[expect(unsafe_code, reason = \"linux-clock\")]\n\
                     unsafe extern \"C\" { fn clock_gettime(id: c_int, tp: *mut timespec) -> c_int; }\n\
                     fn read(id: c_int, tp: *mut timespec) -> c_int {\n\
                     #[expect(unsafe_code, reason = \"linux-clock\")]\n\
                     let status = unsafe { clock_gettime(id, tp) };\n status\n}\n";

fn file(path: &str, source: &str) -> CrateFile {
    CrateFile {
        path: PathBuf::from(path),
        scan: scan(&tokens_with_strings(source).unwrap(), false),
    }
}

fn compiled(paths: &[&str]) -> BTreeMap<String, BTreeSet<PathBuf>> {
    let set: BTreeSet<PathBuf> = paths.iter().map(PathBuf::from).collect();
    BTreeMap::from([
        (LINUX.to_owned(), set),
        (WINDOWS.to_owned(), BTreeSet::new()),
    ])
}

#[test]
fn compare_passes_code_equal_to_the_implemented_rows_on_each_target() {
    let files = [file("src/linux_clock.rs", CLOCK)];
    assert_eq!(
        compare(&inventory(), &files, &compiled(&["src/linux_clock.rs"])),
        []
    );
}

#[test]
fn compare_fails_a_row_implemented_but_missing_from_a_target() {
    let files = [file("src/linux_clock.rs", CLOCK)];
    let found = compare(&inventory(), &files, &compiled(&[]));
    assert!(
        matches!(found.as_slice(), [Violation::NotCompiled { .. }, Violation::NotCompiled { .. }, Violation::Mismatch { target, id, found, .. }]
            if target == LINUX && id == "linux-clock" && found.is_empty()),
        "{found:?}"
    );
}

#[test]
fn compare_fails_code_for_an_approved_only_row() {
    let peer = "#[expect(unsafe_code, reason = \"linux-peercred\")]\n\
                unsafe extern \"C\" { fn getsockopt(fd: c_int) -> c_int; }\n";
    let files = [
        file("src/linux_clock.rs", CLOCK),
        file("src/linux_peercred.rs", peer),
    ];
    let found = compare(
        &inventory(),
        &files,
        &compiled(&["src/linux_clock.rs", "src/linux_peercred.rs"]),
    );
    assert!(
        matches!(found.as_slice(), [Violation::Mismatch { id, expected, .. }]
            if id == "linux-peercred" && expected.is_empty()),
        "{found:?}"
    );
}

#[test]
fn compare_fails_a_swapped_declaration_with_the_same_block_count() {
    let swapped = CLOCK.replace("clock_gettime", "clock_settime");
    let files = [file("src/linux_clock.rs", &swapped)];
    let found = compare(&inventory(), &files, &compiled(&["src/linux_clock.rs"]));
    assert!(
        matches!(found.as_slice(), [Violation::Mismatch { id, .. }] if id == "linux-clock"),
        "{found:?}"
    );
}

#[test]
fn compare_fails_unnamed_unknown_and_misplaced_sites() {
    let unnamed = CLOCK.replacen("#[expect(unsafe_code, reason = \"linux-clock\")]\n", "", 1);
    let unknown = CLOCK.replace("linux-clock", "linux-clocks");
    for (name, files, check) in [
        (
            "unnamed",
            vec![file("src/linux_clock.rs", &unnamed)],
            (|found: &[Violation]| {
                matches!(found.first(), Some(Violation::Unnamed { line: 1, .. }))
            }) as fn(&[Violation]) -> bool,
        ),
        (
            "unknown",
            vec![file("src/linux_clock.rs", &unknown)],
            |found: &[Violation]| matches!(found.first(), Some(Violation::Unknown { id, .. }) if id == "linux-clocks"),
        ),
        (
            "outside its module",
            vec![file("src/clock.rs", CLOCK)],
            |found: &[Violation]| matches!(found.first(), Some(Violation::OutsideModule { module, .. }) if module == "linux_clock"),
        ),
    ] {
        let paths: Vec<String> = files
            .iter()
            .map(|file| file.path.display().to_string())
            .collect();
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        let found = compare(&inventory(), &files, &compiled(&paths));
        assert!(check(&found), "{name}: {found:?}");
    }
}

#[test]
fn guard_violations_compare_the_guard_with_the_list_s_verdict_by_whole_cfg_set() {
    use super::allowlist::guard_of;
    use super::guard_violations;

    const ROOT: &str = "#[cfg(not(all(target_os = \"linux\", target_arch = \"x86_64\")))]\n\
                        compile_error!(\"no\");\n";
    let listed = "target_os=\"linux\"\ntarget_arch=\"x86_64\"\ntarget_vendor=\"unknown\"\n";
    let cfgs = BTreeMap::from([
        (LINUX.to_owned(), super::allowlist::parse_cfg(listed)),
        // The same cfg set under another name: the list's verdict passes it.
        (
            "x86_64-twin-linux-gnu".to_owned(),
            super::allowlist::parse_cfg(listed),
        ),
        // Another vendor: the list's verdict refuses it, the guard does not.
        (
            "x86_64-other-linux-gnu".to_owned(),
            super::allowlist::parse_cfg(&listed.replace("unknown", "other")),
        ),
        (
            WINDOWS.to_owned(),
            super::allowlist::parse_cfg("target_os=\"windows\"\n"),
        ),
    ]);
    let guard = guard_of(ROOT).unwrap();
    let (violations, unlisted) = guard_violations(&guard, &cfgs, &[LINUX, WINDOWS]);
    assert_eq!(
        unlisted,
        ["x86_64-other-linux-gnu", "x86_64-twin-linux-gnu"]
    );
    let targets: Vec<&str> = violations
        .iter()
        .map(|violation| match violation {
            Violation::Target { target, .. } => target.as_str(),
            _ => "",
        })
        .collect();
    assert_eq!(targets, ["x86_64-other-linux-gnu", WINDOWS]);

    let (violations, _) = guard_violations(&guard, &cfgs, &[LINUX, "aarch64-unknown-unknown"]);
    assert!(violations.iter().any(|violation| matches!(
        violation,
        Violation::Target { target, what } if target == "aarch64-unknown-unknown"
            && what.contains("does not know")
    )));
}

#[test]
fn build_script_problem_needs_cargo_s_script_to_be_the_crate_s_own_file() {
    use super::build_script_problem;

    let dir = std::env::temp_dir().join(format!(
        "ironafs-xtask-os-build-script-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("other")).unwrap();
    let script = dir.join("build.rs");
    std::fs::write(&script, "fn main() {}\n").unwrap();
    std::fs::write(dir.join("other/build.rs"), "fn main() {}\n").unwrap();

    assert_eq!(build_script_problem(Some(&script), &dir), None);
    assert!(build_script_problem(None, &dir).is_some());
    let elsewhere = dir.join("other/build.rs");
    assert!(
        build_script_problem(Some(&elsewhere), &dir)
            .is_some_and(|problem| problem.contains("registers"))
    );
    #[cfg(unix)]
    {
        // Another name Cargo could register, a link to the crate's own
        // regular build.rs.
        let alias = dir.join("alias.rs");
        std::os::unix::fs::symlink(&script, &alias).unwrap();
        assert!(
            build_script_problem(Some(&alias), &dir)
                .is_some_and(|problem| problem.contains("registers"))
        );
        std::fs::remove_file(&script).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &script).unwrap();
        assert!(
            build_script_problem(Some(&script), &dir)
                .is_some_and(|problem| problem.contains("not a regular file"))
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn pinned_runs_build_apart_from_check_abi() {
    // check-abi builds the crate with the compiler Cargo selects; check-os's
    // pinned runs must never reuse what those builds leave behind.
    assert_ne!(super::PINNED_BUILD_DIR, crate::abi::BUILD_DIR);

    // Both of Cargo's directories: the target directory, and the build
    // directory, which a configuration (`build.build-dir`) can point
    // elsewhere, at check-abi's builds included.
    let pinned = super::pin::Pinned {
        rustc: PathBuf::from("/pinned/bin/rustc"),
    };
    let private = PathBuf::from("/w/target").join(super::PINNED_BUILD_DIR);
    let mut command = crate::Command::new("cargo");
    command.arg("check");
    super::pin::pinned_build(&mut command, &pinned, &private);
    let arguments: Vec<&std::ffi::OsStr> = command.get_args().collect();
    assert_eq!(
        arguments,
        ["check", "--target-dir", private.to_str().unwrap()]
    );
    let build_dir = command
        .get_envs()
        .find(|(name, _)| *name == "CARGO_BUILD_BUILD_DIR")
        .and_then(|(_, value)| value);
    assert_eq!(build_dir, Some(private.as_os_str()));
}
