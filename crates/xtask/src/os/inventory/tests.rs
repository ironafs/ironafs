// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the inventory reader: the repository's own copy and ADR, and
//! planted departures from them.

use std::path::Path;

use super::{BlockKind, Status, adr_binding_problems, adr_rows, parse, problems};

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
        "{\"X\": \"1) || (0\"}",
        "{\"X\": \"1) | (0\"}",
        "{\"X\": \"(1\"}",
        "{\"X\": \"1)\"}",
        "{\"X\": \")1(\"}",
        "{\"X\": \"1 || 0\"}",
        "{\"X\": \"1 < 2\"}",
        "{\"X\": \"1 > 2\"}",
        "{\"X\": \"1 <<< 2\"}",
        "{\"X\": \"A | (1 << (B - 1))\"}",
        "{\"X\": \"sizeof(int)) + (0\"}",
        "{\"X\": \"1 /* 2 */\"}",
        "{\"X\": \"1, 2\"}",
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

#[test]
fn a_row_that_adopts_one_descriptor_holds_exactly_one_adoption() {
    let inventory = parse(&inventory_text()).unwrap();
    for id in ["linux-udp6", "linux-fdpass"] {
        let row = inventory.rows.iter().find(|row| row.id == id).unwrap();
        let code = row.code.clone().unwrap();
        assert_eq!(adr_binding_problems(row, &code), [] as [String; 0], "{id}");
        let adoption = code
            .blocks
            .iter()
            .find(|block| block.kind == BlockKind::Adopt)
            .cloned()
            .unwrap();
        let mut none = code.clone();
        none.blocks.retain(|block| block.kind != BlockKind::Adopt);
        let mut two = code.clone();
        two.blocks.push(adoption);
        for (case, planted, held) in [("none", none, 0), ("two", two, 2)] {
            let found = adr_binding_problems(row, &planted);
            let expected =
                format!("{id}: holds {held} adoptions; the ADR's row adopts one descriptor");
            assert!(found.contains(&expected), "{id}, {case}: {found:?}");
        }
    }
}

#[test]
fn linux_abi_headers_match_the_adr_in_order_and_case() {
    let mut inventory = parse(&inventory_text()).unwrap();
    let index = inventory
        .rows
        .iter()
        .position(|row| row.id == "linux-random")
        .unwrap();
    let headers = inventory.rows[index].code.as_ref().unwrap().headers.clone();
    assert!(headers.iter().any(|header| header == "sys/syscall.h"));
    let adr = adr_text();
    assert_eq!(problems(&inventory, &adr), [] as [String; 0]);

    let mut missing = headers.clone();
    missing.retain(|header| header != "sys/syscall.h");
    let mut extra = headers.clone();
    extra.push("extra.h".to_owned());
    let mut reordered = headers.clone();
    reordered.reverse();
    let case_changed = headers
        .iter()
        .map(|header| header.replace("syscall.h", "Syscall.h"))
        .collect();
    for changed in [missing, extra, reordered, case_changed] {
        inventory.rows[index].code.as_mut().unwrap().headers = changed.clone();
        let found = problems(&inventory, &adr);
        assert!(
            found.iter().any(|problem| {
                problem.contains("row 8 (linux-random)")
                    && problem.contains("Linux ABI headers")
                    && problem.contains("sys/syscall.h")
            }),
            "{changed:?}: {found:?}"
        );
    }
}

fn with_adr_headers(adr: &str, site: &str, cell: &str) -> String {
    let row = adr
        .lines()
        .find(|line| line.starts_with("| ") && line.contains(&format!(" | `{site}` | ")))
        .unwrap();
    let (rest, _) = row.strip_suffix(" |").unwrap().rsplit_once(" | ").unwrap();
    adr.replacen(row, &format!("{rest} | {cell} |"), 1)
}

#[test]
fn adr_header_cells_require_the_exact_code_span_list_syntax() {
    let inventory = parse(&inventory_text()).unwrap();
    let adr = adr_text();
    for cell in [
        "",
        "none",
        "time.h",
        "``",
        "`time.h`,",
        "`time.h`, ",
        "`time.h`,`sys/types.h`",
        "`time.h`,  `sys/types.h`",
        "`/time.h`",
        "`sys//time.h`",
        "`../time.h`",
        "`sys/.time.h`",
        "`sys/time.h/`",
        "`sys/time.H`",
        "`tíme.h`",
        "`sys\\time.h`",
    ] {
        let changed = with_adr_headers(&adr, "linux-clock", cell);
        let found = problems(&inventory, &changed);
        assert!(
            found
                .iter()
                .any(|problem| problem.starts_with("ADR row 1 (linux-clock) Linux ABI headers:")),
            "{cell:?}: {found:?}"
        );
    }
    let valid = ["_linux-v2.1/abi-types.h".to_owned()];
    assert_eq!(
        super::parse_headers_cell("`_linux-v2.1/abi-types.h`"),
        Ok(valid.to_vec())
    );
}

#[test]
fn both_header_lists_are_validated_independently() {
    let adr = adr_text();
    for (headers, diagnostic) in [
        (vec!["time.h", "time.h"], "duplicate header"),
        (vec!["../time.h"], "invalid header name"),
    ] {
        let cell = headers
            .iter()
            .map(|header| format!("`{header}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let changed_adr = with_adr_headers(&adr, "linux-clock", &cell);
        let mut inventory = parse(&inventory_text()).unwrap();
        let mut changed_inventory = inventory.clone();
        changed_inventory.rows[0].code.as_mut().unwrap().headers =
            headers.into_iter().map(str::to_owned).collect();

        let originals = problems(&inventory, &adr);
        let only_adr = problems(&inventory, &changed_adr);
        let only_mirror = problems(&changed_inventory, &adr);
        let both = problems(&changed_inventory, &changed_adr);
        assert_eq!(originals, [] as [String; 0]);
        assert_eq!(only_adr.len(), 1);
        assert_eq!(only_mirror.len(), 1);
        assert!(only_adr[0].starts_with("ADR row 1 (linux-clock) Linux ABI headers:"));
        assert!(only_mirror[0].starts_with("sites.json row 1 (linux-clock) code.headers:"));
        assert!(only_adr[0].contains(diagnostic));
        assert!(only_mirror[0].contains(diagnostic));
        assert_eq!(both, [only_mirror[0].clone(), only_adr[0].clone()]);

        inventory.rows[0].columns[2] = "a different library".to_owned();
        assert!(problems(&inventory, &adr)[0].contains("the copy keeps the table's text"));
    }
}

#[test]
fn none_means_an_empty_mirror_even_for_a_row_without_code() {
    let mut inventory = parse(&inventory_text()).unwrap();
    let adr = adr_text();
    let route = inventory
        .rows
        .iter()
        .find(|row| row.id == "linux-rtnetlink")
        .unwrap();
    assert!(route.code.is_none());
    let wipe = inventory
        .rows
        .iter_mut()
        .find(|row| row.id == "mem-wipe")
        .unwrap();
    let headers = &mut wipe.code.as_mut().unwrap().headers;
    assert_eq!(*headers, [] as [String; 0]);
    assert_eq!(problems(&inventory, &adr), [] as [String; 0]);

    let changed_adr = with_adr_headers(&adr, "linux-rtnetlink", "`linux/rtnetlink.h`");
    let found = problems(&inventory, &changed_adr);
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("row 23 (linux-rtnetlink)"));
    assert!(found[0].contains("expected [\"linux/rtnetlink.h\"], found []"));

    let wipe = inventory
        .rows
        .iter_mut()
        .find(|row| row.id == "mem-wipe")
        .unwrap();
    wipe.code
        .as_mut()
        .unwrap()
        .headers
        .push("string.h".to_owned());
    let found = problems(&inventory, &adr);
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("row 24 (mem-wipe)"));
    assert!(found[0].contains("expected [], found [\"string.h\"]"));

    let changed_adr = with_adr_headers(&adr, "mem-wipe", "`string.h`");
    let found = problems(&inventory, &changed_adr);
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("row 24 (mem-wipe) Linux ABI headers: expected `none`"));
}

#[test]
fn matching_headers_cannot_extend_a_non_linux_adr_target_scope() {
    let mut inventory = parse(&inventory_text()).unwrap();
    let row = inventory
        .rows
        .iter_mut()
        .find(|row| row.id == "unix-nodump")
        .unwrap();
    assert_eq!(row.columns[0], "Linux, macOS");
    assert_eq!(
        row.code.as_ref().unwrap().headers,
        ["sys/resource.h", "sys/prctl.h"]
    );
    row.columns[0] = "macOS".to_owned();
    let adr = adr_text().replace(" | Linux, macOS | ", " | macOS | ");
    let found = problems(&inventory, &adr);
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("row 11 (unix-nodump) Linux ABI headers: expected `none`"));
}
