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
//! either caught as in `clippy.toml`; and proves a ban written in a format
//! the check cannot read, and a ban key spelt with a TOML escape, refused
//! as unreadable. It builds crates whose build scripts set
//! `CLIPPY_CONF_DIR`, and `Clippy_Conf_Dir`, for their compilers, and one
//! whose build script writes a `clippy.toml`, and proves each refused,
//! beside one whose build script sets another variable, which passes.
//! Last, it plants cargo configurations: one in a workspace whose `[env]`
//! sets `CLIPPY_CONF_DIR` under an escaped key, one in cargo's home that
//! sets it by a dotted key, and one cargo would read that cannot be read,
//! and proves each refused, beside one whose `[env]` sets another
//! variable, which passes.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::{
    CHECK, CONF_DIR, Found, Lint, Violation, cargo_env, check_build_scripts, check_tree,
    unchanged_by,
};
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
        // TOML decodes the escape, so clippy reads the array as its bans.
        name: "an escaped ban key in clippy.toml whose array has an unplanted ban",
        place: Place::Root,
        plant: |text| {
            with_unplanted(text)
                .replace("disallowed-methods = [", "\"disallowed\\u002dmethods\" = [")
        },
        expect: |found| {
            found.file == ROOT_FILE && matches!(found.violation, Violation::Unreadable { .. })
        },
        needs: |_| true,
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
    cargo_configs(&dir.join("cargo"), &mut report)?;
    report(format!(
        "check-bans --self-test: the repository's bans passed and {} planted cases failed as \
         planted",
        CASES.len() + 1
    ))
}

/// The build-script case: a crate under `dir` whose build script runs
/// `body`.
fn build_script_crate(dir: &Path, body: &str, toolchain_toml: &str) -> Result<(), Error> {
    let files = [
        (
            "Cargo.toml",
            "[package]\nname = \"planted\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
             # Not part of the workspace whose target directory holds it.\n[workspace]\n"
                .to_owned(),
        ),
        ("build.rs", format!("fn main() {{\n    {body}\n}}\n")),
        ("src/lib.rs", "pub fn planted() {}\n".to_owned()),
        ("rust-toolchain.toml", toolchain_toml.to_owned()),
    ];
    for (file, text) in files {
        write_file(&dir.join(file), &text)?;
    }
    Ok(())
}

/// A build script's body that sets `name` for its crate's compiler, to a
/// directory that exists and holds no configuration.
fn sets(name: &str) -> String {
    format!(
        "let dir = std::env::var(\"OUT_DIR\").unwrap_or_default();\n    \
         println!(\"cargo::rustc-env={name}={{dir}}\");"
    )
}

/// A build script's body that writes an empty `clippy.toml`, which bans
/// nothing, beside its crate's manifest.
const WRITES_CONFIG: &str = "let dir = std::env::var(\"CARGO_MANIFEST_DIR\").unwrap_or_default();\n    \
     std::fs::write(std::path::Path::new(&dir).join(\"clippy.toml\"), \"\").unwrap_or_default();";

/// Builds a crate under `dir` whose build script runs `body` as the check
/// builds the workspace, and returns what it found.
fn build_planted(dir: &Path, body: &str, toolchain_toml: &str) -> Result<Vec<Found>, Error> {
    build_script_crate(dir, body, toolchain_toml)?;
    unchanged_by(dir, &[dir.to_path_buf()], || {
        check_build_scripts(dir, &dir.join("target"))
    })
}

/// Proves a build script that sets `CLIPPY_CONF_DIR`, in any case, or
/// writes a configuration, is refused, and one that sets another variable
/// is not.
fn build_scripts(
    dir: &Path,
    toolchain_toml: &str,
    report: &mut impl FnMut(String) -> Result<(), Error>,
) -> Result<(), Error> {
    let clean = "a build script that sets another variable";
    let found = build_planted(&dir.join("clean"), &sets("PLANTED"), toolchain_toml)?;
    if !found.is_empty() {
        return Err(failure(clean, &found));
    }
    report(format!("passed: {clean}"))?;
    let mut caught = |case: &'static str, found: &[Found], refused: &dyn Fn(&Found) -> bool| {
        if found.is_empty() || !found.iter().all(refused) {
            return Err(failure(case, found));
        }
        for violation in found {
            report(format!("caught: {case}: {violation}"))?;
        }
        Ok(())
    };
    // Windows reads environment names without regard to case.
    for (case, name, place) in [
        (
            "a build script that sets CLIPPY_CONF_DIR for its compiler",
            CONF_DIR,
            "planted",
        ),
        (
            "a build script that sets Clippy_Conf_Dir for its compiler",
            "Clippy_Conf_Dir",
            "planted-case",
        ),
    ] {
        let found = build_planted(&dir.join(place), &sets(name), toolchain_toml)?;
        caught(
            case,
            &found,
            &|found| matches!(&found.violation, Violation::BuildOverride { name: set, .. } if set == name),
        )?;
    }
    let case = "a build script that writes a clippy.toml beside its manifest";
    let found = build_planted(&dir.join("planted-write"), WRITES_CONFIG, toolchain_toml)?;
    caught(case, &found, &|found| {
        found.file == ROOT_FILE && found.violation == Violation::Changed
    })
}

/// A planted cargo configuration: its case, the file under the case's
/// directory, its text, and what each refusal of it must be.
type CargoCase = (
    &'static str,
    &'static str,
    &'static str,
    fn(&Violation) -> bool,
);

/// Proves a cargo configuration whose `[env]` table sets `CLIPPY_CONF_DIR`,
/// in the workspace or in cargo's home, or one cargo would read that
/// cannot be read, is refused, and one that sets another variable is not.
fn cargo_configs(
    dir: &Path,
    report: &mut impl FnMut(String) -> Result<(), Error>,
) -> Result<(), Error> {
    // Cargo's home is planted too, empty unless a case writes to it.
    let check = |case: &str, file: Option<(&str, &str)>| -> Result<Vec<Found>, Error> {
        let tree = dir.join(case);
        let home = tree.join("home");
        if let Some((file, text)) = file {
            write_file(&tree.join(file), text)?;
        }
        Ok(cargo_env::check(&[tree.join("workspace")], Some(&home)))
    };
    let clean = "a workspace cargo configuration whose [env] sets another variable";
    let found = check(
        "clean",
        Some(("workspace/.cargo/config.toml", "[env]\nPLANTED = \"x\"\n")),
    )?;
    if !found.is_empty() {
        return Err(failure(clean, &found));
    }
    report(format!("passed: {clean}"))?;
    let planted: [CargoCase; 3] = [
        (
            "a workspace cargo configuration whose [env] sets CLIPPY_CONF_DIR, escaped",
            "workspace/.cargo/config.toml",
            "[env]\n\"CLIPPY\\u005fCONF_DIR\" = { value = \"x\", force = true }\n",
            |violation| matches!(violation, Violation::CargoEnv { name, .. } if name == CONF_DIR),
        ),
        (
            "cargo's home configuration setting env.CLIPPY_CONF_DIR",
            "home/config.toml",
            "env.CLIPPY_CONF_DIR = \"x\"\n",
            |violation| matches!(violation, Violation::CargoEnv { name, .. } if name == CONF_DIR),
        ),
        (
            "a workspace cargo configuration check-bans cannot read",
            "workspace/.cargo/config.toml/unreadable",
            "",
            |violation| matches!(violation, Violation::CargoUnread(_)),
        ),
    ];
    for (index, (case, file, text, refused)) in planted.into_iter().enumerate() {
        let found = check(&format!("case-{index}"), Some((file, text)))?;
        let planted_file = |found: &Found| {
            found.file.ends_with(file.trim_end_matches("/unreadable")) && refused(&found.violation)
        };
        if found.is_empty() || !found.iter().all(planted_file) {
            return Err(failure(case, &found));
        }
        for violation in &found {
            report(format!("caught: {case}: {violation}"))?;
        }
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
