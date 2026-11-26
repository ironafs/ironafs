// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Compiler fixtures for the qualified socket name and its evidence rules.

use super::{declarations_of, failure, plant_in, report};
use crate::abi::qualified::baseline;
use crate::abi::{Declarations, Mode, Proof, compile, prove_declarations, prove_withheld};
use crate::os::inventory::{Code, Row};
use crate::rust_source::tokens_with_strings;
use crate::{Error, fixture};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

const GNU: &str = "aarch64-unknown-linux-gnu";
const MUSL: &str = "x86_64-unknown-linux-musl";
const DECLARATIONS: &str =
    "use core::ffi::c_int;\nconst SCM_PIDFD: c_int = 4;\nconst SOCK_STREAM: c_int = 1;\n";
const BASE: &str = "#if !defined(_GNU_SOURCE)\n#error missing GNU feature define\n#endif\n";
const CONTROLS: &str = "#define SOCK_STREAM 1\n#define SO_TYPE 3\nstruct ucred { int pid; };\n";
const GLIBC: &str = "#undef __GLIBC__\n#undef __GLIBC_MINOR__\n#undef __USE_GNU\n\
                    #define __GLIBC__ 2\n#define __GLIBC_MINOR__ 28\n#define __USE_GNU 1\n";
const MUSL_HEADERS: &str =
    "#undef __GLIBC__\n#undef __DEFINED_ssize_t\n#define __DEFINED_ssize_t 1\n";
const AARCH64: &str = "#undef __x86_64__\n#undef __aarch64__\n#define __aarch64__ 1\n";
const X86_64: &str = "#undef __aarch64__\n#undef __x86_64__\n#define __x86_64__ 1\n";
const NAME: &str = "#define SCM_PIDFD 4\n";

pub(super) fn run(dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let header = dir.join("fdpass-fixture.h");
    // Only the socket facts used by the check are handwritten. The common
    // preamble still uses this compiler's own headers and data model.
    let mut row = Row {
        number: "14".to_owned(),
        id: "linux-fdpass".to_owned(),
        columns: Default::default(),
        open: false,
        status: BTreeMap::new(),
        code: None,
    };
    let code = Code {
        module: "linux_fdpass".to_owned(),
        headers: vec![header.to_string_lossy().into_owned()],
        defines: vec!["_GNU_SOURCE".to_owned()],
        expressions: Vec::new(),
        qualified: vec!["SCM_PIDFD".to_owned()],
        extern_blocks: Vec::new(),
        blocks: Vec::new(),
        impls: 0,
        structs: Vec::new(),
        constants: vec!["SCM_PIDFD".to_owned(), "SOCK_STREAM".to_owned()],
        aliases: Vec::new(),
        stand_in: None,
    };
    let gnu = format!("{BASE}{AARCH64}{GLIBC}{CONTROLS}");
    gnu_lanes(&row, code.clone(), &header, &gnu, dir)?;
    baseline_controls(&row, code.clone(), &header, &gnu, dir, output)?;
    row.code = Some(code);
    musl_lane(&mut row, &header, dir)?;
    report(
        output,
        "passed: qualified socket ABI fixtures for complete GNU, legacy GNU and musl",
    )
}

/// Both GNU lanes use the same Rust fixture. The legacy lane may omit
/// only its named socket assertion after the other assertions compile.
fn gnu_lanes(row: &Row, mut code: Code, header: &Path, gnu: &str, dir: &Path) -> Result<(), Error> {
    let modern = format!("{gnu}#undef __GLIBC_MINOR__\n#define __GLIBC_MINOR__ 39\n{NAME}");
    fixture::write_file(header, &modern)?;
    exact(
        "modern GNU with a named constant",
        &prove_gnu(row, &code, dir, DECLARATIONS, Mode::Complete)?,
        &Proof::Proved("linux-fdpass (0 structs, 2 constants, 0 functions)".to_owned()),
    )?;
    let case = "modern GNU with a mismatched qualified value";
    let planted = plant_in(
        DECLARATIONS,
        case,
        "SCM_PIDFD: c_int = 4",
        "SCM_PIDFD: c_int = 5",
    )?;
    refused(
        case,
        &prove_gnu(row, &code, dir, &planted, Mode::Complete)?,
        "linux-fdpass: SCM_PIDFD",
    )?;
    refused(
        "legacy option on other headers",
        &prove_gnu(row, &code, dir, DECLARATIONS, Mode::LegacyGlibc228)?,
        "require glibc 2.28 headers",
    )?;

    fixture::write_file(header, gnu)?;
    refused(
        "ordinary GNU with no named constant",
        &prove_gnu(row, &code, dir, DECLARATIONS, Mode::Complete)?,
        "MISSING SCM_PIDFD",
    )?;
    exact(
        "legacy GNU with one absent qualified constant",
        &prove_gnu(row, &code, dir, DECLARATIONS, Mode::LegacyGlibc228)?,
        &Proof::Partial("linux-fdpass: MISSING SCM_PIDFD".to_owned()),
    )?;
    for (case, from, to, expected) in [
        (
            "an independent assertion with the wrong value",
            "SOCK_STREAM: c_int = 1",
            "SOCK_STREAM: c_int = 2",
            "linux-fdpass: SOCK_STREAM",
        ),
        (
            "no independent assertion",
            "const SOCK_STREAM: c_int = 1;\n",
            "",
            "no independent ABI assertion",
        ),
        (
            "a missing typed entry",
            "const SCM_PIDFD: c_int = 4;\n",
            "",
            "exactly one typed Rust constant",
        ),
        (
            "a duplicate typed entry",
            "const SCM_PIDFD: c_int = 4;\n",
            "const SCM_PIDFD: c_int = 4;\nconst SCM_PIDFD: c_int = 4;\n",
            "exactly one typed Rust constant",
        ),
    ] {
        let planted = plant_in(DECLARATIONS, case, from, to)?;
        refused(
            case,
            &prove_gnu(row, &code, dir, &planted, Mode::LegacyGlibc228)?,
            expected,
        )?;
    }
    code.expressions = vec![("SOCK_STREAM".to_owned(), "SCM_PIDFD".to_owned())];
    refused(
        "an independent assertion depending on the omitted name",
        &prove_gnu(row, &code, dir, DECLARATIONS, Mode::LegacyGlibc228)?,
        "SCM_PIDFD",
    )?;
    code.expressions = vec![("SCM_PIDFD".to_owned(), "4".to_owned())];
    for (headers, mode) in [
        (gnu, Mode::LegacyGlibc228),
        (modern.as_str(), Mode::Complete),
    ] {
        fixture::write_file(header, headers)?;
        refused(
            "a qualified entry with an alternate C expression",
            &prove_gnu(row, &code, dir, DECLARATIONS, mode)?,
            "must be asserted by its C name",
        )?;
    }
    code.expressions.clear();
    fixture::write_file(header, &format!("{gnu}{NAME}"))?;
    refused(
        "legacy GNU with a named constant",
        &prove_gnu(row, &code, dir, DECLARATIONS, Mode::LegacyGlibc228)?,
        "revisit the glibc 2.28 exception",
    )?;

    Ok(())
}

/// The probe earns no evidence when the compiler or socket baseline fails.
fn baseline_controls(
    row: &Row,
    mut code: Code,
    header: &Path,
    gnu: &str,
    dir: &Path,
    output: &mut impl Write,
) -> Result<(), Error> {
    for (case, contents, expected) in [
        (
            "wrong compiler architecture",
            format!("{BASE}{X86_64}{GLIBC}{CONTROLS}"),
            "wrong architecture",
        ),
        (
            "missing GNU header mode",
            format!("{gnu}#undef __USE_GNU\n"),
            "requires __USE_GNU",
        ),
        (
            "missing socket type control",
            format!("{gnu}#undef SO_TYPE\n"),
            "SO_TYPE",
        ),
        (
            "malformed integer control",
            format!("{gnu}#undef SOCK_STREAM\n#define SOCK_STREAM 1.5\n"),
            "invalid operands to binary",
        ),
        (
            "unusable integer control",
            format!("{gnu}#undef SO_TYPE\n#define SO_TYPE (1 / 0)\n"),
            "qualified-name baseline failed",
        ),
        (
            "incomplete credentials control",
            format!(
                "{BASE}{AARCH64}{GLIBC}{}",
                CONTROLS.replace("struct ucred { int pid; };", "struct ucred;")
            ),
            "struct ucred",
        ),
        (
            "missing header",
            format!("{gnu}#include <ironafs_abi_absent_fixture.h>\n"),
            "ironafs_abi_absent_fixture.h",
        ),
    ] {
        fixture::write_file(header, &contents)?;
        refused(
            case,
            &prove_gnu(row, &code, dir, DECLARATIONS, Mode::LegacyGlibc228)?,
            expected,
        )?;
    }
    let case = "late GNU feature define";
    let glibc = plant_in(
        GLIBC,
        case,
        "#define __USE_GNU 1\n",
        "#if defined(_GNU_SOURCE)\n#define __USE_GNU 1\n#endif\n",
    )?;
    fixture::write_file(header, &format!("{AARCH64}{glibc}{CONTROLS}"))?;
    let source =
        baseline(row, &code, GNU, Mode::LegacyGlibc228).map_err(|error| failure(case, error))?;
    if let Err(error) = compile(&source, &dir.join("fdpass-early-gnu.c"))? {
        return Err(failure(case, format!("the early define failed: {error}")));
    }
    let define = "#define _GNU_SOURCE 1\n";
    let header_include = format!("#include <{}>\n", header.display());
    let late = plant_in(&source, case, define, "")?;
    let late = plant_in(
        &late,
        case,
        &header_include,
        &format!("{header_include}{define}"),
    )?;
    match compile(&late, &dir.join("fdpass-late-gnu.c"))? {
        Err(error)
            if error.contains("the socket baseline requires __USE_GNU after the glibc headers") =>
        {
            report(
                output,
                format!("passed: early GNU feature define; caught a late define: {error}"),
            )?;
        }
        Err(error) => {
            return Err(failure(
                case,
                format!("refused for another reason: {error}"),
            ));
        }
        Ok(()) => return Err(failure(case, "the late define compiled".to_owned())),
    }

    code.defines.clear();
    fixture::write_file(header, gnu)?;
    refused(
        "missing GNU feature define",
        &prove_gnu(row, &code, dir, DECLARATIONS, Mode::LegacyGlibc228)?,
        "the socket baseline requires _GNU_SOURCE before the headers",
    )
}

/// The approved-only socket row needs a conclusive result from its headers.
fn musl_lane(row: &mut Row, header: &Path, dir: &Path) -> Result<(), Error> {
    let musl = format!("{BASE}{X86_64}{MUSL_HEADERS}{CONTROLS}");
    fixture::write_file(header, &musl)?;
    exact(
        "musl with an absent qualified name",
        &prove_withheld(row, MUSL, dir)?,
        &Proof::Withheld("linux-fdpass: MISSING SCM_PIDFD".to_owned()),
    )?;
    fixture::write_file(header, &format!("{musl}{NAME}"))?;
    refused(
        "musl with a usable qualified name",
        &prove_withheld(row, MUSL, dir)?,
        "revisit withholding",
    )?;
    "13".clone_into(&mut row.number);
    refused(
        "an unrecognised withheld row",
        &prove_withheld(row, MUSL, dir)?,
        "unrecognised qualified socket row",
    )?;
    Ok(())
}

fn prove_gnu(
    row: &Row,
    code: &Code,
    dir: &Path,
    declarations: &str,
    mode: Mode,
) -> Result<Proof, Error> {
    let mut found = Declarations::default();
    let tokens = tokens_with_strings(declarations)
        .map_err(|error| failure("read the socket fixture", error.to_string()))?;
    declarations_of(&tokens, &mut found)?;
    prove_declarations(row, code, found, GNU, mode, dir)
}

fn exact(case: &'static str, found: &Proof, expected: &Proof) -> Result<(), Error> {
    if found == expected {
        Ok(())
    } else {
        Err(failure(
            case,
            format!("found {found:?}, expected {expected:?}"),
        ))
    }
}

fn refused(case: &'static str, found: &Proof, expected: &str) -> Result<(), Error> {
    if matches!(found, Proof::Failed(reason) if reason.contains(expected)) {
        Ok(())
    } else {
        Err(failure(
            case,
            format!("found {found:?}, expected failure containing {expected:?}"),
        ))
    }
}
