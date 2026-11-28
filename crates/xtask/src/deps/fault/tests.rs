// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the fault feature's rules.

use std::path::PathBuf;

use super::{enabling, violations};
use crate::deps::Violation;
use crate::metadata::{Declaration, Package, Workspace};

const HOME_ID: &str = "path+file:///w/crates/ironafs-disk#0.0.0";

/// A package. A declaration of `ironafs-disk` is one of the home, edge or
/// no edge: the package carries no edges.
fn package(
    name: &str,
    member: bool,
    features: &[(&str, &[&str])],
    declarations: Vec<Declaration>,
) -> Package {
    Package {
        id: if name == "ironafs-disk" {
            HOME_ID.into()
        } else {
            format!("id {name}")
        },
        name: name.into(),
        path: member.then(|| PathBuf::from(format!("/w/crates/{name}"))),
        member,
        dependencies: Vec::new(),
        dev_dependencies: Vec::new(),
        target_kinds: vec!["lib".into()],
        links: None,
        features: features
            .iter()
            .map(|(name, enables)| {
                (
                    (*name).into(),
                    enables.iter().map(|enabled| (*enabled).into()).collect(),
                )
            })
            .collect(),
        declarations,
    }
}

fn home(features: &[(&str, &[&str])]) -> Package {
    package("ironafs-disk", true, features, Vec::new())
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

fn workspace(packages: Vec<Package>) -> Workspace {
    Workspace {
        root: PathBuf::from("/w"),
        target_dir: PathBuf::from("/w/target"),
        members: Vec::new(),
        packages,
    }
}

fn requested(from: &str, feature: &str) -> Violation {
    Violation::FaultRequested {
        from: from.into(),
        feature: feature.into(),
    }
}

fn forwarded(from: &str, feature: &str, enables: &str) -> Violation {
    Violation::FaultForwarded {
        from: from.into(),
        feature: feature.into(),
        enables: enables.into(),
    }
}

#[test]
fn enabling_follows_the_home_s_own_features_and_survives_a_cycle() {
    let home = home(&[
        ("fault", &[]),
        ("a", &["b"]),
        ("b", &["a", "fault"]),
        ("c", &["c"]),
    ]);
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
    let xtask = package(
        "xtask",
        true,
        &[],
        vec![on_disk("ironafs-disk", false, &["fault"])],
    );
    assert_eq!(violations(&workspace(vec![xtask])), []);
}

#[test]
fn violations_tell_a_request_in_a_feature_list_from_forwarding() {
    // xtask's plain request passes; the same request through an optional
    // dependency its own feature turns on is forwarding.
    let allowed = package(
        "xtask",
        true,
        &[],
        vec![on_disk("ironafs-disk", false, &["extra"])],
    );
    let disk = home(&[("fault", &[]), ("extra", &["fault"])]);
    assert_eq!(violations(&workspace(vec![disk, allowed])), []);

    let optional = package(
        "xtask",
        true,
        &[("go", &["dep:d"])],
        vec![on_disk("d", true, &["fault"])],
    );
    assert_eq!(
        violations(&workspace(vec![home(&[("fault", &[])]), optional])),
        [forwarded("xtask", "go", "dep:d")]
    );
}

#[test]
fn violations_count_a_strong_feature_path_that_turns_on_an_asking_optional_dependency() {
    // `d/harmless` turns `d` on with the features its declaration asks for,
    // `fault` among them; the weak `d?/harmless` does not turn it on.
    let xtask = package(
        "xtask",
        true,
        &[("strong", &["d/harmless"]), ("weak", &["d?/harmless"])],
        vec![on_disk("d", true, &["fault"])],
    );
    let disk = home(&[("fault", &[]), ("harmless", &[])]);
    assert_eq!(
        violations(&workspace(vec![disk, xtask])),
        [forwarded("xtask", "strong", "d/harmless")]
    );
}

#[test]
fn violations_refuse_a_request_from_any_package_but_xtask() {
    let other = package(
        "ironafsd",
        true,
        &[("weak", &["disk?/fault"])],
        vec![on_disk("disk", false, &["fault"])],
    );
    assert_eq!(
        violations(&workspace(vec![home(&[("fault", &[])]), other])),
        [
            requested("ironafsd", "fault"),
            forwarded("ironafsd", "weak", "disk?/fault"),
        ]
    );
}

#[test]
fn violations_refuse_a_request_on_an_optional_dependency_besides_its_forwarding() {
    let other = package(
        "ironafsd",
        true,
        &[("go", &["dep:disk"])],
        vec![on_disk("disk", true, &["fault"])],
    );
    assert_eq!(
        violations(&workspace(vec![home(&[("fault", &[])]), other])),
        [
            requested("ironafsd", "fault"),
            forwarded("ironafsd", "go", "dep:disk"),
        ]
    );
}

#[test]
fn violations_refuse_a_request_from_a_package_outside_the_workspace() {
    // A third-party package whose dependency resolves to the home, by a
    // patch for one, asks for it; so does one named xtask from a registry.
    let helper = package(
        "helper",
        false,
        &[],
        vec![on_disk("ironafs-disk", false, &["fault"])],
    );
    let registry_xtask = package(
        "xtask",
        false,
        &[],
        vec![on_disk("ironafs-disk", false, &["fault"])],
    );
    assert_eq!(
        violations(&workspace(vec![
            home(&[("fault", &[])]),
            helper,
            registry_xtask
        ])),
        [requested("helper", "fault"), requested("xtask", "fault")]
    );
}

#[test]
fn violations_read_a_declaration_of_the_home_s_name_as_one_of_the_home() {
    // Whatever package Cargo would take it to, a declaration of
    // `ironafs-disk` is one of the workspace crate: its `extra` turns
    // `fault` on, so the request is refused. The namesake itself is
    // refused by the name rule.
    let canary = package(
        "ironafs-bans-canary",
        true,
        &[],
        vec![on_disk("ironafs-disk", false, &["extra"])],
    );
    let disk = home(&[("fault", &[]), ("extra", &["fault"])]);
    assert_eq!(
        violations(&workspace(vec![disk, canary])),
        [requested("ironafs-bans-canary", "extra")]
    );
}

#[test]
fn violations_find_the_home_s_own_rules() {
    assert_eq!(
        violations(&workspace(vec![home(&[("other", &[])])])),
        [Violation::FaultUndeclared("ironafs-disk".into())]
    );
    let disk = home(&[
        ("default", &["extra"]),
        ("extra", &["fault"]),
        ("fault", &[]),
    ]);
    assert_eq!(
        violations(&workspace(vec![disk])),
        [Violation::FaultByDefault {
            name: "ironafs-disk".into(),
            via: vec!["extra".into()],
        }]
    );
}
