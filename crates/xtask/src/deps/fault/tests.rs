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
        outside: None,
        feature: feature.into(),
    }
}

/// A request from a package outside the workspace, named by its id.
fn requested_outside(from: &str, feature: &str) -> Violation {
    Violation::FaultRequested {
        from: from.into(),
        outside: Some(format!("id {from}")),
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
    // With no workspace ironafs-disk, the fault rules have nothing to read;
    // a declaration of its name is refused by the name rule instead.
    let other = package(
        "ironafsd",
        true,
        &[("f", &["ironafs-disk/fault"])],
        vec![on_disk("ironafs-disk", false, &["fault"])],
    );
    assert_eq!(violations(&workspace(vec![other])), []);
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
        [
            requested_outside("helper", "fault"),
            requested_outside("xtask", "fault")
        ]
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

#[test]
fn violations_forward_through_any_feature_of_the_home_that_turns_fault_on() {
    // `extra` turns `fault` on: a strong or weak path to it, and an optional
    // dependency asking for it that a feature turns on, all forward it.
    let disk = home(&[("fault", &[]), ("extra", &["fault"])]);
    let shipped = package(
        "ironafsd",
        true,
        &[("strong", &["ironafs-disk/extra"]), ("weak", &["d?/extra"])],
        vec![on_disk("ironafs-disk", false, &[]), on_disk("d", true, &[])],
    );
    let xtask = package(
        "xtask",
        true,
        &[("go", &["dep:e"]), ("path", &["e/harmless"])],
        vec![on_disk("e", true, &["extra"])],
    );
    assert_eq!(
        violations(&workspace(vec![disk, shipped, xtask])),
        [
            forwarded("ironafsd", "strong", "ironafs-disk/extra"),
            forwarded("ironafsd", "weak", "d?/extra"),
            forwarded("xtask", "go", "dep:e"),
            forwarded("xtask", "path", "e/harmless"),
        ]
    );
}

#[test]
fn violations_are_not_hidden_by_another_declaration_of_the_same_name() {
    // Two declarations named `d`: a harmless one first, then one asking for
    // `fault`. Turning `d` on forwards through the second.
    let xtask = package(
        "xtask",
        true,
        &[("go", &["dep:d"])],
        vec![
            on_disk("d", true, &[]),
            Declaration {
                kind: "build".into(),
                ..on_disk("d", true, &["fault"])
            },
        ],
    );
    assert_eq!(
        violations(&workspace(vec![home(&[("fault", &[])]), xtask])),
        [forwarded("xtask", "go", "dep:d")]
    );
}

#[test]
fn enabling_follows_a_strong_path_through_an_implicit_feature_of_the_home() {
    // An optional dependency of the home named `fault` has the implicit
    // feature `fault`; `fault/sub` turns it on, so `default` reaches it.
    let disk = home(&[
        ("fault", &["dep:fault"]),
        ("default", &["fault/sub"]),
        ("weak", &["fault?/sub"]),
    ]);
    let mut found = enabling(&disk);
    found.sort();
    assert_eq!(found, [("default", Vec::new()), ("fault", Vec::new())]);
}
