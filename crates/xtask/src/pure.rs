// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pure crates are `#![no_std]` with `extern crate alloc`, and the
//! compiler proves it (ADR 0004, C3, point 1, and C4, point 1). Each pure
//! library is checked for every target in `rust-toolchain.toml`, in the dev
//! and release profiles, against a sysroot that holds only `core` and
//! `alloc`, with `unsafe_code` forbidden on the command line. There `std`
//! does not exist, so no file, socket, thread, clock or system randomness
//! can be named, whatever text produced the code; and the lint stops
//! foreign functions and assembly.
//!
//! Around the build, rules that need no compiler: the cargo configuration
//! may not change the flags the build runs with, a pure crate's
//! dependencies are pure crates with no build script, procedural macro,
//! native library or feature, and its source holds the rules the build
//! cannot. The build proves only the cfgs it compiles, so each source rule
//! holds for every cfg: the library root starts with an unconditional
//! `#![no_std]`, carries `extern crate alloc;` unconditionally at its top
//! level and has no crate-level `#![cfg(...)]`, and no file of the library
//! names `std`, another `no_std`, `unsafe`, assembly, `core::arch`, an
//! `extern` other than `extern crate alloc;` with no attribute, or a way to
//! read source from elsewhere (`include!`, `#[path]`, a symbolic link).
//! The names of the macros that read files or write assembly are refused
//! anywhere, and so is a `#` that does not open an attribute, since a macro
//! can join either to the rest from a fragment.
//! Like `extern`, the names of the macros that read files are refused in
//! the raw text of every file of the package too, so a token the reader
//! misreads cannot hide one.
//!
//! The build's result is checked by identity too (`matrix`): the triples
//! of crate, target and profile the compiler's own outputs show it
//! compiled must be exactly those expected.

mod matrix;
mod self_test;

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::Error;
use crate::deps::{self, Kind};
use crate::json;
use crate::metadata::{self, Member, Package, Workspace};
use crate::rust_source::{self, Token, TokenKind, ident, punct};
use matrix::{Built, Gap, Triple};

#[cfg(test)]
mod tests;

/// Words that may not appear in a cargo configuration file, each with what
/// it would do. Matching is on raw text, so a comment that names one fails
/// too: the rule fails closed rather than parse TOML.
const CONFIG_WORDS: &[(&str, &str)] = &[
    (
        "rustflags",
        "sets compiler flags, which could cap the lints that forbid unsafe code",
    ),
    (
        "rustc",
        "replaces or wraps the compiler, which could drop the flags this check passes",
    ),
    (
        "env",
        "sets the compiler's environment, which could enable unstable features",
    ),
];

/// The only line of a pure crate's source that may say `extern`.
const EXTERN_ALLOWED: &str = "extern crate alloc;";

/// Why a pure library's source may not name what it names.
const STD: &str = "the standard library is out of a pure crate's reach under every cfg, \
                   including those the build does not compile";
const NO_STD: &str = "a pure crate says no_std once, unconditionally, at the start of its \
                      library root";
const UNSAFE: &str = "unsafe code, assembly and foreign functions are out of a pure crate's \
                      reach under every cfg";
const EXTERN: &str = "a pure crate says extern only as `extern crate alloc;`, with no \
                      attribute, so no cfg can bring the standard library or a foreign \
                      function back";
const ELSEWHERE: &str = "a pure library's source is its own `.rs` files: nothing may read \
                         source or data from elsewhere, where these rules do not look";
const IN_TEXT: &str = "no file of a pure package names a macro that reads files, even in a \
                       comment or a string, so no misread token can hide one (ADR 0004, C4, \
                       point 1)";
const FRAGMENT: &str = "an attribute or macro call built from a macro fragment can hide \
                        `#[path]` or `include!` from this check";
const ARCH: &str = "the processor's own interfaces are out of a pure crate's reach under every \
                    cfg: `core::arch` holds safe functions, such as `__cpuid`, whose result \
                    depends on the machine the code runs on";
const HASH: &str = "a `#` not directly followed by `[` or `![` can open an attribute whose \
                    brackets come from a macro fragment, such as `#[path]`, out of this \
                    check's sight";

/// Built-in macros a pure library may not name anywhere, called or not: they
/// read files or write machine code, and a macro can join a name to its `!`
/// from a fragment, out of this check's sight.
const FORBIDDEN_MACROS: &[(&str, &str)] = &[
    ("include", ELSEWHERE),
    ("include_str", ELSEWHERE),
    ("include_bytes", ELSEWHERE),
    ("asm", UNSAFE),
    ("global_asm", UNSAFE),
    ("naked_asm", UNSAFE),
];

/// The built-in macros that read files. Their names are refused in the
/// raw text of every file of a pure package, and of `ironafs-client`, as
/// words on any line, comments and strings included: a rule on raw text
/// holds whatever a token reader gets wrong.
pub(crate) const FILE_MACROS: &[&str] = &["include", "include_str", "include_bytes"];

/// Target kinds a crate in a pure crate's closure may have: its library and
/// targets that are never shipped.
const TARGET_KINDS: &[&str] = &["lib", "rlib", "test", "bench", "example"];

/// The libraries of the toolchain's sysroot the check copies for each
/// target: `core`, `alloc` and what they need. The names are the
/// toolchain's own, which the self-test proves still work.
const SYSROOT_LIBRARIES: &[&str] = &[
    "libcore-",
    "liballoc-",
    "libcompiler_builtins-",
    "librustc_std_workspace_core-",
];

/// Environment variables that would change the compiler or its flags behind
/// this check's back; each is removed from the build's environment.
const CLEARED_ENVIRONMENT: &[&str] = &[
    "RUSTFLAGS",
    "CARGO_BUILD_RUSTFLAGS",
    "RUSTC",
    "CARGO_BUILD_RUSTC",
    "RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "RUSTC_BOOTSTRAP",
];

/// The profiles each pure library is built in: a `cfg(debug_assertions)`
/// branch is compiled in one and not the other.
const PROFILES: &[&str] = &["dev", "release"];

/// One broken rule.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    CargoConfig {
        file: PathBuf,
        line: usize,
        word: &'static str,
        reason: &'static str,
    },
    NoLibrary(String),
    LibraryOutside {
        name: String,
        lib_root: PathBuf,
    },
    NotNoStd(PathBuf),
    NoAlloc(PathBuf),
    RootCfg {
        file: PathBuf,
        line: usize,
    },
    Extern {
        file: PathBuf,
        line: usize,
    },
    Source {
        file: PathBuf,
        line: usize,
        name: String,
        reason: &'static str,
    },
    NamedInText {
        file: PathBuf,
        line: usize,
        word: &'static str,
    },
    Unreadable {
        file: PathBuf,
        error: String,
    },
    Symlink(PathBuf),
    Closure {
        from: String,
        via: Vec<String>,
        to: String,
    },
    Target {
        name: String,
        target: String,
    },
    Links {
        name: String,
        links: String,
    },
    Features {
        name: String,
        features: Vec<String>,
    },
    Build {
        name: String,
        profile: &'static str,
        error: String,
    },
    Matrix(Gap),
}

impl fmt::Display for Violation {
    #[expect(
        clippy::too_many_lines,
        reason = "coding standards rule 10: one arm per violation, each its rule's message"
    )]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CargoConfig {
                file,
                line,
                word,
                reason,
            } => write!(
                formatter,
                "{}:{line}: names {word}: a tracked cargo configuration {reason}; the pure-crate \
                 build sets its own flags (ADR 0004, C4, point 1)",
                file.display()
            ),
            Self::NoLibrary(name) => write!(
                formatter,
                "{name} is a pure crate with no library target (ADR 0004, C4, point 1)"
            ),
            Self::LibraryOutside { name, lib_root } => write!(
                formatter,
                "{name}'s library root {} is outside its package: a pure crate's source is \
                 read from its own directory (ADR 0004, C4, point 1)",
                lib_root.display()
            ),
            Self::NotNoStd(file) => write!(
                formatter,
                "{}: a pure crate's library root starts with `#![no_std]`, unconditionally \
                 (ADR 0004, C4, point 1)",
                file.display()
            ),
            Self::NoAlloc(file) => write!(
                formatter,
                "{}: a pure crate's library root carries the line `{EXTERN_ALLOWED}` at its top \
                 level, with no attribute, so every pure crate has the same form (ADR 0004, C4, \
                 point 1)",
                file.display()
            ),
            Self::RootCfg { file, line } => write!(
                formatter,
                "{}:{line}: a crate-level `#![cfg(...)]` can empty a pure crate for the builds \
                 that would show what it names (ADR 0004, C4, point 1)",
                file.display()
            ),
            Self::Source {
                file,
                line,
                name,
                reason,
            } => write!(
                formatter,
                "{}:{line}: names {name}: {reason} (ADR 0004, C4, point 1)",
                file.display()
            ),
            Self::NamedInText { file, line, word } => {
                write!(
                    formatter,
                    "{}:{line}: says {word}: {IN_TEXT}",
                    file.display()
                )
            }
            Self::Unreadable { file, error } => write!(
                formatter,
                "{}: cannot be read as Rust source ({error}), so the pure-crate rules cannot \
                 be checked on it (ADR 0004, C4, point 1)",
                file.display()
            ),
            Self::Symlink(file) => write!(
                formatter,
                "{}: is a symbolic link: a pure crate's files are its own, so no link can \
                 bring in source from where these rules do not look (ADR 0004, C4, point 1)",
                file.display()
            ),
            Self::Extern { file, line } => write!(
                formatter,
                "{}:{line}: says extern: in a pure crate `extern` appears only as \
                 `{EXTERN_ALLOWED}`, so no cfg can bring the standard library or a foreign \
                 function back (ADR 0004, C4, point 1)",
                file.display()
            ),
            Self::Closure { from, via, to } => {
                write!(formatter, "{from} depends on {to}")?;
                if !via.is_empty() {
                    write!(formatter, " through {}", via.join(" -> "))?;
                }
                write!(
                    formatter,
                    ": a pure crate's dependencies are pure crates only (ADR 0004, C4, points \
                     1 and 4)"
                )
            }
            Self::Target { name, target } => write!(
                formatter,
                "{name} has a {target} target: a pure crate and its dependencies have no build \
                 script or procedural macro, which run code at build time, and ship only a \
                 library (ADR 0004, C3, point 1)"
            ),
            Self::Links { name, links } => write!(
                formatter,
                "{name} links the native library {links}: a pure crate and its dependencies \
                 link none (ADR 0004, C4, point 1)"
            ),
            Self::Features { name, features } => write!(
                formatter,
                "{name} declares the features {}: a pure crate and its dependencies have no \
                 features, so none can turn the standard library on (ADR 0004, C4, point 1)",
                features.join(", ")
            ),
            Self::Build {
                name,
                profile,
                error,
            } => write!(
                formatter,
                "{name} does not build in the {profile} profile against core and alloc alone \
                 with unsafe code forbidden: {error} (ADR 0004, C3, point 1, and C4, point 1)"
            ),
            Self::Matrix(gap) => write!(formatter, "{gap} (ADR 0004, C4, point 1)"),
        }
    }
}

/// How cargo is run on the workspace being checked: this repository with
/// its lock file held fixed, or a fixture offline with its own `--config`
/// values.
pub(crate) struct Cargo<'a> {
    pub(crate) manifest: Option<&'a Path>,
    pub(crate) config: &'a [String],
}

impl Cargo<'_> {
    fn command(&self, root: &Path, subcommand: &str) -> Command {
        let mut command = Command::new("cargo");
        // Cargo reads its configuration from the directory it runs in.
        command.current_dir(root).arg(subcommand);
        for value in self.config {
            command.arg("--config").arg(value);
        }
        match self.manifest {
            Some(manifest) => command
                .arg("--offline")
                .arg("--manifest-path")
                .arg(manifest),
            None => command.arg("--locked"),
        };
        command
    }
}

/// The pure crates of a workspace, by the crate table.
pub(crate) fn pure_members(workspace: &Workspace) -> Vec<&Member> {
    workspace
        .members
        .iter()
        .filter(|member| deps::kind_of(&member.name) == Some(Kind::Pure))
        .collect()
}

/// The targets of `rust-toolchain.toml`: the quoted names in its `targets`
/// array.
///
/// # Errors
///
/// Returns an error if the file lists no target.
pub(crate) fn toolchain_targets(text: &str) -> Result<Vec<String>, Error> {
    let mut lines = text
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim());
    let Some(start) = lines.by_ref().find_map(|line| {
        line.strip_prefix("targets")
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('='))
    }) else {
        return Err(Error::UnexpectedOutput("rust-toolchain.toml"));
    };
    let mut array = start.to_owned();
    for line in lines {
        if array.contains(']') {
            break;
        }
        array.push_str(line);
    }
    let targets: Vec<String> = array
        .split(']')
        .next()
        .unwrap_or_default()
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    if targets.is_empty() {
        return Err(Error::UnexpectedOutput("rust-toolchain.toml"));
    }
    Ok(targets)
}

/// Every cargo configuration file under `root`: `config` or `config.toml`
/// in a `.cargo` directory. The target directory, `.git` and nested
/// checkouts (a directory holding its own `.git`) are not entered; cargo
/// reads a nested checkout's configuration only when run inside it.
///
/// # Errors
///
/// Returns an error if a directory cannot be listed.
pub(crate) fn config_files(root: &Path, target_dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let mut entries = fs::read_dir(&dir)
            .and_then(|entries| {
                entries
                    .map(|entry| entry.map(|entry| entry.path()))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(Error::path_io("list directory", &dir))?;
        entries.sort();
        for path in entries {
            let metadata =
                fs::symlink_metadata(&path).map_err(Error::path_io("read file type", &path))?;
            let name = path.file_name().unwrap_or_default();
            if metadata.is_dir() {
                let nested_checkout = path.join(".git").symlink_metadata().is_ok();
                if name != ".git" && path != target_dir && !nested_checkout {
                    pending.push(path);
                }
            } else if (name == "config" || name == "config.toml")
                && dir.file_name().is_some_and(|parent| parent == ".cargo")
            {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Whether `bytes` holds `word` at `at` with no identifier character on
/// either side. Only ASCII letters, digits and `_` count as identifier
/// characters, so the rules fail closed on any other byte.
fn word_at(bytes: &[u8], at: usize, word: &[u8]) -> bool {
    let is_identifier =
        |byte: Option<&u8>| byte.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_');
    bytes.get(at..at + word.len()) == Some(word)
        && !is_identifier(at.checked_sub(1).and_then(|before| bytes.get(before)))
        && !is_identifier(bytes.get(at + word.len()))
}

fn contains_word(line: &[u8], word: &[u8]) -> bool {
    (0..line.len()).any(|at| word_at(line, at, word))
}

/// The words of `CONFIG_WORDS` a configuration file names, with their line
/// numbers. `rustflags` and `rustc` count anywhere, also inside a longer
/// key; `env` counts as a word, which `[env]`, `env.NAME` and `env = {`
/// all are.
pub(crate) fn config_words(text: &[u8]) -> Vec<(usize, &'static str, &'static str)> {
    let mut found = Vec::new();
    for (index, line) in text.split(|byte| *byte == b'\n').enumerate() {
        for (word, reason) in CONFIG_WORDS {
            let named = if *word == "env" {
                contains_word(line, word.as_bytes())
            } else {
                line.windows(word.len())
                    .any(|window| window == word.as_bytes())
            };
            if named {
                found.push((index + 1, *word, *reason));
            }
        }
    }
    found
}

/// The lines of a pure crate's source file that say `extern` other than as
/// the one allowed line.
pub(crate) fn extern_lines(text: &[u8]) -> Vec<usize> {
    text.split(|byte| *byte == b'\n')
        .enumerate()
        .filter(|(_, line)| {
            contains_word(line, b"extern") && line.trim_ascii() != EXTERN_ALLOWED.as_bytes()
        })
        .map(|(index, _)| index + 1)
        .collect()
}

/// Each name of `FILE_MACROS` that `text` holds as a word, with its line
/// number, in line order.
pub(crate) fn file_macro_words(text: &[u8]) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for (index, line) in text.split(|byte| *byte == b'\n').enumerate() {
        for word in FILE_MACROS {
            if contains_word(line, word.as_bytes()) {
                found.push((index + 1, *word));
            }
        }
    }
    found
}

/// The leading inner attributes of a file, `#![...]` from its start, each
/// as the range of the tokens inside its brackets. An attribute that does
/// not close ends the list.
fn inner_attributes(tokens: &[Token]) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut index = 0;
    while punct(tokens.get(index)) == Some('#')
        && punct(tokens.get(index + 1)) == Some('!')
        && punct(tokens.get(index + 2)) == Some('[')
    {
        let start = index + 3;
        let mut depth = 1_usize;
        let mut at = start;
        while depth > 0 {
            match tokens.get(at).map(|token| &token.kind) {
                Some(TokenKind::Punct('[')) => depth += 1,
                Some(TokenKind::Punct(']')) => depth -= 1,
                Some(_) => {}
                None => return found,
            }
            at += 1;
        }
        found.push(start..at - 1);
        index = at;
    }
    found
}

/// The index of the `no_std` token of the library root's `#![no_std]`: an
/// inner attribute among those the file starts with, holding nothing else.
/// Comments and strings are not tokens, so text in them never counts.
pub(crate) fn root_no_std(tokens: &[Token]) -> Option<usize> {
    inner_attributes(tokens)
        .into_iter()
        .find(|range| range.len() == 1 && ident(tokens.get(range.start)) == Some("no_std"))
        .map(|range| range.start)
}

/// Whether the library root holds `extern crate alloc;` at its top level,
/// outside every brace, with no outer attribute and no visibility before it.
/// Comments and strings are not tokens, so text in them never counts.
pub(crate) fn root_alloc(tokens: &[Token]) -> bool {
    let inner_end = inner_attributes(tokens).last().map(|range| range.end);
    let mut depth = 0_usize;
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::Punct('{') => depth += 1,
            TokenKind::Punct('}') => depth = depth.saturating_sub(1),
            _ => {}
        }
        let line = depth == 0
            && !token.raw
            && ident(Some(token)) == Some("extern")
            && ident(tokens.get(index + 1)) == Some("crate")
            && ident(tokens.get(index + 2)) == Some("alloc")
            && punct(tokens.get(index + 3)) == Some(';');
        let before = index.checked_sub(1);
        let unconditional = match before.and_then(|before| tokens.get(before)) {
            None => true,
            Some(token) => match token.kind {
                TokenKind::Punct(';' | '}') => true,
                // The `]` of the last of the file's leading inner attributes.
                TokenKind::Punct(']') => before == inner_end,
                _ => false,
            },
        };
        if line && unconditional {
            return true;
        }
    }
    false
}

/// The lines of the library root's leading inner attributes that name
/// `cfg`, whether as `#![cfg(...)]` or inside a `cfg_attr`: either can
/// leave the crate empty under some cfg.
pub(crate) fn root_cfg_lines(tokens: &[Token]) -> Vec<usize> {
    inner_attributes(tokens)
        .into_iter()
        .filter(|range| {
            range
                .clone()
                .any(|index| ident(tokens.get(index)) == Some("cfg"))
        })
        .filter_map(|range| tokens.get(range.start).map(|token| token.line))
        .collect()
}

/// What a pure library's source names that it may not, under any cfg:
/// `std`; `no_std` other than at the token `allowed_no_std`; `unsafe`;
/// `arch`, the module of the processor's own interfaces; the names of
/// assembly and of the macros that read files, anywhere; a macro
/// called through a fragment; `#[path]` or a fragment in an attribute; a
/// `#` not directly followed by `[` or `![`; and `extern` other than an
/// `extern crate alloc;` with no outer attribute. A raw identifier such as
/// `r#unsafe` is a name, not the keyword, but `r#include` still names the
/// macro.
pub(crate) fn library_names(
    tokens: &[Token],
    allowed_no_std: Option<usize>,
) -> Vec<(usize, String, &'static str)> {
    let mut found = Vec::new();
    // The depth of brackets in the attribute being read, and whether it is
    // an outer attribute.
    let mut attribute: Option<(usize, bool)> = None;
    let mut fragment_reported = false;
    // The index of the `]` that closed the last outer attribute.
    let mut outer_end = None;
    for (index, token) in tokens.iter().enumerate() {
        let after = tokens.get(index + 1);
        let line = token.line;
        if let Some((depth, outer)) = attribute.as_mut() {
            match token.kind {
                TokenKind::Punct('[') => *depth += 1,
                TokenKind::Punct(']') => {
                    *depth = depth.saturating_sub(1);
                    if *depth == 0 {
                        if *outer {
                            outer_end = Some(index);
                        }
                        attribute = None;
                    }
                }
                _ => {}
            }
        }
        let in_attribute = attribute.is_some();
        match &token.kind {
            TokenKind::Punct('#') => {
                let inner = punct(after) == Some('!');
                let bracket = if inner { tokens.get(index + 2) } else { after };
                if punct(bracket) != Some('[') {
                    found.push((line, "#".to_owned(), HASH));
                } else if !in_attribute {
                    // The bracket itself is counted when it is reached.
                    attribute = Some((0, !inner));
                    fragment_reported = false;
                }
            }
            TokenKind::Punct('$') => {
                if in_attribute && !fragment_reported {
                    fragment_reported = true;
                    found.push((line, "#[$".to_owned(), FRAGMENT));
                } else if let Some(name) = ident(after)
                    && punct(tokens.get(index + 2)) == Some('!')
                {
                    found.push((line, format!("${name}!"), FRAGMENT));
                }
            }
            TokenKind::Ident(word) => {
                let is_macro = punct(after) == Some('!');
                if word == "std" {
                    found.push((line, word.clone(), STD));
                } else if word == "arch" {
                    found.push((line, word.clone(), ARCH));
                } else if word == "no_std" && allowed_no_std != Some(index) {
                    found.push((line, word.clone(), NO_STD));
                } else if !token.raw && word == "unsafe" {
                    found.push((line, word.clone(), UNSAFE));
                } else if !token.raw && word == "extern" {
                    let alloc = ident(after) == Some("crate")
                        && ident(tokens.get(index + 2)) == Some("alloc")
                        && punct(tokens.get(index + 3)) == Some(';');
                    let gated = index
                        .checked_sub(1)
                        .is_some_and(|before| outer_end == Some(before));
                    if !alloc || gated {
                        found.push((line, word.clone(), EXTERN));
                    }
                } else if let Some((_, reason)) =
                    FORBIDDEN_MACROS.iter().find(|(name, _)| name == word)
                {
                    let called = if is_macro { "!" } else { "" };
                    found.push((line, format!("{word}{called}"), reason));
                } else if in_attribute && word == "path" && punct(after) == Some('=') {
                    found.push((line, "#[path]".to_owned(), ELSEWHERE));
                }
            }
            _ => {}
        }
    }
    found.sort_by_key(|(line, _, _)| *line);
    found
}

fn shown(workspace: &Workspace, path: &Path) -> PathBuf {
    path.strip_prefix(&workspace.root)
        .unwrap_or(path)
        .to_path_buf()
}

fn read(path: &Path) -> Result<Vec<u8>, Error> {
    fs::read(path).map_err(Error::path_io("read file", path))
}

/// Every file below `dir` in path order, and apart from them every
/// symbolic link, which is not followed. The directory `skip` is left out.
///
/// # Errors
///
/// Returns an error if a directory cannot be listed.
pub(crate) fn package_files(
    dir: &Path,
    skip: &Path,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), Error> {
    let mut files = Vec::new();
    let mut links = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for path in list(&dir)? {
            let metadata =
                fs::symlink_metadata(&path).map_err(Error::path_io("read file type", &path))?;
            if metadata.is_symlink() {
                links.push(path);
            } else if metadata.is_dir() {
                if path != skip {
                    pending.push(path);
                }
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    links.sort();
    Ok((files, links))
}

/// The entries of `dir`, in path order.
fn list(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut entries = fs::read_dir(dir)
        .and_then(|entries| {
            entries
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(Error::path_io("list source directory", dir))?;
    entries.sort();
    Ok(entries)
}

/// The rules that need no compiler: the cargo configuration, each pure
/// crate's source text and its dependency closure. Returns the violations
/// and the number of configuration files read.
///
/// # Errors
///
/// Returns an error if a file or directory cannot be read.
pub(crate) fn static_violations(workspace: &Workspace) -> Result<(Vec<Violation>, usize), Error> {
    let mut violations = Vec::new();
    let configs = config_files(&workspace.root, &workspace.target_dir)?;
    for file in &configs {
        for (line, word, reason) in config_words(&read(file)?) {
            violations.push(Violation::CargoConfig {
                file: shown(workspace, file),
                line,
                word,
                reason,
            });
        }
    }
    for member in pure_members(workspace) {
        violations.extend(source_violations(workspace, member)?);
        for found in closure_violations(workspace, member) {
            if !violations.contains(&found) {
                violations.push(found);
            }
        }
    }
    Ok((violations, configs.len()))
}

/// The source rules of a pure crate. Every file of its package is read
/// for `extern` and the names of `FILE_MACROS`, whatever its extension, and
/// none may be a symbolic link.
/// The `.rs` files below the library's directory, which are all the
/// library can compile once `#[path]` and `include!` are refused, are read
/// as tokens for what the library may not name.
fn source_violations(workspace: &Workspace, member: &Member) -> Result<Vec<Violation>, Error> {
    let (Some(lib_root), Some(lib_dir)) = (&member.lib_root, member.lib_dir()) else {
        return Ok(vec![Violation::NoLibrary(member.name.clone())]);
    };
    let mut violations = Vec::new();
    if !lib_dir.starts_with(&member.dir) {
        violations.push(Violation::LibraryOutside {
            name: member.name.clone(),
            lib_root: shown(workspace, lib_root),
        });
    }
    let (files, links) = package_files(&member.dir, &workspace.target_dir)?;
    violations.extend(
        links
            .iter()
            .map(|link| Violation::Symlink(shown(workspace, link))),
    );
    let mut root_found = false;
    for file in files {
        let text = read(&file)?;
        for line in extern_lines(&text) {
            violations.push(Violation::Extern {
                file: shown(workspace, &file),
                line,
            });
        }
        for (line, word) in file_macro_words(&text) {
            violations.push(Violation::NamedInText {
                file: shown(workspace, &file),
                line,
                word,
            });
        }
        let is_library_source = file.starts_with(lib_dir)
            && file.extension().is_some_and(|extension| extension == "rs");
        if !is_library_source {
            continue;
        }
        let source = String::from_utf8(text).map_err(|_| "not UTF-8".to_owned());
        match source
            .and_then(|source| rust_source::tokens(&source).map_err(|error| error.to_string()))
        {
            Ok(tokens) => {
                let allowed = if file == *lib_root {
                    root_found = true;
                    let allowed = root_no_std(&tokens);
                    if allowed.is_none() {
                        violations.push(Violation::NotNoStd(shown(workspace, lib_root)));
                    }
                    if !root_alloc(&tokens) {
                        violations.push(Violation::NoAlloc(shown(workspace, lib_root)));
                    }
                    for line in root_cfg_lines(&tokens) {
                        violations.push(Violation::RootCfg {
                            file: shown(workspace, lib_root),
                            line,
                        });
                    }
                    allowed
                } else {
                    None
                };
                for (line, name, reason) in library_names(&tokens, allowed) {
                    violations.push(Violation::Source {
                        file: shown(workspace, &file),
                        line,
                        name,
                        reason,
                    });
                }
            }
            Err(error) => violations.push(Violation::Unreadable {
                file: shown(workspace, &file),
                error,
            }),
        }
    }
    if !root_found
        && !violations
            .iter()
            .any(|found| matches!(found, Violation::LibraryOutside { .. }))
    {
        violations.push(Violation::NotNoStd(shown(workspace, lib_root)));
    }
    Ok(violations)
}

fn is_pure(package: &Package) -> bool {
    package.member && deps::kind_of(&package.name) == Some(Kind::Pure)
}

/// The packages a pure crate reaches through normal and build dependencies
/// must be pure crates; it and each of them may have no build script,
/// procedural macro, native library or feature. A package that is not pure
/// is reported and not followed further.
fn closure_violations(workspace: &Workspace, member: &Member) -> Vec<Violation> {
    let mut violations = Vec::new();
    let Some(start) = workspace
        .packages
        .iter()
        .find(|package| package.id == member.id)
    else {
        return violations;
    };
    let mut checked = vec![start];
    for (package, via) in deps::reachable(workspace, &member.id, &is_pure) {
        if !is_pure(package) {
            violations.push(Violation::Closure {
                from: member.name.clone(),
                via,
                to: package.name.clone(),
            });
        }
        checked.push(package);
    }
    for package in checked {
        for target in &package.target_kinds {
            if !TARGET_KINDS.contains(&target.as_str()) {
                violations.push(Violation::Target {
                    name: package.name.clone(),
                    target: target.clone(),
                });
            }
        }
        if let Some(links) = &package.links {
            violations.push(Violation::Links {
                name: package.name.clone(),
                links: links.clone(),
            });
        }
        if !package.features.is_empty() {
            violations.push(Violation::Features {
                name: package.name.clone(),
                features: package.features.clone(),
            });
        }
    }
    violations
}

/// The toolchain's own sysroot, as the `rustc` cargo runs reports it.
///
/// # Errors
///
/// Returns an error if `rustc` cannot be run or prints no path.
fn toolchain_sysroot(root: &Path) -> Result<PathBuf, Error> {
    const COMMAND: &str = "rustc --print sysroot";
    let output = Command::new("rustc")
        .args(["--print", "sysroot"])
        .current_dir(root)
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::io(COMMAND))?;
    Error::check_status(COMMAND, output.status)?;
    let text = String::from_utf8(output.stdout).map_err(|_| Error::UnexpectedOutput(COMMAND))?;
    let path = text.trim();
    if path.is_empty() {
        return Err(Error::UnexpectedOutput(COMMAND));
    }
    Ok(PathBuf::from(path))
}

/// Writes into `dir` a sysroot that holds, for each target, only the
/// metadata of `core`, `alloc` and what they need, taken from the
/// toolchain's sysroot. Files are hard-linked where the file system allows
/// it, and copied otherwise.
///
/// # Errors
///
/// Returns an error if a target's libraries are missing (the target is not
/// installed) or a file cannot be written.
pub(crate) fn core_sysroot(toolchain: &Path, targets: &[String], dir: &Path) -> Result<(), Error> {
    crate::fixture::remove_dir(dir)?;
    for target in targets {
        let from = toolchain.join("lib/rustlib").join(target).join("lib");
        let to = dir.join("lib/rustlib").join(target).join("lib");
        fs::create_dir_all(&to).map_err(Error::path_io("create sysroot directory", &to))?;
        let libraries = list(&from)?;
        let mut copied = Vec::new();
        for library in libraries {
            let name = library.file_name().unwrap_or_default().to_string_lossy();
            let wanted = SYSROOT_LIBRARIES
                .iter()
                .find(|prefix| name.starts_with(**prefix) && name.ends_with(".rmeta"));
            if let Some(prefix) = wanted {
                let destination = to.join(&*name);
                if fs::hard_link(&library, &destination).is_err() {
                    fs::copy(&library, &destination)
                        .map_err(Error::path_io("copy sysroot library", &destination))?;
                }
                copied.push(*prefix);
            }
        }
        if SYSROOT_LIBRARIES
            .iter()
            .any(|prefix| !copied.contains(prefix))
        {
            return Err(Error::PathIo {
                operation: "find core and alloc for an installed target",
                path: from,
                source: io::ErrorKind::NotFound.into(),
            });
        }
    }
    Ok(())
}

/// The first line of cargo's error output that says what failed.
fn first_error(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let mut lines = text.lines().map(str::trim);
    let first = lines
        .clone()
        .find(|line| line.starts_with("error") && !line.starts_with("error: could not compile"))
        .or_else(|| lines.find(|line| line.starts_with("error")));
    first
        .unwrap_or("cargo failed with no error line")
        .to_owned()
}

/// What the build compiles: each crate, for each target, in each profile.
/// The check builds the full plan of the pure crates present and compares
/// what was compiled with that plan's triples; the self-test builds changed
/// plans to prove a gap is caught.
pub(crate) struct Plan<'a> {
    pub(crate) members: Vec<&'a Member>,
    pub(crate) targets: Vec<String>,
    pub(crate) profiles: Vec<&'static str>,
}

impl<'a> Plan<'a> {
    /// Every pure crate of `workspace` that has a library, every target and
    /// both profiles.
    pub(crate) fn full(workspace: &'a Workspace, targets: &[String]) -> Self {
        Self {
            members: pure_members(workspace)
                .into_iter()
                .filter(|member| member.lib_root.is_some())
                .collect(),
            targets: targets.to_vec(),
            profiles: PROFILES.to_vec(),
        }
    }

    /// The triples the plan builds.
    pub(crate) fn expected(&self) -> BTreeSet<Triple> {
        let names: Vec<&str> = self
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect();
        matrix::expected(&names, &self.targets, &self.profiles)
    }
}

/// What a build found: the rules it broke, and the triples the compiler's
/// outputs show it compiled.
pub(crate) struct BuildResult {
    pub(crate) violations: Vec<Violation>,
    pub(crate) compiled: BTreeSet<Triple>,
}

/// Checks each library of `plan` for its targets, in its profiles, against
/// the sysroot at `sysroot`, with `unsafe_code` forbidden, and requires the
/// triples compiled to be exactly `expected`. A crate and profile whose
/// build failed is left out of the comparison: its failure is reported.
/// Builds go to `build_dir`, apart from the workspace's own, so the changed
/// flags do not rebuild everything else.
///
/// # Errors
///
/// Returns an error if cargo cannot be started or a dependency-info file
/// cannot be read for a reason other than its absence.
pub(crate) fn build_violations(
    workspace: &Workspace,
    cargo: &Cargo<'_>,
    sysroot: &Path,
    plan: &Plan<'_>,
    expected: &BTreeSet<Triple>,
    build_dir: &Path,
) -> Result<BuildResult, Error> {
    let mut flags = std::ffi::OsString::from("--sysroot=");
    flags.push(sysroot);
    flags.push("\u{1f}-Funsafe_code");
    let mut violations = Vec::new();
    let mut reported: Vec<Built> = Vec::new();
    let mut failed: Vec<(&str, &str)> = Vec::new();
    for member in &plan.members {
        for profile in &plan.profiles {
            let mut command = cargo.command(&workspace.root, "check");
            command
                .args(["--lib", "--quiet", "--package", &member.name])
                .args(["--message-format", "json-render-diagnostics"])
                .arg("--target-dir")
                .arg(build_dir)
                .env("CARGO_ENCODED_RUSTFLAGS", &flags);
            for name in CLEARED_ENVIRONMENT {
                command.env_remove(name);
            }
            for target in &plan.targets {
                command.args(["--target", target]);
            }
            if *profile == "release" {
                command.arg("--release");
            }
            let output = command.output().map_err(Error::io("cargo check"))?;
            let messages = String::from_utf8_lossy(&output.stdout);
            for line in messages.lines() {
                if let Ok(message) = json::parse(line)
                    && let Some(found) = matrix::built(&message, workspace, build_dir)
                    && !reported.contains(&found)
                {
                    reported.push(found);
                }
            }
            if !output.status.success() {
                failed.push((member.name.as_str(), profile));
                violations.push(Violation::Build {
                    name: member.name.clone(),
                    profile,
                    error: first_error(&output.stderr),
                });
            }
        }
    }
    let mut compiled = BTreeSet::new();
    for found in reported {
        let read = match fs::read_to_string(&found.dep_info) {
            Ok(text) => matrix::files_read(&text, &workspace.root),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                return Err(Error::PathIo {
                    operation: "read dependency-info file",
                    path: found.dep_info,
                    source: error,
                });
            }
        };
        if !read.iter().any(|file| same_file(file, &found.lib_root)) {
            violations.push(Violation::Matrix(Gap::NothingRead {
                triple: found.triple.clone(),
                dep_info: found.dep_info,
            }));
        }
        compiled.insert(found.triple);
    }
    let compared =
        |triple: &&Triple| !failed.contains(&(triple.name.as_str(), triple.profile.as_str()));
    let want: BTreeSet<Triple> = expected.iter().filter(compared).cloned().collect();
    let got: BTreeSet<Triple> = compiled.iter().filter(compared).cloned().collect();
    violations.extend(
        matrix::compare(&want, &got)
            .into_iter()
            .map(Violation::Matrix),
    );
    Ok(BuildResult {
        violations,
        compiled,
    })
}

/// Whether two paths name the same file, comparing their canonical forms
/// where both exist.
fn same_file(one: &Path, other: &Path) -> bool {
    match (fs::canonicalize(one), fs::canonicalize(other)) {
        (Ok(one), Ok(other)) => one == other,
        _ => one == other,
    }
}

/// Checks this repository's pure crates, or with `self_test` proves on
/// planted fixtures that each rule is caught.
///
/// # Errors
///
/// Returns the violations found, or why the check could not run.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    let toolchain_file = workspace.root.join("rust-toolchain.toml");
    let targets = toolchain_targets(
        &fs::read_to_string(&toolchain_file)
            .map_err(Error::path_io("read toolchain file", &toolchain_file))?,
    )?;
    if self_test {
        let sysroot = toolchain_sysroot(&workspace.root)?;
        return self_test::run(&workspace.target_dir, &sysroot, &targets, output);
    }
    let (mut violations, configs) = static_violations(&workspace)?;
    let plan = Plan::full(&workspace, &targets);
    let mut compiled = BTreeSet::new();
    if !plan.members.is_empty() {
        let dir = workspace.target_dir.join("xtask-pure");
        let sysroot = dir.join("sysroot");
        core_sysroot(&toolchain_sysroot(&workspace.root)?, &targets, &sysroot)?;
        let cargo = Cargo {
            manifest: None,
            config: &[],
        };
        let built = build_violations(
            &workspace,
            &cargo,
            &sysroot,
            &plan,
            &plan.expected(),
            &dir.join("build"),
        )?;
        violations.extend(built.violations);
        compiled = built.compiled;
    }
    if !violations.is_empty() {
        return Err(Error::Rules {
            check: "check-pure",
            violations: violations.iter().map(ToString::to_string).collect(),
        });
    }
    report(&workspace, configs, targets.len(), &compiled, output)
}

/// Prints what a passing run checked: the exact matrix compiled, and the
/// pure crates of the list not yet in the workspace, each checked from the
/// commit that adds it.
///
/// # Errors
///
/// Returns an error if the output cannot be written.
pub(crate) fn report(
    workspace: &Workspace,
    configs: usize,
    targets: usize,
    compiled: &BTreeSet<Triple>,
    output: &mut dyn Write,
) -> Result<(), Error> {
    let mut write =
        |line: String| writeln!(output, "{line}").map_err(Error::io("report check-pure"));
    let crates: BTreeSet<&str> = compiled.iter().map(|triple| triple.name.as_str()).collect();
    write(format!(
        "check-pure: {configs} cargo configuration files read; {} pure crates built for {targets} \
         targets in dev and release against core and alloc alone, {} (crate, target, profile) \
         triples, exactly as expected",
        crates.len(),
        compiled.len()
    ))?;
    for line in matrix::matrix_lines(compiled) {
        write(format!("  compiled: {line}"))?;
    }
    let absent: Vec<&str> = matrix::pure_list()
        .into_iter()
        .filter(|name| !workspace.members.iter().any(|member| member.name == *name))
        .collect();
    if !absent.is_empty() {
        write(format!("  not yet present: {}", absent.join(", ")))?;
    }
    Ok(())
}
