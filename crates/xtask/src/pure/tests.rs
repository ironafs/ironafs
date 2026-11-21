// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the pure-crate rules that need no compiler.

use std::fs;
use std::path::{Path, PathBuf};

use super::{
    Violation, closure_violations, config_files, config_words, extern_lines, first_error,
    library_names, root_no_std, toolchain_targets,
};
use crate::metadata::{Member, Package, Workspace};
use crate::rust_source;

#[test]
fn toolchain_targets_reads_the_targets_array() {
    let text = "[toolchain]\nchannel = \"1.100.0\"\n# targets = [\"commented\"]\n\
                targets = [\n  \"x86_64-unknown-linux-gnu\", # first\n  \"aarch64-apple-darwin\",\n]\n\
                after = [\"not-a-target\"]\n";
    assert_eq!(
        toolchain_targets(text).unwrap(),
        ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]
    );
    assert_eq!(
        toolchain_targets("targets = [\"a\", \"b\"]\n").unwrap(),
        ["a", "b"]
    );
    assert!(toolchain_targets("[toolchain]\nchannel = \"1\"\n").is_err());
    assert!(toolchain_targets("targets = []\n").is_err());
}

#[test]
fn toolchain_targets_reads_this_repository_s_six_targets() {
    let text = include_str!("../../../../rust-toolchain.toml");
    assert_eq!(toolchain_targets(text).unwrap().len(), 6);
}

#[test]
fn config_words_find_flags_compilers_and_environment_anywhere() {
    let text = b"[alias]\nxtask = \"run --package xtask --\"\n\
                 [build]\nrustflags = [\"-C\", \"x\"]\n\
                 [target.'cfg(unix)']\nrustflags = []\n\
                 rustc-wrapper = \"w\"\nrustc-workspace-wrapper = \"w\"\nrustc = \"r\"\n\
                 [env]\nenv.X = \"1\"\nenv = { X = \"1\" }\n\
                 # rustflags only in a comment\n\
                 environment = 1\nCARGO_ENV = 1\nmy-env = 1\n";
    let found: Vec<(usize, &str)> = config_words(text)
        .into_iter()
        .map(|(line, word, _)| (line, word))
        .collect();
    assert_eq!(
        found,
        [
            (4, "rustflags"),
            (6, "rustflags"),
            (7, "rustc"),
            (8, "rustc"),
            (9, "rustc"),
            (10, "env"),
            (11, "env"),
            (12, "env"),
            (13, "rustflags"),
            (16, "env"),
        ]
    );
}

#[test]
fn extern_lines_allow_only_the_alloc_line() {
    let text = b"#![no_std]\n  extern crate alloc;  \r\n\
                 extern crate std;\n\
                 #[cfg(not(debug_assertions))] extern crate std;\n\
                 extern crate alloc as std;\n\
                 extern crate alloc; extern crate std;\n\
                 unsafe extern \"C\" { fn f(); }\n\
                 pub extern \"C\" fn g() {}\n\
                 // extern in a comment\n\
                 let r#extern = 1;\n\
                 extern\xe2\x80\xa8crate std;\n\
                 /// External, externally, my_extern, extern_x: none of these\n";
    assert_eq!(extern_lines(text), [3, 4, 5, 6, 7, 8, 9, 10, 11]);
}

fn root(source: &str) -> Option<usize> {
    root_no_std(&rust_source::tokens(source).unwrap())
}

#[test]
fn root_no_std_wants_the_unconditional_inner_attribute_at_the_start() {
    assert_eq!(root("//! Doc.\n#![no_std]\nextern crate alloc;\n"), Some(3));
    assert_eq!(root("#![doc = \"x\"]\n#![no_std] // why\n"), Some(9));
    assert_eq!(root("#![forbid(unsafe_code)]\n# ! [ no_std ]\n"), Some(11));
    assert_eq!(
        root("/* #![no_std] */\n#![cfg_attr(not(panic = \"abort\"), no_std)]\n"),
        None
    );
    assert_eq!(root("/*\n#![no_std]\n*/\n"), None);
    assert_eq!(root("#![cfg_attr(all(), no_std)]\n"), None);
    assert_eq!(root("const S: &str = \"\n#![no_std]\n\";\n"), None);
    assert_eq!(root("fn f() {}\n#![no_std]\n"), None);
    assert_eq!(root("macro_rules! m { () => { #![no_std] } }\n"), None);
    assert_eq!(root("#![no_std, no_main]\n"), None);
    assert_eq!(root("#![no_std\n"), None);
    assert_eq!(root(""), None);
}

fn names(source: &str, allowed: Option<usize>) -> Vec<(usize, String)> {
    library_names(&rust_source::tokens(source).unwrap(), allowed)
        .into_iter()
        .map(|(line, name, _)| (line, name))
        .collect()
}

#[test]
fn library_names_hold_for_every_cfg() {
    let source = "#![no_std]\n\
                  extern crate alloc;\n\
                  #[cfg(not(panic = \"abort\"))]\n\
                  extern crate alloc;\n\
                  #[cfg(panic = \"abort\")] extern crate std as hidden;\n\
                  #![cfg_attr(not(target_env = \"musl\"), no_std)]\n\
                  use alloc as std;\n\
                  include!(\"../ext.rs\");\n\
                  const T: &str = core::include_str!(\"t\");\n\
                  #[doc = include_bytes!(\"b\")] fn b() {}\n\
                  use core::include as inc;\n\
                  #[path = \"../p.rs\"] mod p;\n\
                  #[cfg_attr(all(), path = \"q.rs\")] mod q;\n\
                  macro_rules! m { ($a:meta) => { #[$a] mod x {} }; ($c:ident) => { $c!(\"x\") } }\n\
                  #[cfg(debug_assertions)] fn u() { unsafe { core::arch::asm!(\"nop\") } }\n\
                  core::arch::global_asm!(\"\");\n\
                  unsafe extern \"C\" { fn f(); }\n\
                  extern crate alloc as a;\n\
                  // std, no_std, include!, unsafe and extern in a comment\n\
                  const S: &str = \"std no_std include!(x) unsafe extern\";\n\
                  fn ok(path: &str, r#unsafe: u8, r#extern: u8) { let path = 1; }\n\
                  struct S2 { std_like: u8 }\n";
    assert_eq!(
        names(source, Some(3)),
        [
            (4, "extern".into()),
            (5, "extern".into()),
            (5, "std".into()),
            (6, "no_std".into()),
            (7, "std".into()),
            (8, "include!".into()),
            (9, "include_str!".into()),
            (10, "include_bytes!".into()),
            (11, "include".into()),
            (12, "#[path]".into()),
            (13, "#[path]".into()),
            (14, "#[$".into()),
            (14, "$c!".into()),
            (15, "unsafe".into()),
            (15, "asm!".into()),
            (16, "global_asm!".into()),
            (17, "unsafe".into()),
            (17, "extern".into()),
            (18, "extern".into()),
        ]
    );
    assert_eq!(names("#![no_std]\n", None), [(1, "no_std".into())]);
}

#[test]
fn library_names_refuse_macro_names_anywhere() {
    let source = "#![no_std]\n\
                  macro_rules! pull { ($bang:tt $name:ident) => { $name $bang(\"x\"); }; }\n\
                  pull!(! include);\n\
                  fn f(include_str: u8, include_bytes: u8) {}\n\
                  mod asm {}\n\
                  use core::arch::{global_asm as g, naked_asm};\n\
                  const R: u8 = r#include;\n\
                  fn ok(included: u8, asm_like: u8, my_include: u8) {}\n";
    assert_eq!(
        names(source, Some(3)),
        [
            (3, "include".into()),
            (4, "include_str".into()),
            (4, "include_bytes".into()),
            (5, "asm".into()),
            (6, "global_asm".into()),
            (6, "naked_asm".into()),
            (7, "include".into()),
        ]
    );
}

#[test]
fn library_names_refuse_a_hash_that_does_not_open_an_attribute() {
    let source = "#![no_std]\n\
                  macro_rules! load { ($group:tt) => { # $group mod hidden; }; }\n\
                  load!([path = \"x\"]);\n\
                  macro_rules! inner { ($group:tt) => { # ! $group }; }\n\
                  macro_rules! pass { ($($t:tt)*) => { $($t)* }; }\n\
                  pass!(#);\n\
                  # [inline] fn spaced() {}\n\
                  # ! [allow(dead_code)]\n\
                  const S: &str = r#\"#\"#;\n\
                  // # in a comment\n";
    assert_eq!(
        names(source, Some(3)),
        [(2, "#".into()), (4, "#".into()), (6, "#".into())]
    );
}

#[cfg(unix)]
#[test]
fn package_files_list_every_file_and_follow_no_link() {
    let root = std::env::temp_dir().join(format!("ironafs-xtask-package-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for path in [
        "ext.rs",
        "src/lib.rs",
        "src/data.bin",
        "tests/it.rs",
        "target/x.rs",
    ] {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
    }
    std::os::unix::fs::symlink(".", root.join("src/loop")).unwrap();
    std::os::unix::fs::symlink("../ext.rs", root.join("src/ext.rs")).unwrap();

    let (files, links) = super::package_files(&root, &root.join("target")).unwrap();

    assert_eq!(
        files,
        [
            root.join("ext.rs"),
            root.join("src/data.bin"),
            root.join("src/lib.rs"),
            root.join("tests/it.rs"),
        ]
    );
    assert_eq!(links, [root.join("src/ext.rs"), root.join("src/loop")]);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn first_error_prefers_the_compiler_s_own_line() {
    assert_eq!(
        first_error(
            b"warning: x\nerror[E0463]: can't find crate for `std`\nerror: could not compile `a`\n"
        ),
        "error[E0463]: can't find crate for `std`"
    );
    assert_eq!(
        first_error(b"error: could not compile `a`\n"),
        "error: could not compile `a`"
    );
    assert_eq!(first_error(b""), "cargo failed with no error line");
}

#[test]
fn config_files_skip_the_target_directory_git_and_nested_checkouts() {
    let root = std::env::temp_dir().join(format!("ironafs-xtask-configs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for (path, contents) in [
        (".cargo/config.toml", ""),
        ("crates/a/.cargo/config", ""),
        ("crates/a/.cargo/other.toml", ""),
        ("crates/a/config.toml", ""),
        ("target/.cargo/config.toml", ""),
        (".git/.cargo/config.toml", ""),
        ("nested/.git", "gitdir: x"),
        ("nested/.cargo/config.toml", ""),
    ] {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    let found = config_files(&root, &root.join("target")).unwrap();

    assert_eq!(
        found,
        [
            root.join(".cargo/config.toml"),
            root.join("crates/a/.cargo/config"),
        ]
    );
    fs::remove_dir_all(&root).unwrap();
}

fn package(name: &str, member: bool, dependencies: &[&str]) -> Package {
    Package {
        id: name.into(),
        name: name.into(),
        path: Some(PathBuf::from(format!("/w/crates/{name}"))),
        member,
        dependencies: dependencies.iter().map(|name| (*name).into()).collect(),
        target_kinds: vec!["lib".into()],
        links: None,
        features: Vec::new(),
    }
}

fn workspace(packages: Vec<Package>) -> Workspace {
    Workspace {
        root: PathBuf::from("/w"),
        target_dir: PathBuf::from("/w/target"),
        members: packages
            .iter()
            .filter(|package| package.member)
            .map(|package| Member {
                name: package.name.clone(),
                id: package.id.clone(),
                dir: PathBuf::from(format!("/w/crates/{}", package.name)),
                lib_root: Some(PathBuf::from(format!(
                    "/w/crates/{}/src/lib.rs",
                    package.name
                ))),
                target_kinds: package.target_kinds.clone(),
            })
            .collect(),
        packages,
    }
}

#[test]
fn closure_violations_keep_a_pure_crate_to_pure_crates_without_build_time_code() {
    let graph = workspace(vec![
        package("ironafs-rx", true, &["ironafs-xdr", "ironafs-client"]),
        Package {
            features: vec!["std".into()],
            ..package("ironafs-xdr", true, &["ironafs-time", "outside"])
        },
        Package {
            target_kinds: vec!["lib".into(), "custom-build".into()],
            links: Some("native".into()),
            ..package("ironafs-time", true, &[])
        },
        package("ironafs-client", true, &["deeper"]),
        Package {
            target_kinds: vec!["proc-macro".into()],
            ..package("outside", false, &[])
        },
        package("deeper", false, &[]),
    ]);
    let member = |name: &str| {
        graph
            .members
            .iter()
            .find(|member| member.name == name)
            .unwrap()
    };

    assert_eq!(
        closure_violations(&graph, member("ironafs-rx")),
        [
            Violation::Closure {
                from: "ironafs-rx".into(),
                via: vec![],
                to: "ironafs-client".into(),
            },
            Violation::Closure {
                from: "ironafs-rx".into(),
                via: vec!["ironafs-xdr".into()],
                to: "outside".into(),
            },
            Violation::Features {
                name: "ironafs-xdr".into(),
                features: vec!["std".into()],
            },
            Violation::Target {
                name: "ironafs-time".into(),
                target: "custom-build".into(),
            },
            Violation::Links {
                name: "ironafs-time".into(),
                links: "native".into(),
            },
            Violation::Target {
                name: "outside".into(),
                target: "proc-macro".into(),
            },
        ]
    );
    assert_eq!(closure_violations(&graph, member("ironafs-time")).len(), 2);
}

#[test]
fn violations_name_the_file_and_the_rule() {
    let shown = Violation::Extern {
        file: Path::new("crates/ironafs-xdr/src/lib.rs").into(),
        line: 2,
    }
    .to_string();
    assert!(shown.starts_with("crates/ironafs-xdr/src/lib.rs:2: says extern"));
    let shown = Violation::Build {
        name: "ironafs-xdr".into(),
        profile: "release",
        error: "error[E0463]".into(),
    }
    .to_string();
    assert!(shown.contains("release profile") && shown.contains("E0463"));
}
