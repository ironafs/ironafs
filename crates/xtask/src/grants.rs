// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Confinement of the AFS-3 rights evaluator (ADR 0004, N28, point 2).

mod self_test;

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::metadata::{self, Workspace};
use crate::rust_source::{self, Token, TokenKind};

#[cfg(test)]
mod tests;

/// Where the evaluator lives and which names only it and the codecs use.
#[derive(Debug)]
pub(crate) struct Confinement {
    /// Names (types and functions) of the evaluator that no other code may
    /// use.
    guarded: &'static [&'static str],
    /// The crate that holds the evaluator. Until it is a workspace member,
    /// only the absence of the guarded names is checked.
    home_crate: &'static str,
    /// The evaluator module's file, relative to the workspace root.
    evaluator: &'static str,
    /// Files, and directories ending in `/`, where the guarded names may
    /// appear: the evaluator module, its children and the codecs.
    allowed: &'static [&'static str],
}

/// The AFS-3 access-mask evaluator and the paths it is confined to. The
/// change that writes the evaluator sets the final names and paths here.
const CONFINEMENT: Confinement = Confinement {
    guarded: &["Afs3Rights", "grants_from_afs3"],
    home_crate: "ironafs-proto",
    evaluator: "crates/ironafs-proto/src/grants.rs",
    allowed: &[
        "crates/ironafs-proto/src/grants.rs",
        "crates/ironafs-proto/src/grants/",
        "crates/ironafs-proto/src/codec/",
    ],
};

/// Keywords that introduce a definition of the name after them.
const DEFINING: &[&str] = &[
    "struct", "enum", "union", "type", "trait", "fn", "const", "static",
];

/// One broken confinement rule.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    NothingScanned,
    Outside {
        file: PathBuf,
        line: usize,
        name: &'static str,
    },
    Renamed {
        file: PathBuf,
        line: usize,
        name: &'static str,
    },
    EvaluatorMissing(PathBuf),
    NotDefined(&'static str),
    Unreadable {
        file: PathBuf,
        error: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingScanned => write!(
                formatter,
                "no Rust file was read in the workspace, so nothing was checked"
            ),
            Self::Outside { file, line, name } => write!(
                formatter,
                "{}:{line}: names {name}, which only the AFS-3 rights evaluator and the codecs \
                 may use (ADR 0004, N28, point 2)",
                file.display()
            ),
            Self::Renamed { file, line, name } => write!(
                formatter,
                "{}:{line}: gives {name} another name, which would carry it outside the \
                 evaluator unguarded (ADR 0004, N28, point 2)",
                file.display()
            ),
            Self::EvaluatorMissing(file) => write!(
                formatter,
                "{} does not exist: update the confinement in crates/xtask/src/grants.rs \
                 to the evaluator's module",
                file.display()
            ),
            Self::NotDefined(name) => write!(
                formatter,
                "{name} is not defined in the evaluator's allowed paths: update the guarded \
                 names in crates/xtask/src/grants.rs"
            ),
            Self::Unreadable { file, error } => {
                write!(
                    formatter,
                    "{}: cannot read as Rust: {error}",
                    file.display()
                )
            }
        }
    }
}

/// Whether a path relative to the workspace root may name the guarded
/// items. Compared by whole components, whatever the platform's separator.
fn is_allowed(confinement: &Confinement, path: &Path) -> bool {
    let components: Vec<_> = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect();
    let path = components.join("/");
    confinement.allowed.iter().any(|allowed| {
        if allowed.ends_with('/') {
            path.starts_with(allowed)
        } else {
            path == *allowed
        }
    })
}

/// Reads every Rust file of every workspace crate and returns the
/// violations and the number of files read.
///
/// # Errors
///
/// Returns an error if a directory cannot be listed or a file read.
fn check(
    workspace: &Workspace,
    confinement: &Confinement,
) -> Result<(Vec<Violation>, usize), Error> {
    let mut files = Vec::new();
    for member in &workspace.members {
        // Build output is not source; everything else is read.
        rust_source::files(
            &member.dir,
            &|path| path == workspace.target_dir,
            &mut files,
        )?;
    }
    let mut violations = Vec::new();
    let mut defined = Vec::new();
    for file in &files {
        let shown = file.strip_prefix(&workspace.root).unwrap_or(file);
        let tokens = match rust_source::read(file)? {
            Ok(tokens) => tokens,
            Err(error) => {
                violations.push(Violation::Unreadable {
                    file: shown.to_path_buf(),
                    error,
                });
                continue;
            }
        };
        if is_allowed(confinement, shown) {
            check_allowed(confinement, shown, &tokens, &mut defined, &mut violations);
        } else {
            for token in &tokens {
                if let TokenKind::Ident(word) = &token.kind
                    && let Some(name) = confinement.guarded.iter().find(|name| **name == word)
                {
                    violations.push(Violation::Outside {
                        file: shown.to_path_buf(),
                        line: token.line,
                        name,
                    });
                }
            }
        }
    }
    if files.is_empty() {
        violations.push(Violation::NothingScanned);
    }
    let home_present = workspace
        .members
        .iter()
        .any(|member| member.name == confinement.home_crate);
    if home_present {
        if workspace.root.join(confinement.evaluator).is_file() {
            violations.extend(
                confinement
                    .guarded
                    .iter()
                    .filter(|name| !defined.contains(name))
                    .map(|name| Violation::NotDefined(name)),
            );
        } else {
            violations.push(Violation::EvaluatorMissing(PathBuf::from(
                confinement.evaluator,
            )));
        }
    }
    Ok((violations, files.len()))
}

/// In a file where the guarded names may appear, records each one that is
/// defined, and reports each one given another name: by `as`, or inside a
/// type alias outside an `impl` or `trait` block.
fn check_allowed(
    confinement: &Confinement,
    shown: &Path,
    tokens: &[Token],
    defined: &mut Vec<&'static str>,
    violations: &mut Vec<Violation>,
) {
    // Whether each open brace began an `impl` or `trait` block.
    let mut blocks: Vec<bool> = Vec::new();
    let mut item_is_impl = false;
    let mut in_alias = false;
    let mut previous: Option<&str> = None;
    for (index, token) in tokens.iter().enumerate() {
        match &token.kind {
            TokenKind::Punct('{') => {
                blocks.push(item_is_impl);
                item_is_impl = false;
            }
            TokenKind::Punct('}') => {
                blocks.pop();
                item_is_impl = false;
            }
            TokenKind::Punct(';') => {
                in_alias = false;
                item_is_impl = false;
            }
            TokenKind::Punct('=') if previous_alias(tokens, index) => {
                in_alias = !blocks.iter().any(|is_impl| *is_impl);
            }
            TokenKind::Ident(word) if word == "impl" || word == "trait" => item_is_impl = true,
            TokenKind::Ident(word) => {
                if let Some(name) = confinement.guarded.iter().find(|name| **name == word) {
                    let renamed = matches!(
                        tokens.get(index + 1).map(|next| &next.kind),
                        Some(TokenKind::Ident(next)) if next == "as"
                    );
                    if renamed || in_alias {
                        violations.push(Violation::Renamed {
                            file: shown.to_path_buf(),
                            line: token.line,
                            name,
                        });
                    } else if previous.is_some_and(|keyword| DEFINING.contains(&keyword)) {
                        defined.push(*name);
                    }
                }
            }
            _ => {}
        }
        previous = match &token.kind {
            TokenKind::Ident(word) => Some(word),
            _ => None,
        };
    }
}

/// Whether the `=` at `index` ends the head of a type alias: `type Name =`
/// or `type Name<...> =`.
fn previous_alias(tokens: &[Token], index: usize) -> bool {
    let mut depth = 0_usize;
    let mut at = index;
    while let Some(before) = at.checked_sub(1) {
        at = before;
        match tokens.get(at).map(|token| &token.kind) {
            Some(TokenKind::Punct('>')) => depth += 1,
            Some(TokenKind::Punct('<')) => depth = depth.saturating_sub(1),
            Some(TokenKind::Ident(word)) if depth == 0 && word == "type" => return true,
            Some(TokenKind::Ident(_)) if depth > 0 => {}
            Some(TokenKind::Ident(_)) => {
                // The alias's own name: the keyword must come right before.
                return at
                    .checked_sub(1)
                    .and_then(|keyword| tokens.get(keyword))
                    .is_some_and(
                        |token| matches!(&token.kind, TokenKind::Ident(word) if word == "type"),
                    );
            }
            Some(_) if depth > 0 => {}
            _ => return false,
        }
    }
    false
}

/// Checks this repository's workspace, or with `self_test` proves on
/// planted fixtures that each rule is caught.
///
/// # Errors
///
/// Returns the violations found, or why the check could not run.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    if self_test {
        return self_test::run(&workspace.target_dir, output);
    }
    let (violations, scanned) = check(&workspace, &CONFINEMENT)?;
    if !violations.is_empty() {
        return Err(Error::Rules {
            check: "check-grants",
            violations: violations.iter().map(ToString::to_string).collect(),
        });
    }
    let home_present = workspace
        .members
        .iter()
        .any(|member| member.name == CONFINEMENT.home_crate);
    let state = if home_present {
        format!(
            "the evaluator is confined to {}",
            CONFINEMENT.allowed.join(", ")
        )
    } else {
        format!(
            "{} is not in the workspace yet, so no file may name them",
            CONFINEMENT.home_crate
        )
    };
    writeln!(
        output,
        "check-grants: {scanned} Rust files read in {} crates; {} named only where allowed; {state}",
        workspace.members.len(),
        CONFINEMENT.guarded.join(", "),
    )
    .map_err(Error::io("report check-grants"))
}
