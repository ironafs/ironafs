// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the crate table and the dependency rules.

use std::path::PathBuf;

use super::{
    CRATES, Kind, OWN_LINTS, Rule, THIRD_PARTY, ThirdPartyLists, Violation,
    dependency_table_violations, edition_violations, graph_violations, inherits_workspace_edition,
    inherits_workspace_lints, kind_of, lint_violations, own_lints_differences,
};
use crate::metadata::{Declaration, Member, Package, Workspace};

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
            dev_dependencies: Vec::new(),
            target_kinds: vec!["lib".into()],
            links: None,
            features: Vec::new(),
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
                    dev_dependencies: Vec::new(),
                    target_kinds: vec!["lib".into()],
                    links: None,
                    features: Vec::new(),
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
                targets: Vec::new(),
                target_kinds: vec!["lib".into()],
                editions: vec!["2024".into()],
                build_script: None,
                declarations: Vec::new(),
                features: Vec::new(),
                renamed: Vec::new(),
            })
            .collect(),
        packages,
    }
}

/// The rules with the repository's third-party lists.
fn graph_violations_of(workspace: &Workspace) -> Vec<Violation> {
    graph_violations(workspace, THIRD_PARTY)
}

fn third_party(from: &str, to: &str) -> Violation {
    Violation::ThirdParty {
        from: from.into(),
        to: to.into(),
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
        "ironafs-disk",
        "ironafsd",
        "ironafs-cli",
        "xtask",
        "ironafs-sim",
        "ironafs-test-support",
        // check-bans' canary, outside the architecture's list.
        "ironafs-bans-canary",
    ];
    expected.sort_unstable();
    assert_eq!(names, expected);
    assert_eq!(kind_of("ironafs-client"), Some(Kind::Composition));
    assert_eq!(kind_of("ironafs-smb"), Some(Kind::Layer));
    assert_eq!(kind_of("ironafs-os"), Some(Kind::Driver));
    assert_eq!(kind_of("ironafs-disk"), Some(Kind::Driver));
    assert_eq!(kind_of("ironafs-sim"), Some(Kind::TestOnly));
    assert_eq!(kind_of("xtask"), Some(Kind::Automation));
    assert_eq!(kind_of("ironafs-unknown"), None);
}

#[test]
fn graph_violations_fail_an_empty_workspace() {
    assert_eq!(
        graph_violations_of(&workspace(&[])),
        [Violation::EmptyWorkspace]
    );
}

#[test]
fn graph_violations_fail_an_unclassified_crate() {
    assert_eq!(
        graph_violations_of(&workspace(&[("ironafs-time", &[]), ("ironafs-new", &[])])),
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
        ("ironafs-os", &["libc"]),
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

    // The repository's lists are empty; these admit what the shape uses.
    let lists: ThirdPartyLists = &[("ironafs-os", &["libc"]), ("ironafs-net", &["outside"])];
    assert_eq!(graph_violations(&allowed, lists), []);
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
        assert_eq!(graph_violations_of(&workspace(members)), [expected]);
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
        graph_violations_of(&members),
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
        graph_violations_of(&workspace(&[
            ("ironafs-time", &["ironafs-events"]),
            ("ironafs-events", &[]),
        ])),
        [Violation::HasDependency {
            name: "ironafs-time".into(),
            dependency: "ironafs-events".into(),
        }]
    );
    assert_eq!(
        graph_violations_of(&workspace(&[("ironafs-events", &["outside"])])),
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
        graph_violations_of(&graph),
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
        graph_violations_of(&graph),
        [Violation::OutsideCrates {
            name: "ironafs-cli".into(),
            dir: PathBuf::from("/w/ironafs-cli"),
        }]
    );
}

#[test]
fn graph_violations_limit_third_party_crates_of_every_shipped_crate() {
    let graph = workspace(&[
        ("ironafs-xdr", &["bytes"]),
        ("ironafs-smb", &["bytes"]),
        ("ironafs-client", &["bytes"]),
        ("ironafs-net", &["bytes"]),
        ("ironafs-cli", &["bytes"]),
        ("ironafs-sim", &["bytes"]),
        ("xtask", &["bytes"]),
    ]);

    assert_eq!(
        graph_violations_of(&graph),
        [
            third_party("ironafs-xdr", "bytes"),
            third_party("ironafs-smb", "bytes"),
            third_party("ironafs-client", "bytes"),
            third_party("ironafs-net", "bytes"),
            third_party("ironafs-cli", "bytes"),
        ]
    );
    let lists: ThirdPartyLists = &[("ironafs-client", &["bytes"])];
    assert_eq!(graph_violations(&graph, lists).len(), 4);
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
    let lists: ThirdPartyLists = &[
        ("ironafs-crypto", &["ring"]),
        ("ironafs-os", &["libc"]),
        ("ironafs-net", &["wrapper"]),
        ("ironafsd", &["ring"]),
    ];

    assert_eq!(
        graph_violations(&graph, lists),
        [
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
fn graph_violations_let_ironafs_gss_reach_only_the_bindings_its_list_names() {
    let mut graph = workspace(&[
        ("ironafs-gss", &["gss-ffi", "openssl-sys"]),
        ("ironafs-net", &["gss-ffi"]),
    ]);
    for found in &mut graph.packages {
        if found.name == "gss-ffi" {
            found.dependencies = vec!["id libc".into()];
        }
    }
    graph.packages.push(Package {
        id: "id libc".into(),
        name: "libc".into(),
        path: None,
        member: false,
        dependencies: Vec::new(),
        dev_dependencies: Vec::new(),
        target_kinds: vec!["lib".into()],
        links: None,
        features: Vec::new(),
    });
    let lists: ThirdPartyLists = &[
        ("ironafs-gss", &["gss-ffi", "libc"]),
        ("ironafs-net", &["gss-ffi", "libc"]),
    ];

    assert_eq!(
        graph_violations(&graph, lists),
        [
            third_party("ironafs-gss", "openssl-sys"),
            Violation::CryptoOutsideHome {
                from: "ironafs-gss".into(),
                via: vec![],
                to: "openssl-sys".into(),
            },
            Violation::CryptoOutsideHome {
                from: "ironafs-net".into(),
                via: vec!["gss-ffi".into()],
                to: "libc".into(),
            },
        ]
    );
}

#[test]
fn graph_violations_keep_layers_and_drivers_to_their_lists_and_the_core_off_drivers() {
    let graph = workspace(&[
        (
            "ironafs-fuse",
            &["ironafs-auth", "ironafs-vfs", "ironafs-os"],
        ),
        ("ironafs-auth", &[]),
        ("ironafs-vfs", &[]),
        ("ironafs-os", &["ironafs-proto"]),
        ("ironafs-time", &[]),
        ("ironafs-proto", &[]),
        (
            "ironafs-net",
            &["ironafs-rx", "ironafs-os", "ironafs-cache"],
        ),
        ("ironafs-rx", &[]),
        ("ironafs-cache", &[]),
        ("ironafs-client", &["ironafs-smb"]),
        ("ironafs-smb", &[]),
        ("ironafs-sim", &["xtask"]),
        ("xtask", &[]),
    ]);

    assert_eq!(
        graph_violations_of(&graph),
        [
            edge("ironafs-fuse", &[], "ironafs-auth", Rule::LayerOutsideList),
            edge("ironafs-os", &[], "ironafs-proto", Rule::DriverOutsideList),
            edge("ironafs-net", &[], "ironafs-cache", Rule::DriverOutsideList),
            edge("ironafs-client", &[], "ironafs-smb", Rule::CoreOnDriver),
            edge("ironafs-sim", &[], "xtask", Rule::OnAutomation),
        ]
    );
}

#[test]
fn third_party_lists_name_every_shipped_crate_and_start_empty() {
    for (name, kind) in CRATES {
        let listed = THIRD_PARTY.iter().find(|(listed, _)| listed == name);
        if matches!(kind, Kind::TestOnly | Kind::Automation) {
            assert!(listed.is_none(), "{name}");
        } else {
            assert_eq!(listed.map(|(_, allowed)| allowed.len()), Some(0), "{name}");
        }
    }
}

#[test]
fn inherits_workspace_lints_accepts_only_the_inherited_table() {
    for manifest in [
        "[package]\nname = \"a\"\n\n[lints]\nworkspace = true\n",
        "[package]\nname = \"a\"\n[lints] # inherited\n  workspace=true  # yes\n[features]\n",
        "lints.workspace = true\n[package]\nname = \"a\"\n",
        "lints = { workspace = true }\n[package]\nname = \"a\"\n",
    ] {
        assert!(inherits_workspace_lints(manifest), "{manifest}");
    }
    for manifest in [
        "[package]\nname = \"a\"\n",
        "[package]\nname = \"a\"\n[lints]\n",
        "[package]\nname = \"a\"\n[lints]\nworkspace = false\n",
        "[package]\nname = \"a\"\n[lints.rust]\nunsafe_code = \"allow\"\n",
        "[package]\nname = \"a\"\n[lints]\nworkspace = true\n[lints.rust]\nunsafe_code = \"allow\"\n",
        "[package]\nname = \"a\"\n[lints]\nworkspace = true\nrust = { unsafe_code = \"allow\" }\n",
        "[package]\nname = \"a\"\n# [lints]\n# workspace = true\n",
        "[package]\nlints.workspace = true\n",
        "[package]\nreadme = \"\"\"\n[lints]\nworkspace = true\n[notes]\n\"\"\"\n",
        "[package]\nreadme = \'\'\'\n[lints]\nworkspace = true\n[notes]\n\'\'\'\n",
        "[package]\n[lints]\nworkspace = true\n[package.metadata]\nnote = \"\"\"x\"\"\"\n",
        "[package]\n[lints]\nworkspace = true\n[target.'cfg(unix)'.lints.rust]\nunsafe_code = \"allow\"\n",
    ] {
        assert!(!inherits_workspace_lints(manifest), "{manifest}");
    }
}

/// A workspace manifest with lints in both tables, as the root's are.
const ROOT_MANIFEST: &str = "[workspace]\nmembers = [\"crates/*\"]\n\n\
                             [workspace.lints.rust]\nunsafe_code = \"forbid\"\n\
                             unreachable_pub = \"warn\"\n\n\
                             [workspace.lints.clippy]\n\
                             pedantic = { level = \"warn\", priority = -1 }\n\
                             unwrap_used = \"deny\"\n";

/// The own table that `ROOT_MANIFEST` allows: the same, with unsafe code
/// denied instead of forbidden.
const OWN_TABLE: &str = "[package]\nname = \"ironafs-os\"\n\n\
                         [lints.rust]\nunsafe_code = \"deny\" # ADR 0005\n\
                         unreachable_pub = \"warn\"\n\n\
                         [lints.clippy]\n\
                         pedantic = {level=\"warn\",priority=-1}\n\
                         unwrap_used = \"deny\"\n";

#[test]
fn lint_violations_skip_only_the_crates_an_adr_lets_keep_their_own_lints() {
    assert_eq!(OWN_LINTS, ["ironafs-os"]);
    let root = std::env::temp_dir().join(format!("ironafs-xtask-lints-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut graph = workspace(&[("ironafs-os", &[]), ("ironafs-net", &[])]);
    graph.root.clone_from(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("Cargo.toml"), ROOT_MANIFEST).unwrap();
    for member in &mut graph.members {
        member.dir = root.join("crates").join(&member.name);
        std::fs::create_dir_all(&member.dir).unwrap();
        std::fs::write(member.dir.join("Cargo.toml"), OWN_TABLE).unwrap();
    }

    assert_eq!(
        lint_violations(&graph, &["ironafs-os"]).unwrap(),
        [Violation::NoWorkspaceLints {
            name: "ironafs-net".into(),
            manifest: PathBuf::from("crates/ironafs-net/Cargo.toml"),
        }]
    );
    assert_eq!(lint_violations(&graph, &[]).unwrap().len(), 2);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn own_lints_differences_accept_the_workspace_table_with_unsafe_code_denied() {
    assert_eq!(
        own_lints_differences(ROOT_MANIFEST, OWN_TABLE),
        [] as [String; 0]
    );
}

#[test]
fn own_lints_differences_name_each_departure_from_the_workspace_table() {
    let cases: [(&str, &str, &str); 6] = [
        (
            "unsafe code allowed",
            "unsafe_code = \"deny\"",
            "unsafe_code = \"allow\"",
        ),
        (
            "unsafe code still forbidden",
            "unsafe_code = \"deny\"",
            "unsafe_code = \"forbid\"",
        ),
        (
            "a lint lowered",
            "unwrap_used = \"deny\"",
            "unwrap_used = \"warn\"",
        ),
        ("a lint dropped", "unwrap_used = \"deny\"\n", ""),
        (
            "a lint added",
            "unwrap_used = \"deny\"",
            "unwrap_used = \"deny\"\nexpect_used = \"allow\"",
        ),
        ("a priority changed", "priority=-1", "priority=0"),
    ];
    for (name, from, to) in cases {
        let manifest = OWN_TABLE.replace(from, to);
        assert_ne!(manifest, OWN_TABLE, "{name}");
        assert_eq!(
            own_lints_differences(ROOT_MANIFEST, &manifest).len(),
            1,
            "{name}: {manifest}"
        );
    }
}

#[test]
fn own_lints_differences_refuse_a_table_they_cannot_read_exactly() {
    for manifest in [
        "[package]\n[lints]\nworkspace = true\n",
        &format!("{OWN_TABLE}[target.'cfg(unix)'.lints.rust]\nunsafe_code = \"allow\"\n"),
        &format!("{OWN_TABLE}[package.metadata]\nnote = \"\"\"\n[lints.rust]\n\"\"\"\n"),
        &OWN_TABLE.replace(
            "[lints.rust]\n",
            "[lints]\nrust = { unsafe_code = \"deny\" }\n[lints.rust]\n",
        ),
        "[package]\nname = \"ironafs-os\"\n",
    ] {
        assert!(
            !own_lints_differences(ROOT_MANIFEST, manifest).is_empty(),
            "{manifest}"
        );
    }
}

#[test]
fn inherits_workspace_edition_accepts_only_the_inherited_edition_in_the_package_table() {
    for manifest in [
        "[package]\nname = \"a\"\nedition.workspace = true\n",
        "[package]\nname = \"a\"\n  edition . workspace=true  # inherited\n[lints]\n",
        "[package]\nedition = { workspace = true }\n",
    ] {
        assert!(inherits_workspace_edition(manifest), "{manifest}");
    }
    for manifest in [
        "[package]\nname = \"a\"\n",
        "[package]\nedition = \"2018\"\n",
        "[package]\nedition = \"2024\"\n",
        "[package]\nedition.workspace = false\n",
        "edition.workspace = true\n[package]\nname = \"a\"\n",
        "[package]\n[lib]\nedition.workspace = true\n",
        "[package]\nedition.workspace = true\n[lib]\nedition = \"2018\"\n",
        "[package]\nedition.workspace = true\n[[bin]]\nname = \"b\"\nedition = \"2018\"\n",
        "[package]\nedition.workspace = true\nlib = { path = \"#\", edition = \"2018\" }\n",
        "[package]\nedition.workspace = true # edition = \"2018\"\nx = 1 # the edition\n",
        "[package]\nedition.workspace = true\nreadme = \"\"\"\nx\n\"\"\"\n",
        "[package]\n# edition.workspace = true\n",
    ] {
        assert!(!inherits_workspace_edition(manifest), "{manifest}");
    }
}

#[test]
fn edition_violations_want_the_inherited_edition_and_the_edition_the_reader_follows() {
    let root = std::env::temp_dir().join(format!("ironafs-xtask-edition-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut graph = workspace(&[("ironafs-a", &[]), ("ironafs-b", &[]), ("ironafs-c", &[])]);
    graph.root.clone_from(&root);
    for (member, manifest) in graph.members.iter_mut().zip([
        "[package]\nedition.workspace = true\n",
        "[package]\nedition = \"2018\"\n",
        "[package]\nedition.workspace = true\n",
    ]) {
        member.dir = root.join("crates").join(&member.name);
        std::fs::create_dir_all(&member.dir).unwrap();
        std::fs::write(member.dir.join("Cargo.toml"), manifest).unwrap();
    }
    if let Some(member) = graph.members.get_mut(2) {
        member.editions = vec!["2024".into(), "2021".into(), "2021".into()];
    }

    assert_eq!(
        edition_violations(&graph).unwrap(),
        [
            Violation::NoWorkspaceEdition {
                name: "ironafs-b".into(),
                manifest: PathBuf::from("crates/ironafs-b/Cargo.toml"),
            },
            Violation::Edition {
                name: "ironafs-c".into(),
                edition: "2021".into(),
            },
        ]
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn dependency_tables_are_cargo_s_declared_dependencies_not_manifest_text() {
    // Cargo's parse of the manifest is the rule: a declared dependency of any
    // kind is refused, and text that only mentions the word is not.
    let root = std::env::temp_dir().join(format!("ironafs-xtask-tables-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut graph = workspace(&[("ironafs-os", &[])]);
    graph.root.clone_from(&root);
    for member in &mut graph.members {
        member.dir = root.join("crates").join(&member.name);
        std::fs::create_dir_all(&member.dir).unwrap();
        std::fs::write(
            member.dir.join("Cargo.toml"),
            "[package]\nname = \"ironafs-os\"\ndescription = \"no dependencies\"\n",
        )
        .unwrap();
    }
    assert_eq!(dependency_table_violations(&graph, &["ironafs-os"]), []);
    if let Some(member) = graph.members.first_mut() {
        member.declarations = vec![Declaration {
            kind: "dev".into(),
            package: "ironafs-time".into(),
            name: "ironafs-time".into(),
            optional: false,
            features: Vec::new(),
        }];
    }
    assert_eq!(
        dependency_table_violations(&graph, &["ironafs-os"]),
        [Violation::DependencyTable {
            name: "ironafs-os".into(),
            manifest: PathBuf::from("crates/ironafs-os/Cargo.toml"),
            line: "declares the dev ironafs-time".into(),
        }]
    );
    std::fs::remove_dir_all(&root).unwrap();
}
