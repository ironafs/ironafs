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

const SYSCALL: &str = "unsafe { abi::syscall(number, pointer, length, FLAGS) }";
const WIPE_BLOCK: &str = "unsafe { core::ptr::write_volatile(byte, 0) }";
const WIPE_NAME: &str = "#[expect(unsafe_code, reason = \"mem-wipe\")]\n";
const STRAY_WIPE: &str = "fn zero(byte: *mut u8) {\n\
                          #[expect(unsafe_code, reason = \"mem-wipe\")]\n\
                          // SAFETY: planted.\n\
                          let () = unsafe { core::ptr::write_volatile(byte, 0) };\n}\n";

const CASES: &[Case] = &[
    Case {
        name: "a declaration swapped for another, the block count unchanged",
        edits: &[
            replace(
                "src/linux_clock/abi.rs",
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
            "src/linux_clock/abi.rs",
            "-> c_int;\n}",
            "-> c_int;\n    pub(super) fn clock_getres(clockid: c_int, res: *mut timespec) -> c_int;\n}",
        )],
        expect: &[|found| mismatch(found, "linux-clock")],
    },
    Case {
        name: "code for a row that is only approved",
        edits: &[add(
            "src/linux_peercred.rs",
            "#[expect(unsafe_code, reason = \"linux-peercred\")]\n\
             unsafe extern \"C\" { fn getsockopt(fd: c_int) -> c_int; }\n",
            LINUX,
        )],
        expect: &[|found| mismatch(found, "linux-peercred")],
    },
    Case {
        name: "an item of an implemented row removed",
        edits: &[replace("src/linux_random.rs", SYSCALL, "-1")],
        expect: &[|found| mismatch(found, "linux-random")],
    },
    Case {
        name: "a second foreign call in a block, its id and the counts unchanged",
        edits: &[replace(
            "src/linux_random.rs",
            SYSCALL,
            "unsafe { abi::syscall(number, pointer, length, FLAGS); abi::syscall(number, pointer, length, FLAGS) }",
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
            "src/linux_clock/abi.rs",
            "/// `CLOCK_MONOTONIC`",
            "#[cfg(target_arch = \"x86_64\")]\nconst PLANTED: u8 = 0;\n\n/// `CLOCK_MONOTONIC`",
        )],
        expect: &[|found| rule(found, CFG)],
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
    target_dir: &Path,
    crate_dir: &Path,
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

    let cases = outside_cases(target_dir)?;
    report(
        output,
        format!(
            "check-os --self-test: the crate's own files passed and {} planted cases failed as \
             planted",
            CASES.len() + 1 + cases
        ),
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
        let root = path == Path::new("src/lib.rs");
        files.push(CrateFile {
            path,
            scan: scan::scan(&tokens, root),
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
