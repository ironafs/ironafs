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
    let cases: [(&str, String, &str); 5] = [
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
            "holds 2 unsafe blocks and impls",
        ),
        (
            "the open flag differing from the ADR",
            text.replacen("\"open\": true", "\"open\": false", 1),
            "an open owner decision",
        ),
    ];
    assert_caught(&text, &cases);
}
