// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the fault feature's rules.

use std::path::PathBuf;

use super::{enabling, violations};
use crate::deps::Violation;
use crate::metadata::{Declaration, Member, Workspace};

fn member(name: &str, features: &[(&str, &[&str])], declarations: Vec<Declaration>) -> Member {
    Member {
        name: name.into(),
        id: name.into(),
        dir: PathBuf::from(format!("/w/crates/{name}")),
        lib_root: None,
        build_script: None,
        targets: Vec::new(),
        target_kinds: vec!["lib".into()],
        editions: vec!["2024".into()],
        declarations,
        features: features
            .iter()
            .map(|(name, enables)| {
                (
                    (*name).into(),
                    enables.iter().map(|enabled| (*enabled).into()).collect(),
                )
            })
            .collect(),
        renamed: Vec::new(),
    }
}

fn on_disk(name: &str, optional: bool, features: &[&str]) -> Declaration {
    Declaration {
        kind: "normal".into(),
        package: "ironafs-disk".into(),
        name: name.into(),
        optional,
        features: features.iter().map(|feature| (*feature).into()).collect(),
    }
}

fn workspace(members: Vec<Member>) -> Workspace {
    Workspace {
        root: PathBuf::from("/w"),
        target_dir: PathBuf::from("/w/target"),
        members,
        packages: Vec::new(),
    }
}

#[test]
fn enabling_follows_the_home_s_own_features_and_survives_a_cycle() {
    let home = member(
        "ironafs-disk",
        &[
            ("fault", &[]),
            ("a", &["b"]),
            ("b", &["a", "fault"]),
            ("c", &["c"]),
        ],
        Vec::new(),
    );
    let mut found = enabling(&home);
    found.sort();
    assert_eq!(
        found,
        [
            ("a", vec!["b".to_owned()]),
            ("b", Vec::new()),
            ("fault", Vec::new()),
        ]
    );
}

#[test]
fn violations_need_no_home_to_pass() {
    let xtask = member(
        "xtask",
        &[],
        vec![on_disk("ironafs-disk", false, &["fault"])],
    );
    assert_eq!(violations(&workspace(vec![xtask])), []);
}

#[test]
fn violations_tell_a_request_in_a_feature_list_from_forwarding() {
    let home = member(
        "ironafs-disk",
        &[("fault", &[]), ("extra", &["fault"])],
        Vec::new(),
    );
    // xtask's plain request passes; the same request through an optional
    // dependency its own feature turns on is forwarding.
    let allowed = member(
        "xtask",
        &[],
        vec![on_disk("ironafs-disk", false, &["extra"])],
    );
    assert_eq!(violations(&workspace(vec![home, allowed])), []);

    let home = member("ironafs-disk", &[("fault", &[])], Vec::new());
    let optional = member(
        "xtask",
        &[("go", &["dep:d"]), ("other", &["d/unrelated"])],
        vec![on_disk("d", true, &["fault"])],
    );
    assert_eq!(
        violations(&workspace(vec![home, optional])),
        [Violation::FaultForwarded {
            from: "xtask".into(),
            feature: "go".into(),
            enables: "dep:d".into(),
        }]
    );
}

#[test]
fn violations_refuse_a_request_from_any_crate_but_xtask() {
    let home = member("ironafs-disk", &[("fault", &[])], Vec::new());
    let other = member(
        "ironafsd",
        &[("weak", &["disk?/fault"])],
        vec![on_disk("disk", false, &["fault"])],
    );
    assert_eq!(
        violations(&workspace(vec![home, other])),
        [
            Violation::FaultRequested {
                from: "ironafsd".into(),
                feature: "fault".into(),
            },
            Violation::FaultForwarded {
                from: "ironafsd".into(),
                feature: "weak".into(),
                enables: "disk?/fault".into(),
            },
        ]
    );
}

#[test]
fn violations_find_the_home_s_own_rules() {
    let home = member("ironafs-disk", &[("other", &[])], Vec::new());
    assert_eq!(
        violations(&workspace(vec![home])),
        [Violation::FaultUndeclared("ironafs-disk".into())]
    );
    let home = member(
        "ironafs-disk",
        &[
            ("default", &["extra"]),
            ("extra", &["fault"]),
            ("fault", &[]),
        ],
        Vec::new(),
    );
    assert_eq!(
        violations(&workspace(vec![home])),
        [Violation::FaultByDefault {
            name: "ironafs-disk".into(),
            via: vec!["extra".into()],
        }]
    );
}
