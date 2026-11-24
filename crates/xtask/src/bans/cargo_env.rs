// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cargo's own configuration as a way around `CLIPPY_CONF_DIR`'s removal.
//!
//! Cargo gives the variables of an `[env]` table to every compiler it
//! runs, after the environment the check hands it, so one there would put
//! back what the check removes. Cargo reads `.cargo/config` and
//! `.cargo/config.toml` in the directory it runs in and every one above,
//! and in its home (`$CARGO_HOME`, by default `.cargo` in the user's home
//! directory). The check reads each of those for the directories it runs
//! cargo in, and refuses one that sets a variable clippy reads, includes
//! another file, or cannot be read.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{Found, Violation};

#[cfg(test)]
mod tests;

/// One piece of a TOML line, outside comments: a key or a string (bare,
/// or quoted and decoded), or a punctuation character.
#[derive(Debug, PartialEq, Eq)]
enum Token {
    Word(String),
    Punct(char),
}

/// The tokens of one TOML `line`, up to any comment; `None` if a string
/// in it does not close on the line or cannot be decoded, or it opens a
/// multi-line string, whose lines `keys` could not tell from keys.
fn tokens(line: &str) -> Option<Vec<Token>> {
    let mut found = Vec::new();
    let mut rest = line;
    while let Some(c) = rest.chars().next() {
        let after_c = rest.get(c.len_utf8()..).unwrap_or_default();
        rest = match c {
            '#' => break,
            '"' | '\'' if rest.starts_with(r#"""""#) || rest.starts_with("'''") => return None,
            '"' => {
                let (text, after) = super::basic_string(after_c)?;
                found.push(Token::Word(text));
                after
            }
            '\'' => {
                let (text, after) = after_c.split_once('\'')?;
                found.push(Token::Word(text.to_owned()));
                after
            }
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' => {
                let end = rest
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                    .unwrap_or(rest.len());
                let (word, after) = rest.split_at(end);
                found.push(Token::Word(word.to_owned()));
                after
            }
            c if c.is_whitespace() => after_c,
            c => {
                found.push(Token::Punct(c));
                after_c
            }
        };
    }
    Some(found)
}

/// Each key part `tokens` names: those of a table header (`[a.b]`,
/// `[[a]]`), and the dotted key before each `=`, which in TOML is only
/// ever a key's, inline tables' included.
fn keys(tokens: &[Token]) -> Vec<&str> {
    let mut found = Vec::new();
    if tokens.first() == Some(&Token::Punct('[')) {
        found.extend(
            tokens
                .iter()
                .skip_while(|token| **token == Token::Punct('['))
                .take_while(|token| **token != Token::Punct(']'))
                .filter_map(|token| match token {
                    Token::Word(word) => Some(word.as_str()),
                    Token::Punct(_) => None,
                }),
        );
    }
    for (at, token) in tokens.iter().enumerate() {
        if *token != Token::Punct('=') {
            continue;
        }
        let mut before = tokens.get(..at).unwrap_or_default().iter().rev();
        while let Some(Token::Word(word)) = before.next() {
            found.push(word.as_str());
            if before.next() != Some(&Token::Punct('.')) {
                break;
            }
        }
    }
    found
}

/// The line a violation of `violations` is on.
fn line_of(violation: &Violation) -> usize {
    match violation {
        Violation::Unreadable { line, .. }
        | Violation::CargoEnv { line, .. }
        | Violation::CargoInclude { line } => *line,
        _ => 0,
    }
}

/// Each line of the cargo configuration `text` that could set a variable
/// clippy reads through an `[env]` table, includes another file, which
/// this does not follow, or cannot be read.
///
/// Keys are read as TOML decodes them, wherever they stand (headers,
/// dotted keys, inline tables over one line or several), with no notion of
/// which table holds them. So a file that names an `env` key anywhere is
/// refused for every key clippy reads (`clippy_reads`) anywhere in it: an
/// alias named for clippy beside an `[env]` table is refused too, which
/// costs a rename.
pub(crate) fn violations(text: &str) -> Vec<Violation> {
    // TOML skips a leading byte order mark; so does this.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = Vec::new();
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        match tokens(line) {
            Some(tokens) => lines.push((
                index + 1,
                keys(&tokens)
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
            )),
            None => found.push(Violation::Unreadable {
                line: index + 1,
                text: line.to_owned(),
            }),
        }
    }
    let env = lines
        .iter()
        .flat_map(|(_, keys)| keys)
        .any(|key| key.eq_ignore_ascii_case("env"));
    for (line, keys) in &lines {
        for key in keys {
            let refused = if key.eq_ignore_ascii_case("include") {
                Violation::CargoInclude { line: *line }
            } else if env && super::clippy_reads(key) {
                Violation::CargoEnv {
                    line: *line,
                    name: key.clone(),
                }
            } else {
                continue;
            };
            if !found.contains(&refused) {
                found.push(refused);
            }
        }
    }
    found.sort_by_key(line_of);
    found
}

/// The names cargo looks for in each `.cargo` directory. Cargo prefers
/// `config` when both are there; both are read.
const NAMES: [&str; 2] = ["config", "config.toml"];

/// The cargo configuration files cargo run in any of `dirs` would probe:
/// both names in the `.cargo` directory of each directory and every one
/// above it, and in `cargo_home`, sorted and each named once.
pub(crate) fn candidates(dirs: &[PathBuf], cargo_home: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|dir| dir.ancestors())
        .map(|dir| dir.join(".cargo"))
        .chain([cargo_home.to_path_buf()])
        .flat_map(|dir| NAMES.map(|name| dir.join(name)))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// Where cargo keeps its home: `$CARGO_HOME`, taken from the current
/// directory if relative as cargo takes it, or `.cargo` in the user's home
/// directory; `None` if neither can be found.
pub(crate) fn cargo_home() -> Option<PathBuf> {
    match std::env::var_os("CARGO_HOME").filter(|home| !home.is_empty()) {
        Some(home) => std::env::current_dir().ok().map(|dir| dir.join(home)),
        None => std::env::home_dir().map(|home| home.join(".cargo")),
    }
}

/// Reads each cargo configuration cargo run in any of `dirs` would read
/// (`candidates`), with `cargo_home` its home, and returns each refusal of
/// `violations`, and each file there that cannot be read. With no home,
/// that is refused: its configuration could not be read.
pub(crate) fn check(dirs: &[PathBuf], cargo_home: Option<&Path>) -> Vec<Found> {
    let Some(cargo_home) = cargo_home else {
        return vec![Found {
            file: "CARGO_HOME".to_owned(),
            violation: Violation::CargoUnread(
                "neither CARGO_HOME nor the user's home directory is known".to_owned(),
            ),
        }];
    };
    let mut found = Vec::new();
    for path in candidates(dirs, cargo_home) {
        let file = path.display().to_string();
        let text = match fs::read(&path) {
            // A dangling link is not a file cargo can skip.
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && fs::symlink_metadata(&path).is_err() =>
            {
                continue;
            }
            Err(error) => Err(error.to_string()),
            Ok(bytes) => String::from_utf8(bytes).map_err(|_| "it is not UTF-8".to_owned()),
        };
        match text {
            Ok(text) => found.extend(violations(&text).into_iter().map(|violation| Found {
                file: file.clone(),
                violation,
            })),
            Err(error) => found.push(Found {
                file,
                violation: Violation::CargoUnread(error),
            }),
        }
    }
    found
}
