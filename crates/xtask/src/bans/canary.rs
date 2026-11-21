// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canary: a workspace member that plants every ban, linted in the
//! same run as the workspace.

use std::fs;
use std::path::Path;

use super::{BANS, Lint, Violation};
use crate::metadata::Package;
use crate::{Error, json};

#[cfg(test)]
mod tests;

/// The canary's package name, and its directory under the root.
pub(crate) const NAME: &str = "ironafs-bans-canary";
pub(crate) const DIR: &str = "crates/ironafs-bans-canary";

/// The canary's manifest, exactly.
pub(crate) fn manifest() -> String {
    "# SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>\n\
     # SPDX-License-Identifier: GPL-3.0-or-later\n\
     \n\
     # Written by `cargo xtask check-bans`, which refuses any other text: no\n\
     # crate may depend on this one.\n\
     [package]\n\
     name = \"ironafs-bans-canary\"\n\
     description = \"Planted calls to every item coding standards rule 7 bans, linted with the workspace to prove the bans fire\"\n\
     version.workspace = true\n\
     edition.workspace = true\n\
     rust-version.workspace = true\n\
     license.workspace = true\n\
     repository.workspace = true\n\
     publish.workspace = true\n\
     \n\
     [features]\n\
     planted = []\n\
     \n\
     [lints]\n\
     workspace = true\n"
        .to_owned()
}

/// The lines of the canary's library before its planted lines.
const HEADER: &str = "// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>\n\
     // SPDX-License-Identifier: GPL-3.0-or-later\n\
     \n\
     //! Planted calls to every item coding standards rule 7 bans, one per line,\n\
     //! written by `cargo xtask check-bans`, which lints them with the workspace\n\
     //! and refuses any other text. Built only with the `planted` feature.\n\
     \n\
     #![cfg(feature = \"planted\")]\n\
     \n";

/// The canary's library, exactly: `HEADER`, then each planted line of
/// `BANS`, in order. Each is kept on its line from rustfmt and marked
/// `must_use`, and the one that plants
/// `Default::default()` expects pedantic's `default_trait_access`, the
/// only other lint they draw; nothing names the bans' lints.
pub(crate) fn source() -> String {
    let mut text = HEADER.to_owned();
    for ban in BANS {
        text.push_str("#[rustfmt::skip] ");
        if ban.planted.contains("Default::default()") {
            text.push_str("#[expect(clippy::default_trait_access)] ");
        }
        text.push_str("#[must_use] ");
        text.push_str(ban.planted);
        text.push('\n');
    }
    text
}

/// The line, from 1, of the canary's library that plants `BANS[index]`.
pub(crate) fn line(index: usize) -> usize {
    HEADER.lines().count() + index + 1
}

/// The library's path, relative to the root, `/`-separated.
fn library() -> String {
    format!("{DIR}/src/lib.rs")
}

/// What is wrong with the canary's `files`, each a path relative to its
/// directory, `/`-separated, and its bytes: a file check-bans does not
/// write (a `clippy.toml`, a build script), one missing, or one whose
/// text differs (an `allow` or `expect` added).
pub(crate) fn tampered(files: &[(String, Vec<u8>)]) -> Vec<Violation> {
    let expected = [("Cargo.toml", manifest()), ("src/lib.rs", source())];
    let mut found = Vec::new();
    for (file, bytes) in files {
        match expected.iter().find(|(name, _)| name == file) {
            None => found.push(Violation::Canary(format!(
                "{DIR} holds {file}, which check-bans does not write: remove it"
            ))),
            Some((_, text)) if text.as_bytes() != bytes.as_slice() => {
                found.push(Violation::Canary(format!(
                    "{DIR}/{file} is not the text check-bans writes: restore it"
                )));
            }
            Some(_) => {}
        }
    }
    for (name, _) in &expected {
        if !files.iter().any(|(file, _)| file == name) {
            found.push(Violation::Canary(format!(
                "{DIR}/{name} is missing: restore it"
            )));
        }
    }
    found
}

/// Reads every file under the canary's directory below `root`, as
/// `tampered` takes them.
///
/// # Errors
///
/// Returns an error if a directory or file cannot be read.
pub(crate) fn files(root: &Path) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let dir = root.join(DIR);
    let mut found = Vec::new();
    let mut pending = vec![dir.clone()];
    while let Some(next) = pending.pop() {
        let entries = fs::read_dir(&next).map_err(Error::path_io("list directory", &next))?;
        for entry in entries {
            let path = entry
                .map_err(Error::path_io("list directory", &next))?
                .path();
            let kind =
                fs::symlink_metadata(&path).map_err(Error::path_io("read file type", &path))?;
            if kind.is_dir() {
                pending.push(path);
            } else {
                let bytes = fs::read(&path).map_err(Error::path_io("read", &path))?;
                let relative = path.strip_prefix(&dir).unwrap_or(&path);
                let name: Vec<String> = relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                found.push((name.join("/"), bytes));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Each package of `packages` that depends on the canary, `canary_id`:
/// the canary stays out of every other crate's graph.
pub(crate) fn depended_on(packages: &[Package], canary_id: &str) -> Vec<Violation> {
    packages
        .iter()
        .filter(|package| package.dependencies.iter().any(|id| id == canary_id))
        .map(|package| Violation::Canary(format!("{} depends on the canary", package.id)))
        .collect()
}

/// The index in `BANS` of the canary line `diagnostic` fired on, if it is
/// that ban's disallowed-item warning, on its own line of the library.
fn canary_ban(diagnostic: &json::Value) -> Option<usize> {
    let code = diagnostic
        .get("code")
        .and_then(|code| code.get("code"))
        .and_then(json::Value::as_str)?;
    let text = diagnostic.get("message").and_then(json::Value::as_str)?;
    let span = diagnostic
        .get("spans")
        .and_then(json::Value::as_array)?
        .iter()
        .find(|span| span.get("is_primary") == Some(&json::Value::Bool(true)))?;
    let file = span.get("file_name").and_then(json::Value::as_str)?;
    let at = match span.get("line_start")? {
        json::Value::Number(number) => number.parse::<usize>().ok()?,
        _ => return None,
    };
    if file.replace('\\', "/") != library() {
        return None;
    }
    BANS.iter().enumerate().position(|(index, ban)| {
        let lint = match ban.lint {
            Lint::Method => "clippy::disallowed_methods",
            Lint::Type => "clippy::disallowed_types",
        };
        line(index) == at && code == lint && text.contains(&format!("`{}`", ban.path))
    })
}

/// What cargo's JSON `messages` from the workspace's lint show: each
/// canary line whose ban drew no diagnostic there, every other warning or
/// error, and each member in `member_ids` cargo reported no artifact of.
/// The canary's diagnostics prove the run linted with the bans, whatever
/// could have changed that (an alias for `clippy`, a compiler wrapper, a
/// variable or configuration); the rest is the lint's own gate: no other
/// warning.
pub(crate) fn verdict(messages: &str, canary_id: &str, member_ids: &[String]) -> Vec<Violation> {
    let mut found = Vec::new();
    let mut fired = vec![false; BANS.len()];
    let mut built = Vec::new();
    for message in messages.lines().filter_map(|line| json::parse(line).ok()) {
        let package = message.get("package_id").and_then(json::Value::as_str);
        match message.get("reason").and_then(json::Value::as_str) {
            Some("compiler-artifact") => built.extend(package.map(str::to_owned)),
            Some("compiler-message") => {
                let Some(diagnostic) = message.get("message") else {
                    found.push(Violation::Lint(
                        "cargo reported a compiler message with no diagnostic".to_owned(),
                    ));
                    continue;
                };
                if package == Some(canary_id)
                    && let Some(index) = canary_ban(diagnostic)
                    && let Some(slot) = fired.get_mut(index)
                {
                    *slot = true;
                    continue;
                }
                let level = diagnostic.get("level").and_then(json::Value::as_str);
                if !matches!(level, Some("note" | "help" | "failure-note")) {
                    let rendered = diagnostic
                        .get("rendered")
                        .or_else(|| diagnostic.get("message"))
                        .and_then(json::Value::as_str)
                        .unwrap_or("a diagnostic with no text");
                    found.push(Violation::Lint(rendered.trim_end().to_owned()));
                }
            }
            _ => {}
        }
    }
    for id in member_ids.iter().filter(|id| !built.contains(id)) {
        found.push(Violation::Unbuilt(format!(
            "cargo reported no artifact of the member {id}"
        )));
    }
    for (index, ban) in BANS.iter().enumerate() {
        if !fired.get(index).copied().unwrap_or_default() {
            found.push(Violation::CanaryMissing {
                path: ban.path,
                line: line(index),
            });
        }
    }
    found
}
