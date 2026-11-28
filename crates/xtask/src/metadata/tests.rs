// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading the workspace graph from `cargo metadata` output.

use std::path::{Path, PathBuf};

use super::{Declaration, Member, Package, Target, Workspace, parse_workspace};
use crate::Error;

const METADATA: &str = r#"{
  "packages": [
    {"name": "ironafs-a", "id": "path+file:///w/crates/ironafs-a#0.0.0", "source": null,
     "features": {}, "links": null, "manifest_path": "/w/crates/ironafs-a/Cargo.toml",
     "dependencies": [{"name": "ironafs-b", "kind": null, "rename": "bee", "optional": false,
                       "features": ["std"]},
                      {"name": "ironafs-c", "kind": "dev", "rename": null, "optional": false, "features": []},
                      {"name": "outside", "kind": "build", "rename": null, "optional": false, "features": []},
                      {"name": "ironafs-d", "kind": "dev", "rename": null, "optional": false, "features": []},
                      {"name": "ironafs-d", "kind": null, "rename": null, "optional": true,
                       "features": []}],
     "targets": [{"kind": ["lib"], "name": "lib", "src_path": "/w/crates/ironafs-a/src/lib.rs", "edition": "2024"}]},
    {"name": "ironafs-b", "id": "path+file:///w/crates/ironafs-b#0.0.0",
     "features": {"std": [], "default": ["std"]}, "links": null, "manifest_path": "/w/crates/ironafs-b/Cargo.toml",
     "dependencies": [{"name": "outside", "kind": null, "rename": null, "optional": false, "features": []}],
     "targets": [{"kind": ["lib"], "name": "lib", "src_path": "/w/crates/ironafs-b/src/lib.rs", "edition": "2024"}]},
    {"name": "ironafs-c", "id": "path+file:///w/crates/ironafs-c#0.0.0",
     "features": {}, "links": null, "manifest_path": "/w/crates/ironafs-c/Cargo.toml", "dependencies": [],
     "targets": [{"kind": ["bin"], "name": "bin", "src_path": "/w/crates/ironafs-c/src/main.rs", "edition": "2024"}]},
    {"name": "ironafs-d", "id": "path+file:///w/crates/ironafs-d#0.0.0",
     "features": {}, "links": "native", "manifest_path": "/w/crates/ironafs-d/Cargo.toml", "dependencies": [],
     "targets": [{"kind": ["lib"], "name": "lib", "src_path": "/w/crates/ironafs-d/src/lib.rs", "edition": "2024"},
                 {"kind": ["custom-build"], "name": "custom-build", "src_path": "/w/crates/ironafs-d/build.rs", "edition": "2018"}]},
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
        declarations: Vec::new(),
        named: Vec::new(),
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
        targets: target_kinds
            .iter()
            .map(|&kind| {
                let file = match kind {
                    "lib" => lib.unwrap(),
                    "bin" => "src/main.rs",
                    _ => "build.rs",
                };
                Target {
                    name: kind.into(),
                    kinds: vec![kind.into()],
                    root: PathBuf::from(format!("/w/crates/{name}/{file}")),
                    required_features: Vec::new(),
                }
            })
            .collect(),
        target_kinds: target_kinds.iter().map(|kind| (*kind).into()).collect(),
        editions: target_kinds.iter().map(|_| "2024".into()).collect(),
        build_script: None,
        declarations: Vec::new(),
        renamed: Vec::new(),
    }
}

fn declaration(
    kind: &str,
    package: &str,
    name: &str,
    optional: bool,
    features: &[&str],
) -> Declaration {
    Declaration {
        kind: kind.into(),
        package: package.into(),
        name: name.into(),
        optional,
        features: features.iter().map(|feature| (*feature).into()).collect(),
    }
}

fn a_declarations() -> Vec<Declaration> {
    vec![
        declaration("normal", "ironafs-b", "bee", false, &["std"]),
        declaration("dev", "ironafs-c", "ironafs-c", false, &[]),
        declaration("build", "outside", "outside", false, &[]),
        declaration("dev", "ironafs-d", "ironafs-d", false, &[]),
        declaration("normal", "ironafs-d", "ironafs-d", true, &[]),
    ]
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
                    declarations: a_declarations(),
                    ..member("ironafs-a", Some("src/lib.rs"), &["lib"])
                },
                Member {
                    declarations: vec![declaration("normal", "outside", "outside", false, &[])],
                    ..member("ironafs-b", Some("src/lib.rs"), &["lib"])
                },
                member("ironafs-c", None, &["bin"]),
                Member {
                    editions: vec!["2024".into(), "2018".into()],
                    build_script: Some(PathBuf::from("/w/crates/ironafs-d/build.rs")),
                    ..member("ironafs-d", Some("src/lib.rs"), &["lib", "custom-build"])
                },
            ],
            packages: vec![
                Package {
                    dev_dependencies: vec![id("ironafs-c")],
                    declarations: a_declarations(),
                    named: vec![
                        ("bee".into(), id("ironafs-b")),
                        ("ironafs_c".into(), id("ironafs-c")),
                        ("outside".into(), id("outside")),
                        ("ironafs_d".into(), id("ironafs-d")),
                    ],
                    ..package(
                        "ironafs-a",
                        true,
                        true,
                        &["ironafs-b", "outside", "ironafs-d"],
                    )
                },
                Package {
                    features: vec![
                        ("std".into(), Vec::new()),
                        ("default".into(), vec!["std".into()]),
                    ],
                    declarations: vec![declaration("normal", "outside", "outside", false, &[])],
                    named: vec![("outside".into(), id("outside"))],
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
        &METADATA.replace("\"rename\": \"bee\"", "\"rename\": 1"),
        &METADATA.replace("\"optional\": true", "\"optional\": null"),
        &METADATA.replace("\"features\": [\"std\"]", "\"features\": [1]"),
        &METADATA.replace("\"default\": [\"std\"]", "\"default\": \"std\""),
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
