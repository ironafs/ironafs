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

use super::inventory::Inventory;
use super::scan::{self, BLOCK_BODY, CFG, TARGET_ENV};
use super::{CrateFile, Violation, compare, outside_violations, rust_files};
use crate::rust_source::tokens_with_strings;
use crate::{Error, fixture};

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
];

/// Runs the clean copies, every planted case, a target whose build read no
/// site file, and the confinement cases.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
pub(super) fn run(
    root: &Path,
    target_dir: &Path,
    crate_dir: &Path,
    adr: &str,
    inventory: &Inventory,
    compiled: &BTreeMap<String, BTreeSet<PathBuf>>,
    output: &mut impl Write,
) -> Result<(), Error> {
    let report = |output: &mut dyn Write, line: String| {
        writeln!(output, "{line}").map_err(Error::io("report check-os self-test"))
    };
    let originals = read_crate(crate_dir)?;

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

    paired_case(&originals, adr, inventory, compiled)?;
    report(
        output,
        "caught: a declaration and its call changed in the code and in sites.json alike, to one \
         the ADR does not name"
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

    allowlist_case(root, crate_dir)?;
    report(
        output,
        "caught: a crate root guard without its vendor and pointer-width predicates".to_owned(),
    )?;

    let cases = outside_cases(target_dir)?;
    report(
        output,
        format!(
            "check-os --self-test: the crate's own files passed and {} planted cases failed as \
             planted",
            CASES.len() + 3 + cases
        ),
    )
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
fn allowlist_case(root: &Path, crate_dir: &Path) -> Result<(), Error> {
    const CASE: &str = "a crate root guard without its vendor and pointer-width predicates";
    let lib = crate_dir.join("src/lib.rs");
    let text = fs::read_to_string(&lib).map_err(Error::path_io("read crate root", &lib))?;
    let mut weakened = text.clone();
    for predicate in [
        "target_pointer_width = \"64\",",
        "target_endian = \"little\",",
        "target_vendor = \"unknown\",",
        "target_vendor = \"pc\",",
    ] {
        weakened = weakened.replace(predicate, "");
    }
    if weakened == text {
        return Err(Error::SelfTest {
            check: CHECK,
            case: CASE,
            found: vec!["the crate root has none of the predicates to remove".to_owned()],
        });
    }
    let accepted = match super::allowlist::accepted_targets(root, &weakened)? {
        Ok(accepted) => accepted,
        Err(error) => {
            return Err(Error::SelfTest {
                check: CHECK,
                case: CASE,
                found: vec![error],
            });
        }
    };
    let caught = ["x86_64-unikraft-linux-musl", "x86_64-win7-windows-msvc"]
        .iter()
        .all(|target| accepted.contains_key(*target));
    if caught {
        Ok(())
    } else {
        Err(Error::SelfTest {
            check: CHECK,
            case: CASE,
            found: accepted.keys().cloned().collect(),
        })
    }
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
