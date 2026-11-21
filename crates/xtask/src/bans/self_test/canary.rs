// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canary's self-test: a workspace holding only the canary, linted as
//! the real workspace is, clean and then under each way around the bans
//! that can be planted, each of which must leave a canary line silent.

use std::path::Path;

use super::super::{Found, Violation, canary, lint};
use super::failure;
use crate::fixture::write_file;
use crate::{Command, Error, metadata};

/// One planted evasion: its name, the files it writes under the
/// workspace (`{root}` in a text stands for the workspace's directory),
/// the variables it sets for cargo, and the violation it must give.
struct Case {
    name: &'static str,
    files: &'static [(&'static str, &'static str)],
    env: &'static [(&'static str, &'static str)],
    needs: fn(&Violation) -> bool,
}

fn silent(violation: &Violation) -> bool {
    matches!(violation, Violation::CanaryMissing { .. })
}

fn tampered(violation: &Violation) -> bool {
    matches!(violation, Violation::Canary(_))
}

/// A compiler wrapper that drops clippy and runs the compiler alone.
const WRAPPER: &str = "#!/bin/sh\nfor arg; do shift; case \"$arg\" in *clippy-driver*) ;; *) set -- \"$@\" \"$arg\";; esac; done\nexec \"$@\"\n";

const CASES: &[Case] = &[
    Case {
        name: "a cargo alias that makes clippy a plain check",
        files: &[(".cargo/config.toml", "[alias]\nclippy = \"check\"\n")],
        env: &[],
        needs: silent,
    },
    Case {
        name: "CARGO_ALIAS_CLIPPY making clippy a plain check",
        files: &[],
        env: &[("CARGO_ALIAS_CLIPPY", "check")],
        needs: silent,
    },
    Case {
        name: "RUSTFLAGS allowing the bans' lints",
        files: &[],
        env: &[(
            "RUSTFLAGS",
            "-Aclippy::disallowed_methods -Aclippy::disallowed_types",
        )],
        needs: silent,
    },
    Case {
        name: "a compiler wrapper that runs the compiler without clippy",
        files: &[
            ("wrap.sh", WRAPPER),
            (
                ".cargo/config.toml",
                "[build]\nrustc-wrapper = \"{root}/wrap.sh\"\n",
            ),
        ],
        env: &[],
        needs: silent,
    },
    Case {
        name: "a byte-order-marked cargo configuration whose [env] sets CLIPPY_CONF_DIR",
        files: &[
            (
                ".cargo/config.toml",
                "\u{feff}[env]\nCLIPPY_CONF_DIR = { value = \"empty\", relative = true, force = true }\n",
            ),
            ("empty/clippy.toml", ""),
        ],
        env: &[],
        needs: silent,
    },
    Case {
        name: "an expect attribute added to the canary",
        files: &[(
            "crates/ironafs-bans-canary/src/lib.rs",
            "#![expect(clippy::disallowed_methods)]\n",
        )],
        env: &[],
        needs: tampered,
    },
    Case {
        name: "a clippy.toml in the canary's directory",
        files: &[("crates/ironafs-bans-canary/clippy.toml", "")],
        env: &[],
        needs: tampered,
    },
];

/// Writes a workspace under `dir` holding only the canary, with the
/// repository's `root_manifest`, `clippy_toml` and `toolchain_toml`, then
/// `files`, and lints it as the real workspace is, with `env` set.
fn lint_planted(
    dir: &Path,
    texts: (&str, &str, &str),
    files: &[(&str, &str)],
    env: &[(&str, &str)],
) -> Result<Vec<Found>, Error> {
    let (root_manifest, clippy_toml, toolchain_toml) = texts;
    let root = dir.to_string_lossy().into_owned();
    let canary_files = [
        ("Cargo.toml", root_manifest.to_owned()),
        ("clippy.toml", clippy_toml.to_owned()),
        ("rust-toolchain.toml", toolchain_toml.to_owned()),
        ("crates/ironafs-bans-canary/Cargo.toml", canary::manifest()),
        ("crates/ironafs-bans-canary/src/lib.rs", canary::source()),
    ];
    for (file, text) in canary_files {
        write_file(&dir.join(file), &text)?;
    }
    for (file, text) in files {
        let path = dir.join(file);
        write_file(&path, &text.replace("{root}", &root))?;
        #[cfg(unix)]
        executable(&path)?;
    }
    let workspace = metadata::load(Some(&dir.join("Cargo.toml")), &[])?;
    lint(&workspace, |command: &mut Command| {
        for (name, value) in env {
            command.env(name, value);
        }
    })
}

/// Makes a planted script runnable.
#[cfg(unix)]
fn executable(path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(Error::path_io("make runnable", path))
}

/// Proves the canary's workspace lints clean, and each planted evasion
/// leaves it silent or is refused. The wrapper is a shell script, so that
/// case runs only where one can.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
pub(super) fn run(
    dir: &Path,
    texts: (&str, &str, &str),
    report: &mut impl FnMut(String) -> Result<(), Error>,
) -> Result<(), Error> {
    let clean = "a workspace whose lint draws every canary warning";
    let found = lint_planted(&dir.join("clean"), texts, &[], &[])?;
    if !found.is_empty() {
        return Err(failure(clean, &found));
    }
    report(format!("passed: {clean}"))?;
    for (index, case) in CASES.iter().enumerate() {
        if cfg!(not(unix)) && case.files.iter().any(|(file, _)| *file == "wrap.sh") {
            report(format!("not run here: {}", case.name))?;
            continue;
        }
        let found = lint_planted(
            &dir.join(format!("case-{index}")),
            texts,
            case.files,
            case.env,
        )?;
        if !found.iter().any(|found| (case.needs)(&found.violation)) {
            return Err(failure(case.name, &found));
        }
        for violation in found.iter().filter(|found| (case.needs)(&found.violation)) {
            report(format!("caught: {}: {violation}", case.name))?;
        }
    }
    Ok(())
}
