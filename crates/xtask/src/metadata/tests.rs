// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for reading the workspace graph from `cargo metadata` output.

use std::path::PathBuf;

use super::{Member, Workspace, parse_workspace};
use crate::Error;

const METADATA: &str = r#"{
  "packages": [
    {"name": "ironafs-a", "id": "path+file:///w/crates/ironafs-a#0.0.0",
     "manifest_path": "/w/crates/ironafs-a/Cargo.toml",
     "targets": [{"kind": ["lib"], "src_path": "/w/crates/ironafs-a/src/lib.rs"}]},
    {"name": "ironafs-b", "id": "path+file:///w/crates/ironafs-b#0.0.0",
     "manifest_path": "/w/crates/ironafs-b/Cargo.toml",
     "targets": [{"kind": ["lib"], "src_path": "/w/crates/ironafs-b/src/lib.rs"}]},
    {"name": "ironafs-c", "id": "path+file:///w/crates/ironafs-c#0.0.0",
     "manifest_path": "/w/crates/ironafs-c/Cargo.toml",
     "targets": [{"kind": ["bin"], "src_path": "/w/crates/ironafs-c/src/main.rs"}]},
    {"name": "ironafs-d", "id": "path+file:///w/crates/ironafs-d#0.0.0",
     "manifest_path": "/w/crates/ironafs-d/Cargo.toml",
     "targets": [{"kind": ["lib"], "src_path": "/w/crates/ironafs-d/src/lib.rs"}]},
    {"name": "outside", "id": "registry+https://example.invalid#outside@1.0.0",
     "manifest_path": "/r/outside/Cargo.toml",
     "targets": [{"kind": ["lib"], "src_path": "/r/outside/src/lib.rs"}]}
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
    {"id": "path+file:///w/crates/ironafs-b#0.0.0", "deps": []},
    {"id": "path+file:///w/crates/ironafs-c#0.0.0", "deps": []},
    {"id": "path+file:///w/crates/ironafs-d#0.0.0", "deps": []},
    {"id": "registry+https://example.invalid#outside@1.0.0", "deps": []}
  ]},
  "target_directory": "/w/target",
  "workspace_root": "/w"
}"#;

#[test]
fn parse_workspace_follows_package_ids_and_keeps_only_shipped_edges() {
    let workspace = parse_workspace(METADATA).unwrap();

    assert_eq!(
        workspace,
        Workspace {
            root: PathBuf::from("/w"),
            target_dir: PathBuf::from("/w/target"),
            members: vec![
                Member {
                    name: "ironafs-a".into(),
                    dir: PathBuf::from("/w/crates/ironafs-a"),
                    lib_dir: Some(PathBuf::from("/w/crates/ironafs-a/src")),
                    dependencies: vec!["ironafs-b".into(), "outside".into(), "ironafs-d".into()],
                },
                Member {
                    name: "ironafs-b".into(),
                    dir: PathBuf::from("/w/crates/ironafs-b"),
                    lib_dir: Some(PathBuf::from("/w/crates/ironafs-b/src")),
                    dependencies: vec![],
                },
                Member {
                    name: "ironafs-c".into(),
                    dir: PathBuf::from("/w/crates/ironafs-c"),
                    lib_dir: None,
                    dependencies: vec![],
                },
                Member {
                    name: "ironafs-d".into(),
                    dir: PathBuf::from("/w/crates/ironafs-d"),
                    lib_dir: Some(PathBuf::from("/w/crates/ironafs-d/src")),
                    dependencies: vec![],
                },
            ],
        }
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
            "{\"id\": \"path+file:///w/crates/ironafs-b#0.0.0\", \"deps\": []},",
            "",
        ),
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
