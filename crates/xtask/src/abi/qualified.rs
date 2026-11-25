// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Evidence for qualified C names, including the partial and withheld
//! cases of ADR 0005, sheet 14.

use std::path::Path;

use super::{DataModel, Declarations, Mode, compile, compile_output, identity, preamble};
use crate::Error;
use crate::os::inventory::{Code, Row};

pub(super) const FDPASS_ROW: &str = "linux-fdpass";
pub(super) const PIDFD: &str = "SCM_PIDFD";

/// The exception and socket controls apply to this exact inventory row.
pub(super) fn socket_row(row: &Row, code: &Code) -> Result<(), String> {
    if row.number == "14" && row.id == FDPASS_ROW && code.qualified == [PIDFD] {
        Ok(())
    } else {
        Err(format!(
            "unrecognised qualified socket row {} ({}) with names {:?}; review ADR 0005, sheet 14",
            row.number, row.id, code.qualified
        ))
    }
}

/// Validate the original declaration list before the compiler decides
/// whether any qualified assertion can be made.
pub(super) fn named_assertions(code: &Code, found: &Declarations) -> Result<(), String> {
    for name in &code.qualified {
        if found
            .constants
            .iter()
            .filter(|(constant, _)| constant == name)
            .count()
            != 1
        {
            return Err(format!("{name} requires exactly one typed Rust constant"));
        }
        if code
            .expressions
            .iter()
            .any(|(constant, _)| constant == name)
        {
            return Err(format!("{name} must be asserted by its C name"));
        }
    }
    Ok(())
}

/// The baseline precedes every probe of the qualified socket name. Its
/// feature macro must precede even the common data-model includes.
pub(super) fn baseline(row: &Row, code: &Code, target: &str, mode: Mode) -> Result<String, String> {
    let mut source = preamble(&row.id, &code.defines, &code.headers, DataModel::of(target));
    source.push_str(&identity(target));
    if row.id == FDPASS_ROW || row.number == "14" {
        socket_row(row, code)?;
        if !code.defines.iter().any(|name| name == "_GNU_SOURCE") {
            return Err("the socket baseline requires _GNU_SOURCE before the headers".to_owned());
        }
        source.push_str(
            "int ironafs_abi_socket_stream = ((SOCK_STREAM) | 0);\n\
             int ironafs_abi_socket_type = ((SO_TYPE) | 0);\n\
             _Static_assert(sizeof(struct ucred) > 0, \"socket baseline: struct ucred\");\n",
        );
        if target.ends_with("-linux-gnu") {
            source.push_str(
                "#if !defined(__USE_GNU)\n\
                 #error \"the socket baseline requires __USE_GNU after the glibc headers\"\n\
                 #endif\n",
            );
        }
        if mode == Mode::LegacyGlibc228 {
            source.push_str(
                "#if !defined(__GLIBC__) || !defined(__GLIBC_MINOR__) || __GLIBC__ != 2 || __GLIBC_MINOR__ != 28\n\
                 #error \"legacy ABI checks require glibc 2.28 headers\"\n\
                 #endif\n",
            );
        }
    }
    Ok(source)
}

/// Which names are absent after the baseline, expression and macro
/// probes. Any inconclusive result fails the row.
pub(super) fn unnamed(
    row: &str,
    base: &str,
    names: &[String],
    dir: &Path,
) -> Result<Result<Vec<String>, String>, Error> {
    if names.is_empty() {
        return Ok(Ok(Vec::new()));
    }
    if let Err(error) = compile(base, &dir.join(format!("{row}-headers.c")))? {
        return Ok(Err(format!("qualified-name baseline failed: {error}")));
    }
    let mut absent = Vec::new();
    let line = base.lines().count() + 1;
    for name in names {
        // A file-scope integer initializer has one expression to
        // diagnose. A failed static assertion can add a second error.
        let source = format!("{base}int ironafs_abi_qualified = (({name}) | 0);\n");
        let file = dir.join(format!("{row}-{name}.c"));
        let output = compile_output(&source, &file, true)?;
        if output.status.success() {
            continue;
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.code() != Some(1) || !absent_diagnostic(&stderr, &file, line, name) {
            return Ok(Err(format!(
                "inconclusive qualified-name probe for {name} ({}): {}",
                output.status,
                stderr.trim_end()
            )));
        }
        let source = format!(
            "{base}#if defined({name})\n#error \"qualified name {name} is a macro\"\n#endif\n"
        );
        if let Err(error) = compile(&source, &dir.join(format!("{row}-{name}-macro.c")))? {
            return Ok(Err(format!(
                "qualified-name macro check for {name} failed: {error}"
            )));
        }
        absent.push(name.clone());
    }
    Ok(Ok(absent))
}

/// Only a single primary error at the probe expression can establish
/// absence. In GCC and Clang's C locale the quoted identifier is ASCII.
pub(super) fn absent_diagnostic(stderr: &str, file: &Path, line: usize, name: &str) -> bool {
    let mut errors = stderr.lines().filter_map(primary_error);
    let Some((location, message)) = errors.next() else {
        return false;
    };
    if errors.next().is_some() {
        return false;
    }
    let prefix = format!("{}:{line}:", file.display());
    let Some(column) = location.strip_prefix(&prefix) else {
        return false;
    };
    column.parse::<usize>().is_ok()
        && (message == format!("error: use of undeclared identifier '{name}'")
            || gcc_undeclared_name(message, name))
}

/// GCC may append one suggested identifier to the exact undeclared-name
/// diagnostic. The suggestion is irrelevant to which name is absent.
fn gcc_undeclared_name(message: &str, name: &str) -> bool {
    let Some(suffix) = message.strip_prefix(&format!(
        "error: '{name}' undeclared here (not in a function)"
    )) else {
        return false;
    };
    if suffix.is_empty() {
        return true;
    }
    let Some(suggested) = suffix
        .strip_prefix("; did you mean '")
        .and_then(|rest| rest.strip_suffix("'?"))
    else {
        return false;
    };
    let mut identifier = suggested.bytes();
    identifier
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && identifier.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn primary_error(line: &str) -> Option<(&str, &str)> {
    if line.starts_with(char::is_whitespace) {
        return None;
    }
    let (location, message) = if line.starts_with("error: ") || line.starts_with("fatal error: ") {
        ("", line)
    } else {
        line.split_once(": ")?
    };
    (message.starts_with("error: ") || message.starts_with("fatal error: "))
        .then_some((location, message))
}
