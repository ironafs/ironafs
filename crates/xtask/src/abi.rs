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
//!   field of the wrong type with the right size and offset fails; an
//!   array member (`[T; N]`) by its address, `T (*)[N]`, so its element
//!   type and count are both checked. A struct's last field may be an
//!   opaque array (its name starts `_opaque`): space a C library fills
//!   with fields that are never read and whose names differ between
//!   libraries or releases (`struct statvfs`'s spares), bound only by the
//!   struct's size and alignment;
//! - each constant's value;
//! - each type alias's size, alignment and sign: an alias names a C
//!   typedef whose spelling differs between C libraries at one size
//!   (`rlim_t`), and a member of that type is checked against the typedef;
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
//! its sites do not ship there. `--self-test` compiles a host-portable
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
/// (`struct statvfs`'s spare fields). Only the struct's size and alignment
/// bind it, so the named fields before it keep their offsets and types.
const OPAQUE: &str = "_opaque";

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

/// The C type of a pointer to a struct member: `T (*)[N]` for an array,
/// so `_Generic` on the member's address checks the element type and the
/// count; for any other member its own type, compared without `&`.
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
        "_Generic(((struct {item} *)0)->{field}, {spelled}: 1)"
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

/// The C11 file of assertions for one row.
///
/// # Errors
///
/// Returns why a declaration has no C equivalent this check can state.
pub(crate) fn c_source(
    row: &str,
    defines: &[String],
    headers: &[String],
    declarations: &Declarations,
    model: DataModel,
) -> Result<String, String> {
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
            None => {}
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
        let last = item.fields.len().saturating_sub(1);
        for (index, ((field, field_type), offset)) in item.fields.iter().zip(offsets).enumerate() {
            if field.starts_with(OPAQUE) {
                if index != last || !matches!(field_type, CType::Array { .. }) {
                    return Err(format!(
                        "struct {name}: an opaque field (`{field}`) is the last field, an array"
                    ));
                }
                continue;
            }
            assert(
                format!("offsetof(struct {name}, {field}) == {offset}"),
                format!("offsetof(struct {name}, {field})"),
            );
            assert(
                member_check(name, field, field_type, declarations, model)?,
                format!("type of {name}.{field}"),
            );
        }
    }
    for (name, value) in &declarations.constants {
        assert(format!("{name} == {value}"), name.clone());
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

/// The declarations of a row's module on one target: every file under
/// `src/<module>/` that the compiler read.
fn row_declarations(
    crate_dir: &Path,
    module: &str,
    read: &std::collections::BTreeSet<PathBuf>,
    abi: &str,
) -> Result<Result<Declarations, String>, Error> {
    let directory = Path::new("src").join(module);
    let mut found = Declarations::default();
    for file in read.iter().filter(|file| file.starts_with(&directory)) {
        let path = crate_dir.join(file);
        let text = fs::read_to_string(&path).map_err(Error::path_io("read source file", &path))?;
        let tokens = match rust_source::tokens_with_strings(&text) {
            Ok(tokens) => tokens,
            Err(error) => return Ok(Err(format!("{}: {error}", file.display()))),
        };
        if let Err(error) = declarations(&tokens, abi, &mut found) {
            return Ok(Err(format!("{}: {error}", file.display())));
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
    let model = DataModel::of(target);
    let mut checked = Vec::new();
    let mut violations = Vec::new();
    for row in inventory
        .rows
        .iter()
        .filter(|row| to_prove.contains(&row.id))
    {
        let Some(code) = row.code.as_ref() else {
            continue;
        };
        let found = match row_declarations(&crate_dir, &code.module, &read, abi_of(model))? {
            Ok(found) => found,
            Err(error) => {
                violations.push(format!("{}: {error}", row.id));
                continue;
            }
        };
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
            violations.push(format!(
                "{}: the declaration files under src/{}/ do not declare every function \
                 sites.json records ({}); found {}",
                row.id,
                code.module,
                declared.join(", "),
                read_names.join(", ")
            ));
            continue;
        }
        let source = match c_source(&row.id, &code.defines, &code.headers, &found, model) {
            Ok(source) => source,
            Err(error) => {
                violations.push(format!("{}: {error}", row.id));
                continue;
            }
        };
        match compile(&source, &dir.join(format!("{}.c", row.id)))? {
            Ok(()) => checked.push(format!(
                "{} ({} structs, {} constants, {} functions)",
                row.id,
                found.structs.len(),
                found.constants.len(),
                found.functions.len()
            )),
            Err(error) => violations.push(format!("{}: {error}", row.id)),
        }
    }
    if !violations.is_empty() {
        return Err(fail(violations));
    }
    writeln!(
        output,
        "check-abi: {target}: the system's C compiler accepted {}",
        checked.join("; ")
    )
    .map_err(Error::io("report check-abi"))
}
