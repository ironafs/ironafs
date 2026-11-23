// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading the workspace graph from `cargo metadata` output.

use std::path::{Path, PathBuf};

use super::{Member, Package, Workspace, parse_workspace};
use crate::Error;

const METADATA: &str = r#"{
  "packages": [
    {"name": "ironafs-a", "id": "path+file:///w/crates/ironafs-a#0.0.0", "source": null,
     "features": {}, "links": null, "manifest_path": "/w/crates/ironafs-a/Cargo.toml",
     "dependencies": [{"name": "ironafs-b", "kind": null}, {"name": "ironafs-c", "kind": "dev"},
                      {"name": "outside", "kind": "build"}, {"name": "ironafs-d", "kind": "dev"},
                      {"name": "ironafs-d", "kind": null}],
     "targets": [{"kind": ["lib"], "src_path": "/w/crates/ironafs-a/src/lib.rs", "edition": "2024"}]},
    {"name": "ironafs-b", "id": "path+file:///w/crates/ironafs-b#0.0.0",
     "features": {"std": [], "default": ["std"]}, "links": null, "manifest_path": "/w/crates/ironafs-b/Cargo.toml", "dependencies": [],
     "targets": [{"kind": ["lib"], "src_path": "/w/crates/ironafs-b/src/lib.rs", "edition": "2024"}]},
    {"name": "ironafs-c", "id": "path+file:///w/crates/ironafs-c#0.0.0",
     "features": {}, "links": null, "manifest_path": "/w/crates/ironafs-c/Cargo.toml", "dependencies": [],
     "targets": [{"kind": ["bin"], "src_path": "/w/crates/ironafs-c/src/main.rs", "edition": "2024"}]},
    {"name": "ironafs-d", "id": "path+file:///w/crates/ironafs-d#0.0.0",
     "features": {}, "links": "native", "manifest_path": "/w/crates/ironafs-d/Cargo.toml", "dependencies": [],
     "targets": [{"kind": ["lib"], "src_path": "/w/crates/ironafs-d/src/lib.rs", "edition": "2024"},
                 {"kind": ["custom-build"], "src_path": "/w/crates/ironafs-d/build.rs", "edition": "2018"}]},
    {"name": "outside", "id": "registry+https://example.invalid#outside@1.0.0",
     "source": "registry+https://example.invalid", "manifest_path": "/r/outside/Cargo.toml", "dependencies": [],
     "features": {}, "links": null,
     "targets": [{"kind": ["proc-macro"], "src_path": "/r/outside/src/lib.rs"}]}
  ],
  "workspace_members": [
    "path+file:///w/crates/ironafs-a#0.0.0",
    "path+file:///w/crates/ironafs-b#0.0.0",
    "path+file:///w/crates/ironafs-c#0.0.0",
    "path+file:///w/crates/ironafs-d#0.0.0"
  ],
  "resolve": {"nodes": [
    {"id": "path+file:///w/crates/ironafs-a#0.0.0", "deps": [
      {"name": "bee", "pkg": "path+file:///w/crates/ironafs-b#0.0.0",
       "dep_kinds": [{"kind": null, "target": null}]},
      {"name": "ironafs_c", "pkg": "path+file:///w/crates/ironafs-c#0.0.0",
       "dep_kinds": [{"kind": "dev", "target": null}]},
      {"name": "outside", "pkg": "registry+https://example.invalid#outside@1.0.0",
       "dep_kinds": [{"kind": "build", "target": null}]},
      {"name": "ironafs_d", "pkg": "path+file:///w/crates/ironafs-d#0.0.0",
       "dep_kinds": [{"kind": "dev", "target": null}, {"kind": null, "target": "cfg(windows)"}]}
    ]},
    {"id": "path+file:///w/crates/ironafs-b#0.0.0", "deps": [
      {"name": "outside", "pkg": "registry+https://example.invalid#outside@1.0.0",
       "dep_kinds": [{"kind": null, "target": null}]}
    ]},
    {"id": "path+file:///w/crates/ironafs-c#0.0.0", "deps": []},
    {"id": "path+file:///w/crates/ironafs-d#0.0.0", "deps": []},
    {"id": "registry+https://example.invalid#outside@1.0.0", "deps": []}
  ]},
  "target_directory": "/w/target",
  "workspace_root": "/w"
}"#;

fn package(name: &str, path: bool, member: bool, dependencies: &[&str]) -> Package {
    Package {
        id: id(name),
        name: name.into(),
        path: path.then(|| PathBuf::from(format!("/w/crates/{name}"))),
        member,
        dependencies: dependencies.iter().map(|name| id(name)).collect(),
        dev_dependencies: Vec::new(),
        target_kinds: vec!["lib".into()],
        links: None,
        features: Vec::new(),
    }
}

fn id(name: &str) -> String {
    if name == "outside" {
        "registry+https://example.invalid#outside@1.0.0".into()
    } else {
        format!("path+file:///w/crates/{name}#0.0.0")
    }
}

fn member(name: &str, lib: Option<&str>, target_kinds: &[&str]) -> Member {
    Member {
        name: name.into(),
        id: id(name),
        dir: PathBuf::from(format!("/w/crates/{name}")),
        lib_root: lib.map(|file| PathBuf::from(format!("/w/crates/{name}/{file}"))),
        target_kinds: target_kinds.iter().map(|kind| (*kind).into()).collect(),
        editions: target_kinds.iter().map(|_| "2024".into()).collect(),
        declared_dependencies: Vec::new(),
        renamed: Vec::new(),
    }
}

#[test]
fn parse_workspace_follows_package_ids_and_keeps_development_edges_apart() {
    let workspace = parse_workspace(METADATA).unwrap();

    assert_eq!(
        workspace,
        Workspace {
            root: PathBuf::from("/w"),
            target_dir: PathBuf::from("/w/target"),
            members: vec![
                // `bee` is ironafs-b under another name.
                Member {
                    renamed: vec!["bee".into()],
                    declared_dependencies: vec![
                        "normal ironafs-b".into(),
                        "dev ironafs-c".into(),
                        "build outside".into(),
                        "dev ironafs-d".into(),
                        "normal ironafs-d".into(),
                    ],
                    ..member("ironafs-a", Some("src/lib.rs"), &["lib"])
                },
                member("ironafs-b", Some("src/lib.rs"), &["lib"]),
                member("ironafs-c", None, &["bin"]),
                Member {
                    editions: vec!["2024".into(), "2018".into()],
                    declared_dependencies: Vec::new(),
                    ..member("ironafs-d", Some("src/lib.rs"), &["lib", "custom-build"])
                },
            ],
            packages: vec![
                Package {
                    dev_dependencies: vec![id("ironafs-c")],
                    ..package(
                        "ironafs-a",
                        true,
                        true,
                        &["ironafs-b", "outside", "ironafs-d"],
                    )
                },
                Package {
                    features: vec!["std".into(), "default".into()],
                    ..package("ironafs-b", true, true, &["outside"])
                },
                Package {
                    target_kinds: vec!["bin".into()],
                    ..package("ironafs-c", true, true, &[])
                },
                Package {
                    target_kinds: vec!["lib".into(), "custom-build".into()],
                    links: Some("native".into()),
                    ..package("ironafs-d", true, true, &[])
                },
                Package {
                    target_kinds: vec!["proc-macro".into()],
                    ..package("outside", false, false, &[])
                },
            ],
        }
    );
    assert_eq!(
        workspace.members[0].lib_dir(),
        Some(Path::new("/w/crates/ironafs-a/src"))
    );
}

#[test]
fn parse_workspace_rejects_output_without_a_resolved_graph() {
    for text in [
        "not json",
        r#"{"packages": [], "workspace_members": [], "target_directory": "/t", "workspace_root": "/w"}"#,
        &METADATA.replace(
            "\"pkg\": \"path+file:///w/crates/ironafs-b#0.0.0\"",
            "\"pkg\": \"path+file:///w/crates/unknown#0.0.0\"",
        ),
        &METADATA.replace(
            "{\"id\": \"path+file:///w/crates/ironafs-c#0.0.0\", \"deps\": []},",
            "",
        ),
        &METADATA.replace("\"source\": null", "\"source\": 1"),
        &METADATA.replace(
            "\"features\": {}, \"links\": null, \"manifest_path\": \"/w/crates/ironafs-c",
            "\"manifest_path\": \"/w/crates/ironafs-c",
        ),
        &METADATA.replace("\"links\": \"native\"", "\"links\": 1"),
        &METADATA.replace(", \"edition\": \"2018\"", ""),
    ] {
        assert!(
            matches!(
                parse_workspace(text),
                Err(Error::UnexpectedOutput("cargo metadata"))
            ),
            "input {text}"
        );
    }
}
