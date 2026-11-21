// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for seed selection, verdicts and the counterexample report.

use super::{Counterexample, Verdict, parse_seed, resolve_seed, seed_from_name};

#[test]
fn parse_seed_accepts_decimal_and_hex() {
    assert_eq!(parse_seed("1234"), Some(1234));
    assert_eq!(parse_seed("0x00ff"), Some(255));
    assert_eq!(parse_seed("0XFF"), Some(255));
    assert_eq!(parse_seed(" 7 "), Some(7));
    assert_eq!(parse_seed("0xffffffffffffffff"), Some(u64::MAX));
}

#[test]
fn parse_seed_rejects_malformed_text() {
    for text in ["", "0x", "-1", "12a", "0x1_0", "18446744073709551616"] {
        assert_eq!(parse_seed(text), None, "{text:?}");
    }
}

#[test]
fn resolve_seed_prefers_environment_then_explicit_then_name() {
    assert_eq!(resolve_seed(Some("0x10"), Some(5), "name"), Ok(16));
    assert_eq!(resolve_seed(None, Some(5), "name"), Ok(5));
    assert_eq!(resolve_seed(None, None, "name"), Ok(seed_from_name("name")));
}

#[test]
fn resolve_seed_rejects_malformed_environment() {
    assert_eq!(
        resolve_seed(Some("seed"), Some(5), "name"),
        Err(
            "IRONAFS_TEST_SEED is not a decimal or 0x-prefixed hexadecimal u64: \"seed\""
                .to_owned()
        )
    );
}

#[test]
fn seed_from_name_is_stable_and_distinguishes_names() {
    assert_eq!(seed_from_name("decode"), seed_from_name("decode"));
    assert_ne!(seed_from_name("decode"), seed_from_name("encode"));
    assert_ne!(seed_from_name(""), seed_from_name("\0"));
}

#[test]
fn verdict_of_unit_is_pass() {
    assert_eq!(().failure(), None);
}

#[test]
fn verdict_of_result_reports_error_debug() {
    assert_eq!(Ok::<(), &str>(()).failure(), None);
    assert_eq!(
        Err::<(), _>("bad length").failure(),
        Some("\"bad length\"".to_owned())
    );
}

fn sample(shrink_budget_exhausted: bool) -> Counterexample<Vec<u8>> {
    Counterexample {
        name: "decode rejects truncated input".to_owned(),
        seed: 0x1234,
        case: 3,
        cases: 256,
        value: vec![100],
        message: "assertion failed: ok".to_owned(),
        shrink_runs: 57,
        shrink_budget_exhausted,
    }
}

#[test]
fn counterexample_display_names_seed_and_replay() {
    assert_eq!(
        sample(false).to_string(),
        "property `decode rejects truncated input` failed on case 4 of 256 \
         (seed 0x0000000000001234)\n\
         minimal input: [100]\n\
         failure: assertion failed: ok\n\
         shrunk in 57 runs\n\
         replay with IRONAFS_TEST_SEED=0x0000000000001234"
    );
}

#[test]
fn counterexample_display_warns_when_budget_ran_out() {
    let report = sample(true).to_string();
    assert!(
        report.contains(
            "shrunk in 57 runs; the shrink budget ran out, so the input may not be minimal\n"
        ),
        "{report}"
    );
}
