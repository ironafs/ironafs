// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canary's self-test: a workspace holding only the canary, linted as
//! the real workspace is, clean and then under each way around the bans
//! that can be planted, each of which must leave a canary line silent.

use std::path::Path;

use super::super::{Found, Violation, canary, clippy_driver, line_keys, lint, workspace_clippy};
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

fn alias_ignored(violation: &Violation) -> bool {
    matches!(violation, Violation::Lint(line) if line.starts_with("warning: user-defined alias"))
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
        // The lint runs `cargo check`, so no alias of `clippy` is read: the
        // second member's banned call is linted whatever the alias says.
        name: "a cargo alias that makes clippy a plain check",
        files: &[
            (".cargo/config.toml", "[alias]\nclippy = \"check\"\n"),
            ("crates/planted/Cargo.toml", PLANTED_MANIFEST),
            BANNED_CALL,
        ],
        env: &[],
        needs: banned_call_linted,
    },
    Case {
        name: "CARGO_ALIAS_CLIPPY making clippy a plain check",
        files: &[("crates/planted/Cargo.toml", PLANTED_MANIFEST), BANNED_CALL],
        env: &[("CARGO_ALIAS_CLIPPY", "check")],
        needs: banned_call_linted,
    },
    Case {
        // Cargo ignores an alias of a built-in, with a warning it prints
        // before it reads CARGO_TERM_COLOR: coloured, under CLICOLOR_FORCE.
        name: "a cargo alias of check, under the caller's CLICOLOR_FORCE",
        files: &[(".cargo/config.toml", "[alias]\ncheck = \"build\"\n")],
        env: &[("CLICOLOR_FORCE", "1")],
        needs: alias_ignored,
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

/// An artifact left by an earlier clippy run of the workspace under a
/// wrapper that ran plain rustc for the second member, in the workspace's
/// own target directory, then in a build directory a cargo configuration
/// names: cargo's key for it holds no wrapper, so a lint that built there
/// would reuse it and never lint that member.
const PRIMED: &[Case] = &[
    Case {
        name: "a second member's artifact left by an earlier clippy run under a wrapper",
        files: &[
            ("wrap.sh", WRAPPER),
            ("crates/planted/Cargo.toml", PLANTED_MANIFEST),
            BANNED_CALL,
        ],
        env: &[("RUSTC_WRAPPER", "{root}/wrap.sh")],
        needs: banned_call_linted,
    },
    Case {
        name: "the same artifact in a build directory a cargo configuration names",
        files: &[
            ("wrap.sh", WRAPPER),
            (
                ".cargo/config.toml",
                "[build]\nbuild-dir = \"{root}/shared\"\n",
            ),
            ("crates/planted/Cargo.toml", PLANTED_MANIFEST),
            BANNED_CALL,
        ],
        env: &[("RUSTC_WRAPPER", "{root}/wrap.sh")],
        needs: banned_call_linted,
    },
    Case {
        // An alias's `--config` comes before every variable the lint sets;
        // the lint runs `cargo check`, so no alias of `clippy` is read.
        name: "the same artifact in a build directory a clippy alias names",
        files: &[
            ("wrap.sh", WRAPPER),
            (
                ".cargo/config.toml",
                "[alias]\nclippy = [\"check\", \"--config\", \"build.build-dir='{root}/shared'\"]\n",
            ),
            ("crates/planted/Cargo.toml", PLANTED_MANIFEST),
            BANNED_CALL,
        ],
        env: &[
            ("RUSTC_WRAPPER", "{root}/wrap.sh"),
            ("CARGO_BUILD_BUILD_DIR", "{root}/shared"),
        ],
        needs: banned_call_linted,
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
/// repository's `root_manifest`, `clippy_toml` and `toolchain_toml`, its
/// one dependency `ironafs-time` taken from `time_crate`, then `files`, and
/// lints it as the real workspace is, with `env` set. With
/// `prime`, it first runs clippy on it as the lint does, into the
/// workspace's own target directory, with `env` set and nothing else
/// adjusted, and then lints it with no variable set.
fn lint_planted(
    dir: &Path,
    texts: (&str, &str, &str),
    time_crate: &Path,
    files: &[(&str, &str)],
    env: &[(&str, &str)],
    prime: bool,
) -> Result<Vec<Found>, Error> {
    let (root_manifest, clippy_toml, toolchain_toml) = texts;
    let root = dir.to_string_lossy().into_owned();
    // The canary's one dependency, outside this workspace, so not a
    // member of it.
    let manifest = format!(
        "{}\n[workspace.dependencies]\nironafs-time = {{ path = '{}' }}\n",
        without_dependencies(root_manifest),
        time_crate.display()
    );
    let canary_files = [
        ("Cargo.toml", manifest),
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
    let set = |command: &mut Command| {
        for (name, value) in env {
            command.env(name, value.replace("{root}", &root));
        }
    };
    if prime {
        let mut command = workspace_clippy(&clippy_driver(dir)?);
        command.current_dir(dir);
        set(&mut command);
        let status = command
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(Error::io("cargo clippy"))?;
        Error::check_status("cargo clippy, priming the planted workspace", status)?;
        return lint(&workspace, |_| {});
    }
    lint(&workspace, set)
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
    time_crate: &Path,
    report: &mut impl FnMut(String) -> Result<(), Error>,
) -> Result<(), Error> {
    let clean = "a workspace whose lint draws every canary warning";
    let found = lint_planted(&dir.join("clean"), texts, time_crate, &[], &[], false)?;
    if !found.is_empty() {
        return Err(failure(clean, &found));
    }
    report(format!("passed: {clean}"))?;
    let cases = CASES.iter().map(|case| (case, false));
    let primed = PRIMED.iter().map(|case| (case, true));
    for (index, (case, prime)) in cases.chain(primed).enumerate() {
        if cfg!(not(unix)) && case.files.iter().any(|(file, _)| *file == "wrap.sh") {
            report(format!("not run here: {}", case.name))?;
            continue;
        }
        let found = lint_planted(
            &dir.join(format!("case-{index}")),
            texts,
            time_crate,
            case.files,
            case.env,
            prime,
        )?;
        if !found.iter().any(|found| (case.needs)(&found.violation)) {
            return Err(failure(case.name, &found));
        }
        for violation in found.iter().filter(|found| (case.needs)(&found.violation)) {
            report(format!("caught: {}: {violation}", case.name))?;
        }
    }
    // The canary's manifest as check-bans wrote it before the constructor
    // bans: without the dependency their planted lines need.
    let name = "the canary's manifest without its ironafs-time dependency";
    let altered: String = canary::manifest()
        .lines()
        .filter(|line| *line != "[dependencies]" && !line.starts_with("ironafs-time = "))
        .map(|line| {
            if line.starts_with("planted = ") {
                "planted = []"
            } else {
                line
            }
        })
        .flat_map(|line| [line, "\n"])
        .collect();
    if altered == canary::manifest() {
        return Err(failure(name, &[]));
    }
    let found = lint_planted(
        &dir.join("no-time"),
        texts,
        time_crate,
        &[("crates/ironafs-bans-canary/Cargo.toml", &altered)],
        &[],
        false,
    )?;
    if !found.iter().any(|found| tampered(&found.violation)) {
        return Err(failure(name, &found));
    }
    for violation in found.iter().filter(|found| tampered(&found.violation)) {
        report(format!("caught: {name}: {violation}"))?;
    }
    Ok(())
}
