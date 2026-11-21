// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-pure --self-test`: builds a clean fixture whose pure crates pass
//! and prints its exact matrix, then plants each way a pure crate could
//! reach the standard library, a foreign function or changed compiler
//! flags, or break the crate root's form, and proves the check fails on
//! each with what was planted. Negative controls build the clean fixture
//! with a crate, a target or a profile skipped, and with an extra crate,
//! and each must fail the matrix. A fixture that adds `ironafs-rxkad` is
//! checked with no change to the check, and a workspace with no pure crate
//! passes.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use super::matrix::{Gap, Triple};
use super::{
    Cargo, Plan, Violation, build_violations, core_sysroot, pure_members, report, static_violations,
};
use crate::Error;
use crate::fixture::{self, Fixture};
use crate::metadata::Workspace;

const CHECK: &str = "check-pure";

/// The pure crate the cases plant in. No other pure crate depends on it,
/// so a broken build is reported for it alone.
const TOP: &str = "ironafs-xdr";
const TOP_LIB: &str = "crates/ironafs-xdr/src/lib.rs";

/// The rest of the planted crate's library root, after its first lines.
const TOP_BODY: &str = "extern crate alloc;\n\
     \n\
     use alloc::vec::Vec;\n\
     \n\
     #[cfg(test)]\n\
     mod tests;\n\
     \n\
     /// External data representation, big-endian.\n\
     pub fn encode(value: u32) -> Vec<u8> {\n\
         value.to_be_bytes().to_vec()\n\
     }\n";

/// The planted crate's library root: `head`, then the body.
fn top_source(head: &str) -> String {
    format!("{head}\n{TOP_BODY}")
}

fn plant_head(fixture: &mut Fixture, head: &str) {
    fixture.file(TOP, "src/lib.rs", Some(&top_source(head)));
}

/// Adds `code` after the clean body, from line 13.
fn plant_code(fixture: &mut Fixture, code: &str) {
    let source = format!("{}{code}", top_source("#![no_std]"));
    fixture.file(TOP, "src/lib.rs", Some(&source));
}

/// Adds `code` after the clean body and lets the crate's own manifest allow
/// unsafe code, so only the check's own `-F unsafe_code` can stop it.
fn plant_unsafe(fixture: &mut Fixture, code: &str) {
    let source = format!("{}{code}", top_source("#![no_std]"));
    fixture.file(TOP, "src/lib.rs", Some(&source));
    fixture.lints(TOP, "\n[lints.rust]\nunsafe_code = \"allow\"\n");
}

/// Two pure crates that use `core` and `alloc` (atomics, `Arc`, ordered
/// maps, formatting, unit tests with no `std`), a development dependency on
/// a crate that uses `std`, an integration test that uses `std`, an
/// `ironafs-client` that uses threads, a root cargo configuration with only
/// an alias, and a nested checkout whose configuration cargo never reads
/// from the root.
fn clean() -> Fixture {
    let mut fixture = Fixture::default();
    fixture.add_crate("ironafs-time", &[]);
    fixture.add_crate(TOP, &["ironafs-time"]);
    fixture.add_crate("ironafs-client", &[TOP]);
    fixture.add_crate("ironafs-test-support", &[]);
    fixture.file(
        "ironafs-time",
        "src/lib.rs",
        Some(
            "#![no_std]\nextern crate alloc;\n\n\
             use alloc::collections::BTreeMap;\nuse alloc::format;\nuse alloc::string::String;\n\
             use alloc::sync::Arc;\nuse core::sync::atomic::{AtomicU64, Ordering};\n\n\
             pub fn count(counter: &AtomicU64) -> u64 { counter.fetch_add(1, Ordering::Relaxed) }\n\
             pub fn show(map: &BTreeMap<u32, Arc<u64>>) -> String { format!(\"{map:?}\") }\n",
        ),
    );
    fixture.file(TOP, "src/lib.rs", Some(&top_source("#![no_std]")));
    fixture.file(
        TOP,
        "src/tests.rs",
        Some(
            "use super::encode;\n\n#[test]\nfn encodes() { assert_eq!(encode(1), alloc::vec![0, 0, 0, 1]); }\n",
        ),
    );
    fixture.file(
        TOP,
        "tests/it/main.rs",
        Some("#[test]\nfn uses_std() { let _ = std::collections::HashMap::<u8, u8>::new(); }\n"),
    );
    fixture.manifest_tail(
        TOP,
        "\n[dev-dependencies]\nironafs-test-support = { path = \"../ironafs-test-support\" }\n",
    );
    fixture.file(
        "ironafs-test-support",
        "src/lib.rs",
        Some("pub fn say() { std::println!(\"x\"); }\n"),
    );
    fixture.file(
        "ironafs-client",
        "src/lib.rs",
        Some("pub fn run(f: fn()) { std::thread::spawn(f); }\n"),
    );
    fixture.root_file(".cargo/config.toml", "[alias]\nx = \"check\"\n");
    fixture.root_file("nested/.git", "gitdir: elsewhere\n");
    fixture.root_file(
        "nested/.cargo/config.toml",
        "[build]\nrustflags = [\"--cap-lints\", \"allow\"]\n",
    );
    fixture
}

fn is_build(violation: &Violation, profile: &str, says: &str) -> bool {
    matches!(violation, Violation::Build { name, profile: found, error }
        if name == TOP && *found == profile && error.contains(says))
}

/// The error the compiler gives when a crate needs `std` and the sysroot has
/// none: "can't find crate for `std`".
const NO_STD_CRATE: &str = "E0463";

fn is_extern(violation: &Violation, line: usize) -> bool {
    matches!(violation, Violation::Extern { file, line: found }
        if file == Path::new(TOP_LIB) && *found == line)
}

fn is_source(violation: &Violation, line: usize, name: &str) -> bool {
    matches!(violation, Violation::Source { file, line: found, name: named, .. }
        if file == Path::new(TOP_LIB) && *found == line && named == name)
}

/// A file of the planted crate outside its library's directory.
fn is_extern_in(violation: &Violation, file: &str, line: usize) -> bool {
    matches!(violation, Violation::Extern { file: found, line: at }
        if found == Path::new(file) && *at == line)
}

/// A planted file of the crate, outside its library's directory, that
/// brings the standard library back under a cfg no build sets.
const HIDDEN_STD: &str = "#[cfg(panic = \"abort\")]\nextern crate std as hidden;\n";

fn is_unreadable(violation: &Violation) -> bool {
    matches!(violation, Violation::Unreadable { file, .. } if file == Path::new(TOP_LIB))
}

/// Whether the violation is `word` found in the raw text of the planted
/// library root at `line`.
fn is_text(violation: &Violation, line: usize, word: &str) -> bool {
    matches!(violation, Violation::NamedInText { file, line: at, word: found }
        if file == Path::new(TOP_LIB) && *at == line && *found == word)
}

fn is_not_no_std(violation: &Violation) -> bool {
    matches!(violation, Violation::NotNoStd(file) if file == Path::new(TOP_LIB))
}

fn is_no_alloc(violation: &Violation) -> bool {
    matches!(violation, Violation::NoAlloc(file) if file == Path::new(TOP_LIB))
}

fn is_root_cfg(violation: &Violation, line: usize) -> bool {
    matches!(violation, Violation::RootCfg { file, line: at }
        if file == Path::new(TOP_LIB) && *at == line)
}

fn is_closure(violation: &Violation, to: &str) -> bool {
    matches!(violation, Violation::Closure { from, via, to: found }
        if from == TOP && via.is_empty() && found == to)
}

fn is_target(violation: &Violation, name: &str, target: &str) -> bool {
    matches!(violation, Violation::Target { name: found, target: kind }
        if found == name && kind == target)
}

fn is_config(violation: &Violation, file: &str, line: usize, word: &str) -> bool {
    matches!(violation, Violation::CargoConfig { file: found, line: at, word: named, .. }
        if found == Path::new(file) && *at == line && *named == word)
}

/// A planted case: what it plants, and what it must produce. Each
/// predicate must match at least one violation, and each violation at
/// least one predicate.
struct Case {
    name: &'static str,
    plant: fn(&mut Fixture),
    expect: &'static [fn(&Violation) -> bool],
}

const CASES: &[Case] = &[
    Case {
        name: "extern crate std in a pure crate",
        plant: |fixture| plant_head(fixture, "#![no_std]\nextern crate std;"),
        expect: &[
            |found| is_extern(found, 2),
            |found| is_source(found, 2, "extern"),
            |found| is_source(found, 2, "std"),
            |found| is_build(found, "dev", NO_STD_CRATE),
            |found| is_build(found, "release", NO_STD_CRATE),
        ],
    },
    Case {
        name: "extern crate std on every target but none",
        plant: |fixture| {
            plant_head(
                fixture,
                "#![no_std]\n#[cfg(not(target_os = \"none\"))]\nextern crate std;",
            );
        },
        expect: &[
            |found| is_extern(found, 3),
            |found| is_source(found, 3, "extern"),
            |found| is_source(found, 3, "std"),
            |found| is_build(found, "dev", NO_STD_CRATE),
            |found| is_build(found, "release", NO_STD_CRATE),
        ],
    },
    Case {
        name: "extern crate std on Windows only",
        plant: |fixture| plant_head(fixture, "#![no_std]\n#[cfg(windows)]\nextern crate std;"),
        expect: &[
            |found| is_extern(found, 3),
            |found| is_source(found, 3, "extern"),
            |found| is_source(found, 3, "std"),
            |found| is_build(found, "dev", NO_STD_CRATE),
            |found| is_build(found, "release", NO_STD_CRATE),
        ],
    },
    Case {
        name: "extern crate std in release builds only",
        plant: |fixture| {
            plant_head(
                fixture,
                "#![no_std]\n#[cfg(not(debug_assertions))]\nextern crate std;",
            );
        },
        expect: &[
            |found| is_extern(found, 3),
            |found| is_source(found, 3, "extern"),
            |found| is_source(found, 3, "std"),
            |found| is_build(found, "release", NO_STD_CRATE),
        ],
    },
    Case {
        name: "no_std only where the target has no operating system",
        plant: |fixture| plant_head(fixture, "#![cfg_attr(target_os = \"none\", no_std)]"),
        expect: &[
            is_not_no_std,
            |found| is_source(found, 1, "no_std"),
            |found| is_build(found, "dev", NO_STD_CRATE),
            |found| is_build(found, "release", NO_STD_CRATE),
        ],
    },
    Case {
        name: "a pure crate without no_std",
        plant: |fixture| plant_head(fixture, "//! Uses the standard library."),
        expect: &[
            is_not_no_std,
            |found| is_build(found, "dev", NO_STD_CRATE),
            |found| is_build(found, "release", NO_STD_CRATE),
        ],
    },
    Case {
        name: "the standard library through a dependency's std feature",
        plant: |fixture| {
            fixture.add_third_party("hostlib", &[]);
            fixture.file(
                "hostlib",
                "src/lib.rs",
                Some("#![cfg_attr(not(feature = \"std\"), no_std)]\npub fn f() {}\n"),
            );
            fixture.manifest_tail("hostlib", "\n[features]\ndefault = [\"std\"]\nstd = []\n");
            fixture.depend(TOP, "hostlib");
        },
        expect: &[
            |found| is_closure(found, "hostlib"),
            |found| {
                matches!(found, Violation::Features { name, features }
                    if name == "hostlib" && features.iter().any(|feature| feature == "std"))
            },
            |found| is_build(found, "dev", NO_STD_CRATE),
            |found| is_build(found, "release", NO_STD_CRATE),
        ],
    },
    Case {
        name: "a foreign function in an unsafe extern block",
        plant: |fixture| {
            plant_unsafe(
                fixture,
                "unsafe extern \"C\" { safe fn getpid() -> i32; }\npub fn pid() -> i32 { getpid() }\n",
            );
        },
        expect: &[
            |found| is_extern(found, 13),
            |found| is_source(found, 13, "unsafe"),
            |found| is_source(found, 13, "extern"),
            |found| is_build(found, "dev", "unsafe"),
            |found| is_build(found, "release", "unsafe"),
        ],
    },
    Case {
        name: "inline assembly",
        plant: |fixture| {
            plant_unsafe(
                fixture,
                "pub fn pause() { unsafe { core::arch::asm!(\"nop\") } }\n",
            );
        },
        expect: &[
            |found| is_source(found, 13, "unsafe"),
            |found| is_source(found, 13, "arch"),
            |found| is_source(found, 13, "asm!"),
            |found| is_build(found, "dev", "unsafe"),
            |found| is_build(found, "release", "unsafe"),
        ],
    },
    Case {
        name: "no_std in a comment, and only under the default panic strategy",
        plant: |fixture| {
            plant_head(
                fixture,
                "/*\n#![no_std]\n*/\n#![cfg_attr(not(panic = \"abort\"), no_std)]",
            );
        },
        expect: &[is_not_no_std, |found| is_source(found, 4, "no_std")],
    },
    Case {
        name: "the standard library through include! of a file outside the library",
        plant: |fixture| {
            plant_head(fixture, "#![no_std]\ninclude!(\"../ext.rs\");");
            fixture.file(TOP, "ext.rs", Some(HIDDEN_STD));
        },
        expect: &[
            |found| is_text(found, 2, "include"),
            |found| is_source(found, 2, "include!"),
            |found| is_extern_in(found, "crates/ironafs-xdr/ext.rs", 2),
        ],
    },
    Case {
        name: "the standard library through a module path outside the library",
        plant: |fixture| {
            plant_head(
                fixture,
                "#![no_std]\n#[path = \"../outside.rs\"]\nmod outside;",
            );
            fixture.file(TOP, "outside.rs", Some(HIDDEN_STD));
        },
        expect: &[
            |found| is_source(found, 2, "#[path]"),
            |found| is_extern_in(found, "crates/ironafs-xdr/outside.rs", 2),
        ],
    },
    Case {
        name: "extern crate alloc under a cfg",
        plant: |fixture| plant_head(fixture, "#![no_std]\n#[cfg(not(panic = \"abort\"))]"),
        expect: &[|found| is_source(found, 3, "extern"), is_no_alloc],
    },
    Case {
        name: "a core-only crate root without extern crate alloc",
        plant: |fixture| {
            fixture.file(
                TOP,
                "src/lib.rs",
                Some("#![no_std]\n\n/// Big-endian.\npub fn encode(value: u32) -> [u8; 4] { value.to_be_bytes() }\n"),
            );
        },
        expect: &[is_no_alloc],
    },
    Case {
        name: "no_std after an item",
        plant: |fixture| {
            fixture.file(
                TOP,
                "src/lib.rs",
                Some("extern crate alloc;\n#![no_std]\n\npub fn f() {}\n"),
            );
        },
        expect: &[
            is_not_no_std,
            |found| is_source(found, 2, "no_std"),
            |found| is_build(found, "dev", "inner attribute"),
            |found| is_build(found, "release", "inner attribute"),
        ],
    },
    Case {
        name: "cfg_attr(not(test), no_std), which every checked build enables",
        plant: |fixture| plant_head(fixture, "#![cfg_attr(not(test), no_std)]"),
        expect: &[is_not_no_std, |found| is_source(found, 1, "no_std")],
    },
    Case {
        name: "a crate-level cfg at the root",
        plant: |fixture| plant_head(fixture, "#![no_std]\n#![cfg(not(windows))]"),
        expect: &[|found| is_root_cfg(found, 2)],
    },
    Case {
        name: "the processor's identity through a safe function of core::arch",
        plant: |fixture| {
            plant_code(
                fixture,
                "#[cfg(target_arch = \"x86_64\")]\n\
                 pub fn id() -> u32 { core::arch::x86_64::__cpuid(0).eax }\n\
                 #[cfg(target_arch = \"x86_64\")]\n\
                 pub fn read() -> fn(u32) -> core::arch::x86_64::CpuidResult { core::arch::x86_64::__cpuid }\n",
            );
        },
        expect: &[
            |found| is_source(found, 14, "arch"),
            |found| is_source(found, 16, "arch"),
        ],
    },
    Case {
        name: "assembly under a panic strategy no build uses",
        plant: |fixture| {
            plant_head(
                fixture,
                "#![no_std]\n#[cfg(panic = \"abort\")]\npub fn raw() { unsafe { core::arch::asm!(\"nop\") } }",
            );
        },
        expect: &[
            |found| is_source(found, 3, "unsafe"),
            |found| is_source(found, 3, "arch"),
            |found| is_source(found, 3, "asm!"),
        ],
    },
    Case {
        name: "the alloc crate renamed std",
        plant: |fixture| plant_head(fixture, "#![no_std]\nuse alloc as std;"),
        expect: &[|found| is_source(found, 2, "std")],
    },
    Case {
        name: "an attribute built from a macro fragment",
        plant: |fixture| {
            plant_head(
                fixture,
                "#![no_std]\nmacro_rules! hide { ($a:meta) => { #[$a] mod x {} }; }",
            );
        },
        expect: &[|found| is_source(found, 2, "#[$")],
    },
    Case {
        name: "the standard library through include! joined to its bang by a macro",
        plant: |fixture| {
            plant_head(
                fixture,
                "#![no_std]\n\
                 macro_rules! pull { ($bang:tt $name:ident) => { $name $bang(\"../ext.rs\"); }; }\n\
                 pull!(! include);",
            );
            fixture.file(TOP, "ext.rs", Some(HIDDEN_STD));
        },
        expect: &[
            |found| is_text(found, 3, "include"),
            |found| is_source(found, 3, "include"),
            |found| is_extern_in(found, "crates/ironafs-xdr/ext.rs", 2),
        ],
    },
    Case {
        name: "include! hidden behind a string with a suffix",
        plant: |fixture| {
            plant_head(
                fixture,
                "#![no_std]\n\
                 macro_rules! drop_all { ($($t:tt)*) => {}; }\n\
                 drop_all!(\"\"r\"\\\" \");\n\
                 include!(\"../ext.rs\"); // \"",
            );
            fixture.file(TOP, "ext.rs", Some(HIDDEN_STD));
        },
        expect: &[
            |found| is_text(found, 4, "include"),
            is_unreadable,
            is_not_no_std,
            |found| is_extern_in(found, "crates/ironafs-xdr/ext.rs", 2),
        ],
    },
    Case {
        name: "include! after a lifetime spelled like a raw string's prefix",
        plant: |fixture| {
            plant_head(
                fixture,
                "#![no_std]\n\
                 macro_rules! drop_all { ($($t:tt)*) => {}; }\n\
                 drop_all!('r\"\\\" \");\n\
                 include!(\"../ext.rs\"); // \"",
            );
            fixture.file(TOP, "ext.rs", Some(HIDDEN_STD));
        },
        expect: &[
            |found| is_text(found, 4, "include"),
            |found| is_source(found, 4, "include!"),
            |found| is_extern_in(found, "crates/ironafs-xdr/ext.rs", 2),
        ],
    },
    Case {
        name: "a shebang line the compiler skips unread",
        plant: |fixture| plant_head(fixture, "#!/usr/bin/env x \"\n#![no_std] // \""),
        expect: &[is_unreadable, is_not_no_std],
    },
    Case {
        name: "the standard library through a module path whose brackets are a macro fragment",
        plant: |fixture| {
            plant_head(
                fixture,
                "#![no_std]\n\
                 macro_rules! load { ($group:tt) => { # $group mod hidden; }; }\n\
                 load!([path = \"../outside.rs\"]);",
            );
            fixture.file(TOP, "outside.rs", Some(HIDDEN_STD));
        },
        expect: &[
            |found| is_source(found, 2, "#"),
            |found| is_extern_in(found, "crates/ironafs-xdr/outside.rs", 2),
        ],
    },
    Case {
        name: "the names of the macros that read files or write assembly, not called",
        plant: |fixture| {
            plant_head(
                fixture,
                "#![no_std]\n\
                 pub fn a(include: u8) {}\n\
                 pub fn b(include_str: u8) {}\n\
                 pub fn c(include_bytes: u8) {}\n\
                 pub mod asm {}\n\
                 pub mod global_asm {}\n\
                 pub mod naked_asm {}",
            );
        },
        expect: &[
            |found| is_text(found, 2, "include"),
            |found| is_text(found, 3, "include_str"),
            |found| is_text(found, 4, "include_bytes"),
            |found| is_source(found, 2, "include"),
            |found| is_source(found, 3, "include_str"),
            |found| is_source(found, 4, "include_bytes"),
            |found| is_source(found, 5, "asm"),
            |found| is_source(found, 6, "global_asm"),
            |found| is_source(found, 7, "naked_asm"),
        ],
    },
    Case {
        name: "the names of the macros that read files in a comment and a file not compiled",
        plant: |fixture| {
            plant_head(fixture, "#![no_std]\n// include_str!(\"key\")");
            fixture.file(TOP, "notes.md", Some("Reads data with include_bytes!.\n"));
        },
        expect: &[
            |found| is_text(found, 2, "include_str"),
            |found| {
                matches!(found, Violation::NamedInText { file, line: 1, word: "include_bytes" }
                    if file == Path::new("crates/ironafs-xdr/notes.md"))
            },
        ],
    },
    Case {
        name: "a root cargo configuration that caps the lints",
        plant: |fixture| {
            plant_unsafe(
                fixture,
                "pub fn pause() { unsafe { core::arch::asm!(\"nop\") } }\n",
            );
            fixture.root_file(
                ".cargo/config.toml",
                "[build]\nrustflags = [\"--cap-lints\", \"allow\"]\n",
            );
        },
        expect: &[
            |found| is_config(found, ".cargo/config.toml", 2, "rustflags"),
            |found| is_source(found, 13, "unsafe"),
            |found| is_source(found, 13, "arch"),
            |found| is_source(found, 13, "asm!"),
            |found| is_build(found, "dev", "unsafe"),
            |found| is_build(found, "release", "unsafe"),
        ],
    },
    Case {
        name: "compiler flags in a nested cargo configuration",
        plant: |fixture| {
            fixture.root_file(
                "crates/ironafs-time/.cargo/config.toml",
                "[target.'cfg(all())']\nrustflags = [\"--cfg\", \"host\"]\n",
            );
        },
        expect: &[|found| {
            is_config(
                found,
                "crates/ironafs-time/.cargo/config.toml",
                2,
                "rustflags",
            )
        }],
    },
    Case {
        name: "a compiler wrapper in a cargo configuration",
        plant: |fixture| {
            fixture.root_file(
                "tools/.cargo/config.toml",
                "[build]\nrustc-wrapper = \"w\"\n",
            );
        },
        expect: &[|found| is_config(found, "tools/.cargo/config.toml", 2, "rustc")],
    },
    Case {
        name: "a workspace compiler wrapper in a cargo configuration",
        plant: |fixture| {
            fixture.root_file(
                "tools/.cargo/config.toml",
                "[build] # wrap\nrustc-workspace-wrapper = \"w\"\n",
            );
        },
        expect: &[|found| is_config(found, "tools/.cargo/config.toml", 2, "rustc")],
    },
    Case {
        name: "the compiler's environment in a cargo configuration",
        plant: |fixture| {
            fixture.root_file("tools/.cargo/config", "[env]\nRUSTC_BOOTSTRAP = \"1\"\n");
        },
        expect: &[|found| is_config(found, "tools/.cargo/config", 1, "env")],
    },
    Case {
        name: "a pure crate with a build script",
        plant: |fixture| fixture.file("ironafs-time", "build.rs", Some("fn main() {}\n")),
        expect: &[|found| is_target(found, "ironafs-time", "custom-build")],
    },
    Case {
        name: "a pure crate that links a native library",
        plant: |fixture| {
            fixture.package_line(TOP, "links = \"native\"");
            fixture.file(TOP, "build.rs", Some("fn main() {}\n"));
        },
        expect: &[
            |found| is_target(found, TOP, "custom-build"),
            |found| matches!(found, Violation::Links { name, links } if name == TOP && links == "native"),
        ],
    },
    Case {
        name: "a pure crate with a feature",
        plant: |fixture| fixture.manifest_tail(TOP, "\n[features]\nextra = []\n"),
        expect: &[|found| {
            matches!(found, Violation::Features { name, features }
                if name == TOP && features == &["extra"])
        }],
    },
    Case {
        name: "a pure crate on a procedural macro",
        plant: |fixture| {
            fixture.add_third_party("derive-it", &[]);
            fixture.manifest_tail("derive-it", "\n[lib]\nproc-macro = true\n");
            fixture.depend(TOP, "derive-it");
        },
        expect: &[
            |found| is_closure(found, "derive-it"),
            |found| is_target(found, "derive-it", "proc-macro"),
        ],
    },
    Case {
        name: "a pure crate on a no_std crate that is not pure",
        plant: |fixture| {
            fixture.add_third_party("tiny", &[]);
            fixture.file("tiny", "src/lib.rs", Some("#![no_std]\npub fn f() {}\n"));
            fixture.depend(TOP, "tiny");
        },
        expect: &[|found| is_closure(found, "tiny")],
    },
    Case {
        name: "a pure crate with no library",
        plant: |fixture| {
            fixture.add_crate("ironafs-config", &[]);
            fixture.file("ironafs-config", "src/lib.rs", None);
            fixture.file("ironafs-config", "src/main.rs", Some("fn main() {}\n"));
        },
        expect: &[
            |found| matches!(found, Violation::NoLibrary(name) if name == "ironafs-config"),
            |found| is_target(found, "ironafs-config", "bin"),
        ],
    },
];

/// Whether the violations are what the case planted.
fn as_planted(case: &Case, found: &[Violation]) -> bool {
    case.expect.iter().all(|expect| found.iter().any(expect))
        && found
            .iter()
            .all(|violation| case.expect.iter().any(|expect| expect(violation)))
}

/// Runs the clean fixture, every planted case, and a workspace with no pure
/// crate, against a core-only sysroot made from `toolchain` for `targets`.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
pub(super) fn run(
    target_dir: &Path,
    toolchain: &Path,
    targets: &[String],
    output: &mut impl Write,
) -> Result<(), Error> {
    let dir = fixture::fresh_dir(target_dir, CHECK)?;
    let result = run_in(&dir, toolchain, targets, output);
    fixture::remove_dir(&dir)?;
    result
}

/// A negative control of the matrix: the clean fixture built with a
/// changed plan, compared with the full plan's triples.
struct Control {
    name: &'static str,
    change: for<'a> fn(&mut Plan<'a>, &'a Workspace),
    expect: fn(&Gap) -> bool,
}

const CONTROLS: &[Control] = &[
    Control {
        name: "a run with one pure crate skipped",
        change: |plan, _| plan.members.retain(|member| member.name != TOP),
        expect: |gap| matches!(gap, Gap::Missing(triple) if triple.name == TOP),
    },
    Control {
        name: "a run with one target skipped",
        change: |plan, _| {
            plan.targets.pop();
        },
        expect: |gap| matches!(gap, Gap::Missing(_)),
    },
    Control {
        name: "a run with the release profile skipped",
        change: |plan, _| plan.profiles.retain(|profile| *profile != "release"),
        expect: |gap| matches!(gap, Gap::Missing(triple) if triple.profile == "release"),
    },
    Control {
        name: "a run with an extra crate built",
        change: |plan, workspace| {
            plan.members.extend(
                workspace
                    .members
                    .iter()
                    .filter(|member| member.name == EXTRA),
            );
        },
        expect: |gap| matches!(gap, Gap::Extra(triple) if triple.name == EXTRA),
    },
];

/// A crate on no pure list that builds against core alone, which the
/// extra-crate control builds as if it were pure.
const EXTRA: &str = "ironafs-net";

fn run_in(
    dir: &Path,
    toolchain: &Path,
    targets: &[String],
    output: &mut impl Write,
) -> Result<(), Error> {
    let sysroot = dir.join("sysroot");
    core_sysroot(toolchain, targets, &sysroot)?;
    let build = Build {
        sysroot: &sysroot,
        targets,
    };

    let clean_run = build.check(&clean(), &dir.join("clean"), "the clean fixture", None)?;
    let crates = || ["ironafs-time", TOP].map(str::to_owned);
    if !clean_run.found.is_empty()
        || clean_run.configs != 1
        || clean_run.pure != 2
        || clean_run.compiled.len() != 2 * targets.len() * 2
        || !clean_run.names().into_iter().eq(crates())
    {
        return Err(failure("the clean fixture", &clean_run.found));
    }
    say(output, "passed: the clean fixture")?;
    let mut lines = Vec::new();
    report(
        &clean_run.workspace,
        clean_run.configs,
        targets.len(),
        &clean_run.compiled,
        &mut lines,
    )
    .map_err(|_| failure("the clean fixture's report", &[]))?;
    say(output, String::from_utf8_lossy(&lines).trim_end())?;

    for (index, case) in CASES.iter().enumerate() {
        let mut planted = clean();
        (case.plant)(&mut planted);
        let run = build.check(
            &planted,
            &dir.join(format!("case-{index}")),
            case.name,
            None,
        )?;
        match run.found.first() {
            Some(first) if as_planted(case, &run.found) => {
                say(output, &format!("caught: {}: {first}", case.name))?;
            }
            _ => return Err(failure(case.name, &run.found)),
        }
    }

    run_controls(&build, dir, output)?;
    run_later(&build, dir, output)?;

    let mut none = Fixture::default();
    none.add_crate("ironafs-client", &[]);
    none.add_crate("ironafs-cli", &["ironafs-client"]);
    let run = build.check(&none, &dir.join("none"), "no pure crates", None)?;
    if !run.found.is_empty() || run.pure != 0 || !run.compiled.is_empty() {
        return Err(failure("no pure crates", &run.found));
    }
    say(output, "passed: no pure crates yet")?;
    say(
        output,
        &format!(
            "check-pure --self-test: the clean fixture, a fixture that adds ironafs-rxkad and a \
         workspace with no pure crate passed; {} planted cases and {} matrix controls failed as \
         planted",
            CASES.len() + 1,
            CONTROLS.len()
        ),
    )
}

/// Builds the clean fixture with each control's changed plan, and requires
/// each to fail the matrix, and only the matrix.
fn run_controls(build: &Build<'_>, dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    for (index, control) in CONTROLS.iter().enumerate() {
        let mut fixture = clean();
        fixture.add_crate(EXTRA, &[]);
        fixture.file(EXTRA, "src/lib.rs", Some("#![no_std]\npub fn f() {}\n"));
        let run = build.check(
            &fixture,
            &dir.join(format!("control-{index}")),
            control.name,
            Some(control.change),
        )?;
        let gaps: Vec<&Gap> = run
            .found
            .iter()
            .filter_map(|found| match found {
                Violation::Matrix(gap) => Some(gap),
                _ => None,
            })
            .collect();
        match run.found.first() {
            Some(first)
                if gaps.len() == run.found.len()
                    && gaps.iter().all(|gap| (control.expect)(gap)) =>
            {
                say(output, &format!("caught: {}: {first}", control.name))?;
            }
            _ => return Err(failure(control.name, &run.found)),
        }
    }
    Ok(())
}

/// A later pure crate is checked from the commit that adds it, with no
/// change to the check: a clean `ironafs-rxkad` passes with all its
/// triples, and one that names `std` fails.
fn run_later(build: &Build<'_>, dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let mut later = clean();
    later.add_crate("ironafs-rxkad", &["ironafs-xdr"]);
    later.file(
        "ironafs-rxkad",
        "src/lib.rs",
        Some("#![no_std]\nextern crate alloc;\n\npub fn level() -> u8 { 1 }\n"),
    );
    let run = build.check(
        &later,
        &dir.join("later"),
        "a fixture that adds ironafs-rxkad",
        None,
    )?;
    let rxkad = run
        .compiled
        .iter()
        .filter(|triple| triple.name == "ironafs-rxkad")
        .count();
    if !run.found.is_empty() || run.pure != 3 || rxkad != build.targets.len() * 2 {
        return Err(failure("a fixture that adds ironafs-rxkad", &run.found));
    }
    say(
        output,
        &format!("passed: a fixture that adds ironafs-rxkad, checked for its {rxkad} triples"),
    )?;
    let mut planted = later;
    planted.file(
        "ironafs-rxkad",
        "src/lib.rs",
        Some("#![no_std]\nextern crate alloc;\nextern crate std;\n"),
    );
    let run = build.check(
        &planted,
        &dir.join("later-planted"),
        "ironafs-rxkad with std",
        None,
    )?;
    let caught = run
        .found
        .iter()
        .any(|found| matches!(found, Violation::Build { name, .. } if name == "ironafs-rxkad"));
    if !caught {
        return Err(failure("ironafs-rxkad with std", &run.found));
    }
    say(output, "caught: ironafs-rxkad with extern crate std")?;
    Ok(())
}

fn say(output: &mut impl Write, line: &str) -> Result<(), Error> {
    writeln!(output, "{line}").map_err(Error::io("report check-pure self-test"))
}

fn failure(case: &'static str, found: &[Violation]) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.iter().map(ToString::to_string).collect(),
    }
}

/// The sysroot and targets every fixture is built with.
struct Build<'a> {
    sysroot: &'a Path,
    targets: &'a [String],
}

/// What one fixture's run found.
struct Run {
    workspace: Workspace,
    found: Vec<Violation>,
    configs: usize,
    pure: usize,
    compiled: BTreeSet<Triple>,
}

impl Run {
    fn names(&self) -> BTreeSet<String> {
        self.compiled
            .iter()
            .map(|triple| triple.name.clone())
            .collect()
    }
}

impl Build<'_> {
    /// Writes a fixture, reads it back through `cargo metadata`, and runs
    /// every rule and the build on it. `change`, for a negative control,
    /// changes the plan built; the triples compared stay the full plan's.
    fn check(
        &self,
        fixture: &Fixture,
        dir: &Path,
        case: &'static str,
        change: Option<for<'a> fn(&mut Plan<'a>, &'a Workspace)>,
    ) -> Result<Run, Error> {
        let workspace = fixture.load(dir, CHECK, case)?;
        let (mut found, configs) = static_violations(&workspace)?;
        let manifest = workspace.root.join("Cargo.toml");
        let config = fixture.config(dir);
        let cargo = Cargo {
            manifest: Some(&manifest),
            config: &config,
        };
        let full = Plan::full(&workspace, self.targets);
        let expected = full.expected();
        let mut plan = Plan::full(&workspace, self.targets);
        if let Some(change) = change {
            change(&mut plan, &workspace);
        }
        let built = build_violations(
            &workspace,
            &cargo,
            self.sysroot,
            &plan,
            &expected,
            &dir.join("build"),
        )?;
        found.extend(built.violations);
        let pure = pure_members(&workspace).len();
        Ok(Run {
            workspace,
            found,
            configs,
            pure,
            compiled: built.compiled,
        })
    }
}
