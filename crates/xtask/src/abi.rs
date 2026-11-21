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
//! constants and its extern blocks' functions, each named as C names it),
//! generates a C11 file of compile-time assertions from them, and compiles
//! it with the platform's own compiler and headers, warnings as errors:
//!
//! - the size and alignment of each struct, and each field's offset, as
//!   `repr(C)` lays them out for the target's data model;
//! - each field's type, by a `_Generic` selection with no default, so a
//!   field of the wrong type with the right size and offset fails;
//! - each constant's value;
//! - each function's type, by a `_Generic` selection with no default on
//!   its address against a pointer to the declared prototype.
//!
//! The C file is never linked into anything. The check proves the host
//! target only: a target on which it has not run is MISSING evidence, and
//! its sites do not ship there. `--self-test` compiles a host-portable
//! fixture of the same shape and plants a member type changed at the same
//! size and offset, a constant changed by one, a parameter's type changed
//! and a return type changed, each of which must fail to compile.

mod self_test;

use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::metadata::{self, Workspace};
use crate::os::{self, Built};
use crate::rust_source::{self, Token, TokenKind, ident, punct};
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

/// Reads the declarations among `tokens` into `found`: every
/// `#[repr(C)]` struct, every `const`, and every function of an `unsafe
/// extern` block.
///
/// # Errors
///
/// Returns what the reader cannot take: a struct without `#[repr(C)]`, a
/// constant that is not an integer literal, or a declaration it cannot
/// read exactly.
pub(crate) fn declarations(tokens: &[Token], found: &mut Declarations) -> Result<(), String> {
    let mut at = 0;
    while let Some(token) = tokens.get(at) {
        let line = token.line;
        match ident(Some(token)) {
            Some("struct") if !token.raw => {
                if !repr_c_before(tokens, at) {
                    return Err(format!(
                        "line {line}: a struct in a declaration file is #[repr(C)]"
                    ));
                }
                let (item, next) = read_struct(tokens, at)
                    .ok_or_else(|| format!("line {line}: a struct that cannot be read"))?;
                found.structs.push(item);
                at = next;
            }
            Some("const") if !token.raw => {
                let (constant, next) = read_constant(tokens, at).ok_or_else(|| {
                    format!("line {line}: a constant that is not `NAME: TYPE = <integer>;`")
                })?;
                found.constants.push(constant);
                at = next;
            }
            // The reader drops the ABI string, so `extern "C" {` reads as
            // `extern {`.
            Some("extern") if !token.raw && punct(tokens.get(at + 1)) == Some('{') => {
                let close = matching(tokens, at + 1)
                    .ok_or_else(|| format!("line {line}: an unclosed extern block"))?;
                read_functions(tokens.get(at + 2..close).unwrap_or_default(), found)
                    .ok_or_else(|| format!("line {line}: an extern block that cannot be read"))?;
                at = close + 1;
            }
            _ => at += 1,
        }
    }
    Ok(())
}

/// Whether the attributes before the `struct` at `at` include
/// `#[repr(C)]`.
fn repr_c_before(tokens: &[Token], at: usize) -> bool {
    let start = tokens
        .get(..at)
        .unwrap_or_default()
        .iter()
        .rposition(|token| matches!(punct(Some(token)), Some(';' | '}')))
        .map_or(0, |found| found + 1);
    let before = tokens.get(start..at).unwrap_or_default();
    before.windows(4).any(|window| {
        ident(window.first()) == Some("repr")
            && punct(window.get(1)) == Some('(')
            && ident(window.get(2)) == Some("C")
            && punct(window.get(3)) == Some(')')
    })
}

/// The index of the `}` that closes the `{` at `open`.
fn matching(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0_usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match punct(Some(token)) {
            Some('{') => depth += 1,
            Some('}') => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
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
    let mut name = ident(tokens.get(next))?;
    next += 1;
    while matches!(
        tokens.get(next).map(|token| &token.kind),
        Some(TokenKind::PathSep)
    ) {
        name = ident(tokens.get(next + 1))?;
        next += 2;
    }
    Some((CType::Named(name.to_owned()), next))
}

fn read_struct(tokens: &[Token], at: usize) -> Option<(Struct, usize)> {
    let name = ident(tokens.get(at + 1))?.to_owned();
    let open = at + 2;
    if punct(tokens.get(open)) != Some('{') {
        return None;
    }
    let close = matching(tokens, open)?;
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
        CType::Named(name) if name == "c_void" => Ok("void".to_owned()),
        CType::Named(name) => scalar(name, model)
            .map(|(spelling, _)| spelling.to_owned())
            .or_else(|| {
                declarations
                    .structs
                    .iter()
                    .any(|item| item.name == *name)
                    .then(|| format!("struct {name}"))
            })
            .ok_or_else(|| format!("`{name}` has no C type this check knows")),
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

/// The size and alignment of a type under `repr(C)`.
fn layout(
    found: &CType,
    declarations: &Declarations,
    model: DataModel,
    depth: usize,
) -> Result<(usize, usize), String> {
    match found {
        CType::Pointer { .. } => Ok((8, 8)),
        CType::Named(name) => {
            if let Some((_, size)) = scalar(name, model) {
                return Ok((size, size));
            }
            let item = declarations
                .structs
                .iter()
                .find(|item| item.name == *name)
                .ok_or_else(|| format!("`{name}` has no C type this check knows"))?;
            if depth > 8 {
                return Err(format!("struct {name} nests too deep"));
            }
            struct_layout(item, declarations, model, depth + 1)
                .map(|(size, align, _)| (size, align))
        }
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
    headers: &[String],
    declarations: &Declarations,
    model: DataModel,
) -> Result<String, String> {
    let mut source = format!(
        "/* Generated by `cargo xtask check-abi` for ironafs-os site {row}; never linked. */\n\
         #define _GNU_SOURCE 1\n#define _DARWIN_C_SOURCE 1\n\
         #include <stddef.h>\n#include <stdint.h>\n"
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

/// Compiles `source` with the platform's C compiler, warnings as errors,
/// syntax only; returns the compiler's first error line on failure.
///
/// # Errors
///
/// Returns an error if the file cannot be written or the compiler cannot
/// be started.
pub(crate) fn compile(source: &str, file: &Path) -> Result<Result<(), String>, Error> {
    crate::fixture::write_file(file, source)?;
    let output = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
        .arg(file)
        .output()
        .map_err(Error::io("run cc"))?;
    if output.status.success() {
        return Ok(Ok(()));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(Err(stderr
        .lines()
        .find(|line| line.contains("error"))
        .unwrap_or("cc failed with no error line")
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

/// The declarations of a row's module on one target: every file under
/// `src/<module>/` that the compiler read.
fn row_declarations(
    crate_dir: &Path,
    module: &str,
    read: &std::collections::BTreeSet<PathBuf>,
) -> Result<Result<Declarations, String>, Error> {
    let directory = Path::new("src").join(module);
    let mut found = Declarations::default();
    for file in read.iter().filter(|file| file.starts_with(&directory)) {
        let path = crate_dir.join(file);
        let text = fs::read_to_string(&path).map_err(Error::path_io("read source file", &path))?;
        let tokens = match rust_source::tokens(&text) {
            Ok(tokens) => tokens,
            Err(error) => return Ok(Err(format!("{}: {error}", file.display()))),
        };
        if let Err(error) = declarations(&tokens, &mut found) {
            return Ok(Err(format!("{}: {error}", file.display())));
        }
    }
    Ok(Ok(found))
}

/// Checks the host target's implemented rows, or with `self_test` proves
/// on a host-portable fixture that each planted mismatch fails to compile.
///
/// # Errors
///
/// Returns the rows whose check failed, or why the check could not run.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    let dir = crate::fixture::fresh_dir(&workspace.target_dir, CHECK)?;
    let result = if self_test {
        self_test::run(&dir, output)
    } else {
        check_host(&workspace, &dir, output)
    };
    crate::fixture::remove_dir(&dir)?;
    result
}

fn check_host(workspace: &Workspace, dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let host = host_target()?;
    let crate_dir = os::crate_dir(workspace)?;
    let fail = |violations: Vec<String>| Error::Rules {
        check: CHECK,
        violations,
    };
    if !os::ALLOWLIST.contains(&host.as_str()) {
        return Err(fail(vec![format!(
            "{host} is not a target of ADR 0005, point 5.3: run this check on one that is"
        )]));
    }
    let inventory = match os::read_inventory(&workspace.root, &crate_dir)? {
        Ok(inventory) => inventory,
        Err(problems) => return Err(fail(problems.iter().map(ToString::to_string).collect())),
    };
    let build_dir = workspace.target_dir.join("xtask-check-os");
    let read = match os::files_for(workspace, &crate_dir, &build_dir, &host)? {
        Built::Files(files) => files,
        Built::Refused(error) => return Err(fail(vec![format!("{host}: {error}")])),
    };
    let model = DataModel::of(&host);
    let mut checked = Vec::new();
    let mut violations = Vec::new();
    for row in &inventory.rows {
        let Some(code) = row.code.as_ref().filter(|_| row.implemented_on(&host)) else {
            continue;
        };
        if code.extern_blocks.is_empty() {
            continue;
        }
        let found = match row_declarations(&crate_dir, &code.module, &read)? {
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
        let source = match c_source(&row.id, &code.headers, &found, model) {
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
    let summary = if checked.is_empty() {
        format!("no row with a foreign item is implemented on {host}; nothing to prove there")
    } else {
        format!(
            "{host}: the system's C compiler accepted {}",
            checked.join("; ")
        )
    };
    writeln!(output, "check-abi: {summary}").map_err(Error::io("report check-abi"))
}
