// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for finding the capability mint's name in a file's tokens.

use super::{MAY_NAME, exposed_lines, hiding_lines, mint_lines};
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
