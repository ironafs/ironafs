// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the declaration reader and the C it generates.

use super::{CType, DataModel, Declarations, Function, Struct, c_source, declarations};
use crate::rust_source::tokens;

const CLOCK: &str = "use core::ffi::{c_int, c_long};\n\
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
    declarations(&tokens(source).unwrap(), &mut found).unwrap();
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

#[test]
fn declarations_refuse_a_struct_without_repr_c() {
    let mut found = Declarations::default();
    let error = declarations(
        &tokens("struct timespec { tv_sec: c_long }").unwrap(),
        &mut found,
    );
    assert!(error.is_err());
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
    let found =
        read("#[repr(C)]\nstruct padded { a: c_char, b: c_int, c: c_char, d: *mut c_void }\n");
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
        &read("#[repr(C)]\nstruct pair { a: c_long, b: c_long }\n"),
        DataModel::Llp64,
    )
    .unwrap();
    assert!(
        source.contains("_Static_assert(sizeof(struct pair) == 8,"),
        "{source}"
    );
}

#[test]
fn c_source_refuses_a_type_it_has_no_c_name_for() {
    let found = read("#[repr(C)]\nstruct odd { a: String }\n");
    assert!(c_source("row", &[], &found, DataModel::Lp64).is_err());
}
