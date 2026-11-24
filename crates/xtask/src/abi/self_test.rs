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
//!   size and offset, a member's size changed, a constant changed by one
//!   (one compared with a C expression too), a
//!   parameter's type changed, a return type changed, an alias of the
//!   wrong size or sign, an alias of `c_char` naming a typedef of the
//!   other sign, and aliases of integers naming typedefs of `_Bool` and
//!   `double`. Each must fail with the assertion that names it;
//! - by the probe of qualified constants: a name the headers do not define
//!   is reported unnamed (so its row is withheld), and one they define is
//!   not;
//! - by the comparison with sites.json's record: a type alias the
//!   declarations state but the record leaves out, and one the record
//!   holds but no declaration states;
//! - by the import reader and resolver, over a row of four files: a
//!   declaration's type imported from another module of the crate while
//!   the row's own type of that name is still declared; a re-export whose
//!   cfg is not in effect on the target, though its file was compiled,
//!   alone and beside a binding through an inline module; an import nested
//!   in a function; an import under another cfg; and a glob;
//! - by the declaration reader, which refuses what C cannot be compared
//!   with: another ABI string, an attribute on a foreign function (such as
//!   `link_name`), a `packed` struct, a renaming import, a type alias, an
//!   import under an attribute holding brackets (alone, and after a cfg,
//!   which a nearest-bracket scan would drop), and a C type name not
//!   imported from `core::ffi`; and by the generator, a live member
//!   renamed opaque that xtask's allowlist does not approve, and a struct
//!   of opaque members alone.
//!
//! On Linux a second fixture, `struct sockaddr` from `<sys/socket.h>`
//! (whose `sa_data` is `char[14]` in glibc and musl, while macOS adds a
//! length byte), must compile, and with its array's element type changed
//! at the same size (`uint8_t`, which is not `char`) must fail at the
//! array's assertion.
//!
//! The fixture's headers are Unix headers. On a Windows host, which has no
//! row with a foreign item yet, the self-test says so and passes; the
//! first Windows row brings a Windows fixture.

use std::io::Write;
use std::path::Path;

use super::{
    DataModel, Declarations, Recorded, applies_here, c_source, compile, declarations,
    fact_differences, header_macros, required_constants, row_declarations, unnamed,
};
use crate::Error;
use crate::fixture;
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
                       const TIMESPEC_SIZE: usize = 16;\n\
                       type rlim_t = u64;\n\
                       #[repr(C)]\n\
                       struct rlimit {\n    rlim_cur: rlim_t,\n    rlim_max: rlim_t,\n}\n\
                       unsafe extern \"C\" {\n\
                       \x20   fn close(fd: c_int) -> c_int;\n\
                       \x20   fn strlen(s: *const c_char) -> usize;\n\
                       }\n";

/// The Linux fixture of an array member, its header, and its plant.
const ARRAY_FIXTURE: &str = "use core::ffi::{c_char, c_ushort};\n\
                             #[repr(C)]\n\
                             struct sockaddr {\n    sa_family: c_ushort,\n    sa_data: [c_char; 14],\n}\n";
const ARRAY_HEADERS: &[&str] = &["sys/socket.h"];
const ARRAY_PLANT: (&str, &str, &str, &str) = (
    "an array member's element type changed at the same size",
    "[c_char; 14]",
    "[u8; 14]",
    "->sa_data, uint8_t (*)[14]: 1",
);

/// The fixture's constants compared with a C expression, as sites.json's
/// `expressions` maps a value a macro gives.
const EXPRESSIONS: &[(&str, &str)] = &[("TIMESPEC_SIZE", "sizeof(struct timespec)")];

/// Mismatches the C compiler must refuse: the case, the text it replaces,
/// and the assertion's label the compiler's error must carry.
const COMPILER: &[(&str, &str, &str, &str)] = &[
    (
        "a member type changed at the same size and offset",
        "tv_nsec: c_long",
        "tv_nsec: c_ulong",
        "->tv_nsec, unsigned long *: 1",
    ),
    (
        "a member's size changed",
        "tv_sec: c_long",
        "tv_sec: c_int",
        "->tv_sec, int *: 1",
    ),
    (
        "a constant changed by one",
        "EINTR: c_int = 4",
        "EINTR: c_int = 5",
        "EINTR == 5",
    ),
    (
        "a constant written with a leading zero, emitted as its decimal value",
        "EINTR: c_int = 4",
        "EINTR: c_int = 010",
        "EINTR == 10",
    ),
    (
        "a constant compared with a C expression changed by one",
        "TIMESPEC_SIZE: usize = 16",
        "TIMESPEC_SIZE: usize = 17",
        "(sizeof(struct timespec)) == 17",
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

/// A typedef of a byte whose sign is not `char`'s on this host, which an
/// alias of `c_char` must not pass as, and the assertion that refuses it.
fn other_sign_byte() -> (&'static str, &'static str) {
    if core::ffi::c_char::MIN == 0 {
        ("int8_t", "((int8_t)-1 > 0) == ((char)-1 > 0)")
    } else {
        ("uint8_t", "((uint8_t)-1 > 0) == ((char)-1 > 0)")
    }
}

/// A name no header defines, which the probe must report unnamed.
const UNNAMED_NAME: &str = "IRONAFS_CHECK_ABI_SELF_TEST_UNNAMED";

/// Declarations the reader must refuse: the case, the text it replaces,
/// and a phrase of the reader's error.
const READER: &[(&str, &str, &str, &str)] = &[
    (
        "a live member renamed opaque, which the allowlist does not approve",
        "    tv_nsec: c_long,\n",
        "    _opaque_nsec: [c_long; 1],\n",
        "allowlist of opaque tails does not approve it",
    ),
    (
        "a struct of opaque members alone",
        "struct timespec {\n    tv_sec: c_long,\n    tv_nsec: c_long,\n}",
        "struct timespec {\n    _opaque: [c_long; 2],\n}",
        "every member is opaque",
    ),
    (
        "a type alias",
        "const EINTR",
        "type c_ulong = u32;\nconst EINTR",
        "a type alias",
    ),
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
        "an import under a cfg and an attribute holding brackets",
        "use core::ffi::{c_char, c_int, c_long, c_uint, c_ulong};",
        "#[cfg(miri)]\n#[doc = stringify!([x])]\npub(super) use core::ffi::{c_char, c_int, c_long, c_uint, c_ulong};",
        "with attributes this check does not read",
    ),
    (
        "an import under an attribute holding brackets",
        "use core::ffi::{c_char, c_int, c_long, c_uint, c_ulong};",
        "#[doc = stringify!([x])]\nuse core::ffi::{c_char, c_int, c_long, c_uint, c_ulong};",
        "with attributes this check does not read",
    ),
    (
        "a renaming import",
        "use core::ffi::{c_char, c_int, c_long, c_uint, c_ulong};",
        "use core::ffi::{c_char, c_int, c_long as c_ulong, c_uint};",
        "a renaming import",
    ),
    (
        "a module named core, shadowing core::ffi",
        "const EINTR",
        "mod core { pub mod ffi { pub type c_int = u8; } }\nconst EINTR",
        "named `core`",
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
    array_fixture(dir, output)?;
    let planted_count = compiler_stage(dir, output)?;
    char_sign_and_probe(dir, output)?;
    non_integer_aliases(dir, output)?;
    array_as_pointer(dir, output)?;
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
    missing_fact_case(dir)?;
    report(
        output,
        "caught: a constant the code names, a C macro of its headers, declared outside the \
         declaration files"
            .to_owned(),
    )?;
    recorded_facts_case(output)?;
    foreign_import_case(dir, output)?;
    report(
        output,
        format!(
            "check-abi --self-test: the fixture compiled; {} planted mismatches failed to compile \
             at their assertions, an unnamed qualified constant was withheld, {} declarations \
             were refused by the reader, a missing fact was found, and a copy that leaves out \
             or adds a type alias was caught, as was a type taken from outside the row's \
             declaration files",
            planted_count + 2 + NON_INTEGER.len(),
            READER.len()
        ),
    )
}

/// The row's code (here, a root file beside the fixture) names `EINTR`, a
/// macro of the fixture's headers: with the fixture's declaration, nothing
/// is missing; with that declaration moved out of the declaration files,
/// `EINTR` is a required fact no assertion proves.
fn missing_fact_case(dir: &Path) -> Result<(), Error> {
    const CASE: &str = "a constant the code names, declared outside the declaration files";
    const ROOT: &str = "fn interrupted(code: c_int) -> bool { code == abi::EINTR }\n";
    let headers: Vec<String> = HEADERS.iter().map(|header| (*header).to_owned()).collect();
    let macros = match header_macros(
        "self-test",
        &[],
        &headers,
        DataModel::Lp64,
        &dir.join("macros.c"),
    )? {
        Ok(macros) => macros,
        Err(error) => return Err(failure(CASE, error)),
    };
    let undeclared = |declarations: &str| -> Result<Vec<String>, Error> {
        let mut found = Declarations::default();
        let tokens =
            tokens_with_strings(declarations).map_err(|error| failure(CASE, error.to_string()))?;
        declarations_of(&tokens, &mut found)?;
        let used: Vec<String> = [ROOT, declarations]
            .iter()
            .filter_map(|text| crate::rust_source::tokens(text).ok())
            .flatten()
            .filter_map(|token| crate::rust_source::ident(Some(&token)).map(str::to_owned))
            .collect();
        Ok(required_constants(used.iter().map(String::as_str), &macros)
            .into_iter()
            .filter(|name| !found.constants.iter().any(|(constant, _)| constant == name))
            .collect())
    };
    let clean = undeclared(FIXTURE)?;
    if !clean.is_empty() {
        return Err(failure(CASE, format!("the fixture itself lacks {clean:?}")));
    }
    let moved = undeclared(&plant(CASE, "const EINTR: c_int = 4;\n", "")?)?;
    if moved == ["EINTR"] {
        Ok(())
    } else {
        Err(failure(CASE, format!("found {moved:?}, not [\"EINTR\"]")))
    }
}

/// sites.json's record of the fixture's C facts, against what the fixture
/// declares: the full record differs in nothing; one without the fixture's
/// type alias `rlim_t` leaves it unrecorded, and one with a `socklen_t` the
/// fixture does not declare records it as missing.
fn recorded_facts_case(output: &mut impl Write) -> Result<(), Error> {
    const CASE: &str = "a type alias left out of the record, or recorded but not declared";
    let mut found = Declarations::default();
    let tokens = tokens_with_strings(FIXTURE).map_err(|error| failure(CASE, error.to_string()))?;
    declarations_of(&tokens, &mut found)?;
    let structs = ["timespec".to_owned(), "rlimit".to_owned()];
    let constants = ["EINTR".to_owned(), "TIMESPEC_SIZE".to_owned()];
    let differences = |aliases: &[String]| {
        fact_differences(
            Recorded {
                structs: &structs,
                constants: &constants,
                aliases,
            },
            &found,
        )
    };
    let none = (Vec::new(), Vec::new());
    let cases = [
        (vec!["rlim_t".to_owned()], none.clone()),
        (Vec::new(), (Vec::new(), vec!["rlim_t".to_owned()])),
        (
            vec!["rlim_t".to_owned(), "socklen_t".to_owned()],
            (vec!["socklen_t".to_owned()], Vec::new()),
        ),
    ];
    for (aliases, expected) in cases {
        let differs = differences(&aliases);
        if differs != expected {
            return Err(failure(
                CASE,
                format!("recording aliases {aliases:?} gave {differs:?}, not {expected:?}"),
            ));
        }
    }
    writeln!(
        output,
        "caught: a type alias declared but not recorded, and one recorded but not declared"
    )
    .map_err(Error::io("report check-abi self-test"))
}

/// A row of four declaration files, read for a glibc target: each C
/// library's file declares `struct timespec`, `abi.rs` re-exports the
/// glibc one under its cfg, and `abi/foreign.rs` declares a function over
/// the `timespec` it imports from `super`. That is read. Each plant must be
/// refused: a type taken from another module of the crate; the musl
/// re-export alone (not in effect on glibc, though musl's file was
/// compiled) beside a binding through an inline module; the musl re-export
/// alone; an import nested in a function; an import under another cfg; and
/// a glob.
fn foreign_import_case(dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    const CASE: &str = "a type whose binding is not the row's own";
    const TARGET: &str = "aarch64-unknown-linux-gnu";
    const LIBRARY: &str = "use core::ffi::c_long;\n#[repr(C)]\n\
                           pub(crate) struct timespec {\n    tv_sec: c_long,\n    tv_nsec: c_long,\n}\n";
    const GNU_EXPORT: &str = "#[cfg(target_env = \"gnu\")]\npub(super) use gnu::timespec;\n";
    const MUSL_EXPORT: &str = "#[cfg(target_env = \"musl\")]\npub(super) use musl::timespec;\n";
    let foreign = |import: &str| {
        format!(
            "use core::ffi::c_int;\n{import}\nunsafe extern \"C\" {{\n    \
             fn clock_gettime(clock: c_int, now: *mut timespec) -> c_int;\n}}\n"
        )
    };
    let crate_dir = dir.join("row-crate");
    let module = Path::new("src").join("row");
    let files = ["abi.rs", "abi/gnu.rs", "abi/musl.rs", "abi/foreign.rs"];
    let read: std::collections::BTreeSet<_> = files.iter().map(|file| module.join(file)).collect();
    let attempt = |abi: &str, import: &str| -> Result<Result<Declarations, String>, Error> {
        let texts = [
            format!("{abi}#[cfg(target_env = \"gnu\")]\nmod gnu;\n"),
            LIBRARY.to_owned(),
            LIBRARY.to_owned(),
            foreign(import),
        ];
        for (file, text) in files.iter().zip(texts) {
            fixture::write_file(&crate_dir.join(&module).join(file), &text)?;
        }
        row_declarations(&crate_dir, "row", &read, TARGET)
    };
    let own = "use super::timespec;";
    if let Err(error) = attempt(GNU_EXPORT, own)? {
        return Err(failure(
            CASE,
            format!("the row's own import was refused: {error}"),
        ));
    }
    let inline = format!(
        "{MUSL_EXPORT}mod inner {{\n    pub(super) use crate::elsewhere::timespec;\n}}\n\
         pub(super) use inner::timespec;\n"
    );
    for (abi, import, says) in [
        (
            GNU_EXPORT,
            "use crate::elsewhere::timespec;",
            "a path (crate::",
        ),
        (inline.as_str(), own, "an inline module"),
        (MUSL_EXPORT, own, "`timespec` is neither imported"),
        (
            GNU_EXPORT,
            "fn f() {\n    use super::timespec;\n}",
            "not at file scope",
        ),
        (
            GNU_EXPORT,
            "#[cfg(unix)]\nuse super::timespec;",
            "an import under #[cfg(unix)]",
        ),
        (GNU_EXPORT, "use super::*;", "a glob import"),
    ] {
        match attempt(abi, import)? {
            Err(error) if error.contains(says) => {}
            Err(error) => {
                return Err(failure(
                    CASE,
                    format!("{import}: refused for another reason than {says:?}: {error}"),
                ));
            }
            Ok(_) => return Err(failure(CASE, format!("{import}: accepted"))),
        }
    }
    writeln!(
        output,
        "caught: a declaration's type imported from another module, an inactive re-export \
         beside a binding through an inline module, an inactive re-export alone, a nested \
         import, an import under another cfg, and a glob"
    )
    .map_err(Error::io("report check-abi self-test"))
}

fn declarations_of(
    tokens: &[crate::rust_source::Token],
    found: &mut Declarations,
) -> Result<(), Error> {
    declarations(tokens, "C", found).map_err(|error| failure("reading the fixture", error))
}

/// The `c_char` alias planted with a typedef of the other sign, which the
/// compiler must refuse, and the probe of qualified constants, which must
/// report only the unnamed one.
fn char_sign_and_probe(dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let (byte, label) = other_sign_byte();
    let name = "an alias of c_char naming a typedef of the other sign";
    let planted = plant(
        name,
        "const EINTR",
        &format!("type {byte} = c_char;\nconst EINTR"),
    )?;
    match stage(&planted, &dir.join("char-sign.c"))? {
        Stage::Compiler(error, at) if at.contains(label) => {
            report(output, format!("caught by the C compiler: {name}: {error}"))?;
        }
        Stage::Compiler(error, _) | Stage::Reader(error) | Stage::Generator(error) => {
            return Err(failure(name, format!("not refused at `{label}`: {error}")));
        }
        Stage::Compiled => return Err(failure(name, "the C compiler accepted it".to_owned())),
    }
    let headers: Vec<String> = HEADERS.iter().map(|header| (*header).to_owned()).collect();
    let names = ["EINTR".to_owned(), UNNAMED_NAME.to_owned()];
    let name = "a qualified constant the headers do not name";
    match unnamed("self-test", &[], &headers, &names, DataModel::Lp64, "", dir)? {
        Ok(absent) if absent == [UNNAMED_NAME] => report(
            output,
            format!("withheld by the probe: {name}; a named one (EINTR) was not"),
        )?,
        Ok(absent) => {
            return Err(failure(
                name,
                format!("the probe found unnamed: {absent:?}"),
            ));
        }
        Err(error) => return Err(failure(name, error)),
    }
    Ok(())
}

/// Aliases of integers the size and sign of a C typedef that is not an
/// integer: the case, the typedef, and the alias planted against it. The
/// headers have no typedef of `_Bool` or `double`, so the generated file
/// gets the typedef after its preamble.
const NON_INTEGER: &[(&str, &str, &str)] = &[
    (
        "an alias of u8 naming a typedef of _Bool",
        "typedef _Bool flag_t;",
        "type flag_t = u8;",
    ),
    (
        "an alias of i64 naming a typedef of double",
        "typedef double real_t;",
        "type real_t = i64;",
    ),
];

/// Each of [`NON_INTEGER`]'s aliases, which pass the size, alignment and
/// sign assertions, must fail at the integer-type assertion.
fn non_integer_aliases(dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let label = "is an integer type other than _Bool";
    for (index, (name, typedef, alias)) in NON_INTEGER.iter().enumerate() {
        let planted = plant(name, "const EINTR", &format!("{alias}\nconst EINTR"))?;
        let file = dir.join(format!("non-integer-{index}.c"));
        match stage_with(&planted, &file, typedef)? {
            Stage::Compiler(error, at) if at.contains(label) => {
                report(output, format!("caught by the C compiler: {name}: {error}"))?;
            }
            Stage::Compiler(error, _) | Stage::Reader(error) | Stage::Generator(error) => {
                return Err(failure(name, format!("not refused at `{label}`: {error}")));
            }
            Stage::Compiled => return Err(failure(name, "the C compiler accepted it".to_owned())),
        }
    }
    Ok(())
}

/// Each compiler-stage plant must fail at its assertion; returns how many
/// ran.
fn compiler_stage(dir: &Path, output: &mut impl Write) -> Result<usize, Error> {
    let compiler = compiler_cases()?;
    let count = compiler.len();
    for (index, case) in compiler.into_iter().enumerate() {
        let Planted {
            name,
            source: planted,
            headers,
            expressions,
            label,
        } = case;
        match stage_of(
            &planted,
            headers,
            expressions,
            &dir.join(format!("compiler-{index}.c")),
            "",
        )? {
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
    Ok(count)
}

/// A C struct with an eight-byte array member, placed in the generated
/// file, and its Rust declaration: with the array declared as one, it must
/// compile; declared as a pointer, of the same size, alignment and offset
/// on LP64, it must fail at the member's type assertion, which compares
/// the member's address, so the array cannot decay to a pointer and match.
fn array_as_pointer(dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    const TYPEDEF: &str = "struct holder { long a; char b[8]; };";
    const HOLDER: &str =
        "#[repr(C)]\nstruct holder {\n    a: c_long,\n    b: [c_char; 8],\n}\nconst EINTR";
    let name = "an array member declared as a pointer of the same size";
    let label = "->b, char * *: 1";
    let clean = plant(name, "const EINTR", HOLDER)?;
    match stage_with(&clean, &dir.join("holder.c"), TYPEDEF)? {
        Stage::Compiled => {}
        Stage::Reader(error) | Stage::Generator(error) | Stage::Compiler(error, _) => {
            return Err(failure(
                name,
                format!("the array itself was refused: {error}"),
            ));
        }
    }
    let planted = plant(
        name,
        "const EINTR",
        &HOLDER.replacen("b: [c_char; 8]", "b: *mut c_char", 1),
    )?;
    match stage_with(&planted, &dir.join("holder-pointer.c"), TYPEDEF)? {
        Stage::Compiler(error, at) if at.contains(label) => {
            report(output, format!("caught by the C compiler: {name}: {error}"))
        }
        Stage::Compiler(error, _) | Stage::Reader(error) | Stage::Generator(error) => {
            Err(failure(name, format!("not refused at `{label}`: {error}")))
        }
        Stage::Compiled => Err(failure(name, "the C compiler accepted it".to_owned())),
    }
}

/// On Linux, the array fixture must compile.
fn array_fixture(dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    if !cfg!(target_os = "linux") {
        return Ok(());
    }
    match stage_of(ARRAY_FIXTURE, ARRAY_HEADERS, &[], &dir.join("array.c"), "")? {
        Stage::Compiled => report(output, "passed: the array fixture".to_owned()),
        Stage::Reader(error) | Stage::Generator(error) | Stage::Compiler(error, _) => {
            Err(failure("the array fixture", error))
        }
    }
}

/// One mismatch planted for the C compiler: its name, the planted
/// declarations with their headers and expressions, and the assertion's
/// label the compiler's error must carry.
struct Planted {
    name: &'static str,
    source: String,
    headers: &'static [&'static str],
    expressions: &'static [(&'static str, &'static str)],
    label: &'static str,
}

/// The compiler-stage plants: the fixture's, and on Linux the array
/// fixture's.
fn compiler_cases() -> Result<Vec<Planted>, Error> {
    let mut cases = Vec::new();
    for &(name, from, to, label) in COMPILER {
        cases.push(Planted {
            name,
            source: plant(name, from, to)?,
            headers: HEADERS,
            expressions: EXPRESSIONS,
            label,
        });
    }
    if cfg!(target_os = "linux") {
        let (name, from, to, label) = ARRAY_PLANT;
        cases.push(Planted {
            name,
            source: plant_in(ARRAY_FIXTURE, name, from, to)?,
            headers: ARRAY_HEADERS,
            expressions: &[],
            label,
        });
    }
    Ok(cases)
}

fn plant(name: &'static str, from: &str, to: &str) -> Result<String, Error> {
    plant_in(FIXTURE, name, from, to)
}

fn plant_in(fixture: &str, name: &'static str, from: &str, to: &str) -> Result<String, Error> {
    if fixture.contains(from) {
        Ok(fixture.replacen(from, to, 1))
    } else {
        Err(failure(
            name,
            format!("the fixture has no `{from}` to plant over"),
        ))
    }
}

/// Reads, generates and compiles one fixture, and says where it stopped.
fn stage(source: &str, file: &Path) -> Result<Stage, Error> {
    stage_with(source, file, "")
}

/// [`stage`], with `typedefs` (C text) placed in the generated file after
/// its preamble, before the first assertion.
fn stage_with(source: &str, file: &Path, typedefs: &str) -> Result<Stage, Error> {
    stage_of(source, HEADERS, EXPRESSIONS, file, typedefs)
}

/// [`stage_with`], for a fixture of its own `headers` and `expressions`.
fn stage_of(
    source: &str,
    headers: &[&str],
    expressions: &[(&str, &str)],
    file: &Path,
    typedefs: &str,
) -> Result<Stage, Error> {
    let tokens = tokens_with_strings(source).map_err(|error| Error::SelfTest {
        check: CHECK,
        case: "reading the fixture",
        found: vec![error.to_string()],
    })?;
    let mut found = Declarations::default();
    if let Err(error) = declarations(&tokens, "C", &mut found) {
        return Ok(Stage::Reader(error));
    }
    let headers: Vec<String> = headers.iter().map(|header| (*header).to_owned()).collect();
    let expressions: Vec<(String, String)> = expressions
        .iter()
        .map(|(name, expression)| ((*name).to_owned(), (*expression).to_owned()))
        .collect();
    let c = match c_source(
        "self-test",
        &[],
        &headers,
        &expressions,
        &found,
        DataModel::Lp64,
    ) {
        Ok(c) => c.replacen("_Static_assert", &format!("{typedefs}\n_Static_assert"), 1),
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

/// Writes one line of the self-test's report.
fn report(output: &mut dyn Write, line: impl std::fmt::Display) -> Result<(), Error> {
    writeln!(output, "{line}").map_err(Error::io("report check-abi self-test"))
}

fn failure(case: &'static str, why: String) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: vec![why],
    }
}
