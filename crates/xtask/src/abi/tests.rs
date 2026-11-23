// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the declaration reader and the C it generates.

use super::{
    CType, DataModel, Declarations, Function, Recorded, Struct, c_source, canonical, declarations,
    fact_differences, identity, object_macros, preamble, required_constants, scan_defines,
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
        (
            "mod core { pub mod ffi { pub type c_int = u8; } }",
            "line 1: an item, import or generic parameter named `core`",
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
    let none = |name: &str| Err(format!("`{name}` unbound"));
    let row_s = |name: &str| {
        if name == "s" {
            Ok(())
        } else {
            Err(format!("`{name}` unbound"))
        }
    };
    let importing = read("use core::ffi::c_long;\nconst A: c_long = 1;\n");
    assert!(canonical(&importing, &none).is_ok());
    let using = read("#[repr(C)]\nstruct s { a: c_long }\n");
    assert_eq!(
        canonical(&using, &row_s).map(|found| found.structs.len()),
        Err("`c_long` unbound".to_owned())
    );
    let mut imported = read("use core::ffi::c_long;\n#[repr(C)]\nstruct s { a: c_long }\n");
    imported = canonical(&imported, &row_s).unwrap();
    assert_eq!(
        imported.structs.first().map(|item| item.fields.clone()),
        Some(vec![("a".into(), CType::Named("core::ffi::c_long".into()))])
    );
}

#[test]
fn declarations_read_imports_of_the_shipped_shapes_with_their_gates() {
    let found = read(
        "use core::{ffi::c_int};\nuse std::io;\nuse super::{iovec, socklen_t};\n\
         #[cfg(target_env = \"gnu\")]\npub(super) use gnu::{cmsghdr, msghdr};\n\
         #[cfg(target_arch = \"x86_64\")]\npub(super) use x86_64::SYS_getrandom;\n\
         #[cfg(not(miri))]\npub(super) use foreign::recvmsg;\n\
         #[cfg(miri)]\npub(super) use stand_in::recvmsg;\n",
    );
    assert_eq!(found.ffi_names, ["c_int".to_owned()]);
    let gates: Vec<(String, super::Gate)> = found
        .imports
        .iter()
        .map(|import| (import.path.join("::"), import.gate.clone()))
        .collect();
    assert_eq!(
        gates,
        [
            ("core::ffi::c_int".to_owned(), super::Gate::Always),
            ("std::io".to_owned(), super::Gate::Always),
            ("super::iovec".to_owned(), super::Gate::Always),
            ("super::socklen_t".to_owned(), super::Gate::Always),
            ("gnu::cmsghdr".to_owned(), super::Gate::Env("gnu".into())),
            ("gnu::msghdr".to_owned(), super::Gate::Env("gnu".into())),
            (
                "x86_64::SYS_getrandom".to_owned(),
                super::Gate::Arch("x86_64".into())
            ),
            ("foreign::recvmsg".to_owned(), super::Gate::Miri(false)),
            ("stand_in::recvmsg".to_owned(), super::Gate::Miri(true)),
        ]
    );
    let env = super::Gate::Env("musl".into());
    assert!(env.in_effect("aarch64-unknown-linux-musl"));
    assert!(!env.in_effect("aarch64-unknown-linux-gnu"));
    assert!(super::Gate::Arch("x86_64".into()).in_effect("x86_64-unknown-linux-musl"));
    assert!(!super::Gate::Miri(true).in_effect("x86_64-unknown-linux-gnu"));
}

#[test]
fn declarations_refuse_imports_whose_binding_is_not_established_structurally() {
    for (source, says) in [
        ("use super::*;\n", "a glob import"),
        ("use super::{a, b\n", "cannot read"),
        ("fn f() { use super::msghdr; }\n", "not at file scope"),
        (
            "mod inner {\n    pub(super) use super::msghdr;\n}\n",
            "an inline module",
        ),
        ("use crate::elsewhere::msghdr;\n", "a path (crate::"),
        ("use self::inner::msghdr;\n", "a path (crate::"),
        ("use gnu::msghdr;\n", "a path (crate::"),
        (
            "#[cfg(unix)]\nuse super::msghdr;\n",
            "an import under #[cfg(unix)]",
        ),
        (
            "#[cfg(target_env = \"gnu\")]\npub(super) use musl::msghdr;\n",
            "an import under #[cfg(target_env=\"gnu\")]",
        ),
        (
            "#[cfg(target_env = \"gnu\")]\n#[allow(unused)]\npub(super) use gnu::msghdr;\n",
            "attributes this check does not read",
        ),
    ] {
        let error = refused(source);
        assert!(error.contains(says), "{source}: {error}");
    }
}

/// A row `row` whose declaration files are `files` (path under
/// `src/row/`, text), all read by the compiler unless listed in `unread`:
/// what `row_declarations` makes of them.
fn row_of(
    name: &str,
    files: &[(&str, &str)],
    unread: &[&str],
    target: &str,
) -> Result<Declarations, String> {
    let root = std::env::temp_dir().join(format!(
        "ironafs-xtask-abi-row-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let mut read = std::collections::BTreeSet::new();
    for (path, text) in files {
        let file = std::path::Path::new("src").join("row").join(path);
        std::fs::create_dir_all(root.join(&file).parent().unwrap()).unwrap();
        std::fs::write(root.join(&file), text).unwrap();
        if !unread.contains(path) {
            read.insert(file);
        }
    }
    let found = super::row_declarations(&root, "row", &read, target).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    found
}

/// The row's own `msghdr`, declared in each C library's file and
/// re-exported by `abi.rs` under that library's cfg, as the shipped rows
/// write it.
const ABI: &str = "use core::ffi::c_int;\n\
                   #[cfg(target_env = \"gnu\")]\npub(super) use gnu::msghdr;\n\
                   #[cfg(target_env = \"musl\")]\npub(super) use musl::msghdr;\n";
const GNU: &str = "use core::ffi::c_int;\n#[repr(C)]\npub(crate) struct msghdr { a: c_int }\n";
const FOREIGN: &str = "use core::ffi::c_int;\nuse super::msghdr;\n\
                       unsafe extern \"C\" {\n    fn recvmsg(m: *mut msghdr) -> c_int;\n}\n";
const GLIBC: &str = "aarch64-unknown-linux-gnu";
const MUSL: &str = "aarch64-unknown-linux-musl";

/// The diagnostic prefix for `file` under `src/row/`, built as
/// `row_declarations` builds it.
fn at_file(file: &str) -> String {
    let path = std::path::Path::new("src").join("row").join(file);
    format!("{}: ", path.display())
}

#[test]
fn row_declarations_follow_each_import_to_the_row_s_own_declaration() {
    for (target, unread) in [(GLIBC, "abi/musl.rs"), (MUSL, "abi/gnu.rs")] {
        let found = row_of(
            "clean",
            &[
                ("abi.rs", ABI),
                ("abi/gnu.rs", GNU),
                ("abi/musl.rs", GNU),
                ("abi/foreign.rs", FOREIGN),
            ],
            &[unread],
            target,
        )
        .unwrap();
        assert_eq!(found.functions.len(), 1, "{target}");
        assert_eq!(found.structs.len(), 1, "{target}");
    }
}

#[test]
fn row_declarations_decide_which_re_export_is_in_effect_from_the_target() {
    // Only the musl re-export, on a glibc target, with musl's file
    // compiled: the re-export is not in effect, so the name is unbound,
    // though its destination file was read.
    let musl_only = "use core::ffi::c_int;\n\
                     #[cfg(target_env = \"musl\")]\npub(super) use musl::msghdr;\n";
    let error = row_of(
        "inactive",
        &[
            ("abi.rs", musl_only),
            ("abi/musl.rs", GNU),
            ("abi/foreign.rs", FOREIGN),
        ],
        &[],
        GLIBC,
    )
    .unwrap_err();
    let prefix = at_file("abi/foreign.rs");
    assert!(error.starts_with(&prefix), "{error}");
    assert!(error.contains("`msghdr` is neither imported"), "{error}");
    // In effect, but its file was not read: refused, not skipped.
    let error = row_of(
        "unread",
        &[
            ("abi.rs", ABI),
            ("abi/gnu.rs", GNU),
            ("abi/foreign.rs", FOREIGN),
        ],
        &["abi/gnu.rs"],
        GLIBC,
    )
    .unwrap_err();
    assert!(
        error.contains("outside the row's declaration files"),
        "{error}"
    );
}

#[test]
fn row_declarations_refuse_a_same_named_type_imported_from_elsewhere() {
    // The row's msghdr is still declared, but the foreign function's file
    // takes its msghdr from another module, or from none.
    for (name, import, says) in [
        ("crate", "use crate::elsewhere::msghdr;", "a path (crate::"),
        (
            "std",
            "use std::ffi::msghdr;",
            "`msghdr` is imported from std::ffi::msghdr",
        ),
        ("unimported", "", "`msghdr` is neither imported"),
        (
            "twice",
            "use super::msghdr;\nuse super::{msghdr};",
            "more than one import",
        ),
    ] {
        let foreign = format!(
            "use core::ffi::c_int;\n{import}\n\
             unsafe extern \"C\" {{\n    fn recvmsg(m: *mut msghdr) -> c_int;\n}}\n"
        );
        let error = row_of(
            name,
            &[
                ("abi.rs", ABI),
                ("abi/gnu.rs", GNU),
                ("abi/musl.rs", GNU),
                ("abi/foreign.rs", &foreign),
            ],
            &["abi/musl.rs"],
            GLIBC,
        )
        .unwrap_err();
        assert!(
            error.starts_with(&at_file("abi/foreign.rs")),
            "{name}: {error}"
        );
        assert!(error.contains(says), "{name}: {error}");
    }
}

#[test]
fn fact_differences_name_each_recorded_fact_not_declared_and_each_declared_not_recorded() {
    let found = read("use core::ffi::c_int;\nconst EINTR: c_int = 4;\ntype rlim_t = u64;\n");
    let structs = ["timespec".to_owned()];
    let constants = ["EINTR".to_owned(), "SYS_getrandom".to_owned()];
    let recorded = |aliases| Recorded {
        structs: &structs,
        constants: &constants,
        aliases,
    };
    let rlim = ["rlim_t".to_owned()];
    assert_eq!(
        fact_differences(recorded(&rlim), &found),
        (vec!["timespec".into(), "SYS_getrandom".into()], Vec::new())
    );
    // An alias declared but not recorded, and one recorded but not
    // declared.
    assert_eq!(
        fact_differences(recorded(&[]), &found),
        (
            vec!["timespec".into(), "SYS_getrandom".into()],
            vec!["rlim_t".into()]
        )
    );
    let socklen = ["rlim_t".to_owned(), "socklen_t".to_owned()];
    assert_eq!(
        fact_differences(recorded(&socklen), &found).0,
        ["timespec", "SYS_getrandom", "socklen_t"]
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
fn constants_are_emitted_as_normalized_decimal_values() {
    let found = read(
        "const LEADING: c_int = 046;\nconst SPLIT: c_int = 0_1_0;\nconst NEG: c_int = -007;\n",
    );
    assert_eq!(
        found.constants,
        [
            ("LEADING".to_owned(), "46".to_owned()),
            ("SPLIT".to_owned(), "10".to_owned()),
            ("NEG".to_owned(), "-7".to_owned()),
        ]
    );
}

#[test]
fn object_macros_reads_the_preprocessor_s_definitions() {
    let dump = "#define EINTR 4\n#define _GNU_SOURCE 1\n#define FD_SET(fd, set) x\n\
                #define __x86_64__ 1\n";
    let found = object_macros(dump);
    assert!(found.contains("EINTR") && found.contains("__x86_64__"));
    assert!(!found.contains("FD_SET"));
}

#[test]
fn required_constants_are_the_c_macros_the_row_s_code_names() {
    let macros =
        object_macros("#define EINTR 4\n#define EPERM 1\n#define unix 1\n#define errno x\n");
    // EINTR is used by the code and is a C macro; FLAGS is used but is no
    // C macro; unix and errno have no upper-case letter, so a Rust `cfg`
    // word or a variable is never taken for one.
    let used = ["EINTR", "FLAGS", "unix", "errno", "c_int"];
    assert_eq!(required_constants(used.iter().copied(), &macros), ["EINTR"]);
}

#[test]
fn the_macro_scan_reads_every_definition_the_assertions_read_and_gnu_s() {
    let headers = ["time.h".to_owned()];
    for defines in [vec![], vec!["_GNU_SOURCE".to_owned()]] {
        let start = preamble("linux-clock", &defines, &headers, DataModel::Lp64);
        assert!(start.contains("#define _DEFAULT_SOURCE 1\n"));
        let source = c_source(
            "linux-clock",
            &defines,
            &headers,
            &[],
            &read(CLOCK),
            DataModel::Lp64,
        )
        .unwrap();
        assert!(source.starts_with(&start));
        // The scan's preamble holds every line of the assertions' and
        // defines `_GNU_SOURCE` once, so the macros the assertions see are
        // among those the scan sees.
        let scanned = preamble(
            "linux-clock",
            &scan_defines(&defines),
            &headers,
            DataModel::Lp64,
        );
        assert!(
            start
                .lines()
                .all(|line| scanned.lines().any(|seen| seen == line)),
            "{scanned}"
        );
        assert_eq!(
            scanned
                .lines()
                .filter(|line| *line == "#define _GNU_SOURCE 1")
                .count(),
            1,
            "{scanned}"
        );
    }
}

/// A crate directory holding `src/<module>.rs` with `text`, the files the
/// compiler read for the row, and the directory for the generated C.
#[cfg(target_os = "linux")]
fn row_files(
    name: &str,
    module: &str,
    text: &str,
) -> (
    std::path::PathBuf,
    std::collections::BTreeSet<std::path::PathBuf>,
) {
    let root =
        std::env::temp_dir().join(format!("ironafs-xtask-abi-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    let file = std::path::Path::new("src").join(format!("{module}.rs"));
    std::fs::write(root.join(&file), text).unwrap();
    (root, [file].into_iter().collect())
}

/// A row's code with these `headers` and `defines`, and no recorded facts.
#[cfg(target_os = "linux")]
fn row_code(module: &str, headers: &[&str], defines: &[&str]) -> crate::os::inventory::Code {
    crate::os::inventory::Code {
        module: module.to_owned(),
        headers: headers.iter().map(|header| (*header).to_owned()).collect(),
        defines: defines.iter().map(|define| (*define).to_owned()).collect(),
        expressions: Vec::new(),
        qualified: Vec::new(),
        extern_blocks: Vec::new(),
        blocks: Vec::new(),
        impls: 0,
        structs: Vec::new(),
        constants: Vec::new(),
        aliases: Vec::new(),
        stand_in: None,
    }
}

/// A row whose code names `O_PATH`, which glibc's `<fcntl.h>` defines only
/// under `_GNU_SOURCE`, and declares nothing: the fact is missing whether or
/// not the row opts in to `_GNU_SOURCE` itself.
#[cfg(target_os = "linux")]
#[test]
fn fact_problem_finds_a_gnu_only_macro_the_code_names_with_or_without_the_row_s_opt_in() {
    let (root, compiled) = row_files("gnu-macro", "row", "const PATH: c_int = O_PATH;\n");
    for defines in [&[][..], &["_GNU_SOURCE"][..]] {
        let code = row_code("row", &["fcntl.h"], defines);
        let found = super::fact_problem(
            "row",
            &code,
            &Declarations::default(),
            &root,
            &compiled,
            DataModel::Lp64,
            &root,
        )
        .unwrap();
        assert!(
            found.as_deref().is_some_and(
                |problem| problem.starts_with("the row's code names O_PATH, a C macro")
            ),
            "{defines:?}: {found:?}"
        );
    }
    std::fs::remove_dir_all(&root).unwrap();
}

/// `CLOCK_BOOTTIME`, which glibc's `<time.h>` defines under
/// `_DEFAULT_SOURCE`: the scan requires it of a row that names it, and the
/// assertions' preamble, without `_GNU_SOURCE`, compiles the assertion of
/// its value.
#[cfg(target_os = "linux")]
#[test]
fn clock_boottime_is_a_required_fact_and_asserted_under_the_default_source() {
    let (root, compiled) = row_files("boottime", "clock", "const BOOT: c_int = CLOCK_BOOTTIME;\n");
    let code = row_code("clock", &["time.h"], &[]);
    let found = super::fact_problem(
        "clock",
        &code,
        &Declarations::default(),
        &root,
        &compiled,
        DataModel::Lp64,
        &root,
    )
    .unwrap();
    assert!(
        found.as_deref().is_some_and(
            |problem| problem.starts_with("the row's code names CLOCK_BOOTTIME, a C macro")
        ),
        "{found:?}"
    );
    let declared = read("use core::ffi::c_int;\nconst CLOCK_BOOTTIME: c_int = 7;\n");
    let source = c_source(
        "clock",
        &[],
        &["time.h".to_owned()],
        &[],
        &declared,
        DataModel::Lp64,
    )
    .unwrap();
    assert!(!source.contains("_GNU_SOURCE"), "{source}");
    assert_eq!(
        super::compile(&source, &root.join("boottime.c")).unwrap(),
        Ok(())
    );
    std::fs::remove_dir_all(&root).unwrap();
}
