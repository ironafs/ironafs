// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the exact matrix: what the compiler's own outputs say was
//! built, and how that set is compared with the expected one.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{
    Built, Gap, Triple, built, compare, expected, files_read, matrix_lines, pure_list, read_gap,
};
use crate::json;
use crate::metadata::{Member, Workspace};

fn triple(name: &str, target: &str, profile: &str) -> Triple {
    Triple {
        name: name.into(),
        target: target.into(),
        profile: profile.into(),
    }
}

#[test]
fn pure_list_names_the_thirteen_crates_of_c4_point_1() {
    let names = pure_list();
    assert_eq!(names.len(), 13);
    for name in [
        "ironafs-rxkad",
        "ironafs-auth",
        "ironafs-rxgk",
        "ironafs-time",
    ] {
        assert!(names.contains(&name), "{name}");
    }
}

#[test]
fn expected_is_every_crate_target_and_profile() {
    let targets = ["a".to_owned(), "b".to_owned()];
    let found = expected(&["x", "y"], &targets, &["dev", "release"]);
    assert_eq!(found.len(), 8);
    assert!(found.contains(&triple("y", "b", "release")));
    assert_eq!(expected(&[], &targets, &["dev"]), BTreeSet::new());
}

#[test]
fn compare_reports_each_missing_and_extra_triple() {
    let want: BTreeSet<Triple> = [triple("x", "a", "dev"), triple("x", "a", "release")].into();
    let got: BTreeSet<Triple> = [triple("x", "a", "dev"), triple("y", "a", "dev")].into();
    assert_eq!(
        compare(&want, &got),
        [
            Gap::Missing(triple("x", "a", "release")),
            Gap::Extra(triple("y", "a", "dev")),
        ]
    );
    assert_eq!(compare(&want, &want), [] as [Gap; 0]);
}

fn workspace(root: &Path) -> Workspace {
    Workspace {
        root: root.to_path_buf(),
        target_dir: root.join("target"),
        members: vec![Member {
            name: "ironafs-xdr".into(),
            id: "path+file:///w/crates/ironafs-xdr#0.0.0".into(),
            dir: root.join("crates/ironafs-xdr"),
            lib_root: Some(root.join("crates/ironafs-xdr/src/lib.rs")),
            target_kinds: vec!["lib".into()],
            editions: vec!["2024".into()],
            declared_dependencies: Vec::new(),
            renamed: Vec::new(),
        }],
        packages: vec![],
    }
}

fn artifact(id: &str, kind: &str, file: &str) -> json::Value {
    json::parse(&format!(
        "{{\"reason\":\"compiler-artifact\",\"package_id\":\"{id}\",\
         \"target\":{{\"kind\":[\"{kind}\"],\"src_path\":\"/w/crates/ironafs-xdr/src/lib.rs\"}},\
         \"filenames\":[\"{file}\"],\"fresh\":true}}"
    ))
    .unwrap()
}

#[test]
fn built_reads_the_triple_from_where_the_compiler_wrote_its_metadata() {
    let root = Path::new("/w");
    let space = workspace(root);
    let build = Path::new("/w/target/b");
    let id = "path+file:///w/crates/ironafs-xdr#0.0.0";

    // The layout with a directory per unit, and the older shared one.
    for file in [
        "/w/target/b/x86_64-unknown-linux-gnu/debug/build/ironafs-xdr/0a1b/out/libironafs_xdr-0a1b.rmeta",
        "/w/target/b/x86_64-unknown-linux-gnu/debug/deps/libironafs_xdr-0a1b.rmeta",
    ] {
        let found = built(&artifact(id, "lib", file), &space, build).unwrap();
        assert_eq!(
            found.triple,
            triple("ironafs-xdr", "x86_64-unknown-linux-gnu", "dev")
        );
        assert_eq!(
            found.dep_info,
            Path::new(file).with_file_name("ironafs_xdr-0a1b.d")
        );
        assert_eq!(
            found.lib_root,
            Path::new("/w/crates/ironafs-xdr/src/lib.rs")
        );
    }
    let release = "/w/target/b/aarch64-apple-darwin/release/deps/libironafs_xdr-0a1b.rmeta";
    assert_eq!(
        built(&artifact(id, "lib", release), &space, build)
            .unwrap()
            .triple
            .profile,
        "release"
    );

    // Only a member's library counts: a dependency outside the workspace,
    // a build script and other messages do not.
    let outside = "registry+https://example#tiny@1.0.0";
    assert!(built(&artifact(outside, "lib", release), &space, build).is_none());
    assert!(built(&artifact(id, "custom-build", release), &space, build).is_none());
    let other = json::parse("{\"reason\":\"build-finished\",\"success\":true}").unwrap();
    assert!(built(&other, &space, build).is_none());

    // Metadata written anywhere but a target's profile directory is no
    // triple of the matrix, so it is extra.
    let stray = "/elsewhere/libironafs_xdr-0a1b.rmeta";
    let found = built(&artifact(id, "lib", stray), &space, build).unwrap();
    assert_eq!(found.triple.target, "?");
    let host = "/w/target/b/debug/deps/libironafs_xdr-0a1b.rmeta";
    let found = built(&artifact(id, "lib", host), &space, build).unwrap();
    assert_eq!(found.triple.profile, "?");
}

#[test]
fn files_read_are_the_prerequisites_of_the_dependency_info_s_rules() {
    let text = "/t/out/x-1.d: crates/x/src/lib.rs crates/x/src/a\\ b.rs\n\n\
                /t/out/libx-1.rmeta: crates/x/src/lib.rs crates/x/src/a\\ b.rs\n\n\
                crates/x/src/lib.rs:\n\
                crates/x/src/a\\ b.rs:\n\
                \n\
                # env-dep:CARGO_PKG_NAME=x\n";
    assert_eq!(
        files_read(text, Path::new("/w")),
        [
            PathBuf::from("/w/crates/x/src/lib.rs"),
            PathBuf::from("/w/crates/x/src/a b.rs"),
        ]
    );
    // A unit that read nothing names only its outputs, which are not reads.
    let empty = "/t/out/x-1.d:\n\n/t/out/libx-1.rmeta:\n";
    assert_eq!(files_read(empty, Path::new("/w")), [] as [PathBuf; 0]);
}

#[test]
fn matrix_lines_show_each_crate_and_profile_with_its_targets() {
    let set: BTreeSet<Triple> = [
        triple("ironafs-time", "b", "dev"),
        triple("ironafs-time", "a", "dev"),
        triple("ironafs-time", "a", "release"),
    ]
    .into();
    assert_eq!(
        matrix_lines(&set),
        ["ironafs-time, dev: a, b", "ironafs-time, release: a"]
    );
}

#[test]
fn gaps_say_what_is_missing_or_extra() {
    let shown = Gap::Missing(triple("ironafs-xdr", "a", "dev")).to_string();
    assert!(shown.contains("ironafs-xdr") && shown.contains("was not compiled"));
    let shown = Gap::NothingRead {
        triple: triple("ironafs-xdr", "a", "dev"),
        dep_info: "x.d".into(),
    }
    .to_string();
    assert!(shown.contains("x.d"));
}

#[test]
fn read_gap_wants_the_library_root_among_the_files_read() {
    let found = Built {
        triple: triple("ironafs-xdr", "a", "dev"),
        dep_info: "/t/out/ironafs_xdr-1.d".into(),
        lib_root: "/w/crates/ironafs-xdr/src/lib.rs".into(),
    };
    let gap = Some(Gap::NothingRead {
        triple: found.triple.clone(),
        dep_info: found.dep_info.clone(),
    });
    let root = Path::new("/w");
    let read = "/t/out/libironafs_xdr-1.rmeta: crates/ironafs-xdr/src/lib.rs\n";
    assert_eq!(read_gap(&found, Some(read), root), None);
    // Missing, empty, or naming only another file.
    assert_eq!(read_gap(&found, None, root), gap);
    assert_eq!(read_gap(&found, Some(""), root), gap);
    let other = "/t/out/libironafs_xdr-1.rmeta: crates/ironafs-xdr/src/other.rs\n";
    assert_eq!(read_gap(&found, Some(other), root), gap);
}
