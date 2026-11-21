// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the `JUnit` report reader.

use super::{Case, parse};

fn case(binary: &str, name: &str, passed: bool) -> Case {
    Case {
        binary: binary.to_owned(),
        name: name.to_owned(),
        passed,
    }
}

#[test]
fn parse_reads_passed_and_failed_cases_by_suite() {
    let report = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="3">
    <testsuite name="xtask" tests="2">
        <testcase name="a::ok" classname="xtask" time="0.1"/>
        <testcase name="a::bad" classname="xtask">
            <failure type="test failure">assertion &lt;failed&gt;</failure>
            <system-out><![CDATA[seed 1 <testcase name="fake"/>]]></system-out>
        </testcase>
    </testsuite>
    <!-- a comment <testcase name="hidden"/> -->
    <testsuite name="ironafs-cli::bin/afs" tests="1">
        <testcase name="b::with_output"><system-out>out</system-out><system-err>err</system-err></testcase>
    </testsuite>
</testsuites>
"#;

    assert_eq!(
        parse(report).unwrap(),
        [
            case("xtask", "a::ok", true),
            case("xtask", "a::bad", false),
            case("ironafs-cli::bin/afs", "b::with_output", true),
        ]
    );
}

#[test]
fn parse_counts_any_other_child_of_a_case_as_not_passed() {
    for child in [
        "<error/>",
        "<skipped/>",
        "<flakyFailure/>",
        "<rerunFailure>x</rerunFailure>",
    ] {
        let report = format!(
            "<testsuites><testsuite name=\"x\"><testcase name=\"t\">{child}</testcase></testsuite></testsuites>"
        );
        assert_eq!(parse(&report).unwrap(), [case("x", "t", false)], "{child}");
    }
}

#[test]
fn parse_decodes_entities_in_names() {
    let report = "<testsuites><testsuite name='a&amp;b'>\
                  <testcase name=\"q&quot;&apos;&lt;&gt;&#65;&#x42;\"/></testsuite></testsuites>";

    assert_eq!(parse(report).unwrap(), [case("a&b", "q\"'<>AB", true)]);
}

#[test]
fn parse_rejects_malformed_reports() {
    for report in [
        "",
        "<testsuites>",
        "<testsuites><testsuite name=\"x\"></testsuites>",
        "<testsuites><testcase name=\"t\"/></testsuites>",
        "<testsuites><testsuite><testcase name=\"t\"/></testsuite></testsuites>",
        "<testsuites><testsuite name=\"x\"><testcase/></testsuite></testsuites>",
        "<testsuites><testsuite name=\"x\"><testcase name=\"t&bogus;\"/></testsuite></testsuites>",
        "<testsuites><testsuite name=\"x\"><testcase name=t/></testsuite></testsuites>",
        "<other/>",
        "<testsuites/><testsuites/>",
        "<testsuites><!-- unterminated </testsuites>",
    ] {
        assert!(parse(report).is_err(), "{report:?}");
    }
}
