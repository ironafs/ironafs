// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the crate table and the dependency rules.

use std::path::PathBuf;

use super::{CRATES, Kind, Rule, THIRD_PARTY, Violation, graph_violations, kind_of};
use crate::metadata::{Member, Package, Workspace};

/// A workspace of the named members, each under `crates/`; every other
/// name a member depends on is a third-party package with no dependencies.
fn workspace(members: &[(&str, &[&str])]) -> Workspace {
    let id = |name: &str| format!("id {name}");
    let mut packages: Vec<Package> = members
        .iter()
        .map(|(name, dependencies)| Package {
            id: id(name),
            name: (*name).into(),
            path: Some(PathBuf::from(format!("/w/crates/{name}"))),
            member: true,
            dependencies: dependencies.iter().map(|name| id(name)).collect(),
        })
        .collect();
    for (_, dependencies) in members {
        for name in *dependencies {
            if !packages.iter().any(|found| found.name == *name) {
                packages.push(Package {
                    id: id(name),
                    name: (*name).into(),
                    path: None,
                    member: false,
                    dependencies: Vec::new(),
                });
            }
        }
    }
    Workspace {
        root: PathBuf::from("/w"),
        target_dir: PathBuf::from("/w/target"),
        members: members
            .iter()
            .map(|(name, _)| Member {
                name: (*name).into(),
                id: id(name),
                dir: PathBuf::from(format!("/w/crates/{name}")),
                lib_root: None,
                target_kinds: vec!["lib".into()],
            })
            .collect(),
        packages,
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
            &["ironafs-xdr", "ironafs-time", "ironafs-crypto"],
        ),
        ("ironafs-proto", &["ironafs-rx", "ironafs-xdr"]),
        ("ironafs-vfs", &["ironafs-time"]),
        ("ironafs-cache", &["ironafs-proto", "ironafs-vfs"]),
        (
            "ironafs-client",
            &["ironafs-cache", "ironafs-proto", "ironafs-rx"],
        ),
        ("ironafs-net", &["ironafs-rx", "ironafs-os", "outside"]),
        ("ironafs-os", &["ironafs-time", "libc"]),
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
        ("ironafs-ipc", &["ironafs-auth"]),
        ("ironafs-auth", &["ironafs-cache"]),
        ("ironafs-cache", &[]),
    ]);

    assert_eq!(
        graph_violations(&members),
        [edge(
            "ironafs-fuse",
            &["ironafs-ipc", "ironafs-auth"],
            "ironafs-cache",
            Rule::LayerBypassesCore
        )]
    );
}

#[test]
fn graph_violations_fail_any_dependency_of_a_crate_with_none() {
    assert_eq!(
        graph_violations(&workspace(&[
            ("ironafs-time", &["ironafs-events"]),
            ("ironafs-events", &[]),
        ])),
        [Violation::HasDependency {
            name: "ironafs-time".into(),
            dependency: "ironafs-events".into(),
        }]
    );
    assert_eq!(
        graph_violations(&workspace(&[("ironafs-events", &["outside"])])),
        [
            Violation::HasDependency {
                name: "ironafs-events".into(),
                dependency: "outside".into(),
            },
            Violation::ThirdParty {
                from: "ironafs-events".into(),
                to: "outside".into(),
            },
        ]
    );
}

#[test]
fn graph_violations_follow_edges_through_packages_outside_the_workspace() {
    let mut graph = workspace(&[("ironafs-xdr", &["helper"]), ("ironafs-net", &[])]);
    for found in &mut graph.packages {
        if found.name == "helper" {
            found.path = Some(PathBuf::from("/elsewhere/helper"));
            found.dependencies = vec!["id ironafs-net".into()];
        }
    }

    assert_eq!(
        graph_violations(&graph),
        [
            Violation::NotAMember {
                name: "helper".into(),
                dir: PathBuf::from("/elsewhere/helper"),
            },
            Violation::ThirdParty {
                from: "ironafs-xdr".into(),
                to: "helper".into(),
            },
            edge(
                "ironafs-xdr",
                &["helper"],
                "ironafs-net",
                Rule::PureOnNonPure
            ),
        ]
    );
}

#[test]
fn graph_violations_fail_a_member_outside_crates() {
    let mut graph = workspace(&[("ironafs-cli", &[])]);
    for found in &mut graph.packages {
        found.path = Some(PathBuf::from("/w/ironafs-cli"));
    }

    assert_eq!(
        graph_violations(&graph),
        [Violation::OutsideCrates {
            name: "ironafs-cli".into(),
            dir: PathBuf::from("/w/ironafs-cli"),
        }]
    );
}

#[test]
fn graph_violations_limit_third_party_crates_by_kind() {
    let graph = workspace(&[
        ("ironafs-xdr", &["bytes"]),
        ("ironafs-smb", &["bytes"]),
        ("ironafs-client", &["bytes"]),
        ("ironafs-net", &["bytes"]),
        ("ironafs-cli", &["bytes"]),
    ]);

    assert_eq!(
        graph_violations(&graph),
        [
            Violation::ThirdParty {
                from: "ironafs-xdr".into(),
                to: "bytes".into(),
            },
            Violation::ThirdParty {
                from: "ironafs-smb".into(),
                to: "bytes".into(),
            },
        ]
    );
}

#[test]
fn graph_violations_keep_cryptography_and_bindings_in_their_homes() {
    let mut graph = workspace(&[
        ("ironafs-crypto", &["ring"]),
        ("ironafs-os", &["libc"]),
        ("ironafs-net", &["ironafs-os", "wrapper"]),
        ("ironafsd", &["ironafs-net", "ring"]),
    ]);
    for found in &mut graph.packages {
        if found.name == "wrapper" {
            found.dependencies = vec!["id libc".into()];
        }
    }

    assert_eq!(
        graph_violations(&graph),
        [
            Violation::ThirdParty {
                from: "ironafs-crypto".into(),
                to: "ring".into(),
            },
            Violation::CryptoOutsideHome {
                from: "ironafs-net".into(),
                via: vec!["wrapper".into()],
                to: "libc".into(),
            },
            Violation::CryptoOutsideHome {
                from: "ironafsd".into(),
                via: vec![],
                to: "ring".into(),
            },
        ]
    );
}

#[test]
fn graph_violations_keep_layers_to_their_list_and_the_core_off_drivers() {
    let graph = workspace(&[
        (
            "ironafs-fuse",
            &["ironafs-auth", "ironafs-vfs", "ironafs-os"],
        ),
        ("ironafs-auth", &[]),
        ("ironafs-vfs", &[]),
        ("ironafs-os", &[]),
        ("ironafs-client", &["ironafs-smb"]),
        ("ironafs-smb", &[]),
        ("ironafs-sim", &["xtask"]),
        ("xtask", &[]),
    ]);

    assert_eq!(
        graph_violations(&graph),
        [
            edge("ironafs-fuse", &[], "ironafs-auth", Rule::LayerOutsideList),
            edge("ironafs-client", &[], "ironafs-smb", Rule::CoreOnDriver),
            edge("ironafs-sim", &[], "xtask", Rule::OnAutomation),
        ]
    );
}

#[test]
fn third_party_lists_name_every_pure_and_layer_crate_and_start_empty() {
    for (name, kind) in CRATES {
        let listed = THIRD_PARTY.iter().find(|(listed, _)| listed == name);
        if matches!(kind, Kind::Pure | Kind::Layer) {
            assert_eq!(listed.map(|(_, allowed)| allowed.len()), Some(0), "{name}");
        } else {
            assert!(listed.is_none(), "{name}");
        }
    }
}
