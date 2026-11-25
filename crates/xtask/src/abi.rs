// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-abi`: the foreign declarations of `ironafs-os` agree with the C
//! headers of the system they run on (ADR 0005, point 5.6).
//!
//! The compiler cannot check a declaration against the C library: a wrong
//! parameter type, struct layout or constant fails silently. So for each
//! row of `sites.json` implemented on the host's target and holding a
//! foreign item, this check reads the row's declarations from the Rust
//! source the compiler read for that target (every file under the row's
//! module directory, `src/<module>/`: its `#[repr(C)]` structs, its
//! constants, its type aliases and its extern blocks' functions, each
//! named as C names it),
//! generates a C11 file of compile-time assertions from them, and compiles
//! it with the platform's own compiler and headers, warnings as errors:
//!
//! - the size and alignment of each struct, and each field's offset, as
//!   `repr(C)` lays them out for the target's data model;
//! - each field's type, by a `_Generic` selection with no default over
//!   the field's address (`T *`), so a field of the wrong type with the
//!   right size and offset fails, and a C array member, which as a value
//!   would decay to a pointer, never passes for a pointer field; an array
//!   member (`[T; N]`) as `T (*)[N]`, so its element type and count are
//!   both checked. A struct's last field may be an
//!   opaque array of integers (its name starts `_opaque` and it is
//!   private): space a C library fills with fields that are never read and
//!   whose names differ between libraries or releases (`struct statvfs`'s
//!   spares). Only a tail `OPAQUE_TAILS` approves, by row, struct, the C
//!   member it follows, element and count, may be opaque; it is bound by
//!   where that C member ends and by its length to the struct's end; a
//!   struct of opaque fields alone is refused;
//! - each constant's value, against its C name, or against the C
//!   expression sites.json maps it to (`expressions`), for a value a macro
//!   gives, such as `CMSG_LEN(sizeof(int))`;
//! - each type alias's size, alignment and sign: an alias names a C
//!   typedef whose spelling differs between C libraries at one size
//!   (`rlim_t`), and a member of that type is checked against the typedef.
//!   An alias of `c_char`, whose sign differs by target, has `char`'s sign
//!   on the target compiled for;
//! - each function's type, by a `_Generic` selection with no default on
//!   its address against a pointer to the declared prototype.
//!
//! Which constants must be asserted comes from the compiled code, not
//! from the editable `sites.json`: every identifier the files compiled for
//! the row name (`src/<module>.rs` and `src/<module>/`) that is an
//! object-like macro of the row's headers (the C preprocessor's `-dM`)
//! must be declared, and so asserted, in the declaration files. The copy's
//! recorded facts (`c_facts`: structs, constants and type aliases) must
//! equal what those files declare. Constants are
//! emitted as normalized decimals, and no file of the crate may name an
//! item `core`, so `core::ffi` is the core crate's.
//!
//! Every type name a declaration uses is resolved in its own file, by a
//! structural rule rather than by evaluating cfgs or scopes. A declaration
//! file may hold, as imports, only file-scope `use` items: unattributed
//! ones from `core`, `std` or `super`, and the gated re-export pairs the
//! shipped files use, each from the child module its cfg names
//! (`#[cfg(target_env = "gnu")]` from `gnu`, `"musl"` from `musl`,
//! `#[cfg(target_arch = ...)]` from that architecture's module, and
//! `#[cfg(not(miri))]` from `foreign`, `#[cfg(miri)]` from `stand_in`).
//! Which gated import is in effect is decided from the target triple (and
//! `check-abi` never runs under Miri), not from which files were compiled.
//! Inline modules, nested imports, imports under any other attribute,
//! `crate::`, `self::` and other child paths, globs and renames are
//! refused. A type name is then read as a primitive, a name imported from
//! `core::ffi`, or a struct or type alias the file declares or imports,
//! through exactly one import in effect, from a declaration file of the
//! row that declares or re-exports it in turn; any other name is refused.
//!
//! That is what this check establishes: the import and gate shapes it
//! reads, and the ones it refuses. It does not establish that the type
//! Rust binds is the type checked where a declaration file holds an
//! item under a cfg, a local item named like a primitive or a `core::ffi`
//! scalar, or a union or enum item (no shipped file holds any of these);
//! TODO(#179): establish it for those shapes too.

mod qualified;
mod self_test;

use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Output;

use crate::metadata::{self, Workspace};
use crate::os::inventory::{Code, Row, withheld_on_musl};
use crate::os::{self, Built};
use crate::rust_source::{self, Token, TokenKind, ident, matching, punct};
use crate::{Command, Error};

#[cfg(test)]
mod tests;

const CHECK: &str = "check-abi";

/// Whether this check is the full proof or the glibc 2.28 compatibility run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Complete,
    LegacyGlibc228,
}

/// Whether an explicit target can select the glibc 2.28 compatibility run.
pub(super) fn legacy_target(target: &str) -> bool {
    os::ALLOWLIST.contains(&target) && target.ends_with("-linux-gnu")
}

/// The target-directory subdirectory `check-abi` builds the crate in, with
/// the compiler Cargo selects; `check-os`'s pinned runs use their own.
pub(crate) const BUILD_DIR: &str = "xtask-check-os";

/// A type as a declaration writes it: a name (its path's last segment), a
/// raw pointer, or an array (a struct member only, as `[T; N]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CType {
    Named(String),
    Pointer { constant: bool, to: Box<CType> },
    Array { of: Box<CType>, count: usize },
}

/// The prefix of an opaque field's name: the last field of a struct, an
/// array, standing for space a C library fills with fields this crate
/// never reads and names differently between libraries or releases
/// (`struct statvfs`'s spare fields). Only a tail `OPAQUE_TAILS` approves
/// may carry it; it is bound by where the C member before it ends and by
/// its length to the struct's end, and the named fields before it keep
/// their offsets and types.
const OPAQUE: &str = "_opaque";

/// An opaque tail this check approves: in `row`'s declarations,
/// `structure`'s last field `field`, following the C member `after`, an
/// array of `count` `element`s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OpaqueTail {
    pub(crate) row: &'static str,
    pub(crate) structure: &'static str,
    pub(crate) field: &'static str,
    pub(crate) after: &'static str,
    pub(crate) element: &'static str,
    pub(crate) count: usize,
}

/// The opaque tails approved, each a class S change of this check, never
/// of a row's own inventory entry: `struct statvfs`'s 24 bytes after
/// `f_namemax` (glibc 2.28 `int __f_spare[6]`, glibc 2.41 `unsigned int
/// f_type` and `int __f_spare[5]`, musl `unsigned int f_type` and `int
/// __reserved[5]`, measured).
pub(crate) const OPAQUE_TAILS: &[OpaqueTail] = &[OpaqueTail {
    row: "linux-statvfs",
    structure: "statvfs",
    field: "_opaque_tail",
    after: "f_namemax",
    element: "c_int",
    count: 6,
}];

/// A `#[repr(C)]` struct and its fields in order.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Struct {
    pub(crate) name: String,
    pub(crate) fields: Vec<(String, CType)>,
}

/// A function of an extern block.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Function {
    pub(crate) name: String,
    pub(crate) parameters: Vec<CType>,
    pub(crate) variadic: bool,
    pub(crate) returns: Option<CType>,
}

/// Everything a row's declaration files state about C.
#[derive(Debug, Default)]
pub(crate) struct Declarations {
    pub(crate) structs: Vec<Struct>,
    /// Each constant's name and value, as written (an integer, perhaps
    /// negative).
    pub(crate) constants: Vec<(String, String)>,
    pub(crate) functions: Vec<Function>,
    /// Each `type NAME = <scalar>;`: a C typedef the declarations name.
    pub(crate) aliases: Vec<(String, CType)>,
    /// The names the declaration files import from `core::ffi`: only these
    /// spell C's types (a `c_int` from anywhere else is not C's `int`).
    pub(crate) ffi_names: Vec<String>,
    /// Every import of the file, `core::ffi`'s included.
    pub(crate) imports: Vec<Import>,
    /// The opaque tails `OPAQUE_TAILS` approves for the row: the only
    /// fields that may be opaque.
    pub(crate) opaque: Vec<OpaqueTail>,
}

/// The sizes of C's integer types on the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DataModel {
    /// Linux and macOS on 64-bit: `long` and pointers are 8 bytes.
    Lp64,
    /// 64-bit Windows: `long` is 4 bytes, pointers 8.
    Llp64,
}

impl DataModel {
    fn of(target: &str) -> Self {
        if target.contains("-windows-") {
            Self::Llp64
        } else {
            Self::Lp64
        }
    }
}

/// Reads the declarations among `tokens` (read with their strings) into
/// `found`: every `#[repr(C)]` struct, every `const`, every function of an
/// `extern "<abi>"` block, and the names imported from `core::ffi`.
///
/// # Errors
///
/// Returns what the reader refuses rather than guess at: an extern block
/// whose ABI is not `abi`, an attribute on a foreign function (such as
/// `link_name`, which would link another symbol), a struct whose `repr` is
/// not exactly `C` (such as `packed` or `align`, which change its layout),
/// a renaming import, a constant that is not an integer literal, or a
/// declaration it cannot read exactly.
pub(crate) fn declarations(
    tokens: &[Token],
    abi: &str,
    found: &mut Declarations,
) -> Result<(), String> {
    if let Some(line) = rust_source::core_shadows(tokens).first() {
        return Err(format!(
            "line {line}: an item, import or generic parameter named `core`, under which \
             `core::ffi` would not be the core crate's"
        ));
    }
    if let Some(token) = tokens.iter().enumerate().find_map(|(at, token)| {
        (ident(Some(token)) == Some("mod") && !token.raw && punct(tokens.get(at + 2)) == Some('{'))
            .then_some(token)
    }) {
        return Err(format!(
            "line {}: an inline module, whose imports and items this check does not resolve",
            token.line
        ));
    }
    read_imports(tokens, found)?;
    let mut at = 0;
    while let Some(token) = tokens.get(at) {
        let line = token.line;
        match ident(Some(token)).filter(|_| !token.raw) {
            Some("struct") => {
                repr_c_only(tokens, at).map_err(|error| format!("line {line}: {error}"))?;
                if public_opaque(tokens, at) {
                    return Err(format!(
                        "line {line}: an opaque field is private, so no code outside its \
                         struct's module reads it"
                    ));
                }
                let (item, next) = read_struct(tokens, at)
                    .ok_or_else(|| format!("line {line}: a struct that cannot be read"))?;
                found.structs.push(item);
                at = next;
            }
            Some("type") => {
                let (alias, next) = read_alias(tokens, at).ok_or_else(|| {
                    format!("line {line}: a type alias that is not `NAME = <C scalar>;`")
                })?;
                // A C typedef's own name (`rlim_t`), never a scalar's: an
                // alias named `c_long` or `u32` would make that name not
                // mean what it says.
                if scalar(&alias.0, DataModel::Lp64).is_some() || alias.0 == "c_void" {
                    return Err(format!(
                        "line {line}: a type alias named as a scalar, under which a name would \
                         not mean what it says"
                    ));
                }
                found.aliases.push(alias);
                at = next;
            }
            Some("const") => {
                let (constant, next) = read_constant(tokens, at).ok_or_else(|| {
                    format!("line {line}: a constant that is not `NAME: TYPE = <integer>;`")
                })?;
                found.constants.push(constant);
                at = next;
            }
            Some("use") => {
                // Read, with its context, by `read_imports` before the loop.
                at = tokens
                    .get(at..)
                    .unwrap_or_default()
                    .iter()
                    .position(|token| punct(Some(token)) == Some(';'))
                    .map_or(tokens.len(), |offset| at + offset + 1);
            }
            Some("extern") => {
                let written = match tokens.get(at + 1).map(|token| &token.kind) {
                    Some(TokenKind::Str(written)) => written.as_str(),
                    _ => "",
                };
                if written != abi || punct(tokens.get(at + 2)) != Some('{') {
                    return Err(format!(
                        "line {line}: an extern block whose ABI is \"{written}\", not \"{abi}\""
                    ));
                }
                let close = matching(tokens, at + 2, '{', '}')
                    .ok_or_else(|| format!("line {line}: an unclosed extern block"))?;
                let body = tokens.get(at + 3..close).unwrap_or_default();
                if body.iter().any(|token| punct(Some(token)) == Some('#')) {
                    return Err(format!(
                        "line {line}: an attribute on a foreign function (such as link_name), \
                         which this check cannot compare with C"
                    ));
                }
                read_functions(body, found)
                    .ok_or_else(|| format!("line {line}: an extern block that cannot be read"))?;
                at = close + 1;
            }
            _ => at += 1,
        }
    }
    Ok(())
}

/// The cfg under which one of a declaration file's imports is in effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Gate {
    /// No attribute: always.
    Always,
    /// `#[cfg(target_env = "<env>")] pub(super) use <env>::...;`: on that
    /// C library's targets.
    Env(String),
    /// `#[cfg(target_arch = "<arch>")] pub(super) use <arch>::...;`: on
    /// that architecture's targets.
    Arch(String),
    /// `#[cfg(not(miri))] pub(super) use foreign::...;` (false) or
    /// `#[cfg(miri)] pub(super) use stand_in::...;` (true): never under
    /// Miri, or only under it. `check-abi` builds without Miri.
    Miri(bool),
}

impl Gate {
    /// Whether an import under this gate is in effect on `target`
    /// (`check-abi` never runs under Miri).
    pub(crate) fn in_effect(&self, target: &str) -> bool {
        match self {
            Self::Always => true,
            Self::Env(env) => target.rsplit('-').next() == Some(env.as_str()),
            Self::Arch(arch) => target.split('-').next() == Some(arch.as_str()),
            Self::Miri(miri) => !miri,
        }
    }
}

/// One import of a declaration file: the path it names, ending in the name
/// it binds, and the cfg under which it is in effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Import {
    pub(crate) path: Vec<String>,
    pub(crate) gate: Gate,
}

/// The one gated import shape for each cfg a declaration file may put on
/// an import: the attribute as written, and the module its paths must
/// start at.
fn gate_of(attribute: &str, first: &str) -> Option<Gate> {
    let gate = match attribute {
        "cfg(target_env=\"gnu\")" => Gate::Env("gnu".to_owned()),
        "cfg(target_env=\"musl\")" => Gate::Env("musl".to_owned()),
        "cfg(target_arch=\"aarch64\")" => Gate::Arch("aarch64".to_owned()),
        "cfg(target_arch=\"x86_64\")" => Gate::Arch("x86_64".to_owned()),
        "cfg(not(miri))" => Gate::Miri(false),
        "cfg(miri)" => Gate::Miri(true),
        _ => return None,
    };
    let module = match &gate {
        Gate::Env(name) | Gate::Arch(name) => name.as_str(),
        Gate::Miri(false) => "foreign",
        Gate::Miri(true) => "stand_in",
        Gate::Always => return None,
    };
    (first == module).then_some(gate)
}

/// Reads every import of a declaration file into `found`, refusing each
/// form whose binding this check does not establish structurally: an
/// import not at file scope, a glob, a rename, a tree it cannot read, an
/// unattributed import that does not start at `core`, `std` or `super`
/// (`crate::`, `self::` and child-module paths), and an attributed import
/// other than the per-C-library, per-architecture and Miri pairs, each of
/// which re-exports from the child module its cfg names.
fn read_imports(tokens: &[Token], found: &mut Declarations) -> Result<(), String> {
    for item in rust_source::use_items(tokens) {
        let line = item.line;
        if item.renamed {
            return Err(format!(
                "line {line}: a renaming import, under which a name would not mean what it says"
            ));
        }
        if item.glob || !item.plain {
            return Err(format!(
                "line {line}: an import this check cannot read, or a glob import, which could \
                 bind any name"
            ));
        }
        if item.depth != 0 {
            return Err(format!(
                "line {line}: an import not at file scope, whose scope this check does not \
                 resolve"
            ));
        }
        for path in item.paths {
            let first = path.first().map_or("", String::as_str);
            let gate = match item.attributes.as_slice() {
                [] if matches!(first, "core" | "std" | "super") && path.len() > 1 => Gate::Always,
                [] => {
                    return Err(format!(
                        "line {line}: an import from {}, a path (crate::, self:: or a child \
                         module) this check does not resolve",
                        path.join("::")
                    ));
                }
                [attribute] if path.len() == 2 => gate_of(attribute, first).ok_or_else(|| {
                    format!(
                        "line {line}: an import under #[{attribute}]; only the per-C-library, \
                         per-architecture and Miri pairs, each from the module its cfg names, \
                         are read"
                    )
                })?,
                _ => {
                    return Err(format!(
                        "line {line}: an import with attributes this check does not read"
                    ));
                }
            };
            if let ([first, second, name], Gate::Always) = (path.as_slice(), &gate)
                && first == "core"
                && second == "ffi"
            {
                found.ffi_names.push(name.clone());
            }
            found.imports.push(Import { path, gate });
        }
    }
    Ok(())
}

/// Checks that the attributes before the `struct` at `at` include
/// `#[repr(C)]` and no other `repr`.
fn repr_c_only(tokens: &[Token], at: usize) -> Result<(), String> {
    let start = tokens
        .get(..at)
        .unwrap_or_default()
        .iter()
        .rposition(|token| matches!(punct(Some(token)), Some(';' | '}')))
        .map_or(0, |found| found + 1);
    let before = tokens.get(start..at).unwrap_or_default();
    let mut repr_c = false;
    for (index, token) in before.iter().enumerate() {
        if ident(Some(token)) != Some("repr") {
            continue;
        }
        let close =
            matching(before, index + 1, '(', ')').ok_or("a repr attribute that cannot be read")?;
        let inside = before.get(index + 2..close).unwrap_or_default();
        if inside.len() == 1 && ident(inside.first()) == Some("C") {
            repr_c = true;
        } else {
            return Err(
                "a struct whose repr is not exactly C (packed or align change its layout)"
                    .to_owned(),
            );
        }
    }
    if repr_c {
        Ok(())
    } else {
        Err("a struct in a declaration file is #[repr(C)]".to_owned())
    }
}

/// Whether the struct at `at` gives an opaque field a visibility.
fn public_opaque(tokens: &[Token], at: usize) -> bool {
    let Some(close) = matching(tokens, at + 2, '{', '}') else {
        return false;
    };
    let body = tokens.get(at + 3..close).unwrap_or_default();
    body.iter().enumerate().any(|(index, token)| {
        ident(Some(token)) == Some("pub")
            && ident(body.get(skip_visibility(body, index)))
                .is_some_and(|name| name.starts_with(OPAQUE))
    })
}

/// Skips a visibility (`pub`, `pub(...)`) at `at`.
fn skip_visibility(tokens: &[Token], mut at: usize) -> usize {
    if ident(tokens.get(at)) == Some("pub") {
        at += 1;
        if punct(tokens.get(at)) == Some('(') {
            while tokens.get(at).is_some() && punct(tokens.get(at)) != Some(')') {
                at += 1;
            }
            at += 1;
        }
    }
    at
}

/// Reads a type starting at `at`; returns it and the index after it.
fn read_type(tokens: &[Token], at: usize) -> Option<(CType, usize)> {
    if punct(tokens.get(at)) == Some('[') {
        let (of, next) = read_type(tokens, at + 1)?;
        if punct(tokens.get(next)) != Some(';') {
            return None;
        }
        let digits = ident(tokens.get(next + 1))?;
        if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let count = digits.parse().ok()?;
        if punct(tokens.get(next + 2)) != Some(']') {
            return None;
        }
        return Some((
            CType::Array {
                of: Box::new(of),
                count,
            },
            next + 3,
        ));
    }
    if punct(tokens.get(at)) == Some('*') {
        let constant = match ident(tokens.get(at + 1))? {
            "const" => true,
            "mut" => false,
            _ => return None,
        };
        let (to, next) = read_type(tokens, at + 2)?;
        return Some((
            CType::Pointer {
                constant,
                to: Box::new(to),
            },
            next,
        ));
    }
    let mut next = at;
    if matches!(
        tokens.get(next).map(|token| &token.kind),
        Some(TokenKind::PathSep)
    ) {
        next += 1;
    }
    let mut name = ident(tokens.get(next))?.to_owned();
    next += 1;
    while matches!(
        tokens.get(next).map(|token| &token.kind),
        Some(TokenKind::PathSep)
    ) {
        name.push_str("::");
        name.push_str(ident(tokens.get(next + 1))?);
        next += 2;
    }
    Some((CType::Named(name), next))
}

fn read_struct(tokens: &[Token], at: usize) -> Option<(Struct, usize)> {
    let name = ident(tokens.get(at + 1))?.to_owned();
    let open = at + 2;
    if punct(tokens.get(open)) != Some('{') {
        return None;
    }
    let close = matching(tokens, open, '{', '}')?;
    let mut fields = Vec::new();
    let mut next = open + 1;
    while next < close {
        next = skip_visibility(tokens, next);
        let field = ident(tokens.get(next))?.to_owned();
        if punct(tokens.get(next + 1)) != Some(':') {
            return None;
        }
        let (field_type, after) = read_type(tokens, next + 2)?;
        fields.push((field, field_type));
        next = after;
        if punct(tokens.get(next)) == Some(',') {
            next += 1;
        }
    }
    Some((Struct { name, fields }, close + 1))
}

fn read_constant(tokens: &[Token], at: usize) -> Option<((String, String), usize)> {
    let name = ident(tokens.get(at + 1))?.to_owned();
    if punct(tokens.get(at + 2)) != Some(':') {
        return None;
    }
    let (_, after) = read_type(tokens, at + 3)?;
    if punct(tokens.get(after)) != Some('=') {
        return None;
    }
    let mut next = after + 1;
    let mut value = String::new();
    if punct(tokens.get(next)) == Some('-') {
        value.push('-');
        next += 1;
    }
    let digits = ident(tokens.get(next))?;
    if !digits
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'_')
    {
        return None;
    }
    // Rust reads `046` as forty-six, C as an octal thirty-eight: the value
    // is emitted as its normalized decimal, never as written.
    let number: u128 = digits.replace('_', "").parse().ok()?;
    value.push_str(&number.to_string());
    if punct(tokens.get(next + 1)) != Some(';') {
        return None;
    }
    Some(((name, value), next + 2))
}

/// Reads `type NAME = <scalar>;` at `at`: a C typedef whose type differs
/// between C libraries at one size (`rlim_t` is `unsigned long` in glibc
/// and `unsigned long long` in musl), so the C side names the typedef and
/// asserts its size, alignment and sign instead of one spelling.
fn read_alias(tokens: &[Token], at: usize) -> Option<((String, CType), usize)> {
    let name = ident(tokens.get(at + 1))?.to_owned();
    if punct(tokens.get(at + 2)) != Some('=') {
        return None;
    }
    let (aliased, next) = read_type(tokens, at + 3)?;
    let CType::Named(scalar_name) = &aliased else {
        return None;
    };
    scalar(
        scalar_name
            .strip_prefix("core::ffi::")
            .unwrap_or(scalar_name),
        DataModel::Lp64,
    )?;
    if punct(tokens.get(next)) != Some(';') {
        return None;
    }
    Some(((name, aliased), next + 1))
}

/// Reads the functions of an extern block's body.
fn read_functions(body: &[Token], found: &mut Declarations) -> Option<()> {
    let mut at = 0;
    while at < body.len() {
        while punct(body.get(at)) == Some('#') {
            while body.get(at).is_some() && punct(body.get(at)) != Some(']') {
                at += 1;
            }
            at += 1;
        }
        at = skip_visibility(body, at);
        if matches!(ident(body.get(at)), Some("safe" | "unsafe")) {
            at += 1;
        }
        if ident(body.get(at)) != Some("fn") {
            return None;
        }
        let name = ident(body.get(at + 1))?.to_owned();
        if punct(body.get(at + 2)) != Some('(') {
            return None;
        }
        at += 3;
        let mut parameters = Vec::new();
        let mut variadic = false;
        loop {
            match punct(body.get(at)) {
                Some(')') => {
                    at += 1;
                    break;
                }
                Some(',') => at += 1,
                Some('.') => {
                    variadic = true;
                    while punct(body.get(at)) == Some('.') {
                        at += 1;
                    }
                }
                _ => {
                    ident(body.get(at))?;
                    if punct(body.get(at + 1)) != Some(':') {
                        return None;
                    }
                    let (parameter, next) = read_type(body, at + 2)?;
                    parameters.push(parameter);
                    at = next;
                }
            }
        }
        let returns = if punct(body.get(at)) == Some('-') && punct(body.get(at + 1)) == Some('>') {
            let (returned, next) = read_type(body, at + 2)?;
            at = next;
            Some(returned)
        } else {
            None
        };
        if punct(body.get(at)) != Some(';') {
            return None;
        }
        at += 1;
        found.functions.push(Function {
            name,
            parameters,
            variadic,
            returns,
        });
    }
    Some(())
}

/// The C spelling, size and alignment of a named type, or `None` for a
/// name with no C type here (a struct is looked up separately).
fn scalar(name: &str, model: DataModel) -> Option<(&'static str, usize)> {
    let long = match model {
        DataModel::Lp64 => 8,
        DataModel::Llp64 => 4,
    };
    Some(match name {
        "c_char" => ("char", 1),
        "c_schar" => ("signed char", 1),
        "c_uchar" => ("unsigned char", 1),
        "c_short" => ("short", 2),
        "c_ushort" => ("unsigned short", 2),
        "c_int" => ("int", 4),
        "c_uint" => ("unsigned int", 4),
        "c_long" => ("long", long),
        "c_ulong" => ("unsigned long", long),
        "c_longlong" => ("long long", 8),
        "c_ulonglong" => ("unsigned long long", 8),
        "i8" => ("int8_t", 1),
        "u8" => ("uint8_t", 1),
        "i16" => ("int16_t", 2),
        "u16" => ("uint16_t", 2),
        "i32" => ("int32_t", 4),
        "u32" => ("uint32_t", 4),
        "i64" => ("int64_t", 8),
        "u64" => ("uint64_t", 8),
        "usize" => ("size_t", 8),
        "isize" => ("ssize_t", 8),
        _ => return None,
    })
}

/// The C spelling of a type, as it would declare an abstract declarator's
/// base: `int`, `struct timespec`, `const char *`.
fn c_name(found: &CType, declarations: &Declarations, model: DataModel) -> Result<String, String> {
    match found {
        CType::Named(name) => match resolve(name, declarations)? {
            Resolved::Scalar("c_void") => Ok("void".to_owned()),
            Resolved::Scalar(name) => scalar(name, model)
                .map(|(spelling, _)| spelling.to_owned())
                .ok_or_else(|| format!("`{name}` has no C type this check knows")),
            Resolved::Struct(name) => Ok(format!("struct {name}")),
            Resolved::Alias(name, _) => Ok(name.to_owned()),
        },
        CType::Array { .. } => Err(ARRAY_PLACE.to_owned()),
        CType::Pointer { constant, to } => {
            let base = c_name(to, declarations, model)?;
            Ok(if *constant {
                format!("const {base} *")
            } else {
                format!("{base} *")
            })
        }
    }
}

/// Where an array may stand.
const ARRAY_PLACE: &str = "an array is a struct member only, never a parameter or return type";

/// The C type of a pointer to a struct member, which `_Generic` compares
/// with the member's address: `T (*)[N]` for an array, so the element type
/// and the count are checked; `T *` for any other member. The address,
/// not the member's value: a value undergoes lvalue conversion, so a C
/// array member would decay to a pointer and match a pointer field, and
/// its top-level qualifiers would be dropped.
fn member_check(
    item: &str,
    field: &str,
    found: &CType,
    declarations: &Declarations,
    model: DataModel,
) -> Result<String, String> {
    if let CType::Array { of, count } = found {
        let element = c_name(of, declarations, model)?;
        return Ok(format!(
            "_Generic(&((struct {item} *)0)->{field}, {element} (*)[{count}]: 1)"
        ));
    }
    let spelled = c_name(found, declarations, model)?;
    Ok(format!(
        "_Generic(&((struct {item} *)0)->{field}, {spelled} *: 1)"
    ))
}

/// What a type's name refers to.
enum Resolved<'a> {
    /// A `core::ffi` type or a primitive integer, by its name.
    Scalar(&'a str),
    /// A struct of the declaration files.
    Struct(&'a str),
    /// A type alias of the declaration files (a C typedef), and the scalar
    /// it aliases.
    Alias(&'a str, &'a CType),
}

/// The primitive integer types, which no import can rename (renaming
/// imports are refused).
const PRIMITIVES: &[&str] = &[
    "i8", "u8", "i16", "u16", "i32", "u32", "i64", "u64", "usize", "isize",
];

/// Resolves a type name by its identity, not its last segment: a C type is
/// a name imported from `core::ffi` or written `core::ffi::<name>`; a
/// primitive integer is itself; any other name must be a struct the
/// declaration files declare.
fn resolve<'a>(name: &'a str, declarations: &'a Declarations) -> Result<Resolved<'a>, String> {
    if let Some(rest) = name.strip_prefix("core::ffi::") {
        return if rest.contains("::") {
            Err(format!("`{name}` is no type of core::ffi"))
        } else {
            Ok(Resolved::Scalar(rest))
        };
    }
    if name.contains("::") {
        return Err(format!(
            "`{name}`: a type path other than core::ffi, whose identity this check cannot prove"
        ));
    }
    if PRIMITIVES.contains(&name) {
        return Ok(Resolved::Scalar(name));
    }
    if declarations
        .ffi_names
        .iter()
        .any(|imported| imported == name)
    {
        return Ok(Resolved::Scalar(name));
    }
    if let Some((_, aliased)) = declarations.aliases.iter().find(|(alias, _)| alias == name) {
        return Ok(Resolved::Alias(name, aliased));
    }
    if declarations.structs.iter().any(|item| item.name == name) {
        return Ok(Resolved::Struct(name));
    }
    Err(format!(
        "`{name}` is neither imported from core::ffi, a primitive integer, nor a struct or type alias \
         of the declaration files"
    ))
}

/// Whether a scalar is unsigned (`Some(true)`), signed (`Some(false)`), or
/// differs by target (`c_char`).
fn unsigned(name: &str) -> Option<bool> {
    match name {
        "c_char" => None,
        "u8" | "u16" | "u32" | "u64" | "usize" | "c_uchar" | "c_ushort" | "c_uint" | "c_ulong"
        | "c_ulonglong" => Some(true),
        _ => Some(false),
    }
}

/// Refuses an expression mapped to a constant the declarations do not
/// hold, which would otherwise prove nothing.
fn unmapped_expressions(
    expressions: &[(String, String)],
    declarations: &Declarations,
) -> Result<(), String> {
    for (name, _) in expressions {
        if !declarations
            .constants
            .iter()
            .any(|(constant, _)| constant == name)
        {
            return Err(format!(
                "sites.json maps `{name}` to a C expression, but the declarations hold no such \
                 constant"
            ));
        }
    }
    Ok(())
}

/// The size and alignment of a type under `repr(C)`.
fn layout(
    found: &CType,
    declarations: &Declarations,
    model: DataModel,
    depth: usize,
) -> Result<(usize, usize), String> {
    match found {
        CType::Pointer { .. } => Ok((8, 8)),
        CType::Array { of, count } => {
            if *count == 0 {
                return Err("an array of no elements, which C11 does not allow".to_owned());
            }
            let (size, align) = layout(of, declarations, model, depth)?;
            let total = size
                .checked_mul(*count)
                .ok_or_else(|| "an array larger than memory".to_owned())?;
            Ok((total, align))
        }
        CType::Named(name) => match resolve(name, declarations)? {
            Resolved::Scalar("c_void") => Err("`c_void` has no size".to_owned()),
            Resolved::Scalar(name) => scalar(name, model)
                .map(|(_, size)| (size, size))
                .ok_or_else(|| format!("`{name}` has no C type this check knows")),
            Resolved::Alias(name, aliased) => {
                if depth > 8 {
                    return Err(format!("type {name} nests too deep"));
                }
                layout(aliased, declarations, model, depth + 1)
            }
            Resolved::Struct(name) => {
                let item = declarations
                    .structs
                    .iter()
                    .find(|item| item.name == name)
                    .ok_or_else(|| format!("struct {name} is not declared"))?;
                if depth > 8 {
                    return Err(format!("struct {name} nests too deep"));
                }
                struct_layout(item, declarations, model, depth + 1)
                    .map(|(size, align, _)| (size, align))
            }
        },
    }
}

/// A struct's size, alignment and field offsets under `repr(C)`.
fn struct_layout(
    item: &Struct,
    declarations: &Declarations,
    model: DataModel,
    depth: usize,
) -> Result<(usize, usize, Vec<usize>), String> {
    let mut offset = 0_usize;
    let mut align = 1_usize;
    let mut offsets = Vec::new();
    for (_, field) in &item.fields {
        let (size, field_align) = layout(field, declarations, model, depth)?;
        offset = offset.next_multiple_of(field_align);
        offsets.push(offset);
        offset += size;
        align = align.max(field_align);
    }
    Ok((offset.next_multiple_of(align), align, offsets))
}

/// The assertions of one struct: its size and alignment, and each named
/// field's offset and type; an approved opaque tail by where the C member
/// before it ends and by the bytes from there to the struct's end.
fn struct_assertions(
    item: &Struct,
    declarations: &Declarations,
    model: DataModel,
) -> Result<Vec<(String, String)>, String> {
    let name = &item.name;
    let (size, align, offsets) = struct_layout(item, declarations, model, 0)?;
    let mut found = vec![
        (
            format!("sizeof(struct {name}) == {size}"),
            format!("sizeof(struct {name})"),
        ),
        (
            format!("_Alignof(struct {name}) == {align}"),
            format!("_Alignof(struct {name})"),
        ),
    ];
    let last = item.fields.len().saturating_sub(1);
    if item
        .fields
        .iter()
        .all(|(field, _)| field.starts_with(OPAQUE))
        && !item.fields.is_empty()
    {
        return Err(format!(
            "struct {name}: every member is opaque, so no member would be checked"
        ));
    }
    for (index, ((field, field_type), offset)) in item.fields.iter().zip(offsets).enumerate() {
        if field.starts_with(OPAQUE) {
            let Some(tail) = declarations
                .opaque
                .iter()
                .find(|tail| tail.structure == name && tail.field == field)
            else {
                return Err(format!(
                    "struct {name}: `{field}` is opaque, but xtask's allowlist of opaque tails \
                     does not approve it"
                ));
            };
            let before = index
                .checked_sub(1)
                .and_then(|previous| item.fields.get(previous))
                .map(|(previous, _)| previous.as_str());
            if index != last
                || before != Some(tail.after)
                || !tail_array(field_type, tail, declarations)
            {
                return Err(format!(
                    "struct {name}: the opaque tail `{field}` is the last field, after `{}`, and \
                     {} of {}, as xtask's allowlist approves it",
                    tail.after, tail.count, tail.element
                ));
            }
            let (length, _) = layout(field_type, declarations, model, 0)?;
            let after = tail.after;
            found.push((
                format!(
                    "offsetof(struct {name}, {after}) + sizeof(((struct {name} *)0)->{after}) == \
                     {offset}"
                ),
                format!("the start of {name}'s opaque tail"),
            ));
            found.push((
                format!("sizeof(struct {name}) - {offset} == {length}"),
                format!("the length of {name}'s opaque tail"),
            ));
            continue;
        }
        found.push((
            format!("offsetof(struct {name}, {field}) == {offset}"),
            format!("offsetof(struct {name}, {field})"),
        ));
        found.push((
            member_check(name, field, field_type, declarations, model)?,
            format!("type of {name}.{field}"),
        ));
    }
    Ok(found)
}

/// Whether `found` is `tail`'s array: `count` of its integer `element`.
fn tail_array(found: &CType, tail: &OpaqueTail, declarations: &Declarations) -> bool {
    let CType::Array { of, count } = found else {
        return false;
    };
    let CType::Named(element) = of.as_ref() else {
        return false;
    };
    let resolved = match resolve(element, declarations) {
        Ok(Resolved::Scalar(name)) => name.strip_prefix("core::ffi::").unwrap_or(name),
        _ => return false,
    };
    *count == tail.count && resolved == tail.element
}

/// The C11 file of assertions for one row.
///
/// # Errors
///
/// Returns why a declaration has no C equivalent this check can state.
pub(crate) fn c_source(
    row: &str,
    defines: &[String],
    headers: &[String],
    expressions: &[(String, String)],
    declarations: &Declarations,
    model: DataModel,
) -> Result<String, String> {
    let mut source = preamble(row, defines, headers, model);
    let mut assert = |condition: String, what: String| {
        let _ = writeln!(source, "_Static_assert({condition}, \"{row}: {what}\");");
    };
    for (name, aliased) in &declarations.aliases {
        let (size, align) = layout(aliased, declarations, model, 0)?;
        assert(
            format!("sizeof({name}) == {size}"),
            format!("sizeof({name})"),
        );
        assert(
            format!("_Alignof({name}) == {align}"),
            format!("_Alignof({name})"),
        );
        let CType::Named(aliased_name) = aliased else {
            return Err(format!("`{name}` aliases a type that is not a scalar"));
        };
        let Resolved::Scalar(scalar_name) = resolve(aliased_name, declarations)? else {
            return Err(format!("`{name}` aliases a type that is not a scalar"));
        };
        // Every alias names an integer: `^` is a constraint violation on a
        // floating or pointer type under every flag, and `2 != 1` after the
        // casts refuses `_Bool`, which a cast of -1 would let pass as
        // unsigned. The operands of each comparison have one type, so no
        // sign-comparison warning arises.
        assert(
            format!("(({name})0 ^ ({name})0) == ({name})0 && ({name})2 != ({name})1"),
            format!("{name} is an integer type other than _Bool"),
        );
        match unsigned(scalar_name) {
            Some(true) => assert(format!("({name})-1 > 0"), format!("{name} is unsigned")),
            Some(false) => assert(format!("({name})-1 < 0"), format!("{name} is signed")),
            // `c_char`: the generated file is compiled for the target
            // proved, where C's `char` has `c_char`'s sign.
            None => assert(
                format!("(({name})-1 > 0) == ((char)-1 > 0)"),
                format!("{name} has char's sign"),
            ),
        }
    }
    for tail in &declarations.opaque {
        let held = declarations.structs.iter().any(|item| {
            item.name == tail.structure && item.fields.iter().any(|(field, _)| field == tail.field)
        });
        if !held {
            return Err(format!(
                "xtask's allowlist approves the opaque tail `{}.{}`, which the declarations do \
                 not hold",
                tail.structure, tail.field
            ));
        }
    }
    for item in &declarations.structs {
        for (condition, what) in struct_assertions(item, declarations, model)? {
            assert(condition, what);
        }
    }
    unmapped_expressions(expressions, declarations)?;
    for (name, value) in &declarations.constants {
        match expressions.iter().find(|(constant, _)| constant == name) {
            Some((_, expression)) => assert(format!("({expression}) == {value}"), name.clone()),
            None => assert(format!("{name} == {value}"), name.clone()),
        }
    }
    for function in &declarations.functions {
        let returns = match &function.returns {
            Some(returned) => c_name(returned, declarations, model)?,
            None => "void".to_owned(),
        };
        let mut parameters = function
            .parameters
            .iter()
            .map(|parameter| c_name(parameter, declarations, model))
            .collect::<Result<Vec<_>, _>>()?;
        if function.variadic {
            parameters.push("...".to_owned());
        }
        let list = if parameters.is_empty() {
            "void".to_owned()
        } else {
            parameters.join(", ")
        };
        let name = &function.name;
        assert(
            format!("_Generic(&{name}, {returns} (*)({list}): 1)"),
            format!("type of {name}"),
        );
    }
    Ok(source)
}

/// The start of every generated C file of a row, the macro scan's and the
/// assertions' alike: the feature macros (`defines`), the POSIX and
/// default declarations (`_DEFAULT_SOURCE`), the common headers and the
/// row's own. The scan's `defines` are the assertions' plus `_GNU_SOURCE`
/// ([`scan_defines`]), so the macros the assertions see are a subset of
/// those the scan sees: a constant the assertions could prove is one the
/// scan finds.
pub(crate) fn preamble(
    row: &str,
    defines: &[String],
    headers: &[String],
    model: DataModel,
) -> String {
    let mut source = format!(
        "/* Generated by `cargo xtask check-abi` for ironafs-os site {row}; never linked. */\n"
    );
    for define in defines {
        let _ = writeln!(source, "#define {define} 1");
    }
    source.push_str(
        "#define _DEFAULT_SOURCE 1\n#define _DARWIN_C_SOURCE 1\n\
         #include <stddef.h>\n#include <stdint.h>\n",
    );
    if model == DataModel::Lp64 {
        source.push_str("#include <sys/types.h>\n");
    }
    for header in headers {
        let _ = writeln!(source, "#include <{header}>");
    }
    source
}

/// The feature macros of the macro scan: the row's `defines`, which the
/// assertions also read, and `_GNU_SOURCE` if the row does not name it. The
/// scan only preprocesses, so the widest feature set costs nothing there,
/// and a macro glibc exposes only under `_GNU_SOURCE` (`O_PATH`) that the
/// row's code names is found even when the row has not opted in; the
/// assertions then fail to compile until it does. The assertions keep the
/// row's own set: a global `_GNU_SOURCE` would change which prototypes
/// they are proved against (glibc's `setrlimit` takes an enum under it).
pub(crate) fn scan_defines(defines: &[String]) -> Vec<String> {
    let mut scanned = defines.to_vec();
    if !scanned.iter().any(|define| define == "_GNU_SOURCE") {
        scanned.push("_GNU_SOURCE".to_owned());
    }
    scanned
}

/// The object-like macros the C compiler's preprocessor defines after the
/// row's `preamble` with the scan's feature macros ([`scan_defines`])
/// (`$CC -E -dM`), with the same compiler as `compile`.
///
/// # Errors
///
/// Returns an error if the file cannot be written or the compiler started;
/// a preprocessor failure is `Ok(Err(..))`.
pub(crate) fn header_macros(
    row: &str,
    defines: &[String],
    headers: &[String],
    model: DataModel,
    file: &Path,
) -> Result<Result<std::collections::BTreeSet<String>, String>, Error> {
    crate::fixture::write_file(file, &preamble(row, &scan_defines(defines), headers, model))?;
    let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    let output = Command::new(compiler)
        .args(["-std=c11", "-E", "-dM"])
        .arg(file)
        .output()
        .map_err(Error::io("run the C preprocessor"))?;
    if !output.status.success() {
        return Ok(Err(format!(
            "the C preprocessor failed on the row's headers: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or_default()
        )));
    }
    Ok(Ok(object_macros(&String::from_utf8_lossy(&output.stdout))))
}

/// Compiles `source` with the platform's C compiler (`$CC`, or `cc`),
/// warnings as errors, syntax only; returns the compiler's first error line
/// on failure.
///
/// # Errors
///
/// Returns an error if the file cannot be written or the compiler cannot
/// be started.
pub(crate) fn compile(source: &str, file: &Path) -> Result<Result<(), String>, Error> {
    let output = compile_output(source, file, false)?;
    if output.status.success() {
        return Ok(Ok(()));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(Err(stderr
        .lines()
        .find(|line| line.contains("error"))
        .unwrap_or("the C compiler failed with no error line")
        .to_owned()))
}

/// Run the syntax check, fixing diagnostic formatting when the result is
/// to be parsed by the qualified-name probe.
fn compile_output(source: &str, file: &Path, pin_diagnostics: bool) -> Result<Output, Error> {
    crate::fixture::write_file(file, source)?;
    let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    let mut command = Command::new(compiler);
    command.args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"]);
    if pin_diagnostics {
        command.arg("-fdiagnostics-color=never").env("LC_ALL", "C");
    }
    command
        .arg(file)
        .output()
        .map_err(Error::io("run the C compiler"))
}

/// The host's target triple, from `rustc -vV`.
fn host_target() -> Result<String, Error> {
    let output = Command::new("rustc")
        .arg("-vV")
        .output()
        .map_err(Error::io("run rustc -vV"))?;
    Error::check_status("rustc -vV", output.status)?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
        .ok_or(Error::UnexpectedOutput("rustc -vV"))
}

/// The ABI string a declaration for `model`'s system is written with.
fn abi_of(model: DataModel) -> &'static str {
    match model {
        DataModel::Lp64 => "C",
        DataModel::Llp64 => "system",
    }
}

/// One file's declarations with every type name rewritten to its identity
/// in that file's own scope, as this check reads it: a name the file
/// imports from `core::ffi` becomes `core::ffi::<name>`, a primitive's
/// name stays itself, and any other name must be one `row` binds, by this
/// file's own declarations and imports, to a struct or type alias of the
/// row's declaration files. So an import in one file never vouches for a
/// name in another, and a name imported from outside the row is refused.
/// Item-level cfg, a local item named like a primitive or scalar, and
/// union or enum items are not read here (see the module comment;
/// TODO(#179)).
fn canonical(
    file: &Declarations,
    row: &dyn Fn(&str) -> Result<(), String>,
) -> Result<Declarations, String> {
    let rewrite = |found: &CType| -> Result<CType, String> {
        fn walk(
            found: &CType,
            file: &Declarations,
            row: &dyn Fn(&str) -> Result<(), String>,
        ) -> Result<CType, String> {
            match found {
                CType::Pointer { constant, to } => Ok(CType::Pointer {
                    constant: *constant,
                    to: Box::new(walk(to, file, row)?),
                }),
                CType::Array { of, count } => Ok(CType::Array {
                    of: Box::new(walk(of, file, row)?),
                    count: *count,
                }),
                CType::Named(name) => {
                    if name.starts_with("core::ffi::") || PRIMITIVES.contains(&name.as_str()) {
                        Ok(CType::Named(name.clone()))
                    } else if file.ffi_names.iter().any(|imported| imported == name) {
                        Ok(CType::Named(format!("core::ffi::{name}")))
                    } else {
                        row(name).map(|()| CType::Named(name.clone()))
                    }
                }
            }
        }
        walk(found, file, row)
    };
    let mut out = Declarations::default();
    for item in &file.structs {
        let fields = item
            .fields
            .iter()
            .map(|(name, field)| Ok((name.clone(), rewrite(field)?)))
            .collect::<Result<Vec<_>, String>>()?;
        out.structs.push(Struct {
            name: item.name.clone(),
            fields,
        });
    }
    for function in &file.functions {
        out.functions.push(Function {
            name: function.name.clone(),
            parameters: function
                .parameters
                .iter()
                .map(&rewrite)
                .collect::<Result<Vec<_>, String>>()?,
            variadic: function.variadic,
            returns: function.returns.as_ref().map(&rewrite).transpose()?,
        });
    }
    for (name, aliased) in &file.aliases {
        out.aliases.push((name.clone(), rewrite(aliased)?));
    }
    out.constants.clone_from(&file.constants);
    Ok(out)
}

/// The C facts sites.json records for a row, by kind.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Recorded<'a> {
    pub(crate) structs: &'a [String],
    pub(crate) constants: &'a [String],
    pub(crate) aliases: &'a [String],
}

impl<'a> From<&'a os::inventory::Code> for Recorded<'a> {
    fn from(code: &'a os::inventory::Code) -> Self {
        Self {
            structs: &code.structs,
            constants: &code.constants,
            aliases: &code.aliases,
        }
    }
}

/// How the recorded facts differ from those `found` declares: the
/// recorded structs, constants and type aliases it does not declare, then
/// those it declares that are not recorded, each in that order.
pub(crate) fn fact_differences(
    recorded: Recorded<'_>,
    found: &Declarations,
) -> (Vec<String>, Vec<String>) {
    let declared: [Vec<&str>; 3] = [
        found
            .structs
            .iter()
            .map(|item| item.name.as_str())
            .collect(),
        found
            .constants
            .iter()
            .map(|(name, _)| name.as_str())
            .collect(),
        found
            .aliases
            .iter()
            .map(|(name, _)| name.as_str())
            .collect(),
    ];
    let recorded = [recorded.structs, recorded.constants, recorded.aliases];
    let missing = recorded
        .iter()
        .zip(&declared)
        .flat_map(|(recorded, declared)| {
            recorded
                .iter()
                .filter(|name| !declared.contains(&name.as_str()))
                .cloned()
        })
        .collect();
    let unrecorded = declared
        .iter()
        .zip(&recorded)
        .flat_map(|(declared, recorded)| {
            declared
                .iter()
                .filter(|name| !recorded.iter().any(|recorded| recorded == *name))
                .map(|name| (*name).to_owned())
        })
        .collect();
    (missing, unrecorded)
}

/// C preprocessor checks that the compiler compiling the generated file is
/// the target's: its architecture, and its C library (`__GLIBC__` for
/// glibc; for musl, no `__GLIBC__` and musl's own `__DEFINED_ssize_t`,
/// which its `<sys/types.h>` defines). Empty for a target this check
/// cannot identify that way.
pub(crate) fn identity(target: &str) -> String {
    let arch = if target.starts_with("x86_64-") {
        "__x86_64__"
    } else if target.starts_with("aarch64-") {
        "__aarch64__"
    } else {
        return String::new();
    };
    let mut checks = format!(
        "#if !defined({arch})\n#error \"the C compiler is not {target}'s: wrong architecture\"\n#endif\n"
    );
    if target.ends_with("-linux-gnu") {
        let _ = write!(
            checks,
            "#if !defined(__GLIBC__)\n#error \"the C compiler is not {target}'s: not glibc's headers\"\n#endif\n"
        );
    } else if target.ends_with("-linux-musl") {
        let _ = write!(
            checks,
            "#if defined(__GLIBC__) || !defined(__DEFINED_ssize_t)\n#error \"the C compiler is not {target}'s: not musl's headers\"\n#endif\n"
        );
    }
    checks
}

/// The declarations of a row's module on one target: every file under
/// `src/<module>/` that the compiler read.
fn row_declarations(
    crate_dir: &Path,
    module: &str,
    read: &std::collections::BTreeSet<PathBuf>,
    target: &str,
) -> Result<Result<Declarations, String>, Error> {
    let abi = abi_of(DataModel::of(target));
    let directory = Path::new("src").join(module);
    let mut files = Vec::new();
    for file in read.iter().filter(|file| file.starts_with(&directory)) {
        let path = crate_dir.join(file);
        let text = fs::read_to_string(&path).map_err(Error::path_io("read source file", &path))?;
        let tokens = match rust_source::tokens_with_strings(&text) {
            Ok(tokens) => tokens,
            Err(error) => return Ok(Err(format!("{}: {error}", file.display()))),
        };
        let mut one = Declarations::default();
        if let Err(error) = declarations(&tokens, abi, &mut one) {
            return Ok(Err(format!("{}: {error}", file.display())));
        }
        files.push((file.clone(), one));
    }
    let mut found = Declarations::default();
    for (index, (file, one)) in files.iter().enumerate() {
        let row = |name: &str| bound_in_row(&files, target, index, name, 0);
        match canonical(one, &row) {
            Ok(one) => {
                found.structs.extend(one.structs);
                found.aliases.extend(one.aliases);
                found.constants.extend(one.constants);
                found.functions.extend(one.functions);
            }
            Err(error) => return Ok(Err(format!("{}: {error}", file.display()))),
        }
    }
    Ok(Ok(found))
}

/// How many re-exports a name is followed through before the chain is
/// refused.
const MAX_REEXPORTS: usize = 8;

/// Whether the file `files[at]` binds `name` to a struct or type alias of
/// the row's declaration files (`files`): it declares it, or exactly one of
/// its imports in effect on `target` names it in a declaration file of the
/// row that binds it so in turn. Which imports are in effect is decided
/// from the target and each import's gate (see [`Gate`]), never from which
/// files were compiled. An import from outside the row, one in effect whose
/// file the compiler did not read, a name neither declared nor imported,
/// and a name two imports in effect bind are refused: the type Rust uses
/// there may not be the row's.
pub(crate) fn bound_in_row(
    files: &[(PathBuf, Declarations)],
    target: &str,
    at: usize,
    name: &str,
    depth: usize,
) -> Result<(), String> {
    let Some((file, one)) = files.get(at) else {
        return Err(format!("`{name}`: no such declaration file"));
    };
    if one.structs.iter().any(|item| item.name == name)
        || one.aliases.iter().any(|(alias, _)| alias == name)
    {
        return Ok(());
    }
    if depth > MAX_REEXPORTS {
        return Err(format!(
            "`{name}`: a chain of re-exports this check does not follow"
        ));
    }
    let module = module_of(file);
    let mut bound = Vec::new();
    for import in one.imports.iter().filter(|import| {
        import.gate.in_effect(target) && import.path.last().is_some_and(|last| last == name)
    }) {
        let shown = import.path.join("::");
        let outside = || {
            Err(format!(
                "`{name}` is imported from {shown}, outside the row's declaration files, so \
                 the type Rust uses may not be the one this check proves"
            ))
        };
        let Some(destination) = import
            .path
            .split_last()
            .and_then(|(_, modules)| import_target(module.as_deref(), modules))
        else {
            bound.push(outside());
            continue;
        };
        let destination_file = Path::new("src").join(format!("{}.rs", destination.join("/")));
        match files.iter().position(|(file, _)| *file == destination_file) {
            Some(index) => bound.push(bound_in_row(files, target, index, name, depth + 1)),
            None => bound.push(outside()),
        }
    }
    match bound.len() {
        0 => Err(format!(
            "`{name}` is neither imported from core::ffi in its own file, a primitive integer, \
             nor a struct or type alias its own file declares or imports, in effect, from the \
             row's declaration files"
        )),
        1 => bound
            .pop()
            .unwrap_or_else(|| Err(format!("`{name}`: unbound"))),
        _ => Err(format!(
            "`{name}` is bound by more than one import in effect, so which type Rust uses \
             is not one this check can name"
        )),
    }
}

/// The module path of a source file of the crate, `src/a/b.rs` giving
/// `a::b`, as segments; `None` for the crate root or a path not of that
/// form.
fn module_of(file: &Path) -> Option<Vec<String>> {
    let relative = file.strip_prefix("src").ok()?;
    let stem = relative.with_extension("");
    let segments: Vec<String> = stem
        .components()
        .map(|part| part.as_os_str().to_str().map(str::to_owned))
        .collect::<Option<_>>()?;
    (segments.first().is_some_and(|first| first != "lib")).then_some(segments)
}

/// The module an import's module path (`modules`, without the name)
/// reaches from `module`: `super` steps up, `self` stays, `crate` starts
/// at the root and any other leading name is a child module. `None` for a
/// path into another crate (`core`, `std` or any other), or that leaves
/// the crate.
fn import_target(module: Option<&[String]>, modules: &[String]) -> Option<Vec<String>> {
    let mut at: Vec<String> = module?.to_vec();
    let mut segments = modules.iter();
    match segments.as_slice().first().map(String::as_str) {
        Some("crate") => {
            at.clear();
            segments.next();
        }
        Some("core" | "std" | "alloc") | None => return None,
        _ => {}
    }
    for segment in segments {
        match segment.as_str() {
            "super" => {
                at.pop()?;
            }
            "self" => {}
            other => at.push(other.to_owned()),
        }
    }
    (!at.is_empty()).then_some(at)
}

/// Every identifier in the files the compiler read for the row of `module`
/// (`src/<module>.rs` and everything under `src/<module>/`).
fn row_identifiers(
    crate_dir: &Path,
    module: &str,
    read: &std::collections::BTreeSet<PathBuf>,
) -> Result<Result<std::collections::BTreeSet<String>, String>, Error> {
    let root = Path::new("src").join(format!("{module}.rs"));
    let directory = Path::new("src").join(module);
    let mut used = std::collections::BTreeSet::new();
    for file in read
        .iter()
        .filter(|file| **file == root || file.starts_with(&directory))
    {
        let path = crate_dir.join(file);
        let text = fs::read_to_string(&path).map_err(Error::path_io("read source file", &path))?;
        match rust_source::tokens(&text) {
            Ok(tokens) => used.extend(
                tokens
                    .iter()
                    .filter_map(|token| ident(Some(token)))
                    .map(str::to_owned),
            ),
            Err(error) => return Ok(Err(format!("{}: {error}", file.display()))),
        }
    }
    Ok(Ok(used))
}

/// The rows implemented on `target` that hold a foreign item: what this
/// check proves there. Empty on a target with none, where it has nothing
/// to prove; `cargo xtask check` then skips it, and a CI lane that runs it
/// fails.
///
/// # Errors
///
/// Returns an error if the copy cannot be read, or its problems.
pub(crate) fn rows_to_prove(workspace: &Workspace, target: &str) -> Result<Vec<String>, Error> {
    let crate_dir = os::crate_dir(workspace)?;
    let inventory = match os::read_inventory(&workspace.root, &crate_dir)? {
        Ok(inventory) => inventory,
        Err(problems) => {
            return Err(Error::Rules {
                check: CHECK,
                violations: problems.iter().map(ToString::to_string).collect(),
            });
        }
    };
    Ok(inventory
        .rows
        .iter()
        .filter(|row| {
            row.implemented_on(target)
                && row
                    .code
                    .as_ref()
                    .is_some_and(|code| !code.extern_blocks.is_empty())
        })
        .map(|row| row.id.clone())
        .collect())
}

/// Whether this host has anything for `check-abi` to prove.
///
/// # Errors
///
/// As `rows_to_prove`, and if the host's triple cannot be read.
pub(crate) fn applies_here() -> Result<(bool, String), Error> {
    let workspace = metadata::load(None, &[])?;
    let host = host_target()?;
    let rows = rows_to_prove(&workspace, &host)?;
    Ok((!rows.is_empty(), host))
}

/// Checks `target` (the host's by default), or with `self_test` proves on
/// a host-portable fixture that each planted mismatch is caught.
///
/// # Errors
///
/// Returns the rows whose check failed, a target with nothing to prove, or
/// why the check could not run.
pub(super) fn run(
    self_test: bool,
    target: Option<&str>,
    mode: Mode,
    output: &mut impl Write,
) -> Result<(), Error> {
    if mode == Mode::LegacyGlibc228 && (self_test || !target.is_some_and(legacy_target)) {
        return Err(Error::Rules {
            check: CHECK,
            violations: vec!["--legacy-glibc-2-28 requires an explicit GNU target".to_owned()],
        });
    }
    let workspace = metadata::load(None, &[])?;
    let dir = crate::fixture::fresh_dir(&workspace.target_dir, CHECK)?;
    let result = if self_test {
        self_test::run(&dir, output)
    } else {
        let target = match target {
            Some(target) => target.to_owned(),
            None => host_target()?,
        };
        check_target(&workspace, &target, mode, &dir, output)
    };
    crate::fixture::remove_dir(&dir)?;
    result
}

fn check_target(
    workspace: &Workspace,
    target: &str,
    mode: Mode,
    dir: &Path,
    output: &mut impl Write,
) -> Result<(), Error> {
    let crate_dir = os::crate_dir(workspace)?;
    let fail = |violations: Vec<String>| Error::Rules {
        check: CHECK,
        violations,
    };
    if !os::ALLOWLIST.contains(&target) {
        return Err(fail(vec![format!(
            "{target} is not a target of ADR 0005, point 5.3: run this check on one that is"
        )]));
    }
    let to_prove = rows_to_prove(workspace, target)?;
    if to_prove.is_empty() {
        return Err(fail(vec![format!(
            "no row with a foreign item is implemented on {target}, so this run proves nothing"
        )]));
    }
    let inventory = match os::read_inventory(&workspace.root, &crate_dir)? {
        Ok(inventory) => inventory,
        Err(problems) => return Err(fail(problems.iter().map(ToString::to_string).collect())),
    };
    let build_dir = workspace.target_dir.join(BUILD_DIR);
    let read = match os::files_for(None, workspace, &crate_dir, &build_dir, target)? {
        Built::Files(files) => files,
        Built::Refused(error) => return Err(fail(vec![format!("{target}: {error}")])),
    };
    let mut checked = Vec::new();
    let mut incomplete = Vec::new();
    let mut violations = Vec::new();
    let musl = target.ends_with("-linux-musl");
    // `core::ffi` names C's types only where no file of the crate shadows
    // `core`, a glob import from another file included.
    for file in &read {
        let path = crate_dir.join(file);
        let text = fs::read_to_string(&path).map_err(Error::path_io("read source file", &path))?;
        match rust_source::tokens(&text) {
            Ok(tokens) => {
                if let Some(line) = rust_source::core_shadows(&tokens).first() {
                    violations.push(format!(
                        "{}:{line}: an item, import or generic parameter named `core`, under \
                         which `core::ffi` would not be the core crate's",
                        file.display()
                    ));
                }
            }
            Err(error) => violations.push(format!("{}: {error}", file.display())),
        }
    }
    for row in &inventory.rows {
        let proof = if musl && withheld_on_musl(row) {
            prove_withheld(row, target, dir)?
        } else if to_prove.contains(&row.id) {
            prove_row(row, &crate_dir, &read, target, mode, dir)?
        } else {
            continue;
        };
        match proof {
            Proof::Proved(proved) => checked.push(proved),
            Proof::Partial(why) => incomplete.push(format!("partial {why}")),
            Proof::Withheld(why) => incomplete.push(format!("withheld {why}")),
            Proof::Failed(problem) => violations.push(problem),
        }
    }
    if (musl || mode == Mode::LegacyGlibc228) && incomplete.len() != 1 {
        violations.push(format!(
            "{target}: expected one {} result for {}: MISSING {}",
            if musl { "withheld" } else { "partial" },
            qualified::FDPASS_ROW,
            qualified::PIDFD
        ));
    }
    if !violations.is_empty() {
        return Err(fail(violations));
    }
    let mut report =
        |line: String| writeln!(output, "{line}").map_err(Error::io("report check-abi"));
    for evidence in incomplete {
        report(format!("check-abi: {target}: {evidence}"))?;
    }
    report(format!(
        "check-abi: {target}: the system's C compiler accepted {}",
        checked.join("; ")
    ))
}

/// What the check found for one row.
#[derive(Debug, PartialEq, Eq)]
enum Proof {
    /// What the C compiler accepted.
    Proved(String),
    /// The legacy lane's omitted assertion, after all others passed.
    Partial(String),
    /// The approved musl row whose qualified constant is absent.
    Withheld(String),
    /// Why the row failed.
    Failed(String),
}

/// Proves an implemented row from its compiled declaration files.
fn prove_row(
    row: &Row,
    crate_dir: &Path,
    read: &std::collections::BTreeSet<PathBuf>,
    target: &str,
    mode: Mode,
    dir: &Path,
) -> Result<Proof, Error> {
    let failed = |problem: String| Ok(Proof::Failed(format!("{}: {problem}", row.id)));
    let Some(code) = row.code.as_ref() else {
        return failed("no declarations recorded for the implemented row".to_owned());
    };
    let model = DataModel::of(target);
    let mut found = match row_declarations(crate_dir, &code.module, read, target)? {
        Ok(found) => found,
        Err(error) => return failed(error),
    };
    found.opaque = OPAQUE_TAILS
        .iter()
        .filter(|tail| tail.row == row.id)
        .copied()
        .collect();
    if let Some(problem) = fact_problem(&row.id, code, &found, crate_dir, read, model, dir)? {
        return failed(problem);
    }
    let declared: Vec<&str> = code
        .extern_blocks
        .iter()
        .flat_map(|block| &block.items)
        .map(|item| item.name.as_str())
        .collect();
    let read_names: Vec<&str> = found
        .functions
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    if declared.iter().any(|name| !read_names.contains(name)) {
        return failed(format!(
            "the declaration files under src/{}/ do not declare every function sites.json \
             records ({}); found {}",
            code.module,
            declared.join(", "),
            read_names.join(", ")
        ));
    }
    prove_declarations(row, code, found, target, mode, dir)
}

/// Proves the validated declaration list. Only the legacy socket row may
/// remove an assertion, and only after its name has been proved absent.
fn prove_declarations(
    row: &Row,
    code: &Code,
    mut found: Declarations,
    target: &str,
    mode: Mode,
    dir: &Path,
) -> Result<Proof, Error> {
    let failed = |problem: String| Ok(Proof::Failed(format!("{}: {problem}", row.id)));
    if let Err(error) = qualified::named_assertions(code, &found) {
        return failed(error);
    }
    let base = match qualified::baseline(row, code, target, mode) {
        Ok(base) => base,
        Err(error) => return failed(error),
    };
    let absent = match qualified::unnamed(&row.id, &base, &code.qualified, dir)? {
        Ok(absent) => absent,
        Err(error) => return failed(error),
    };
    let legacy_socket = mode == Mode::LegacyGlibc228 && row.id == qualified::FDPASS_ROW;
    if legacy_socket {
        if absent != [qualified::PIDFD] {
            return failed(format!(
                "the headers name {}; revisit the glibc 2.28 exception",
                qualified::PIDFD
            ));
        }
        found.constants.retain(|(name, _)| name != qualified::PIDFD);
        if found.constants.is_empty()
            && found.structs.is_empty()
            && found.functions.is_empty()
            && found.aliases.is_empty()
        {
            return failed("no independent ABI assertion remains".to_owned());
        }
    } else if !absent.is_empty() {
        return failed(format!("MISSING {}", absent.join(", ")));
    }
    let source = match c_source(
        &row.id,
        &code.defines,
        &code.headers,
        &code.expressions,
        &found,
        DataModel::of(target),
    ) {
        Ok(source) => format!("{source}{}", identity(target)),
        Err(error) => return failed(error),
    };
    if let Err(error) = compile(&source, &dir.join(format!("{}.c", row.id)))? {
        return failed(error);
    }
    if legacy_socket {
        Ok(Proof::Partial(format!(
            "{}: MISSING {}",
            row.id,
            qualified::PIDFD
        )))
    } else {
        Ok(Proof::Proved(format!(
            "{} ({} structs, {} constants, {} functions)",
            row.id,
            found.structs.len(),
            found.constants.len(),
            found.functions.len()
        )))
    }
}

/// Probes the approved musl row's headers without reading Rust
/// declarations from a module that the target does not compile.
fn prove_withheld(row: &Row, target: &str, dir: &Path) -> Result<Proof, Error> {
    let failed = |problem: String| Ok(Proof::Failed(format!("{}: {problem}", row.id)));
    let Some(code) = row.code.as_ref() else {
        return failed("no C headers recorded for the withheld row".to_owned());
    };
    let base = match qualified::socket_row(row, code)
        .and_then(|()| qualified::baseline(row, code, target, Mode::Complete))
    {
        Ok(base) => base,
        Err(error) => return failed(error),
    };
    match qualified::unnamed(&row.id, &base, &code.qualified, dir)? {
        Ok(absent) if absent == [qualified::PIDFD] => Ok(Proof::Withheld(format!(
            "{}: MISSING {}",
            row.id,
            qualified::PIDFD
        ))),
        Ok(_) => failed(format!(
            "the headers name {}; revisit withholding",
            qualified::PIDFD
        )),
        Err(error) => failed(error),
    }
}

/// Why the row's C facts are not the ones its compiled code needs, if they
/// are not: a C macro of its headers that its code names but no
/// declaration file declares, or sites.json's facts differing from the
/// declaration files.
fn fact_problem(
    row: &str,
    code: &os::inventory::Code,
    found: &Declarations,
    crate_dir: &Path,
    read: &std::collections::BTreeSet<PathBuf>,
    model: DataModel,
    dir: &Path,
) -> Result<Option<String>, Error> {
    // The facts the row must assert come from its compiled code: every C
    // macro of its headers that a file the compiler read for the row names
    // (`src/<module>.rs` and everything under `src/<module>/`).
    let macros = match header_macros(
        row,
        &code.defines,
        &code.headers,
        model,
        &dir.join(format!("{row}-macros.c")),
    )? {
        Ok(macros) => macros,
        Err(error) => return Ok(Some(error)),
    };
    let used = match row_identifiers(crate_dir, &code.module, read)? {
        Ok(used) => used,
        Err(error) => return Ok(Some(error)),
    };
    let declared: Vec<&str> = found
        .constants
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    let undeclared: Vec<String> = required_constants(used.iter().map(String::as_str), &macros)
        .into_iter()
        .filter(|name| !declared.contains(&name.as_str()))
        .collect();
    if !undeclared.is_empty() {
        return Ok(Some(format!(
            "the row's code names {}, a C macro of its headers, which no declaration file the \
             compiler read under src/{}/ declares, so no assertion would prove it",
            undeclared.join(", "),
            code.module
        )));
    }
    // The copy records what the compiled declarations state, no more and
    // no less: it is checked against them, never the source of them.
    let (missing, unrecorded) = fact_differences(Recorded::from(code), found);
    if missing.is_empty() && unrecorded.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!(
            "sites.json's C facts differ from the declaration files the compiler read under \
             src/{}/: recorded but not declared [{}], declared but not recorded [{}]",
            code.module,
            missing.join(", "),
            unrecorded.join(", ")
        )))
    }
}

/// The names of the object-like macros a preprocessor's `-dM` `dump`
/// defines (`#define NAME value`, not `#define NAME(args) ...`).
pub(crate) fn object_macros(dump: &str) -> std::collections::BTreeSet<String> {
    dump.lines()
        .filter_map(|line| line.strip_prefix("#define "))
        .filter_map(|rest| {
            let name: String = rest
                .chars()
                .take_while(|next| next.is_ascii_alphanumeric() || *next == '_')
                .collect();
            let after = rest.get(name.len()..).unwrap_or_default();
            (!name.is_empty() && !after.starts_with('(')).then_some(name)
        })
        .collect()
}

/// The C facts a row must declare and assert, from its compiled code: each
/// identifier `used` in the files the compiler read for the row that is an
/// object-like macro of the row's C headers (`macros`) and has an
/// upper-case letter (so a Rust `cfg` word such as `unix`, or a variable
/// such as `errno`, which C also defines, is never taken for one). Sorted,
/// each once.
pub(crate) fn required_constants<'a>(
    used: impl Iterator<Item = &'a str>,
    macros: &std::collections::BTreeSet<String>,
) -> Vec<String> {
    used.filter(|name| name.bytes().any(|byte| byte.is_ascii_uppercase()))
        .filter(|name| macros.contains(*name))
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<String>>()
        .into_iter()
        .collect()
}
