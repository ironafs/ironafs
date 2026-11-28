// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The clock and randomness bans of coding standards rule 7, and the
//! deadline constructors' ban (the seams specification's second net, 5.3,
//! point 2), kept by clippy's `disallowed-methods` and `disallowed-types`
//! in the root `clippy.toml`, and proved to fire.
//!
//! `ironafs-time` has its own `clippy.toml`, since inside that crate the
//! constructors' path names nothing: it must be the root file without the
//! constructor bans, line for line apart from comments.
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
//! such file is checked the same way, and must carry every ban itself,
//! except `ironafs-time`'s, which is held to the root's lines without the
//! constructor bans (`crate_config_drift`) instead of linted on a fixture.
//! `CLIPPY_CONF_DIR` comes before all of them: clippy then reads the
//! directory it names for every crate. The check refuses to run while it
//! is set, before it builds anything, and `cargo xtask check` lints
//! without it. A build script can
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
//! None of those static reads can see everything that changes what the
//! workspace's lint runs (a cargo alias for `clippy`, a compiler wrapper,
//! flags or settings from the environment, a configuration rewritten and
//! put back), so the lint carries its own proof. The canary
//! (`crates/ironafs-bans-canary`, `canary`) plants every ban, one per
//! line, behind a feature only that lint turns on; the lint `cargo xtask
//! check` gates on, and check-bans, run clippy on the whole workspace with
//! it, and require each canary line's disallowed-item warning, on that
//! line, and no other warning. A missing one fails whatever its cause. The
//! canary must be exactly the text check-bans writes, with no other file
//! (a `clippy.toml`, a build script), and no crate may depend on it, as a
//! development dependency either. The lint also turns off any compiler
//! wrapper (`no_wrapper`), so none can run plain rustc for another member
//! while the canary is linted with clippy, and builds into a fresh, empty
//! directory (`isolated_build`), so no artifact an earlier run built
//! without clippy is reused. It runs clippy as `cargo check` with clippy's
//! driver (`clippy_json`), as `cargo clippy` does inside, so no cargo
//! alias is read. The canary proves the lint ran
//! with the root configuration's bans on one member;
//! a member's own configuration is proved by the fixture, and a change
//! that reaches only some members' compilation (a build script's variable
//! for its own crate, a configuration written and restored around one
//! crate) is seen only through the static reads above.
//!
//! The ban arrays are read in one format only (`read`); a ban key or a
//! ban array line in any other form is refused, not skipped. Keys are
//! matched as TOML decodes them, so a quoted or escaped spelling of a ban
//! key is refused too, and a quoted key that cannot be decoded is refused.
//!
//! What the bans cannot see is named in the root `clippy.toml`. The core's
//! own rules (`ironafs-client` names no clock at all) are in `check-deps`,
//! and the pure crates cannot name the standard library (`check-pure`).

mod canary;
mod cargo_env;
mod self_test;

use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::fixture::write_file;
use crate::metadata::Workspace;
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
        lint: Lint::Method,
        path: "ironafs_time::Deadline::boot_after",
        planted: "pub fn deadline_boot(now: &ironafs_time::Stamp) -> bool { ironafs_time::Deadline::boot_after(now, now, core::time::Duration::ZERO).is_ok() }",
    },
    Ban {
        lint: Lint::Method,
        path: "ironafs_time::Deadline::awake_after",
        planted: "pub fn deadline_awake() -> fn(&ironafs_time::Stamp, &ironafs_time::Stamp, core::time::Duration) -> Result<ironafs_time::Deadline, ironafs_time::DeadlineError> { ironafs_time::Deadline::awake_after }",
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
    /// The canary's ban on `path`, planted on `line` of its library, drew
    /// no diagnostic there in the workspace's lint.
    CanaryMissing { path: &'static str, line: usize },
    /// The workspace's lint gave a warning or error other than the
    /// canary's, rendered as given.
    Lint(String),
    /// The canary is not as check-bans writes it, or a crate depends on it.
    Canary(String),
    /// The configuration was added, removed or rewritten while the
    /// workspace was built.
    Changed,
    /// `ironafs-time`'s own clippy.toml is not the root one without the
    /// constructor bans; the first line that differs.
    CrateConfigDrift(String),
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
            Self::CrateConfigDrift(line) => write!(
                formatter,
                "crates/ironafs-time/clippy.toml must be the root clippy.toml without the \
                 Deadline constructor bans; first difference at: {line}"
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
            Self::CanaryMissing { path, line } => write!(
                formatter,
                "the canary's call to {path} on line {line} of {}/src/lib.rs drew no \
                 disallowed-item warning in the workspace's lint, so that lint did not run \
                 clippy with the bans this check proves (an alias, a wrapper, a configuration \
                 or a variable changed it): find what and remove it",
                canary::DIR
            ),
            Self::Lint(rendered) => write!(
                formatter,
                "the workspace's lint gave a warning or error: {rendered}"
            ),
            Self::Canary(why) => write!(formatter, "the canary: {why}"),
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

/// The bans `ironafs-time`'s own clippy.toml leaves out: inside that crate
/// the paths name nothing, and the constructors are defined and tested
/// there (seams spec 5.3, point 2).
pub(crate) const TIME_CRATE_EXEMPT: &[&str] = &[
    "ironafs_time::Deadline::boot_after",
    "ironafs_time::Deadline::awake_after",
];

/// Where `ironafs-time`'s own clippy.toml is, from the workspace root.
pub(crate) const TIME_CRATE_CONFIG: &str = "crates/ironafs-time/clippy.toml";

/// The lines of a clippy.toml that configure anything: not blank, not a
/// comment.
fn config_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty() && !line.starts_with('#')
        })
        .collect()
}

/// Checks that `own`, a crate's clippy.toml, configures exactly what the
/// root's `root` does without the constructor bans, in the same order:
/// clippy reads the nearest file whole, so any other difference would
/// drop or change a ban for that crate. Comments may differ.
pub(crate) fn crate_config_drift(root: &str, own: &str) -> Vec<Violation> {
    let expected: Vec<&str> = config_lines(root)
        .into_iter()
        .filter(|line| {
            !TIME_CRATE_EXEMPT
                .iter()
                .any(|path| line.contains(&format!("\"{path}\"")))
        })
        .collect();
    let own = config_lines(own);
    if own == expected {
        return Vec::new();
    }
    let index = own
        .iter()
        .zip(&expected)
        .position(|(own, expected)| own != expected)
        .unwrap_or_else(|| own.len().min(expected.len()));
    let line = match (own.get(index), expected.get(index)) {
        (Some(extra), _) if !expected.contains(extra) => extra,
        (_, Some(missing)) => missing,
        (Some(extra), None) => extra,
        (None, None) => "",
    };
    vec![Violation::CrateConfigDrift(line.to_owned())]
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
    // TOML skips a leading byte order mark; so does this.
    let clippy_toml = clippy_toml.strip_prefix('\u{feff}').unwrap_or(clippy_toml);
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
        .and_then(|kinds| {
            kinds
                .iter()
                .map(json::Value::as_str)
                .collect::<Option<Vec<_>>>()
        })
        .ok_or_else(|| {
            "cargo reported an artifact whose target kinds it did not list".to_owned()
        })?;
    if !kinds.contains(&"custom-build") {
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
    let mut last = None;
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
            Some("compiler-message") => Ok(()),
            _ => Err("cargo printed a message with no reason check-bans knows".to_owned()),
        };
        last = message
            .get("reason")
            .and_then(json::Value::as_str)
            .map(str::to_owned);
        if let Err(error) = read {
            found.push(Violation::Unbuilt(error));
        }
    }
    // Cargo may run one compiled build script for more than one unit, so
    // each compiled script needs a report, and may have more.
    let count = |list: &[String], package: &String| list.iter().filter(|id| *id == package).count();
    let mut checked: Vec<&String> = Vec::new();
    for package in &built {
        if checked.contains(&package) {
            continue;
        }
        checked.push(package);
        if count(&reported, package) < count(&built, package) {
            found.push(Violation::Unbuilt(format!(
                "cargo compiled {package}'s build script but reported no output from it"
            )));
        }
    }
    if finished != [true] || last.as_deref() != Some("build-finished") {
        found.push(Violation::Unbuilt(
            "cargo did not end with one build-finished record saying the build succeeded"
                .to_owned(),
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
/// toolchain file `toolchain_toml`, and none unresolved. `dir` is emptied
/// first, so no cargo configuration left in it (by a build script of the
/// workspace's lint, say) is read: the preflight scans only the
/// directories above it.
///
/// # Errors
///
/// Returns an error if the fixture cannot be written or cargo cannot be
/// started.
pub(crate) fn check(
    clippy_toml: &str,
    toolchain_toml: &str,
    time_crate: &Path,
    dir: &Path,
) -> Result<Vec<Violation>, Error> {
    crate::fixture::remove_dir(dir)?;
    let mut violations: Vec<Violation> = BANS
        .iter()
        .filter(|ban| !listed(clippy_toml, ban.path))
        .map(|ban| Violation::NotListed { path: ban.path })
        .collect();
    violations.dedup();
    // The constructors' bans need the crate that defines them.
    let manifest = format!(
        "[package]\nname = \"bans-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [dependencies]\nironafs-time = {{ path = '{}' }}\n\n\
         # Not part of the workspace whose target directory holds it.\n[workspace]\n",
        time_crate.display()
    );
    let files = [
        ("Cargo.toml", manifest.as_str()),
        ("clippy.toml", clippy_toml),
        ("rust-toolchain.toml", toolchain_toml),
    ];
    for (name, text) in files {
        write_file(&dir.join(name), text)?;
    }
    write_file(&dir.join("src").join("lib.rs"), &planted_source())?;
    let mut command = cargo();
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
    no_wrapper(&mut command);
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
/// of these must carry every ban itself (`ironafs-time`'s, every ban but
/// the constructors').
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
/// name, must be planted (`uncovered`). `ironafs-time`'s own file
/// (`TIME_CRATE_CONFIG`) is the one exception: its crate defines the
/// constructors, so it must be the root `clippy.toml` without their bans
/// (`crate_config_drift`), and it is held to that rather than linted on a
/// fixture. The fixtures depend on the crate at `time_crate`, which
/// defines the constructors. With `conf_dir`, the environment's
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
    time_crate: &Path,
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
    let root_path = root.join(own);
    let root_text = fs::read_to_string(&root_path).map_err(Error::path_io("read", &root_path))?;
    let mut found = Vec::new();
    for (index, file) in files.iter().enumerate() {
        let path = root.join(file);
        let text = fs::read_to_string(&path).map_err(Error::path_io("read", &path))?;
        let name = file.display().to_string();
        let mut violations = unreadable(&text);
        if root_config(file) {
            violations.extend(uncovered(&text));
        }
        if file == Path::new(TIME_CRATE_CONFIG) {
            violations.extend(crate_config_drift(&root_text, &text));
        } else {
            violations.extend(check(
                &text,
                toolchain_toml,
                time_crate,
                &dir.join(index.to_string()),
            )?);
        }
        found.extend(violations.into_iter().map(|violation| Found {
            file: name.clone(),
            violation,
        }));
    }
    Ok(found)
}

/// Cargo, run with its home named outright: a relative `CARGO_HOME` is
/// taken from each run's own directory, so it is resolved once
/// (`cargo_env::cargo_home`), and every run reads the home check-bans read.
fn cargo() -> Command {
    let mut command = Command::new("cargo");
    if let Some(home) = cargo_env::cargo_home() {
        command.env("CARGO_HOME", home);
    }
    command
}

/// Clippy on every target, with cargo's JSON messages, and without an
/// inherited `CLIPPY_CONF_DIR`, which would put one other file in place of
/// the configurations this check proves. It runs clippy as `cargo clippy`
/// does inside: `cargo check` with `driver`, the toolchain's
/// `clippy-driver` (`clippy_driver`), as the workspace wrapper
/// (`as_clippy`). `check` is built into cargo, which ignores an alias of
/// that name, so no alias, of `clippy` or any other, is consulted.
fn clippy_json(driver: &Path) -> Command {
    let mut command = cargo();
    command
        .args(["check", "--all-targets", "--message-format", "json"])
        .env_remove(CONF_DIR)
        .env_remove("CARGO_BUILD_WARNINGS");
    as_clippy(&mut command, driver);
    command
}

/// The toolchain's `clippy-driver`, beside the `rustc` that cargo run in
/// `root` uses, where `cargo clippy` finds it.
///
/// # Errors
///
/// Returns an error if the toolchain's sysroot cannot be read or holds no
/// `clippy-driver`.
pub(crate) fn clippy_driver(root: &Path) -> Result<PathBuf, Error> {
    let driver = crate::pure::toolchain_sysroot(root)?
        .join("bin")
        .join(format!("clippy-driver{}", std::env::consts::EXE_SUFFIX));
    if driver.is_file() {
        Ok(driver)
    } else {
        Err(Error::UnexpectedOutput(
            "rustc --print sysroot (no clippy-driver)",
        ))
    }
}

/// What `cargo clippy` sets on the `cargo check` it runs: `driver` as the
/// workspace wrapper, which the environment variable sets over any
/// configuration, and no extra clippy arguments. Set over anything the
/// caller set.
fn as_clippy(command: &mut Command, driver: &Path) {
    command
        .env("RUSTC_WORKSPACE_WRAPPER", driver)
        .env("CLIPPY_ARGS", "");
}

/// The workspace's lint, the one `cargo xtask check` gates on: clippy on
/// every target of every member with every feature on. That turns on the
/// canary's `planted` feature, and it compiles code behind a feature no
/// build enables, which no ban could otherwise reach; so does
/// `ironafs-disk`'s fault feature, which bans reach only this way (ADR
/// 0004, C4, point 3). The pure crates have no features (C4, point 1),
/// and the features outside them are meant to build with all the others,
/// so no feature can be one that only some builds may combine; one that
/// cannot build with the others fails the lint, which is the safe side. Its warnings are not denied by cargo,
/// since the canary's must appear; `lint` denies every other one.
pub(crate) fn workspace_clippy(driver: &Path) -> Command {
    let mut command = clippy_json(driver);
    command.args(["--workspace", "--all-features"]);
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
/// of the workspace at `root` (`config_files`), and each cargo
/// configuration cargo run in any of `cargo_dirs` reads
/// (`cargo_env::candidates`): the directories the preflight scanned. Each
/// with its bytes now.
///
/// # Errors
///
/// Returns an error if a configuration cannot be read.
pub(crate) fn snapshot(
    root: &Path,
    member_dirs: &[PathBuf],
    cargo_dirs: &[PathBuf],
) -> Result<Snapshot, Error> {
    // With no home known, the root's own `.cargo` stands in: check-bans
    // refuses to run without one (`cargo_env::check`).
    let home = cargo_env::cargo_home().unwrap_or_else(|| root.join(".cargo"));
    let cargo_configs = cargo_env::candidates(cargo_dirs, &home)
        .into_iter()
        .filter(|path| path.symlink_metadata().is_ok());
    config_files(root, member_dirs, Path::is_file)
        .into_iter()
        .chain(cargo_configs)
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
/// script added, removed or rewrote while it ran (`changed`): clippy's,
/// and cargo's for runs in any of `cargo_dirs`.
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
    cargo_dirs: &[PathBuf],
    build: impl FnOnce() -> Result<Vec<Found>, Error>,
) -> Result<Vec<Found>, Error> {
    let before = snapshot(root, member_dirs, cargo_dirs)?;
    let mut found = build()?;
    found.extend(changed(&before, &snapshot(root, member_dirs, cargo_dirs)?));
    Ok(found)
}

/// What `Found` names for a violation of the workspace's lint.
const LINT: &str = "workspace lint";

/// The canary's static refusals for `workspace`: it must be a member,
/// exactly as check-bans writes it, and no package may depend on it.
/// Returns them, and the canary's package id.
fn canary_state(workspace: &Workspace) -> Result<(Vec<Violation>, Option<String>), Error> {
    let Some(member) = workspace
        .members
        .iter()
        .find(|member| member.name == canary::NAME)
    else {
        return Ok((
            vec![Violation::Canary(format!(
                "{} is not a workspace member, so nothing proves the workspace's lint runs \
                 with the bans",
                canary::DIR
            ))],
            None,
        ));
    };
    let mut found = canary::elsewhere(&workspace.root, &member.dir);
    found.extend(canary::tampered(&canary::files(&workspace.root)?));
    found.extend(canary::depended_on(&workspace.packages, &member.id));
    Ok((found, Some(member.id.clone())))
}

/// Runs the workspace's lint (`workspace_clippy`) on `workspace`, in its
/// root, after `adjust` (which only the self-test uses, to plant an
/// evasion), and returns what is wrong: the canary's state
/// (`canary_state`), each canary line that drew no diagnostic and every
/// other warning (`canary::verdict`), each warning cargo itself prints
/// (`cargo_warnings`), each variable a build script sets
/// that clippy reads (`build_overrides`), and a run that failed.
///
/// # Errors
///
/// Returns an error if the canary cannot be read or cargo cannot start.
pub(crate) fn lint(
    workspace: &Workspace,
    adjust: impl FnOnce(&mut Command),
) -> Result<Vec<Found>, Error> {
    let (mut violations, canary_id) = canary_state(workspace)?;
    let paths: Vec<&Path> = [workspace.root.as_path()]
        .into_iter()
        .chain(workspace.members.iter().map(|member| member.dir.as_path()))
        .collect();
    violations.extend(line_breaks(&paths));
    violations.extend(outside_root(&workspace.root, &member_dirs(workspace)));
    let driver = clippy_driver(&workspace.root)?;
    let mut command = workspace_clippy(&driver);
    // Cargo reads its configuration from the directory it runs in up, so
    // it runs in the root, whose configuration check-bans reads.
    command.current_dir(&workspace.root);
    adjust(&mut command);
    // Over anything the caller set, so cargo's own warnings on standard
    // error are printed, each on its own line, in plain text.
    plain_terminal(&mut command);
    no_wrapper(&mut command);
    as_clippy(&mut command, &driver);
    let build = crate::fixture::fresh_dir(&workspace.target_dir, LINT_BUILD)?;
    if let Some(refusal) = template(&build) {
        violations.push(refusal);
        return Ok(found_in_lint(violations));
    }
    isolated_build(&mut command, &build)?;
    let output = command.output().map_err(Error::io("cargo clippy"));
    crate::fixture::remove_dir(&build)?;
    let output = output?;
    let messages = String::from_utf8_lossy(&output.stdout);
    violations.extend(build_overrides(&messages));
    violations.extend(cargo_warnings(&String::from_utf8_lossy(&output.stderr)));
    let member_ids: Vec<String> = workspace
        .members
        .iter()
        .map(|member| member.id.clone())
        .collect();
    violations.extend(canary::verdict(
        &messages,
        canary_id.as_deref().unwrap_or_default(),
        &member_ids,
    ));
    if !output.status.success() {
        violations.push(Violation::Unbuilt(first_error(&output.stderr)));
    }
    Ok(found_in_lint(violations))
}

/// `violations`, each as found in the workspace's lint.
fn found_in_lint(violations: Vec<Violation>) -> Vec<Found> {
    violations
        .into_iter()
        .map(|violation| Found {
            file: LINT.to_owned(),
            violation,
        })
        .collect()
}

/// A refusal if `dir` holds `{` or `}`: cargo reads its build directory
/// as a template, so such a path would name another directory.
fn template(dir: &Path) -> Option<Violation> {
    dir.to_string_lossy().contains(['{', '}']).then(|| {
        Violation::Unbuilt(format!(
            "the path \"{}\" holds {{ or }}, which cargo reads in a build directory as a \
             template: move the workspace",
            dir.to_string_lossy().escape_debug()
        ))
    })
}

/// Each warning or error cargo printed on its standard error, `stderr`,
/// in the workspace's lint: an unused manifest key, a build script's
/// `cargo::warning`, or any other. The compiler's diagnostics, the
/// canary's among them, arrive as JSON on standard output instead.
pub(crate) fn cargo_warnings(stderr: &str) -> Vec<Violation> {
    // A carriage return ends a line on a terminal too.
    stderr
        .split(['\n', '\r'])
        .map(str::trim)
        .filter(|line| {
            ["warning", "error"].iter().any(|word| {
                line.strip_prefix(word)
                    .is_some_and(|rest| rest.starts_with(':') || rest.starts_with('['))
            })
        })
        .map(|line| Violation::Lint(line.to_owned()))
        .collect()
}

/// The terminal settings the workspace's lint gives cargo, over any the
/// caller inherited.
/// Cargo takes these variables over its configuration files (only a
/// `--config` argument, which the lint never passes, comes first), so a
/// caller's `term.quiet` cannot hide a build script's warning, a progress
/// bar cannot share a line with one, and no colour code or extra line
/// changes how they read.
pub(crate) const TERMINAL: &[(&str, &str)] = &[
    ("CARGO_TERM_QUIET", "false"),
    ("CARGO_TERM_VERBOSE", "false"),
    ("CARGO_TERM_PROGRESS_WHEN", "never"),
    ("CARGO_TERM_COLOR", "never"),
];

/// Sets `TERMINAL` on `command`, and removes `CLICOLOR_FORCE`, which
/// colours a warning cargo prints before it reads `CARGO_TERM_COLOR`.
pub(crate) fn plain_terminal(command: &mut Command) {
    for (name, value) in TERMINAL {
        command.env(name, value);
    }
    command.env_remove("CLICOLOR_FORCE");
}

/// Runs cargo with no compiler wrapper around clippy's. An empty
/// `RUSTC_WRAPPER` turns off one set by the environment
/// (`CARGO_BUILD_RUSTC_WRAPPER` too) or by any cargo configuration, and
/// `as_clippy` sets the workspace wrapper to clippy's driver over one set
/// either way; so no wrapper can run plain rustc for some members while
/// the canary is linted. Set over anything the caller set.
fn no_wrapper(command: &mut Command) {
    command.env("RUSTC_WRAPPER", "");
}

/// The directory, under the workspace's target directory, that the
/// workspace's lint builds into, fresh for each run.
const LINT_BUILD: &str = "check-bans-lint";

/// Builds into `dir` for both cargo's target directory and its build
/// directory (`build.build-dir`, which a cargo configuration can point
/// elsewhere). Cargo reuses an artifact whose key matches, and no compiler
/// wrapper is part of that key: an artifact an earlier run built without
/// clippy, under a wrapper, would be reused and its crate never linted.
/// The lint gives it a fresh, empty `dir`; `check-os` gives its pinned
/// runs a directory of their own. Set over anything the caller set.
///
/// Cargo reads `{` and `}` in a build directory as its path templates
/// (`{workspace-root}` and others) and documents no escape for them, so a
/// `dir` holding either is refused, with nothing set, rather than passed
/// on to mean another directory.
///
/// # Errors
///
/// Returns an error naming `dir` if its path holds `{` or `}`.
pub(crate) fn isolated_build(command: &mut Command, dir: &Path) -> Result<(), Error> {
    if dir
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .any(|byte| matches!(byte, b'{' | b'}'))
    {
        return Err(Error::path_io("use as Cargo's build directory", dir)(
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the path holds `{` or `}`, which Cargo reads as a build-directory template, \
                 and Cargo documents no escape",
            ),
        ));
    }
    command
        .arg("--target-dir")
        .arg(dir)
        .env("CARGO_BUILD_BUILD_DIR", dir);
    Ok(())
}

/// A refusal for each of `paths` that holds a line break.
/// Cargo prints package paths in its text output, where a line break
/// would let a path read as a warning of its own, or hide one.
pub(crate) fn line_breaks(paths: &[&Path]) -> Vec<Violation> {
    paths
        .iter()
        .filter(|path| path.to_string_lossy().contains(['\n', '\r']))
        .map(|path| {
            Violation::Unbuilt(format!(
                "the path \"{}\" holds a line break, which cargo's text output cannot carry \
                 unambiguously: move the workspace",
                path.to_string_lossy().escape_debug()
            ))
        })
        .collect()
}

/// A refusal for each of `member_dirs` outside the workspace's `root`.
///
/// The configurations a member is linted with are read only from its
/// directory up to the root (`config_files`), so one outside it would be
/// linted with a configuration nothing checks.
pub(crate) fn outside_root(root: &Path, member_dirs: &[PathBuf]) -> Vec<Violation> {
    member_dirs
        .iter()
        .filter(|dir| !dir.starts_with(root))
        .map(|dir| {
            Violation::Unbuilt(format!(
                "the member at {} is outside the workspace root, so the clippy configuration \
                 it is linted with is not checked: move it under the root",
                dir.display()
            ))
        })
        .collect()
}

/// The member directories of `workspace`.
fn member_dirs(workspace: &Workspace) -> Vec<PathBuf> {
    workspace
        .members
        .iter()
        .map(|member| member.dir.clone())
        .collect()
}

/// `cargo xtask check`'s lint: the workspace's lint (`lint`), after the
/// same refusal of cargo configurations as check-bans makes for the root
/// (`cargo_env::check`), refusing a configuration a build script changed
/// while it ran (`unchanged_by`).
///
/// # Errors
///
/// Returns the violations, or why the lint could not run.
pub(crate) fn lint_workspace() -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    let roots = [workspace.root.clone()];
    let cargo = cargo_env::check(&roots, cargo_env::cargo_home().as_deref());
    let found = if cargo.is_empty() {
        unchanged_by(&workspace.root, &member_dirs(&workspace), &roots, || {
            lint(&workspace, |_| {})
        })?
    } else {
        cargo
    };
    if found.is_empty() {
        Ok(())
    } else {
        Err(Error::Rules {
            check: CHECK,
            violations: found.iter().map(ToString::to_string).collect(),
        })
    }
}

/// What `Found` names for a build script's violation.
const BUILD_SCRIPTS: &str = "build scripts";

/// Runs clippy on the crate at `root`, building into `target_dir`, and
/// refuses each variable clippy reads that its build script sets for its
/// compiler (`build_overrides`). The self-test's planted build scripts
/// are checked so; the workspace's are read from its lint (`lint`).
///
/// # Errors
///
/// Returns an error if cargo cannot be started.
pub(crate) fn check_build_scripts(root: &Path, target_dir: &Path) -> Result<Vec<Found>, Error> {
    let mut command = clippy_json(&clippy_driver(root)?);
    command
        .current_dir(root)
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
    let time_config = read(TIME_CRATE_CONFIG)?;
    let time_crate = workspace.root.join("crates").join("ironafs-time");
    if self_test {
        let root_manifest = read("Cargo.toml")?;
        return self_test::run(
            &workspace.target_dir,
            &root_manifest,
            &clippy_toml,
            &toolchain_toml,
            &time_config,
            &time_crate,
            output,
        );
    }
    let refuse = |found: &[Found]| Error::Rules {
        check: CHECK,
        violations: found.iter().map(ToString::to_string).collect(),
    };
    // Refused before anything is built.
    let conf_dir = std::env::var_os(CONF_DIR);
    if let Some(refusal) = overridden(conf_dir.as_deref()) {
        return Err(refuse(&[refusal]));
    }
    let member_dirs = member_dirs(&workspace);
    let dir = crate::fixture::fresh_dir(&workspace.target_dir, CHECK)?;
    // Every cargo run below is in the root or under the fixtures' directory.
    let cargo = cargo_env::check(
        &[workspace.root.clone(), dir.clone()],
        cargo_env::cargo_home().as_deref(),
    );
    // The snapshot below covers the same directories.
    if !cargo.is_empty() {
        return Err(refuse(&cargo));
    }
    // The workspace's lint runs first, so the configurations proved are
    // the ones left after its build scripts, and one they changed is
    // refused.
    let scanned = [workspace.root.clone(), dir.clone()];
    let linted = unchanged_by(&workspace.root, &member_dirs, &scanned, || {
        lint(&workspace, |_| {})
    })?;
    let result = check_tree(
        &workspace.root,
        &member_dirs,
        conf_dir.as_deref(),
        &toolchain_toml,
        &time_crate,
        &dir,
    );
    crate::fixture::remove_dir(&dir)?;
    let mut violations = linted;
    violations.extend(result?);
    if !violations.is_empty() {
        return Err(refuse(&violations));
    }
    let files: Vec<String> = config_files(&workspace.root, &member_dirs, Path::is_file)
        .iter()
        .filter(|file| *file != Path::new(TIME_CRATE_CONFIG))
        .map(|file| file.display().to_string())
        .collect();
    writeln!(
        output,
        "check-bans: {} planted calls each tripped their ban in each of {}; no ban is \
         unresolved; the workspace's lint drew each of the canary's {} warnings on its line \
         and no other; no build script reported a variable clippy reads, and none left a \
         configuration changed; no cargo configuration read sets one; {TIME_CRATE_CONFIG} is \
         clippy.toml without the constructor bans",
        BANS.len(),
        files.join(", "),
        BANS.len()
    )
    .map_err(Error::io("report check-bans"))
}
