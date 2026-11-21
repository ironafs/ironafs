// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-abi --self-test`: a fixture of declarations whose C facts are the
//! same on every Unix target of the allowlist (`struct timespec`, `EINTR`,
//! `close`, `strlen`, and `struct rlimit` through the typedef `rlim_t`,
//! whose spelling differs between C libraries at one size) must compile, and each planted mismatch ADR 0005,
//! point 5.6, names must not: a member type changed at the same size and
//! offset, a constant changed by one, a parameter's type changed and a
//! return type changed; and an alias of the wrong size or sign. Each runs through the same reader, generator and
//! compiler as the real check.

use std::io::Write;
use std::path::Path;

use super::{DataModel, Declarations, c_source, compile, declarations};
use crate::Error;
use crate::rust_source::tokens;

const CHECK: &str = "check-abi";

const HEADERS: &[&str] = &[
    "errno.h",
    "string.h",
    "sys/resource.h",
    "time.h",
    "unistd.h",
];

const FIXTURE: &str = "use core::ffi::{c_char, c_int, c_long};\n\
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

/// Each planted mismatch: what it is, and the text it replaces.
const PLANTED: &[(&str, &str, &str)] = &[
    (
        "a member type changed at the same size and offset",
        "tv_nsec: c_long",
        "tv_nsec: c_ulong",
    ),
    (
        "a constant changed by one",
        "EINTR: c_int = 4",
        "EINTR: c_int = 5",
    ),
    (
        "a parameter's type changed",
        "fn close(fd: c_int)",
        "fn close(fd: c_uint)",
    ),
    (
        "a return type changed",
        "*const c_char) -> usize",
        "*const c_char) -> isize",
    ),
    ("a member's size changed", "tv_sec: c_long", "tv_sec: c_int"),
    (
        "an alias of the wrong size",
        "type rlim_t = u64",
        "type rlim_t = u32",
    ),
    (
        "an alias of the wrong sign",
        "type rlim_t = u64",
        "type rlim_t = i64",
    ),
];

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
        return Err(failure(
            "the fixture",
            "the fixture's headers are Unix headers; the Windows fixture comes with the first \
             Windows site"
                .to_owned(),
        ));
    }
    if let Err(error) = generate_and_compile(FIXTURE, &dir.join("clean.c"))? {
        return Err(failure("the fixture", error));
    }
    report(output, "passed: the fixture".to_owned())?;
    for (index, (name, from, to)) in PLANTED.iter().enumerate() {
        if !FIXTURE.contains(from) {
            return Err(failure(
                name,
                format!("the fixture has no `{from}` to plant over"),
            ));
        }
        let planted = FIXTURE.replacen(from, to, 1);
        match generate_and_compile(&planted, &dir.join(format!("case-{index}.c")))? {
            Err(error) => report(output, format!("caught: {name}: {error}"))?,
            Ok(()) => return Err(failure(name, "the C compiler accepted it".to_owned())),
        }
    }
    report(
        output,
        format!(
            "check-abi --self-test: the fixture compiled and {} planted mismatches failed to compile",
            PLANTED.len()
        ),
    )
}

/// Reads, generates and compiles one fixture; the inner error is the
/// reader's, the generator's or the compiler's.
fn generate_and_compile(source: &str, file: &Path) -> Result<Result<(), String>, Error> {
    let tokens = tokens(source).map_err(|error| Error::SelfTest {
        check: CHECK,
        case: "reading the fixture",
        found: vec![error.to_string()],
    })?;
    let mut found = Declarations::default();
    if let Err(error) = declarations(&tokens, &mut found) {
        return Ok(Err(error));
    }
    let headers: Vec<String> = HEADERS.iter().map(|header| (*header).to_owned()).collect();
    match c_source("self-test", &headers, &found, DataModel::Lp64) {
        Ok(c) => compile(&c, file),
        Err(error) => Ok(Err(error)),
    }
}

fn failure(case: &'static str, why: String) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: vec![why],
    }
}
