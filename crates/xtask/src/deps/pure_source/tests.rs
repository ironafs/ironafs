// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for what the source of a pure crate and of `ironafs-client` may
//! not name.

use std::fs;
use std::path::{Path, PathBuf};

use super::{forbidden, gates_every_test_module, hidden_names, scan};
use crate::deps::{Kind, Violation};
use crate::metadata::{Member, Workspace};
use crate::rust_source::tokens;

#[test]
fn forbidden_names_io_clocks_and_aliases_but_not_values() {
    for path in [
        "std::net",
        "std::net::UdpSocket::bind",
        "std::net::*",
        "std::fs::read",
        "std::thread::spawn",
        "std::os::unix::net::UnixStream",
        "std::process::exit",
        "std::env::var",
        "std::io::stdout",
        "std::io::Stderr",
        "std::io",
        "std::io::*",
        "std::time::Instant::now",
        "std::time::SystemTime",
        "std::time::UNIX_EPOCH",
        "std::time::*",
        "std::time",
        "std::*",
        "std",
    ] {
        assert!(
            forbidden(path, Kind::Pure).is_some(),
            "{path} should be forbidden"
        );
    }
    for path in [
        "std::time::Duration",
        "std::io::Read",
        "std::io::Error",
        "std::fmt",
        "std::netlink",
        "std::collections::BTreeMap",
    ] {
        assert_eq!(
            forbidden(path, Kind::Pure),
            None,
            "{path} should be allowed"
        );
    }
}

#[test]
fn forbidden_lets_only_the_composition_crate_use_threads() {
    assert!(forbidden("std::thread::spawn", Kind::Pure).is_some());
    assert_eq!(forbidden("std::thread::spawn", Kind::Composition), None);
    assert_eq!(forbidden("std::thread", Kind::Composition), None);
    assert!(forbidden("std::net::UdpSocket", Kind::Composition).is_some());
    assert!(forbidden("std::time::Instant", Kind::Composition).is_some());
}

fn names(source: &str) -> Vec<(usize, String)> {
    hidden_names(&tokens(source).unwrap())
        .into_iter()
        .map(|(line, name, _)| (line, name))
        .collect()
}

#[test]
fn hidden_names_find_what_reaches_the_system_without_a_std_path() {
    let source = "thread::sleep(d);\n\
                  let x = r.recv_timeout(d);\n\
                  let m = HashMap::new();\n\
                  let n = HashSet::<u8>::with_capacity(4);\n\
                  let s: RandomState = h;\n\
                  if path.exists() {}\n\
                  let p = Path::canonicalize;\n\
                  println!(\"x\");\n\
                  macro_rules! v { ($i:ident) => { $i::fs::read(p) }; }\n\
                  include!(\"other.rs\");\n\
                  #[cfg_attr(test, path = \"other.rs\")]\nmod other;\n\
                  #![path = \"x\"]\n";

    assert_eq!(
        names(source),
        [
            (1, "sleep".into()),
            (2, "recv_timeout".into()),
            (3, "HashMap::new".into()),
            (4, "HashSet::with_capacity".into()),
            (5, "RandomState".into()),
            (6, "exists".into()),
            (7, "canonicalize".into()),
            (8, "println!".into()),
            (9, "$i::".into()),
            (10, "include!".into()),
            (11, "#[path]".into()),
            (13, "#[path]".into()),
        ]
    );
}

#[test]
fn hidden_names_leave_ordinary_code_alone() {
    let source = "let a = entry.metadata;\n\
                  let b = map.get(&k).exists;\n\
                  let c: HashMap<u8, u8, Fixed> = HashMap::with_hasher(Fixed);\n\
                  macro_rules! w { () => { $crate::f() }; }\n\
                  let path = \"x\";\n\
                  #[derive(Debug)]\nstruct S;\n\
                  fn f(print: u8) -> u8 { print }\n";

    assert_eq!(names(source), []);
}

#[test]
fn gates_every_test_module_accepts_only_cfg_test_directly_above() {
    for (source, gated) in [
        ("#[cfg(test)]\nmod tests;\n", true),
        ("mod a;\n#[cfg(test)]\nmod tests;\n", true),
        ("mod tests;\n", false),
        ("#[cfg(test)]\nmod a;\nmod tests;\n", false),
        ("#[cfg(not(test))]\nmod tests;\n", false),
        ("#[cfg(test)]\npub mod tests;\n", false),
        (
            "mod tests;\nmod inner {\n#[cfg(test)]\nmod tests;\n}\n",
            false,
        ),
        ("mod other;\n", false),
    ] {
        assert_eq!(
            gates_every_test_module(&tokens(source).unwrap()),
            gated,
            "{source}"
        );
    }
}

fn member(root: &Path, name: &str, files: &[(&str, &str)]) -> Member {
    let dir = root.join(name);
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
        target_kinds: vec!["lib".into(), "test".into()],
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

fn source(file: &str, line: usize, name: &str, kind: Kind) -> Violation {
    Violation::ForbiddenSource {
        file: PathBuf::from(file),
        line,
        name: name.into(),
        reason: forbidden(name, kind).unwrap(),
    }
}

#[test]
fn scan_reports_each_forbidden_name_and_skips_only_gated_test_modules() {
    let workspace = workspace("pure-source", |root| {
        vec![
            member(
                root,
                "ironafs-xdr",
                &[
                    (
                        "src/lib.rs",
                        "mod codec;\n#[cfg(test)]\nmod tests;\nuse core::net::Ipv4Addr;\n",
                    ),
                    (
                        "src/codec.rs",
                        "// std::fs\nfn f() {\n    std::thread::spawn(g);\n}\n\
                         #[cfg(test)]\nmod tests;\n",
                    ),
                    ("src/tests.rs", "use std::fs;\n"),
                    ("src/codec/tests.rs", "use std::net::UdpSocket;\n"),
                    ("src/codec/tests/helper.rs", "use std::net::UdpSocket;\n"),
                ],
            ),
            member(
                root,
                "ironafs-proto",
                &[
                    ("src/lib.rs", "mod tests;\n"),
                    ("src/tests.rs", "use std::fs;\n"),
                ],
            ),
            Member {
                name: "ironafs-net".into(),
                id: "ironafs-net".into(),
                dir: root.join("ironafs-net"),
                lib_root: Some(root.join("missing/lib.rs")),
                target_kinds: vec!["lib".into()],
            },
            Member {
                name: "ironafs-config".into(),
                id: "ironafs-config".into(),
                dir: root.join("ironafs-config"),
                lib_root: None,
                target_kinds: vec!["bin".into(), "custom-build".into()],
            },
            member(
                root,
                "ironafs-client",
                &[("src/lib.rs", "use std::thread;\nuse std::net::UdpSocket;\n")],
            ),
            member(root, "ironafs-time", &[("src/lib.rs", "use std as s;\n")]),
            member(root, "ironafs-vfs", &[("src/lib.rs", "\"unterminated\n")]),
        ]
    });

    let (violations, scanned) = scan(&workspace).unwrap();

    assert_eq!(
        scanned,
        [
            PathBuf::from("ironafs-xdr/src/codec.rs"),
            PathBuf::from("ironafs-xdr/src/lib.rs"),
            PathBuf::from("ironafs-proto/src/lib.rs"),
            PathBuf::from("ironafs-proto/src/tests.rs"),
            PathBuf::from("ironafs-client/src/lib.rs"),
            PathBuf::from("ironafs-time/src/lib.rs"),
            PathBuf::from("ironafs-vfs/src/lib.rs"),
        ]
    );
    assert_eq!(
        violations,
        [
            Violation::ForbiddenSource {
                file: PathBuf::from("ironafs-xdr/src/codec.rs"),
                line: 3,
                name: "std::thread::spawn".into(),
                reason: forbidden("std::thread", Kind::Pure).unwrap(),
            },
            source("ironafs-proto/src/tests.rs", 1, "std::fs", Kind::Pure),
            Violation::ShippedTarget {
                name: "ironafs-config".into(),
                target: "bin".into(),
            },
            Violation::ShippedTarget {
                name: "ironafs-config".into(),
                target: "custom-build".into(),
            },
            Violation::NoLibrary("ironafs-config".into()),
            source(
                "ironafs-client/src/lib.rs",
                2,
                "std::net::UdpSocket",
                Kind::Composition
            ),
            source("ironafs-time/src/lib.rs", 1, "std", Kind::Pure),
            Violation::Unreadable {
                file: PathBuf::from("ironafs-vfs/src/lib.rs"),
                error: "line 1: unterminated string".into(),
            },
        ]
    );
    fs::remove_dir_all(&workspace.root).unwrap();
}

#[test]
fn scan_fails_a_scanned_crate_with_no_library_source() {
    let workspace = workspace("pure-empty", |root| {
        vec![member(root, "ironafs-xdr", &[("src/notes.txt", "")])]
    });

    let (violations, scanned) = scan(&workspace).unwrap();

    assert_eq!(violations, [Violation::NoSource("ironafs-xdr".into())]);
    assert_eq!(scanned, [] as [PathBuf; 0]);
    fs::remove_dir_all(&workspace.root).unwrap();
}
