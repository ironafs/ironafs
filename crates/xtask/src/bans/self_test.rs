// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-bans --self-test`: runs the check on a workspace holding the
//! repository's `clippy.toml`, which must pass, then on copies with a
//! method ban removed, a type ban removed, a ban misspelt and a ban no
//! planted line proves, on a crate whose own `clippy.toml` leaves out a
//! ban, and with `CLIPPY_CONF_DIR` naming a directory whose configuration
//! leaves out a ban, and proves the check fails on each, naming that ban
//! and that file, or the variable. It also reads a ban array with an
//! indented key, and a root `.clippy.toml`, and proves an unplanted ban in
//! either caught as in `clippy.toml`; and it builds a crate whose build script sets
//! `CLIPPY_CONF_DIR` for its compiler and proves that refused, beside one
//! whose build script sets another variable, which passes.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::{CHECK, CONF_DIR, Found, Lint, Violation, check_build_scripts, check_tree};
use crate::Error;
use crate::fixture::{self, write_file};

/// The ban each planted case breaks.
const REMOVED: &str = "std::time::SystemTime::now";
const REMOVED_TYPE: &str = "std::process::Command";
const MISSPELT: &str = "std::time::Instant::elapsed";
const LEFT_OUT: &str = "std::collections::HashMap";
/// A ban clippy resolves, which no planted line reaches.
const UNPLANTED: &str = "std::thread::sleep";

/// The workspace's own configuration, and the one a planted crate's own
/// configuration is written to.
const ROOT_FILE: &str = "clippy.toml";
/// The root name clippy prefers when both are present.
const DOT_ROOT_FILE: &str = ".clippy.toml";
const CRATE_DIR: &str = "crates/planted";
const CRATE_FILE: &str = "crates/planted/clippy.toml";

/// Where a case writes its planted configuration.
#[derive(Clone, Copy)]
enum Place {
    /// In place of the workspace's `clippy.toml`.
    Root,
    /// As the workspace's `.clippy.toml`, beside its `clippy.toml`.
    DotRoot,
    /// As a member crate's own `clippy.toml`, beside the workspace's.
    Crate,
    /// In a directory outside the workspace that `CLIPPY_CONF_DIR` names.
    ConfDir,
}

/// A planted configuration, what every violation it gives must be, and
/// one violation it must give.
struct Case {
    name: &'static str,
    place: Place,
    plant: fn(&str) -> String,
    expect: fn(&Found) -> bool,
    needs: fn(&Found) -> bool,
}

const CASES: &[Case] = &[
    Case {
        name: "a ban removed from clippy.toml",
        place: Place::Root,
        plant: |text| without(text, REMOVED),
        expect: |found| found.file == ROOT_FILE && names(&found.violation, REMOVED),
        // Clippy itself stops firing, not only the listing check.
        needs: |found| matches!(found.violation, Violation::NotFired { path, .. } if path == REMOVED),
    },
    Case {
        name: "a type ban removed from clippy.toml",
        place: Place::Root,
        plant: |text| without(text, REMOVED_TYPE),
        expect: |found| found.file == ROOT_FILE && names(&found.violation, REMOVED_TYPE),
        needs: |found| matches!(found.violation, Violation::NotFired { path, .. } if path == REMOVED_TYPE),
    },
    Case {
        name: "a ban misspelt in clippy.toml",
        place: Place::Root,
        plant: |text| text.replace(&format!("\"{MISSPELT}\""), &format!("\"{MISSPELT}x\"")),
        expect: |found| {
            found.file == ROOT_FILE
                && (names(&found.violation, MISSPELT)
                    || matches!(&found.violation, Violation::Unresolved(line)
                        if line.contains(&format!("{MISSPELT}x")))
                    || matches!(&found.violation, Violation::Uncovered { path, .. }
                        if *path == format!("{MISSPELT}x")))
        },
        // Clippy's own warning that the path names nothing is caught.
        needs: |found| matches!(found.violation, Violation::Unresolved(_)),
    },
    Case {
        // Clippy reads only the nearest configuration, so a crate's own
        // file replaces the workspace's for that crate.
        name: "a crate's own clippy.toml that leaves out a ban",
        place: Place::Crate,
        plant: |text| without(text, LEFT_OUT),
        expect: |found| found.file == CRATE_FILE && names(&found.violation, LEFT_OUT),
        needs: |found| matches!(found.violation, Violation::NotFired { path, .. } if path == LEFT_OUT),
    },
    Case {
        // Clippy would enforce it, but nothing would prove it still does.
        name: "a ban in clippy.toml that no planted line reaches",
        place: Place::Root,
        plant: with_unplanted,
        expect: |found| found.file == ROOT_FILE && unplanted(&found.violation),
        needs: |_| true,
    },
    Case {
        // Indenting the key is valid TOML; the array is still read.
        name: "an indented ban key in clippy.toml whose array has an unplanted ban",
        place: Place::Root,
        plant: |text| {
            with_unplanted(text).replace("disallowed-methods = [", "  disallowed-methods = [")
        },
        expect: |found| found.file == ROOT_FILE && unplanted(&found.violation),
        needs: |_| true,
    },
    Case {
        name: "a ban in a format check-bans cannot read",
        place: Place::Root,
        plant: |text| {
            text.replace(
                "disallowed-methods = [\n",
                &format!("disallowed-methods = [\n  \"{UNPLANTED}\",\n"),
            )
        },
        expect: |found| {
            found.file == ROOT_FILE
                && (matches!(found.violation, Violation::Unreadable { .. })
                    || matches!(&found.violation, Violation::Clippy(_)))
        },
        needs: |found| matches!(found.violation, Violation::Unreadable { .. }),
    },
    Case {
        // Clippy prefers `.clippy.toml` when both names are in a directory.
        name: "a root .clippy.toml with a ban no planted line reaches",
        place: Place::DotRoot,
        plant: with_unplanted,
        expect: |found| found.file == DOT_ROOT_FILE && unplanted(&found.violation),
        needs: |_| true,
    },
    Case {
        // Clippy reads the directory the variable names in place of every
        // crate's own configuration, so the files checked would not be the
        // ones linted with.
        name: "CLIPPY_CONF_DIR naming a configuration that leaves out a ban",
        place: Place::ConfDir,
        plant: |text| without(text, LEFT_OUT),
        expect: |found| {
            found.file == CONF_DIR && matches!(found.violation, Violation::Overridden(_))
        },
        needs: |_| true,
    },
];

/// `text` with a method ban on `UNPLANTED` added first to its methods.
fn with_unplanted(text: &str) -> String {
    text.replace(
        "disallowed-methods = [\n",
        &format!("disallowed-methods = [\n  {{ path = \"{UNPLANTED}\", reason = \"r\" }},\n"),
    )
}

/// Whether `found` is the method ban on `UNPLANTED` that no line proves.
fn unplanted(found: &Violation) -> bool {
    matches!(found, Violation::Uncovered { lint: Lint::Method, path } if path == UNPLANTED)
}

/// `text` without the lines that name `path` as an entry.
fn without(text: &str, path: &str) -> String {
    text.lines()
        .filter(|line| !line.contains(&format!("\"{path}\"")))
        .flat_map(|line| [line, "\n"])
        .collect()
}

/// Whether `found` is a missing or silent ban on `path`.
fn names(found: &Violation, path: &str) -> bool {
    matches!(found, Violation::NotListed { path: named } | Violation::NotFired { path: named, .. }
        if *named == path)
}

/// Writes a workspace under `dir` with `root` as its `clippy.toml` and one
/// member crate, with the planted text written where its place says, and
/// checks it: in place of `root`, as the root's `.clippy.toml`, as the
/// crate's own `clippy.toml`, or in a directory outside the workspace that
/// `CLIPPY_CONF_DIR` names.
fn check_planted(
    dir: &Path,
    root: &str,
    planted: (Place, &str),
    toolchain_toml: &str,
) -> Result<Vec<Found>, Error> {
    let (place, text) = planted;
    let own = matches!(place, Place::Crate).then_some(text);
    let elsewhere = matches!(place, Place::ConfDir).then_some(text);
    let root = if matches!(place, Place::Root) {
        text
    } else {
        root
    };
    let tree = dir.join("tree");
    write_file(&tree.join(ROOT_FILE), root)?;
    if matches!(place, Place::DotRoot) {
        write_file(&tree.join(DOT_ROOT_FILE), text)?;
    }
    write_file(&tree.join(CRATE_DIR).join("Cargo.toml"), "")?;
    if let Some(own) = own {
        write_file(&tree.join(CRATE_FILE), own)?;
    }
    let conf_dir = dir.join("elsewhere");
    if let Some(elsewhere) = elsewhere {
        write_file(&conf_dir.join(ROOT_FILE), elsewhere)?;
    }
    let members: Vec<PathBuf> = vec![tree.join(CRATE_DIR)];
    check_tree(
        &tree,
        &members,
        elsewhere.map(|_| conf_dir.as_os_str()),
        toolchain_toml,
        &dir.join("lint"),
    )
}

/// Runs the clean configuration and every planted case.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
pub(super) fn run(
    target_dir: &Path,
    clippy_toml: &str,
    toolchain_toml: &str,
    output: &mut impl Write,
) -> Result<(), Error> {
    let dir = fixture::fresh_dir(target_dir, CHECK)?;
    let result = run_in(&dir, clippy_toml, toolchain_toml, output);
    fixture::remove_dir(&dir)?;
    result
}

fn run_in(
    dir: &Path,
    clippy_toml: &str,
    toolchain_toml: &str,
    output: &mut impl Write,
) -> Result<(), Error> {
    let mut report =
        |line: String| writeln!(output, "{line}").map_err(Error::io("report check-bans self-test"));
    let clean = "the repository's clippy.toml";
    // The clean case plants the repository's file in its own place.
    let found = check_planted(
        &dir.join("clean"),
        clippy_toml,
        (Place::Root, clippy_toml),
        toolchain_toml,
    )?;
    if !found.is_empty() {
        return Err(failure(clean, &found));
    }
    report(format!("passed: {clean}"))?;
    for (index, case) in CASES.iter().enumerate() {
        let planted = (case.plant)(clippy_toml);
        if planted == clippy_toml {
            return Err(failure(case.name, &[]));
        }
        let case_dir = dir.join(format!("case-{index}"));
        let found = check_planted(
            &case_dir,
            clippy_toml,
            (case.place, &planted),
            toolchain_toml,
        )?;
        match found.first() {
            Some(_) if found.iter().all(case.expect) && found.iter().any(case.needs) => {
                for violation in &found {
                    report(format!("caught: {}: {violation}", case.name))?;
                }
            }
            _ => return Err(failure(case.name, &found)),
        }
    }
    build_scripts(&dir.join("build"), toolchain_toml, &mut report)?;
    report(format!(
        "check-bans --self-test: the repository's bans passed and {} planted cases failed as \
         planted",
        CASES.len() + 1
    ))
}

/// The build-script case: a crate whose build script sets `name` for its
/// compiler, under `dir`.
fn build_script_crate(dir: &Path, name: &str, toolchain_toml: &str) -> Result<(), Error> {
    let files = [
        (
            "Cargo.toml",
            "[package]\nname = \"planted\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
             # Not part of the workspace whose target directory holds it.\n[workspace]\n"
                .to_owned(),
        ),
        (
            "build.rs",
            // A directory that exists and holds no configuration.
            format!(
                "fn main() {{\n    let dir = std::env::var(\"OUT_DIR\").unwrap_or_default();\n    \
                 println!(\"cargo::rustc-env={name}={{dir}}\");\n}}\n"
            ),
        ),
        ("src/lib.rs", "pub fn planted() {}\n".to_owned()),
        ("rust-toolchain.toml", toolchain_toml.to_owned()),
    ];
    for (file, text) in files {
        write_file(&dir.join(file), &text)?;
    }
    Ok(())
}

/// Proves a build script that sets `CLIPPY_CONF_DIR` is refused, and one
/// that sets another variable is not.
fn build_scripts(
    dir: &Path,
    toolchain_toml: &str,
    report: &mut impl FnMut(String) -> Result<(), Error>,
) -> Result<(), Error> {
    let clean = "a build script that sets another variable";
    let clean_dir = dir.join("clean");
    build_script_crate(&clean_dir, "PLANTED", toolchain_toml)?;
    let found = check_build_scripts(&clean_dir, &clean_dir.join("target"))?;
    if !found.is_empty() {
        return Err(failure(clean, &found));
    }
    report(format!("passed: {clean}"))?;
    let case = "a build script that sets CLIPPY_CONF_DIR for its compiler";
    let case_dir = dir.join("planted");
    build_script_crate(&case_dir, CONF_DIR, toolchain_toml)?;
    let found = check_build_scripts(&case_dir, &case_dir.join("target"))?;
    let refused = |found: &Found| matches!(&found.violation, Violation::BuildOverride { name, .. } if name == CONF_DIR);
    if found.is_empty() || !found.iter().all(refused) {
        return Err(failure(case, &found));
    }
    for violation in &found {
        report(format!("caught: {case}: {violation}"))?;
    }
    Ok(())
}

fn failure(case: &'static str, found: &[Found]) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.iter().map(ToString::to_string).collect(),
    }
}
