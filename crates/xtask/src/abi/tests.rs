// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the declaration reader and the C it generates.

use super::{
    CType, DataModel, Declarations, Function, OPAQUE_TAILS, OpaqueTail, Struct, c_source,
    canonical, declarations, identity, missing_facts,
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
        &[],
        &["time.h".into()],
        &[],
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
    let source = c_source("row", &[], &[], &[], &found, DataModel::Lp64).unwrap();
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
        &[],
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
            "`String` is neither imported from core::ffi, a primitive integer, nor a struct or type alias of \
             the declaration files",
        ),
        (
            "#[repr(C)]\nstruct odd { a: c_long }\n",
            "`c_long` is neither imported from core::ffi, a primitive integer, nor a struct or type alias of \
             the declaration files",
        ),
        (
            "#[repr(C)]\nstruct odd { a: libc::c_long }\n",
            "`libc::c_long`: a type path other than core::ffi, whose identity this check \
             cannot prove",
        ),
    ] {
        assert_eq!(
            c_source("row", &[], &[], &[], &read(source), DataModel::Lp64),
            Err(error.to_owned()),
            "{source}"
        );
    }
    let written_out = read("#[repr(C)]\nstruct ok { a: core::ffi::c_long }\n");
    assert!(
        c_source("row", &[], &[], &[], &written_out, DataModel::Lp64)
            .unwrap()
            .contains("_Generic(((struct ok *)0)->a, long: 1)")
    );
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
        assert_eq!(
            refused(source),
            "line 1: a type alias that is not `NAME = <C scalar>;`",
            "{source}"
        );
    }
}

#[test]
fn c_source_reads_an_alias_of_a_core_ffi_type_by_its_identity() {
    let unsigned = c_source(
        "row",
        &[],
        &[],
        &[],
        &read("type rlim_t = core::ffi::c_ulong;"),
        DataModel::Lp64,
    )
    .unwrap();
    assert!(
        unsigned
            .lines()
            .any(|line| line == "_Static_assert((rlim_t)-1 > 0, \"row: rlim_t is unsigned\");"),
        "{unsigned}"
    );
    let not_imported = c_source(
        "row",
        &[],
        &[],
        &[],
        &read("type rlim_t = c_ulong;"),
        DataModel::Lp64,
    );
    assert_eq!(
        not_imported.unwrap_err(),
        "`c_ulong` is neither imported from core::ffi, a primitive integer, nor a struct or type \
         alias of the declaration files"
    );
}

#[test]
fn c_source_names_an_alias_by_its_c_typedef_and_asserts_its_size_alignment_and_sign() {
    let source = c_source(
        "unix-nodump",
        &[],
        &["sys/resource.h".into()],
        &[],
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
    let source = c_source(
        "row",
        &[],
        &[],
        &[],
        &read("type ssize_t = isize;"),
        DataModel::Lp64,
    )
    .unwrap();
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
    let source = c_source(
        "row",
        &[],
        &[],
        &[],
        &Declarations::default(),
        DataModel::Lp64,
    )
    .unwrap();
    assert!(source.contains("#define _DEFAULT_SOURCE 1\n"), "{source}");
    assert!(!source.contains("_GNU_SOURCE"), "{source}");
}

#[test]
fn c_source_defines_a_row_s_feature_macros_before_any_include() {
    // glibc and musl declare `struct ucred` only under _GNU_SOURCE.
    let source = c_source(
        "linux-peercred",
        &["_GNU_SOURCE".to_owned()],
        &["sys/socket.h".to_owned()],
        &[],
        &Declarations::default(),
        DataModel::Lp64,
    )
    .unwrap();
    let define = source.find("#define _GNU_SOURCE 1\n").expect(&source);
    assert!(define < source.find("#include").unwrap(), "{source}");
}

#[test]
fn c_source_asserts_a_constant_named_by_an_expression_against_that_expression() {
    // CMSG_LEN and CMSG_SPACE are macros: a constant can only be compared
    // with what the macro gives, by the expression sites.json maps it to.
    let source = c_source(
        "linux-fdpass",
        &[],
        &["sys/socket.h".to_owned()],
        &[(
            "CMSG_LEN_ONE".to_owned(),
            "CMSG_LEN(sizeof(int))".to_owned(),
        )],
        &read("const CMSG_LEN_ONE: usize = 20;\nconst SOL_SOCKET: c_int = 1;"),
        DataModel::Lp64,
    )
    .unwrap();
    for line in [
        "_Static_assert((CMSG_LEN(sizeof(int))) == 20, \"linux-fdpass: CMSG_LEN_ONE\");",
        "_Static_assert(SOL_SOCKET == 1, \"linux-fdpass: SOL_SOCKET\");",
    ] {
        assert!(
            source.lines().any(|found| found == line),
            "{line}\n{source}"
        );
    }
}

#[test]
fn c_source_refuses_an_expression_for_no_constant() {
    let error = c_source(
        "row",
        &[],
        &[],
        &[("MISSING".to_owned(), "sizeof(int)".to_owned())],
        &read("const PRESENT: c_int = 1;"),
        DataModel::Lp64,
    )
    .unwrap_err();
    assert_eq!(
        error,
        "sites.json maps `MISSING` to a C expression, but the declarations hold no such constant"
    );
}

#[test]
fn declarations_refuse_a_type_alias_named_as_a_scalar() {
    for source in ["type c_long = u32;", "type u32 = u64;", "type c_void = u8;"] {
        assert_eq!(
            refused(source),
            "line 1: a type alias named as a scalar, under which a name would not mean what it \
             says",
            "{source}"
        );
    }
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
             nor a struct or type alias of the declaration files"
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

#[test]
fn c_source_asserts_an_array_member_by_its_element_type_and_count() {
    let found = read(
        "use core::ffi::{c_char, c_ushort};\n\
         #[repr(C)]\nstruct in6_addr { s6_addr32: [u32; 4] }\n\
         #[repr(C)]\nstruct sockaddr { sa_family: c_ushort, sa_data: [c_char; 14] }\n\
         #[repr(C)]\nstruct holder { family: c_ushort, address: in6_addr }\n",
    );
    let source = c_source("row", &[], &[], &[], &found, DataModel::Lp64).unwrap();
    for line in [
        "_Static_assert(sizeof(struct in6_addr) == 16, \"row: sizeof(struct in6_addr)\");",
        "_Static_assert(_Alignof(struct in6_addr) == 4, \"row: _Alignof(struct in6_addr)\");",
        "_Static_assert(_Generic(&((struct in6_addr *)0)->s6_addr32, uint32_t (*)[4]: 1), \"row: type of in6_addr.s6_addr32\");",
        "_Static_assert(sizeof(struct sockaddr) == 16, \"row: sizeof(struct sockaddr)\");",
        "_Static_assert(offsetof(struct sockaddr, sa_data) == 2, \"row: offsetof(struct sockaddr, sa_data)\");",
        "_Static_assert(_Generic(&((struct sockaddr *)0)->sa_data, char (*)[14]: 1), \"row: type of sockaddr.sa_data\");",
        "_Static_assert(offsetof(struct holder, address) == 4, \"row: offsetof(struct holder, address)\");",
        "_Static_assert(sizeof(struct holder) == 20, \"row: sizeof(struct holder)\");",
    ] {
        assert!(
            source.lines().any(|found| found == line),
            "{line}\n{source}"
        );
    }
}

/// The tail every case below approves: `s`'s `_opaque`, two `c_int`s
/// after `a`.
const TAIL: OpaqueTail = OpaqueTail {
    row: "row",
    structure: "s",
    field: "_opaque",
    after: "a",
    element: "c_int",
    count: 2,
};

/// The declarations of `source`, with `tails` the opaque tails xtask's
/// allowlist approves for its row.
fn read_with_opaque(source: &str, tails: &[OpaqueTail]) -> Declarations {
    let mut found = read(source);
    found.opaque = tails.to_vec();
    found
}

#[test]
fn c_source_binds_an_opaque_tail_by_where_the_c_member_before_it_ends_and_its_length() {
    let tail = OpaqueTail {
        structure: "statvfs",
        field: "_opaque_tail",
        after: "f_namemax",
        count: 6,
        ..TAIL
    };
    let found = read_with_opaque(
        "use core::ffi::{c_int, c_ulong};\n\
         #[repr(C)]\nstruct statvfs { f_bsize: c_ulong, f_namemax: c_ulong, _opaque_tail: [c_int; 6] }\n",
        &[tail],
    );
    let source = c_source("row", &[], &[], &[], &found, DataModel::Lp64).unwrap();
    for line in [
        "_Static_assert(sizeof(struct statvfs) == 40, \"row: sizeof(struct statvfs)\");",
        "_Static_assert(offsetof(struct statvfs, f_namemax) == 8, \"row: offsetof(struct statvfs, f_namemax)\");",
        "_Static_assert(offsetof(struct statvfs, f_namemax) + sizeof(((struct statvfs *)0)->f_namemax) == 16, \"row: the start of statvfs's opaque tail\");",
        "_Static_assert(sizeof(struct statvfs) - 16 == 24, \"row: the length of statvfs's opaque tail\");",
    ] {
        assert!(
            source.lines().any(|found| found == line),
            "{line}\n{source}"
        );
    }
    assert!(!source.contains("_opaque_tail"), "{source}");
}

#[test]
fn c_source_refuses_an_opaque_field_the_allowlist_does_not_approve_as_written() {
    let approved = "use core::ffi::{c_int, c_ulong};\n#[repr(C)]\nstruct s { b: c_ulong, a: c_ulong, _opaque: [c_int; 2] }\n";
    for (source, tails, error) in [
        (
            // A live member renamed opaque, which the allowlist does not name.
            approved,
            &[][..],
            "struct s: `_opaque` is opaque, but xtask's allowlist of opaque tails does not approve it",
        ),
        (
            // The member before the tail removed and the tail grown over it:
            // size, alignment and the other offsets unchanged.
            "use core::ffi::{c_int, c_ulong};\n#[repr(C)]\nstruct s { b: c_ulong, _opaque: [c_int; 4] }\n",
            &[TAIL][..],
            "struct s: the opaque tail `_opaque` is the last field, after `a`, and 2 of c_int, as xtask's allowlist approves it",
        ),
        (
            "use core::ffi::{c_int, c_ulong};\n#[repr(C)]\nstruct s { b: c_ulong, a: c_ulong, _opaque: [c_int; 3] }\n",
            &[TAIL][..],
            "struct s: the opaque tail `_opaque` is the last field, after `a`, and 2 of c_int, as xtask's allowlist approves it",
        ),
        (
            "use core::ffi::{c_uint, c_ulong};\n#[repr(C)]\nstruct s { b: c_ulong, a: c_ulong, _opaque: [c_uint; 2] }\n",
            &[TAIL][..],
            "struct s: the opaque tail `_opaque` is the last field, after `a`, and 2 of c_int, as xtask's allowlist approves it",
        ),
        (
            "use core::ffi::{c_int, c_ulong};\n#[repr(C)]\nstruct s { _opaque: [c_int; 2], a: c_ulong }\n",
            &[TAIL][..],
            "struct s: the opaque tail `_opaque` is the last field, after `a`, and 2 of c_int, as xtask's allowlist approves it",
        ),
        (
            // A struct whose every member is opaque checks no member.
            "use core::ffi::c_int;\n#[repr(C)]\nstruct s { _opaque: [c_int; 2] }\n",
            &[TAIL][..],
            "struct s: every member is opaque, so no member would be checked",
        ),
        (
            approved,
            &[
                TAIL,
                OpaqueTail {
                    structure: "t",
                    ..TAIL
                },
            ][..],
            "xtask's allowlist approves the opaque tail `t._opaque`, which the declarations do not hold",
        ),
    ] {
        assert_eq!(
            c_source(
                "row",
                &[],
                &[],
                &[],
                &read_with_opaque(source, tails),
                DataModel::Lp64
            )
            .unwrap_err(),
            error
        );
    }
}

#[test]
fn the_allowlist_approves_only_statvfs_s_tail_after_f_namemax() {
    assert_eq!(
        OPAQUE_TAILS,
        [OpaqueTail {
            row: "linux-statvfs",
            structure: "statvfs",
            field: "_opaque_tail",
            after: "f_namemax",
            element: "c_int",
            count: 6,
        }]
    );
}

#[test]
fn declarations_refuse_an_opaque_field_others_can_reach() {
    assert_eq!(
        refused(
            "use core::ffi::c_int;\n#[repr(C)]\nstruct s { a: c_int, pub(super) _opaque: [c_int; 2] }\n"
        ),
        "line 3: an opaque field is private, so no code outside its struct's module reads it"
    );
}

#[test]
fn c_source_refuses_an_array_parameter_and_a_zero_length_array() {
    let parameter = read(
        "use core::ffi::c_int;\n\
         unsafe extern \"C\" { fn f(a: [c_int; 2]) -> c_int; }\n",
    );
    assert_eq!(
        c_source("row", &[], &[], &[], &parameter, DataModel::Lp64).unwrap_err(),
        "an array is a struct member only, never a parameter or return type"
    );
    let empty = read("use core::ffi::c_int;\n#[repr(C)]\nstruct s { a: c_int, b: [c_int; 0] }\n");
    assert_eq!(
        c_source("row", &[], &[], &[], &empty, DataModel::Lp64).unwrap_err(),
        "an array of no elements, which C11 does not allow"
    );
}
