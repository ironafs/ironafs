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
//! Every method and type ban of the root `clippy.toml` must have a planted
//! line, so a ban added there is proved too, and a member's own file is
//! held to the same lines.
//!
//! Clippy reads only the configuration nearest a crate and merges nothing,
//! so a crate's own `clippy.toml` replaces the root's for that crate. Each
//! such file is checked the same way, and must carry every ban itself.
//! `CLIPPY_CONF_DIR` comes before all of them: clippy then reads the
//! directory it names for every crate. The check refuses to run while it
//! is set, and `cargo xtask check` lints without it. A build script can
//! set it again for its own crate's compiler (`cargo::rustc-env`), or set
//! where clippy's lookup starts or its lint arguments, so the check runs
//! the workspace's clippy and refuses any build script that does, in any
//! case of the name.
//!
//! A build script runs before its crate is linted and can also write a
//! configuration. The check builds the workspace first and proves the
//! configurations left after it, and both it and `cargo xtask check`'s
//! own lint refuse a configuration added, removed or rewritten while the
//! workspace was built (`unchanged_by`). A build script that rewrites one
//! and puts it back before the build ends is not seen: it is code run with
//! the developer's rights, which rule 21 and `check-deps` hold, not this.
//!
//! Cargo's own configuration can set the variable for every compiler too,
//! through an `[env]` table: the check reads each configuration file cargo
//! would load and refuses one that does (`cargo_env`).
//!
//! The ban arrays are read in one format only (`read`); a ban key or a
//! ban array line in any other form is refused, not skipped. Keys are
//! matched as TOML decodes them, so a quoted or escaped spelling of a ban
//! key is refused too, and a quoted key that cannot be decoded is refused.
//!
//! What the bans cannot see is named in the root `clippy.toml`. The core's
//! own rules (`ironafs-client` names no clock at all) are in `check-deps`,
//! and the pure crates cannot name the standard library (`check-pure`).

mod cargo_env;
mod self_test;

use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::fixture::write_file;
use crate::pure::{CLEARED_ENVIRONMENT, first_error};
use crate::{Command, Error};
use crate::{json, metadata};

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
    /// The root `clippy.toml` bans a path no planted line proves.
    Uncovered { lint: Lint, path: String },
    /// `CLIPPY_CONF_DIR` is set, to the value given.
    Overridden(String),
    /// A line, numbered from 1, in a ban format `entries` cannot read.
    Unreadable { line: usize, text: String },
    /// A package's build script sets `name` for its crate's compiler,
    /// a variable that changes what clippy runs with.
    BuildOverride { package: String, name: String },
    /// The workspace's clippy did not finish, with the error given.
    Unbuilt(String),
    /// A cargo configuration's `[env]` table sets `name`, a variable
    /// clippy reads, for every compiler cargo runs, on the line given.
    CargoEnv { line: usize, name: String },
    /// A cargo configuration includes another file, on the line given.
    CargoInclude { line: usize },
    /// A cargo configuration file cargo would read cannot be read.
    CargoUnread(String),
    /// The configuration was added, removed or rewritten while the
    /// workspace was built.
    Changed,
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
            Self::Uncovered { lint, path } => write!(
                formatter,
                "clippy.toml bans the {} {path}, but check-bans plants no line that reaches \
                 it, so nothing proves the ban fires: add one to BANS (coding standards rule 7)",
                lint.word()
            ),
            Self::Overridden(value) => write!(
                formatter,
                "{CONF_DIR} is set (to {value:?}), so clippy would lint every crate with that \
                 directory's configuration instead of the files this check proves: unset it"
            ),
            Self::Unreadable { line, text } => write!(
                formatter,
                "line {line} is in a ban format check-bans cannot read, so its bans would go \
                 unchecked: `{text}`; write each ban as `{{ path = \"...\", reason = \"...\" }},` \
                 on its own line under `disallowed-methods = [` or `disallowed-types = [`"
            ),
            Self::BuildOverride { package, name } => write!(
                formatter,
                "the build script of {package} sets {name} for its crate's compiler, which \
                 changes the configuration or lints clippy runs with there: remove it"
            ),
            Self::CargoEnv { line, name } => write!(
                formatter,
                "line {line} sets {name} in cargo's [env] table, which cargo gives every \
                 compiler it runs, so clippy would read it although the check removes it: \
                 remove it"
            ),
            Self::CargoInclude { line } => write!(
                formatter,
                "line {line} includes another cargo configuration, which check-bans does not \
                 follow, so an [env] table there would go unchecked: remove it"
            ),
            Self::CargoUnread(error) => write!(
                formatter,
                "cargo would read this configuration, but check-bans cannot, so an [env] table \
                 in it would go unchecked: {error}"
            ),
            Self::Changed => write!(
                formatter,
                "added, removed or rewritten while the workspace was built, so clippy may have \
                 linted with a configuration this check did not prove: a build script writes \
                 it; remove that"
            ),
            Self::Unbuilt(error) => write!(
                formatter,
                "the workspace's clippy did not finish, so not every build script's output was \
                 read: {error}"
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

/// The variable through which clippy reads one directory's configuration
/// for every crate, before and instead of the one nearest each crate.
/// Set from outside, it would lint the workspace with a file this check
/// never saw, so the check refuses to run under it and `cargo xtask check`
/// lints without it.
pub(crate) const CONF_DIR: &str = "CLIPPY_CONF_DIR";

/// A ban key's line, `disallowed-methods = [` or `disallowed-types = [`,
/// indented or not, and the lint its array lists.
fn key(trimmed: &str) -> Option<Lint> {
    match trimmed {
        "disallowed-methods = [" => Some(Lint::Method),
        "disallowed-types = [" => Some(Lint::Type),
        _ => None,
    }
}

/// The path of a ban written as `{ path = "..." }` or
/// `{ path = "...", reason = "..." }`, optionally followed by a comma: one
/// inline table holding one entry. Neither string may hold a quote or an
/// escape, so the text read is the text TOML decodes and nothing after the
/// path can open another entry.
fn entry(trimmed: &str) -> Option<&str> {
    let plain = |text: &str| !text.contains(['"', '\\']);
    let table = trimmed.strip_suffix(',').unwrap_or(trimmed);
    let inner = table.strip_prefix("{ path = \"")?.strip_suffix(" }")?;
    let (path, rest) = inner.split_once('"')?;
    let reason = rest
        .strip_prefix(", reason = \"")
        .and_then(|reason| reason.strip_suffix('"'));
    let readable = rest.is_empty() || reason.is_some_and(plain);
    (plain(path) && readable).then_some(path)
}

/// The keys a TOML line starts with, each part decoded as TOML decodes it:
/// a table header's (`[a.b]`, `[[a]]`) or a key's (`a."b" = ...`). A line
/// that starts with no key gives none; one whose quoted part TOML could not
/// decode gives `None`. The decoded keys, not their spelling, say whether a
/// line names a ban.
fn line_keys(trimmed: &str) -> Option<Vec<String>> {
    let mut rest = trimmed.trim_start_matches('[').trim_start();
    let mut keys = Vec::new();
    loop {
        let (key, after) = if let Some(quoted) = rest.strip_prefix('"') {
            basic_string(quoted)?
        } else if let Some(quoted) = rest.strip_prefix('\'') {
            let (key, after) = quoted.split_once('\'')?;
            (key.to_owned(), after)
        } else {
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(rest.len());
            if end == 0 {
                return Some(keys);
            }
            let (key, after) = rest.split_at(end);
            (key.to_owned(), after)
        };
        keys.push(key);
        match after.trim_start().strip_prefix('.') {
            Some(next) => rest = next.trim_start(),
            None => return Some(keys),
        }
    }
}

/// The text of a TOML basic string whose opening quote is already read,
/// with its escapes decoded (TOML 1.0's, and 1.1's `\e` and `\xHH`), and
/// what follows its closing quote; `None` if it is not closed or an escape
/// is not one TOML decodes.
fn basic_string(text: &str) -> Option<(String, &str)> {
    let mut decoded = String::new();
    let mut chars = text.char_indices();
    while let Some((at, c)) = chars.next() {
        match c {
            '"' => return Some((decoded, text.get(at + 1..)?)),
            '\\' => {
                let (_, escape) = chars.next()?;
                let digits = match escape {
                    'x' => 2,
                    'u' => 4,
                    'U' => 8,
                    _ => 0,
                };
                let simple = match escape {
                    'b' => Some('\u{8}'),
                    't' => Some('\t'),
                    'n' => Some('\n'),
                    'f' => Some('\u{c}'),
                    'r' => Some('\r'),
                    'e' => Some('\u{1b}'),
                    '"' => Some('"'),
                    '\\' => Some('\\'),
                    _ => None,
                };
                if let Some(simple) = simple {
                    decoded.push(simple);
                } else if digits == 0 {
                    return None;
                } else {
                    let hex: String = (0..digits)
                        .map(|_| chars.next().map(|(_, digit)| digit))
                        .collect::<Option<String>>()?;
                    if !hex.chars().all(|digit| digit.is_ascii_hexdigit()) {
                        return None;
                    }
                    decoded.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
            }
            _ => decoded.push(c),
        }
    }
    None
}

/// Whether `text` names a ban key in any spelling clippy might read: either
/// lint, singular or plural, with `-` or `_`, in any case.
fn names_a_ban(text: &str) -> bool {
    let text = text.to_ascii_lowercase().replace('_', "-");
    ["disallowed-method", "disallowed-type"]
        .iter()
        .any(|stem| text.contains(stem))
}

/// Reads the `disallowed-methods` and `disallowed-types` arrays of
/// `clippy_toml` in the one format it accepts: the key on its own line,
/// each ban on its own line as `entry` reads it, and `]` on its own line.
/// Returns each entry with its lint, and each line that names a ban key or
/// sits in a ban array in any other form, so a ban written otherwise is
/// refused rather than skipped.
fn read(clippy_toml: &str) -> (Vec<(Lint, &str)>, Vec<Violation>) {
    let mut lint = None;
    let mut found = Vec::new();
    let mut refused = Vec::new();
    for (index, line) in clippy_toml.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let readable = match lint {
            Some(_) if trimmed == "]" => {
                lint = None;
                true
            }
            Some(current) => entry(trimmed)
                .map(|path| found.push((current, path)))
                .is_some(),
            None => {
                lint = key(trimmed);
                let named = line_keys(trimmed)
                    .map(|keys| names_a_ban(trimmed) || keys.iter().any(|key| names_a_ban(key)));
                lint.is_some() || named == Some(false)
            }
        };
        if !readable {
            refused.push(Violation::Unreadable {
                line: index + 1,
                text: line.to_owned(),
            });
        }
    }
    (found, refused)
}

/// Each line of `clippy_toml` in a ban format `read` cannot read.
pub(crate) fn unreadable(clippy_toml: &str) -> Vec<Violation> {
    read(clippy_toml).1
}

/// Each method or type ban of the root `clippy_toml` that no entry of
/// `BANS` plants under the same lint: nothing would prove it fires. A
/// member's own file may ban more (its allocations, say), which rule 7
/// does not need proved.
pub(crate) fn uncovered(clippy_toml: &str) -> Vec<Violation> {
    read(clippy_toml)
        .0
        .into_iter()
        .filter(|&(lint, path)| !BANS.iter().any(|ban| ban.lint == lint && ban.path == path))
        .map(|(lint, path)| Violation::Uncovered {
            lint,
            path: path.to_owned(),
        })
        .collect()
}

/// Whether `file`, relative to the workspace root, is a configuration in
/// the root itself, under either name clippy looks for.
pub(crate) fn root_config(file: &Path) -> bool {
    file.parent() == Some(Path::new("")) && CONFIG_NAMES.iter().any(|name| file == Path::new(name))
}

/// Whether a build script's `cargo::rustc-env` variable `name` changes what
/// clippy runs with: where its lookup starts (`CARGO_MANIFEST_DIR`), the
/// directory it reads in place of that lookup (`CLIPPY_CONF_DIR`), or its
/// lint arguments (`CLIPPY_ARGS`) and any other variable of its own.
///
/// Names are compared without regard to case on every host. Windows reads
/// environment names so, and the reports read may come from a build for
/// another host or target than the one this check runs on; refusing
/// `clippy_conf_dir` on a host that would not read it costs nothing.
fn clippy_reads(name: &str) -> bool {
    let name = name.to_uppercase();
    name == "CARGO_MANIFEST_DIR" || name.contains("CLIPPY")
}

/// What a build script reported, as cargo's `build-script-executed`
/// `message` gives it: its package and the name of each variable it sets
/// for its crate's compiler. A report missing either, or with a variable
/// that is not a `[name, value]` pair of strings, is refused with why.
fn script_report(message: &json::Value) -> Result<(&str, Vec<&str>), String> {
    let package = message
        .get("package_id")
        .and_then(json::Value::as_str)
        .ok_or("cargo reported a build script's output with no package")?;
    let pairs = message
        .get("env")
        .and_then(json::Value::as_array)
        .ok_or_else(|| format!("cargo reported {package}'s build script with no env list"))?;
    let names = pairs
        .iter()
        .map(|pair| match pair.as_array() {
            Some([name, value]) => name.as_str().filter(|_| value.as_str().is_some()),
            _ => None,
        })
        .collect::<Option<Vec<&str>>>()
        .ok_or_else(|| {
            format!("cargo reported {package}'s build script with a variable that is not a name and value")
        })?;
    Ok((package, names))
}

/// The package whose build script cargo's `compiler-artifact` `message`
/// says it compiled, if it is one; `Err` if it names no package.
fn script_built(message: &json::Value) -> Result<Option<&str>, String> {
    let kinds = message
        .get("target")
        .and_then(|target| target.get("kind"))
        .and_then(json::Value::as_array)
        .unwrap_or_default();
    if !kinds
        .iter()
        .any(|kind| kind.as_str() == Some("custom-build"))
    {
        return Ok(None);
    }
    message
        .get("package_id")
        .and_then(json::Value::as_str)
        .map(Some)
        .ok_or_else(|| "cargo reported a build script compiled for no package".to_owned())
}

/// Each variable clippy reads that a build script sets for its crate's
/// compiler, from cargo's JSON `messages`. Cargo applies a build script's
/// `rustc-env` after the variables `cargo xtask check` removes, so they
/// are read from what the build scripts reported.
///
/// The messages are read closed: a line that is not JSON, a report that
/// is not whole, a build script compiled with no report of its run, or no
/// single `build-finished` record saying the build succeeded, is refused,
/// so a pass means every build script's report was read.
pub(crate) fn build_overrides(messages: &str) -> Vec<Violation> {
    let mut found = Vec::new();
    let mut built = Vec::new();
    let mut reported = Vec::new();
    let mut finished = Vec::new();
    for line in messages.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(message) = json::parse(line) else {
            found.push(Violation::Unbuilt(
                "cargo printed a line check-bans cannot parse as a message".to_owned(),
            ));
            continue;
        };
        let read = match message.get("reason").and_then(json::Value::as_str) {
            Some("build-finished") => {
                finished.push(message.get("success") == Some(&json::Value::Bool(true)));
                Ok(())
            }
            Some("compiler-artifact") => {
                script_built(&message).map(|package| built.extend(package.map(str::to_owned)))
            }
            Some("build-script-executed") => script_report(&message).map(|(package, names)| {
                reported.push(package.to_owned());
                found.extend(
                    names
                        .into_iter()
                        .filter(|name| clippy_reads(name))
                        .map(|name| Violation::BuildOverride {
                            package: package.to_owned(),
                            name: name.to_owned(),
                        }),
                );
            }),
            _ => Ok(()),
        };
        if let Err(error) = read {
            found.push(Violation::Unbuilt(error));
        }
    }
    for package in built.iter().filter(|package| !reported.contains(package)) {
        found.push(Violation::Unbuilt(format!(
            "cargo compiled {package}'s build script but reported no output from it"
        )));
    }
    if finished != [true] {
        found.push(Violation::Unbuilt(
            "cargo did not report one build-finished record saying the build succeeded".to_owned(),
        ));
    }
    found
}

/// The refusal to check while `value`, the environment's `CLIPPY_CONF_DIR`,
/// is set, whatever it names.
pub(crate) fn overridden(value: Option<&OsStr>) -> Option<Found> {
    value.map(|value| Found {
        file: CONF_DIR.to_owned(),
        violation: Violation::Overridden(value.to_string_lossy().into_owned()),
    })
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
        .env(CONF_DIR, dir)
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
/// configuration a member can be linted with (`config_files`), each in a
/// format `read` accepts (`unreadable`) and with fixtures under its own
/// directory below `dir`. Every ban of a root configuration, under either
/// name, must be planted (`uncovered`). With `conf_dir`, the environment's
/// `CLIPPY_CONF_DIR`, set, clippy would read none of these, so the check
/// refuses before it lints anything (`overridden`).
///
/// # Errors
///
/// Returns an error if a configuration cannot be read, a fixture cannot be
/// written or cargo cannot be started.
pub(crate) fn check_tree(
    root: &Path,
    member_dirs: &[PathBuf],
    conf_dir: Option<&OsStr>,
    toolchain_toml: &str,
    dir: &Path,
) -> Result<Vec<Found>, Error> {
    if let Some(refusal) = overridden(conf_dir) {
        return Ok(vec![refusal]);
    }
    let mut files = config_files(root, member_dirs, Path::is_file);
    let own = Path::new("clippy.toml");
    if !files.iter().any(|file| file == own) {
        files.insert(0, own.to_path_buf());
    }
    let mut found = Vec::new();
    for (index, file) in files.iter().enumerate() {
        let path = root.join(file);
        let text = fs::read_to_string(&path).map_err(Error::path_io("read", &path))?;
        let name = file.display().to_string();
        let mut violations = unreadable(&text);
        if root_config(file) {
            violations.extend(uncovered(&text));
        }
        violations.extend(check(&text, toolchain_toml, &dir.join(index.to_string()))?);
        found.extend(violations.into_iter().map(|violation| Found {
            file: name.clone(),
            violation,
        }));
    }
    Ok(found)
}

/// The workspace's clippy run, with every crate linted under the
/// configuration nearest it, the files this check proves: an inherited
/// `CLIPPY_CONF_DIR` would put one other file in their place.
pub(crate) fn workspace_clippy() -> Command {
    let mut command = Command::new("cargo");
    command
        .args(["clippy", "--all-targets", "--workspace"])
        .env("CARGO_BUILD_WARNINGS", "deny")
        .env_remove(CONF_DIR);
    command
}

/// A configuration clippy can read, relative to the workspace root, and
/// its bytes.
pub(crate) type Snapshot = Vec<(PathBuf, Vec<u8>)>;

/// Each configuration in `before` or `after` that one of them lacks or
/// holds other bytes in.
pub(crate) fn changed(before: &Snapshot, after: &Snapshot) -> Vec<Found> {
    let mut files: Vec<&PathBuf> = before.iter().chain(after).map(|(file, _)| file).collect();
    files.sort();
    files.dedup();
    let bytes = |snapshot: &'_ Snapshot, file: &PathBuf| {
        snapshot
            .iter()
            .find_map(|(named, bytes)| (named == file).then(|| bytes.clone()))
    };
    files
        .into_iter()
        .filter(|file| bytes(before, file) != bytes(after, file))
        .map(|file| Found {
            file: file.display().to_string(),
            violation: Violation::Changed,
        })
        .collect()
}

/// Each configuration clippy can read for a member crate in `member_dirs`
/// of the workspace at `root` (`config_files`), with its bytes now.
///
/// # Errors
///
/// Returns an error if a configuration cannot be read.
pub(crate) fn snapshot(root: &Path, member_dirs: &[PathBuf]) -> Result<Snapshot, Error> {
    config_files(root, member_dirs, Path::is_file)
        .into_iter()
        .map(|file| {
            let path = root.join(&file);
            fs::read(&path)
                .map(|bytes| (file, bytes))
                .map_err(Error::path_io("read", &path))
        })
        .collect()
}

/// Runs `build`, which builds the workspace at `root` whose members are in
/// `member_dirs`, and adds to what it found each configuration a build
/// script added, removed or rewrote while it ran (`changed`).
///
/// Cargo runs a build script before compiling its crate, so one that
/// writes a configuration changes what clippy lints that crate, and those
/// after it, with. This sees every such change left in place when the
/// build ends. It cannot see a build script that writes a configuration
/// and puts the old bytes back before the build ends: a build script is
/// code run with the developer's rights, which no check of its output
/// bounds; what may run as one is held by rule 21 and `check-deps`.
///
/// # Errors
///
/// Returns an error if a configuration cannot be read, or `build` fails.
pub(crate) fn unchanged_by(
    root: &Path,
    member_dirs: &[PathBuf],
    build: impl FnOnce() -> Result<Vec<Found>, Error>,
) -> Result<Vec<Found>, Error> {
    let before = snapshot(root, member_dirs)?;
    let mut found = build()?;
    found.extend(changed(&before, &snapshot(root, member_dirs)?));
    Ok(found)
}

/// Runs the workspace's clippy (`workspace_clippy`) at the workspace the
/// current directory is in, and refuses a configuration a build script
/// changed while it ran, as `unchanged_by` does.
///
/// # Errors
///
/// Returns an error if clippy fails, or a configuration changed.
pub(crate) fn lint_workspace(name: &'static str) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    let member_dirs: Vec<PathBuf> = workspace
        .members
        .iter()
        .map(|member| member.dir.clone())
        .collect();
    let before = snapshot(&workspace.root, &member_dirs)?;
    // Cargo reads its configuration from the directory it runs in up, so
    // it runs in the root, whose configuration check-bans reads.
    let status = workspace_clippy()
        .current_dir(&workspace.root)
        .status()
        .map_err(Error::io(name))?;
    let found = changed(&before, &snapshot(&workspace.root, &member_dirs)?);
    if !found.is_empty() {
        return Err(Error::Rules {
            check: CHECK,
            violations: found.iter().map(ToString::to_string).collect(),
        });
    }
    Error::check_status(name, status)
}

/// What `Found` names for a build script's violation.
const BUILD_SCRIPTS: &str = "build scripts";

/// Runs the workspace's clippy at `root`, building into `target_dir`, and
/// refuses each variable clippy reads that a build script sets for its
/// crate's compiler (`build_overrides`): `CLIPPY_CONF_DIR` removed from
/// the environment comes back that way, and the lookup or lint arguments
/// with it. Cargo reports a build script's output again when it does not
/// rerun it, so a built workspace is read without rebuilding.
///
/// # Errors
///
/// Returns an error if cargo cannot be started.
pub(crate) fn check_build_scripts(root: &Path, target_dir: &Path) -> Result<Vec<Found>, Error> {
    let mut command = workspace_clippy();
    command
        .current_dir(root)
        .args(["--quiet", "--message-format", "json-render-diagnostics"])
        .arg("--target-dir")
        .arg(target_dir);
    let output = command.output().map_err(Error::io("cargo clippy"))?;
    let mut violations = build_overrides(&String::from_utf8_lossy(&output.stdout));
    if !output.status.success() {
        violations.push(Violation::Unbuilt(first_error(&output.stderr)));
    }
    Ok(violations
        .into_iter()
        .map(|violation| Found {
            file: BUILD_SCRIPTS.to_owned(),
            violation,
        })
        .collect())
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
    // Every cargo run below is in the root or under the target directory.
    let cargo = cargo_env::check(
        &[workspace.root.clone(), workspace.target_dir.clone()],
        cargo_env::cargo_home().as_deref(),
    );
    if !cargo.is_empty() {
        return Err(Error::Rules {
            check: CHECK,
            violations: cargo.iter().map(ToString::to_string).collect(),
        });
    }
    // The build scripts run first, so the configurations proved are the
    // ones left after them, and one they changed is refused.
    let build = unchanged_by(&workspace.root, &member_dirs, || {
        check_build_scripts(&workspace.root, &workspace.target_dir)
    })?;
    let dir = crate::fixture::fresh_dir(&workspace.target_dir, CHECK)?;
    let conf_dir = std::env::var_os(CONF_DIR);
    let result = check_tree(
        &workspace.root,
        &member_dirs,
        conf_dir.as_deref(),
        &toolchain_toml,
        &dir,
    );
    crate::fixture::remove_dir(&dir)?;
    let mut violations = build;
    violations.extend(result?);
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
        "check-bans: {} planted calls each tripped their ban in each of {}; no ban is \
         unresolved, and no build script or cargo configuration changes what clippy runs \
         with",
        BANS.len(),
        files.join(", ")
    )
    .map_err(Error::io("report check-bans"))
}
