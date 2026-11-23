// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the inventory reader: the repository's own copy and ADR, and
//! planted departures from them.

use std::path::Path;

use super::{BlockKind, Status, adr_rows, parse, problems};

fn repository_file(path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap()
}

fn inventory_text() -> String {
    repository_file("crates/ironafs-os/sites.json")
}

fn adr_text() -> String {
    repository_file("docs/adr/0005-ironafs-os.md")
}

fn problems_of(inventory: &str) -> Vec<String> {
    problems(&parse(inventory).unwrap(), &adr_text())
}

#[test]
fn the_repository_copy_matches_the_adr_table_and_its_own_rules() {
    assert_eq!(problems_of(&inventory_text()), [] as [String; 0]);
}

#[test]
fn adr_rows_read_all_twenty_four_rows_with_escaped_bars_kept() {
    let rows = adr_rows(&adr_text());
    assert_eq!(rows.len(), 24);
    assert_eq!(rows.first().map(|row| row.id.as_str()), Some("linux-clock"));
    assert_eq!(rows.last().map(|row| row.id.as_str()), Some("mem-wipe"));
    let udp6 = rows.iter().find(|row| row.id == "linux-udp6").unwrap();
    assert!(udp6.columns[1].contains("`SOCK_DGRAM \\| SOCK_CLOEXEC`"));
}

#[test]
fn parse_reads_statuses_and_the_code_of_implemented_rows() {
    let inventory = parse(&inventory_text()).unwrap();
    let clock = inventory
        .rows
        .iter()
        .find(|row| row.id == "linux-clock")
        .unwrap();
    assert_eq!(
        clock.status.get("aarch64-unknown-linux-gnu"),
        Some(&Status::Implemented)
    );
    let code = clock.code.as_ref().unwrap();
    assert_eq!(code.module, "linux_clock");
    assert_eq!(
        code.blocks.first().map(|block| block.kind),
        Some(BlockKind::Call)
    );
    let pipe_io = inventory
        .rows
        .iter()
        .find(|row| row.id == "windows-pipe-io")
        .unwrap();
    assert!(pipe_io.open && pipe_io.code.is_none());
}

#[test]
fn parse_reads_a_row_s_c_expressions_and_refuses_any_outside_their_alphabet() {
    let text = inventory_text();
    let with = |expressions: &str| {
        text.replacen(
            "\"module\": \"linux_clock\",",
            &format!("\"module\": \"linux_clock\",\n      \"expressions\": {expressions},"),
            1,
        )
    };
    let clock = |inventory: &super::Inventory| {
        inventory
            .rows
            .iter()
            .find(|row| row.id == "linux-clock")
            .and_then(|row| row.code.clone())
            .unwrap()
    };
    assert_eq!(
        clock(&parse(&text).unwrap()).expressions,
        Vec::<(String, String)>::new()
    );
    assert_eq!(
        clock(&parse(&with("{\"CMSG_LEN_ONE\": \"CMSG_LEN(sizeof(int))\"}")).unwrap()).expressions,
        [(
            "CMSG_LEN_ONE".to_owned(),
            "CMSG_LEN(sizeof(int))".to_owned()
        )]
    );
    for refused in [
        "{\"X\": \"1; int y\"}",
        "{\"X\": \"\\\"s\\\"\"}",
        "{\"X\": \"1)\\n#include <x.h>\\n(1\"}",
        "{\"X\": \"\"}",
        "{\"x y\": \"1\"}",
        "{\"X\": \"4\"}",
        "{\"X\": \"(2 * 2) + 0x0\"}",
    ] {
        let error = parse(&with(refused)).unwrap_err();
        assert!(error.contains("expressions"), "{refused}: {error}");
    }
}

#[test]
fn parse_reads_a_row_s_feature_macros_and_refuses_any_but_gnu_source() {
    let text = inventory_text();
    let with = |defines: &str| {
        text.replacen(
            "\"module\": \"linux_clock\",",
            &format!("\"module\": \"linux_clock\",\n      \"defines\": {defines},"),
            1,
        )
    };
    let clock = |inventory: &super::Inventory| {
        inventory
            .rows
            .iter()
            .find(|row| row.id == "linux-clock")
            .and_then(|row| row.code.clone())
            .unwrap()
    };
    assert_eq!(clock(&parse(&text).unwrap()).defines, Vec::<String>::new());
    assert_eq!(
        clock(&parse(&with("[\"_GNU_SOURCE\"]")).unwrap()).defines,
        ["_GNU_SOURCE"]
    );
    let refused = parse(&with("[\"_POSIX_C_SOURCE 1\\n#include <x.h>\"]")).unwrap_err();
    assert!(refused.contains("is not a feature macro"), "{refused}");
}

#[test]
fn parse_reads_a_row_s_qualified_constants_and_refuses_any_it_does_not_record() {
    let text = inventory_text();
    let with = |qualified: &str| {
        text.replacen(
            "\"module\": \"linux_clock\",",
            &format!("\"module\": \"linux_clock\",\n      \"qualified\": {qualified},"),
            1,
        )
    };
    let clock = |inventory: &super::Inventory| {
        inventory
            .rows
            .iter()
            .find(|row| row.id == "linux-clock")
            .and_then(|row| row.code.clone())
            .unwrap()
    };
    assert_eq!(
        clock(&parse(&text).unwrap()).qualified,
        Vec::<String>::new()
    );
    assert_eq!(
        clock(&parse(&with("[\"CLOCK_BOOTTIME\"]")).unwrap()).qualified,
        ["CLOCK_BOOTTIME"]
    );
    // A qualified constant proved through an expression would not be
    // compared with the name the probe found.
    let both = with("[\"CLOCK_BOOTTIME\"]").replacen(
        "\"module\": \"linux_clock\",",
        "\"module\": \"linux_clock\",\n      \"expressions\": {\"CLOCK_BOOTTIME\": \"CLOCK_MONOTONIC + 6\"},",
        1,
    );
    let refused = parse(&both).unwrap_err();
    assert!(
        refused.contains("compared with its own C name"),
        "{refused}"
    );
    let refused = parse(&with("[\"SCM_PIDFD\"]")).unwrap_err();
    assert!(
        refused.contains("not one of the row's constants"),
        "{refused}"
    );
}

#[test]
fn problems_name_each_planted_departure() {
    let text = inventory_text();
    let cases: [(&str, String, &str); 9] = [
        (
            "a text column changed",
            text.replacen("called with `CLOCK_BOOTTIME`", "called with `CLOCK_REALTIME`", 1),
            "differs from the ADR's row",
        ),
        (
            "musl differing from glibc",
            text.replacen(
                "\"x86_64-unknown-linux-musl\": \"implemented\"",
                "\"x86_64-unknown-linux-musl\": \"approved\"",
                1,
            ),
            "a musl build compiles",
        ),
        (
            "a target outside the allowlist",
            text.replacen(
                "\"aarch64-apple-darwin\": \"implemented\"",
                "\"aarch64-apple-darwin\": \"implemented\",\n        \"x86_64-apple-darwin\": \"implemented\"",
                1,
            ),
            "is not a target of ADR 0005",
        ),
        (
            "an implemented row with no code",
            text.replacen(
                "\"aarch64-apple-darwin\": \"approved\"",
                "\"aarch64-apple-darwin\": \"implemented\"",
                1,
            ),
            "its code is not recorded",
        ),
        (
            "an open row implemented",
            text.replacen(
                "\"open\": true,\n      \"status\": {\n        \"x86_64-pc-windows-msvc\": \"approved\"",
                "\"open\": true,\n      \"status\": {\n        \"x86_64-pc-windows-msvc\": \"implemented\"",
                1,
            ),
            "the ADR leaves this row open",
        ),
        (
            "a call of a function the row does not declare",
            text.replacen(
                "\"operation\": \"abi::clock_gettime\"",
                "\"operation\": \"abi::clock_settime\"",
                1,
            ),
            "cannot hold `abi::clock_settime(id, out)`",
        ),
        (
            "a write that is not volatile",
            text.replacen(
                "\"operation\": \"core::ptr::write_volatile\"",
                "\"operation\": \"core::ptr::write\"",
                1,
            ),
            "cannot hold `core::ptr::write(byte, 0)`",
        ),
        (
            "an unknown block kind",
            text.replacen("\"kind\": \"write\"", "\"kind\": \"poke\"", 1),
            "block kind `poke`",
        ),
        (
            "a row removed",
            text.replacen("\"id\": \"darwin-clock\"", "\"id\": \"darwin-clocks\"", 1),
            "differs from the ADR's row",
        ),
    ];
    assert_caught(&text, &cases);
}

/// Each case's plant changes the copy, and the problems found include its
/// expected phrase.
#[track_caller]
fn assert_caught(text: &str, cases: &[(&str, String, &str)]) {
    for (name, planted, expected) in cases {
        assert_ne!(planted.as_str(), text, "{name}: the plant changed nothing");
        let found = match parse(planted) {
            Ok(inventory) => problems(&inventory, &adr_text()),
            Err(error) => vec![error],
        };
        assert!(
            found.iter().any(|problem| problem.contains(*expected)),
            "{name}: {found:?}"
        );
    }
}

#[test]
fn problems_bind_the_recorded_code_to_the_adr_row() {
    let text = inventory_text();
    let cases: [(&str, String, &str); 9] = [
        (
            "a same-named write of another path, the last segment kept, recorded alike",
            text.replacen(
                "\"operation\": \"core::ptr::write_volatile\"",
                "\"operation\": \"crate::shadow::write_volatile\"",
                1,
            ),
            "cannot hold `crate::shadow::write_volatile(byte, 0)`",
        ),
        (
            "the volatile write swapped for an unsafe impl, recorded alike",
            text.replacen(
                "\"blocks\": [\n          {\n            \"kind\": \"write\",\n            \"operation\": \"core::ptr::write_volatile\",\n            \"arguments\": [\n              \"byte\",\n              \"0\"\n            ]\n          }\n        ],\n        \"impls\": 0",
                "\"blocks\": [],\n        \"impls\": 1",
                1,
            ),
            "records 1 unsafe impls; the ADR's row names none",
        ),
        (
            "the volatile write removed, recorded alike",
            text.replacen(
                "\"blocks\": [\n          {\n            \"kind\": \"write\",\n            \"operation\": \"core::ptr::write_volatile\",\n            \"arguments\": [\n              \"byte\",\n              \"0\"\n            ]\n          }\n        ],",
                "\"blocks\": [],",
                1,
            ),
            "no write block of zero is recorded",
        ),
        (
            "a syscall call with three variadic arguments, recorded alike",
            text.replacen(
                "\"flags\",\n              \"unused3\",\n              \"unused4\",\n              \"unused5\"",
                "\"flags\"",
                1,
            ),
            "calls syscall with 4 arguments",
        ),
        (
            "a volatile write of a value other than zero",
            text.replacen("\"byte\",\n              \"0\"", "\"byte\",\n              \"1\"", 1),
            "cannot hold `core::ptr::write_volatile(byte, 1)`",
        ),
        (
            "a declaration and its call, both recorded, that the ADR does not name",
            text.replacen("\"name\": \"clock_gettime\"", "\"name\": \"clock_settime\"", 1)
                .replacen(
                    "\"operation\": \"abi::clock_gettime\"",
                    "\"operation\": \"abi::clock_settime\"",
                    1,
                ),
            "declares `clock_settime`, which the ADR's row does not name",
        ),
        (
            "a link the ADR's row does not name",
            text.replacen("\"link\": null", "\"link\": \"name=rt\"", 1),
            "an extern block linked as",
        ),
        (
            "a second block where the ADR's row says one",
            text.replacen(
                "\"blocks\": [\n          {\n            \"kind\": \"write\"",
                "\"blocks\": [\n          {\n            \"kind\": \"write\", \"operation\": \"core::ptr::write_volatile\", \"arguments\": [\"byte\", \"0\"]\n          },\n          {\n            \"kind\": \"write\"",
                1,
            ),
            "holds 2 unsafe blocks;",
        ),
        (
            "the open flag differing from the ADR",
            text.replacen("\"open\": true", "\"open\": false", 1),
            "an open owner decision",
        ),
    ];
    assert_caught(&text, &cases);
}
