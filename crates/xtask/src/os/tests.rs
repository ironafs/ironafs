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
