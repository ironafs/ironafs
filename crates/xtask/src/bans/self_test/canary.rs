// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canary's self-test: a workspace holding only the canary, linted as
//! the real workspace is, clean and then under each way around the bans
//! that can be planted, each of which must leave a canary line silent.

use std::path::Path;

use super::super::{Found, Violation, canary, line_keys, lint};
use super::failure;
use crate::fixture::write_file;
use crate::{Command, Error, metadata};

#[cfg(test)]
mod tests;

/// One planted evasion: its name, the files it writes under the
/// workspace and the variables it sets for cargo (`{root}` in either
/// stands for the workspace's directory), and the violation it must give.
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

fn unused_key(violation: &Violation) -> bool {
    matches!(violation, Violation::Lint(line) if line.contains("unused manifest key"))
}

fn script_warning(violation: &Violation) -> bool {
    matches!(violation, Violation::Lint(line) if line.contains("planted-warning"))
}

/// A second member's manifest, and the same with a key cargo does not use.
const PLANTED_MANIFEST: &str = "[package]\nname = \"planted\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[lints]\nworkspace = true\n";
const PLANTED_UNUSED_KEY: &str = "[package]\nname = \"planted\"\nversion = \"0.0.0\"\nedition = \"2024\"\nbogus = 1\n\n[lints]\nworkspace = true\n";

/// A second member whose build script prints a warning.
const WARNING_SCRIPT: &[(&str, &str)] = &[
    ("crates/planted/Cargo.toml", PLANTED_MANIFEST),
    (
        "crates/planted/build.rs",
        "fn main() {\n    println!(\"cargo::warning=planted-warning\");\n}\n",
    ),
    ("crates/planted/src/lib.rs", ""),
];

/// `WARNING_SCRIPT` with a cargo configuration that asks for quiet.
const WARNING_SCRIPT_QUIET: &[(&str, &str)] = &[
    ("crates/planted/Cargo.toml", PLANTED_MANIFEST),
    (
        "crates/planted/build.rs",
        "fn main() {\n    println!(\"cargo::warning=planted-warning\");\n}\n",
    ),
    ("crates/planted/src/lib.rs", ""),
    (".cargo/config.toml", "[term]\nquiet = true\n"),
];

fn banned_call_linted(violation: &Violation) -> bool {
    matches!(violation, Violation::Lint(line) if line.contains("disallowed method"))
}

/// A second member that calls a banned item only behind a feature no
/// build enables.
const FEATURE_GATED: &[(&str, &str)] = &[
    (
        "crates/planted/Cargo.toml",
        "[package]\nname = \"planted\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [features]\nhidden = []\n\n[lints]\nworkspace = true\n",
    ),
    (
        "crates/planted/src/lib.rs",
        "#[cfg(feature = \"hidden\")]\n#[must_use]\npub fn hidden() -> std::time::Instant {\n    \
         std::time::Instant::now()\n}\n",
    ),
];

/// A compiler wrapper that runs clippy for the canary alone, and drops it
/// to run the compiler alone for every other crate.
const WRAPPER: &str = "#!/bin/sh\ncase \" $* \" in *\" --crate-name ironafs_bans_canary \"*) exec \"$@\";; esac\nfor arg; do shift; case \"$arg\" in *clippy-driver*) ;; *) set -- \"$@\" \"$arg\";; esac; done\nexec \"$@\"\n";

/// A second member that calls a banned item.
const BANNED_CALL: (&str, &str) = (
    "crates/planted/src/lib.rs",
    "#[must_use]\npub fn now() -> std::time::Instant {\n    std::time::Instant::now()\n}\n",
);

const CASES: &[Case] = &[
    Case {
        name: "a banned call behind a feature no build enables",
        files: FEATURE_GATED,
        env: &[],
        needs: banned_call_linted,
    },
    Case {
        // Cargo prints its own warnings on standard error, not as JSON.
        name: "a member's manifest with a key cargo does not use",
        files: &[
            ("crates/planted/Cargo.toml", PLANTED_UNUSED_KEY),
            ("crates/planted/src/lib.rs", ""),
        ],
        env: &[],
        needs: unused_key,
    },
    Case {
        name: "a member's build script that prints a warning",
        files: WARNING_SCRIPT,
        env: &[],
        needs: script_warning,
    },
    Case {
        // Quiet mode drops a build script's warnings, not the JSON.
        name: "a build script's warning under the caller's CARGO_TERM_QUIET",
        files: WARNING_SCRIPT,
        env: &[("CARGO_TERM_QUIET", "true")],
        needs: script_warning,
    },
    Case {
        name: "a build script's warning under a cargo configuration's term.quiet",
        files: WARNING_SCRIPT_QUIET,
        env: &[],
        needs: script_warning,
    },
    Case {
        // A progress bar ends its line with a carriage return.
        name: "a build script's warning under the caller's forced progress bar",
        files: WARNING_SCRIPT,
        env: &[
            ("CARGO_TERM_PROGRESS_WHEN", "always"),
            ("CARGO_TERM_PROGRESS_WIDTH", "80"),
        ],
        needs: script_warning,
    },
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
        name: "a configured compiler wrapper that runs another member without clippy",
        files: &[
            ("wrap.sh", WRAPPER),
            (
                ".cargo/config.toml",
                "[build]\nrustc-wrapper = \"{root}/wrap.sh\"\n",
            ),
            ("crates/planted/Cargo.toml", PLANTED_MANIFEST),
            BANNED_CALL,
        ],
        env: &[],
        needs: banned_call_linted,
    },
    Case {
        name: "an inherited compiler wrapper that runs another member without clippy",
        files: &[
            ("wrap.sh", WRAPPER),
            ("crates/planted/Cargo.toml", PLANTED_MANIFEST),
            BANNED_CALL,
        ],
        env: &[("RUSTC_WRAPPER", "{root}/wrap.sh")],
        needs: banned_call_linted,
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

/// `manifest` without its `[workspace.dependencies]` table, whose crates
/// the canary's workspace does not hold: cargo would warn of each.
fn without_dependencies(manifest: &str) -> String {
    let mut inside = false;
    manifest
        .lines()
        .filter(|line| {
            // A table header, read as TOML reads it.
            if line.trim_start().starts_with('[') {
                inside = line_keys(line.trim())
                    .is_some_and(|keys| keys == ["workspace", "dependencies"]);
            }
            !inside
        })
        .flat_map(|line| [line, "\n"])
        .collect()
}

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
        ("Cargo.toml", without_dependencies(root_manifest)),
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
            command.env(name, value.replace("{root}", &root));
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
