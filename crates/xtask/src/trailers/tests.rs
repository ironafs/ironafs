// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for trailer blocks, rule diagnostics and landed commit messages.

use super::{Rules, Violation, run_pr, validate};
use crate::Error;

const EDITORIAL: &str = "Class: E\nAssisted-by: none";
const CONTRIBUTOR: &str = "Signed-off-by: Zoë Example <zoe+dev@example.org>";
const MAINTAINER: &str = "Signed-off-by: Adrian Ana <adrian@adrianana.com>";
const REVIEW: &str = "AI-Reviewed-by: review-tool:Model_1.2";

#[track_caller]
fn assert_violation(message: &str, rules: Rules, line: usize, expected_fix: &str) {
    let violation = validate(message, rules).unwrap_err();
    assert_eq!(violation.line, line, "{violation}");
    assert!(violation.fix.contains(expected_fix), "{violation}");
}

#[test]
fn trailers_requires_a_final_trailer_paragraph() {
    for message in ["", " \r\n\n"] {
        assert_violation(message, Rules::MaintainerPr, 1, "trailer block");
    }
    assert_violation("No trailers here.", Rules::MaintainerPr, 1, "trailer block");
    assert_violation(
        "Title\n\nAssisted-by: none\n\nClass: E",
        Rules::MaintainerPr,
        5,
        "add an Assisted-by:",
    );
    assert_eq!(
        validate(
            "Subject\r\n\r\nClass: P\r\nAssisted-by: bogus?\r\n\r\nWritten by Claude Code (claude-opus-5-5) for the maintainer.\r\n\r\nClass: E\r\nAssisted-by: none\r\n \t\r\n",
            Rules::MaintainerPr,
        ),
        Ok(())
    );
}

#[test]
fn trailers_never_interprets_fenced_code_as_trailers() {
    for marker in ['`', '~'] {
        for width in 3..=8 {
            let fence = marker.to_string().repeat(width);
            let short = marker.to_string().repeat(width - 1);
            let example = format!("  {fence}text\n{short}\n{fence} extra\n\n{EDITORIAL}");
            assert_violation(&example, Rules::MaintainerPr, 5, "outside fenced code");
            assert_violation(
                &format!("{example}\n{fence}"),
                Rules::MaintainerPr,
                5,
                "outside fenced code",
            );
            assert_eq!(
                validate(
                    &format!("{example}\n{fence} \t\n\n{EDITORIAL}"),
                    Rules::MaintainerPr,
                ),
                Ok(())
            );
        }
    }
}

#[test]
fn trailers_never_interprets_quotes_as_trailers() {
    let quote = "> Class: E\n > Assisted-by: none";
    assert_violation(quote, Rules::MaintainerPr, 1, "outside quoted text");
    assert_violation(
        "Assisted-by: none\n> Class: E",
        Rules::MaintainerPr,
        2,
        "outside quoted text",
    );
    assert_eq!(
        validate(&format!("{quote}\n\n{EDITORIAL}"), Rules::MaintainerPr),
        Ok(())
    );
}

#[test]
fn trailers_rejects_unknown_keys_and_malformed_lines_in_the_block() {
    assert_eq!(
        validate(
            &format!("Subject\n\nOther-key: in the prose\n\n{EDITORIAL}"),
            Rules::MaintainerPr,
        ),
        Ok(())
    );
    assert_violation(
        &format!("{EDITORIAL}\nOther-key: value"),
        Rules::MaintainerPr,
        3,
        "unknown trailer key",
    );
    assert_violation(
        &format!("{EDITORIAL}\ntrailing prose"),
        Rules::MaintainerPr,
        3,
        "key and a colon",
    );
}

#[test]
fn pr_requires_exactly_one_recognized_class() {
    for class in ['E', 'I', 'P', 'S', 'R'] {
        assert_eq!(
            validate(
                &format!("Class: {class}\nAssisted-by: none"),
                Rules::MaintainerPr,
            ),
            Ok(())
        );
    }
    assert_violation(
        "Assisted-by: none",
        Rules::MaintainerPr,
        1,
        "add one Class:",
    );
    assert_violation(
        &format!("{EDITORIAL}\nClass: E"),
        Rules::MaintainerPr,
        3,
        "duplicate Class:",
    );
    for invalid in ["", "e", "A", "E I"] {
        assert_violation(
            &format!("Class: {invalid}\nAssisted-by: none"),
            Rules::MaintainerPr,
            1,
            "Class: value to E, I, P, S or R",
        );
    }
}

#[test]
fn trailers_requires_assistance_with_exclusive_none_or_valid_ids() {
    assert_eq!(validate("Assisted-by: none", Rules::Commit), Ok(()));
    assert_eq!(
        validate(
            "Assisted-by: A_z.0-9:model_v2.1\nAssisted-by: other:service",
            Rules::Commit,
        ),
        Ok(())
    );
    for invalid in [
        "", "agent", "agent:", ":model", "a:b:c", "a:b c", "a:é", "a/b:c",
    ] {
        assert_violation(
            &format!("Assisted-by: {invalid}"),
            Rules::Commit,
            1,
            "use Assisted-by: none or Assisted-by: <agent>:<model>",
        );
        assert_violation(
            &format!("{EDITORIAL}\nAI-Reviewed-by: {invalid}"),
            Rules::MaintainerPr,
            3,
            "use AI-Reviewed-by: <agent>:<model>",
        );
    }
    for (first, second) in [("none", "a:b"), ("a:b", "none"), ("none", "none")] {
        assert_violation(
            &format!("Assisted-by: {first}\nAssisted-by: {second}"),
            Rules::Commit,
            2,
            "must be the only Assisted-by:",
        );
    }
}

#[test]
fn pr_validates_optional_review_trailers() {
    assert_eq!(
        validate("Class: S\nAssisted-by: none", Rules::MaintainerPr),
        Ok(())
    );
    assert_eq!(
        validate(
            &format!("{EDITORIAL}\n{REVIEW}\n{REVIEW}"),
            Rules::MaintainerPr
        ),
        Ok(())
    );
    assert_violation(
        &format!("{EDITORIAL}\nAI-Reviewed-by: none"),
        Rules::MaintainerPr,
        3,
        "use AI-Reviewed-by: <agent>:<model>",
    );
}

#[test]
fn trailers_accepts_human_credit_in_every_mode() {
    for (rules, required) in [
        (Rules::MaintainerPr, EDITORIAL.to_owned()),
        (Rules::ContributorPr, format!("{EDITORIAL}\n{CONTRIBUTOR}")),
        (
            Rules::Main,
            format!("Class: I\nAssisted-by: none\n{REVIEW}\n{MAINTAINER}"),
        ),
        (Rules::Commit, "Assisted-by: none".to_owned()),
    ] {
        assert_eq!(
            validate(
                &format!(
                    "{required}\nCo-authored-by: Zoë Example <zoe+dev@example.org>\nReviewed-by: Taylor Example <taylor@example.org>"
                ),
                rules,
            ),
            Ok(()),
            "{rules:?}"
        );
    }
}

#[test]
fn trailers_rejects_malformed_human_credit() {
    for key in ["Co-authored-by", "Reviewed-by"] {
        for (rules, required, line) in [
            (Rules::MaintainerPr, EDITORIAL.to_owned(), 3),
            (Rules::Main, format!("{EDITORIAL}\n{MAINTAINER}"), 4),
            (Rules::Commit, "Assisted-by: none".to_owned(), 2),
        ] {
            assert_violation(
                &format!("{required}\n{key}: Zoë Example <zoe>"),
                rules,
                line,
                &format!("use {key}: Name <email>"),
            );
        }
    }
}

#[test]
fn human_credit_does_not_satisfy_signoff_or_ai_review_requirements() {
    assert_violation(
        &format!("{EDITORIAL}\nCo-authored-by: Adrian Ana <adrian@adrianana.com>"),
        Rules::Main,
        1,
        "add a Signed-off-by: with a maintainer email address",
    );
    assert_violation(
        &format!(
            "Class: I\nAssisted-by: none\n{MAINTAINER}\nReviewed-by: Taylor Example <taylor@example.org>"
        ),
        Rules::Main,
        1,
        "add an AI-Reviewed-by:",
    );
}

#[test]
fn pr_requires_a_well_formed_signoff_for_outside_contributors() {
    assert_eq!(
        validate(&format!("{EDITORIAL}\n{CONTRIBUTOR}"), Rules::ContributorPr),
        Ok(())
    );
    assert_eq!(validate(EDITORIAL, Rules::MaintainerPr), Ok(()));
    assert_violation(EDITORIAL, Rules::ContributorPr, 1, "add a Signed-off-by:");
    for invalid in [
        "",
        "<dev@example.org>",
        "Name dev@example.org",
        "Name <dev>",
        "Name <@example.org>",
        "Name <dev@>",
        "Name <dev@@example.org>",
        "Name <de v@example.org>",
        "Name <dev@example..org>",
        "Name <dev@example.org> extra",
    ] {
        for rules in [Rules::ContributorPr, Rules::MaintainerPr] {
            assert_violation(
                &format!("{EDITORIAL}\nSigned-off-by: {invalid}"),
                rules,
                3,
                "use Signed-off-by: Name <email>",
            );
        }
    }
}

#[test]
fn pr_applies_author_rules_and_notices_the_dependabot_exception() {
    let mut output = Vec::new();
    run_pr(
        "dependabot[bot]",
        || panic!("dependabot's proposal should pass without reading the body"),
        &mut output,
    )
    .unwrap();
    assert_eq!(
        output,
        b"notice: the maintainer's squash message for this Dependabot proposal is checked on main\n"
    );
    output.clear();
    for author in ["AdrianIAna", "adrianiana"] {
        run_pr(author, || Ok(EDITORIAL.into()), &mut output).unwrap();
    }
    for (author, expected_fix) in [
        ("adrianiana2", "add a Signed-off-by:"),
        ("dependabot", "add a Signed-off-by:"),
        ("", "set PR_AUTHOR"),
    ] {
        let Err(Error::Trailer(violation)) = run_pr(author, || Ok(EDITORIAL.into()), &mut output)
        else {
            panic!("expected a pull request trailer violation for {author:?}");
        };
        assert_eq!(violation.line, 1);
        assert!(violation.fix.contains(expected_fix), "{violation}");
    }
    assert_eq!(output, [] as [u8; 0]);
}

#[test]
fn commit_requires_assistance_and_refuses_squash_trailers() {
    assert_eq!(
        validate("Title\n\nClass: P\n\nAssisted-by: none", Rules::Commit,),
        Ok(())
    );
    assert_violation("Title\n", Rules::Commit, 1, "trailer block");
    for (trailer, key) in [
        ("Class: E", "Class:"),
        (REVIEW, "AI-Reviewed-by:"),
        (CONTRIBUTOR, "Signed-off-by:"),
    ] {
        assert_violation(
            &format!("Assisted-by: none\n{trailer}"),
            Rules::Commit,
            2,
            &format!("remove {key} from the branch commit"),
        );
    }
}

#[test]
fn main_requires_a_maintainer_signoff_and_review_except_for_class_e() {
    assert_eq!(
        validate(&format!("{EDITORIAL}\n{MAINTAINER}"), Rules::Main),
        Ok(())
    );
    assert_violation(
        &format!("{EDITORIAL}\n{CONTRIBUTOR}"),
        Rules::Main,
        1,
        "add a Signed-off-by: with a maintainer email address",
    );
    assert_violation(
        EDITORIAL,
        Rules::Main,
        1,
        "add a Signed-off-by: with a maintainer email address",
    );
    for class in ['I', 'P', 'S', 'R'] {
        let message = format!("Title\n\nClass: {class}\nAssisted-by: a:b\n{MAINTAINER}");
        assert_violation(&message, Rules::Main, 3, "add an AI-Reviewed-by:");
        assert_eq!(
            validate(&format!("{message}\n{REVIEW}\n{CONTRIBUTOR}"), Rules::Main),
            Ok(())
        );
    }
}

#[test]
fn main_accepts_the_root_and_both_squash_messages() {
    for (revision, message) in [
        (
            "ce1cb25",
            include_str!("../../tests/fixtures/trailers/ce1cb25.txt"),
        ),
        (
            "792b63c",
            include_str!("../../tests/fixtures/trailers/792b63c.txt"),
        ),
        (
            "41c3ab1",
            include_str!("../../tests/fixtures/trailers/41c3ab1.txt"),
        ),
    ] {
        assert_eq!(validate(message, Rules::Main), Ok(()), "{revision}");
    }
}

#[test]
fn trailer_diagnostic_displays_the_line_and_fix() {
    let violation = Violation {
        line: 5,
        fix: "add an Assisted-by: trailer",
    };
    assert_eq!(violation.to_string(), "line 5: add an Assisted-by: trailer");
}
