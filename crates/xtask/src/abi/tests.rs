// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the declaration reader and the C it generates.

use super::{
    CType, DataModel, Declarations, Function, Struct, c_source, canonical, declarations, identity,
    missing_facts,
};
use crate::rust_source::tokens_with_strings;

const CLOCK: &str = "use core::ffi::{c_char, c_int, c_long};\n\
                     #[repr(C)]\n#[derive(Debug)]\n\
                     pub(super) struct timespec {\n pub(super) tv_sec: c_long,\n tv_nsec: c_long,\n}\n\
                     pub(super) const CLOCK_BOOTTIME: c_int = 7;\n\
                     const NEGATIVE: c_int = -1;\n\
                     // SAFETY: a comment.\n\
                     #[expect(unsafe_code, reason = \"linux-clock\")]\n\
                     unsafe extern \"C\" {\n\
                     /// Doc.\n\
                     pub(super) fn clock_gettime(clockid: c_int, tp: *mut timespec) -> c_int;\n\
                     pub(super) fn syscall(number: c_long, ...) -> c_long;\n\
                     safe fn strlen(s: *const c_char) -> usize;\n\
                     fn nothing();\n}\n";

fn read(source: &str) -> Declarations {
    let mut found = Declarations::default();
    declarations(&tokens_with_strings(source).unwrap(), "C", &mut found).unwrap();
    found
}

#[test]
fn declarations_read_structs_constants_and_foreign_functions() {
    let found = read(CLOCK);
    assert_eq!(
        found.structs,
        [Struct {
            name: "timespec".into(),
            fields: vec![
                ("tv_sec".into(), CType::Named("c_long".into())),
                ("tv_nsec".into(), CType::Named("c_long".into())),
            ],
        }]
    );
    assert_eq!(
        found.constants,
        [
            ("CLOCK_BOOTTIME".into(), "7".into()),
            ("NEGATIVE".into(), "-1".into())
        ]
    );
    assert_eq!(
        found.functions,
        [
            Function {
                name: "clock_gettime".into(),
                parameters: vec![
                    CType::Named("c_int".into()),
                    CType::Pointer {
                        constant: false,
                        to: Box::new(CType::Named("timespec".into())),
                    },
                ],
                variadic: false,
                returns: Some(CType::Named("c_int".into())),
            },
            Function {
                name: "syscall".into(),
                parameters: vec![CType::Named("c_long".into())],
                variadic: true,
                returns: Some(CType::Named("c_long".into())),
            },
            Function {
                name: "strlen".into(),
                parameters: vec![CType::Pointer {
                    constant: true,
                    to: Box::new(CType::Named("c_char".into())),
                }],
                variadic: false,
                returns: Some(CType::Named("usize".into())),
            },
            Function {
                name: "nothing".into(),
                parameters: Vec::new(),
                variadic: false,
                returns: None,
            },
        ]
    );
}

/// The reader's error for `source`.
#[track_caller]
fn refused(source: &str) -> String {
    let mut found = Declarations::default();
    declarations(&tokens_with_strings(source).unwrap(), "C", &mut found).unwrap_err()
}

#[test]
fn declarations_refuse_what_c_cannot_be_compared_with() {
    for (source, phrase) in [
        (
            "struct timespec { tv_sec: c_long }",
            "line 1: a struct in a declaration file is #[repr(C)]",
        ),
        (
            "#[repr(C, packed)]\nstruct timespec { tv_sec: c_long }",
            "line 2: a struct whose repr is not exactly C",
        ),
        (
            "#[repr(C)]\n#[repr(align(16))]\nstruct timespec { tv_sec: c_long }",
            "line 3: a struct whose repr is not exactly C",
        ),
        (
            "unsafe extern \"system\" { fn close(fd: c_int) -> c_int; }",
            "line 1: an extern block whose ABI is \"system\", not \"C\"",
        ),
        (
            "unsafe extern \"C\" { #[link_name = \"other\"] fn close(fd: c_int) -> c_int; }",
            "line 1: an attribute on a foreign function",
        ),
        (
            "use core::ffi::c_long as c_int;",
            "line 1: a renaming import",
        ),
    ] {
        let error = refused(source);
        assert!(error.starts_with(phrase), "{source}: {error}");
    }
}

#[test]
fn c_source_asserts_layout_member_types_constants_and_function_types() {
    let source = c_source(
        "linux-clock",
        &["time.h".into()],
        &read(CLOCK),
        DataModel::Lp64,
    )
    .unwrap();
    for line in [
        "#include <time.h>",
        "_Static_assert(sizeof(struct timespec) == 16, \"linux-clock: sizeof(struct timespec)\");",
        "_Static_assert(_Alignof(struct timespec) == 8, \"linux-clock: _Alignof(struct timespec)\");",
        "_Static_assert(offsetof(struct timespec, tv_nsec) == 8, \"linux-clock: offsetof(struct timespec, tv_nsec)\");",
        "_Static_assert(_Generic(((struct timespec *)0)->tv_sec, long: 1), \"linux-clock: type of timespec.tv_sec\");",
        "_Static_assert(CLOCK_BOOTTIME == 7, \"linux-clock: CLOCK_BOOTTIME\");",
        "_Static_assert(NEGATIVE == -1, \"linux-clock: NEGATIVE\");",
        "_Static_assert(_Generic(&clock_gettime, int (*)(int, struct timespec *): 1), \"linux-clock: type of clock_gettime\");",
        "_Static_assert(_Generic(&syscall, long (*)(long, ...): 1), \"linux-clock: type of syscall\");",
        "_Static_assert(_Generic(&strlen, size_t (*)(const char *): 1), \"linux-clock: type of strlen\");",
        "_Static_assert(_Generic(&nothing, void (*)(void): 1), \"linux-clock: type of nothing\");",
    ] {
        assert!(
            source.lines().any(|found| found == line),
            "{line}\n{source}"
        );
    }
}

#[test]
fn c_source_lays_out_padding_as_repr_c_does() {
    let found = read(
        "use core::ffi::{c_char, c_int, c_void};\n\
         #[repr(C)]\nstruct padded { a: c_char, b: c_int, c: c_char, d: *mut c_void }\n",
    );
    let source = c_source("row", &[], &found, DataModel::Lp64).unwrap();
    for line in [
        "_Static_assert(offsetof(struct padded, b) == 4, \"row: offsetof(struct padded, b)\");",
        "_Static_assert(offsetof(struct padded, c) == 8, \"row: offsetof(struct padded, c)\");",
        "_Static_assert(offsetof(struct padded, d) == 16, \"row: offsetof(struct padded, d)\");",
        "_Static_assert(sizeof(struct padded) == 24, \"row: sizeof(struct padded)\");",
    ] {
        assert!(
            source.lines().any(|found| found == line),
            "{line}\n{source}"
        );
    }
}

#[test]
fn c_source_takes_long_as_four_bytes_on_windows() {
    let source = c_source(
        "row",
        &[],
        &read("use core::ffi::c_long;\n#[repr(C)]\nstruct pair { a: c_long, b: c_long }\n"),
        DataModel::Llp64,
    )
    .unwrap();
    assert!(
        source.contains("_Static_assert(sizeof(struct pair) == 8,"),
        "{source}"
    );
}

#[test]
fn c_source_resolves_type_names_by_identity_not_spelling() {
    for (source, error) in [
        (
            "#[repr(C)]\nstruct odd { a: String }\n",
            "`String` is neither imported from core::ffi, a primitive integer, nor a struct of \
             the declaration files",
        ),
        (
            "#[repr(C)]\nstruct odd { a: c_long }\n",
            "`c_long` is neither imported from core::ffi, a primitive integer, nor a struct of \
             the declaration files",
        ),
        (
            "#[repr(C)]\nstruct odd { a: libc::c_long }\n",
            "`libc::c_long`: a type path other than core::ffi, whose identity this check \
             cannot prove",
        ),
    ] {
        assert_eq!(
            c_source("row", &[], &read(source), DataModel::Lp64),
            Err(error.to_owned()),
            "{source}"
        );
    }
    let written_out = read("#[repr(C)]\nstruct ok { a: core::ffi::c_long }\n");
    assert!(
        c_source("row", &[], &written_out, DataModel::Lp64)
            .unwrap()
            .contains("_Generic(((struct ok *)0)->a, long: 1)")
    );
}

#[test]
fn declarations_refuse_a_type_alias() {
    assert!(refused("type c_long = u32;").starts_with("line 1: a type alias"));
}

#[test]
fn canonical_resolves_each_file_by_its_own_imports() {
    let importing = read("use core::ffi::c_long;\nconst A: c_long = 1;\n");
    assert!(canonical(&importing, &[]).is_ok());
    let using = read("#[repr(C)]\nstruct s { a: c_long }\n");
    assert_eq!(
        canonical(&using, &["s".to_owned()]).map(|found| found.structs.len()),
        Err(
            "`c_long` is neither imported from core::ffi in its own file, a primitive integer, \
             nor a struct of the declaration files"
                .to_owned()
        )
    );
    let mut imported = read("use core::ffi::c_long;\n#[repr(C)]\nstruct s { a: c_long }\n");
    imported = canonical(&imported, &["s".to_owned()]).unwrap();
    assert_eq!(
        imported.structs.first().map(|item| item.fields.clone()),
        Some(vec![("a".into(), CType::Named("core::ffi::c_long".into()))])
    );
}

#[test]
fn missing_facts_names_a_recorded_constant_or_struct_not_declared() {
    let found = read("use core::ffi::c_int;\nconst EINTR: c_int = 4;\n");
    assert_eq!(
        missing_facts(
            &["timespec".into()],
            &["EINTR".into(), "SYS_getrandom".into()],
            &found
        ),
        ["timespec", "SYS_getrandom"]
    );
}

#[test]
fn identity_names_the_architecture_and_c_library_of_the_target() {
    let musl = identity("aarch64-unknown-linux-musl");
    assert!(musl.contains("#if !defined(__aarch64__)"), "{musl}");
    assert!(
        musl.contains("#if defined(__GLIBC__) || !defined(__DEFINED_ssize_t)"),
        "{musl}"
    );
    let gnu = identity("x86_64-unknown-linux-gnu");
    assert!(gnu.contains("#if !defined(__x86_64__)"), "{gnu}");
    assert!(gnu.contains("#if !defined(__GLIBC__)"), "{gnu}");
}
