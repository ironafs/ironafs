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
        id: name.into(),
        lib_root: Some(dir.join("src/lib.rs")),
        dir,
        target_kinds: vec!["lib".into()],
        editions: vec!["2024".into()],
        target_roots: Vec::new(),
    }
}

fn workspace(name: &str, members: impl FnOnce(&Path) -> Vec<Member>) -> Workspace {
    let root = std::env::temp_dir().join(format!("ironafs-xtask-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    Workspace {
        target_dir: root.join("target"),
        members: members(&root),
        packages: Vec::new(),
        root,
    }
}

#[test]
fn check_reports_names_outside_the_module_but_not_in_comments_or_strings() {
    let mut workspace = workspace("grants-outside", |root| {
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
                        "pub(crate) struct Rights(u32);\npub(crate) fn evaluate() {}\n",
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
                    ("tests/it/main.rs", "type R = r#Rights;\n"),
                    (".hidden/x.rs", "type R = Rights;\n"),
                    ("src/target/x.rs", "type R = Rights;\n"),
                    ("build-output/debug/build.rs", "type R = Rights;\n"),
                ],
            ),
        ]
    });
    // Only the workspace's own build output is not read.
    workspace.target_dir = workspace.root.join("crates/ironafs-cache/build-output");

    let (violations, scanned) = check(&workspace, &TEST).unwrap();

    let outside = |file: &str, line, name| Violation::Outside {
        file: PathBuf::from(file),
        line,
        name,
    };
    assert_eq!(
        violations,
        [
            outside("crates/ironafs-cache/.hidden/x.rs", 1, "Rights"),
            outside("crates/ironafs-cache/src/lib.rs", 2, "evaluate"),
            outside("crates/ironafs-cache/src/target/x.rs", 1, "Rights"),
            outside("crates/ironafs-cache/tests/it/main.rs", 1, "Rights"),
        ]
    );
    assert_eq!(scanned, 8);
    fs::remove_dir_all(&workspace.root).unwrap();
}

#[test]
fn check_refuses_other_names_for_the_guarded_items_in_allowed_files() {
    let workspace = workspace("grants-renamed", |root| {
        vec![member(
            root,
            "ironafs-proto",
            &[
                (
                    "src/grants.rs",
                    "pub(crate) struct Rights(u32);\npub(crate) fn evaluate() {}\n\
                     impl core::ops::Not for Rights {\n    type Output = Rights;\n    \
                     fn not(self) -> Rights { self }\n}\n\
                     pub type Mask<T> = (Rights, T);\n\
                     impl core::ops::Neg for Rights {\n    type Output = Self;\n    \
                     fn neg(self) -> Self { self }\n}\n\
                     pub fn f() -> impl Sized {\n    type Hidden = Rights;\n    0_u8\n}\n",
                ),
                (
                    "src/codec/fs.rs",
                    "pub use crate::grants::{Rights as Mask, evaluate};\n\
                     pub type Check = fn() -> crate::grants::Rights;\n\
                     pub type Plain = u32;\nlet x: Rights = y;\n",
                ),
            ],
        )]
    });

    let renamed = |file: &str, line, name| Violation::Renamed {
        file: PathBuf::from(file),
        line,
        name,
    };
    assert_eq!(
        check(&workspace, &TEST).unwrap().0,
        [
            renamed("crates/ironafs-proto/src/codec/fs.rs", 1, "Rights"),
            renamed("crates/ironafs-proto/src/codec/fs.rs", 2, "Rights"),
            renamed("crates/ironafs-proto/src/grants.rs", 4, "Rights"),
            renamed("crates/ironafs-proto/src/grants.rs", 7, "Rights"),
            renamed("crates/ironafs-proto/src/grants.rs", 13, "Rights"),
        ]
    );
    fs::remove_dir_all(&workspace.root).unwrap();
}

#[test]
fn check_requires_the_guarded_definitions_to_stay_inside_their_crate() {
    let workspace = workspace("grants-public", |root| {
        vec![member(
            root,
            "ironafs-proto",
            &[
                (
                    "src/grants.rs",
                    "#[derive(Debug)]\npub struct Rights(u32);\n\
                     pub(in crate::codec) const unsafe fn evaluate() {}\n",
                ),
                (
                    "src/codec/fs.rs",
                    "pub(super) struct Rights;\nstruct Rights;\n\
                     macro_rules! d { ($v:vis) => { $v fn evaluate() {} }; }\n\
                     pub extern \"C\" fn evaluate() {}\n\
                     impl X { pub(crate) fn evaluate(&self) {} }\n\
                     const unsafe extern \"C\" fn evaluate() {}\n\
                     pub(crate) unsafe extern \"system\" fn evaluate() {}\n",
                ),
            ],
        )]
    });

    let public = |file: &str, line, name| Violation::Public {
        file: PathBuf::from(file),
        line,
        name,
    };
    assert_eq!(
        check(&workspace, &TEST).unwrap().0,
        [
            public("crates/ironafs-proto/src/codec/fs.rs", 3, "evaluate"),
            public("crates/ironafs-proto/src/codec/fs.rs", 4, "evaluate"),
            public("crates/ironafs-proto/src/grants.rs", 2, "Rights"),
        ]
    );
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
                ("src/grants.rs", "pub(crate) struct Rights(u32);\n"),
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
