// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the reader of nextest's machine-readable test list.

use super::parse;

const LIST: &str = r#"{
  "rust-build-meta": {
    "target-directory": "/work/target",
    "target-platforms": [{"triple": "x86_64-unknown-linux-gnu", "target-features": "unknown"}],
    "target-platform": null
  },
  "test-count": 3,
  "rust-suites": {
    "demo": {
      "package-name": "demo",
      "binary-id": "demo",
      "kind": "lib",
      "status": "listed",
      "testcases": {
        "a": {"kind": "test", "ignored": false, "filter-match": {"status": "matches"}},
        "q": {"kind": "test", "ignored": true, "filter-match": {"status": "matches"}}
      }
    },
    "demo::bin/tool": {
      "package-name": "demo",
      "binary-id": "demo::bin/tool",
      "status": "listed",
      "testcases": {}
    },
    "other::it": {
      "package-name": "other",
      "binary-id": "other::it",
      "status": "listed",
      "testcases": {
        "x::y": {"kind": "test", "ignored": false, "filter-match": {"status": "mismatch", "reason": "ignored"}}
      }
    }
  }
}"#;

#[test]
fn parse_reads_target_and_every_listed_test() {
    let (target, target_dir, listed) = parse(LIST).unwrap();

    assert_eq!(target, "x86_64-unknown-linux-gnu");
    assert_eq!(target_dir, std::path::Path::new("/work/target"));
    let names: Vec<_> = listed
        .iter()
        .map(|test| format!("{} {} {}", test.package, test.binary, test.name))
        .collect();
    assert_eq!(
        names,
        ["demo demo a", "demo demo q", "other other::it x::y"]
    );
}

#[test]
fn parse_rejects_a_list_it_cannot_trust() {
    for broken in [
        LIST.replace("\"test-count\": 3", "\"test-count\": 4"),
        LIST.replace(
            r#"[{"triple": "x86_64-unknown-linux-gnu", "target-features": "unknown"}]"#,
            "[]",
        ),
        LIST.replace("\"package-name\": \"other\",", ""),
        LIST.replace("rust-suites", "suites"),
        LIST.replace("target-directory", "target-dir"),
        "not json".to_owned(),
    ] {
        assert!(parse(&broken).is_err(), "{broken}");
    }
}
