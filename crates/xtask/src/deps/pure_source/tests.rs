// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for what the source of a pure crate and of `ironafs-client` may
//! not name.

use std::fs;
use std::path::{Path, PathBuf};

use super::{forbidden, gates_every_test_module, hidden_names, scan};
use crate::deps::Violation;
use crate::metadata::{Member, Workspace};
use crate::rust_source::tokens;

#[test]
fn forbidden_names_io_clocks_paths_and_aliases_but_not_values_or_threads() {
    for path in [
        "std::net",
        "std::net::UdpSocket::bind",
        "std::net::*",
        "std::fs::read",
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
        "std::path",
        "std::path::*",
        "std::path::Path",
        "std::path::PathBuf::from",
        "std::path::absolute",
        "std::backtrace::Backtrace::capture",
        "std::*",
        "std",
    ] {
        assert!(forbidden(path).is_some(), "{path} should be forbidden");
    }
    for path in [
        "std::time::Duration",
        "std::io::Read",
        "std::io::Error",
        "std::fmt",
        "std::netlink",
        "std::collections::BTreeMap",
        "std::thread",
        "std::thread::spawn",
        "std::sync::Mutex",
    ] {
        assert_eq!(forbidden(path), None, "{path} should be allowed");
    }
}

fn names(source: &str) -> Vec<(usize, String)> {
    hidden_names(&tokens(source).unwrap())
        .into_iter()
        .map(|(line, name, _)| (line, name))
        .collect()
}

const HIDDEN: &str = "thread::sleep(d);\n\
                      let x = r.recv_timeout(d);\n\
                      let m = HashMap::new();\n\
                      let n: HashSet<u8, Fixed> = HashSet::with_hasher(Fixed);\n\
                      let s: RandomState = h;\n\
                      println!(\"x\");\n\
                      macro_rules! v { ($i:ident) => { $i::fs::read(p) }; }\n\
                      include!(\"other.rs\");\n\
                      #[cfg_attr(test, path = \"other.rs\")]\nmod other;\n\
                      #![path = \"x\"]\n\
                      unsafe extern \"C\" { safe fn getpid() -> i32; }\n\
                      core::arch::global_asm!(\"nop\");\n";

#[test]
fn hidden_names_find_what_reaches_the_system_without_a_std_path() {
    assert_eq!(
        names(HIDDEN),
        [
            (1, "sleep".into()),
            (2, "recv_timeout".into()),
            (3, "HashMap".into()),
            (4, "HashSet".into()),
            (4, "HashSet".into()),
            (5, "RandomState".into()),
            (6, "println!".into()),
            (7, "$i::".into()),
            (8, "include!".into()),
            (9, "#[path]".into()),
            (11, "#[path]".into()),
            (12, "unsafe".into()),
            (12, "extern".into()),
            (13, "global_asm!".into()),
        ]
    );
}

#[test]
fn hidden_names_leave_ordinary_code_alone() {
    let source = "let a = entry.metadata;\n\
                  if node.exists() { node.metadata() }\n\
                  let c: BTreeMap<u8, u8> = BTreeMap::new();\n\
                  macro_rules! w { () => { $crate::f() }; }\n\
                  let path = \"x\";\n\
                  extern crate alloc;\n\
                  #[derive(Debug)]\nstruct S;\n\
                  fn f(print: u8) -> u8 { print }\n\
                  let r#unsafe = 1;\nlet r#extern = r#unsafe;\n";

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

fn source(file: &str, line: usize, name: &str) -> Violation {
    Violation::ForbiddenSource {
        file: PathBuf::from(file),
        line,
        name: name.into(),
        reason: forbidden(name).unwrap(),
    }
}

fn macro_path(file: &str, line: usize, name: &str) -> Violation {
    Violation::ForbiddenSource {
        file: PathBuf::from(file),
        line,
        name: name.into(),
        reason: super::MACRO_PATH,
    }
}

#[test]
fn scan_reads_only_ironafs_client_and_skips_only_gated_test_modules() {
    let workspace = workspace("client-source", |root| {
        vec![
            member(
                root,
                "ironafs-client",
                &[
                    (
                        "src/lib.rs",
                        "mod codec;\n#[cfg(test)]\nmod tests;\nuse std::thread;\n",
                    ),
                    (
                        "src/codec.rs",
                        "// std::fs\nfn f(p: &P) -> bool {\n    std::path::Path::new(p).exists()\n}\n\
                         #[cfg(test)]\nmod tests;\n\
                         use crate::report::print;\nuse self::inner::asm;\nuse core::arch::asm as go;\n",
                    ),
                    ("src/tests.rs", "use std::fs;\n"),
                    ("src/codec/tests.rs", "use std::net::UdpSocket;\n"),
                    ("src/codec/tests/helper.rs", "use std::net::UdpSocket;\n"),
                ],
            ),
            member(
                root,
                "ironafs-xdr",
                &[("src/lib.rs", "extern crate std;\nuse std::fs;\n")],
            ),
        ]
    });

    let (violations, scanned) = scan(&workspace).unwrap();

    assert_eq!(
        scanned,
        [
            PathBuf::from("ironafs-client/src/codec.rs"),
            PathBuf::from("ironafs-client/src/lib.rs"),
        ]
    );
    assert_eq!(
        violations,
        [
            source("ironafs-client/src/codec.rs", 3, "std::path::Path::new"),
            macro_path("ironafs-client/src/codec.rs", 9, "core::arch::asm"),
        ]
    );
    fs::remove_dir_all(&workspace.root).unwrap();
}

#[test]
fn scan_fails_ironafs_client_without_a_library_or_with_unreadable_source() {
    let broken = workspace("client-broken", |root| {
        vec![Member {
            name: "ironafs-client".into(),
            id: "ironafs-client".into(),
            dir: root.join("ironafs-client"),
            lib_root: None,
            target_kinds: vec!["bin".into(), "custom-build".into()],
        }]
    });
    let (violations, _) = scan(&broken).unwrap();
    assert_eq!(
        violations,
        [
            Violation::ShippedTarget {
                name: "ironafs-client".into(),
                target: "bin".into(),
            },
            Violation::ShippedTarget {
                name: "ironafs-client".into(),
                target: "custom-build".into(),
            },
            Violation::NoLibrary("ironafs-client".into()),
        ]
    );

    let unreadable = workspace("client-unreadable", |root| {
        vec![member(
            root,
            "ironafs-client",
            &[("src/lib.rs", "\"unterminated\n")],
        )]
    });
    let (violations, _) = scan(&unreadable).unwrap();
    assert_eq!(
        violations,
        [Violation::Unreadable {
            file: PathBuf::from("ironafs-client/src/lib.rs"),
            error: "line 1: unterminated string".into(),
        }]
    );
    fs::remove_dir_all(&unreadable.root).unwrap();
}

#[test]
fn scan_fails_ironafs_client_with_no_library_source() {
    let workspace = workspace("client-empty", |root| {
        vec![member(root, "ironafs-client", &[("src/notes.txt", "")])]
    });

    let (violations, scanned) = scan(&workspace).unwrap();

    assert_eq!(violations, [Violation::NoSource("ironafs-client".into())]);
    assert_eq!(scanned, [] as [PathBuf; 0]);
    fs::remove_dir_all(&workspace.root).unwrap();
}
