// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The site reader of `check-os`: what each file of `ironafs-os` holds that
//! ADR 0005's table counts, read from its tokens.
//!
//! A *site* is one `unsafe extern` block, one `unsafe` block or one `unsafe
//! impl`, named by the `#[expect(unsafe_code, reason = "<site id>")]` on
//! the item or statement that holds it (ADR 0005, point 6.1). For an
//! extern block the reader records each declared function and the block's
//! `#[link]` attribute; for a block, the one operation it holds (point
//! 6.3). A block whose body is anything but one call of a path with plain
//! arguments (names, literals, `&`, `&raw mut`, casts) is refused, so a
//! second foreign call or a raw read cannot hide beside the recorded one:
//! pointers are made, and values converted, in safe code around the block.
//!
//! The reader also refuses what could put code out of its sight or make
//! one target's code differ from what the compiler's file list shows:
//! macros defined in the crate, `include!` and `#[path]`, assembly, a
//! `cfg` in a file that holds a site (other than the gate of its unit-test
//! module), and `target_env` anywhere but the crate root's target
//! allowlist, the gates by which a row's `abi.rs` chooses its C library's
//! declarations (`#[cfg(target_env = "gnu")] mod gnu;` and
//! `pub(super) use gnu::...;`, and `musl`'s; ADR 0005, point 5.2, declares
//! `struct msghdr` per C library), and the crate root's glibc-only gates.
//! Those library files may hold no site, so a musl build compiles exactly
//! the glibc build's sites, less the rows the glibc-only gates leave out;
//! their declarations are proved on each library by `check-abi`.
//!
//! A **glibc-only gate** is exactly `#[cfg(all(target_os = "linux",
//! target_env = "gnu"))]`, or that condition inside `any(test, ...)`, on a
//! `mod NAME;` or a `pub use NAME::...;` of the crate root. It leaves a row
//! out of the musl build where the row's evidence is withheld there (ADR
//! 0005, sheet 14). The reader records each module the first form gates,
//! and `check-os` requires them to be exactly the modules of the rows the
//! copy implements on glibc and only approves on musl.

use crate::rust_source::{Token, TokenKind, core_shadows, ident, matching, punct};

#[cfg(test)]
mod tests;

pub(crate) const UNSAFE_ITEM: &str = "an unsafe function, trait or foreign-callable function: \
                                      the crate defines none (ADR 0005, points 1.2 and 2)";
pub(crate) const BLOCK_BODY: &str = "an unsafe block holds exactly one call of a path, with \
                                     plain arguments, and nothing else (ADR 0005, point 6.3)";
pub(crate) const EXTERN_FORM: &str = "a foreign declaration is written \
                                      `unsafe extern \"<abi>\" { fn ...; }` (ADR 0005, point 2)";
pub(crate) const EXTERN_ITEM: &str = "an extern block holds function declarations only \
                                      (ADR 0005, point 2)";
pub(crate) const LINK: &str = "a #[link] attribute stands only on a foreign declaration block \
                               (ADR 0005, point 6.7)";
pub(crate) const HIDDEN: &str = "macro_rules!, include!, #[path] and assembly could put code out \
                                 of this check's sight (ADR 0005, point 6.7)";
pub(crate) const CFG: &str = "a file that holds a site names cfg only as `#[cfg(test)] mod \
                              tests;`, so each target's sites are the files the compiler reads \
                              for it (ADR 0005, point 6.7)";
pub(crate) const TARGET_ENV: &str = "target_env is named only in the crate root's target \
                                     allowlist and glibc-only gates, and in an abi.rs's gates \
                                     `#[cfg(target_env = \"<gnu|musl>\")]` on that library's \
                                     `mod` or `pub(super) use`, so a musl build compiles the \
                                     glibc build's sites, less the rows withheld on musl";
pub(crate) const LIBRARY_SITE: &str = "a C library's declaration file (abi/gnu.rs, abi/musl.rs, \
                                       or a file below abi/gnu/ or abi/musl/) holds no site, so \
                                       a musl build's sites are the glibc build's (ADR 0005, \
                                       points 5.2 and 6.7)";
pub(crate) const CORE: &str = "an item, import or generic parameter named `core`, under which \
                               `core::...` in this crate could name something other than the \
                               core crate (ADR 0005, point 6.7)";
pub(crate) const TWO_NAMES: &str = "one statement or item carries more than one \
                                    #[expect(unsafe_code)] (ADR 0005, point 6.1)";
pub(crate) const OUTSIDE: &str = "unsafe code, a foreign declaration or function, or a #[link] \
                                  attribute outside ironafs-os (ADR 0005, point 1.5)";

/// One function of an extern block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExternItem {
    pub(crate) name: String,
    /// Declared `safe fn`.
    pub(crate) safe: bool,
}

/// What a site is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SiteKind {
    /// An `unsafe extern` block, its `#[link]` arguments as `key=value`
    /// pairs joined by `;`, and its functions in source order.
    Extern {
        link: Option<String>,
        items: Vec<ExternItem>,
    },
    /// An `unsafe` block: the path it calls, as written, and its
    /// arguments, each as its tokens' text joined by spaces.
    Block {
        operation: String,
        arguments: Vec<String>,
    },
    /// An `unsafe impl`.
    Impl,
    /// An `unsafe fn` standing in for a foreign function under Miri: only
    /// in a Miri stand-in, named `<site id>: Miri stand-in`.
    StandInFn,
}

/// The suffix of the name a Miri stand-in's sites carry (ADR 0005, point
/// 6.5): `#[expect(unsafe_code, reason = "<site id>: Miri stand-in")]`.
pub(crate) const STAND_IN: &str = ": Miri stand-in";

/// One site, its name (`None` if it carries no named expectation) and the
/// line of its `unsafe`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Site {
    pub(crate) id: Option<String>,
    pub(crate) line: usize,
    pub(crate) kind: SiteKind,
}

/// A rule broken at a line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) line: usize,
    pub(crate) what: &'static str,
}

/// What one file holds.
#[derive(Debug, Default)]
pub(crate) struct Scan {
    pub(crate) sites: Vec<Site>,
    pub(crate) problems: Vec<Problem>,
    /// The modules the crate root's glibc-only gates leave out of a musl
    /// build (`#[cfg(all(target_os = "linux", target_env = "gnu"))] mod
    /// NAME;`).
    pub(crate) glibc_only: Vec<String>,
}

/// Macros and attributes that read code from elsewhere or write machine
/// code.
const HIDING_WORDS: &[&str] = &[
    "macro_rules",
    "include",
    "include_str",
    "include_bytes",
    "asm",
    "global_asm",
    "naked_asm",
];

/// Where a file of the crate sits, which decides where it may name
/// `target_env`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    /// `src/lib.rs`, whose target allowlist names it.
    Root,
    /// A row module's `abi.rs`.
    Abi,
    /// A C library's declaration file under it, `abi/gnu.rs` or
    /// `abi/musl.rs`, or any file of that library's module below them
    /// (`abi/gnu/...`, `abi/musl/...`).
    Library,
    Other,
}

/// The place of the file at `path`, relative to the crate. A path with a
/// component that is not UTF-8 is [`Place::Other`] as a whole, never read
/// with that component dropped.
pub(crate) fn place_of(path: &std::path::Path) -> Place {
    let Some(parts) = path
        .components()
        .map(|part| part.as_os_str().to_str())
        .collect::<Option<Vec<&str>>>()
    else {
        return Place::Other;
    };
    match parts.as_slice() {
        ["src", "lib.rs"] => Place::Root,
        ["src", _, "abi.rs"] => Place::Abi,
        ["src", _, "abi", "gnu.rs" | "musl.rs"] | ["src", _, "abi", "gnu" | "musl", ..] => {
            Place::Library
        }
        _ => Place::Other,
    }
}

/// The C libraries whose declarations an `abi.rs` may choose by
/// `target_env`, each in the module of its own name.
const LIBRARIES: &[&str] = &["gnu", "musl"];

/// Whether the `target_env` at `index` is the whole condition of a gate
/// `#[cfg(target_env = "<library>")]` on the library's own module or an
/// import from it.
fn library_gate(tokens: &[Token], index: usize) -> bool {
    let at = |offset: usize| tokens.get(index + offset);
    let before = |offset: usize| index.checked_sub(offset).and_then(|at| tokens.get(at));
    let library = match at(2).map(|token| &token.kind) {
        Some(TokenKind::Str(library)) => library.as_str(),
        _ => return false,
    };
    LIBRARIES.contains(&library)
        && punct(before(4)) == Some('#')
        && punct(before(3)) == Some('[')
        && ident(before(2)) == Some("cfg")
        && punct(before(1)) == Some('(')
        && punct(at(1)) == Some('=')
        && punct(at(3)) == Some(')')
        && punct(at(4)) == Some(']')
        && gated_item(tokens, index + 5, library)
}

/// Whether the item at `at` is `mod <library>;` or
/// `pub(super) use <library>::...;` with no renaming: the module of the
/// library's declarations, or an import from it.
fn gated_item(tokens: &[Token], at: usize, library: &str) -> bool {
    let token = |offset: usize| tokens.get(at + offset);
    if ident(token(0)) == Some("mod") {
        return ident(token(1)) == Some(library) && punct(token(2)) == Some(';');
    }
    let import = ident(token(0)) == Some("pub")
        && punct(token(1)) == Some('(')
        && ident(token(2)) == Some("super")
        && punct(token(3)) == Some(')')
        && ident(token(4)) == Some("use")
        && ident(token(5)) == Some(library)
        && matches!(token(6).map(|token| &token.kind), Some(TokenKind::PathSep));
    let Some(end) = tokens
        .get(at..)
        .unwrap_or_default()
        .iter()
        .position(|token| punct(Some(token)) == Some(';'))
    else {
        return false;
    };
    import
        && !tokens
            .get(at..at + end)
            .unwrap_or_default()
            .iter()
            .any(|token| ident(Some(token)) == Some("as"))
}

/// One token of a gate's fixed shape.
#[derive(Clone, Copy)]
enum Shape {
    Punct(char),
    Ident(&'static str),
    Str(&'static str),
}

/// `#[cfg(all(target_os = "linux", target_env = "gnu"))]`, and the
/// position of `target_env` in it.
const GLIBC_ONLY: (&[Shape], usize) = (
    &[
        Shape::Punct('#'),
        Shape::Punct('['),
        Shape::Ident("cfg"),
        Shape::Punct('('),
        Shape::Ident("all"),
        Shape::Punct('('),
        Shape::Ident("target_os"),
        Shape::Punct('='),
        Shape::Str("linux"),
        Shape::Punct(','),
        Shape::Ident("target_env"),
        Shape::Punct('='),
        Shape::Str("gnu"),
        Shape::Punct(')'),
        Shape::Punct(')'),
        Shape::Punct(']'),
    ],
    10,
);

/// `#[cfg(any(test, all(target_os = "linux", target_env = "gnu")))]`, for
/// a module without sites that the gated rows and the tests use, and the
/// position of `target_env` in it.
const GLIBC_ONLY_OR_TEST: (&[Shape], usize) = (
    &[
        Shape::Punct('#'),
        Shape::Punct('['),
        Shape::Ident("cfg"),
        Shape::Punct('('),
        Shape::Ident("any"),
        Shape::Punct('('),
        Shape::Ident("test"),
        Shape::Punct(','),
        Shape::Ident("all"),
        Shape::Punct('('),
        Shape::Ident("target_os"),
        Shape::Punct('='),
        Shape::Str("linux"),
        Shape::Punct(','),
        Shape::Ident("target_env"),
        Shape::Punct('='),
        Shape::Str("gnu"),
        Shape::Punct(')'),
        Shape::Punct(')'),
        Shape::Punct(')'),
        Shape::Punct(']'),
    ],
    14,
);

/// Whether the tokens from `start` have `shape`.
fn has_shape(tokens: &[Token], start: usize, shape: &[Shape]) -> bool {
    shape.iter().enumerate().all(|(offset, expected)| {
        let token = tokens.get(start + offset);
        match expected {
            Shape::Punct(c) => punct(token) == Some(*c),
            Shape::Ident(name) => token.is_some_and(|token| !token.raw) && ident(token) == Some(name),
            Shape::Str(text) => {
                matches!(token.map(|token| &token.kind), Some(TokenKind::Str(found)) if found == text)
            }
        }
    })
}

/// What a `target_env` in the crate root gates.
enum Gate {
    /// A module the musl build leaves out.
    Module(String),
    /// An import, or the module without sites the tests also use.
    Other,
    /// Not a glibc-only gate.
    No,
}

/// What the `target_env` at `index` gates, if it is in a glibc-only gate
/// of the crate root on `mod NAME;` or `pub use NAME::...;`: the module,
/// for the first form on a `mod`.
fn glibc_gate(tokens: &[Token], index: usize) -> Gate {
    for (shape, at) in [GLIBC_ONLY, GLIBC_ONLY_OR_TEST] {
        let Some(start) = index.checked_sub(at) else {
            continue;
        };
        if !has_shape(tokens, start, shape) {
            continue;
        }
        let item = start + shape.len();
        let token = |offset: usize| tokens.get(item + offset);
        if ident(token(0)) == Some("mod") && punct(token(2)) == Some(';') {
            return match ident(token(1)) {
                Some(name) if at == GLIBC_ONLY.1 => Gate::Module(name.to_owned()),
                Some(_) => Gate::Other,
                None => Gate::No,
            };
        }
        let Some(end) = tokens
            .get(item..)
            .unwrap_or_default()
            .iter()
            .position(|token| punct(Some(token)) == Some(';'))
        else {
            return Gate::No;
        };
        let import = ident(token(0)) == Some("pub")
            && ident(token(1)) == Some("use")
            && ident(token(2)).is_some()
            && matches!(token(3).map(|token| &token.kind), Some(TokenKind::PathSep))
            && !tokens
                .get(item..item + end)
                .unwrap_or_default()
                .iter()
                .any(|token| ident(Some(token)) == Some("as"));
        return if import { Gate::Other } else { Gate::No };
    }
    Gate::No
}

/// Reads the sites and problems of one file of the crate, at `place`.
pub(crate) fn scan(tokens: &[Token], place: Place) -> Scan {
    let root = place == Place::Root;
    let mut scan = Scan::default();
    let allowlist = if root { allowlist_range(tokens) } else { None };
    let mut extern_end = 0;
    let mut cfg_lines = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let problem = |what| Problem {
            line: token.line,
            what,
        };
        let Some(word) = ident(Some(token)) else {
            continue;
        };
        // A raw identifier is a name, never a keyword: `r#unsafe` opens no
        // block. But `r#cfg`, `r#path` and the other built-in names below
        // are not keywords, so their raw spelling still names the built-in
        // and is checked like the plain one.
        let keyword = !token.raw;
        match word {
            "unsafe" if keyword && index >= extern_end => {
                if let Some(end) = read_unsafe(tokens, index, &mut scan) {
                    extern_end = end;
                }
            }
            "extern" if keyword && !follows_unsafe(tokens, index) => {
                scan.problems.push(problem(EXTERN_FORM));
            }
            "link" if opens_attribute(tokens, index) && !on_extern_block(tokens, index) => {
                scan.problems.push(problem(LINK));
            }
            "path" if opens_attribute(tokens, index) => scan.problems.push(problem(HIDDEN)),
            word if HIDING_WORDS.contains(&word) => scan.problems.push(problem(HIDDEN)),
            "cfg" | "cfg_attr" if !is_test_gate(tokens, index) => cfg_lines.push(token.line),
            "target_env"
                if !(allowlist
                    .as_ref()
                    .is_some_and(|range| range.contains(&index))
                    || place == Place::Abi && library_gate(tokens, index)) =>
            {
                match glibc_gate(tokens, index) {
                    Gate::Module(module) if root => scan.glibc_only.push(module),
                    Gate::Other if root => {}
                    _ => scan.problems.push(problem(TARGET_ENV)),
                }
            }
            _ => {}
        }
    }
    if place == Place::Library {
        scan.problems.extend(scan.sites.iter().map(|site| Problem {
            line: site.line,
            what: LIBRARY_SITE,
        }));
    }
    if !scan.sites.is_empty() {
        scan.problems.extend(
            cfg_lines
                .into_iter()
                .map(|line| Problem { line, what: CFG }),
        );
    }
    // Every file of the crate: a `core` item anywhere could reach a site's
    // file through a glob import.
    scan.problems.extend(
        core_shadows(tokens)
            .into_iter()
            .map(|line| Problem { line, what: CORE }),
    );
    scan
}

/// Every `unsafe`, foreign declaration or function, and `#[link]` in a
/// file outside the crate. `extern crate` is allowed.
pub(crate) fn outside_crate(tokens: &[Token]) -> Vec<Problem> {
    let mut found = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let flagged = match ident(Some(token)).filter(|_| !token.raw) {
            Some("unsafe") => true,
            Some("extern") => ident(tokens.get(index + 1)) != Some("crate"),
            Some("link") => opens_attribute(tokens, index),
            _ => false,
        };
        if flagged {
            found.push(Problem {
                line: token.line,
                what: OUTSIDE,
            });
        }
    }
    found
}

/// Reads the site an `unsafe` at `index` opens into `scan`; returns the
/// end of an extern block's body, inside which the reader skips the
/// declarations' own `unsafe`.
fn read_unsafe(tokens: &[Token], index: usize, scan: &mut Scan) -> Option<usize> {
    let line = tokens.get(index).map_or(0, |token| token.line);
    let start = statement_start(tokens, index);
    let attributes = tokens.get(start..index).unwrap_or_default();
    let names = expectation_names(attributes);
    if names.len() > 1 {
        scan.problems.push(Problem {
            line,
            what: TWO_NAMES,
        });
    }
    let id = names.into_iter().next();
    let mut push = |kind| {
        scan.sites.push(Site {
            id: id.clone(),
            line,
            kind,
        });
    };
    let next = tokens.get(index + 1);
    match (ident(next), punct(next)) {
        (Some("extern"), _) => {
            let abi = matches!(
                tokens.get(index + 2).map(|token| &token.kind),
                Some(TokenKind::Str(_))
            );
            if abi && ident(tokens.get(index + 3)) == Some("fn") {
                scan.problems.push(Problem {
                    line,
                    what: UNSAFE_ITEM,
                });
                return None;
            }
            let open = index + 3;
            let close = (abi && punct(tokens.get(open)) == Some('{'))
                .then(|| matching(tokens, open, '{', '}'))
                .flatten();
            let Some(close) = close else {
                scan.problems.push(Problem {
                    line,
                    what: EXTERN_FORM,
                });
                return None;
            };
            match extern_items(tokens.get(open + 1..close).unwrap_or_default()) {
                Some(items) => push(SiteKind::Extern {
                    link: link_arguments(attributes),
                    items,
                }),
                None => scan.problems.push(Problem {
                    line,
                    what: EXTERN_ITEM,
                }),
            }
            Some(close + 1)
        }
        (_, Some('{')) => {
            let call = matching(tokens, index + 1, '{', '}')
                .and_then(|close| tokens.get(index + 2..close))
                .and_then(one_call);
            match call {
                Some((operation, arguments)) => push(SiteKind::Block {
                    operation,
                    arguments,
                }),
                None => scan.problems.push(Problem {
                    line,
                    what: BLOCK_BODY,
                }),
            }
            None
        }
        (Some("impl"), _) => {
            push(SiteKind::Impl);
            None
        }
        (Some("fn"), _) if id.as_deref().is_some_and(|id| id.ends_with(STAND_IN)) => {
            push(SiteKind::StandInFn);
            None
        }
        _ => {
            scan.problems.push(Problem {
                line,
                what: UNSAFE_ITEM,
            });
            None
        }
    }
}

/// Where the statement or item holding the token at `index` starts: just
/// after the last `;`, `{` or `}` before it.
fn statement_start(tokens: &[Token], index: usize) -> usize {
    let before = tokens.get(..index).unwrap_or_default();
    before
        .iter()
        .rposition(|token| matches!(punct(Some(token)), Some(';' | '{' | '}')))
        .map_or(0, |at| at + 1)
}

/// The site ids of the `#[expect(unsafe_code, reason = "<id>")]`
/// attributes among `tokens`.
fn expectation_names(tokens: &[Token]) -> Vec<String> {
    let mut names = Vec::new();
    for (index, _) in tokens.iter().enumerate() {
        let at = |offset: usize| tokens.get(index + offset);
        let shape = punct(at(0)) == Some('#')
            && punct(at(1)) == Some('[')
            && ident(at(2)) == Some("expect")
            && punct(at(3)) == Some('(')
            && ident(at(4)) == Some("unsafe_code")
            && punct(at(5)) == Some(',')
            && ident(at(6)) == Some("reason")
            && punct(at(7)) == Some('=')
            && punct(at(9)) == Some(')')
            && punct(at(10)) == Some(']');
        if let (true, Some(TokenKind::Str(name))) = (shape, at(8).map(|token| &token.kind)) {
            names.push(name.clone());
        }
    }
    names
}

/// The `key = "value"` pairs of a `#[link(...)]` among `tokens`.
fn link_arguments(tokens: &[Token]) -> Option<String> {
    let start = tokens.iter().enumerate().position(|(index, _)| {
        punct(tokens.get(index)) == Some('#')
            && punct(tokens.get(index + 1)) == Some('[')
            && ident(tokens.get(index + 2)) == Some("link")
    })?;
    let close = matching(tokens, start + 1, '[', ']')?;
    let inside = tokens.get(start + 3..close).unwrap_or_default();
    let pairs: Vec<String> = inside
        .windows(3)
        .filter_map(|window| match window {
            [key, equals, value] if punct(Some(equals)) == Some('=') => {
                match (ident(Some(key)), &value.kind) {
                    (Some(key), TokenKind::Str(value)) => Some(format!("{key}={value}")),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();
    Some(pairs.join(";"))
}

/// The functions an extern block's body declares, or `None` if it holds
/// anything else.
fn extern_items(body: &[Token]) -> Option<Vec<ExternItem>> {
    let mut items = Vec::new();
    let mut at = 0;
    while at < body.len() {
        while punct(body.get(at)) == Some('#') {
            at = matching(body, at + 1, '[', ']')? + 1;
        }
        if ident(body.get(at)) == Some("pub") {
            at += 1;
            if punct(body.get(at)) == Some('(') {
                at = matching(body, at, '(', ')')? + 1;
            }
        }
        let safe = match ident(body.get(at)) {
            Some("safe") => {
                at += 1;
                true
            }
            Some("unsafe") => {
                at += 1;
                false
            }
            _ => false,
        };
        if ident(body.get(at)) != Some("fn") {
            return None;
        }
        let name = ident(body.get(at + 1))?.to_owned();
        let end = body
            .get(at..)?
            .iter()
            .position(|token| matches!(punct(Some(token)), Some(';' | '{')))?
            + at;
        if punct(body.get(end)) != Some(';') {
            return None;
        }
        items.push(ExternItem { name, safe });
        at = end + 1;
    }
    Some(items)
}

/// The path and arguments of a block body that is exactly one call of a
/// path with plain arguments, and an optional `;`.
fn one_call(body: &[Token]) -> Option<(String, Vec<String>)> {
    let mut at = 0;
    let mut path = String::new();
    if matches!(
        body.first().map(|token| &token.kind),
        Some(TokenKind::PathSep)
    ) {
        path.push_str("::");
        at = 1;
    }
    path.push_str(ident(body.get(at))?);
    at += 1;
    while matches!(
        body.get(at).map(|token| &token.kind),
        Some(TokenKind::PathSep)
    ) {
        path.push_str("::");
        path.push_str(ident(body.get(at + 1))?);
        at += 2;
    }
    if punct(body.get(at)) != Some('(') {
        return None;
    }
    let close = matching(body, at, '(', ')')?;
    let mut arguments = Vec::new();
    let mut argument: Vec<String> = Vec::new();
    for token in body.get(at + 1..close)? {
        let text = match &token.kind {
            TokenKind::Ident(name) => name.clone(),
            TokenKind::PathSep => "::".to_owned(),
            TokenKind::Str(text) => format!("\"{text}\""),
            TokenKind::Punct(',') => {
                arguments.push(std::mem::take(&mut argument).join(" "));
                continue;
            }
            TokenKind::Punct(next @ ('&' | '-')) => next.to_string(),
            TokenKind::Punct(_) | TokenKind::Lifetime => return None,
        };
        argument.push(text);
    }
    if !argument.is_empty() {
        arguments.push(argument.join(" "));
    }
    let rest = body.get(close + 1..).unwrap_or_default();
    let ends = rest.is_empty() || (rest.len() == 1 && punct(rest.first()) == Some(';'));
    ends.then_some((path, arguments))
}

/// Whether the word at `index` is the first inside `#[` or `#![`.
fn opens_attribute(tokens: &[Token], index: usize) -> bool {
    let before = |back: usize| index.checked_sub(back).and_then(|at| tokens.get(at));
    punct(before(1)) == Some('[')
        && (punct(before(2)) == Some('#')
            || (punct(before(2)) == Some('!') && punct(before(3)) == Some('#')))
}

/// Whether the `extern` at `index` follows an `unsafe`.
fn follows_unsafe(tokens: &[Token], index: usize) -> bool {
    index
        .checked_sub(1)
        .and_then(|at| tokens.get(at))
        .is_some_and(|token| !token.raw && ident(Some(token)) == Some("unsafe"))
}

/// Whether the attribute whose first word is at `index` is followed, past
/// any further attributes, by `unsafe extern`.
fn on_extern_block(tokens: &[Token], index: usize) -> bool {
    let Some(mut at) = index
        .checked_sub(1)
        .and_then(|open| matching(tokens, open, '[', ']'))
        .map(|close| close + 1)
    else {
        return false;
    };
    while punct(tokens.get(at)) == Some('#') {
        match matching(tokens, at + 1, '[', ']') {
            Some(close) => at = close + 1,
            None => return false,
        }
    }
    ident(tokens.get(at)) == Some("unsafe") && ident(tokens.get(at + 1)) == Some("extern")
}

/// Whether the `cfg` at `index` is `#[cfg(test)] mod tests;`.
fn is_test_gate(tokens: &[Token], index: usize) -> bool {
    let at = |offset: usize| tokens.get(index + offset);
    opens_attribute(tokens, index)
        && punct(at(1)) == Some('(')
        && ident(at(2)) == Some("test")
        && punct(at(3)) == Some(')')
        && punct(at(4)) == Some(']')
        && ident(at(5)) == Some("mod")
        && ident(at(6)) == Some("tests")
        && punct(at(7)) == Some(';')
}

/// The token range of the crate root's target allowlist: the attribute
/// `#[cfg(not(any(...)))]` directly before the first `compile_error`.
pub(crate) fn allowlist_range(tokens: &[Token]) -> Option<std::ops::Range<usize>> {
    let call = tokens
        .iter()
        .position(|token| ident(Some(token)) == Some("compile_error"))?;
    let close = call.checked_sub(1)?;
    if punct(tokens.get(close)) != Some(']') {
        return None;
    }
    let open = (0..close)
        .rev()
        .find(|&open| matching(tokens, open, '[', ']') == Some(close))?;
    let hash = open.checked_sub(1)?;
    let shape = punct(tokens.get(hash)) == Some('#')
        && ident(tokens.get(open + 1)) == Some("cfg")
        && punct(tokens.get(open + 2)) == Some('(')
        && ident(tokens.get(open + 3)) == Some("not");
    shape.then_some(hash..call)
}
