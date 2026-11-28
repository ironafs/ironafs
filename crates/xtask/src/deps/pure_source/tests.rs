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
                      core::arch::global_asm!(\"nop\");\n\
                      macro_rules! c { ($m:ident) => { $m!(\"../x.rs\") }; }\n\
                      macro_rules! a { ($a:meta) => { #[$a] mod x; }; }\n";

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
            (14, "$m!".into()),
            (15, "#[$".into()),
        ]
    );
}

#[test]
fn hidden_names_find_include_and_global_asm_anywhere() {
    let source = "macro_rules! pull { ($bang:tt $name:ident) => { $name $bang(\"x\"); }; }\n\
                  pull!(! include);\n\
                  pull!(! global_asm);\n\
                  fn f(include: u8) {}\n\
                  const R: u8 = r#include;\n\
                  fn ok(included: u8, asm: u8, naked_asm: u8) {}\n";
    assert_eq!(
        names(source),
        [
            (2, "include".into()),
            (3, "global_asm".into()),
            (4, "include".into()),
            (5, "include".into()),
        ]
    );
}

#[test]
fn hidden_names_find_the_file_reading_macros_anywhere() {
    let source = "const K: &str = include_str!(\"key\");\n\
                  const B: &[u8] = core::include_bytes!(\"b\");\n\
                  macro_rules! pull { ($bang:tt $name:ident) => { $name $bang(\"x\"); }; }\n\
                  pull!(! include_bytes);\n\
                  fn f(include_str: u8) {}\n\
                  const R: u8 = r#include_bytes;\n\
                  fn ok(include_string: u8, my_include_str: u8) {}\n";
    assert_eq!(
        names(source),
        [
            (1, "include_str!".into()),
            (2, "include_bytes!".into()),
            (4, "include_bytes".into()),
            (5, "include_str".into()),
            (6, "include_bytes".into()),
        ]
    );
}

#[test]
fn hidden_names_find_printing_macro_names_anywhere() {
    let source = "macro_rules! pull { ($bang:tt $name:ident) => { $name $bang(\"x\"); }; }\n\
                  pull!(! println);\n\
                  fn f(print: u8, eprint: u8) {}\n\
                  use crate::report::eprintln;\n\
                  let d = r#dbg;\n\
                  dbg!(x);\n\
                  fn ok(printed: u8, my_print: u8, dbg_like: u8) {}\n";
    assert_eq!(
        names(source),
        [
            (2, "println".into()),
            (3, "print".into()),
            (3, "eprint".into()),
            (4, "eprintln".into()),
            (5, "dbg".into()),
            (6, "dbg!".into()),
        ]
    );
}

#[test]
fn hidden_names_refuse_a_hash_that_does_not_open_an_attribute() {
    let source = "macro_rules! load { ($group:tt) => { # $group mod hidden; }; }\n\
                  load!([path = \"x\"]);\n\
                  macro_rules! inner { ($group:tt) => { # ! $group }; }\n\
                  pass!(#);\n\
                  # [inline] fn spaced() {}\n\
                  # ! [allow(dead_code)]\n\
                  const S: &str = r#\"#\"#;\n";
    assert_eq!(
        names(source),
        [(1, "#".into()), (3, "#".into()), (4, "#".into())]
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
                  fn f(printed: u8) -> u8 { printed }\n\
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
        targets: Vec::new(),
        target_kinds: vec!["lib".into(), "test".into()],
        editions: vec!["2024".into()],
        build_script: None,
        declarations: Vec::new(),
        features: Vec::new(),
        renamed: Vec::new(),
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
                         use crate::report::show;\nuse self::inner::asm;\nuse core::arch::asm as go;\n",
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
            targets: Vec::new(),
            target_kinds: vec!["bin".into(), "custom-build".into()],
            editions: vec!["2024".into()],
            build_script: None,
            declarations: Vec::new(),
            features: Vec::new(),
            renamed: Vec::new(),
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
fn scan_refuses_the_file_reading_macro_names_in_the_text_of_every_client_file() {
    let workspace = workspace("client-text", |root| {
        vec![
            member(
                root,
                "ironafs-client",
                &[
                    (
                        "src/lib.rs",
                        "#[cfg(test)]\nmod tests;\n// include! in a comment\n",
                    ),
                    ("src/tests.rs", "const K: &str = include_str!(\"k\");\n"),
                    ("tests/it/main.rs", "\n\nlet b = \"include_bytes\";\n"),
                    (
                        "notes.txt",
                        "included, my_include, include_strs: none of these\n",
                    ),
                ],
            ),
            member(
                root,
                "ironafs-xdr",
                &[("src/lib.rs", "include!(\"elsewhere.rs\");\n")],
            ),
        ]
    });

    let (violations, _) = scan(&workspace).unwrap();

    let in_text = |file: &str, line: usize, word: &'static str| Violation::NamedInText {
        file: PathBuf::from(file),
        line,
        word,
    };
    assert_eq!(
        violations,
        [
            in_text("ironafs-client/src/lib.rs", 3, "include"),
            in_text("ironafs-client/src/tests.rs", 1, "include_str"),
            in_text("ironafs-client/tests/it/main.rs", 3, "include_bytes"),
        ]
    );
    fs::remove_dir_all(&workspace.root).unwrap();
}

#[cfg(unix)]
#[test]
fn scan_reads_the_text_through_a_link_to_a_file_and_refuses_a_link_to_a_directory() {
    let workspace = workspace("client-links", |root| {
        vec![member(
            root,
            "ironafs-client",
            &[("src/lib.rs", "\n"), ("data/key.txt", "include_str\n")],
        )]
    });
    let client = workspace.root.join("ironafs-client");
    std::os::unix::fs::symlink("../data/key.txt", client.join("src/key.txt")).unwrap();
    std::os::unix::fs::symlink("../data", client.join("src/data")).unwrap();

    let (violations, _) = scan(&workspace).unwrap();

    assert_eq!(
        violations,
        [
            Violation::Unreadable {
                file: PathBuf::from("ironafs-client/src/data"),
                error: "a symbolic link to no file, which this check does not follow".into(),
            },
            Violation::NamedInText {
                file: PathBuf::from("ironafs-client/data/key.txt"),
                line: 1,
                word: "include_str",
            },
            Violation::NamedInText {
                file: PathBuf::from("ironafs-client/src/key.txt"),
                line: 1,
                word: "include_str",
            },
        ]
    );
    fs::remove_dir_all(&workspace.root).unwrap();
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
