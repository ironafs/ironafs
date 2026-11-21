// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The clock and randomness bans of coding standards rule 7, kept by
//! clippy's `disallowed-methods` and `disallowed-types` in the root
//! `clippy.toml`, and proved to fire.
//!
//! A ban clippy cannot resolve only warns, and a ban removed from the file
//! is silent, so neither is trusted as written. The check builds a small
//! fixture crate that reaches each banned item in each form code can (a
//! call, a function path, a method call; for a type, a constructor, a
//! `Default`, a `collect`), lints it with the repository's own
//! `clippy.toml`, and requires the ban's warning on every planted line and
//! no unresolved ban.
//!
//! Clippy reads only the configuration nearest a crate and merges nothing,
//! so a crate's own `clippy.toml` replaces the root's for that crate. Each
//! such file is checked the same way, and must carry every ban itself.
//!
//! What the bans cannot see is named in the root `clippy.toml`. The core's
//! own rules (`ironafs-client` names no clock at all) are in `check-deps`,
//! and the pure crates cannot name the standard library (`check-pure`).

mod self_test;

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::fixture::write_file;
use crate::metadata;
use crate::pure::{CLEARED_ENVIRONMENT, first_error};
use crate::{Command, Error};

#[cfg(test)]
mod tests;

const CHECK: &str = "check-bans";

/// The clippy lint a ban is listed under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lint {
    /// `disallowed-methods`: a call, function path or method call.
    Method,
    /// `disallowed-types`: any naming of the type.
    Type,
}

impl Lint {
    /// What clippy's warning calls a banned item of this lint.
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Method => "method",
            Self::Type => "type",
        }
    }
}

/// One ban and a planted line that must trip it. Each planted line defines
/// one function, named for its form, and is written on its own line of the
/// fixture. A planted line names the banned item only where the form it
/// plants needs it, so the warning comes from that form.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ban {
    pub(crate) lint: Lint,
    pub(crate) path: &'static str,
    pub(crate) planted: &'static str,
}

/// Every ban rule 7 needs, each in every form that reaches it.
///
/// The hashed collections and their hasher are banned as types: a map or
/// set built with `new`, `default`, `from` or `collect` seeds its hasher
/// from the operating system without naming a method a method ban could
/// see, and `RandomState::default` is a trait method no method path
/// names. A subprocess can read the host's clock or randomness, so
/// `Command` is banned too.
pub(crate) const BANS: &[Ban] = &[
    Ban {
        lint: Lint::Method,
        path: "std::time::Instant::now",
        planted: "pub fn instant_call() -> std::time::Instant { std::time::Instant::now() }",
    },
    Ban {
        lint: Lint::Method,
        path: "std::time::Instant::now",
        planted: "pub fn instant_path() -> fn() -> std::time::Instant { std::time::Instant::now }",
    },
    Ban {
        lint: Lint::Method,
        path: "std::time::Instant::elapsed",
        planted: "pub fn instant_method(at: std::time::Instant) -> core::time::Duration { at.elapsed() }",
    },
    Ban {
        lint: Lint::Method,
        path: "std::time::SystemTime::now",
        planted: "pub fn system_call() -> std::time::SystemTime { std::time::SystemTime::now() }",
    },
    Ban {
        lint: Lint::Method,
        path: "std::time::SystemTime::elapsed",
        planted: "pub fn system_method(at: std::time::SystemTime) -> bool { at.elapsed().is_ok() }",
    },
    Ban {
        lint: Lint::Type,
        path: "std::hash::RandomState",
        planted: "pub fn random_state() -> u64 { use std::hash::BuildHasher; std::hash::RandomState::new().hash_one(1_u8) }",
    },
    Ban {
        lint: Lint::Type,
        path: "std::hash::RandomState",
        planted: "pub fn random_default() -> std::hash::RandomState { Default::default() }",
    },
    Ban {
        lint: Lint::Type,
        path: "std::hash::RandomState",
        planted: "pub fn random_default_call() -> u64 { use std::hash::BuildHasher; std::hash::RandomState::default().hash_one(1_u8) }",
    },
    Ban {
        lint: Lint::Type,
        path: "std::collections::HashMap",
        planted: "pub fn hash_map_new() -> bool { let mut map = std::collections::HashMap::new(); map.insert(1_u8, 2_u8); map.is_empty() }",
    },
    Ban {
        lint: Lint::Type,
        path: "std::collections::HashMap",
        planted: "pub fn hash_map() -> usize { std::collections::HashMap::<u8, u8>::default().len() }",
    },
    Ban {
        lint: Lint::Type,
        path: "std::collections::HashMap",
        planted: "pub fn hash_map_collect() -> usize { [(1_u8, 2_u8)].into_iter().collect::<std::collections::HashMap<_, _>>().len() }",
    },
    Ban {
        lint: Lint::Type,
        path: "std::collections::HashSet",
        planted: "pub fn hash_set() -> usize { std::collections::HashSet::<u8>::new().len() }",
    },
    Ban {
        lint: Lint::Type,
        path: "std::process::Command",
        planted: "pub fn subprocess() -> bool { std::process::Command::new(\"date\").status().is_ok() }",
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
        let says = format!("disallowed {} `{}`", ban.lint.word(), ban.path);
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
        write_file(&dir.join(name), text)?;
    }
    write_file(&dir.join("src").join("lib.rs"), &planted_source())?;
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

/// A violation and the configuration file it is in, relative to the
/// workspace root.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Found {
    pub(crate) file: String,
    pub(crate) violation: Violation,
}

impl fmt::Display for Found {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.file, self.violation)
    }
}

/// The names clippy looks for in each directory, in its order.
const CONFIG_NAMES: [&str; 2] = [".clippy.toml", "clippy.toml"];

/// The clippy configurations, relative to `root`, that a member crate in
/// `member_dirs` can be linted with: each file `is_file` finds under one of
/// `CONFIG_NAMES` in a member's directory or a directory between it and
/// the root, the root's own included, sorted and each named once.
///
/// Clippy reads only the nearest configuration and merges nothing, so each
/// of these must carry every ban itself.
pub(crate) fn config_files(
    root: &Path,
    member_dirs: &[PathBuf],
    is_file: impl Fn(&Path) -> bool,
) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = member_dirs
        .iter()
        .flat_map(|member| member.ancestors())
        .filter(|dir| dir.starts_with(root))
        .flat_map(|dir| CONFIG_NAMES.map(|name| dir.join(name)))
        .filter(|path| is_file(path))
        .filter_map(|path| path.strip_prefix(root).ok().map(Path::to_path_buf))
        .collect();
    files.sort();
    files.dedup();
    files
}

/// Checks the bans of the workspace at `root`, whose member crates are in
/// `member_dirs`: its `clippy.toml`, which must exist, and every other
/// configuration a member can be linted with (`config_files`), each with
/// fixtures under its own directory below `dir`.
///
/// # Errors
///
/// Returns an error if a configuration cannot be read, a fixture cannot be
/// written or cargo cannot be started.
pub(crate) fn check_tree(
    root: &Path,
    member_dirs: &[PathBuf],
    toolchain_toml: &str,
    dir: &Path,
) -> Result<Vec<Found>, Error> {
    let mut files = config_files(root, member_dirs, Path::is_file);
    let own = PathBuf::from("clippy.toml");
    if !files.contains(&own) {
        files.insert(0, own);
    }
    let mut found = Vec::new();
    for (index, file) in files.iter().enumerate() {
        let path = root.join(file);
        let text = fs::read_to_string(&path).map_err(Error::path_io("read", &path))?;
        let name = file.display().to_string();
        found.extend(
            check(&text, toolchain_toml, &dir.join(index.to_string()))?
                .into_iter()
                .map(|violation| Found {
                    file: name.clone(),
                    violation,
                }),
        );
    }
    Ok(found)
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
    let member_dirs: Vec<PathBuf> = workspace
        .members
        .iter()
        .map(|member| member.dir.clone())
        .collect();
    let dir = crate::fixture::fresh_dir(&workspace.target_dir, CHECK)?;
    let result = check_tree(&workspace.root, &member_dirs, &toolchain_toml, &dir);
    crate::fixture::remove_dir(&dir)?;
    let violations = result?;
    if !violations.is_empty() {
        return Err(Error::Rules {
            check: CHECK,
            violations: violations.iter().map(ToString::to_string).collect(),
        });
    }
    let files: Vec<String> = config_files(&workspace.root, &member_dirs, Path::is_file)
        .iter()
        .map(|file| file.display().to_string())
        .collect();
    writeln!(
        output,
        "check-bans: {} planted calls each tripped their ban in each of {}; no ban is unresolved",
        BANS.len(),
        files.join(", ")
    )
    .map_err(Error::io("report check-bans"))
}
