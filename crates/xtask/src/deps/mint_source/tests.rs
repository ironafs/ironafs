// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for finding the capability mint's name in a file's tokens.

use super::{MAY_NAME, mint_lines};
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
