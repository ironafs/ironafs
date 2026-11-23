// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-os --self-test`: plants each change ADR 0005, point 6.7, names in
//! copies of the crate's own files, held in memory, and proves the
//! comparison fails on it with what was planted, against the files each
//! target's real build read. The unplanted copies must pass. The
//! confinement rule is proved on a small tree written under the target
//! directory.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::allowlist::{Cfg, guard_of};
use super::gate::{self, BUILD_SCRIPT};
use super::inventory::Inventory;
use super::pin::{self, Pinned};
use super::scan::{self, BLOCK_BODY, CFG, CORE, TARGET_ENV};
use super::{
    ALLOWLIST, CrateFile, GLIBC_ONLY, Violation, compare, gate_violations, guard_violations,
    outside_violations, rust_files,
};
use crate::pure::toolchain_targets;
use crate::rust_source::tokens_with_strings;
use crate::{Command, Error, fixture};

const CHECK: &str = "check-os";

/// One edit of the crate's files: replace `from` by `to` in `file`, or,
/// with `from` empty, add `file` holding `to`, compiled for `compiled_on`.
struct Edit {
    file: &'static str,
    from: &'static str,
    to: &'static str,
    compiled_on: &'static [&'static str],
}

const fn replace(file: &'static str, from: &'static str, to: &'static str) -> Edit {
    Edit {
        file,
        from,
        to,
        compiled_on: &[],
    }
}

/// An edit that also compiles `file` for `compiled_on`.
const fn replace_compiled(
    file: &'static str,
    from: &'static str,
    to: &'static str,
    compiled_on: &'static [&'static str],
) -> Edit {
    Edit {
        file,
        from,
        to,
        compiled_on,
    }
}

const fn add(file: &'static str, to: &'static str, compiled_on: &'static [&'static str]) -> Edit {
    Edit {
        file,
        from: "",
        to,
        compiled_on,
    }
}

const LINUX: &[&str] = &["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];
const EVERY: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
];

type Expect = fn(&Violation) -> bool;

struct Case {
    name: &'static str,
    edits: &'static [Edit],
    expect: &'static [Expect],
}

fn mismatch(violation: &Violation, row: &str) -> bool {
    matches!(violation, Violation::Mismatch { id, .. } if id == row)
}

fn rule(violation: &Violation, rule: &str) -> bool {
    matches!(violation, Violation::Rule { what, .. } if *what == rule)
}

const SYSCALL: &str = "abi::syscall(number, address, length, flags, unused3, unused4, unused5)";
const WIPE_BLOCK: &str = "unsafe { core::ptr::write_volatile(byte, 0) }";
const WIPE_NAME: &str = "#[expect(unsafe_code, reason = \"mem-wipe\")]\n";
const STRAY_WIPE: &str = "fn zero(byte: *mut u8) {\n\
                          #[expect(unsafe_code, reason = \"mem-wipe\")]\n\
                          // SAFETY: planted.\n\
                          let () = unsafe { core::ptr::write_volatile(byte, 0) };\n}\n";

const CASES: &[Case] = &[
    Case {
        name: "a second block in a Miri stand-in",
        edits: &[replace(
            "src/linux_clock/abi/stand_in.rs",
            "    0\n}",
            "    #[expect(unsafe_code, reason = \"linux-clock: Miri stand-in\")]\n    \
             // SAFETY: planted.\n    let () = unsafe { core::ptr::write(tp, reading) };\n    0\n}",
        )],
        expect: &[|found| mismatch(found, "linux-clock: Miri stand-in")],
    },
    Case {
        name: "a variadic call with fewer arguments than recorded (three after the number)",
        edits: &[replace(
            "src/linux_random.rs",
            SYSCALL,
            "abi::syscall(number, address, length, flags)",
        )],
        expect: &[|found| mismatch(found, "linux-random")],
    },
    Case {
        name: "a volatile write of a value other than zero",
        edits: &[replace(
            "src/mem_wipe.rs",
            WIPE_BLOCK,
            "unsafe { core::ptr::write_volatile(byte, 1) }",
        )],
        expect: &[|found| mismatch(found, "mem-wipe")],
    },
    Case {
        name: "a cfg spelled as a raw identifier in a file that holds a site",
        edits: &[replace(
            "src/linux_clock/abi/foreign.rs",
            "use super::timespec;",
            "use super::timespec;\n\n#[r#cfg(target_arch = \"x86_64\")]\nconst PLANTED: u8 = 0;",
        )],
        expect: &[|found| rule(found, CFG)],
    },
    Case {
        name: "a Miri stand-in compiled for a target",
        edits: &[replace_compiled(
            "src/linux_clock/abi/stand_in.rs",
            "static SECONDS",
            "static SECONDS",
            LINUX,
        )],
        expect: &[
            |found| rule(found, super::STAND_IN_COMPILED),
            |found| mismatch(found, "linux-clock: Miri stand-in"),
        ],
    },
    Case {
        name: "a Miri stand-in in a file not named stand_in.rs",
        edits: &[add(
            "src/linux_clock/abi/fake.rs",
            "#[expect(unsafe_code, reason = \"linux-clock: Miri stand-in\")]\n\
             pub(super) unsafe fn clock_gettime() -> c_int { 0 }\n",
            &[],
        )],
        expect: &[
            |found| matches!(found, Violation::OutsideModule { id, .. } if id == "linux-clock: Miri stand-in"),
            |found| mismatch(found, "linux-clock: Miri stand-in"),
        ],
    },
    Case {
        name: "an unsafe fn not named as a Miri stand-in",
        edits: &[add(
            "src/linux_clock/helper.rs",
            "#[expect(unsafe_code, reason = \"linux-clock\")]\n\
             pub(super) unsafe fn helper() -> c_int { 0 }\n",
            LINUX,
        )],
        expect: &[|found| rule(found, super::scan::UNSAFE_ITEM)],
    },
    Case {
        name: "a declaration swapped for another, the block count unchanged",
        edits: &[
            replace(
                "src/linux_clock/abi/foreign.rs",
                "fn clock_gettime(",
                "fn clock_settime(",
            ),
            replace(
                "src/linux_clock.rs",
                "abi::clock_gettime(",
                "abi::clock_settime(",
            ),
        ],
        expect: &[|found| mismatch(found, "linux-clock")],
    },
    Case {
        name: "a foreign function added to an existing extern block",
        edits: &[replace(
            "src/linux_clock/abi/foreign.rs",
            "-> c_int;\n}",
            "-> c_int;\n    pub(super) fn clock_getres(clockid: c_int, res: *mut timespec) -> c_int;\n}",
        )],
        expect: &[|found| mismatch(found, "linux-clock")],
    },
    Case {
        name: "code for a row that is only approved",
        edits: &[add(
            "src/linux_mount.rs",
            "#[expect(unsafe_code, reason = \"linux-mount\")]\n\
             unsafe extern \"C\" { fn mount(source: c_int) -> c_int; }\n",
            LINUX,
        )],
        expect: &[|found| mismatch(found, "linux-mount")],
    },
    Case {
        name: "an item of an implemented row removed",
        edits: &[replace(
            "src/linux_random.rs",
            "unsafe { abi::syscall(number, address, length, flags, unused3, unused4, unused5) }",
            "-1",
        )],
        expect: &[|found| mismatch(found, "linux-random")],
    },
    Case {
        name: "a second foreign call in a block, its id and the counts unchanged",
        edits: &[replace(
            "src/linux_random.rs",
            SYSCALL,
            "abi::syscall(number, address, length, flags, unused3, unused4, unused5);\n        abi::syscall(number, address, length, flags, unused3, unused4, unused5)",
        )],
        expect: &[
            |found| rule(found, BLOCK_BODY),
            |found| mismatch(found, "linux-random"),
        ],
    },
    Case {
        name: "a raw read added to a block, its id and the counts unchanged",
        edits: &[replace(
            "src/mem_wipe.rs",
            WIPE_BLOCK,
            "unsafe { core::ptr::write_volatile(byte, *byte) }",
        )],
        expect: &[
            |found| rule(found, BLOCK_BODY),
            |found| mismatch(found, "mem-wipe"),
        ],
    },
    Case {
        name: "a site with no named expectation",
        edits: &[replace("src/mem_wipe.rs", WIPE_NAME, "")],
        expect: &[
            |found| matches!(found, Violation::Unnamed { file, .. } if file == Path::new("src/mem_wipe.rs")),
            |found| mismatch(found, "mem-wipe"),
        ],
    },
    Case {
        name: "a site outside its row's module",
        edits: &[add("src/stray.rs", STRAY_WIPE, EVERY)],
        expect: &[
            |found| matches!(found, Violation::OutsideModule { id, .. } if id == "mem-wipe"),
            |found| mismatch(found, "mem-wipe"),
        ],
    },
    Case {
        name: "a module named core, beside a site",
        edits: &[replace(
            "src/mem_wipe.rs",
            "fn zero(byte: &mut u8) {",
            "mod core {\n    pub(super) mod ptr {\n        pub(in crate::mem_wipe) fn \
             write_volatile(byte: *mut u8, value: u8) {\n            let _ = (byte, value);\n        \
             }\n    }\n}\n\nfn zero(byte: &mut u8) {",
        )],
        expect: &[|found| rule(found, CORE)],
    },
    Case {
        name: "a site in a file no target compiles",
        edits: &[add("src/mem_wipe/stray.rs", STRAY_WIPE, &[])],
        expect: &[|found| matches!(found, Violation::NotCompiled { id, .. } if id == "mem-wipe")],
    },
    Case {
        name: "a cfg in a file that holds a site",
        edits: &[replace(
            "src/linux_clock/abi/foreign.rs",
            "use super::timespec;",
            "use super::timespec;\n\n#[cfg(target_arch = \"x86_64\")]\nconst PLANTED: u8 = 0;",
        )],
        expect: &[|found| rule(found, CFG)],
    },
    Case {
        name: "a site in a C library's declaration file",
        edits: &[replace(
            "src/linux_fdpass/abi/gnu.rs",
            "impl cmsghdr {",
            "fn planted() {\n    #[expect(unsafe_code, reason = \"linux-fdpass\")]\n    \
             // SAFETY: planted.\n    let () = unsafe { core::hint::unreachable_unchecked() };\n}\n\n\
             impl cmsghdr {",
        )],
        expect: &[
            |found| rule(found, super::scan::LIBRARY_SITE),
            // The library file now holds a site, so its own cfg attributes
            // are refused too, and the glibc build's sites differ.
            |found| rule(found, CFG),
            |found| mismatch(found, "linux-fdpass"),
        ],
    },
    Case {
        name: "a target_env gate in an abi.rs on a module not named for its library",
        edits: &[replace(
            "src/linux_fdpass/abi.rs",
            "#[cfg(target_env = \"musl\")]\nmod musl;",
            "#[cfg(target_env = \"musl\")]\nmod gnu;",
        )],
        expect: &[|found| rule(found, TARGET_ENV)],
    },
    Case {
        name: "target_env outside the crate root's allowlist",
        edits: &[replace(
            "src/linux_random/number.rs",
            "#[cfg(target_arch = \"aarch64\")]\nmod aarch64;",
            "#[cfg(target_arch = \"aarch64\")]\nmod aarch64;\n#[cfg(target_env = \"musl\")]\nmod musl;",
        )],
        expect: &[|found| rule(found, TARGET_ENV)],
    },
    Case {
        name: "a row withheld on musl left in the musl build",
        edits: &[replace(
            "src/lib.rs",
            "#[cfg(all(target_os = \"linux\", target_env = \"gnu\"))]\nmod linux_fdpass;",
            "#[cfg(target_os = \"linux\")]\nmod linux_fdpass;",
        )],
        expect: &[|found| rule(found, GLIBC_ONLY)],
    },
    Case {
        name: "a row implemented on musl left out of the musl build",
        edits: &[replace(
            "src/lib.rs",
            "#[cfg(target_os = \"linux\")]\nmod linux_peercred;",
            "#[cfg(all(target_os = \"linux\", target_env = \"gnu\"))]\nmod linux_peercred;",
        )],
        expect: &[|found| rule(found, GLIBC_ONLY)],
    },
];

/// What the self-test runs with: the pinned compiler, the repository, its
/// target directory, the crate and every built-in target's cfg set.
pub(super) struct Context<'a> {
    pub(super) pinned: &'a Pinned,
    pub(super) root: &'a Path,
    pub(super) target_dir: &'a Path,
    pub(super) crate_dir: &'a Path,
    pub(super) cfgs: &'a BTreeMap<String, Cfg>,
}

/// Runs the clean copies, every planted case, a target whose build read no
/// site file, and the confinement cases.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
pub(super) fn run(
    context: &Context<'_>,
    inventory: &Inventory,
    compiled: &BTreeMap<String, BTreeSet<PathBuf>>,
    output: &mut impl Write,
) -> Result<(), Error> {
    let Context {
        root,
        target_dir,
        crate_dir,
        cfgs,
        ..
    } = *context;
    let report = |output: &mut dyn Write, line: String| {
        writeln!(output, "{line}").map_err(Error::io("report check-os self-test"))
    };
    let originals = read_crate(crate_dir)?;
    let adr_path = root.join(super::inventory::ADR);
    let adr = fs::read_to_string(&adr_path).map_err(Error::path_io("read ADR", &adr_path))?;

    let clean = check_edited(&originals, &[], inventory, compiled)?;
    if !clean.is_empty() {
        return Err(failure("the crate's own files", &clean));
    }
    report(output, "passed: the crate's own files".to_owned())?;

    for case in CASES {
        let found = check_edited(&originals, case.edits, inventory, compiled)?;
        let as_planted = case.expect.iter().all(|expect| found.iter().any(expect))
            && found
                .iter()
                .all(|violation| case.expect.iter().any(|expect| expect(violation)));
        match found.first() {
            Some(first) if as_planted => report(output, format!("caught: {}: {first}", case.name))?,
            _ => return Err(failure(case.name, &found)),
        }
    }

    paired_case(&originals, &adr, inventory, compiled)?;
    report(
        output,
        "caught: a declaration and its call changed in the code and in sites.json alike, to one \
         the ADR does not name"
            .to_owned(),
    )?;

    paired_write_case(&originals, &adr, inventory, compiled)?;
    report(
        output,
        "caught: the wipe's write changed in the code and in sites.json alike, to a same-named \
         write elsewhere"
            .to_owned(),
    )?;

    let mut emptied = compiled.clone();
    for files in emptied.values_mut() {
        files.retain(|file| file == Path::new("src/lib.rs"));
    }
    let found = check_edited(&originals, &[], inventory, &emptied)?;
    let mismatch = |found: &Violation| matches!(found, Violation::Mismatch { .. });
    let not_compiled = |found: &Violation| matches!(found, Violation::NotCompiled { .. });
    let as_planted = found.iter().any(mismatch)
        && found.iter().any(not_compiled)
        && found
            .iter()
            .all(|found| mismatch(found) || not_compiled(found));
    if !as_planted {
        return Err(failure("a build that read only the crate root", &found));
    }
    report(
        output,
        "caught: a build that read only the crate root".to_owned(),
    )?;

    boundary_cases(context, &originals, inventory, compiled, output)?;
    let musl = musl_cases(&originals, inventory, compiled, output)?;

    let lib = originals
        .get(Path::new("src/lib.rs"))
        .cloned()
        .unwrap_or_default();
    allowlist_case(&lib, cfgs)?;
    report(
        output,
        "caught: a crate root guard without its vendor and pointer-width predicates".to_owned(),
    )?;

    let guard = guard_of(&lib).map_err(|error| Error::SelfTest {
        check: CHECK,
        case: "the crate root's guard",
        found: vec![error],
    })?;
    let (guard_clean, unlisted) = guard_violations(&guard, cfgs, ALLOWLIST);
    if !guard_clean.is_empty() {
        return Err(failure("the crate root's guard, unplanted", &guard_clean));
    }
    target_cases(context, &originals, &unlisted, output)?;

    let cases = outside_cases(target_dir)?;
    report(
        output,
        format!(
            "check-os --self-test: the crate's own files passed and {} planted cases failed as \
             planted",
            CASES.len() + 4 + BOUNDARY_CASES + TARGET_CASES + musl + cases
        ),
    )
}

/// For each musl target the check built (its standard library installed,
/// as in each native musl CI lane), a site planted for that target alone:
/// the comparison of that target's own build with the copy must catch it.
/// The input is synthetic: the planted file is added to the file list that
/// target's real build produced, not compiled by a build of its own.
/// Returns how many it planted.
fn musl_cases(
    originals: &BTreeMap<PathBuf, String>,
    inventory: &Inventory,
    compiled: &BTreeMap<String, BTreeSet<PathBuf>>,
    output: &mut impl Write,
) -> Result<usize, Error> {
    const CASE: &str =
        "a site planted for a musl target alone (synthetic input: added to its build's file list)";
    let mut planted = 0;
    for target in compiled.keys().filter(|target| target.ends_with("-musl")) {
        let mut only = compiled.clone();
        if let Some(files) = only.get_mut(target) {
            files.insert(PathBuf::from("src/mem_wipe/stray.rs"));
        }
        let mut texts = originals.clone();
        texts.insert(
            PathBuf::from("src/mem_wipe/stray.rs"),
            STRAY_WIPE.to_owned(),
        );
        let found = check_edited(&texts, &[], inventory, &only)?;
        let caught = !found.is_empty()
            && found.iter().all(|violation| {
                matches!(violation, Violation::Mismatch { target: found, id, .. }
                    if found == target && id == "mem-wipe")
            });
        if !caught {
            return Err(failure(CASE, &found));
        }
        writeln!(output, "caught: {CASE}: {target}")
            .map_err(Error::io("report check-os self-test"))?;
        planted += 1;
    }
    Ok(planted)
}

/// How many cases `boundary_cases` plants.
const BOUNDARY_CASES: usize = 2;

/// The cases at the check's boundaries: a file the compiler read that was
/// not scanned, and a cfg key that tells musl from glibc.
fn boundary_cases(
    context: &Context<'_>,
    originals: &BTreeMap<PathBuf, String>,
    inventory: &Inventory,
    compiled: &BTreeMap<String, BTreeSet<PathBuf>>,
    output: &mut impl Write,
) -> Result<(), Error> {
    let report = |output: &mut dyn Write, line: String| {
        writeln!(output, "{line}").map_err(Error::io("report check-os self-test"))
    };
    let mut unscanned = compiled.clone();
    for files in unscanned.values_mut() {
        files.insert(PathBuf::from("src/unscanned.rs"));
    }
    let found = check_edited(originals, &[], inventory, &unscanned)?;
    let as_planted = !found.is_empty()
        && found.iter().all(|found| {
            matches!(found, Violation::NotScanned { file, .. } if file == Path::new("src/unscanned.rs"))
        });
    if !as_planted {
        return Err(failure("a compiled file this check did not scan", &found));
    }
    report(
        output,
        "caught: a compiled file this check did not scan".to_owned(),
    )?;

    let keys = super::allowlist::musl_keys(context.pinned, context.root)?;
    let lib = originals
        .get(Path::new("src/lib.rs"))
        .cloned()
        .unwrap_or_default();
    let planted = format!("{lib}\n#[cfg(target_feature = \"crt-static\")]\nmod musl_only;\n");
    let found = super::musl_key_uses(Path::new("src/lib.rs"), &planted, &keys);
    if found.len() != 1
        || !matches!(found.first(), Some(Violation::Rule { what, .. }) if *what == super::MUSL_KEY)
    {
        return Err(failure(
            "a module gated on crt-static, which only musl targets set",
            &found,
        ));
    }
    if !super::musl_key_uses(Path::new("src/lib.rs"), &lib, &keys).is_empty() {
        return Err(failure("the crate root, unplanted, naming a musl key", &[]));
    }
    report(
        output,
        "caught: a module gated on crt-static, which only musl targets set".to_owned(),
    )?;

    Ok(())
}

/// The wipe's write changed in the code and in `sites.json` alike to a
/// same-named write elsewhere (`shadow::write_volatile`, the last segment
/// kept), so the two agree: the comparison finds nothing, and only the
/// binding of a write block to `core`'s own volatile write refuses it.
fn paired_write_case(
    originals: &BTreeMap<PathBuf, String>,
    adr: &str,
    inventory: &Inventory,
    compiled: &BTreeMap<String, BTreeSet<PathBuf>>,
) -> Result<(), Error> {
    const CASE: &str = "the wipe's write changed in the code and in sites.json alike";
    const SUBSTITUTE: &str = "shadow::write_volatile";
    let edits = [replace(
        "src/mem_wipe.rs",
        WIPE_BLOCK,
        "unsafe { shadow::write_volatile(byte, 0) }",
    )];
    let mut changed = inventory.clone();
    for row in changed.rows.iter_mut().filter(|row| row.id == "mem-wipe") {
        if let Some(code) = row.code.as_mut() {
            for block in &mut code.blocks {
                SUBSTITUTE.clone_into(&mut block.operation);
            }
        }
    }
    let found = check_edited(originals, &edits, &changed, compiled)?;
    if !found.is_empty() {
        return Err(failure(CASE, &found));
    }
    let problems = super::inventory::problems(&changed, adr);
    let caught = !problems.is_empty()
        && problems
            .iter()
            .all(|problem| problem.contains(SUBSTITUTE) || problem.contains("no write block"));
    if caught {
        Ok(())
    } else {
        Err(Error::SelfTest {
            check: CHECK,
            case: CASE,
            found: problems,
        })
    }
}

/// A change made to the code and to `sites.json` alike, so the two agree:
/// `clock_settime` declared and called in place of `clock_gettime`. Only
/// the binding of the copy to the ADR's own row can catch it.
fn paired_case(
    originals: &BTreeMap<PathBuf, String>,
    adr: &str,
    inventory: &Inventory,
    compiled: &BTreeMap<String, BTreeSet<PathBuf>>,
) -> Result<(), Error> {
    const CASE: &str = "a declaration changed in the code and in sites.json alike";
    let edits = [
        replace(
            "src/linux_clock/abi/foreign.rs",
            "fn clock_gettime(",
            "fn clock_settime(",
        ),
        replace(
            "src/linux_clock/abi.rs",
            "use foreign::clock_gettime;",
            "use foreign::clock_settime as clock_gettime;",
        ),
        replace(
            "src/linux_clock.rs",
            "abi::clock_gettime(",
            "abi::clock_settime(",
        ),
    ];
    let mut changed = inventory.clone();
    for row in changed
        .rows
        .iter_mut()
        .filter(|row| row.id == "linux-clock")
    {
        if let Some(code) = row.code.as_mut() {
            for item in code
                .extern_blocks
                .iter_mut()
                .flat_map(|block| &mut block.items)
            {
                "clock_settime".clone_into(&mut item.name);
            }
            for block in &mut code.blocks {
                "abi::clock_settime".clone_into(&mut block.operation);
            }
        }
    }
    let found = check_edited(originals, &edits, &changed, compiled)?;
    if !found.is_empty() {
        return Err(failure(CASE, &found));
    }
    let problems = super::inventory::problems(&changed, adr);
    let caught = !problems.is_empty()
        && problems
            .iter()
            .all(|problem| problem.contains("clock_settime") && problem.contains("does not name"));
    if caught {
        Ok(())
    } else {
        Err(Error::SelfTest {
            check: CHECK,
            case: CASE,
            found: problems,
        })
    }
}

/// The crate root's guard with its vendor, pointer-width, endian and ABI
/// predicates removed must accept targets outside the allowlist whose cfg
/// sets differ from every allowlisted one (32-bit, big-endian, other
/// vendors), which `check-os` refuses.
fn allowlist_case(lib: &str, cfgs: &BTreeMap<String, Cfg>) -> Result<(), Error> {
    const CASE: &str = "a crate root guard without its vendor and pointer-width predicates";
    let mut weakened = lib.to_owned();
    for predicate in [
        "target_pointer_width = \"64\",",
        "target_endian = \"little\",",
        "target_vendor = \"unknown\",",
        "target_vendor = \"pc\",",
    ] {
        weakened = weakened.replace(predicate, "");
    }
    if weakened == lib {
        return Err(Error::SelfTest {
            check: CHECK,
            case: CASE,
            found: vec!["the crate root has none of the predicates to remove".to_owned()],
        });
    }
    let found = guard_found(&weakened, cfgs, CASE)?;
    let caught = ["x86_64-unikraft-linux-musl", "x86_64-win7-windows-msvc"]
        .iter()
        .all(|target| found.iter().any(|violation| names(violation, target)));
    if caught {
        Ok(())
    } else {
        Err(failure(CASE, &found))
    }
}

/// The guard violations of crate root `lib` on `cfgs`.
fn guard_found(
    lib: &str,
    cfgs: &BTreeMap<String, Cfg>,
    case: &'static str,
) -> Result<Vec<Violation>, Error> {
    let guard = guard_of(lib).map_err(|error| Error::SelfTest {
        check: CHECK,
        case,
        found: vec![error],
    })?;
    Ok(guard_violations(&guard, cfgs, ALLOWLIST).0)
}

fn names(violation: &Violation, name: &str) -> bool {
    matches!(violation, Violation::Target { target, .. } if target == name)
}

/// The listed triple a planted gate drops or refuses.
const DROPPED: &str = "aarch64-pc-windows-msvc";

/// The test by which the gate accepts a target, as the build script
/// writes it.
const ACCEPTS: &str = "TARGETS.contains(&target.as_str())";

/// What the check found, as the gate's two checks report it: the list
/// against the reference copy, or the gate's verdicts when run.
#[derive(Clone, Copy)]
enum Kind {
    /// A list problem naming the triple.
    List,
    /// A verdict on the triple (or the special value of that name).
    Verdict,
}

/// Whether `violation` is of `kind` and about `name`.
fn about(violation: &Violation, kind: Kind, name: &str) -> bool {
    match (kind, violation) {
        (Kind::List, Violation::Target { target, what }) => {
            target == GATE && what.starts_with(&format!("{name}: "))
        }
        (Kind::Verdict, Violation::Target { target, .. }) => target == name,
        _ => false,
    }
}

/// The target name of a violation about the gate as a whole.
const GATE: &str = "the build script's gate";

/// A planted copy of the build script, by its edits (each `from`, there
/// once, replaced by `to`), and what the check must find: violations of
/// one kind only, about one name, or every listed triple.
struct GateCase {
    name: &'static str,
    edits: &'static [(&'static str, &'static str)],
    kind: Kind,
    /// The name every violation is about; `None` for every listed triple,
    /// each with at least one violation.
    about: Option<&'static str>,
}

const GATE_CASES: &[GateCase] = &[
    GateCase {
        name: "a gate list missing one listed triple, while its code still accepts it",
        edits: &[
            ("[&str; 7]", "[&str; 6]"),
            ("    \"aarch64-pc-windows-msvc\",\n", ""),
            (
                ACCEPTS,
                "(TARGETS.contains(&target.as_str()) || target == \"aarch64-pc-windows-msvc\")",
            ),
        ],
        kind: Kind::List,
        about: Some(DROPPED),
    },
    GateCase {
        name: "a gate that refuses one listed triple, while its list still names it",
        edits: &[(
            ACCEPTS,
            "TARGETS.contains(&target.as_str()) && target != \"aarch64-pc-windows-msvc\"",
        )],
        kind: Kind::Verdict,
        about: Some(DROPPED),
    },
    GateCase {
        name: "a gate that prints the target unescaped",
        edits: &[("format!(\"{target:?}\")", "target")],
        kind: Kind::Verdict,
        about: Some(LINE_BREAK),
    },
    GateCase {
        name: "a gate whose rerun rule names more than the build script",
        edits: &[(
            "    println!(\"cargo::rerun-if-changed=build.rs\");\n",
            "    println!(\"cargo::rerun-if-changed=build.rs\");\n    \
             println!(\"cargo::rerun-if-changed=src\");\n",
        )],
        kind: Kind::Verdict,
        about: None,
    },
];

/// The verdict check's name for its line-break value.
const LINE_BREAK: &str = "a TARGET holding a line break";

/// Whether `found` is what `case` planted.
fn as_planted(case: &GateCase, found: &[Violation]) -> bool {
    match case.about {
        // The unescaped gate also prints every refused triple unquoted;
        // the line break must be among what is found, and only verdicts.
        Some(LINE_BREAK) => {
            found
                .iter()
                .any(|violation| about(violation, case.kind, LINE_BREAK))
                && found.iter().all(|violation| !about_list(violation))
        }
        Some(name) => {
            !found.is_empty()
                && found
                    .iter()
                    .all(|violation| about(violation, case.kind, name))
        }
        None => {
            ALLOWLIST.iter().all(|target| {
                found
                    .iter()
                    .any(|violation| about(violation, case.kind, target))
            }) && found.iter().all(|violation| !about_list(violation))
        }
    }
}

fn about_list(violation: &Violation) -> bool {
    matches!(violation, Violation::Target { target, .. } if target == GATE)
}

/// How many cases `target_cases` plants.
const TARGET_CASES: usize = GATE_CASES.len() + 4;

/// What point 5.3's cases are planted in and checked against.
struct Planting<'a> {
    context: &'a Context<'a>,
    /// This process's own fixture directory, emptied first.
    scratch: PathBuf,
    originals: &'a BTreeMap<PathBuf, String>,
    targets: Vec<String>,
    script: String,
    lib: String,
}

impl Planting<'_> {
    /// What the gate's check finds in `planted`, a build script written
    /// under the fixture directory for `case`.
    fn gate_found(&self, case: &str, planted: &str) -> Result<Vec<Violation>, Error> {
        let dir = self.scratch.join(case.replace(' ', "-"));
        let script = dir.join(BUILD_SCRIPT);
        fixture::write_file(&script, planted)?;
        gate_violations(
            self.context.pinned,
            self.context.root,
            &script,
            &dir.join("gate"),
            &self.targets,
        )
    }
}

/// Point 5.3's negative tests, each run through the check's own functions
/// on a planted copy of the build script or the crate root, each planting
/// one change: a gate whose code accepts one cfg-equal extra triple (its
/// verdict; also built through Cargo) and one whose list names it (the list
/// check); a gate list missing one listed triple; a gate that refuses one
/// listed triple; and a guard widened by one target. With a gate that prints
/// the target unescaped, one that names more than itself in its rerun rule,
/// and a crate-level attribute that removes the guard, which the compiler
/// confirms.
fn target_cases(
    context: &Context<'_>,
    originals: &BTreeMap<PathBuf, String>,
    unlisted: &[String],
    output: &mut impl Write,
) -> Result<(), Error> {
    let text = |path: &str| originals.get(Path::new(path)).cloned().unwrap_or_default();
    let planting = Planting {
        context,
        scratch: fixture::fresh_dir(context.target_dir, "check-os-targets")?,
        originals,
        targets: context.cfgs.keys().cloned().collect(),
        script: text(BUILD_SCRIPT),
        lib: text("src/lib.rs"),
    };
    let result = planted_cases(&planting, unlisted, output);
    fixture::remove_dir(&planting.scratch)?;
    result
}

/// The cases of `target_cases`, in `planting`'s fixture directory.
fn planted_cases(
    planting: &Planting<'_>,
    unlisted: &[String],
    output: &mut impl Write,
) -> Result<(), Error> {
    let clean = planting.gate_found("the build script", &planting.script)?;
    if !clean.is_empty() {
        return Err(failure("the build script, unplanted", &clean));
    }
    extra_cases(planting, unlisted, output)?;
    for case in GATE_CASES {
        let mut planted = planting.script.clone();
        for (from, to) in case.edits {
            planted = plant(&planted, from, to, case.name)?;
        }
        let found = planting.gate_found(case.name, &planted)?;
        if !as_planted(case, &found) {
            return Err(failure(case.name, &found));
        }
        report_caught(output, case.name, &first(&found))?;
    }
    widened_case(planting, output)?;
    crate_attribute_case(planting, output)
}

fn report_caught(output: &mut impl Write, case: &str, found: &str) -> Result<(), Error> {
    writeln!(output, "caught: {case}: {found}").map_err(Error::io("report check-os self-test"))
}

/// Two gates for the first of `unlisted`, a target whose cfg set equals a
/// listed one's, each planting one change. One accepts it in its code,
/// its list unchanged: only the verdict check catches it, and Cargo, given
/// a package holding that script, must show it ran the script to success
/// for that target (its `build-script-executed` report), and
/// `cargo_problem` must refuse the result, whether the build then failed
/// or not. The other names it in its list while its code still refuses
/// it: only the list check catches it.
fn extra_cases(
    planting: &Planting<'_>,
    unlisted: &[String],
    output: &mut impl Write,
) -> Result<(), Error> {
    const ACCEPTED: &str = "a gate whose code accepts one cfg-equal extra triple";
    const LISTED: &str = "a gate list naming one cfg-equal extra triple, while its code refuses it";
    let extra = unlisted.first().ok_or(Error::SelfTest {
        check: CHECK,
        case: ACCEPTED,
        found: vec!["no target has the cfg set of a listed one to plant".to_owned()],
    })?;

    let accepting = plant(
        &planting.script,
        ACCEPTS,
        &format!("({ACCEPTS} || target == \"{extra}\")"),
        ACCEPTED,
    )?;
    let found = planting.gate_found(ACCEPTED, &accepting)?;
    if found.is_empty()
        || !found
            .iter()
            .all(|violation| about(violation, Kind::Verdict, extra))
    {
        return Err(failure(ACCEPTED, &found));
    }
    let package = planting.scratch.join("package");
    write_package(&package, planting.originals, &accepting)?;
    let manifest = package.join("Cargo.toml");
    let planted = crate::metadata::load_in(Some(&manifest), &[], |command| {
        pin::pin_cargo(command, planting.context.pinned);
    })?;
    let run = gate::cargo_run(
        planting.context.pinned,
        planting.context.root,
        &manifest,
        &super::crate_member(&planted)?.id,
        false,
        extra,
        &package.join("target"),
    )?;
    // Whatever Cargo's exit status (a target whose standard library is
    // installed may build), the planted script must have run to success
    // for it, and the check must refuse the result.
    let problem = gate::cargo_problem(&run, extra);
    let Some(problem) = problem.filter(|_| run.script_accepted) else {
        return Err(Error::SelfTest {
            check: CHECK,
            case: ACCEPTED,
            found: vec![format!(
                "Cargo did not report the planted script run to success for {extra}, or the \
                 check accepted the result: {}",
                crate::pure::first_error(run.stderr.as_bytes())
            )],
        });
    };
    report_caught(
        output,
        ACCEPTED,
        &format!("{}; through Cargo ({extra}): {problem}", first(&found)),
    )?;

    let listed = plant(
        &planting.script,
        "const TARGETS: [&str; 7] = [",
        &format!("const TARGETS: [&str; 8] = [\n    \"{extra}\","),
        LISTED,
    )?;
    let listed = plant(
        &listed,
        ACCEPTS,
        &format!("{ACCEPTS} && target != \"{extra}\""),
        LISTED,
    )?;
    let found = planting.gate_found(LISTED, &listed)?;
    if found.is_empty()
        || !found
            .iter()
            .all(|violation| about(violation, Kind::List, extra))
    {
        return Err(failure(LISTED, &found));
    }
    report_caught(output, LISTED, &first(&found))
}

/// The crate root's guard widened by one target, `x86_64-unknown-freebsd`:
/// the guard's verdict disagrees with the list's on that target alone.
fn widened_case(planting: &Planting<'_>, output: &mut impl Write) -> Result<(), Error> {
    const CASE: &str = "a guard widened by one target";
    let planted = plant(
        &planting.lib,
        "        all(\n            target_os = \"macos\",",
        "        all(target_os = \"freebsd\", target_arch = \"x86_64\"),\n        all(\n            \
         target_os = \"macos\",",
        CASE,
    )?;
    let found = guard_found(&planted, planting.context.cfgs, CASE)?;
    if !matches!(found.as_slice(), [violation] if names(violation, "x86_64-unknown-freebsd")) {
        return Err(failure(CASE, &found));
    }
    report_caught(output, CASE, &first(&found))
}

/// A crate-level attribute that removes the crate, and the guard with it,
/// on every target. The guard reader refuses it; the pinned compiler
/// confirms the attribute matters: the crate root up to its guard, built
/// for a toolchain target outside the list, fails with the guard's error
/// as written and builds once the attribute is planted.
fn crate_attribute_case(planting: &Planting<'_>, output: &mut impl Write) -> Result<(), Error> {
    const CASE: &str = "a crate-level attribute that removes the crate and its guard";
    const ATTRIBUTE: &str = "#![cfg_attr(not(target_pointer_width = \"128\"), cfg(any()))]\n";
    let fail = |found: String| Error::SelfTest {
        check: CHECK,
        case: CASE,
        found: vec![found],
    };
    let guard_at = planting
        .lib
        .find("#[cfg(not(all(")
        .ok_or_else(|| fail("the crate root has no guard to plant before".to_owned()))?;
    let planted = format!(
        "{}{ATTRIBUTE}{}",
        planting.lib.get(..guard_at).unwrap_or_default(),
        planting.lib.get(guard_at..).unwrap_or_default()
    );
    let Err(refused) = guard_of(&planted) else {
        return Err(fail(
            "the guard reader accepted the planted root".to_owned(),
        ));
    };
    let toolchain = fs::read_to_string(planting.context.root.join("rust-toolchain.toml"))
        .map_err(Error::path_io("read toolchain file", planting.context.root))?;
    let target = toolchain_targets(&toolchain)?
        .into_iter()
        .find(|target| !ALLOWLIST.contains(&target.as_str()))
        .ok_or_else(|| fail("rust-toolchain.toml names no target outside the list".to_owned()))?;
    let clean = compile_probe(planting, "probe-clean", &planting.lib, &target)?;
    let suppressed = compile_probe(planting, "probe-planted", &planted, &target)?;
    match (clean, suppressed) {
        (Err(error), Ok(())) if error.contains(super::ALLOWLIST_ERROR) => report_caught(
            output,
            CASE,
            &format!(
                "{refused}; the compiler, for {target}, refuses the root as written and builds \
                 it with the attribute"
            ),
        ),
        (clean, suppressed) => Err(fail(format!(
            "the compiler for {target}: as written {clean:?}, with the attribute {suppressed:?}"
        ))),
    }
}

/// Compiles the crate root `lib` up to the end of its guard, alone, for
/// `target` with the pinned compiler; `Err` holds the compiler's errors.
fn compile_probe(
    planting: &Planting<'_>,
    name: &str,
    lib: &str,
    target: &str,
) -> Result<Result<(), String>, Error> {
    const END: &str = "compile_error!(";
    let end = lib
        .find(END)
        .and_then(|start| lib.get(start..)?.find(");").map(|close| start + close + 2))
        .ok_or(Error::SelfTest {
            check: CHECK,
            case: "a crate-level attribute that removes the crate and its guard",
            found: vec!["the crate root has no compile_error! call".to_owned()],
        })?;
    let dir = planting.scratch.join(name);
    let source = dir.join("lib.rs");
    fixture::write_file(
        &source,
        &format!("{}\n", lib.get(..end).unwrap_or_default()),
    )?;
    let output = Command::new(&planting.context.pinned.rustc)
        .current_dir(planting.context.root)
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .args(["--crate-name", "probe", "--target", target, "--out-dir"])
        .arg(&dir)
        .arg(&source)
        .output()
        .map_err(Error::io("run rustc on a crate root probe"))?;
    if output.status.success() {
        Ok(Ok(()))
    } else {
        Ok(Err(String::from_utf8_lossy(&output.stderr).into_owned()))
    }
}

/// `text` with its one `from` replaced by `to`.
fn plant(text: &str, from: &str, to: &str, case: &'static str) -> Result<String, Error> {
    if text.matches(from).count() == 1 {
        Ok(text.replacen(from, to, 1))
    } else {
        Err(Error::SelfTest {
            check: CHECK,
            case,
            found: vec![format!("the text to replace is not there once: {from:?}")],
        })
    }
}

fn first(found: &[Violation]) -> String {
    found.first().map_or_else(String::new, ToString::to_string)
}

/// Writes a package of its own at `dir`: the crate's sources and the
/// planted build script, under a manifest that names it `ironafs-os`.
fn write_package(
    dir: &Path,
    originals: &BTreeMap<PathBuf, String>,
    script: &str,
) -> Result<(), Error> {
    for (path, text) in originals {
        fixture::write_file(&dir.join(path), text)?;
    }
    fixture::write_file(&dir.join(BUILD_SCRIPT), script)?;
    fixture::write_file(
        &dir.join("Cargo.toml"),
        "[package]\nname = \"ironafs-os\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         build = \"build.rs\"\npublish = false\n\n[workspace]\n",
    )
}

/// The crate's `.rs` files, relative to its directory, with their text.
fn read_crate(crate_dir: &Path) -> Result<BTreeMap<PathBuf, String>, Error> {
    let mut files = BTreeMap::new();
    for file in rust_files(crate_dir, &|_| false)? {
        let text = fs::read_to_string(&file).map_err(Error::path_io("read source file", &file))?;
        files.insert(
            file.strip_prefix(crate_dir).unwrap_or(&file).to_path_buf(),
            text,
        );
    }
    Ok(files)
}

/// Applies `edits` to copies of `originals` and runs the comparison. An
/// edit whose text is not found is a failure of the self-test itself: the
/// case would plant nothing.
fn check_edited(
    originals: &BTreeMap<PathBuf, String>,
    edits: &[Edit],
    inventory: &Inventory,
    compiled: &BTreeMap<String, BTreeSet<PathBuf>>,
) -> Result<Vec<Violation>, Error> {
    let mut texts = originals.clone();
    let mut compiled = compiled.clone();
    for edit in edits {
        let path = PathBuf::from(edit.file);
        if edit.from.is_empty() {
            texts.insert(path.clone(), edit.to.to_owned());
            for target in edit.compiled_on {
                if let Some(files) = compiled.get_mut(*target) {
                    files.insert(path.clone());
                }
            }
            continue;
        }
        for target in edit.compiled_on {
            if let Some(files) = compiled.get_mut(*target) {
                files.insert(path.clone());
            }
        }
        let text = texts
            .get_mut(&path)
            .filter(|text| text.contains(edit.from))
            .ok_or(Error::SelfTest {
                check: CHECK,
                case: edit.file,
                found: vec![format!(
                    "the text to plant over is not in the file: {}",
                    edit.from
                )],
            })?;
        *text = text.replacen(edit.from, edit.to, 1);
    }
    let mut files = Vec::new();
    for (path, text) in texts {
        let tokens = tokens_with_strings(&text).map_err(|error| Error::SelfTest {
            check: CHECK,
            case: "reading a planted file",
            found: vec![error.to_string()],
        })?;
        let place = scan::place_of(&path);
        files.push(CrateFile {
            path,
            scan: scan::scan(&tokens, place),
        });
    }
    Ok(compare(inventory, &files, &compiled))
}

/// Writes a small tree, `ironafs-os` and another crate, and proves the
/// confinement rule finds `unsafe`, a foreign declaration and `#[link]` in
/// the other crate and nothing in the crate allowed to hold them, nor in a
/// string or a comment. Returns the number of cases.
fn outside_cases(target_dir: &Path) -> Result<usize, Error> {
    let dir = fixture::fresh_dir(target_dir, CHECK)?;
    let result = outside_in(&dir);
    fixture::remove_dir(&dir)?;
    result
}

fn outside_in(dir: &Path) -> Result<usize, Error> {
    let os_dir = dir.join("crates/ironafs-os");
    fixture::write_file(
        &os_dir.join("src/lib.rs"),
        "fn f() { unsafe { g() } }\n#[link(name = \"c\")]\nunsafe extern \"C\" { fn g(); }\n",
    )?;
    fixture::write_file(
        &dir.join("crates/other/src/lib.rs"),
        "//! Says unsafe only here.\nconst S: &str = \"unsafe extern\";\nextern crate alloc;\n",
    )?;
    let clean = outside_violations(dir, &os_dir)?;
    if !clean.is_empty() {
        return Err(failure("confinement: the clean tree", &clean));
    }
    let planted = [
        (
            "an unsafe block outside the crate",
            "fn f() { unsafe { g() } }\n",
        ),
        (
            "a foreign declaration outside the crate",
            "unsafe extern \"C\" { fn g(); }\n",
        ),
        (
            "a #[link] outside the crate",
            "#[link(name = \"c\")]\nfn f() {}\n",
        ),
        (
            "an unsafe impl in a test file outside the crate",
            "unsafe impl Send for S {}\n",
        ),
    ];
    for (name, source) in planted {
        let file = dir.join("crates/other/tests/planted.rs");
        fixture::write_file(&file, source)?;
        let found = outside_violations(dir, &os_dir)?;
        let all_planted = !found.is_empty()
            && found.iter().all(|violation| {
                matches!(violation, Violation::Outside { file, .. }
                    if file == Path::new("crates/other/tests/planted.rs"))
            });
        if !all_planted {
            return Err(failure(name, &found));
        }
    }
    Ok(planted.len() + 1)
}

fn failure(case: &'static str, found: &[Violation]) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.iter().map(ToString::to_string).collect(),
    }
}
