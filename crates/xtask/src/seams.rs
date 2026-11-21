// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The seam rules the compiler cannot see (seams and scheduling
//! specification, 5.2, point 5, and 5.3, point 2), checked on the
//! workspace's own source as tokens.
//!
//! - **`pure_crate_calls_no_seam`.** A pure crate holds formats, decisions
//!   and plans, and calls no seam and holds no seam value. In each pure
//!   crate's library source, a seam trait's name (`Clock`, `Rng`, `Rpc`,
//!   `Disk`) appears only where its home crate defines it (`trait Name`),
//!   where its home crate's library root re-exports it (`pub use`), and as
//!   an enum variant the path resolves to: declared in an `enum` body;
//!   `Owner::Name` where `Owner` is an enum of this crate that declares it
//!   (alone, or after `crate`, `self`, `super` or a module), or a home
//!   crate's enum that declares it, reached as `home_crate::Owner` or
//!   imported by name with `use home_crate::...::Owner`; or `Self::Name` in
//!   a file that declares it. Anything else, a `::`-rooted path included,
//!   is refused, not exempt. So that a home crate's name always means the
//!   crate: a `use` or `extern crate` item naming a home crate with `as` or
//!   `*` is refused, an item named like a home crate is refused, and a pure
//!   crate's manifest may not rename a dependency (`package = ...`). A seam
//!   trait's body provides no method.
//! - **`deadline_built_from_a_fresh_sample`.** Every source of every crate
//!   builds a `Deadline` only inside its one named sampling function; in
//!   `ironafs-time` the constructors' names appear only where they are
//!   defined. Inside the sampling function: one fresh sample, bound by
//!   `let now = clock.sample().map_err(DeadlineError::Clock)?;` from a
//!   parameter whose type names `Clock`; each constructor called
//!   directly, after that binding, as
//!   `Deadline::boot_after(&now, from, ...)`, with `from` the function's
//!   own parameter; and the two names appear nowhere else in the body (no
//!   parameter, pattern, shadowing, use or assignment of either). The
//!   function holds no closure, no nested function, and no constructor
//!   inside a macro call. Every call of it passes, as its `from`, a
//!   request's recorded `.arrival` or a reply's `first_sent` unwrapped with
//!   `.as_ref()`. An allowance of `clippy::disallowed_methods` sits on no
//!   module, `impl` or trait of a crate that names a constructor, and on no
//!   other function that does.
//!
//! Both checks refuse a `#[path]` attribute and an `include!` anywhere,
//! so every compiled file is where the walk reads it, as what it is, and
//! none passes as test code under another name. `#[cfg(test)]` items, test modules their parent gates
//! (coding standards rule 15) and `tests/` directories are left out. Both
//! checks read source as written and expand no macro; the second trusts
//! that only the ingress writes a request's `arrival` and only the call
//! owner a reply's `first_sent` (5.3, point 2, "Limits"): review covers
//! both, and the self-test records them as passes. Clippy's ban on the two
//! constructors (`check-bans`) is the second net.

mod self_test;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::deps::{self, Kind};
use crate::metadata::{self, Workspace};
use crate::rust_source::{self, Token, TokenKind, ident, punct};

#[cfg(test)]
mod tests;

const CHECK: &str = "check-seams";

/// The four seams of ADR 0004, C3, point 3, each with its home crate.
const SEAMS: &[(&str, &str)] = &[
    ("Clock", "ironafs-time"),
    ("Rng", "ironafs-time"),
    ("Rpc", "ironafs-rx"),
    ("Disk", "ironafs-cache"),
];

/// The crate whose deadline constructors are checked, and the constructors.
const TIME_CRATE: &str = "ironafs-time";
const CONSTRUCTORS: &[&str] = &["boot_after", "awake_after"];

/// The lint the constructors would be listed under.
const ALLOWANCE: &str = "disallowed_methods";

/// A crate's one sampling function, named by its full path.
#[derive(Debug)]
pub(crate) struct Sampling {
    pub(crate) krate: &'static str,
    pub(crate) path: &'static str,
}

impl Sampling {
    fn name(&self) -> &'static str {
        self.path.rsplit("::").next().unwrap_or(self.path)
    }
}

/// The workspace's sampling functions: at most one per crate.
const SAMPLING: &[Sampling] = &[Sampling {
    krate: "ironafs-client",
    path: "ironafs_client::time::deadline_after",
}];

/// One source file, its test items removed.
#[derive(Debug)]
pub(crate) struct SourceFile {
    krate: String,
    path: PathBuf,
    pure: bool,
    lib_root: bool,
    tokens: Vec<Token>,
}

/// One broken seam rule.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    /// A seam named in a pure crate outside its definition and re-export.
    SeamInPure {
        file: PathBuf,
        line: usize,
        name: &'static str,
    },
    /// A renamed or glob import of a seam's home crate in a pure crate,
    /// which could bring a seam in under another name.
    HomeImport { file: PathBuf, line: usize },
    /// An item in a pure crate named like a seam's home crate, which a
    /// path could take for the crate.
    HomeShadow { file: PathBuf, line: usize },
    /// A pure crate's manifest renames a dependency (`package = ...`), so a
    /// crate could be named in source by any name.
    RenamedDependency { file: PathBuf, line: usize },
    /// An `include!` of source from another file.
    IncludeMacro { file: PathBuf, line: usize },
    /// A seam trait that provides a method.
    ProvidedMethod {
        file: PathBuf,
        line: usize,
        name: &'static str,
    },
    /// A constructor outside the crate's sampling function.
    Outside {
        file: PathBuf,
        line: usize,
        name: &'static str,
    },
    /// A constructor or sampling function used as a value, not called.
    AsValue {
        file: PathBuf,
        line: usize,
        name: String,
    },
    /// A sampling function given another name.
    Renamed { file: PathBuf, line: usize },
    /// A constructor inside a macro call.
    InMacro { file: PathBuf, line: usize },
    /// A closure or a nested function in a sampling function.
    Hides {
        file: PathBuf,
        line: usize,
        what: &'static str,
    },
    /// No single fresh-sample binding in the sampling function.
    NoFreshSample { file: PathBuf, line: usize },
    /// The fresh sample's binding is reassigned or shadowed.
    Reassigned { file: PathBuf, line: usize },
    /// A constructor whose `now` is not the fresh sample's binding.
    NowNotFresh { file: PathBuf, line: usize },
    /// A sampling function with no parameter named `from`.
    NoFrom { file: PathBuf, line: usize },
    /// A call whose `from` is not a recorded arrival or `first_sent`.
    BadFrom { file: PathBuf, line: usize },
    /// A constructor whose `from` is not the sampling function's `from`
    /// parameter.
    NotFromParameter { file: PathBuf, line: usize },
    /// A `#[path]` attribute, which can bring in a file the walk does not
    /// read as what it is.
    PathAttribute { file: PathBuf, line: usize },
    /// An allowance of the constructors' lint where it may not sit.
    Allowance {
        file: PathBuf,
        line: usize,
        on: &'static str,
    },
    /// A listed sampling function defined more than once in its crate.
    Duplicate { path: &'static str },
    /// A listed sampling function missing from its crate.
    Missing { path: &'static str },
    /// A source file the token reader could not read.
    Unreadable { file: PathBuf, error: String },
    /// Nothing was read.
    NothingScanned,
}

impl fmt::Display for Violation {
    #[expect(
        clippy::too_many_lines,
        reason = "one message per violation, in the enum's order"
    )]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let at = |formatter: &mut fmt::Formatter<'_>, file: &Path, line: &usize| {
            write!(formatter, "{}:{line}: ", file.display())
        };
        match self {
            Self::SeamInPure { file, line, name } => {
                at(formatter, file, line)?;
                write!(
                    formatter,
                    "a pure crate names the seam {name}: pure crates call no seam and hold no \
                     seam value (seams spec 5.2, point 5)"
                )
            }
            Self::HomeImport { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str(
                    "a pure crate renames or glob-imports a seam's home crate, which could name \
                     a seam out of this check's sight",
                )
            }
            Self::HomeShadow { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str(
                    "a pure crate declares an item named like a seam's home crate, which a path \
                     could take for the crate",
                )
            }
            Self::RenamedDependency { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str(
                    "a pure crate's manifest renames a dependency, so source could name a crate \
                     by any name",
                )
            }
            Self::IncludeMacro { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str(
                    "an `include!` brings in source from a file the walk does not read as what \
                     it is",
                )
            }
            Self::ProvidedMethod { file, line, name } => {
                at(formatter, file, line)?;
                write!(
                    formatter,
                    "the seam {name} provides a method: a seam is a trait with no logic"
                )
            }
            Self::Outside { file, line, name } => {
                at(formatter, file, line)?;
                write!(
                    formatter,
                    "Deadline::{name} outside the crate's named sampling function (seams spec \
                     5.3, point 2)"
                )
            }
            Self::AsValue { file, line, name } => {
                at(formatter, file, line)?;
                write!(formatter, "{name} used as a value, not called directly")
            }
            Self::Renamed { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str("a sampling function is given another name")
            }
            Self::InMacro { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str("a deadline constructor inside a macro call")
            }
            Self::Hides { file, line, what } => {
                at(formatter, file, line)?;
                write!(formatter, "a sampling function holds {what}")
            }
            Self::NoFreshSample { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str(
                    "a sampling function must bind one fresh sample as \
                     `let now = clock.sample().map_err(DeadlineError::Clock)?;`",
                )
            }
            Self::Reassigned { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str("the fresh sample's binding is reassigned or shadowed")
            }
            Self::NowNotFresh { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str(
                    "a deadline constructor's `now` must be `&` and the fresh sample's binding",
                )
            }
            Self::NoFrom { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str("a sampling function has no parameter named `from`")
            }
            Self::BadFrom { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str(
                    "a sampling function's `from` must be a request's recorded `.arrival` or a \
                     reply's `first_sent.as_ref()`",
                )
            }
            Self::NotFromParameter { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str(
                    "a deadline constructor's `from` must be the sampling function's own `from` \
                     parameter",
                )
            }
            Self::PathAttribute { file, line } => {
                at(formatter, file, line)?;
                formatter.write_str(
                    "a `#[path]` attribute: a module's file must be where the walk reads it, so \
                     no file escapes these checks or passes as test code",
                )
            }
            Self::Allowance { file, line, on } => {
                at(formatter, file, line)?;
                write!(
                    formatter,
                    "an allowance of clippy::{ALLOWANCE} on {on} of a crate that builds \
                     deadlines: it may sit only on the named sampling function"
                )
            }
            Self::Duplicate { path } => write!(formatter, "{path} is defined more than once"),
            Self::Missing { path } => {
                write!(
                    formatter,
                    "the listed sampling function {path} is not defined"
                )
            }
            Self::Unreadable { file, error } => {
                write!(
                    formatter,
                    "{}: not readable as Rust: {error}",
                    file.display()
                )
            }
            Self::NothingScanned => formatter.write_str("no source file was read"),
        }
    }
}

/// Reads `source` as one file of `krate`, with its test items removed.
///
/// # Errors
///
/// Returns why the source is not readable as Rust tokens.
pub(crate) fn source_file(
    krate: &str,
    path: &str,
    pure: bool,
    lib_root: bool,
    source: &str,
) -> Result<SourceFile, String> {
    let tokens = rust_source::tokens(source).map_err(|error| error.to_string())?;
    Ok(SourceFile {
        krate: krate.to_owned(),
        path: PathBuf::from(path),
        pure,
        lib_root,
        tokens: without_test_items(tokens),
    })
}

/// Each line of a pure crate's manifest that renames a dependency: a
/// `package` key, inline or in a dependency table, but not the
/// `[package]` header.
pub(crate) fn renamed_dependencies(path: &Path, manifest: &str) -> Vec<Violation> {
    manifest
        .lines()
        .enumerate()
        .filter(|(_, text)| {
            let code = text.split('#').next().unwrap_or("");
            let mut rest = code;
            // A `package` key: the word, then `=` after optional spaces,
            // at a line's start or after `{` or `,` in an inline table.
            while let Some(at) = rest.find("package") {
                let (head, tail) = rest.split_at(at);
                let before = head.trim_end();
                let tail = tail.get("package".len()..).unwrap_or("");
                let after = tail.trim_start();
                if (before.is_empty() || before.ends_with('{') || before.ends_with(','))
                    && after.starts_with('=')
                {
                    return true;
                }
                rest = tail;
            }
            false
        })
        .map(|(index, _)| Violation::RenamedDependency {
            file: path.to_path_buf(),
            line: index + 1,
        })
        .collect()
}

/// Runs both checks over `files`, with `sampling` as the sampling
/// functions.
pub(crate) fn check(files: &[SourceFile], sampling: &[Sampling]) -> Vec<Violation> {
    let mut violations = Vec::new();
    if files.is_empty() {
        violations.push(Violation::NothingScanned);
    }
    for file in files {
        for index in 0..file.tokens.len() {
            if attribute_end(&file.tokens, index).is_some() {
                let open = if is_punct(&file.tokens, index + 1, '!') {
                    index + 2
                } else {
                    index + 1
                };
                if is_ident(&file.tokens, open + 1, "path") {
                    violations.push(Violation::PathAttribute {
                        file: file.path.clone(),
                        line: line(&file.tokens, index),
                    });
                }
            }
            if is_ident(&file.tokens, index, "include") && is_punct(&file.tokens, index + 1, '!') {
                violations.push(Violation::IncludeMacro {
                    file: file.path.clone(),
                    line: line(&file.tokens, index),
                });
            }
        }
    }
    // The variants each seam's home crate declares, by the name source
    // gives that crate.
    let home_variants: BTreeMap<String, BTreeSet<(String, String)>> = SEAMS
        .iter()
        .map(|(_, home)| {
            let declared = files
                .iter()
                .filter(|file| file.krate == *home)
                .flat_map(|file| declared_variants(&file.tokens))
                .map(|(enumeration, variant, _)| (enumeration, variant))
                .collect();
            (home.replace('-', "_"), declared)
        })
        .collect();
    for file in files.iter().filter(|file| file.pure) {
        let local: BTreeSet<(String, String)> = files
            .iter()
            .filter(|other| other.krate == file.krate)
            .flat_map(|other| declared_variants(&other.tokens))
            .map(|(enumeration, variant, _)| (enumeration, variant))
            .collect();
        pure_crate_calls_no_seam(file, &local, &home_variants, &mut violations);
    }
    deadline_built_from_a_fresh_sample(files, sampling, &mut violations);
    violations
}

fn kind(tokens: &[Token], index: usize) -> Option<&TokenKind> {
    tokens.get(index).map(|token| &token.kind)
}

fn line(tokens: &[Token], index: usize) -> usize {
    tokens.get(index).map_or(0, |token| token.line)
}

fn is_ident(tokens: &[Token], index: usize, word: &str) -> bool {
    ident(tokens.get(index)) == Some(word)
}

fn is_punct(tokens: &[Token], index: usize, character: char) -> bool {
    punct(tokens.get(index)) == Some(character)
}

/// The index of the delimiter that closes the one at `open`.
fn closing(tokens: &[Token], open: usize) -> Option<usize> {
    let (opener, closer) = match punct(tokens.get(open))? {
        '(' => ('(', ')'),
        '[' => ('[', ']'),
        '{' => ('{', '}'),
        _ => return None,
    };
    let mut depth = 0_usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match &token.kind {
            TokenKind::Punct(character) if *character == opener => depth += 1,
            TokenKind::Punct(character) if *character == closer => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// The index just past the item that starts at `start`: its `;` at depth
/// zero, or the block that closes it.
fn item_end(tokens: &[Token], start: usize) -> usize {
    let mut index = start;
    while let Some(token) = tokens.get(index) {
        match &token.kind {
            TokenKind::Punct(';') => return index + 1,
            TokenKind::Punct('{' | '(' | '[') => {
                let Some(close) = closing(tokens, index) else {
                    return tokens.len();
                };
                if is_punct(tokens, index, '{') {
                    // A block ends the item unless a `;` follows directly,
                    // as after a `static` or `const` initialiser block.
                    return if is_punct(tokens, close + 1, ';') {
                        close + 2
                    } else {
                        close + 1
                    };
                }
                index = close + 1;
            }
            _ => index += 1,
        }
    }
    tokens.len()
}

/// The index just past an attribute that starts at `start` (`#[...]` or
/// `#![...]`), or `None` if none starts there.
fn attribute_end(tokens: &[Token], start: usize) -> Option<usize> {
    if !is_punct(tokens, start, '#') {
        return None;
    }
    let open = if is_punct(tokens, start + 1, '!') {
        start + 2
    } else {
        start + 1
    };
    if !is_punct(tokens, open, '[') {
        return None;
    }
    closing(tokens, open).map(|close| close + 1)
}

/// Whether the attribute that starts at `start` is `#[cfg(test)]` or
/// `#![cfg(test)]`.
fn is_cfg_test(tokens: &[Token], start: usize) -> bool {
    let open = if is_punct(tokens, start + 1, '!') {
        start + 2
    } else {
        start + 1
    };
    is_punct(tokens, start, '#')
        && is_punct(tokens, open, '[')
        && is_ident(tokens, open + 1, "cfg")
        && is_punct(tokens, open + 2, '(')
        && is_ident(tokens, open + 3, "test")
        && is_punct(tokens, open + 4, ')')
        && is_punct(tokens, open + 5, ']')
}

/// The tokens with every `#[cfg(test)]` item removed; none at all under a
/// file-level `#![cfg(test)]`.
fn without_test_items(tokens: Vec<Token>) -> Vec<Token> {
    let mut removed = vec![false; tokens.len()];
    let mut index = 0;
    while index < tokens.len() {
        if is_cfg_test(&tokens, index) {
            if is_punct(&tokens, index + 1, '!') {
                return Vec::new();
            }
            let mut end = attribute_end(&tokens, index).unwrap_or(tokens.len());
            while let Some(next) = attribute_end(&tokens, end) {
                end = next;
            }
            let end = item_end(&tokens, end);
            for flag in removed.iter_mut().take(end).skip(index) {
                *flag = true;
            }
            index = end;
        } else {
            index += 1;
        }
    }
    tokens
        .into_iter()
        .zip(removed)
        .filter_map(|(token, removed)| (!removed).then_some(token))
        .collect()
}

/// The seam a token names, if any.
fn seam(tokens: &[Token], index: usize) -> Option<(&'static str, &'static str)> {
    let word = ident(tokens.get(index))?;
    SEAMS.iter().copied().find(|(name, _)| *name == word)
}

/// The crate names of the seams' home crates, as source writes them.
fn home_crate_idents() -> impl Iterator<Item = String> {
    SEAMS.iter().map(|(_, home)| home.replace('-', "_"))
}

/// Keywords that can stand before a `::`-rooted path, so are never one of
/// its segments.
const KEYWORDS: &[&str] = &[
    "as", "async", "await", "box", "break", "const", "continue", "dyn", "else", "enum", "extern",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "static", "struct", "trait", "type", "union", "unsafe", "use", "where", "while",
];

/// Item keywords whose name a path could take for a crate's.
const NAMING: &[&str] = &[
    "mod", "enum", "struct", "union", "type", "trait", "fn", "const", "static",
];

/// The names a file imports from a seam's home crate by `use` (no `as`,
/// no glob: those are refused), each with that crate's source name.
fn home_imports(tokens: &[Token], homes: &[String]) -> BTreeMap<String, String> {
    let mut imports = BTreeMap::new();
    for index in 0..tokens.len() {
        if !is_ident(tokens, index, "use") {
            continue;
        }
        let start = if matches!(kind(tokens, index + 1), Some(TokenKind::PathSep)) {
            index + 2
        } else {
            index + 1
        };
        let Some(home) = ident(tokens.get(start)).filter(|word| homes.iter().any(|h| h == word))
        else {
            continue;
        };
        let end = item_end(tokens, index);
        for at in start + 1..end {
            if let Some(name) = ident(tokens.get(at))
                && matches!(punct(tokens.get(at + 1)), Some(',' | '}' | ';'))
            {
                imports.insert(name.to_owned(), home.to_owned());
            }
        }
    }
    imports
}

/// Whether the seam name at `index` resolves to an enum variant, not the
/// seam: `Owner::Name` where `Owner` is an enum that declares `Name` and
/// the path reaches it, through a seam's home crate by its own name, or
/// through this crate (a path from `crate`, `self`, `super` or a module,
/// or `Owner` alone where it is declared in this crate or imported from a
/// home crate). A `::`-rooted path whose first segment is the owner names a
/// crate, and anything else unresolvable is refused, not exempt.
fn resolves_to_variant(
    tokens: &[Token],
    index: usize,
    name: &str,
    local: &BTreeSet<(String, String)>,
    home_variants: &BTreeMap<String, BTreeSet<(String, String)>>,
    imports: &BTreeMap<String, String>,
    file_declares: &[(String, String, usize)],
) -> bool {
    let Some(owner) = variant_path_owner(tokens, index) else {
        return false;
    };
    let pair = (owner.to_owned(), name.to_owned());
    if owner == "Self" {
        return file_declares.iter().any(|(_, variant, _)| variant == name);
    }
    if index >= 3 && matches!(kind(tokens, index - 3), Some(TokenKind::PathSep)) {
        let segment = index
            .checked_sub(4)
            .and_then(|at| ident(tokens.get(at)))
            .filter(|word| !KEYWORDS.contains(word));
        return match segment {
            Some(segment) if home_variants.contains_key(segment) => home_variants
                .get(segment)
                .is_some_and(|declared| declared.contains(&pair)),
            Some(_) => local.contains(&pair),
            // `::Owner::Name`: the owner is a crate.
            None => false,
        };
    }
    local.contains(&pair)
        || imports.get(owner).is_some_and(|home| {
            home_variants
                .get(home)
                .is_some_and(|declared| declared.contains(&pair))
        })
}

fn pure_crate_calls_no_seam(
    file: &SourceFile,
    local: &BTreeSet<(String, String)>,
    home_variants: &BTreeMap<String, BTreeSet<(String, String)>>,
    violations: &mut Vec<Violation>,
) {
    let tokens = &file.tokens;
    // A `use` or `extern crate` item that names a home crate and renames
    // or globs: the only ways to name a seam without writing its name.
    let homes: Vec<String> = home_crate_idents().collect();
    for index in 0..tokens.len() {
        if !(is_ident(tokens, index, "use") || is_ident(tokens, index, "extern")) {
            continue;
        }
        let end = item_end(tokens, index);
        let names_home = (index..end).any(|at| {
            ident(tokens.get(at)).is_some_and(|word| homes.iter().any(|home| home == word))
        });
        let renames =
            (index..end).any(|at| is_ident(tokens, at, "as") || is_punct(tokens, at, '*'));
        if names_home && renames {
            violations.push(Violation::HomeImport {
                file: file.path.clone(),
                line: line(tokens, index),
            });
        }
    }
    // An item named like a home crate, which a path could take for it.
    for index in 1..tokens.len() {
        if ident(tokens.get(index)).is_some_and(|word| homes.iter().any(|home| home == word))
            && ident(tokens.get(index - 1)).is_some_and(|word| NAMING.contains(&word))
        {
            violations.push(Violation::HomeShadow {
                file: file.path.clone(),
                line: line(tokens, index),
            });
        }
    }
    let imports = home_imports(tokens, &homes);
    // The token ranges of `pub use ... ;` items of a home crate's root.
    let mut reexport = vec![false; tokens.len()];
    if file.lib_root {
        let mut index = 0;
        while index < tokens.len() {
            if is_ident(tokens, index, "pub") && is_ident(tokens, index + 1, "use") {
                let end = item_end(tokens, index);
                for flag in reexport.iter_mut().take(end).skip(index) {
                    *flag = true;
                }
                index = end;
            } else {
                index += 1;
            }
        }
    }
    let declared: Vec<(String, String, usize)> = declared_variants(tokens);
    for index in 0..tokens.len() {
        let Some((name, home)) = seam(tokens, index) else {
            continue;
        };
        // An enum variant with a seam's name, such as
        // `DeadlineError::Clock`, is not the seam: its declaration, a path
        // through an enum that declares it (in this crate or a seam's home
        // crate), or `Self::` in a file that declares it.
        let declaration = declared.iter().any(|(_, _, at)| *at == index);
        let through_enum = resolves_to_variant(
            tokens,
            index,
            name,
            local,
            home_variants,
            &imports,
            &declared,
        );
        if declaration || through_enum {
            continue;
        }
        let at_home = file.krate == home;
        let defined = at_home && index > 0 && is_ident(tokens, index - 1, "trait");
        let reexported = at_home
            && reexport.get(index).copied().unwrap_or(false)
            && !is_ident(tokens, index + 1, "as");
        if defined {
            check_trait_body(file, index, name, violations);
        } else if !reexported {
            violations.push(Violation::SeamInPure {
                file: file.path.clone(),
                line: line(tokens, index),
                name,
            });
        }
    }
}

/// The variants declared directly in each `enum` body, as `(enum,
/// variant, index of the variant's name)`: the first identifier of the
/// body and each after a comma at the body's own depth.
fn declared_variants(tokens: &[Token]) -> Vec<(String, String, usize)> {
    let mut declared = Vec::new();
    for index in 0..tokens.len() {
        if !is_ident(tokens, index, "enum") {
            continue;
        }
        let Some(enumeration) = ident(tokens.get(index + 1)) else {
            continue;
        };
        let Some(open) = (index..tokens.len()).find(|at| is_punct(tokens, *at, '{')) else {
            continue;
        };
        let close = closing(tokens, open).unwrap_or(tokens.len());
        let mut at = open + 1;
        let mut expecting = true;
        while at < close {
            match kind(tokens, at) {
                Some(TokenKind::Punct('(' | '{' | '[')) => {
                    at = closing(tokens, at).map_or(close, |end| end + 1);
                    continue;
                }
                Some(TokenKind::Punct('#')) => {
                    at = attribute_end(tokens, at).unwrap_or(at + 1);
                    continue;
                }
                Some(TokenKind::Punct(',')) => expecting = true,
                Some(TokenKind::Ident(variant)) if expecting => {
                    declared.push((enumeration.to_owned(), variant.clone(), at));
                    expecting = false;
                }
                _ => {}
            }
            at += 1;
        }
    }
    declared
}

/// The segment before `::` ahead of the identifier at `index`, if any.
fn variant_path_owner(tokens: &[Token], index: usize) -> Option<&str> {
    if index < 2 || !matches!(kind(tokens, index - 1), Some(TokenKind::PathSep)) {
        return None;
    }
    ident(tokens.get(index - 2))
}

/// Reports a provided method in the body of the seam trait named at
/// `name_at`: any block inside the trait's own braces.
fn check_trait_body(
    file: &SourceFile,
    name_at: usize,
    name: &'static str,
    violations: &mut Vec<Violation>,
) {
    let tokens = &file.tokens;
    let Some(open) = (name_at..tokens.len()).find(|index| is_punct(tokens, *index, '{')) else {
        return;
    };
    let close = closing(tokens, open).unwrap_or(tokens.len());
    if let Some(inner) = (open + 1..close).find(|index| is_punct(tokens, *index, '{')) {
        violations.push(Violation::ProvidedMethod {
            file: file.path.clone(),
            line: line(tokens, inner),
            name,
        });
    }
}

/// A sampling function found in a file.
struct Found {
    /// Its parameter list's delimiters.
    params: (usize, usize),
    /// Its body's braces.
    body: (usize, usize),
}

/// The definitions of `name` in `tokens`: `fn name (...) ... { ... }`.
fn definitions(tokens: &[Token], name: &str) -> Vec<Found> {
    let mut found = Vec::new();
    for index in 0..tokens.len() {
        if !(is_ident(tokens, index, "fn") && is_ident(tokens, index + 1, name)) {
            continue;
        }
        let Some(open) = (index + 2..tokens.len()).find(|at| is_punct(tokens, *at, '(')) else {
            continue;
        };
        let Some(close) = closing(tokens, open) else {
            continue;
        };
        let Some(body) = (close..tokens.len()).find(|at| is_punct(tokens, *at, '{')) else {
            continue;
        };
        let Some(end) = closing(tokens, body) else {
            continue;
        };
        found.push(Found {
            params: (open, close),
            body: (body, end),
        });
    }
    found
}

/// The ranges of the arguments or parameters inside the delimiters at
/// `open` and `close`, split at commas at depth zero.
fn split_list(tokens: &[Token], open: usize, close: usize) -> Vec<(usize, usize)> {
    let mut parts = Vec::new();
    let mut start = open + 1;
    let mut index = open + 1;
    while index < close {
        match kind(tokens, index) {
            Some(TokenKind::Punct('(' | '[' | '{')) => {
                index = closing(tokens, index).map_or(close, |end| end + 1);
            }
            Some(TokenKind::Punct('<')) => {
                // Generic arguments in a parameter's type.
                let mut depth = 0_usize;
                while index < close {
                    match kind(tokens, index) {
                        Some(TokenKind::Punct('<')) => depth += 1,
                        Some(TokenKind::Punct('>')) => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    index += 1;
                }
                index += 1;
            }
            Some(TokenKind::Punct(',')) => {
                parts.push((start, index));
                start = index + 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    if start < close {
        parts.push((start, close));
    }
    parts
}

/// The name a parameter binds: its first identifier after `&` and `mut`.
fn parameter_name(tokens: &[Token], (start, end): (usize, usize)) -> Option<&str> {
    (start..end)
        .filter_map(|index| ident(tokens.get(index)))
        .find(|word| *word != "mut")
}

/// The binding of the one fresh sample, as `(name, index of let)`:
/// `let NAME = PARAM . sample ( ) . map_err ( DeadlineError :: Clock ) ? ;`
/// with PARAM one of `params`, the parameters whose type names `Clock`.
fn fresh_bindings<'a>(
    tokens: &'a [Token],
    body: (usize, usize),
    params: &[&str],
) -> Vec<(&'a str, usize)> {
    let mut found = Vec::new();
    for index in body.0..body.1 {
        if !is_ident(tokens, index, "let") {
            continue;
        }
        let Some(name) = ident(tokens.get(index + 1)) else {
            continue;
        };
        let shape: [(usize, &str); 15] = [
            (2, "="),
            (4, "."),
            (5, "sample"),
            (6, "("),
            (7, ")"),
            (8, "."),
            (9, "map_err"),
            (10, "("),
            (11, "DeadlineError"),
            (12, "::"),
            (13, "Clock"),
            (14, ")"),
            (15, "?"),
            (16, ";"),
            (3, ""),
        ];
        let matches = shape.iter().all(|(offset, expected)| {
            let at = index + offset;
            match (expected, kind(tokens, at)) {
                (&"", Some(TokenKind::Ident(param))) => params.contains(&param.as_str()),
                (&"::", Some(TokenKind::PathSep)) => true,
                (word, Some(TokenKind::Ident(found))) => found == word,
                (word, Some(TokenKind::Punct(found))) => word.chars().eq(std::iter::once(*found)),
                _ => false,
            }
        });
        if matches {
            found.push((name, index));
        }
    }
    found
}

/// Whether the token at `at` lies inside a macro call's delimiters within
/// `body`.
fn inside_macro(tokens: &[Token], body: (usize, usize), at: usize) -> bool {
    let mut index = body.0 + 1;
    while index < at {
        let opens_macro = is_punct(tokens, index, '!')
            && index > 0
            && ident(tokens.get(index - 1)).is_some()
            && matches!(punct(tokens.get(index + 1)), Some('(' | '[' | '{'));
        if opens_macro && let Some(close) = closing(tokens, index + 1) {
            if at < close {
                return true;
            }
            index = close + 1;
            continue;
        }
        index += 1;
    }
    false
}

/// Whether the argument tokens are a recorded arrival or `first_sent`:
/// `[&] a.b.arrival`, or `a.b.first_sent.as_ref()` followed by `?` or by
/// `.ok_or(...)?`.
fn is_recorded_from(tokens: &[Token], (start, end): (usize, usize)) -> bool {
    let start = if is_punct(tokens, start, '&') {
        start + 1
    } else {
        start
    };
    // A field path: ident (. ident)*, at least one dot.
    let mut index = start;
    let mut last = None;
    let mut dots = 0;
    while index < end {
        let Some(word) = ident(tokens.get(index)) else {
            return false;
        };
        last = Some((word, index));
        if is_punct(tokens, index + 1, '.') && ident(tokens.get(index + 2)).is_some() {
            if ident(tokens.get(index + 2)) == Some("as_ref") && word == "first_sent" {
                break;
            }
            dots += 1;
            index += 2;
        } else {
            break;
        }
    }
    let Some((field, at)) = last else {
        return false;
    };
    if dots == 0 {
        return false;
    }
    match field {
        "arrival" => at + 1 == end,
        "first_sent" => {
            let unwrapped = is_punct(tokens, at + 1, '.')
                && is_ident(tokens, at + 2, "as_ref")
                && is_punct(tokens, at + 3, '(')
                && is_punct(tokens, at + 4, ')');
            if !unwrapped {
                return false;
            }
            let rest = at + 5;
            if is_punct(tokens, rest, '?') {
                return rest + 1 == end;
            }
            is_punct(tokens, rest, '.')
                && is_ident(tokens, rest + 1, "ok_or")
                && is_punct(tokens, rest + 2, '(')
                && closing(tokens, rest + 2)
                    .is_some_and(|close| is_punct(tokens, close + 1, '?') && close + 2 == end)
        }
        _ => false,
    }
}

/// What an attribute at `start` sits on: the keyword of the next item,
/// after further attributes and a visibility.
fn attribute_target(tokens: &[Token], end: usize) -> (Option<&str>, usize) {
    let mut index = end;
    while let Some(next) = attribute_end(tokens, index) {
        index = next;
    }
    if is_ident(tokens, index, "pub") {
        index += 1;
        if is_punct(tokens, index, '(') {
            index = closing(tokens, index).map_or(index, |close| close + 1);
        }
    }
    while ident(tokens.get(index))
        .is_some_and(|word| ["const", "async", "unsafe", "extern", "default"].contains(&word))
    {
        index += 1;
    }
    (ident(tokens.get(index)), index)
}

#[expect(
    clippy::too_many_lines,
    reason = "the rules of 5.3, point 2, read as one pass in their written order"
)]
fn deadline_built_from_a_fresh_sample(
    files: &[SourceFile],
    sampling: &[Sampling],
    violations: &mut Vec<Violation>,
) {
    let files: Vec<&SourceFile> = files.iter().collect();
    // Every sampling function's definitions, and the index of its `from`.
    let mut from_index: Vec<(&Sampling, Option<usize>)> = Vec::new();
    for listed in sampling {
        let in_crate: Vec<&&SourceFile> = files
            .iter()
            .filter(|file| file.krate == listed.krate)
            .collect();
        let mut count = 0;
        let mut from = None;
        for file in &in_crate {
            for found in definitions(&file.tokens, listed.name()) {
                count += 1;
                let params: Vec<(usize, usize)> =
                    split_list(&file.tokens, found.params.0, found.params.1);
                let names: Vec<&str> = params
                    .iter()
                    .filter_map(|range| parameter_name(&file.tokens, *range))
                    .collect();
                from = names.iter().position(|name| *name == "from");
                if from.is_none() {
                    violations.push(Violation::NoFrom {
                        file: file.path.clone(),
                        line: line(&file.tokens, found.params.0),
                    });
                }
                // The parameters whose type names `Clock`: the only
                // receivers of the fresh sample.
                let clocks: Vec<&str> = params
                    .iter()
                    .filter(|(start, end)| {
                        let colon = (*start..*end).find(|at| is_punct(&file.tokens, *at, ':'));
                        colon.is_some_and(|colon| {
                            (colon..*end).any(|at| is_ident(&file.tokens, at, "Clock"))
                        })
                    })
                    .filter_map(|range| parameter_name(&file.tokens, *range))
                    .collect();
                check_sampling_body(file, &found, &names, &clocks, violations);
            }
        }
        if count > 1 {
            violations.push(Violation::Duplicate { path: listed.path });
        }
        if count == 0 && !in_crate.is_empty() {
            violations.push(Violation::Missing { path: listed.path });
        }
        from_index.push((listed, from));
    }

    for file in &files {
        let tokens = &file.tokens;
        let bodies: Vec<(usize, usize)> = sampling
            .iter()
            .filter(|listed| listed.krate == file.krate)
            .flat_map(|listed| definitions(tokens, listed.name()))
            .map(|found| found.body)
            .collect();
        let builds = tokens
            .iter()
            .any(|token| matches!(&token.kind, TokenKind::Ident(word) if CONSTRUCTORS.contains(&word.as_str())));
        let crate_builds = builds
            || files.iter().any(|other| {
                other.krate == file.krate
                    && other.tokens.iter().any(|token| {
                        matches!(&token.kind, TokenKind::Ident(word) if CONSTRUCTORS.contains(&word.as_str()))
                    })
            });
        for index in 0..tokens.len() {
            // Constructors outside the sampling function.
            // In `ironafs-time` a constructor's name may appear only where
            // it is defined.
            let definition =
                file.krate == TIME_CRATE && index > 0 && is_ident(tokens, index - 1, "fn");
            if let Some(name) = ident(tokens.get(index))
                .and_then(|word| CONSTRUCTORS.iter().copied().find(|c| *c == word))
                && !definition
                && !bodies
                    .iter()
                    .any(|(open, close)| *open < index && index < *close)
            {
                violations.push(Violation::Outside {
                    file: file.path.clone(),
                    line: line(tokens, index),
                    name,
                });
            }
            // Allowances of the constructors' lint.
            if let Some(end) = attribute_end(tokens, index)
                && (index..end).any(|at| is_ident(tokens, at, ALLOWANCE))
            {
                let inner = is_punct(tokens, index + 1, '!');
                let (target, at) = attribute_target(tokens, end);
                let on = if inner {
                    Some("a module or the crate")
                } else {
                    match target {
                        Some("mod") => Some("a module"),
                        Some("impl") => Some("an impl"),
                        Some("trait") => Some("a trait"),
                        Some("fn") => {
                            let sampling_here = sampling.iter().any(|listed| {
                                listed.krate == file.krate
                                    && is_ident(tokens, at + 1, listed.name())
                            });
                            let item = (at, item_end(tokens, at));
                            let names_constructor = (item.0..item.1).any(|i| {
                                ident(tokens.get(i)).is_some_and(|w| CONSTRUCTORS.contains(&w))
                            });
                            (!sampling_here && names_constructor).then_some("another function")
                        }
                        _ => None,
                    }
                };
                if let Some(on) = on
                    && crate_builds
                {
                    violations.push(Violation::Allowance {
                        file: file.path.clone(),
                        line: line(tokens, index),
                        on,
                    });
                }
            }
            // Calls of a sampling function.
            for (listed, from) in &from_index {
                if !is_ident(tokens, index, listed.name())
                    || (index > 0 && is_ident(tokens, index - 1, "fn"))
                {
                    continue;
                }
                let shown = file.path.clone();
                let at_line = line(tokens, index);
                if is_ident(tokens, index + 1, "as") {
                    violations.push(Violation::Renamed {
                        file: shown,
                        line: at_line,
                    });
                } else if is_punct(tokens, index + 1, '(') {
                    let close = closing(tokens, index + 1).unwrap_or(tokens.len());
                    let arguments = split_list(tokens, index + 1, close);
                    let recorded = from
                        .and_then(|from| arguments.get(from))
                        .is_some_and(|range| is_recorded_from(tokens, *range));
                    if !recorded {
                        violations.push(Violation::BadFrom {
                            file: shown,
                            line: at_line,
                        });
                    }
                } else if !in_use_item(tokens, index) {
                    violations.push(Violation::AsValue {
                        file: shown,
                        line: at_line,
                        name: listed.name().to_owned(),
                    });
                }
            }
        }
    }
}

/// Whether the token at `index` is inside a `use` item.
fn in_use_item(tokens: &[Token], index: usize) -> bool {
    let mut at = index;
    while let Some(before) = at.checked_sub(1) {
        at = before;
        match kind(tokens, at) {
            Some(TokenKind::Ident(word)) if word == "use" => return true,
            Some(TokenKind::Ident(_) | TokenKind::PathSep | TokenKind::Punct('{' | ',' | '*')) => {}
            _ => return false,
        }
    }
    false
}

/// The rules inside one sampling function's body.
fn check_sampling_body(
    file: &SourceFile,
    found: &Found,
    params: &[&str],
    clocks: &[&str],
    violations: &mut Vec<Violation>,
) {
    let tokens = &file.tokens;
    let (open, close) = found.body;
    let shown = || file.path.clone();
    for index in open + 1..close {
        if is_punct(tokens, index, '|') {
            violations.push(Violation::Hides {
                file: shown(),
                line: line(tokens, index),
                what: "a closure (or a `|`)",
            });
            break;
        }
    }
    for index in open + 1..close {
        if is_ident(tokens, index, "fn") {
            violations.push(Violation::Hides {
                file: shown(),
                line: line(tokens, index),
                what: "a nested function",
            });
        }
    }
    let bindings = fresh_bindings(tokens, found.body, clocks);
    let now = if let [(name, _)] = bindings.as_slice() {
        Some(*name)
    } else {
        violations.push(Violation::NoFreshSample {
            file: shown(),
            line: line(tokens, open),
        });
        None
    };
    let bound_at = bindings.first().map(|(_, at)| *at);
    if let Some(now) = now
        && params.contains(&now)
    {
        violations.push(Violation::Reassigned {
            file: shown(),
            line: line(tokens, open),
        });
    }
    names_only_in_constructor_arguments(file, found.body, now, bound_at, violations);
    for index in open + 1..close {
        let Some(name) = ident(tokens.get(index)) else {
            continue;
        };
        if !CONSTRUCTORS.contains(&name) {
            continue;
        }
        let at_line = line(tokens, index);
        if inside_macro(tokens, found.body, index) {
            violations.push(Violation::InMacro {
                file: shown(),
                line: at_line,
            });
            continue;
        }
        let path_call = index > 0
            && matches!(kind(tokens, index - 1), Some(TokenKind::PathSep))
            && is_punct(tokens, index + 1, '(');
        if !path_call {
            violations.push(Violation::AsValue {
                file: shown(),
                line: at_line,
                name: name.to_owned(),
            });
            continue;
        }
        // `now` is the fresh binding, used after it is bound.
        let fresh = now.is_some_and(|now| {
            is_punct(tokens, index + 2, '&')
                && is_ident(tokens, index + 3, now)
                && is_punct(tokens, index + 4, ',')
                && bound_at.is_some_and(|at| at < index)
        });
        if !fresh {
            violations.push(Violation::NowNotFresh {
                file: shown(),
                line: at_line,
            });
        }
        let from = is_ident(tokens, index + 5, "from") && is_punct(tokens, index + 6, ',');
        if !from {
            violations.push(Violation::NotFromParameter {
                file: shown(),
                line: at_line,
            });
        }
    }
}

/// Reports every appearance, in a sampling function's body, of the fresh
/// binding's name or of `from` outside the constructor arguments they
/// fill: another binding, a use or an assignment. So the constructor's
/// arguments can only be the fresh sample, after it is bound, and the
/// parameter.
fn names_only_in_constructor_arguments(
    file: &SourceFile,
    (open, close): (usize, usize),
    now: Option<&str>,
    bound_at: Option<usize>,
    violations: &mut Vec<Violation>,
) {
    let tokens = &file.tokens;
    // The constructor arguments the fresh binding and `from` may fill:
    // `CONSTRUCTOR ( & now , from ,`, by position.
    let in_constructor = |index: usize, offset: usize| {
        index >= offset
            && ident(tokens.get(index - offset)).is_some_and(|word| CONSTRUCTORS.contains(&word))
    };
    // Every other appearance of either name in the body is another
    // binding, a use or an assignment: refused, so the constructor's
    // arguments can only be the fresh sample and the parameter.
    for index in open + 1..close {
        if let Some(now) = now
            && is_ident(tokens, index, now)
            && Some(index) != bound_at.map(|at| at + 1)
            && !(in_constructor(index, 3) && is_punct(tokens, index - 1, '&'))
        {
            violations.push(Violation::Reassigned {
                file: file.path.clone(),
                line: line(tokens, index),
            });
        }
        if is_ident(tokens, index, "from") && !in_constructor(index, 5) {
            violations.push(Violation::NotFromParameter {
                file: file.path.clone(),
                line: line(tokens, index),
            });
        }
    }
}

/// Reads every workspace member's non-test source, with a violation for
/// each file that is not readable as Rust.
///
/// # Errors
///
/// Returns an error if a directory cannot be listed or a file read.
fn workspace_files(workspace: &Workspace) -> Result<(Vec<SourceFile>, Vec<Violation>), Error> {
    let mut sources = Vec::new();
    let mut unreadable = Vec::new();
    for member in &workspace.members {
        let pure = deps::kind_of(&member.name) == Some(Kind::Pure);
        let lib_root = member.lib_root.as_deref();
        let lib_dir = member.lib_dir();
        let tests_dir = member.dir.join("tests");
        let benches_dir = member.dir.join("benches");
        let mut files = Vec::new();
        rust_source::files(
            &member.dir,
            &|path| {
                path == workspace.target_dir
                    || path == tests_dir
                    || path == benches_dir
                    || matches!((lib_dir, lib_root), (Some(dir), Some(root))
                        if path != root
                            && deps::pure_source::is_gated_test_module(path, dir, root))
            },
            &mut files,
        )?;
        for file in files {
            let shown = file
                .strip_prefix(&workspace.root)
                .unwrap_or(&file)
                .to_path_buf();
            match rust_source::read(&file)? {
                Ok(tokens) => sources.push(SourceFile {
                    krate: member.name.clone(),
                    lib_root: lib_root == Some(file.as_path()),
                    // Only a pure crate's library is pure code.
                    pure: pure && lib_dir.is_some_and(|dir| file.starts_with(dir)),
                    path: shown,
                    tokens: without_test_items(tokens),
                }),
                Err(error) => unreadable.push(Violation::Unreadable { file: shown, error }),
            }
        }
    }
    Ok((sources, unreadable))
}

/// Checks this repository's workspace, or with `self_test` proves on
/// planted sources that each rule is caught.
///
/// # Errors
///
/// Returns the violations found, or why the check could not run.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    if self_test {
        return self_test::run(output);
    }
    let workspace = metadata::load(None, &[])?;
    let (files, mut violations) = workspace_files(&workspace)?;
    for member in &workspace.members {
        if deps::kind_of(&member.name) == Some(Kind::Pure) {
            let manifest = member.dir.join("Cargo.toml");
            let text = std::fs::read_to_string(&manifest)
                .map_err(Error::path_io("read manifest", &manifest))?;
            let shown = manifest.strip_prefix(&workspace.root).unwrap_or(&manifest);
            violations.extend(renamed_dependencies(shown, &text));
        }
    }
    violations.extend(check(&files, SAMPLING));
    if !violations.is_empty() {
        return Err(Error::Rules {
            check: CHECK,
            violations: violations.iter().map(ToString::to_string).collect(),
        });
    }
    let pure = files.iter().filter(|file| file.pure).count();
    writeln!(
        output,
        "check-seams: {} Rust files read in {} crates ({pure} pure); no pure crate names a seam \
         outside its definition; every deadline is built from a fresh sample (sampling \
         functions: {})",
        files.len(),
        workspace.members.len(),
        SAMPLING
            .iter()
            .map(|listed| {
                let present = workspace
                    .members
                    .iter()
                    .any(|member| member.name == listed.krate);
                if present {
                    listed.path.to_owned()
                } else {
                    format!("{}, its crate not in the workspace yet", listed.path)
                }
            })
            .collect::<Vec<_>>()
            .join("; "),
    )
    .map_err(Error::io("report check-seams"))
}
