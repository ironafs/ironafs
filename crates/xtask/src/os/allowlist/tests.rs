// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the target-allowlist evaluator.

use super::{Cfg, accepts, guard, parse_cfg};
use crate::rust_source::tokens_with_strings;

fn cfg(lines: &str) -> Cfg {
    parse_cfg(lines)
}

const LINUX_GNU: &str = "panic=\"unwind\"\ntarget_abi=\"\"\ntarget_arch=\"x86_64\"\n\
                         target_endian=\"little\"\ntarget_env=\"gnu\"\ntarget_os=\"linux\"\n\
                         target_pointer_width=\"64\"\ntarget_vendor=\"unknown\"\nunix\n";

const ROOT: &str = "#[cfg(not(all(target_pointer_width = \"64\", any(all(target_os = \"linux\", \
                    target_vendor = \"unknown\", any(target_env = \"gnu\", target_env = \"musl\")), \
                    all(target_os = \"macos\")))))]\ncompile_error!(\"no\");\n";

#[test]
fn guard_reads_the_allowlist_and_accepts_only_matching_cfg_sets() {
    let expression = guard(&tokens_with_strings(ROOT).unwrap()).unwrap();
    assert!(accepts(&expression, &cfg(LINUX_GNU)));
    assert!(!accepts(
        &expression,
        &cfg(&LINUX_GNU.replace("target_pointer_width=\"64\"", "target_pointer_width=\"32\""))
    ));
    assert!(!accepts(
        &expression,
        &cfg(&LINUX_GNU.replace("target_vendor=\"unknown\"", "target_vendor=\"unikraft\""))
    ));
    assert!(accepts(
        &expression,
        &cfg(&LINUX_GNU.replace("target_os=\"linux\"", "target_os=\"macos\""))
    ));
}

#[test]
fn guard_refuses_a_root_with_no_allowlist_or_an_unknown_predicate() {
    assert!(guard(&tokens_with_strings("fn main() {}").unwrap()).is_err());
    let odd = ROOT.replace("any(target_env", "one_of(target_env");
    assert!(guard(&tokens_with_strings(&odd).unwrap()).is_err());
}

#[test]
fn parse_cfg_reads_names_and_key_value_pairs() {
    let found = cfg("unix\ntarget_os=\"linux\"\ntarget_feature=\"sse\"\ntarget_feature=\"sse2\"\n");
    assert!(found.contains(&("unix".to_owned(), None)));
    assert!(found.contains(&("target_feature".to_owned(), Some("sse2".to_owned()))));
    assert_eq!(found.len(), 4);
}

#[test]
fn guard_refuses_a_guard_another_attribute_could_switch_off() {
    for source in [
        format!("#[cfg(any())]\n{ROOT}"),
        format!("mod inner {{\n{ROOT}}}\n"),
        format!("fn f() {{}}\nconst A: u8 = 1 + {ROOT}"),
    ] {
        assert!(
            guard(&tokens_with_strings(&source).unwrap()).is_err(),
            "{source}"
        );
    }
    let with_inner = format!("#![no_std]\n//! Docs.\n{ROOT}");
    assert!(guard(&tokens_with_strings(&with_inner).unwrap()).is_ok());
}
