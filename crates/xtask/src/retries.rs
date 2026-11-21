// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! No test is ever retried (coding standards rule 15; the testing
//! specification, 5.4, point 6).
//!
//! nextest takes its retry policy from a profile, a per-test override, its
//! command line and an environment variable. This check reads each place
//! in the repository that can set one: the profiles and overrides in
//! `.config/nextest.toml`; the commands and environments of everything
//! under `.github/` and of every cargo configuration file (aliases and
//! `[env]`); and the Rust source of `xtask`, which runs nextest itself.
//!
//! In the profiles, `retries` may only ever be written `retries = 0`: any
//! other value, and `retries` as a table in any spelling (inline, a table
//! header or a dotted key), fails. In commands, the option and the variable
//! must be followed by a plain 0, and an option that points nextest at a
//! configuration file this check does not read fails.
//!
//! Commands are matched on raw text, comments included, so the check fails
//! closed rather than parse YAML, shell or Rust. An escape that spells a
//! letter, digit, `-`, `_` or `=` fails too, since it could spell a name
//! the raw text does not show.

mod self_test;

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{Error, metadata, rust_source};

#[cfg(test)]
mod tests;

const CHECK: &str = "check-retries";

/// The nextest configuration file, relative to the workspace root.
const PROFILES: &str = ".config/nextest.toml";

/// The directory of GitHub's workflows and actions, relative to the root.
const GITHUB: &str = ".github";

/// The crate whose Rust source runs nextest, and this check's own files
/// in it, which name the option and the variable to look for them.
const XTASK: &str = "crates/xtask";
const OWN_FILES: &[&str] = &["crates/xtask/src/retries.rs", "crates/xtask/src/retries"];

/// nextest's command-line option and environment variable for retries.
const OPTION: &str = "--retries";
const VARIABLE: &str = "NEXTEST_RETRIES";

/// nextest's options that read another configuration file.
const CONFIG_OPTIONS: &[&str] = &["--config-file", "--tool-config-file"];

/// How a plain 0 may end in a command's text: alone, or closing the quotes
/// or the code span it stands in. Anything else attached to it (a second
/// quoted part, a variable) would join it in a shell.
const PLAIN_ZEROS: &[&str] = &["0", "0\"", "0'", "0`"];

/// One place that could make nextest retry a test.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    /// A file or directory the check must read does not exist.
    Missing(PathBuf),
    /// A profile or per-test override sets `retries` to something other
    /// than 0.
    Profile {
        file: PathBuf,
        line: usize,
        value: String,
    },
    /// A profile or per-test override writes `retries` as a table: a table
    /// header or a dotted key.
    ProfileTable { file: PathBuf, line: usize },
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
    /// An escape in a command file spells a character of a name.
    Escape {
        file: PathBuf,
        line: usize,
        spelled: char,
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
            Self::Missing(path) => write!(
                formatter,
                "{} does not exist, so it was not read: {RULE}",
                path.display()
            ),
            Self::Profile { file, line, value } => write!(
                formatter,
                "{}:{line}: sets retries to {}; every profile and per-test override sets \
                 retries = 0: {RULE}",
                file.display(),
                shown(value)
            ),
            Self::ProfileTable { file, line } => write!(
                formatter,
                "{}:{line}: writes retries as a table; every profile and per-test override \
                 sets retries = 0: {RULE}",
                file.display()
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
            Self::Escape {
                file,
                line,
                spelled,
            } => write!(
                formatter,
                "{}:{line}: an escape spells `{spelled}`, which could hide a retries setting \
                 from this check; write the character itself",
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
/// spelling, quotes included, and its content with escapes decoded.
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

/// The character `count` hex digits at `from` encode, if they all are hex
/// digits and the value is a character.
fn hex_char(characters: &[char], from: usize, count: usize) -> Option<char> {
    let digits = characters.get(from..from + count)?;
    let mut value = 0_u32;
    for digit in digits {
        value = value.checked_mul(16)?.checked_add(digit.to_digit(16)?)?;
    }
    char::from_u32(value)
}

/// Splits TOML text into tokens, each with its line. Strings of all four
/// kinds are read whole, so a `#` or a line inside one is not a comment or
/// a key, and a basic string's escapes are decoded as TOML decodes them.
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
            let basic = character == '"';
            let delimiter = match (basic, starts(at, "\"\"\""), starts(at, "'''")) {
                (true, true, _) => "\"\"\"",
                (true, false, _) => "\"",
                (false, _, true) => "'''",
                (false, _, false) => "'",
            };
            let multiline = delimiter.len() == 3;
            let start_line = line;
            let start = at;
            at += delimiter.len();
            let mut content = String::new();
            loop {
                let Some(&next) = characters.get(at) else {
                    return Err(format!("line {start_line}: unterminated string"));
                };
                if starts(at, delimiter) {
                    break;
                }
                if next == '\n' {
                    if !multiline {
                        return Err(format!("line {start_line}: unterminated string"));
                    }
                    line += 1;
                    content.push(next);
                    at += 1;
                } else if next == '\\' && basic {
                    at = decode_escape(&characters, at, multiline, &mut line, &mut content)?;
                } else {
                    content.push(next);
                    at += 1;
                }
            }
            at += delimiter.len();
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

/// Decodes the escape whose backslash is at `at` in a basic string into
/// `content`, counting the lines a line-ending backslash joins, and returns
/// where reading goes on. An escape TOML does not define is an error.
fn decode_escape(
    characters: &[char],
    at: usize,
    multiline: bool,
    line: &mut usize,
    content: &mut String,
) -> Result<usize, String> {
    let invalid = |line: usize| format!("line {line}: invalid escape");
    let simple = match characters.get(at + 1) {
        Some('b') => Some('\u{8}'),
        Some('t') => Some('\t'),
        Some('n') => Some('\n'),
        Some('f') => Some('\u{c}'),
        Some('r') => Some('\r'),
        Some('e') => Some('\u{1b}'),
        Some('"') => Some('"'),
        Some('\\') => Some('\\'),
        _ => None,
    };
    if let Some(decoded) = simple {
        content.push(decoded);
        return Ok(at + 2);
    }
    let digits = match characters.get(at + 1) {
        Some('x') => 2,
        Some('u') => 4,
        Some('U') => 8,
        _ => 0,
    };
    if digits > 0 {
        let decoded = hex_char(characters, at + 2, digits).ok_or_else(|| invalid(*line))?;
        content.push(decoded);
        return Ok(at + 2 + digits);
    }
    // A line-ending backslash: only white space may follow it on its line,
    // and it joins every white space and line break after it.
    let mut next = at + 1;
    while characters
        .get(next)
        .is_some_and(|&space| space == ' ' || space == '\t' || space == '\r')
    {
        next += 1;
    }
    if !multiline || characters.get(next) != Some(&'\n') {
        return Err(invalid(*line));
    }
    while let Some(&space) = characters.get(next) {
        if space == '\n' {
            *line += 1;
        } else if !space.is_whitespace() {
            break;
        }
        next += 1;
    }
    Ok(next)
}

/// What encloses a token of a TOML file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Nesting {
    Header,
    Array,
    InlineTable,
}

/// The violations in the text of a nextest configuration file. A key or
/// table-header part named `retries`, bare or quoted, must be the last part
/// of its key, set to a plain 0.
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
    let mut nesting: Vec<Nesting> = Vec::new();
    for (index, (token, line)) in tokens.iter().enumerate() {
        let before = index.checked_sub(1).and_then(kind);
        let line_start = before.is_none_or(|before| *before == Toml::Newline);
        let is_key = match token {
            Toml::Bare(word) => word == "retries",
            Toml::Str { content, .. } => content == "retries",
            Toml::Punct('[') => {
                let header = nesting.is_empty() && line_start || before == Some(&Toml::Punct('['));
                nesting.push(if header {
                    Nesting::Header
                } else {
                    Nesting::Array
                });
                false
            }
            Toml::Punct('{') => {
                nesting.push(Nesting::InlineTable);
                false
            }
            Toml::Punct(']' | '}') => {
                nesting.pop();
                false
            }
            _ => false,
        };
        let key_position = match nesting.last() {
            None => line_start || before == Some(&Toml::Punct('.')),
            Some(Nesting::Header) => {
                matches!(before, Some(Toml::Punct('[' | '.')))
            }
            Some(Nesting::InlineTable) => {
                matches!(before, Some(Toml::Newline | Toml::Punct('{' | ',' | '.')))
            }
            Some(Nesting::Array) => false,
        };
        if !is_key || !key_position {
            continue;
        }
        if kind(index + 1) != Some(&Toml::Punct('=')) {
            violations.push(Violation::ProfileTable {
                file: file.to_path_buf(),
                line: *line,
            });
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

/// The value written after a name: separators, opening quotes and line
/// breaks are skipped, then the value runs to the next separator, any
/// quote inside it kept, so `"0"1` reads as `0"1`, which a shell would
/// join into 01.
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
        .is_some_and(|byte| !b" \t\r\n,]})&|;".contains(byte))
    {
        at += 1;
    }
    String::from_utf8_lossy(bytes.get(start..at).unwrap_or_default()).into_owned()
}

/// The character the escape whose backslash is at `at` spells, and the
/// escape's length: `\xHH`, `\uHHHH`, `\u{H...}`, `\UHHHHHHHH`, or one to
/// three octal digits. `\\` spells a backslash.
fn escape_at(bytes: &[u8], at: usize) -> Option<(char, usize)> {
    let hex = |from: usize, most: usize| {
        let digits = bytes
            .get(from..)
            .unwrap_or_default()
            .iter()
            .take(most)
            .take_while(|byte| byte.is_ascii_hexdigit())
            .count();
        let text = std::str::from_utf8(bytes.get(from..from + digits)?).ok()?;
        let value = u32::from_str_radix(text, 16).ok()?;
        Some((char::from_u32(value)?, digits))
    };
    match bytes.get(at + 1) {
        Some(b'\\') => Some(('\\', 2)),
        Some(b'x') => hex(at + 2, 2).map(|(spelled, digits)| (spelled, 2 + digits)),
        Some(b'u') if bytes.get(at + 2) == Some(&b'{') => {
            hex(at + 3, 6).map(|(spelled, digits)| (spelled, 3 + digits))
        }
        Some(b'u') => hex(at + 2, 4).map(|(spelled, digits)| (spelled, 2 + digits)),
        Some(b'U') => hex(at + 2, 8).map(|(spelled, digits)| (spelled, 2 + digits)),
        Some(b'0'..=b'7') => {
            let digits = bytes
                .get(at + 1..)
                .unwrap_or_default()
                .iter()
                .take(3)
                .take_while(|byte| (b'0'..=b'7').contains(byte))
                .count();
            let text = std::str::from_utf8(bytes.get(at + 1..at + 1 + digits)?).ok()?;
            let value = u32::from_str_radix(text, 8).ok()?;
            Some((char::from_u32(value)?, 1 + digits))
        }
        _ => None,
    }
}

/// The violations in a file that holds commands or environment settings:
/// the retries option or variable with a value other than 0, an option
/// that makes nextest read another configuration file, and an escape that
/// spells a character of a name.
pub(crate) fn command_violations(text: &str, file: &Path) -> Vec<Violation> {
    let bytes = text.as_bytes();
    let mut violations = Vec::new();
    let mut line = 1;
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if byte == b'\n' {
            line += 1;
        }
        if byte == b'\\'
            && let Some((spelled, length)) = escape_at(bytes, at)
        {
            if spelled.is_ascii_alphanumeric() || matches!(spelled, '-' | '_' | '=') {
                violations.push(Violation::Escape {
                    file: file.to_path_buf(),
                    line,
                    spelled,
                });
            }
            at += length;
            continue;
        }
        for (name, is_option) in [(OPTION, true), (VARIABLE, false)] {
            if word_at(bytes, at, name) {
                let value = value_after(bytes, at + name.len());
                if !PLAIN_ZEROS.contains(&value.as_str()) {
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
        at += 1;
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

/// Reads every place under `root` that can set retries. Returns the
/// violations in path order and the number of files read.
///
/// # Errors
///
/// Returns an error if a directory cannot be listed or a file read.
pub(crate) fn check(root: &Path, target_dir: &Path) -> Result<(Vec<Violation>, usize), Error> {
    let profiles = root.join(PROFILES);
    let xtask = root.join(XTASK);
    let mut violations = Vec::new();
    let mut files = Vec::new();
    if profiles.is_file() {
        files.push(profiles.clone());
    } else {
        violations.push(Violation::Missing(PathBuf::from(PROFILES)));
    }
    files_under(&root.join(GITHUB), &mut files)?;
    files.extend(crate::pure::config_files(root, target_dir)?);
    if xtask.is_dir() {
        let own: Vec<PathBuf> = OWN_FILES.iter().map(|own| root.join(own)).collect();
        rust_source::files(
            &xtask,
            &|path| path == target_dir || own.iter().any(|own| own == path),
            &mut files,
        )?;
    } else {
        violations.push(Violation::Missing(PathBuf::from(XTASK)));
    }
    files.sort();
    files.dedup();

    for file in &files {
        let bytes = fs::read(file).map_err(Error::path_io("read file", file))?;
        let text = String::from_utf8_lossy(&bytes);
        let relative = file.strip_prefix(root).unwrap_or(file);
        if *file == profiles {
            violations.extend(profile_violations(&text, relative));
        } else {
            violations.extend(command_violations(&text, relative));
        }
    }
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
        "{CHECK}: {read} files read; no profile, override, command or environment retries a test"
    )
    .map_err(Error::io("report check-retries"))
}
