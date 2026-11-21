// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pure crates are `#![no_std]` with `extern crate alloc`, and the
//! compiler proves it (ADR 0004, C3, point 1, and C4, point 1). Each pure
//! library is checked for every target in `rust-toolchain.toml`, in the dev
//! and release profiles, against a sysroot that holds only `core` and
//! `alloc`, with `unsafe_code` forbidden on the command line. There `std`
//! does not exist, so no file, socket, thread, clock or system randomness
//! can be named, whatever text produced the code; and the lint stops
//! foreign functions and assembly.
//!
//! Around the build, rules that need no compiler: the cargo configuration
//! may not change the flags the build runs with, the source may say
//! `extern` only as `extern crate alloc;`, and a pure crate's dependencies
//! are pure crates with no build script, procedural macro, native library
//! or feature.

mod self_test;

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::Error;
use crate::deps::{self, Kind};
use crate::metadata::{self, Member, Package, Workspace};

#[cfg(test)]
mod tests;

/// Words that may not appear in a cargo configuration file, each with what
/// it would do. Matching is on raw text, so a comment that names one fails
/// too: the rule fails closed rather than parse TOML.
const CONFIG_WORDS: &[(&str, &str)] = &[
    (
        "rustflags",
        "sets compiler flags, which could cap the lints that forbid unsafe code",
    ),
    (
        "rustc",
        "replaces or wraps the compiler, which could drop the flags this check passes",
    ),
    (
        "env",
        "sets the compiler's environment, which could enable unstable features",
    ),
];

/// The only line of a pure crate's source that may say `extern`.
const EXTERN_ALLOWED: &str = "extern crate alloc;";

/// The line that makes a crate `#![no_std]`, which its library root holds.
const NO_STD: &str = "#![no_std]";

/// Target kinds a crate in a pure crate's closure may have: its library and
/// targets that are never shipped.
const TARGET_KINDS: &[&str] = &["lib", "rlib", "test", "bench", "example"];

/// The libraries of the toolchain's sysroot the check copies for each
/// target: `core`, `alloc` and what they need. The names are the
/// toolchain's own, which the self-test proves still work.
const SYSROOT_LIBRARIES: &[&str] = &[
    "libcore-",
    "liballoc-",
    "libcompiler_builtins-",
    "librustc_std_workspace_core-",
];

/// Environment variables that would change the compiler or its flags behind
/// this check's back; each is removed from the build's environment.
const CLEARED_ENVIRONMENT: &[&str] = &[
    "RUSTFLAGS",
    "CARGO_BUILD_RUSTFLAGS",
    "RUSTC",
    "CARGO_BUILD_RUSTC",
    "RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "RUSTC_BOOTSTRAP",
];

/// The profiles each pure library is built in: a `cfg(debug_assertions)`
/// branch is compiled in one and not the other.
const PROFILES: &[&str] = &["dev", "release"];

/// One broken rule.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    CargoConfig {
        file: PathBuf,
        line: usize,
        word: &'static str,
        reason: &'static str,
    },
    NoLibrary(String),
    NotNoStd(PathBuf),
    Extern {
        file: PathBuf,
        line: usize,
    },
    Closure {
        from: String,
        via: Vec<String>,
        to: String,
    },
    Target {
        name: String,
        target: String,
    },
    Links {
        name: String,
        links: String,
    },
    Features {
        name: String,
        features: Vec<String>,
    },
    Build {
        name: String,
        profile: &'static str,
        error: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CargoConfig {
                file,
                line,
                word,
                reason,
            } => write!(
                formatter,
                "{}:{line}: names {word}: a tracked cargo configuration {reason}; the pure-crate \
                 build sets its own flags (ADR 0004, C4, point 1)",
                file.display()
            ),
            Self::NoLibrary(name) => write!(
                formatter,
                "{name} is a pure crate with no library target (ADR 0004, C4, point 1)"
            ),
            Self::NotNoStd(file) => write!(
                formatter,
                "{}: a pure crate's library root has the line `{NO_STD}`, unconditionally \
                 (ADR 0004, C4, point 1)",
                file.display()
            ),
            Self::Extern { file, line } => write!(
                formatter,
                "{}:{line}: says extern: in a pure crate `extern` appears only as \
                 `{EXTERN_ALLOWED}`, so no cfg can bring the standard library or a foreign \
                 function back (ADR 0004, C4, point 1)",
                file.display()
            ),
            Self::Closure { from, via, to } => {
                write!(formatter, "{from} depends on {to}")?;
                if !via.is_empty() {
                    write!(formatter, " through {}", via.join(" -> "))?;
                }
                write!(
                    formatter,
                    ": a pure crate's dependencies are pure crates only (ADR 0004, C4, points \
                     1 and 4)"
                )
            }
            Self::Target { name, target } => write!(
                formatter,
                "{name} has a {target} target: a pure crate and its dependencies have no build \
                 script or procedural macro, which run code at build time, and ship only a \
                 library (ADR 0004, C3, point 1)"
            ),
            Self::Links { name, links } => write!(
                formatter,
                "{name} links the native library {links}: a pure crate and its dependencies \
                 link none (ADR 0004, C4, point 1)"
            ),
            Self::Features { name, features } => write!(
                formatter,
                "{name} declares the features {}: a pure crate and its dependencies have no \
                 features, so none can turn the standard library on (ADR 0004, C4, point 1)",
                features.join(", ")
            ),
            Self::Build {
                name,
                profile,
                error,
            } => write!(
                formatter,
                "{name} does not build in the {profile} profile against core and alloc alone \
                 with unsafe code forbidden: {error} (ADR 0004, C3, point 1, and C4, point 1)"
            ),
        }
    }
}

/// How cargo is run on the workspace being checked: this repository with
/// its lock file held fixed, or a fixture offline with its own `--config`
/// values.
pub(crate) struct Cargo<'a> {
    pub(crate) manifest: Option<&'a Path>,
    pub(crate) config: &'a [String],
}

impl Cargo<'_> {
    fn command(&self, root: &Path, subcommand: &str) -> Command {
        let mut command = Command::new("cargo");
        // Cargo reads its configuration from the directory it runs in.
        command.current_dir(root).arg(subcommand);
        for value in self.config {
            command.arg("--config").arg(value);
        }
        match self.manifest {
            Some(manifest) => command
                .arg("--offline")
                .arg("--manifest-path")
                .arg(manifest),
            None => command.arg("--locked"),
        };
        command
    }
}

/// The pure crates of a workspace, by the crate table.
pub(crate) fn pure_members(workspace: &Workspace) -> Vec<&Member> {
    workspace
        .members
        .iter()
        .filter(|member| deps::kind_of(&member.name) == Some(Kind::Pure))
        .collect()
}

/// The targets of `rust-toolchain.toml`: the quoted names in its `targets`
/// array.
///
/// # Errors
///
/// Returns an error if the file lists no target.
pub(crate) fn toolchain_targets(text: &str) -> Result<Vec<String>, Error> {
    let mut lines = text
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim());
    let Some(start) = lines.by_ref().find_map(|line| {
        line.strip_prefix("targets")
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('='))
    }) else {
        return Err(Error::UnexpectedOutput("rust-toolchain.toml"));
    };
    let mut array = start.to_owned();
    for line in lines {
        if array.contains(']') {
            break;
        }
        array.push_str(line);
    }
    let targets: Vec<String> = array
        .split(']')
        .next()
        .unwrap_or_default()
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    if targets.is_empty() {
        return Err(Error::UnexpectedOutput("rust-toolchain.toml"));
    }
    Ok(targets)
}

/// Every cargo configuration file under `root`: `config` or `config.toml`
/// in a `.cargo` directory. The target directory, `.git` and nested
/// checkouts (a directory holding its own `.git`) are not entered; cargo
/// reads a nested checkout's configuration only when run inside it.
///
/// # Errors
///
/// Returns an error if a directory cannot be listed.
pub(crate) fn config_files(root: &Path, target_dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let mut entries = fs::read_dir(&dir)
            .and_then(|entries| {
                entries
                    .map(|entry| entry.map(|entry| entry.path()))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(Error::path_io("list directory", &dir))?;
        entries.sort();
        for path in entries {
            let metadata =
                fs::symlink_metadata(&path).map_err(Error::path_io("read file type", &path))?;
            let name = path.file_name().unwrap_or_default();
            if metadata.is_dir() {
                let nested_checkout = path.join(".git").symlink_metadata().is_ok();
                if name != ".git" && path != target_dir && !nested_checkout {
                    pending.push(path);
                }
            } else if (name == "config" || name == "config.toml")
                && dir.file_name().is_some_and(|parent| parent == ".cargo")
            {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Whether `bytes` holds `word` at `at` with no identifier character on
/// either side. Only ASCII letters, digits and `_` count as identifier
/// characters, so the rules fail closed on any other byte.
fn word_at(bytes: &[u8], at: usize, word: &[u8]) -> bool {
    let is_identifier =
        |byte: Option<&u8>| byte.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_');
    bytes.get(at..at + word.len()) == Some(word)
        && !is_identifier(at.checked_sub(1).and_then(|before| bytes.get(before)))
        && !is_identifier(bytes.get(at + word.len()))
}

fn contains_word(line: &[u8], word: &[u8]) -> bool {
    (0..line.len()).any(|at| word_at(line, at, word))
}

/// The words of `CONFIG_WORDS` a configuration file names, with their line
/// numbers. `rustflags` and `rustc` count anywhere, also inside a longer
/// key; `env` counts as a word, which `[env]`, `env.NAME` and `env = {`
/// all are.
pub(crate) fn config_words(text: &[u8]) -> Vec<(usize, &'static str, &'static str)> {
    let mut found = Vec::new();
    for (index, line) in text.split(|byte| *byte == b'\n').enumerate() {
        for (word, reason) in CONFIG_WORDS {
            let named = if *word == "env" {
                contains_word(line, word.as_bytes())
            } else {
                line.windows(word.len())
                    .any(|window| window == word.as_bytes())
            };
            if named {
                found.push((index + 1, *word, *reason));
            }
        }
    }
    found
}

/// The lines of a pure crate's source file that say `extern` other than as
/// the one allowed line.
pub(crate) fn extern_lines(text: &[u8]) -> Vec<usize> {
    text.split(|byte| *byte == b'\n')
        .enumerate()
        .filter(|(_, line)| {
            contains_word(line, b"extern") && line.trim_ascii() != EXTERN_ALLOWED.as_bytes()
        })
        .map(|(index, _)| index + 1)
        .collect()
}

/// Whether a library root holds the `#![no_std]` line.
pub(crate) fn has_no_std(text: &[u8]) -> bool {
    text.split(|byte| *byte == b'\n')
        .any(|line| line.trim_ascii() == NO_STD.as_bytes())
}

fn shown(workspace: &Workspace, path: &Path) -> PathBuf {
    path.strip_prefix(&workspace.root)
        .unwrap_or(path)
        .to_path_buf()
}

fn read(path: &Path) -> Result<Vec<u8>, Error> {
    fs::read(path).map_err(Error::path_io("read file", path))
}

/// Every file below `dir`, in path order.
fn all_files(dir: &Path, found: &mut Vec<PathBuf>) -> Result<(), Error> {
    let mut entries = fs::read_dir(dir)
        .and_then(|entries| {
            entries
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(Error::path_io("list source directory", dir))?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            all_files(&path, found)?;
        } else {
            found.push(path);
        }
    }
    Ok(())
}

/// The rules that need no compiler: the cargo configuration, each pure
/// crate's source text and its dependency closure. Returns the violations
/// and the number of configuration files read.
///
/// # Errors
///
/// Returns an error if a file or directory cannot be read.
pub(crate) fn static_violations(workspace: &Workspace) -> Result<(Vec<Violation>, usize), Error> {
    let mut violations = Vec::new();
    let configs = config_files(&workspace.root, &workspace.target_dir)?;
    for file in &configs {
        for (line, word, reason) in config_words(&read(file)?) {
            violations.push(Violation::CargoConfig {
                file: shown(workspace, file),
                line,
                word,
                reason,
            });
        }
    }
    for member in pure_members(workspace) {
        violations.extend(source_violations(workspace, member)?);
        for found in closure_violations(workspace, member) {
            if !violations.contains(&found) {
                violations.push(found);
            }
        }
    }
    Ok((violations, configs.len()))
}

/// The library root's `#![no_std]` line, and `extern` in any file below
/// the library's directory, whatever its extension: `include!` can read a
/// file that is not Rust.
fn source_violations(workspace: &Workspace, member: &Member) -> Result<Vec<Violation>, Error> {
    let (Some(lib_root), Some(lib_dir)) = (&member.lib_root, member.lib_dir()) else {
        return Ok(vec![Violation::NoLibrary(member.name.clone())]);
    };
    let mut violations = Vec::new();
    if !has_no_std(&read(lib_root)?) {
        violations.push(Violation::NotNoStd(shown(workspace, lib_root)));
    }
    let mut files = Vec::new();
    all_files(lib_dir, &mut files)?;
    for file in files {
        for line in extern_lines(&read(&file)?) {
            violations.push(Violation::Extern {
                file: shown(workspace, &file),
                line,
            });
        }
    }
    Ok(violations)
}

fn is_pure(package: &Package) -> bool {
    package.member && deps::kind_of(&package.name) == Some(Kind::Pure)
}

/// The packages a pure crate reaches through normal and build dependencies
/// must be pure crates; it and each of them may have no build script,
/// procedural macro, native library or feature. A package that is not pure
/// is reported and not followed further.
fn closure_violations(workspace: &Workspace, member: &Member) -> Vec<Violation> {
    let mut violations = Vec::new();
    let Some(start) = workspace
        .packages
        .iter()
        .find(|package| package.id == member.id)
    else {
        return violations;
    };
    let mut checked = vec![start];
    for (package, via) in deps::reachable(workspace, &member.id, &is_pure) {
        if !is_pure(package) {
            violations.push(Violation::Closure {
                from: member.name.clone(),
                via,
                to: package.name.clone(),
            });
        }
        checked.push(package);
    }
    for package in checked {
        for target in &package.target_kinds {
            if !TARGET_KINDS.contains(&target.as_str()) {
                violations.push(Violation::Target {
                    name: package.name.clone(),
                    target: target.clone(),
                });
            }
        }
        if let Some(links) = &package.links {
            violations.push(Violation::Links {
                name: package.name.clone(),
                links: links.clone(),
            });
        }
        if !package.features.is_empty() {
            violations.push(Violation::Features {
                name: package.name.clone(),
                features: package.features.clone(),
            });
        }
    }
    violations
}

/// The toolchain's own sysroot, as the `rustc` cargo runs reports it.
///
/// # Errors
///
/// Returns an error if `rustc` cannot be run or prints no path.
fn toolchain_sysroot(root: &Path) -> Result<PathBuf, Error> {
    const COMMAND: &str = "rustc --print sysroot";
    let output = Command::new("rustc")
        .args(["--print", "sysroot"])
        .current_dir(root)
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::io(COMMAND))?;
    Error::check_status(COMMAND, output.status)?;
    let text = String::from_utf8(output.stdout).map_err(|_| Error::UnexpectedOutput(COMMAND))?;
    let path = text.trim();
    if path.is_empty() {
        return Err(Error::UnexpectedOutput(COMMAND));
    }
    Ok(PathBuf::from(path))
}

/// Writes into `dir` a sysroot that holds, for each target, only the
/// metadata of `core`, `alloc` and what they need, taken from the
/// toolchain's sysroot. Files are hard-linked where the file system allows
/// it, and copied otherwise.
///
/// # Errors
///
/// Returns an error if a target's libraries are missing (the target is not
/// installed) or a file cannot be written.
pub(crate) fn core_sysroot(toolchain: &Path, targets: &[String], dir: &Path) -> Result<(), Error> {
    crate::fixture::remove_dir(dir)?;
    for target in targets {
        let from = toolchain.join("lib/rustlib").join(target).join("lib");
        let to = dir.join("lib/rustlib").join(target).join("lib");
        fs::create_dir_all(&to).map_err(Error::path_io("create sysroot directory", &to))?;
        let mut libraries = Vec::new();
        all_files(&from, &mut libraries)?;
        let mut copied = Vec::new();
        for library in libraries {
            let name = library.file_name().unwrap_or_default().to_string_lossy();
            let wanted = SYSROOT_LIBRARIES
                .iter()
                .find(|prefix| name.starts_with(**prefix) && name.ends_with(".rmeta"));
            if let Some(prefix) = wanted {
                let destination = to.join(&*name);
                if fs::hard_link(&library, &destination).is_err() {
                    fs::copy(&library, &destination)
                        .map_err(Error::path_io("copy sysroot library", &destination))?;
                }
                copied.push(*prefix);
            }
        }
        if SYSROOT_LIBRARIES
            .iter()
            .any(|prefix| !copied.contains(prefix))
        {
            return Err(Error::PathIo {
                operation: "find core and alloc for an installed target",
                path: from,
                source: io::ErrorKind::NotFound.into(),
            });
        }
    }
    Ok(())
}

/// The first line of cargo's error output that says what failed.
fn first_error(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let mut lines = text.lines().map(str::trim);
    let first = lines
        .clone()
        .find(|line| line.starts_with("error") && !line.starts_with("error: could not compile"))
        .or_else(|| lines.find(|line| line.starts_with("error")));
    first
        .unwrap_or("cargo failed with no error line")
        .to_owned()
}

/// Checks each pure library for every target, in each profile, against the
/// sysroot at `sysroot`, with `unsafe_code` forbidden. Builds go to
/// `build_dir`, apart from the workspace's own, so the changed flags do not
/// rebuild everything else.
///
/// # Errors
///
/// Returns an error if cargo cannot be started.
pub(crate) fn build_violations(
    workspace: &Workspace,
    cargo: &Cargo<'_>,
    sysroot: &Path,
    targets: &[String],
    build_dir: &Path,
) -> Result<Vec<Violation>, Error> {
    let mut flags = std::ffi::OsString::from("--sysroot=");
    flags.push(sysroot);
    flags.push("\u{1f}-Funsafe_code");
    let mut violations = Vec::new();
    for member in pure_members(workspace) {
        if member.lib_root.is_none() {
            continue;
        }
        for profile in PROFILES {
            let mut command = cargo.command(&workspace.root, "check");
            command
                .args(["--lib", "--quiet", "--package", &member.name])
                .arg("--target-dir")
                .arg(build_dir)
                .env("CARGO_ENCODED_RUSTFLAGS", &flags)
                .stdout(Stdio::null());
            for name in CLEARED_ENVIRONMENT {
                command.env_remove(name);
            }
            for target in targets {
                command.args(["--target", target]);
            }
            if *profile == "release" {
                command.arg("--release");
            }
            let output = command.output().map_err(Error::io("cargo check"))?;
            if !output.status.success() {
                violations.push(Violation::Build {
                    name: member.name.clone(),
                    profile,
                    error: first_error(&output.stderr),
                });
            }
        }
    }
    Ok(violations)
}

/// Checks this repository's pure crates, or with `self_test` proves on
/// planted fixtures that each rule is caught.
///
/// # Errors
///
/// Returns the violations found, or why the check could not run.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    let toolchain_file = workspace.root.join("rust-toolchain.toml");
    let targets = toolchain_targets(
        &fs::read_to_string(&toolchain_file)
            .map_err(Error::path_io("read toolchain file", &toolchain_file))?,
    )?;
    if self_test {
        let sysroot = toolchain_sysroot(&workspace.root)?;
        return self_test::run(&workspace.target_dir, &sysroot, &targets, output);
    }
    let (mut violations, configs) = static_violations(&workspace)?;
    let pure: Vec<&str> = pure_members(&workspace)
        .iter()
        .map(|member| member.name.as_str())
        .collect();
    if !pure.is_empty() {
        let dir = workspace.target_dir.join("xtask-pure");
        let sysroot = dir.join("sysroot");
        core_sysroot(&toolchain_sysroot(&workspace.root)?, &targets, &sysroot)?;
        let cargo = Cargo {
            manifest: None,
            config: &[],
        };
        violations.extend(build_violations(
            &workspace,
            &cargo,
            &sysroot,
            &targets,
            &dir.join("build"),
        )?);
    }
    if !violations.is_empty() {
        return Err(Error::Rules {
            check: "check-pure",
            violations: violations.iter().map(ToString::to_string).collect(),
        });
    }
    let built = if pure.is_empty() {
        "no pure crates yet".to_owned()
    } else {
        format!(
            "{} pure crates ({}) built for {} targets in dev and release against core and alloc \
             alone",
            pure.len(),
            pure.join(", "),
            targets.len()
        )
    };
    writeln!(
        output,
        "check-pure: {configs} cargo configuration files read; {built}"
    )
    .map_err(Error::io("report check-pure"))
}
