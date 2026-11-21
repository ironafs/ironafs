// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the standard-library paths a pure crate may not name.

use std::fs;
use std::path::{Path, PathBuf};

use super::{forbidden, scan};
use crate::deps::Violation;
use crate::metadata::{Member, Workspace};

#[test]
fn forbidden_names_io_clocks_and_aliases_but_not_values() {
    for path in [
        "std::net",
        "std::net::UdpSocket::bind",
        "std::net::*",
        "std::fs::read",
        "std::thread::spawn",
        "std::time::Instant::now",
        "std::time::SystemTime",
        "std::time::UNIX_EPOCH",
        "std::time::*",
        "std::time",
        "std::*",
        "std",
    ] {
        assert!(forbidden(path).is_some(), "{path} should be forbidden");
    }
    for path in [
        "std::time::Duration",
        "std::io::Read",
        "std::fmt",
        "std::netlink",
        "std::collections::BTreeMap",
    ] {
        assert_eq!(forbidden(path), None, "{path} should be allowed");
    }
}

fn pure_member(root: &Path, name: &str, files: &[(&str, &str)]) -> Member {
    let dir = root.join(name);
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

#[test]
fn scan_reports_each_forbidden_path_in_library_files_and_skips_tests() {
    let root =
        std::env::temp_dir().join(format!("ironafs-xtask-pure-source-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let workspace = Workspace {
        target_dir: root.join("target"),
        members: vec![
            pure_member(
                &root,
                "ironafs-xdr",
                &[
                    (
                        "src/lib.rs",
                        "mod codec;\n#[cfg(test)]\nmod tests;\nuse core::net::Ipv4Addr;\n",
                    ),
                    (
                        "src/codec.rs",
                        "// std::fs\nfn f() {\n    std::thread::spawn(g);\n}\n",
                    ),
                    ("src/tests.rs", "use std::fs;\n"),
                    ("src/codec/tests.rs", "use std::net::UdpSocket;\n"),
                    ("src/codec/tests/helper.rs", "use std::net::UdpSocket;\n"),
                ],
            ),
            Member {
                name: "ironafs-net".into(),
                dir: root.join("ironafs-net"),
                lib_dir: Some(root.join("missing")),
                dependencies: vec![],
            },
            Member {
                name: "ironafs-config".into(),
                dir: root.join("ironafs-config"),
                lib_dir: None,
                dependencies: vec![],
            },
            pure_member(&root, "ironafs-time", &[("src/lib.rs", "use std as s;\n")]),
            pure_member(&root, "ironafs-vfs", &[("src/lib.rs", "\"unterminated\n")]),
        ],
        root: root.clone(),
    };

    let (violations, scanned) = scan(&workspace).unwrap();

    assert_eq!(
        scanned,
        [
            PathBuf::from("ironafs-xdr/src/codec.rs"),
            PathBuf::from("ironafs-xdr/src/lib.rs"),
            PathBuf::from("ironafs-time/src/lib.rs"),
            PathBuf::from("ironafs-vfs/src/lib.rs"),
        ]
    );
    assert_eq!(
        violations,
        [
            Violation::ForbiddenStdPath {
                file: PathBuf::from("ironafs-xdr/src/codec.rs"),
                line: 3,
                path: "std::thread::spawn".into(),
                reason: forbidden("std::thread").unwrap(),
            },
            Violation::NoLibrary("ironafs-config".into()),
            Violation::ForbiddenStdPath {
                file: PathBuf::from("ironafs-time/src/lib.rs"),
                line: 1,
                path: "std".into(),
                reason: forbidden("std").unwrap(),
            },
            Violation::Unreadable {
                file: PathBuf::from("ironafs-vfs/src/lib.rs"),
                error: "line 1: unterminated string".into(),
            },
        ]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_fails_a_pure_crate_with_no_library_source() {
    let root =
        std::env::temp_dir().join(format!("ironafs-xtask-pure-empty-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let member = pure_member(&root, "ironafs-xdr", &[("src/tests.rs", "")]);
    let workspace = Workspace {
        target_dir: root.join("target"),
        members: vec![member],
        root: root.clone(),
    };

    let (violations, scanned) = scan(&workspace).unwrap();

    assert_eq!(violations, [Violation::NoSource("ironafs-xdr".into())]);
    assert_eq!(scanned, [] as [PathBuf; 0]);
    fs::remove_dir_all(root).unwrap();
}
