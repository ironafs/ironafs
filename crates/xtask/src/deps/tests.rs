// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the crate table and the dependency rules.

use std::path::PathBuf;

use super::{CRATES, Kind, Rule, Violation, graph_violations, kind_of};
use crate::metadata::{Member, Workspace};

fn workspace(members: &[(&str, &[&str])]) -> Workspace {
    Workspace {
        root: PathBuf::from("/w"),
        target_dir: PathBuf::from("/w/target"),
        members: members
            .iter()
            .map(|(name, dependencies)| Member {
                name: (*name).into(),
                dir: PathBuf::from(format!("/w/crates/{name}")),
                lib_dir: None,
                dependencies: dependencies.iter().map(|name| (*name).into()).collect(),
            })
            .collect(),
    }
}

fn edge(from: &str, via: &[&str], to: &str, rule: Rule) -> Violation {
    Violation::Edge {
        from: from.into(),
        via: via.iter().map(|name| (*name).into()).collect(),
        to: to.into(),
        rule,
    }
}

#[test]
fn crate_table_names_every_crate_of_the_architecture_once() {
    let mut names: Vec<&str> = CRATES.iter().map(|(name, _)| *name).collect();
    names.sort_unstable();
    let mut expected = vec![
        "ironafs-xdr",
        "ironafs-rx",
        "ironafs-rxkad",
        "ironafs-rxgk",
        "ironafs-proto",
        "ironafs-vfs",
        "ironafs-cache",
        "ironafs-auth",
        "ironafs-ipc",
        "ironafs-config",
        "ironafs-events",
        "ironafs-crypto",
        "ironafs-time",
        "ironafs-client",
        "ironafs-net",
        "ironafs-fuse",
        "ironafs-smb",
        "ironafs-cloudfiles",
        "ironafs-gss",
        "ironafs-os",
        "ironafsd",
        "ironafs-cli",
        "xtask",
        "ironafs-sim",
        "ironafs-test-support",
    ];
    expected.sort_unstable();
    assert_eq!(names, expected);
    assert_eq!(kind_of("ironafs-client"), Some(Kind::Composition));
    assert_eq!(kind_of("ironafs-smb"), Some(Kind::Layer));
    assert_eq!(kind_of("ironafs-os"), Some(Kind::Driver));
    assert_eq!(kind_of("ironafs-sim"), Some(Kind::TestOnly));
    assert_eq!(kind_of("xtask"), Some(Kind::Automation));
    assert_eq!(kind_of("ironafs-unknown"), None);
}

#[test]
fn graph_violations_fail_an_empty_workspace() {
    assert_eq!(
        graph_violations(&workspace(&[])),
        [Violation::EmptyWorkspace]
    );
}

#[test]
fn graph_violations_fail_an_unclassified_crate() {
    assert_eq!(
        graph_violations(&workspace(&[("ironafs-time", &[]), ("ironafs-new", &[])])),
        [Violation::Unclassified("ironafs-new".into())]
    );
}

#[test]
fn graph_violations_pass_the_allowed_shape() {
    let allowed = workspace(&[
        ("ironafs-time", &[]),
        ("ironafs-xdr", &["ironafs-time"]),
        ("ironafs-crypto", &[]),
        (
            "ironafs-rx",
            &["ironafs-xdr", "ironafs-time", "ironafs-crypto", "outside"],
        ),
        ("ironafs-proto", &["ironafs-rx", "ironafs-xdr"]),
        ("ironafs-vfs", &["ironafs-time"]),
        ("ironafs-cache", &["ironafs-proto", "ironafs-vfs"]),
        (
            "ironafs-client",
            &["ironafs-cache", "ironafs-proto", "ironafs-rx"],
        ),
        ("ironafs-os", &["ironafs-time"]),
        ("ironafs-net", &["ironafs-rx", "ironafs-os"]),
        (
            "ironafs-fuse",
            &["ironafs-vfs", "ironafs-os", "ironafs-crypto"],
        ),
        (
            "ironafsd",
            &["ironafs-client", "ironafs-net", "ironafs-fuse"],
        ),
        ("ironafs-sim", &["ironafs-client"]),
        ("xtask", &["ironafs-sim"]),
    ]);

    assert_eq!(graph_violations(&allowed), []);
}

#[test]
fn graph_violations_name_each_broken_rule() {
    for (members, expected) in [
        (
            &[
                ("ironafs-proto", &["ironafs-net"][..]),
                ("ironafs-net", &[]),
            ][..],
            edge("ironafs-proto", &[], "ironafs-net", Rule::PureOnNonPure),
        ),
        (
            &[
                ("ironafs-vfs", &["ironafs-client"][..]),
                ("ironafs-client", &[]),
            ],
            edge("ironafs-vfs", &[], "ironafs-client", Rule::PureOnNonPure),
        ),
        (
            &[
                ("ironafs-smb", &["ironafs-client"][..]),
                ("ironafs-client", &[]),
            ],
            edge(
                "ironafs-smb",
                &[],
                "ironafs-client",
                Rule::LayerBypassesCore,
            ),
        ),
        (
            &[
                ("ironafs-rx", &["ironafs-proto"][..]),
                ("ironafs-proto", &[]),
            ],
            edge("ironafs-rx", &[], "ironafs-proto", Rule::RxKnowsAfs),
        ),
        (
            &[
                ("ironafs-rxkad", &["ironafs-cache"][..]),
                ("ironafs-cache", &[]),
            ],
            edge(
                "ironafs-rxkad",
                &[],
                "ironafs-cache",
                Rule::ProtocolOnClientSide,
            ),
        ),
        (
            &[("ironafsd", &["ironafs-sim"][..]), ("ironafs-sim", &[])],
            edge("ironafsd", &[], "ironafs-sim", Rule::ShippedOnTestOnly),
        ),
        (
            &[
                ("ironafs-cache", &["ironafs-test-support"][..]),
                ("ironafs-test-support", &[]),
            ],
            edge(
                "ironafs-cache",
                &[],
                "ironafs-test-support",
                Rule::ShippedOnTestOnly,
            ),
        ),
    ] {
        assert_eq!(graph_violations(&workspace(members)), [expected]);
    }
}

#[test]
fn graph_violations_follow_the_graph_through_other_crates() {
    let members = workspace(&[
        ("ironafs-fuse", &["ironafs-ipc"]),
        ("ironafs-ipc", &["ironafs-events"]),
        ("ironafs-events", &["ironafs-cache"]),
        ("ironafs-cache", &[]),
    ]);

    assert_eq!(
        graph_violations(&members),
        [edge(
            "ironafs-fuse",
            &["ironafs-ipc", "ironafs-events"],
            "ironafs-cache",
            Rule::LayerBypassesCore
        )]
    );
}

#[test]
fn graph_violations_fail_any_dependency_of_ironafs_time() {
    assert_eq!(
        graph_violations(&workspace(&[("ironafs-time", &["outside"])])),
        [Violation::TimeHasDependency("outside".into())]
    );
}
