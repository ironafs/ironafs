// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for confining the rights evaluator's names to their module.

use std::fs;
use std::path::{Path, PathBuf};

use super::{CONFINEMENT, Confinement, Violation, check, is_allowed};
use crate::metadata::{Member, Workspace};

const TEST: Confinement = Confinement {
    guarded: &["Rights", "evaluate"],
    home_crate: "ironafs-proto",
    evaluator: "crates/ironafs-proto/src/grants.rs",
    allowed: &[
        "crates/ironafs-proto/src/grants.rs",
        "crates/ironafs-proto/src/grants/",
        "crates/ironafs-proto/src/codec/",
    ],
};

#[test]
fn confinement_keeps_the_evaluator_inside_its_allowed_paths() {
    for configured in [&CONFINEMENT, &TEST] {
        assert_ne!(configured.guarded, &[] as &[&str]);
        assert!(is_allowed(configured, Path::new(configured.evaluator)));
        assert!(
            configured
                .allowed
                .iter()
                .all(|path| path.starts_with(&format!("crates/{}/", configured.home_crate)))
        );
    }
}

#[test]
fn is_allowed_matches_files_and_directories_by_whole_components() {
    for (path, allowed) in [
        ("crates/ironafs-proto/src/grants.rs", true),
        ("crates/ironafs-proto/src/grants/tests.rs", true),
        ("crates/ironafs-proto/src/codec/fs/reply.rs", true),
        ("crates/ironafs-proto/src/grants_extra.rs", false),
        ("crates/ironafs-proto/src/grants.rs.bak/x.rs", false),
        ("crates/ironafs-proto/src/codecs/x.rs", false),
        ("crates/ironafs-proto/src/lib.rs", false),
        ("crates/ironafs-cache/src/grants.rs", false),
    ] {
        assert_eq!(is_allowed(&TEST, Path::new(path)), allowed, "{path}");
    }
}

fn member(root: &Path, name: &str, files: &[(&str, &str)]) -> Member {
    let dir = root.join("crates").join(name);
    for (path, contents) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    Member {
        name: name.into(),
        lib_dir: Some(dir.join("src")),
        dir,
        dependencies: vec![],
    }
}

fn workspace(name: &str, members: impl FnOnce(&Path) -> Vec<Member>) -> Workspace {
    let root = std::env::temp_dir().join(format!("ironafs-xtask-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    Workspace {
        target_dir: root.join("target"),
        members: members(&root),
        root,
    }
}

#[test]
fn check_reports_names_outside_the_module_but_not_in_comments_or_strings() {
    let workspace = workspace("grants-outside", |root| {
        vec![
            member(
                root,
                "ironafs-proto",
                &[
                    (
                        "src/lib.rs",
                        "// Rights\nconst S: &str = \"evaluate\";\nmod grants;\n",
                    ),
                    (
                        "src/grants.rs",
                        "pub struct Rights(u32);\npub fn evaluate() {}\n",
                    ),
                    ("src/grants/tests.rs", "use super::{Rights, evaluate};\n"),
                    ("src/codec/fs.rs", "use crate::grants::Rights;\n"),
                ],
            ),
            member(
                root,
                "ironafs-cache",
                &[
                    (
                        "src/lib.rs",
                        "fn f() {\n    ironafs_proto::grants::evaluate();\n}\n",
                    ),
                    ("tests/it/main.rs", "type R = Rights;\n"),
                    ("target/debug/build.rs", "type R = Rights;\n"),
                ],
            ),
        ]
    });

    let (violations, scanned) = check(&workspace, &TEST).unwrap();

    assert_eq!(
        violations,
        [
            Violation::Outside {
                file: PathBuf::from("crates/ironafs-cache/src/lib.rs"),
                line: 2,
                name: "evaluate",
            },
            Violation::Outside {
                file: PathBuf::from("crates/ironafs-cache/tests/it/main.rs"),
                line: 1,
                name: "Rights",
            },
        ]
    );
    assert_eq!(scanned, 6);
    fs::remove_dir_all(&workspace.root).unwrap();
}

#[test]
fn check_requires_the_module_and_its_definitions_once_the_home_crate_exists() {
    let missing = workspace("grants-missing", |root| {
        vec![member(root, "ironafs-proto", &[("src/lib.rs", "")])]
    });
    assert_eq!(
        check(&missing, &TEST).unwrap().0,
        [Violation::EvaluatorMissing(PathBuf::from(
            "crates/ironafs-proto/src/grants.rs"
        ))]
    );
    fs::remove_dir_all(&missing.root).unwrap();

    let undefined = workspace("grants-undefined", |root| {
        vec![member(
            root,
            "ironafs-proto",
            &[
                ("src/lib.rs", ""),
                ("src/grants.rs", "pub struct Rights(u32);\n"),
            ],
        )]
    });
    assert_eq!(
        check(&undefined, &TEST).unwrap().0,
        [Violation::NotDefined("evaluate")]
    );
    fs::remove_dir_all(&undefined.root).unwrap();
}

#[test]
fn check_before_the_home_crate_exists_still_reads_every_file() {
    let before = workspace("grants-before", |root| {
        vec![member(root, "ironafs-cache", &[("src/lib.rs", "")])]
    });
    assert_eq!(check(&before, &TEST).unwrap(), (vec![], 1));
    fs::remove_dir_all(&before.root).unwrap();

    let empty = workspace("grants-empty", |_| vec![]);
    assert_eq!(
        check(&empty, &TEST).unwrap(),
        (vec![Violation::NothingScanned], 0)
    );
}
