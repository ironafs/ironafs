// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! No test is ever retried (coding standards rule 15; the testing
//! specification, 5.4, point 6).
//!
//! nextest takes its retry count from a profile, a per-test override, its
//! command line and an environment variable. This check reads each place
//! in the repository that can set one: the profiles and overrides in
//! `.config/nextest.toml`; the commands and environments of everything
//! under `.github/` and of every cargo configuration file (aliases and
//! `[env]`); and the test run of `cargo xtask check`. A value other than a
//! plain 0 fails, and so does a command that points nextest at a
//! configuration file this check does not read.
//!
//! Commands are matched on raw text, comments included, so the check fails
//! closed rather than parse YAML or shell.

mod self_test;

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{Error, metadata};

#[cfg(test)]
mod tests;

const CHECK: &str = "check-retries";

/// The nextest configuration file, relative to the workspace root.
const PROFILES: &str = ".config/nextest.toml";

/// The directory of GitHub's workflows and actions, relative to the root.
const GITHUB: &str = ".github";

/// The file that holds `cargo xtask check`'s test run.
const XTASK_CHECK: &str = "crates/xtask/src/check.rs";

/// nextest's command-line option and environment variable for retries.
const OPTION: &str = "--retries";
const VARIABLE: &str = "NEXTEST_RETRIES";

/// nextest's options that read another configuration file.
const CONFIG_OPTIONS: &[&str] = &["--config-file", "--tool-config-file"];

/// One place that could make nextest retry a test.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    /// The nextest configuration file is missing, so no profile was read.
    ProfilesMissing(PathBuf),
    /// A profile or per-test override sets `retries` to something other
    /// than 0.
    Profile {
        file: PathBuf,
        line: usize,
        value: String,
    },
    /// A command passes the retries option with something other than 0.
    Option {
        file: PathBuf,
        line: usize,
        value: String,
    },
    /// An environment sets nextest's retries variable to something other
    /// than 0.
    Environment {
        file: PathBuf,
        line: usize,
        value: String,
    },
    /// A command names a nextest configuration file this check does not read.
    ConfigFile {
        file: PathBuf,
        line: usize,
        option: &'static str,
    },
    /// The nextest configuration file cannot be read as TOML.
    Unreadable { file: PathBuf, error: String },
}

/// How a value is shown in a message: quoted, or "nothing".
fn shown(value: &str) -> String {
    if value.is_empty() {
        "nothing".to_owned()
    } else {
        format!("`{value}`")
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        const RULE: &str = "no test is ever retried (coding standards rule 15)";
        match self {
            Self::ProfilesMissing(file) => write!(
                formatter,
                "{} does not exist, so no nextest profile was read: {RULE}",
                file.display()
            ),
            Self::Profile { file, line, value } => write!(
                formatter,
                "{}:{line}: sets retries to {}; every profile and per-test override sets \
                 retries = 0: {RULE}",
                file.display(),
                shown(value)
            ),
            Self::Option { file, line, value } => write!(
                formatter,
                "{}:{line}: passes {OPTION} {}; a command passes {OPTION} 0: {RULE}",
                file.display(),
                shown(value)
            ),
            Self::Environment { file, line, value } => write!(
                formatter,
                "{}:{line}: sets {VARIABLE} to {}; only 0 is allowed: {RULE}",
                file.display(),
                shown(value)
            ),
            Self::ConfigFile { file, line, option } => write!(
                formatter,
                "{}:{line}: passes {option}, a nextest configuration this check does not read; \
                 profiles live in {PROFILES}",
                file.display()
            ),
            Self::Unreadable { file, error } => write!(
                formatter,
                "{}: cannot read as TOML, so its retries are unknown: {error}",
                file.display()
            ),
        }
    }
}

/// A token of a TOML file. Comments are dropped; a string keeps its
/// spelling, quotes included, and its content.
#[derive(Debug, PartialEq, Eq)]
enum Toml {
    Bare(String),
    Str { raw: String, content: String },
    Punct(char),
    Newline,
}

/// Whether a character belongs to a bare TOML key or a plain value.
fn is_bare(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '+')
}

/// Splits TOML text into tokens, each with its line. Strings of all four
/// kinds are read whole, so a `#` or a line inside one is not a comment
/// or a key.
fn toml_tokens(text: &str) -> Result<Vec<(Toml, usize)>, String> {
    let characters: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut line = 1;
    let mut at = 0;
    let starts = |from: usize, pattern: &str| {
        pattern
            .chars()
            .enumerate()
            .all(|(offset, expected)| characters.get(from + offset) == Some(&expected))
    };
    while let Some(&character) = characters.get(at) {
        if character == '\n' {
            tokens.push((Toml::Newline, line));
            line += 1;
            at += 1;
        } else if character.is_whitespace() {
            at += 1;
        } else if character == '#' {
            while characters.get(at).is_some_and(|&next| next != '\n') {
                at += 1;
            }
        } else if character == '"' || character == '\'' {
            let multiline = starts(at, if character == '"' { "\"\"\"" } else { "'''" });
            let quotes = if multiline { 3 } else { 1 };
            let start_line = line;
            let start = at;
            at += quotes;
            let content_start = at;
            loop {
                let Some(&next) = characters.get(at) else {
                    return Err(format!("line {start_line}: unterminated string"));
                };
                let closes = if multiline {
                    starts(at, if character == '"' { "\"\"\"" } else { "'''" })
                } else {
                    next == character
                };
                if closes {
                    break;
                }
                if next == '\n' {
                    if !multiline {
                        return Err(format!("line {start_line}: unterminated string"));
                    }
                    line += 1;
                }
                at += if next == '\\' && character == '"' {
                    2
                } else {
                    1
                };
            }
            let content: String = characters
                .get(content_start..at)
                .unwrap_or_default()
                .iter()
                .collect();
            at += quotes;
            let raw: String = characters
                .get(start..at)
                .unwrap_or_default()
                .iter()
                .collect();
            tokens.push((Toml::Str { raw, content }, start_line));
        } else if is_bare(character) {
            let start = at;
            while characters.get(at).copied().is_some_and(is_bare) {
                at += 1;
            }
            let word = characters
                .get(start..at)
                .unwrap_or_default()
                .iter()
                .collect();
            tokens.push((Toml::Bare(word), line));
        } else {
            tokens.push((Toml::Punct(character), line));
            at += 1;
        }
    }
    Ok(tokens)
}

/// The violations in the text of a nextest configuration file: every key
/// named `retries`, bare or quoted, as the last part of a dotted key or
/// inside an inline table, whose value is not a plain 0.
pub(crate) fn profile_violations(text: &str, file: &Path) -> Vec<Violation> {
    let tokens = match toml_tokens(text) {
        Ok(tokens) => tokens,
        Err(error) => {
            return vec![Violation::Unreadable {
                file: file.to_path_buf(),
                error,
            }];
        }
    };
    let kind = |index: usize| tokens.get(index).map(|(token, _)| token);
    let mut violations = Vec::new();
    for (index, (token, line)) in tokens.iter().enumerate() {
        let is_key = match token {
            Toml::Bare(word) => word == "retries",
            Toml::Str { content, .. } => content == "retries",
            _ => false,
        };
        let at_key_start = index
            .checked_sub(1)
            .and_then(kind)
            .is_none_or(|before| matches!(before, Toml::Newline | Toml::Punct('.' | '{' | ',')));
        if !is_key || !at_key_start || kind(index + 1) != Some(&Toml::Punct('=')) {
            continue;
        }
        let value = kind(index + 2);
        let plain_zero = matches!(value, Some(Toml::Bare(word)) if word == "0")
            && kind(index + 3)
                .is_none_or(|after| matches!(after, Toml::Newline | Toml::Punct(',' | '}')));
        if !plain_zero {
            let value = match value {
                Some(Toml::Bare(word)) => word.clone(),
                Some(Toml::Str { raw, .. }) => raw.clone(),
                Some(Toml::Punct(character)) => character.to_string(),
                Some(Toml::Newline) | None => String::new(),
            };
            violations.push(Violation::Profile {
                file: file.to_path_buf(),
                line: *line,
                value,
            });
        }
    }
    violations
}

/// Whether `bytes` holds `word` at `at` with no character of an option or
/// variable name on either side.
fn word_at(bytes: &[u8], at: usize, word: &str) -> bool {
    let is_name = |byte: Option<&u8>| {
        byte.is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    };
    bytes.get(at..at + word.len()) == Some(word.as_bytes())
        && !is_name(at.checked_sub(1).and_then(|before| bytes.get(before)))
        && !is_name(bytes.get(at + word.len()))
}

/// The value written after a name: separators, quotes and line breaks are
/// skipped, then the value runs to the next separator.
fn value_after(bytes: &[u8], from: usize) -> String {
    let mut at = from;
    while bytes
        .get(at)
        .is_some_and(|byte| b" \t\r\n=:\"',".contains(byte))
    {
        at += 1;
    }
    let start = at;
    while bytes
        .get(at)
        .is_some_and(|byte| !b" \t\r\n\"',]})&|;".contains(byte))
    {
        at += 1;
    }
    String::from_utf8_lossy(bytes.get(start..at).unwrap_or_default()).into_owned()
}

/// The violations in a file that holds commands or environment settings:
/// the retries option or variable with a value other than 0, and an option
/// that makes nextest read another configuration file.
pub(crate) fn command_violations(text: &str, file: &Path) -> Vec<Violation> {
    let bytes = text.as_bytes();
    let mut violations = Vec::new();
    let mut line = 1;
    for at in 0..bytes.len() {
        if bytes.get(at) == Some(&b'\n') {
            line += 1;
            continue;
        }
        for (name, is_option) in [(OPTION, true), (VARIABLE, false)] {
            if word_at(bytes, at, name) {
                let value = value_after(bytes, at + name.len());
                if value != "0" {
                    let file = file.to_path_buf();
                    violations.push(if is_option {
                        Violation::Option { file, line, value }
                    } else {
                        Violation::Environment { file, line, value }
                    });
                }
            }
        }
        if let Some(option) = CONFIG_OPTIONS
            .iter()
            .find(|option| word_at(bytes, at, option))
        {
            violations.push(Violation::ConfigFile {
                file: file.to_path_buf(),
                line,
                option,
            });
        }
    }
    violations
}

/// Every file under `dir`, in path order; nothing if it does not exist.
fn files_under(dir: &Path, found: &mut Vec<PathBuf>) -> Result<(), Error> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)
        .and_then(|entries| {
            entries
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(Error::path_io("list directory", dir))?;
    entries.sort();
    for path in entries {
        let metadata =
            fs::symlink_metadata(&path).map_err(Error::path_io("read file type", &path))?;
        if metadata.is_dir() {
            files_under(&path, found)?;
        } else {
            found.push(path);
        }
    }
    Ok(())
}

/// Reads every place under `root` that can set retries, and `cargo xtask
/// check`'s own test run. Returns the violations in path order and the
/// number of files read.
///
/// # Errors
///
/// Returns an error if a directory cannot be listed or a file read.
pub(crate) fn check(root: &Path, target_dir: &Path) -> Result<(Vec<Violation>, usize), Error> {
    let profiles = root.join(PROFILES);
    let mut files = Vec::new();
    if profiles.is_file() {
        files.push(profiles);
    }
    files_under(&root.join(GITHUB), &mut files)?;
    files.extend(crate::pure::config_files(root, target_dir)?);
    files.sort();
    files.dedup();

    let mut violations = Vec::new();
    if !files.iter().any(|file| file.ends_with(PROFILES)) {
        violations.push(Violation::ProfilesMissing(PathBuf::from(PROFILES)));
    }
    for file in &files {
        let bytes = fs::read(file).map_err(Error::path_io("read file", file))?;
        let text = String::from_utf8_lossy(&bytes);
        let relative = file.strip_prefix(root).unwrap_or(file);
        if relative == Path::new(PROFILES) {
            violations.extend(profile_violations(&text, relative));
        } else {
            violations.extend(command_violations(&text, relative));
        }
    }
    violations.extend(command_violations(
        &crate::check::NEXTEST_RUN.join(" "),
        Path::new(XTASK_CHECK),
    ));
    Ok((violations, files.len()))
}

/// Checks this repository, or with `self_test` proves on planted fixtures
/// that each rule is caught.
///
/// # Errors
///
/// Returns the violations found, or why the check could not run.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    if self_test {
        return self_test::run(&workspace.target_dir, output);
    }
    let (violations, read) = check(&workspace.root, &workspace.target_dir)?;
    if !violations.is_empty() {
        return Err(Error::Rules {
            check: CHECK,
            violations: violations.iter().map(ToString::to_string).collect(),
        });
    }
    writeln!(
        output,
        "{CHECK}: {read} files read and the xtask test run; no profile, override, command or \
         environment retries a test"
    )
    .map_err(Error::io("report check-retries"))
}
