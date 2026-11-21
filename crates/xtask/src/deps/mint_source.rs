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
//! - **Outside [`MAY_NAME`],** every Rust file of every workspace crate
//!   (library, binaries, build script, tests, benches, examples, compiled
//!   or not) is read as tokens, and any identifier spelt like the mint's
//!   type, raw or not, is refused. A rename (`as`), a glob import followed
//!   by the bare name, a path and a raw identifier each still spell it.
//!   Comments and string literals are not identifiers, so naming the mint
//!   in prose is allowed. So that no source enters unread, these crates
//!   may not use `include!` (which would splice a file that is not `.rs`,
//!   or a build script's output under the target directory), `#[path]`
//!   (also inside `cfg_attr`), an attribute built from a macro fragment, a
//!   `#` that opens no attribute, or a macro called through a fragment
//!   ([`hiding_lines`]).
//! - **In the allowed crates other than `ironafs-vfs`,** no public route
//!   may hand the mint out ([`exposed_lines`]): a public re-export,
//!   function or method, constant or static, type alias or field naming
//!   it, or an exported macro whose body names it.
//!
//! **Limits, which review holds:**
//! - in an allowed crate, a trait's items, a trait implementation (`Deref`
//!   to the mint, `From` or `Into` it) and an enum variant holding one are
//!   not read as routes;
//! - a procedural macro can build the identifier from a string. No
//!   third-party crate, procedural or not, enters without the owner's
//!   decision (the third-party lists), but a workspace procedural-macro
//!   crate's output is not seen here;
//! - only workspace members are read. A local crate outside the workspace
//!   is refused by the graph rules, and registry and git crates are
//!   third-party; the excluded `fuzz/` crate, which ships nothing, is not
//!   read.

use std::path::{Path, PathBuf};

use super::{Violation, pure_source};
use crate::Error;
use crate::metadata::Workspace;
use crate::pure::package_files;
use crate::rust_source::{self, Token, TokenKind, ident, punct};

#[cfg(test)]
mod tests;

/// The mint's type name, as source spells it.
const NAME: &str = "Mint";

/// The crate that defines the mint, and exposes it on purpose.
const DEFINES: &str = "ironafs-vfs";

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

/// The lines where source this check does not read could enter a file,
/// each with what was found: `include!` (named anywhere, since a macro can
/// join the name to its `!` from a fragment), `#[path]` (also inside
/// `cfg_attr`), an attribute built from a macro fragment, a `#` that does
/// not open an attribute, and a macro called through a fragment. These are
/// the findings of `ironafs-client`'s source check that bring in source;
/// `include_str!` and `include_bytes!` give a literal, never tokens, so
/// they are not among them.
pub(super) fn hiding_lines(tokens: &[Token]) -> Vec<(usize, String)> {
    pure_source::hidden_names(tokens)
        .into_iter()
        .filter(|(_, name, _)| {
            matches!(
                name.as_str(),
                "include" | "include!" | "#[path]" | "#[$" | "#"
            ) || (name.starts_with('$') && name.ends_with('!'))
        })
        .map(|(line, name, _)| (line, name))
        .collect()
}

/// The public routes by which an allowed crate would hand the mint to
/// code outside it, each with the line it starts on: a public re-export,
/// function or method signature, constant or static, type alias, or field
/// (named or tuple) whose tokens name the mint, and an exported macro whose
/// body does. A `pub(...)` restriction keeps an item inside the crate and
/// is not a route. Not seen here, so held by review: a trait's items and a
/// trait implementation (`Deref` to the mint, `From` or `Into` it), and an
/// enum variant that holds one.
pub(super) fn exposed_lines(tokens: &[Token]) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let next = tokens.get(index + 1);
        if ident(Some(token)) == Some("pub") && !token.raw && punct(next) != Some('(') {
            if let Some((kind, span)) = public_item(tokens, index + 1)
                && names_mint(span)
            {
                found.push((token.line, kind));
            }
        } else if punct(Some(token)) == Some('#')
            && punct(next) == Some('[')
            && ident(tokens.get(index + 2)) == Some("macro_export")
            && exported_macro_body(tokens, index + 3).is_some_and(names_mint)
        {
            found.push((token.line, "an exported macro"));
        }
    }
    found
}

/// What the item after a `pub` at `start` is, and the tokens that make up
/// what it exposes; `None` for an item whose own members carry their own
/// `pub` (a struct, an enum, a trait, a module, an implementation).
fn public_item(tokens: &[Token], start: usize) -> Option<(&'static str, &[Token])> {
    const QUALIFIERS: &[&str] = &["unsafe", "async", "extern", "safe"];
    let mut at = start;
    while ident(tokens.get(at)).is_some_and(|word| QUALIFIERS.contains(&word)) {
        at += 1;
    }
    let rest = tokens.get(at..)?;
    let word = ident(rest.first());
    let after = ident(rest.get(1));
    match word {
        Some("use") => Some(("a public re-export", until(rest, &[';']))),
        Some("fn") => Some(("a public function", until(rest, &['{', ';']))),
        Some("const") if after == Some("fn") || QUALIFIERS.iter().any(|q| after == Some(q)) => {
            Some(("a public function", until(rest, &['{', ';'])))
        }
        Some("const" | "static") => Some(("a public constant or static", until(rest, &[';']))),
        Some("type") => Some(("a public type alias", until(rest, &[';']))),
        Some(
            "struct" | "enum" | "trait" | "mod" | "impl" | "union" | "macro" | "crate" | "self"
            | "super" | "in",
        ) => None,
        _ => Some(("a public field", field(rest))),
    }
}

/// The tokens up to, not including, the first of `ends`.
fn until<'a>(tokens: &'a [Token], ends: &[char]) -> &'a [Token] {
    let end = tokens
        .iter()
        .position(|token| punct(Some(token)).is_some_and(|mark| ends.contains(&mark)))
        .unwrap_or(tokens.len());
    tokens.get(..end).unwrap_or(tokens)
}

/// A field's tokens: up to the `,` that ends it, or the `}` or `)` that
/// closes its struct, outside any brackets of its own type.
fn field(tokens: &[Token]) -> &[Token] {
    let mut depth = 0_usize;
    let mut previous = None;
    for (index, token) in tokens.iter().enumerate() {
        match punct(Some(token)) {
            Some('(' | '[' | '{' | '<') => depth += 1,
            // The `>` of `->` closes nothing.
            Some('>') if previous != Some('-') => depth = depth.saturating_sub(1),
            Some(',' | ')' | '}') if depth == 0 => return tokens.get(..index).unwrap_or(tokens),
            Some(')' | ']' | '}') => depth = depth.saturating_sub(1),
            _ => {}
        }
        previous = punct(Some(token));
    }
    tokens
}

/// The body of the `macro_rules!` that follows a `#[macro_export` at
/// `start`, between its outer brackets.
fn exported_macro_body(tokens: &[Token], start: usize) -> Option<&[Token]> {
    let rest = tokens.get(start..)?;
    let name = rest
        .iter()
        .position(|token| ident(Some(token)) == Some("macro_rules"))?;
    // `macro_rules`, `!`, the macro's name, then the opening bracket.
    let open = name.checked_add(3)?;
    let mut depth = 0_usize;
    for (index, token) in rest.iter().enumerate().skip(open) {
        match punct(Some(token)) {
            Some('(' | '[' | '{') => depth += 1,
            Some(')' | ']' | '}') => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return rest.get(open..=index);
                }
            }
            _ => {}
        }
    }
    rest.get(open..)
}

fn names_mint(tokens: &[Token]) -> bool {
    !mint_lines(tokens).is_empty()
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
        let name = member.name.as_str();
        if name == DEFINES {
            continue;
        }
        let allowed = MAY_NAME.contains(&name);
        for (shown, tokens) in rust_files(workspace, &member.dir, &mut violations)? {
            if allowed {
                violations.extend(exposed_lines(&tokens).into_iter().map(|(line, route)| {
                    Violation::ExposesMint {
                        file: shown.clone(),
                        line,
                        route,
                    }
                }));
                continue;
            }
            violations.extend(
                mint_lines(&tokens)
                    .into_iter()
                    .map(|line| Violation::NamesMint {
                        file: shown.clone(),
                        line,
                    }),
            );
            violations.extend(hiding_lines(&tokens).into_iter().map(|(line, what)| {
                Violation::HidesSource {
                    file: shown.clone(),
                    line,
                    what,
                }
            }));
        }
    }
    Ok(violations)
}

/// Every Rust file of the package in `dir`, as shown and as tokens. A file
/// that cannot be read as Rust, or a symbolic link to a Rust name that
/// leads to no file, is added to `violations` instead.
///
/// # Errors
///
/// Returns an error if a directory cannot be listed or a file cannot be
/// read.
fn rust_files(
    workspace: &Workspace,
    dir: &Path,
    violations: &mut Vec<Violation>,
) -> Result<Vec<(PathBuf, Vec<Token>)>, Error> {
    let (mut files, links) = package_files(dir, &workspace.target_dir)?;
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
    let mut read = Vec::new();
    for file in files {
        let shown = shown(workspace, &file);
        match rust_source::read(&file)? {
            Ok(tokens) => read.push((shown, tokens)),
            Err(error) => violations.push(Violation::Unreadable { file: shown, error }),
        }
    }
    Ok(read)
}

fn is_rust(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "rs")
}

fn shown(workspace: &Workspace, path: &Path) -> PathBuf {
    path.strip_prefix(&workspace.root)
        .unwrap_or(path)
        .to_path_buf()
}
