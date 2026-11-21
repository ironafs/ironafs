// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Only the composition names the capability mint (the contract
//! specification, 5.11; it serves ADR 0004, C8, point 2).
//!
//! `ironafs_vfs::Mint` is the one constructor of caller and handle
//! capabilities. The core refuses every capability it did not record, so a
//! mint built elsewhere makes nothing the core accepts; this rule is the
//! second barrier, which keeps any other crate's code from even looking
//! like it mints. The type cannot hold it, so this check reads source.
//!
//! In every workspace crate outside [`MAY_NAME`], every Rust file of the
//! package (library, binaries, build script, tests, benches, examples,
//! compiled or not) is read as tokens, and any identifier spelt like the
//! mint's type, raw or not, is refused. That is stricter than refusing
//! paths from `ironafs_vfs`: a rename (`as`), a glob import followed by the
//! bare name, a path and a raw identifier each still spell it, and a crate
//! outside the list has no use for a type of its own under that name.
//! Comments and string literals are not identifiers, so naming the mint in
//! prose is allowed. A macro cannot build the identifier from pieces on
//! stable Rust. A crate on the list could re-export the type under another
//! name; review holds that, and a layer cannot depend on `ironafs-client`
//! or, outside its tests, on a test-only crate (the graph rules).

use std::path::{Path, PathBuf};

use super::Violation;
use crate::Error;
use crate::metadata::Workspace;
use crate::pure::package_files;
use crate::rust_source::{self, Token, TokenKind};

#[cfg(test)]
mod tests;

/// The mint's type name, as source spells it.
const NAME: &str = "Mint";

/// The crates that may name the mint: its definition, the composition
/// that creates the one mint, the core that holds it, and the test-only
/// crates that play the core.
pub(super) const MAY_NAME: &[&str] = &[
    "ironafs-vfs",
    "ironafsd",
    "ironafs-client",
    "ironafs-sim",
    "ironafs-test-support",
];

/// The lines on which an identifier spells the mint's name.
pub(super) fn mint_lines(tokens: &[Token]) -> Vec<usize> {
    tokens
        .iter()
        .filter(|token| matches!(&token.kind, TokenKind::Ident(word) if word == NAME))
        .map(|token| token.line)
        .collect()
}

/// Reads every Rust file of every workspace crate outside [`MAY_NAME`],
/// and reports each line that names the mint, and each file it cannot
/// read as Rust.
///
/// # Errors
///
/// Returns an error if a directory cannot be listed or a file cannot be
/// read.
pub(super) fn scan(workspace: &Workspace) -> Result<Vec<Violation>, Error> {
    let mut violations = Vec::new();
    for member in &workspace.members {
        if MAY_NAME.contains(&member.name.as_str()) {
            continue;
        }
        let (mut files, links) = package_files(&member.dir, &workspace.target_dir)?;
        for link in links {
            if !is_rust(&link) {
                continue;
            }
            if link.is_file() {
                files.push(link);
            } else {
                violations.push(Violation::Unreadable {
                    file: shown(workspace, &link),
                    error: "a symbolic link to no file, which this check does not follow".into(),
                });
            }
        }
        files.retain(|file| is_rust(file));
        files.sort();
        for file in files {
            let shown = shown(workspace, &file);
            match rust_source::read(&file)? {
                Ok(tokens) => violations.extend(mint_lines(&tokens).into_iter().map(|line| {
                    Violation::NamesMint {
                        file: shown.clone(),
                        line,
                    }
                })),
                Err(error) => violations.push(Violation::Unreadable { file: shown, error }),
            }
        }
    }
    Ok(violations)
}

fn is_rust(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "rs")
}

fn shown(workspace: &Workspace, path: &Path) -> PathBuf {
    path.strip_prefix(&workspace.root)
        .unwrap_or(path)
        .to_path_buf()
}
