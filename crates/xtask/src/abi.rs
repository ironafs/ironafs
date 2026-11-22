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
//! - each field's type, by a `_Generic` selection with no default, so a
//!   field of the wrong type with the right size and offset fails;
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
//! The file asks for the POSIX and default declarations
//! (`_DEFAULT_SOURCE`), not the GNU ones: under `_GNU_SOURCE` glibc
//! declares `setrlimit`'s resource as an enum, where POSIX and musl declare
//! `int`. A row whose declarations C shows only under `_GNU_SOURCE`
//! (`struct ucred`, in glibc and musl) names it in sites.json's `defines`,
//! the only feature macro the copy accepts. The C file is never linked into anything. The check proves the host
//! target only: a target on which it has not run is MISSING evidence, and
//! its sites do not ship there. A constant sites.json lists as `qualified`
//! (one a target's headers may not name, such as `SCM_PIDFD`) is proved by
//! name where the headers name it; where they do not, the row is reported
//! withheld on that target, MISSING evidence, and never proved from
//! another target's value (ADR 0005, sheet 14). `--self-test` compiles a host-portable
//! fixture of the same shape and plants a member type changed at the same
//! size and offset, a constant changed by one, a parameter's type changed,
//! a return type changed and an alias of the wrong size or sign, each of
//! which must fail to compile.

mod self_test;

use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::metadata::{self, Workspace};
use crate::os::{self, Built};
use crate::rust_source::{self, Token, TokenKind, ident, matching, punct};
use crate::{Command, Error};

#[cfg(test)]
mod tests;

const CHECK: &str = "check-abi";

/// A type as a declaration writes it: a name (its path's last segment) or
/// a raw pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CType {
    Named(String),
    Pointer { constant: bool, to: Box<CType> },
}

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
    let mut at = 0;
    while let Some(token) = tokens.get(at) {
        let line = token.line;
        match ident(Some(token)).filter(|_| !token.raw) {
            Some("struct") => {
                repr_c_only(tokens, at).map_err(|error| format!("line {line}: {error}"))?;
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
                let end = tokens
                    .get(at..)
                    .unwrap_or_default()
                    .iter()
                    .position(|token| punct(Some(token)) == Some(';'))
                    .map(|offset| at + offset)
                    .ok_or_else(|| format!("line {line}: an unterminated use"))?;
                let item = tokens.get(at + 1..end).unwrap_or_default();
                if item
                    .iter()
                    .any(|token| ident(Some(token)) == Some("as") && !token.raw)
                {
                    return Err(format!(
                        "line {line}: a renaming import, under which a name would not mean what \
                         it says"
                    ));
                }
                let from_ffi = ident(item.first()) == Some("core")
                    && ident(item.get(2)) == Some("ffi")
                    && matches!(
                        item.get(3).map(|token| &token.kind),
                        Some(TokenKind::PathSep)
                    );
                if from_ffi {
                    found.ffi_names.extend(
                        item.iter()
                            .skip(4)
                            .filter_map(|token| ident(Some(token)))
                            .map(str::to_owned),
                    );
                }
                at = end + 1;
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
    value.push_str(&digits.replace('_', ""));
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

/// The start of a row's C file: its feature macros and headers.
fn prelude(row: &str, defines: &[String], headers: &[String], model: DataModel) -> String {
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

/// The error a probe of an unnamed constant raises, which no compiler
/// writes of its own accord.
const UNNAMED: &str = "ironafs-check-abi: not named by these headers";

/// Which of `names` the row's headers, with its feature macros, do not
/// name on the target the compiler (with `identity`'s checks) is for: one
/// probe each, compiled as the row's file is. `Err` for a probe that
/// failed for any other reason.
///
/// # Errors
///
/// Returns an error if a probe cannot be written or compiled.
pub(crate) fn unnamed(
    row: &str,
    defines: &[String],
    headers: &[String],
    names: &[String],
    model: DataModel,
    identity: &str,
    dir: &Path,
) -> Result<Result<Vec<String>, String>, Error> {
    let mut absent = Vec::new();
    for name in names {
        let source = format!(
            "{}#ifndef {name}\n#error \"{UNNAMED}\"\n#endif\n{identity}",
            prelude(row, defines, headers, model)
        );
        match compile(&source, &dir.join(format!("{row}-{name}.c")))? {
            Ok(()) => {}
            Err(error) if error.contains(UNNAMED) => absent.push(name.clone()),
            Err(error) => return Ok(Err(error)),
        }
    }
    Ok(Ok(absent))
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
    let mut source = prelude(row, defines, headers, model);
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
        match unsigned(scalar_name) {
            Some(true) => assert(format!("({name})-1 > 0"), format!("{name} is unsigned")),
            Some(false) => assert(format!("({name})-1 < 0"), format!("{name} is signed")),
            // `c_char`: the generated file is compiled for the target
            // proved, where C's `char` has `c_char`'s sign. The cast is
            // also what requires an integer type.
            None => assert(
                format!("(({name})-1 > 0) == ((char)-1 > 0)"),
                format!("{name} has char's sign"),
            ),
        }
    }
    for item in &declarations.structs {
        let name = &item.name;
        let (size, align, offsets) = struct_layout(item, declarations, model, 0)?;
        assert(
            format!("sizeof(struct {name}) == {size}"),
            format!("sizeof(struct {name})"),
        );
        assert(
            format!("_Alignof(struct {name}) == {align}"),
            format!("_Alignof(struct {name})"),
        );
        for ((field, field_type), offset) in item.fields.iter().zip(offsets) {
            assert(
                format!("offsetof(struct {name}, {field}) == {offset}"),
                format!("offsetof(struct {name}, {field})"),
            );
            let spelled = c_name(field_type, declarations, model)?;
            assert(
                format!("_Generic(((struct {name} *)0)->{field}, {spelled}: 1)"),
                format!("type of {name}.{field}"),
            );
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

/// Compiles `source` with the platform's C compiler (`$CC`, or `cc`),
/// warnings as errors, syntax only; returns the compiler's first error line
/// on failure.
///
/// # Errors
///
/// Returns an error if the file cannot be written or the compiler cannot
/// be started.
pub(crate) fn compile(source: &str, file: &Path) -> Result<Result<(), String>, Error> {
    crate::fixture::write_file(file, source)?;
    let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    let output = Command::new(compiler)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
        .arg(file)
        .output()
        .map_err(Error::io("run the C compiler"))?;
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
/// in that file's own scope: a name the file imports from `core::ffi`
/// becomes `core::ffi::<name>`, a primitive stays itself, and any other
/// name must be one of the row's structs. So an import in one file never
/// vouches for a name in another.
fn canonical(file: &Declarations, structs: &[String]) -> Result<Declarations, String> {
    // `structs` holds the row's struct and type alias names: the names a
    // declaration file declares for the whole row.
    let rewrite = |found: &CType| -> Result<CType, String> {
        fn walk(found: &CType, file: &Declarations, structs: &[String]) -> Result<CType, String> {
            match found {
                CType::Pointer { constant, to } => Ok(CType::Pointer {
                    constant: *constant,
                    to: Box::new(walk(to, file, structs)?),
                }),
                CType::Named(name) => {
                    if name.starts_with("core::ffi::") || PRIMITIVES.contains(&name.as_str()) {
                        Ok(CType::Named(name.clone()))
                    } else if file.ffi_names.iter().any(|imported| imported == name) {
                        Ok(CType::Named(format!("core::ffi::{name}")))
                    } else if structs.iter().any(|item| item == name) {
                        Ok(CType::Named(name.clone()))
                    } else {
                        Err(format!(
                            "`{name}` is neither imported from core::ffi in its own file, a \
                             primitive integer, nor a struct or type alias of the declaration \
                             files"
                        ))
                    }
                }
            }
        }
        walk(found, file, structs)
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

/// The recorded structs and constants that `found` does not declare.
pub(crate) fn missing_facts(
    structs: &[String],
    constants: &[String],
    found: &Declarations,
) -> Vec<String> {
    structs
        .iter()
        .filter(|name| !found.structs.iter().any(|item| &item.name == *name))
        .chain(constants.iter().filter(|name| {
            !found
                .constants
                .iter()
                .any(|(constant, _)| constant == *name)
        }))
        .cloned()
        .collect()
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
    abi: &str,
) -> Result<Result<Declarations, String>, Error> {
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
    let structs: Vec<String> = files
        .iter()
        .flat_map(|(_, one)| {
            one.structs
                .iter()
                .map(|item| item.name.clone())
                .chain(one.aliases.iter().map(|(name, _)| name.clone()))
        })
        .collect();
    let mut found = Declarations::default();
    for (file, one) in files {
        match canonical(&one, &structs) {
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
    output: &mut impl Write,
) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    let dir = crate::fixture::fresh_dir(&workspace.target_dir, CHECK)?;
    let result = if self_test {
        self_test::run(&dir, output)
    } else {
        let target = match target {
            Some(target) => target.to_owned(),
            None => host_target()?,
        };
        check_target(&workspace, &target, &dir, output)
    };
    crate::fixture::remove_dir(&dir)?;
    result
}

fn check_target(
    workspace: &Workspace,
    target: &str,
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
    let build_dir = workspace.target_dir.join("xtask-check-os");
    let read = match os::files_for(workspace, &crate_dir, &build_dir, target)? {
        Built::Files(files) => files,
        Built::Refused(error) => return Err(fail(vec![format!("{target}: {error}")])),
    };
    let mut checked = Vec::new();
    let mut withheld = Vec::new();
    let mut violations = Vec::new();
    for row in inventory
        .rows
        .iter()
        .filter(|row| to_prove.contains(&row.id))
    {
        match prove_row(row, &crate_dir, &read, target, dir)? {
            Some(Proof::Proved(proved)) => checked.push(proved),
            Some(Proof::Withheld(why)) => withheld.push(why),
            Some(Proof::Failed(problem)) => violations.push(problem),
            None => {}
        }
    }
    if !violations.is_empty() {
        return Err(fail(violations));
    }
    let mut report =
        |line: String| writeln!(output, "{line}").map_err(Error::io("report check-abi"));
    for why in withheld {
        report(format!(
            "check-abi: {target}: withheld {why}: MISSING evidence, so the row does not ship on \
             this target"
        ))?;
    }
    report(format!(
        "check-abi: {target}: the system's C compiler accepted {}",
        checked.join("; ")
    ))
}

/// What the check found for one row.
enum Proof {
    /// What the C compiler accepted.
    Proved(String),
    /// The row and the qualified constants the target's headers do not
    /// name.
    Withheld(String),
    /// Why the row failed.
    Failed(String),
}

/// Proves one row's declarations on `target`; `None` for a row with no
/// code.
fn prove_row(
    row: &os::inventory::Row,
    crate_dir: &Path,
    read: &std::collections::BTreeSet<PathBuf>,
    target: &str,
    dir: &Path,
) -> Result<Option<Proof>, Error> {
    let Some(code) = row.code.as_ref() else {
        return Ok(None);
    };
    let model = DataModel::of(target);
    let failed = |problem: String| Ok(Some(Proof::Failed(format!("{}: {problem}", row.id))));
    let found = match row_declarations(crate_dir, &code.module, read, abi_of(model))? {
        Ok(found) => found,
        Err(error) => return failed(error),
    };
    let missing = missing_facts(&code.structs, &code.constants, &found);
    if !missing.is_empty() {
        return failed(format!(
            "the declaration files the compiler read under src/{}/ state no {}, which \
             sites.json records as the row's C facts",
            code.module,
            missing.join(", ")
        ));
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
    match unnamed(
        &row.id,
        &code.defines,
        &code.headers,
        &code.qualified,
        model,
        &identity(target),
        dir,
    )? {
        Ok(absent) if absent.is_empty() => {}
        Ok(absent) => {
            return Ok(Some(Proof::Withheld(format!(
                "{}: its headers do not name {}",
                row.id,
                absent.join(", ")
            ))));
        }
        Err(error) => return failed(error),
    }
    let source = match c_source(
        &row.id,
        &code.defines,
        &code.headers,
        &code.expressions,
        &found,
        model,
    ) {
        Ok(source) => format!("{source}{}", identity(target)),
        Err(error) => return failed(error),
    };
    Ok(Some(
        match compile(&source, &dir.join(format!("{}.c", row.id)))? {
            Ok(()) => Proof::Proved(format!(
                "{} ({} structs, {} constants, {} functions)",
                row.id,
                found.structs.len(),
                found.constants.len(),
                found.functions.len()
            )),
            Err(error) => Proof::Failed(format!("{}: {error}", row.id)),
        },
    ))
}
