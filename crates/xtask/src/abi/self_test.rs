// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-abi --self-test`: a fixture of declarations whose C facts are the
//! same on every Unix target of the allowlist (`struct timespec`, `EINTR`,
//! `close`, `strlen`, and `struct rlimit` through the typedef `rlim_t`,
//! whose spelling differs between C libraries at one size) must compile,
//! and each planted mismatch must be caught at the stage that owns it:
//!
//! - by the C compiler, after the declarations were read and the C
//!   generated (ADR 0005, point 5.6): a member type changed at the same
//!   size and offset, a member's size changed, a constant changed by one, a
//!   parameter's type changed, a return type changed, and an alias of the
//!   wrong size or sign. Each must fail with the assertion that names it;
//! - by the declaration reader, which refuses what C cannot be compared
//!   with: another ABI string, an attribute on a foreign function (such as
//!   `link_name`), a `packed` struct, a renaming import, and a C type name
//!   not imported from `core::ffi`.
//!
//! The fixture's headers are Unix headers. On a Windows host, which has no
//! row with a foreign item yet, the self-test says so and passes; the
//! first Windows row brings a Windows fixture.

use std::io::Write;
use std::path::Path;

use super::{DataModel, Declarations, applies_here, c_source, compile, declarations};
use crate::Error;
use crate::rust_source::tokens_with_strings;

const CHECK: &str = "check-abi";

const HEADERS: &[&str] = &[
    "errno.h",
    "string.h",
    "sys/resource.h",
    "time.h",
    "unistd.h",
];

const FIXTURE: &str = "use core::ffi::{c_char, c_int, c_long, c_uint, c_ulong};\n\
                       #[repr(C)]\n\
                       struct timespec {\n    tv_sec: c_long,\n    tv_nsec: c_long,\n}\n\
                       const EINTR: c_int = 4;\n\
                       type rlim_t = u64;\n\
                       #[repr(C)]\n\
                       struct rlimit {\n    rlim_cur: rlim_t,\n    rlim_max: rlim_t,\n}\n\
                       unsafe extern \"C\" {\n\
                       \x20   fn close(fd: c_int) -> c_int;\n\
                       \x20   fn strlen(s: *const c_char) -> usize;\n\
                       }\n";

/// Mismatches the C compiler must refuse: the case, the text it replaces,
/// and the assertion's label the compiler's error must carry.
const COMPILER: &[(&str, &str, &str, &str)] = &[
    (
        "a member type changed at the same size and offset",
        "tv_nsec: c_long",
        "tv_nsec: c_ulong",
        "->tv_nsec, unsigned long: 1",
    ),
    (
        "a member's size changed",
        "tv_sec: c_long",
        "tv_sec: c_int",
        "->tv_sec, int: 1",
    ),
    (
        "a constant changed by one",
        "EINTR: c_int = 4",
        "EINTR: c_int = 5",
        "EINTR == 5",
    ),
    (
        "a parameter's type changed",
        "fn close(fd: c_int)",
        "fn close(fd: c_uint)",
        "(&close, int (*)(unsigned int)",
    ),
    (
        "a return type changed",
        "*const c_char) -> usize",
        "*const c_char) -> isize",
        "(&strlen, ssize_t (*)(const char *)",
    ),
    (
        "an alias of the wrong size",
        "type rlim_t = u64",
        "type rlim_t = u32",
        "sizeof(rlim_t) == 4",
    ),
    (
        "an alias of the wrong sign",
        "type rlim_t = u64",
        "type rlim_t = i64",
        "(rlim_t)-1 < 0",
    ),
];

/// Declarations the reader must refuse: the case, the text it replaces,
/// and a phrase of the reader's error.
const READER: &[(&str, &str, &str, &str)] = &[
    (
        "another ABI string",
        "unsafe extern \"C\" {",
        "unsafe extern \"system\" {",
        "whose ABI is \"system\"",
    ),
    (
        "a foreign function linked under another symbol",
        "    fn close(",
        "    #[link_name = \"close$NOCANCEL\"]\n    fn close(",
        "an attribute on a foreign function",
    ),
    (
        "a packed struct",
        "#[repr(C)]\n",
        "#[repr(C)]\n#[repr(packed)]\n",
        "repr is not exactly C",
    ),
    (
        "a renaming import",
        "use core::ffi::{c_char, c_int, c_long, c_uint, c_ulong};",
        "use core::ffi::{c_char, c_int, c_long as c_ulong, c_uint};",
        "a renaming import",
    ),
    (
        "a C type name not imported from core::ffi",
        "use core::ffi::{c_char, c_int, c_long, c_uint, c_ulong};",
        "use core::ffi::{c_char, c_int, c_uint, c_ulong};",
        "`c_long` is neither imported from core::ffi",
    ),
];

/// Where a fixture stopped.
enum Stage {
    Reader(String),
    Generator(String),
    /// The compiler's error, and the generated line it points at.
    Compiler(String, String),
    Compiled,
}

/// Compiles the fixture, then each planted case.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
pub(super) fn run(dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let report = |output: &mut dyn Write, line: String| {
        writeln!(output, "{line}").map_err(Error::io("report check-abi self-test"))
    };
    if cfg!(windows) {
        let (applies, host) = applies_here()?;
        if applies {
            return Err(failure(
                "the fixture",
                format!("{host} has rows to prove, but the fixture's headers are Unix headers"),
            ));
        }
        return report(
            output,
            format!(
                "check-abi --self-test: nothing to qualify on {host} (no row with a foreign item), \
                 and the fixture is Unix's"
            ),
        );
    }
    match stage(FIXTURE, &dir.join("clean.c"))? {
        Stage::Compiled => report(output, "passed: the fixture".to_owned())?,
        Stage::Reader(error) | Stage::Generator(error) | Stage::Compiler(error, _) => {
            return Err(failure("the fixture", error));
        }
    }
    for (index, (name, from, to, label)) in COMPILER.iter().enumerate() {
        let planted = plant(name, from, to)?;
        match stage(&planted, &dir.join(format!("compiler-{index}.c")))? {
            Stage::Compiler(error, at) if at.contains(label) => {
                report(output, format!("caught by the C compiler: {name}: {error}"))?;
            }
            Stage::Compiler(error, at) => {
                return Err(failure(
                    name,
                    format!("the compiler failed at `{at}`, not at `{label}`: {error}"),
                ));
            }
            Stage::Reader(error) | Stage::Generator(error) => {
                return Err(failure(
                    name,
                    format!("stopped before the compiler, which it must reach: {error}"),
                ));
            }
            Stage::Compiled => {
                return Err(failure(name, "the C compiler accepted it".to_owned()));
            }
        }
    }
    for (name, from, to, phrase) in READER {
        let planted = plant(name, from, to)?;
        match stage(&planted, &dir.join("reader.c"))? {
            Stage::Reader(error) | Stage::Generator(error) if error.contains(phrase) => {
                report(output, format!("caught by the reader: {name}: {error}"))?;
            }
            Stage::Reader(error) | Stage::Generator(error) | Stage::Compiler(error, _) => {
                return Err(failure(
                    name,
                    format!("not refused for `{phrase}`: {error}"),
                ));
            }
            Stage::Compiled => return Err(failure(name, "it was accepted".to_owned())),
        }
    }
    report(
        output,
        format!(
            "check-abi --self-test: the fixture compiled; {} planted mismatches failed to compile \
             at their assertions and {} declarations were refused by the reader",
            COMPILER.len(),
            READER.len()
        ),
    )
}

fn plant(name: &'static str, from: &str, to: &str) -> Result<String, Error> {
    if FIXTURE.contains(from) {
        Ok(FIXTURE.replacen(from, to, 1))
    } else {
        Err(failure(
            name,
            format!("the fixture has no `{from}` to plant over"),
        ))
    }
}

/// Reads, generates and compiles one fixture, and says where it stopped.
fn stage(source: &str, file: &Path) -> Result<Stage, Error> {
    let tokens = tokens_with_strings(source).map_err(|error| Error::SelfTest {
        check: CHECK,
        case: "reading the fixture",
        found: vec![error.to_string()],
    })?;
    let mut found = Declarations::default();
    if let Err(error) = declarations(&tokens, "C", &mut found) {
        return Ok(Stage::Reader(error));
    }
    let headers: Vec<String> = HEADERS.iter().map(|header| (*header).to_owned()).collect();
    let c = match c_source("self-test", &headers, &found, DataModel::Lp64) {
        Ok(c) => c,
        Err(error) => return Ok(Stage::Generator(error)),
    };
    Ok(match compile(&c, file)? {
        Ok(()) => Stage::Compiled,
        Err(error) => {
            let at = error_line(&error, file)
                .and_then(|line| c.lines().nth(line.checked_sub(1)?))
                .unwrap_or_default()
                .to_owned();
            Stage::Compiler(error, at)
        }
    })
}

/// The line number a compiler error gives for `file`: `<file>:<line>:...`.
fn error_line(error: &str, file: &Path) -> Option<usize> {
    let rest = error.strip_prefix(&*file.to_string_lossy())?;
    rest.strip_prefix(':')?.split(':').next()?.parse().ok()
}

fn failure(case: &'static str, why: String) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: vec![why],
    }
}
