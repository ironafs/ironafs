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

const RLIMIT: &str = "use core::ffi::c_int;\n\
                      pub(super) type rlim_t = u64;\n\
                      #[repr(C)]\n\
                      pub(super) struct rlimit {\n pub(super) rlim_cur: rlim_t,\n pub(super) rlim_max: rlim_t,\n}\n\
                      pub(super) const RLIMIT_CORE: c_int = 4;\n\
                      unsafe extern \"C\" {\n\
                      pub(super) fn setrlimit(resource: c_int, rlim: *const rlimit) -> c_int;\n}\n";

#[test]
fn declarations_read_a_type_alias_of_a_scalar() {
    let found = read(RLIMIT);
    assert_eq!(
        found.aliases,
        [("rlim_t".to_owned(), CType::Named("u64".into()))]
    );
    assert_eq!(
        found.structs,
        [Struct {
            name: "rlimit".into(),
            fields: vec![
                ("rlim_cur".into(), CType::Named("rlim_t".into())),
                ("rlim_max".into(), CType::Named("rlim_t".into())),
            ],
        }]
    );
}

#[test]
fn declarations_refuse_an_alias_of_anything_but_a_scalar() {
    for source in [
        "type handle = *mut c_void;",
        "type stamp = timespec;",
        "type pair = (u8, u8);",
    ] {
        let mut found = Declarations::default();
        assert!(
            declarations(&tokens(source).unwrap(), &mut found).is_err(),
            "{source}"
        );
    }
}

#[test]
fn c_source_names_an_alias_by_its_c_typedef_and_asserts_its_size_alignment_and_sign() {
    let source = c_source(
        "unix-nodump",
        &["sys/resource.h".into()],
        &read(RLIMIT),
        DataModel::Lp64,
    )
    .unwrap();
    for line in [
        "_Static_assert(sizeof(rlim_t) == 8, \"unix-nodump: sizeof(rlim_t)\");",
        "_Static_assert(_Alignof(rlim_t) == 8, \"unix-nodump: _Alignof(rlim_t)\");",
        "_Static_assert((rlim_t)-1 > 0, \"unix-nodump: rlim_t is unsigned\");",
        "_Static_assert(sizeof(struct rlimit) == 16, \"unix-nodump: sizeof(struct rlimit)\");",
        "_Static_assert(_Generic(((struct rlimit *)0)->rlim_max, rlim_t: 1), \"unix-nodump: type of rlimit.rlim_max\");",
        "_Static_assert(_Generic(&setrlimit, int (*)(int, const struct rlimit *): 1), \"unix-nodump: type of setrlimit\");",
    ] {
        assert!(
            source.lines().any(|found| found == line),
            "{line}\n{source}"
        );
    }
}

#[test]
fn c_source_asserts_a_signed_alias_is_signed() {
    let source = c_source("row", &[], &read("type ssize_t = isize;"), DataModel::Lp64).unwrap();
    assert!(
        source
            .lines()
            .any(|line| line == "_Static_assert((ssize_t)-1 < 0, \"row: ssize_t is signed\");"),
        "{source}"
    );
}

#[test]
fn c_source_asks_for_the_posix_prototypes_not_the_gnu_ones() {
    // Under _GNU_SOURCE glibc declares setrlimit's resource as an enum,
    // whose compatible type is not `int`; POSIX and musl declare `int`.
    let source = c_source("row", &[], &Declarations::default(), DataModel::Lp64).unwrap();
    assert!(source.contains("#define _DEFAULT_SOURCE 1\n"), "{source}");
    assert!(!source.contains("_GNU_SOURCE"), "{source}");
}
