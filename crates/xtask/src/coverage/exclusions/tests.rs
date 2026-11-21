// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the exclusion list, its file patterns and the test-file rule.

use std::path::Path;

use super::{Pattern, ignore_regex, is_test_file, parse_list};

fn pattern(text: &str) -> Pattern {
    Pattern::parse(text).unwrap()
}

#[test]
fn star_matches_within_one_segment_only() {
    let star = pattern("crates/a/src/*.rs");

    assert!(star.matches("crates/a/src/lib.rs"));
    assert!(!star.matches("crates/a/src/deep/lib.rs"));
    assert!(!star.matches("crates/a/src/lib.rsx"));
}

#[test]
fn double_star_matches_any_number_of_segments() {
    let middle = pattern("crates/a/**/gen.rs");
    assert!(middle.matches("crates/a/gen.rs"));
    assert!(middle.matches("crates/a/src/x/y/gen.rs"));
    assert!(!middle.matches("crates/b/gen.rs"));

    let leading = pattern("**/gen.rs");
    assert!(leading.matches("gen.rs"));
    assert!(leading.matches("crates/a/src/gen.rs"));

    let trailing = pattern("crates/a/src/generated/**");
    assert!(trailing.matches("crates/a/src/generated/x.rs"));
    assert!(trailing.matches("crates/a/src/generated/x/y.rs"));
    assert!(!trailing.matches("crates/a/src/other.rs"));
}

#[test]
fn star_inside_a_name_matches_part_of_it() {
    let partial = pattern("crates/a/src/gen_*_codec.rs");
    assert!(partial.matches("crates/a/src/gen_fs_codec.rs"));
    assert!(partial.matches("crates/a/src/gen__codec.rs"));
    assert!(!partial.matches("crates/a/src/gen_fs_codec.rs.bak"));
}

#[test]
fn parse_refuses_unsupported_patterns() {
    for text in [
        "",
        "/crates/a.rs",
        "crates/../a.rs",
        "crates/a?.rs",
        "crates/[ab].rs",
        "crates/a b.rs",
        "crates//a.rs",
        "crates/a**.rs",
        "crates/a/",
    ] {
        assert!(Pattern::parse(text).is_err(), "accepted {text:?}");
    }
}

#[test]
fn list_reads_patterns_with_reasons_and_skips_comments() {
    let (list, violations) = parse_list(
        "# SPDX header\n\n# a comment\ncrates/a/src/gen/** # generated codec, table 2\n",
    );

    assert_eq!(violations, Vec::<String>::new());
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].reason, "generated codec, table 2");
    assert!(list[0].pattern.matches("crates/a/src/gen/x.rs"));
}

#[test]
fn list_refuses_an_entry_without_a_reason() {
    for text in [
        "crates/a/src/x.rs\n",
        "crates/a/src/x.rs #\n",
        "crates/a/src/x.rs #   \n",
    ] {
        let (list, violations) = parse_list(text);

        assert!(list.is_empty());
        assert_eq!(violations.len(), 1, "{text:?}");
        assert!(violations[0].contains("line 1"), "{violations:?}");
        assert!(violations[0].contains("no reason"), "{violations:?}");
    }
}

#[test]
fn list_refuses_a_bad_pattern_with_its_line() {
    let (_, violations) = parse_list("# header\ncrates/a?.rs # why\n");

    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("line 2"), "{violations:?}");
}

#[test]
fn ignore_regex_anchors_each_pattern_at_the_root() {
    let (list, _) = parse_list("crates/a/src/gen/** # r\ncrates/b/src/*.rs # r\n");

    let regex = ignore_regex(Path::new("/w/ironafs"), &list).unwrap();

    assert_eq!(
        regex.as_deref(),
        Some("^/w/ironafs/(crates/a/src/gen/.*|crates/b/src/[^/]*[.]rs)$")
    );
}

#[test]
fn ignore_regex_is_none_for_an_empty_list() {
    assert_eq!(ignore_regex(Path::new("/w"), &[]).unwrap(), None);
}

#[test]
fn ignore_regex_escapes_the_root_and_refuses_unescapable_roots() {
    let (list, _) = parse_list("a.rs # r\n");

    assert_eq!(
        ignore_regex(Path::new("/w/x.y+z"), &list)
            .unwrap()
            .as_deref(),
        Some("^/w/x[.]y[+]z/(a[.]rs)$")
    );
    for root in ["/w/[x]", "/w/a\\b", "/w/^x", "relative"] {
        assert!(ignore_regex(Path::new(root), &list).is_err(), "{root}");
    }
}

#[test]
fn double_star_regex_covers_leading_and_middle_segments() {
    let (list, _) = parse_list("**/gen.rs # r\ncrates/**/x.rs # r\n");

    assert_eq!(
        ignore_regex(Path::new("/w"), &list).unwrap().as_deref(),
        Some("^/w/((.*/)?gen[.]rs|crates/(.*/)?x[.]rs)$")
    );
}

#[test]
fn test_files_follow_the_tools_default_rule() {
    for path in [
        "crates/a/tests/it/main.rs",
        "crates/a/src/foo/tests.rs",
        "crates/a/src/tests.rs",
        "crates/a/src/foo_tests.rs",
        "crates/a/src/foo-tests.rs",
        "crates/a/examples/x.rs",
        "crates/a/benches/b.rs",
        "tests/coverage/x.rs",
    ] {
        assert!(is_test_file(path), "{path}");
    }
    for path in [
        "crates/a/src/lib.rs",
        "crates/a/src/testsuite.rs",
        "crates/a/src/contests.rs",
        "crates/a/src/tests_helper.rs",
    ] {
        assert!(!is_test_file(path), "{path}");
    }
}

#[test]
fn matching_many_wildcards_against_a_long_name_takes_bounded_time() {
    let name = format!("crates/{}.rs", "a".repeat(4000));
    let wildcards = pattern("crates/*a*a*a*a*a*a*a*b.rs");
    let segments = pattern("**/**/**/**/**/**/x/**/y.rs");
    let path = format!("{}y.rs", "d/".repeat(2000));

    assert!(!wildcards.matches(&name));
    assert!(!segments.matches(&path));
}
