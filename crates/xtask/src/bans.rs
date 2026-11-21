// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The clock and randomness bans of coding standards rule 7, kept by
//! clippy's `disallowed-methods` in the root `clippy.toml`, and proved to
//! fire.
//!
//! A ban clippy cannot resolve only warns, and a ban removed from the file
//! is silent, so neither is trusted as written. The check builds a small
//! fixture crate that calls each banned item in each form code can reach
//! it (a call, a function path, a method call), lints it with the
//! repository's own `clippy.toml`, and requires a disallowed-method
//! warning on every planted line and no unresolved ban.
//!
//! What the bans cannot see is named in the root `clippy.toml`. The core's
//! own rules (`ironafs-client` names no clock at all) are in `check-deps`,
//! and the pure crates cannot name the standard library (`check-pure`).

mod self_test;

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::Error;
use crate::metadata;
use crate::pure::{CLEARED_ENVIRONMENT, first_error};

#[cfg(test)]
mod tests;

const CHECK: &str = "check-bans";

/// One ban and a planted line that must trip it. Each planted line defines
/// one function, named for its form, and is written on its own line of the
/// fixture.
#[derive(Debug)]
pub(crate) struct Ban {
    pub(crate) path: &'static str,
    pub(crate) planted: &'static str,
}

/// Every ban rule 7 needs, each in every form that reaches it.
pub(crate) const BANS: &[Ban] = &[
    Ban {
        path: "std::time::Instant::now",
        planted: "pub fn instant_call() -> std::time::Instant { std::time::Instant::now() }",
    },
    Ban {
        path: "std::time::Instant::now",
        planted: "pub fn instant_path() -> fn() -> std::time::Instant { std::time::Instant::now }",
    },
    Ban {
        path: "std::time::Instant::elapsed",
        planted: "pub fn instant_method(at: std::time::Instant) -> core::time::Duration { at.elapsed() }",
    },
    Ban {
        path: "std::time::SystemTime::now",
        planted: "pub fn system_call() -> std::time::SystemTime { std::time::SystemTime::now() }",
    },
    Ban {
        path: "std::time::SystemTime::elapsed",
        planted: "pub fn system_method(at: std::time::SystemTime) -> bool { at.elapsed().is_ok() }",
    },
    Ban {
        path: "std::hash::RandomState::new",
        planted: "pub fn random_state() -> std::hash::RandomState { std::hash::RandomState::new() }",
    },
];

/// One way the bans fail.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    /// `clippy.toml` has no entry for the ban's path.
    NotListed { path: &'static str },
    /// The planted line drew no disallowed-method warning for the path.
    NotFired {
        path: &'static str,
        planted: &'static str,
    },
    /// Clippy reported a configured path that names nothing, as given.
    Unresolved(String),
    /// Clippy did not run to completion on the fixture.
    Clippy(String),
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotListed { path } => write!(
                formatter,
                "clippy.toml does not ban {path} (coding standards rule 7)"
            ),
            Self::NotFired { path, planted } => write!(
                formatter,
                "the ban on {path} did not fire on `{planted}`: it is missing, misspelt or no \
                 longer resolves (coding standards rule 7)"
            ),
            Self::Unresolved(line) => write!(
                formatter,
                "clippy cannot resolve a ban, so it bans nothing: {line}"
            ),
            Self::Clippy(error) => write!(
                formatter,
                "clippy did not finish on the planted calls: {error}"
            ),
        }
    }
}

/// Whether `clippy_toml` has an entry whose path is exactly `path`.
/// Entries are written as `path = "..."`; a comment line does not count.
pub(crate) fn listed(clippy_toml: &str, path: &str) -> bool {
    let entry = format!("path = \"{path}\"");
    clippy_toml
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .any(|line| line.contains(&entry))
}

/// The fixture's library: line `n` holds the planted line of `BANS[n - 1]`.
pub(crate) fn planted_source() -> String {
    BANS.iter().flat_map(|ban| [ban.planted, "\n"]).collect()
}

/// What clippy's short-format `output` on the fixture shows about each ban.
/// Clippy says a configured path "does not refer to" an item when it
/// cannot resolve it.
pub(crate) fn fired_violations(output: &str) -> Vec<Violation> {
    let mut violations: Vec<Violation> = output
        .lines()
        .filter(|line| line.contains("does not refer to"))
        .map(|line| Violation::Unresolved(line.trim().to_owned()))
        .collect();
    for (index, ban) in BANS.iter().enumerate() {
        let at = format!("src/lib.rs:{}:", index + 1);
        let says = format!("disallowed method `{}`", ban.path);
        if !output
            .lines()
            .any(|line| line.starts_with(&at) && line.contains(&says))
        {
            violations.push(Violation::NotFired {
                path: ban.path,
                planted: ban.planted,
            });
        }
    }
    violations
}

/// Checks the bans in `clippy_toml`: each listed, each firing on its
/// planted lines in a fixture under `dir` linted with that file and the
/// toolchain file `toolchain_toml`, and none unresolved.
///
/// # Errors
///
/// Returns an error if the fixture cannot be written or cargo cannot be
/// started.
pub(crate) fn check(
    clippy_toml: &str,
    toolchain_toml: &str,
    dir: &Path,
) -> Result<Vec<Violation>, Error> {
    let mut violations: Vec<Violation> = BANS
        .iter()
        .filter(|ban| !listed(clippy_toml, ban.path))
        .map(|ban| Violation::NotListed { path: ban.path })
        .collect();
    violations.dedup();
    let files = [
        (
            "Cargo.toml",
            "[package]\nname = \"bans-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
             # Not part of the workspace whose target directory holds it.\n[workspace]\n",
        ),
        ("clippy.toml", clippy_toml),
        ("rust-toolchain.toml", toolchain_toml),
    ];
    for (name, text) in files {
        write(&dir.join(name), text)?;
    }
    write(&dir.join("src").join("lib.rs"), &planted_source())?;
    let mut command = Command::new("cargo");
    // Rustup picks the toolchain from the directory cargo runs in.
    command
        .current_dir(dir)
        .args([
            "clippy",
            "--offline",
            "--quiet",
            "--message-format",
            "short",
        ])
        .arg("--target-dir")
        .arg(dir.join("target"))
        .env("CLIPPY_CONF_DIR", dir)
        .env_remove("CARGO_BUILD_WARNINGS")
        .env_remove("CLIPPY_ARGS");
    for name in CLEARED_ENVIRONMENT {
        command.env_remove(name);
    }
    let output = command.output().map_err(Error::io("cargo clippy"))?;
    if output.status.success() {
        violations.extend(fired_violations(&String::from_utf8_lossy(&output.stderr)));
    } else {
        violations.push(Violation::Clippy(first_error(&output.stderr)));
    }
    Ok(violations)
}

fn write(path: &Path, text: &str) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(Error::path_io("create fixture directory", parent))?;
    }
    fs::write(path, text).map_err(Error::path_io("write fixture file", path))
}

/// Checks this repository's bans, or with `self_test` proves on planted
/// configurations that a removed or misspelt ban is caught.
///
/// # Errors
///
/// Returns the violations found, or why the check could not run.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    let read = |name: &str| {
        let path = workspace.root.join(name);
        fs::read_to_string(&path).map_err(Error::path_io("read", &path))
    };
    let clippy_toml = read("clippy.toml")?;
    let toolchain_toml = read("rust-toolchain.toml")?;
    if self_test {
        return self_test::run(&workspace.target_dir, &clippy_toml, &toolchain_toml, output);
    }
    let dir = crate::fixture::fresh_dir(&workspace.target_dir, CHECK)?;
    let result = check(&clippy_toml, &toolchain_toml, &dir);
    crate::fixture::remove_dir(&dir)?;
    let violations = result?;
    if !violations.is_empty() {
        return Err(Error::Rules {
            check: CHECK,
            violations: violations.iter().map(ToString::to_string).collect(),
        });
    }
    writeln!(
        output,
        "check-bans: {} planted calls each tripped their ban in clippy.toml; no ban is unresolved",
        BANS.len()
    )
    .map_err(Error::io("report check-bans"))
}
