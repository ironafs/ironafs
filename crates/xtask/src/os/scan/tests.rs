// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the site reader of `check-os`.

use super::{
    BLOCK_BODY, CFG, EXTERN_FORM, EXTERN_ITEM, ExternItem, HIDDEN, LIBRARY_SITE, LINK, OUTSIDE,
    Place, Scan, Site, SiteKind, TARGET_ENV, TWO_NAMES, UNSAFE_ITEM, outside_crate, scan,
};
use crate::rust_source::tokens_with_strings;

fn read(source: &str, root: bool) -> Scan {
    read_at(source, if root { Place::Root } else { Place::Other })
}

fn read_at(source: &str, place: Place) -> Scan {
    scan(&tokens_with_strings(source).unwrap(), place)
}

fn whats(scan: &Scan) -> Vec<&'static str> {
    scan.problems.iter().map(|problem| problem.what).collect()
}

fn problems(source: &str) -> Vec<&'static str> {
    read(source, false)
        .problems
        .into_iter()
        .map(|problem| problem.what)
        .collect()
}

const CLOCK: &str = r#"
use core::ffi::c_int;

/// The declaration.
// SAFETY: written from clock_gettime(2).
#[expect(unsafe_code, reason = "linux-clock")]
unsafe extern "C" {
    /// Reads a clock.
    fn clock_gettime(clockid: c_int, tp: *mut timespec) -> c_int;
    pub(super) safe fn other() -> c_int;
}

fn read(id: c_int, time: &mut timespec) -> c_int {
    #[expect(unsafe_code, reason = "linux-clock")]
    // SAFETY: the pointer is exclusive for the call.
    let status = unsafe { clock_gettime(id, time) };
    status
}

#[cfg(test)]
mod tests;
"#;

#[test]
fn scan_reads_a_named_extern_block_and_block_with_their_items_and_operation() {
    let found = read(CLOCK, false);
    assert_eq!(found.problems, []);
    assert_eq!(
        found.sites,
        [
            Site {
                id: Some("linux-clock".into()),
                line: 7,
                kind: SiteKind::Extern {
                    link: None,
                    items: vec![
                        ExternItem {
                            name: "clock_gettime".into(),
                            safe: false,
                        },
                        ExternItem {
                            name: "other".into(),
                            safe: true,
                        },
                    ],
                },
            },
            Site {
                id: Some("linux-clock".into()),
                line: 16,
                kind: SiteKind::Block {
                    operation: "clock_gettime".into(),
                    arguments: vec!["id".into(), "time".into()],
                },
            },
        ]
    );
}

#[test]
fn scan_refuses_a_block_with_anything_but_one_plain_call() {
    for body in [
        "clock_gettime(id, time); clock_gettime(id, time)",
        "clock_gettime(id, other(time))",
        "clock_gettime(id, *time)",
        "clock_gettime(id, time.field)",
        "clock_gettime(id, time[0])",
        "{ clock_gettime(id, time) }",
        "clock_gettime",
        "",
    ] {
        let source = CLOCK.replace("clock_gettime(id, time)", body);
        assert_eq!(problems(&source), [BLOCK_BODY], "{body}");
    }
}

#[test]
fn scan_keeps_a_raw_borrow_and_a_cast_as_plain_arguments() {
    let source = CLOCK.replace(
        "clock_gettime(id, time)",
        "abi::clock_gettime(id, &raw mut local, len as usize, -1, core::ptr::null_mut)",
    );
    let found = read(&source, false);
    assert_eq!(found.problems, []);
    assert_eq!(
        found.sites.get(1).map(|site| site.kind.clone()),
        Some(SiteKind::Block {
            operation: "abi::clock_gettime".into(),
            arguments: vec![
                "id".into(),
                "& raw mut local".into(),
                "len as usize".into(),
                "- 1".into(),
                "core :: ptr :: null_mut".into(),
            ],
        })
    );
}

#[test]
fn scan_checks_raw_spellings_of_built_in_names_but_not_of_keywords() {
    assert_eq!(
        problems(&format!(
            "{CLOCK}#[r#cfg(target_arch = \"x86_64\")]\nconst A: u8 = 1;\n"
        )),
        [CFG]
    );
    assert_eq!(problems("#[r#path = \"x.rs\"]\nmod x;"), [HIDDEN]);
    assert_eq!(problems("r#include!(\"x.rs\");"), [HIDDEN]);
    assert_eq!(
        problems("fn f() { let r#unsafe = 1; let r#extern = 2; }"),
        [] as [&str; 0]
    );
}

#[test]
fn scan_reads_an_unsafe_fn_only_as_a_named_miri_stand_in() {
    let stand_in = read(
        "#[expect(unsafe_code, reason = \"linux-clock: Miri stand-in\")]\n\
         pub(super) unsafe fn clock_gettime(id: c_int, tp: *mut timespec) -> c_int { 0 }\n",
        false,
    );
    assert_eq!(stand_in.problems, []);
    assert_eq!(
        stand_in
            .sites
            .first()
            .map(|site| (site.id.clone(), site.kind.clone())),
        Some((
            Some("linux-clock: Miri stand-in".into()),
            SiteKind::StandInFn
        ))
    );
    assert_eq!(
        problems(
            "#[expect(unsafe_code, reason = \"linux-clock\")]\n\
             pub(super) unsafe fn clock_gettime() -> c_int { 0 }\n"
        ),
        [UNSAFE_ITEM]
    );
}

#[test]
fn scan_records_a_site_with_no_name_and_refuses_two_names() {
    let unnamed = read(
        &CLOCK.replace(
            "    #[expect(unsafe_code, reason = \"linux-clock\")]\n    // SAFETY",
            "    // SAFETY",
        ),
        false,
    );
    assert_eq!(unnamed.sites.get(1).map(|site| site.id.clone()), Some(None));
    let twice = CLOCK.replace(
        "    #[expect(unsafe_code, reason = \"linux-clock\")]\n    // SAFETY",
        "    #[expect(unsafe_code, reason = \"linux-clock\")]\n    #[expect(unsafe_code, reason = \"linux-random\")]\n    // SAFETY",
    );
    assert_eq!(problems(&twice), [TWO_NAMES]);
}

#[test]
fn scan_reads_an_unsafe_impl_and_a_link_attribute() {
    let found = read(
        "#[expect(unsafe_code, reason = \"windows-dns\")]\nunsafe impl Send for Block {}\n\
         #[expect(unsafe_code, reason = \"windows-dns\")]\n\
         #[link(name = \"Dnsapi\", kind = \"raw-dylib\")]\n\
         unsafe extern \"system\" { fn DnsFree(data: *mut c_void, kind: c_int); }\n",
        false,
    );
    assert_eq!(found.problems, []);
    assert_eq!(
        found
            .sites
            .iter()
            .map(|site| site.kind.clone())
            .collect::<Vec<_>>(),
        [
            SiteKind::Impl,
            SiteKind::Extern {
                link: Some("name=Dnsapi;kind=raw-dylib".into()),
                items: vec![ExternItem {
                    name: "DnsFree".into(),
                    safe: false,
                }],
            },
        ]
    );
}

#[test]
fn scan_refuses_unsafe_functions_traits_and_other_foreign_forms() {
    for (source, expected) in [
        ("pub unsafe fn f() {}", UNSAFE_ITEM),
        ("unsafe trait T {}", UNSAFE_ITEM),
        ("unsafe extern \"C\" fn callback() {}", UNSAFE_ITEM),
        ("extern \"C\" fn callback() {}", EXTERN_FORM),
        ("unsafe extern { fn f(); }", EXTERN_FORM),
        ("unsafe extern \"C\" { static X: c_int; }", EXTERN_ITEM),
        ("#[link(name = \"c\")]\nfn f() {}", LINK),
        ("macro_rules! m { () => {} }", HIDDEN),
        ("include!(\"x.rs\");", HIDDEN),
        ("#[path = \"x.rs\"]\nmod x;", HIDDEN),
        ("fn f() { core::arch::asm!(\"nop\"); }", HIDDEN),
    ] {
        assert_eq!(problems(source), [expected], "{source}");
    }
}

#[test]
fn scan_allows_cfg_only_as_the_test_module_gate_in_a_file_with_a_site() {
    assert_eq!(problems(CLOCK), [] as [&str; 0]);
    for added in [
        "#[cfg(target_arch = \"x86_64\")]\nconst A: u8 = 1;\n",
        "#[cfg_attr(test, derive(Debug))]\nstruct S;\n",
        "fn f() -> bool { cfg!(miri) }\n",
    ] {
        assert_eq!(problems(&format!("{CLOCK}{added}")), [CFG], "{added}");
    }
    assert_eq!(
        problems("#[cfg(target_os = \"linux\")]\nmod linux_clock;\n"),
        [] as [&str; 0]
    );
}

#[test]
fn scan_allows_target_env_only_in_the_crate_root_allowlist() {
    let allowlist = "#[cfg(not(any(all(target_os = \"linux\", target_env = \"gnu\"))))]\n\
                     compile_error!(\"targets\");\n";
    assert_eq!(read(allowlist, true).problems, []);
    assert_eq!(
        read(allowlist, false)
            .problems
            .iter()
            .map(|problem| problem.what)
            .collect::<Vec<_>>(),
        [TARGET_ENV]
    );
    let later = format!("{allowlist}#[cfg(target_env = \"musl\")]\nmod musl;\n");
    assert_eq!(
        read(&later, true)
            .problems
            .iter()
            .map(|problem| problem.what)
            .collect::<Vec<_>>(),
        [TARGET_ENV]
    );
}

#[test]
fn scan_reads_the_crate_root_s_glibc_only_gates_and_no_other_target_env() {
    let gates = "#[cfg(any(test, all(target_os = \"linux\", target_env = \"gnu\")))]\nmod checks;\n\
                 #[cfg(all(target_os = \"linux\", target_env = \"gnu\"))]\nmod linux_fdpass;\n\
                 #[cfg(all(target_os = \"linux\", target_env = \"gnu\"))]\npub use linux_fdpass::send_fd;\n";
    let root = read(gates, true);
    assert_eq!(root.problems, []);
    assert_eq!(root.glibc_only, ["linux_fdpass"]);
    // Not in the crate root; another library; a renaming import.
    assert_eq!(read(gates, false).problems.len(), 3);
    for refused in [
        "#[cfg(all(target_os = \"linux\", target_env = \"musl\"))]\nmod linux_fdpass;\n",
        "#[cfg(all(target_os = \"linux\", target_env = \"gnu\"))]\npub use linux_fdpass::send_fd as s;\n",
        "#[cfg(all(target_env = \"gnu\", target_os = \"linux\"))]\nmod linux_fdpass;\n",
    ] {
        let found = read(refused, true);
        assert_eq!(
            found
                .problems
                .iter()
                .map(|problem| problem.what)
                .collect::<Vec<_>>(),
            [TARGET_ENV],
            "{refused}"
        );
        assert!(found.glibc_only.is_empty(), "{refused}");
    }
}

#[test]
fn outside_crate_finds_unsafe_foreign_blocks_and_link_attributes_only() {
    let found = |source: &str| -> Vec<(usize, &'static str)> {
        outside_crate(&tokens_with_strings(source).unwrap())
            .into_iter()
            .map(|problem| (problem.line, problem.what))
            .collect()
    };
    assert_eq!(
        found("fn f() {\n let s = \"unsafe extern\";\n // unsafe\n}\nextern crate alloc;\n"),
        []
    );
    assert_eq!(
        found(
            "fn f() {\n unsafe { g() }\n}\nunsafe extern \"C\" {}\n#[link(name = \"c\")]\nextern \"C\" fn h() {}\n"
        ),
        [
            (2, OUTSIDE),
            (4, OUTSIDE),
            (4, OUTSIDE),
            (5, OUTSIDE),
            (6, OUTSIDE)
        ]
    );
}

/// The gates that choose a C library's declarations, as an `abi.rs` writes
/// them (ADR 0005, point 5.2: `struct msghdr` and `struct cmsghdr` are
/// declared per C library).
const LIBRARY_GATES: &str = "#[cfg(target_env = \"gnu\")]\nmod gnu;\n\
                             #[cfg(target_env = \"musl\")]\nmod musl;\n\
                             #[cfg(target_env = \"gnu\")]\npub(super) use gnu::{cmsghdr, msghdr};\n\
                             #[cfg(target_env = \"musl\")]\npub(super) use musl::{cmsghdr, msghdr};\n";

#[test]
fn scan_allows_target_env_in_an_abi_file_only_to_gate_a_c_library_module() {
    assert_eq!(
        whats(&read_at(LIBRARY_GATES, Place::Abi)),
        Vec::<&str>::new()
    );
    for place in [Place::Other, Place::Library] {
        assert_eq!(
            whats(&read_at(LIBRARY_GATES, place)),
            [TARGET_ENV; 4],
            "{place:?}"
        );
    }
    for planted in [
        // A gate on anything but the module of its own library.
        "#[cfg(target_env = \"musl\")]\nmod gnu;\n",
        "#[cfg(target_env = \"musl\")]\npub(super) use gnu::msghdr;\n",
        "#[cfg(target_env = \"musl\")]\npub use musl::msghdr;\n",
        "#[cfg(target_env = \"musl\")]\npub(super) use musl as gnu;\n",
        "#[cfg(target_env = \"musl\")]\nconst PLANTED: u8 = 0;\n",
        "#[cfg(target_env = \"uclibc\")]\nmod uclibc;\n",
        "#[cfg(not(target_env = \"musl\"))]\nmod gnu;\n",
        "#[cfg(any(target_env = \"musl\", target_os = \"linux\"))]\nmod musl;\n",
        "#[cfg_attr(target_env = \"musl\", path = \"x.rs\")]\nmod musl;\n",
    ] {
        assert!(
            whats(&read_at(planted, Place::Abi)).contains(&TARGET_ENV),
            "{planted}"
        );
    }
}

#[test]
fn scan_refuses_any_site_in_a_c_library_declaration_file() {
    // A library's file is compiled for one C library only, so a site there
    // would make a musl build's sites differ from the glibc build's.
    let found = read_at(CLOCK, Place::Library);
    assert!(whats(&found).contains(&LIBRARY_SITE), "{found:?}");
    assert!(!whats(&read_at(CLOCK, Place::Other)).contains(&LIBRARY_SITE));
}

#[test]
fn place_of_names_the_root_the_abi_files_and_the_library_files() {
    use super::place_of;
    use std::path::Path;
    assert_eq!(place_of(Path::new("src/lib.rs")), Place::Root);
    assert_eq!(place_of(Path::new("src/linux_fdpass/abi.rs")), Place::Abi);
    assert_eq!(
        place_of(Path::new("src/linux_fdpass/abi/gnu.rs")),
        Place::Library
    );
    assert_eq!(
        place_of(Path::new("src/linux_fdpass/abi/musl.rs")),
        Place::Library
    );
    assert_eq!(
        place_of(Path::new("src/linux_random/number.rs")),
        Place::Other
    );
    assert_eq!(place_of(Path::new("src/abi.rs")), Place::Other);
    // A library's module may have children, or be written `gnu/mod.rs`:
    // every file below it is that library's.
    for library_file in [
        "src/linux_fdpass/abi/gnu/mod.rs",
        "src/linux_fdpass/abi/gnu/planted.rs",
        "src/linux_fdpass/abi/musl/deeper/planted.rs",
    ] {
        assert_eq!(
            place_of(Path::new(library_file)),
            Place::Library,
            "{library_file}"
        );
    }
}

#[cfg(unix)]
#[test]
fn place_of_a_path_with_a_component_that_is_not_utf_8_is_other() {
    use super::place_of;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;
    // Dropping the component would read `src/lib.rs`, the crate root.
    let path = Path::new("src")
        .join(OsStr::from_bytes(b"\xff"))
        .join("lib.rs");
    assert_eq!(place_of(&path), Place::Other);
}
