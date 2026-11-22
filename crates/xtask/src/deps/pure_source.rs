// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the source of `ironafs-client` may not name (ADR 0004, C4,
//! point 2): it may use threads, but makes no socket or operating-system
//! call, and reads clocks and randomness only through the seams of
//! `ironafs-time` (coding standards rule 7). The standard library is not a
//! crate edge, so the graph alone cannot show it. The pure crates are not
//! scanned here: `check-pure` builds them where the standard library does
//! not exist, and holds their source to its own rules.
//!
//! The scan reads tokens, not a compiled crate, so it also refuses what
//! would hide a path from it: renames and globs of the standard library,
//! imported or renamed built-in macros, macro path fragments, macros called
//! and attributes built through a fragment, a `#` that does not open an
//! attribute, `#[path]`, and the names of the macros that read files,
//! `global_asm` and the printing macros anywhere. It refuses `unsafe` and
//! `extern` too, which reach the operating system with no path at all.
//!
//! Apart from the tokens, every file of the package is read as raw text for
//! the names of the macros that read files, as `check-pure` reads a pure
//! package, so a token the reader misreads cannot hide one.

use std::fs;
use std::path::{Path, PathBuf};

use super::Kind;
use super::Violation;
use crate::Error;
use crate::metadata::{Member, Workspace};
use crate::pure::{FILE_MACROS, file_macro_words, package_files};
use crate::rust_source::{self, Token, TokenKind, ident, punct};

#[cfg(test)]
mod tests;

const CLOCK: &str = "clocks come only through the Clock interface of ironafs-time";
const WAIT: &str = "a wait with a timeout reads the system clock; time comes only through \
                    the Clock interface of ironafs-time";
const RANDOM: &str = "the standard library's random hasher state draws a seed from the \
                      operating system; randomness comes only through the Rng interface of \
                      ironafs-time";
const HASH_COLLECTION: &str = "ironafs-client names no standard hash collection: its operation \
                               code runs under the simulator, where iteration order must not \
                               change between runs, and a type alias, a collect or a derived \
                               default seeds the default hasher without a constructor's name; \
                               use a BTreeMap or a BTreeSet";
const UNSAFE: &str = "unsafe code and foreign functions belong in ironafs-os, under its own \
                      ADR (AGENTS.md)";
const MACRO_PATH: &str = "an imported built-in macro can be renamed out of this check's sight; \
                          call it by its own name";
const FILES: &str = "files belong in a driver";
const INCLUDE: &str = "include!, include_str! and include_bytes! read files from where this check \
                       does not look, and their names are refused anywhere, since a macro can \
                       join a name to its `!`";
const PATH: &str = "a path attribute loads a module from where this check does not look";
const HASH: &str = "a # not directly followed by [ or ![ can open an attribute whose brackets \
                    come from a macro fragment, such as #[path], out of sight";
const STREAMS: &str = "the standard streams belong in a binary; events go through ironafs-events";

/// Paths forbidden with everything below them.
const FORBIDDEN_TREES: &[(&str, &str)] = &[
    (
        "std::net",
        "sockets and name lookup belong in a driver; address values are in core::net",
    ),
    ("std::fs", FILES),
    (
        "std::path",
        "a path's methods read the file system, and files belong in a driver",
    ),
    (
        "std::os",
        "operating-system calls belong in ironafs-os or a driver",
    ),
    ("std::process", "processes belong in a driver or a binary"),
    (
        "std::env",
        "the environment belongs in a binary; configuration comes through ironafs-config",
    ),
    ("std::io::stdin", STREAMS),
    ("std::io::stdout", STREAMS),
    ("std::io::stderr", STREAMS),
    ("std::io::Stdin", STREAMS),
    ("std::io::Stdout", STREAMS),
    ("std::io::Stderr", STREAMS),
    ("std::time::Instant", CLOCK),
    ("std::time::SystemTime", CLOCK),
    ("std::time::UNIX_EPOCH", CLOCK),
    (
        "std::backtrace",
        "a backtrace reads the environment and the program's own files",
    ),
];

/// Paths forbidden as written, because they would hide the ones above.
const FORBIDDEN_EXACT: &[(&str, &str)] = &[
    (
        "std",
        "a bare or renamed standard library hides the paths this check looks for",
    ),
    (
        "std::*",
        "a glob of the standard library hides the paths this check looks for",
    ),
    (
        "std::time",
        "importing the module hides its clocks from this check; name core::time::Duration",
    ),
    (
        "std::time::*",
        "a glob of std::time brings in its clocks; name core::time::Duration",
    ),
    (
        "std::io",
        "importing the module hides its standard streams from this check; name the items, \
         such as std::io::Read",
    ),
    (
        "std::io::*",
        "a glob of std::io brings in its standard streams; name the items",
    ),
];

/// Names that wait on the system clock wherever they appear.
const HIDDEN_CLOCKS: &[&str] = &[
    "sleep",
    "sleep_ms",
    "sleep_until",
    "park_timeout",
    "park_timeout_ms",
    "recv_timeout",
    "recv_deadline",
    "wait_timeout",
    "wait_timeout_ms",
    "wait_timeout_while",
];

/// Macros that write to the standard streams, refused wherever their name
/// appears, called or not, since a macro can join a name to its `!` from a
/// fragment.
const PRINTING: &[&str] = &["print", "println", "eprint", "eprintln", "dbg"];

/// Built-in macros this check finds by name, which an import could rename.
const NAMED_MACROS: &[&str] = &[
    "print",
    "println",
    "eprint",
    "eprintln",
    "dbg",
    "include",
    "include_str",
    "include_bytes",
    "asm",
    "global_asm",
    "naked_asm",
];

/// First segments of a path inside the crate: an import from them names
/// the crate's own item, which is checked where it is defined.
const LOCAL_ROOTS: &[&str] = &["crate", "self", "super"];

/// Macros that write machine code, which reaches the operating system
/// without a path or an `unsafe` block in the case of `global_asm!`.
const ASSEMBLY: &[&str] = &["asm", "global_asm", "naked_asm"];

/// The assembly macro refused wherever its name appears, called or not: a
/// macro can join a name to its `!` from a fragment, out of this check's
/// sight, and it needs no `unsafe` to do harm. The other assembly macros
/// need an `unsafe` block or attribute, which is refused anyway. The macros
/// that read files, `FILE_MACROS`, are refused anywhere too.
const ASSEMBLY_ANYWHERE: &[&str] = &["global_asm"];

/// Target kinds a scanned crate may have: its library and targets that are
/// never shipped. A build script would run I/O at build time.
const SCANNED_TARGETS: &[&str] = &["lib", "rlib", "test", "bench", "example"];

/// Why `ironafs-client` may not name the standard-library `path`, if it
/// may not.
pub(super) fn forbidden(path: &str) -> Option<&'static str> {
    let exact = FORBIDDEN_EXACT
        .iter()
        .find_map(|(forbidden, reason)| (*forbidden == path).then_some(*reason));
    exact.or_else(|| {
        FORBIDDEN_TREES.iter().find_map(|(tree, reason)| {
            path.strip_prefix(tree)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
                .then_some(*reason)
        })
    })
}

/// Whether the source of a crate of this kind is read: only
/// `ironafs-client`'s.
pub(super) fn is_scanned(kind: Kind) -> bool {
    kind == Kind::Composition
}

/// Reads the library source of `ironafs-client`, and reports each
/// forbidden name. A unit-test module is left out only
/// where its parent declares it as `#[cfg(test)] mod tests;` (coding
/// standards rule 15): then it never reaches a shipped build. Returns the
/// violations and the files read.
///
/// # Errors
///
/// Returns an error if a source directory cannot be listed or a file cannot
/// be read.
pub(super) fn scan(workspace: &Workspace) -> Result<(Vec<Violation>, Vec<PathBuf>), Error> {
    let mut violations = Vec::new();
    let mut scanned = Vec::new();
    for member in &workspace.members {
        if !super::kind_of(&member.name).is_some_and(is_scanned) {
            continue;
        }
        for target in &member.target_kinds {
            if !SCANNED_TARGETS.contains(&target.as_str()) {
                violations.push(Violation::ShippedTarget {
                    name: member.name.clone(),
                    target: target.clone(),
                });
            }
        }
        let (Some(lib_root), Some(lib_dir)) = (&member.lib_root, member.lib_dir()) else {
            violations.push(Violation::NoLibrary(member.name.clone()));
            continue;
        };
        let mut files = Vec::new();
        rust_source::files(
            lib_dir,
            &|path| path != lib_root && is_gated_test_module(path, lib_dir, lib_root),
            &mut files,
        )?;
        if files.is_empty() {
            violations.push(Violation::NoSource(member.name.clone()));
        }
        for file in files {
            let shown = shown(workspace, &file);
            check_file(&file, &shown, &mut violations)?;
            scanned.push(shown);
        }
        violations.extend(text_violations(workspace, member)?);
    }
    Ok((violations, scanned))
}

/// Reads every file of the member's package as raw text, whatever its
/// extension and whether or not it is compiled, for the names of the macros
/// that read files. A symbolic link is read through when it leads to a
/// file; one that leads anywhere else is reported unread.
///
/// # Errors
///
/// Returns an error if a directory cannot be listed or a file cannot be
/// read.
fn text_violations(workspace: &Workspace, member: &Member) -> Result<Vec<Violation>, Error> {
    let (mut files, links) = package_files(&member.dir, &workspace.target_dir)?;
    let mut violations = Vec::new();
    for link in links {
        if link.is_file() {
            files.push(link);
        } else {
            violations.push(Violation::Unreadable {
                file: shown(workspace, &link),
                error: "a symbolic link to no file, which this check does not follow".into(),
            });
        }
    }
    files.sort();
    for file in files {
        let text = fs::read(&file).map_err(Error::path_io("read file", &file))?;
        for (line, word) in file_macro_words(&text) {
            violations.push(Violation::NamedInText {
                file: shown(workspace, &file),
                line,
                word,
            });
        }
    }
    Ok(violations)
}

fn shown(workspace: &Workspace, path: &Path) -> PathBuf {
    path.strip_prefix(&workspace.root)
        .unwrap_or(path)
        .to_path_buf()
}

/// Whether `path` is the file or directory of a module `tests` whose parent
/// declares every `mod tests` it has under `#[cfg(test)]`.
pub(crate) fn is_gated_test_module(path: &Path, lib_dir: &Path, lib_root: &Path) -> bool {
    let named_tests = path
        .file_name()
        .is_some_and(|name| name == "tests" || name == "tests.rs");
    let Some(dir) = path.parent().filter(|_| named_tests) else {
        return false;
    };
    let parent = if dir == lib_dir {
        lib_root.to_path_buf()
    } else if dir.with_extension("rs").is_file() {
        dir.with_extension("rs")
    } else {
        dir.join("mod.rs")
    };
    match rust_source::read(&parent) {
        Ok(Ok(tokens)) => gates_every_test_module(&tokens),
        _ => false,
    }
}

/// Whether the tokens declare `mod tests` at least once, and only directly
/// under `#[cfg(test)]`.
pub(super) fn gates_every_test_module(tokens: &[Token]) -> bool {
    let gate = [
        TokenKind::Punct('#'),
        TokenKind::Punct('['),
        TokenKind::Ident("cfg".into()),
        TokenKind::Punct('('),
        TokenKind::Ident("test".into()),
        TokenKind::Punct(')'),
        TokenKind::Punct(']'),
    ];
    let mut declared = false;
    for (index, pair) in tokens.windows(2).enumerate() {
        if ident(pair.first()) == Some("mod") && ident(pair.get(1)) == Some("tests") {
            declared = true;
            let gated = index
                .checked_sub(gate.len())
                .and_then(|start| tokens.get(start..index))
                .is_some_and(|before| {
                    before
                        .iter()
                        .zip(&gate)
                        .all(|(token, expected)| &token.kind == expected)
                });
            if !gated {
                return false;
            }
        }
    }
    declared
}

fn check_file(file: &Path, shown: &Path, violations: &mut Vec<Violation>) -> Result<(), Error> {
    match rust_source::read(file)? {
        Ok(tokens) => {
            let mut found: Vec<(usize, String, &'static str)> = Vec::new();
            for std_path in rust_source::std_paths(&tokens) {
                if let Some(reason) = forbidden(&std_path.path) {
                    found.push((std_path.line, std_path.path, reason));
                }
            }
            for used in rust_source::use_paths(&tokens) {
                let first = used.path.split("::").next().unwrap_or_default();
                let last = used.path.rsplit("::").next().unwrap_or_default();
                if NAMED_MACROS.contains(&last) && !LOCAL_ROOTS.contains(&first) {
                    found.push((used.line, used.path, MACRO_PATH));
                }
            }
            found.extend(hidden_names(&tokens));
            found.sort_by_key(|(line, _, _)| *line);
            violations.extend(found.into_iter().map(|(line, name, reason)| {
                Violation::ForbiddenSource {
                    file: shown.to_path_buf(),
                    line,
                    name,
                    reason,
                }
            }));
        }
        Err(error) => violations.push(Violation::Unreadable {
            file: shown.to_path_buf(),
            error,
        }),
    }
    Ok(())
}

/// Finds what reaches the operating system without a standard-library path
/// the scan could see: `unsafe`, `extern` and assembly, hidden clocks, the
/// standard hash collections and their random state, printing macros,
/// macro path fragments, macros called and attributes built through a
/// fragment, a `#` not directly followed by `[` or `![`, `#[path]`, and the
/// names of the macros that read files, `global_asm` and the printing
/// macros anywhere. A raw identifier such as `r#unsafe` is a name, not the
/// keyword, but `r#include` still names the macro.
pub(super) fn hidden_names(tokens: &[Token]) -> Vec<(usize, String, &'static str)> {
    let mut found = Vec::new();
    let mut attribute_depth: Option<usize> = None;
    let mut fragment_reported = false;
    for (index, token) in tokens.iter().enumerate() {
        let after = tokens.get(index + 1);
        let line = token.line;
        if let Some(depth) = attribute_depth.as_mut() {
            match token.kind {
                TokenKind::Punct('[') => *depth += 1,
                TokenKind::Punct(']') => {
                    *depth = depth.saturating_sub(1);
                    if *depth == 0 {
                        attribute_depth = None;
                    }
                }
                _ => {}
            }
        }
        match &token.kind {
            TokenKind::Punct('#') => {
                let bracket = if punct(after) == Some('!') {
                    tokens.get(index + 2)
                } else {
                    after
                };
                if punct(bracket) != Some('[') {
                    found.push((line, "#".to_owned(), HASH));
                } else if attribute_depth.is_none() {
                    // The bracket itself is counted when it is reached.
                    attribute_depth = Some(0);
                    fragment_reported = false;
                }
            }
            TokenKind::Punct('$') => {
                let fragment = ident(after).filter(|name| *name != "crate");
                let next = tokens.get(index + 2);
                if attribute_depth.is_some() && !fragment_reported {
                    fragment_reported = true;
                    found.push((
                        line,
                        "#[$".to_owned(),
                        "an attribute built from a macro fragment can hide #[path] out of sight",
                    ));
                } else if let Some(fragment) = fragment
                    && next.is_some_and(|token| token.kind == TokenKind::PathSep)
                {
                    found.push((
                        line,
                        format!("${fragment}::"),
                        "a macro path fragment can name the standard library out of sight",
                    ));
                } else if let Some(fragment) = fragment
                    && punct(next) == Some('!')
                {
                    found.push((
                        line,
                        format!("${fragment}!"),
                        "a macro called through a fragment can be include! or a printing macro \
                         out of sight",
                    ));
                }
            }
            TokenKind::Ident(word) => {
                let is_macro = punct(after) == Some('!');
                let extern_crate = word == "extern" && ident(after) == Some("crate");
                if !token.raw && (word == "unsafe" || (word == "extern" && !extern_crate)) {
                    found.push((line, word.clone(), UNSAFE));
                } else if (is_macro || ASSEMBLY_ANYWHERE.contains(&word.as_str()))
                    && ASSEMBLY.contains(&word.as_str())
                {
                    let called = if is_macro { "!" } else { "" };
                    found.push((line, format!("{word}{called}"), UNSAFE));
                } else if HIDDEN_CLOCKS.contains(&word.as_str()) {
                    found.push((line, word.clone(), WAIT));
                } else if word == "RandomState" {
                    found.push((line, word.clone(), RANDOM));
                } else if word == "HashMap" || word == "HashSet" {
                    found.push((line, word.clone(), HASH_COLLECTION));
                } else if PRINTING.contains(&word.as_str()) {
                    let called = if is_macro { "!" } else { "" };
                    found.push((line, format!("{word}{called}"), STREAMS));
                } else if FILE_MACROS.contains(&word.as_str()) {
                    let called = if is_macro { "!" } else { "" };
                    found.push((line, format!("{word}{called}"), INCLUDE));
                } else if attribute_depth.is_some() && word == "path" && punct(after) == Some('=') {
                    found.push((line, "#[path]".to_owned(), PATH));
                }
            }
            _ => {}
        }
    }
    found
}
