// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for finding the capability mint's name in a file's tokens.

use std::fs;
use std::path::{Path, PathBuf};

use super::{MAY_NAME, exposed_lines, hiding_lines, mint_lines, scan};
use crate::deps::Violation;
use crate::metadata::{Member, Workspace};
use crate::rust_source;

fn lines_of(source: &str) -> Vec<usize> {
    mint_lines(&rust_source::tokens(source).unwrap())
}

#[test]
fn mint_lines_find_the_name_in_a_use_a_path_a_rename_and_a_raw_identifier() {
    let source = "use ironafs_vfs::Mint;\n\
                  fn f() { let _ = ironafs_vfs::Mint::new(); }\n\
                  use ironafs_vfs::Mint as Maker;\n\
                  use ironafs_vfs::r#Mint;\n";
    assert_eq!(lines_of(source), vec![1, 2, 3, 4]);
}

#[test]
fn mint_lines_find_the_bare_name_after_a_glob() {
    assert_eq!(
        lines_of("use ironafs_vfs::*;\nfn f() { let _ = Mint::new(); }\n"),
        vec![2]
    );
}

#[test]
fn mint_lines_ignore_comments_strings_and_other_names() {
    let source = "// Mint\n/// Mint\nconst A: &str = \"Mint\";\nstruct Minted;\nfn mint() {}\n";
    assert_eq!(lines_of(source), Vec::<usize>::new());
}

#[test]
fn may_name_lists_the_definition_the_composition_the_core_and_the_test_crates() {
    assert_eq!(
        MAY_NAME,
        &[
            "ironafs-vfs",
            "ironafsd",
            "ironafs-client",
            "ironafs-sim",
            "ironafs-test-support",
        ]
    );
}

fn hiding_of(source: &str) -> Vec<(usize, String)> {
    hiding_lines(&rust_source::tokens(source).unwrap())
}

#[test]
fn hiding_lines_find_include_and_path_however_written() {
    let source = "include!(\"mint.inc\");\n\
                  #[path = \"mint.inc\"] mod a;\n\
                  #[cfg_attr(unix, path = \"mint.inc\")] mod b;\n\
                  macro_rules! pull { ($m:ident) => { $m!(\"mint.inc\"); } }\n\
                  pull!(include);\n\
                  #![path = \"x\"]\n";
    let found: Vec<usize> = hiding_of(source)
        .into_iter()
        .map(|(line, _)| line)
        .collect();
    assert_eq!(found, vec![1, 2, 3, 4, 5, 6]);
}

#[test]
fn hiding_lines_allow_literal_includes_and_other_attributes() {
    let source = "const A: &str = include_str!(\"a.txt\");\n\
                  const B: &[u8] = include_bytes!(\"b.bin\");\n\
                  #[derive(Debug)] struct S { path: u8 }\n\
                  // include!(\"x\") in a comment\n\
                  const C: &str = \"#[path = x] include!\";\n\
                  fn f(path: &str) -> &str { path }\n";
    assert_eq!(hiding_of(source), Vec::<(usize, String)>::new());
}

fn exposed_of(source: &str) -> Vec<(usize, &'static str)> {
    exposed_lines(&rust_source::tokens(source).unwrap())
}

#[test]
fn exposed_lines_find_each_public_route_to_the_mint() {
    let source = "pub use ironafs_vfs::Mint as Maker;\n\
                  pub fn make<A, B>(a: A, b: B) -> ironafs_vfs::Mint where A: Copy { todo() }\n\
                  impl S { pub const fn take(m: Mint) {} }\n\
                  pub const M: Option<Mint> = None;\n\
                  pub static N: Option<Mint> = None;\n\
                  pub type T = Mint;\n\
                  pub struct S { pub held: Vec<(u8, Mint)>, other: u8 }\n\
                  pub struct W(pub Mint);\n\
                  #[macro_export]\n\
                  macro_rules! mint { () => { $crate::Mint::new() }; }\n";
    assert_eq!(
        exposed_of(source),
        vec![
            (1, "a public re-export"),
            (2, "a public function"),
            (3, "a public function"),
            (4, "a public constant or static"),
            (5, "a public constant or static"),
            (6, "a public type alias"),
            (7, "a public field"),
            (8, "a public field"),
            (9, "an exported macro"),
        ]
    );
}

#[test]
fn exposed_lines_allow_the_mint_kept_inside_the_crate() {
    let source = "use ironafs_vfs::Mint;\n\
                  pub(crate) use ironafs_vfs::Mint as Inner;\n\
                  pub(crate) fn make() -> Mint { Mint::new() }\n\
                  fn private() -> Mint { Mint::new() }\n\
                  pub fn count(m: &Core) -> u64 { let _m: Mint = Mint::new(); 1 }\n\
                  pub struct Core { mint: Mint, pub size: u64 }\n\
                  macro_rules! local { () => { Mint::new() }; }\n\
                  #[macro_export]\n\
                  macro_rules! other { () => { 1 }; }\n";
    assert_eq!(exposed_of(source), Vec::<(usize, &'static str)>::new());
}

#[test]
fn may_name_holds_only_the_contract_the_composition_the_core_and_test_only_crates() {
    use crate::deps::{Kind, kind_of};
    let kinds: Vec<Option<Kind>> = MAY_NAME.iter().map(|name| kind_of(name)).collect();
    assert_eq!(
        kinds,
        vec![
            Some(Kind::Pure),
            Some(Kind::Binary),
            Some(Kind::Composition),
            Some(Kind::TestOnly),
            Some(Kind::TestOnly),
        ]
    );
}

/// The token reader drops literals, so an ABI string never stands between
/// the qualifiers and `fn`; a later private use of the mint is not read as
/// part of the function.
#[test]
fn exposed_lines_read_an_abi_function_as_a_function() {
    let source = "pub extern \"C\" fn callback() {}\n\
                  pub unsafe extern \"C\" fn other(x: u8) -> u8 { x }\n\
                  fn private() -> Mint { Mint::new() }\n\
                  pub extern \"C\" fn leaks() -> ironafs_vfs::Mint { todo() }\n";
    assert_eq!(exposed_of(source), vec![(4, "a public function")]);
}

#[test]
fn exposed_lines_read_a_tuple_field_whose_type_starts_with_a_path_separator() {
    assert_eq!(
        exposed_of("pub struct W(pub ::ironafs_vfs::Mint);\n"),
        vec![(1, "a public field")]
    );
}

/// A workspace member named `name` at `crates/<name>` under `root`, with
/// one library target whose root file is `lib`.
fn member_at(root: &Path, name: &str, lib: PathBuf) -> Member {
    Member {
        name: name.into(),
        id: name.into(),
        dir: root.join("crates").join(name),
        lib_root: Some(lib.clone()),
        target_kinds: vec!["lib".into()],
        editions: vec!["2024".into()],
        target_roots: vec![lib],
    }
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// rustc reads `mod hidden;` as `hidden/mod.rs` through a link to a
/// directory, and a target's root file may lie outside its package, so
/// both are reported rather than left unread.
#[cfg(unix)]
#[test]
fn scan_reports_a_directory_link_and_a_target_outside_the_package() {
    let root = std::env::temp_dir().join(format!("ironafs-xtask-mint-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let mint = "pub fn m() -> ironafs_vfs::Mint { ironafs_vfs::Mint::new() }\n";
    write(&root.join("other/hidden/mod.rs"), mint);
    write(&root.join("elsewhere/lib.rs"), mint);
    let linked = root.join("crates/ironafs-fuse");
    write(&linked.join("src/lib.rs"), "mod hidden;\n");
    std::os::unix::fs::symlink(root.join("other/hidden"), linked.join("src/hidden")).unwrap();
    write(&root.join("crates/ironafs-smb/src/lib.rs"), "");
    // An allowed crate's source outside its package would skip the check of
    // its public routes in the same way.
    write(&root.join("crates/ironafs-sim/src/lib.rs"), "");
    let workspace = Workspace {
        target_dir: root.join("target"),
        members: vec![
            member_at(&root, "ironafs-fuse", linked.join("src/lib.rs")),
            // Cargo gives a manifest's path as written, `..` and all.
            member_at(
                &root,
                "ironafs-smb",
                root.join("crates/ironafs-smb/../../elsewhere/lib.rs"),
            ),
            member_at(&root, "ironafs-sim", root.join("elsewhere/lib.rs")),
            member_at(&root, "ironafs-vfs", root.join("elsewhere/lib.rs")),
        ],
        packages: Vec::new(),
        root: root.clone(),
    };

    let violations = scan(&workspace).unwrap();

    assert_eq!(
        violations,
        [
            Violation::Unreadable {
                file: PathBuf::from("crates/ironafs-fuse/src/hidden"),
                error: "a symbolic link to a directory, which this check does not follow".into(),
            },
            Violation::TargetOutside {
                name: "ironafs-smb".into(),
                file: PathBuf::from("elsewhere/lib.rs"),
            },
            Violation::TargetOutside {
                name: "ironafs-sim".into(),
                file: PathBuf::from("elsewhere/lib.rs"),
            },
        ]
    );
    fs::remove_dir_all(&root).unwrap();
}
