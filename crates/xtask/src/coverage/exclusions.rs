// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The coverage exclusion list, `tests/coverage/exclusions.txt`, and the
//! rule that keeps test files out of the report.
//!
//! Each entry is a file pattern relative to the workspace root and a
//! reason: `<pattern> # <reason>`. A pattern uses `*` for any run of
//! characters inside one path segment and `**` as a whole segment for any
//! number of segments; nothing else is special, so the pattern means the
//! same thing here and in the regular expression handed to the tool.

use std::path::Path;

#[cfg(test)]
mod tests;

/// One path segment of a pattern.
#[derive(Debug, PartialEq, Eq)]
enum Segment {
    /// `**`: any number of segments (at least one at the end).
    Any,
    /// A name, where `*` matches any run of characters.
    Glob(String),
}

/// A file pattern relative to the workspace root.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Pattern {
    segments: Vec<Segment>,
}

impl Pattern {
    /// Reads a pattern.
    ///
    /// # Errors
    ///
    /// Returns why the text is not a supported pattern: empty, absolute,
    /// with an empty, `.` or `..` segment, `**` inside a name, or a
    /// character other than ASCII letters, digits, `_`, `-`, `.`, `/` and
    /// `*`.
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        if text.is_empty() {
            return Err("empty pattern".to_owned());
        }
        if let Some(bad) = text
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '*')))
        {
            return Err(format!(
                "pattern `{text}` has unsupported character {bad:?}"
            ));
        }
        let segments = text
            .split('/')
            .map(|segment| match segment {
                "" => Err(format!(
                    "pattern `{text}` is absolute, ends in `/` or has an empty segment"
                )),
                "." | ".." => Err(format!("pattern `{text}` has a `{segment}` segment")),
                "**" => Ok(Segment::Any),
                name if name.contains("**") => {
                    Err(format!("pattern `{text}` has `**` inside a name"))
                }
                name => Ok(Segment::Glob(name.to_owned())),
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { segments })
    }

    /// Whether `relative`, a `/`-separated path from the workspace root,
    /// matches.
    pub(super) fn matches(&self, relative: &str) -> bool {
        let parts: Vec<&str> = relative.split('/').collect();
        match_segments(&self.segments, &parts)
    }

    /// The same pattern as a regular expression over the relative path,
    /// without anchors.
    fn regex(&self) -> String {
        let mut regex = String::new();
        let last = self.segments.len().saturating_sub(1);
        for (index, segment) in self.segments.iter().enumerate() {
            match segment {
                Segment::Any if index == last => regex.push_str(".*"),
                Segment::Any => regex.push_str("(.*/)?"),
                Segment::Glob(name) => {
                    for c in name.chars() {
                        match c {
                            '*' => regex.push_str("[^/]*"),
                            '.' => regex.push_str("[.]"),
                            other => regex.push(other),
                        }
                    }
                    if index != last {
                        regex.push('/');
                    }
                }
            }
        }
        regex
    }
}

/// Matches `input` against `pattern`, where items for which `is_star`
/// holds match any run of input items (none included) and other items
/// match one input item each by `eq`.
///
/// Two pointers with one saved backtrack point: each mismatch moves the
/// last star's start forward by one, so the work is at most the product of
/// the two lengths, never exponential in the number of stars.
fn wildcard<P, S>(
    pattern: &[P],
    input: &[S],
    is_star: impl Fn(&P) -> bool,
    eq: impl Fn(&P, &S) -> bool,
) -> bool {
    let (mut next, mut at) = (0, 0);
    // The pattern index after the last star seen, and where its run ends.
    let mut backtrack: Option<(usize, usize)> = None;
    while at < input.len() {
        match (pattern.get(next), input.get(at)) {
            (Some(item), _) if is_star(item) => {
                backtrack = Some((next + 1, at));
                next += 1;
            }
            (Some(item), Some(value)) if eq(item, value) => {
                next += 1;
                at += 1;
            }
            _ => match backtrack {
                Some((after, end)) => {
                    backtrack = Some((after, end + 1));
                    next = after;
                    at = end + 1;
                }
                None => return false,
            },
        }
    }
    pattern
        .get(next..)
        .is_some_and(|rest| rest.iter().all(is_star))
}

fn match_segments(segments: &[Segment], parts: &[&str]) -> bool {
    let run = |parts: &[&str]| {
        wildcard(
            segments,
            parts,
            |segment| *segment == Segment::Any,
            |segment, part| match segment {
                Segment::Glob(name) => match_name(name.as_bytes(), part.as_bytes()),
                Segment::Any => false,
            },
        )
    };
    // A trailing `**` matches at least one segment: the pattern, its `**`
    // matching none or more, must then match all but the last segment.
    if matches!(segments.last(), Some(Segment::Any)) {
        parts.split_last().is_some_and(|(_, head)| run(head))
    } else {
        run(parts)
    }
}

/// Matches one name against a glob whose only wildcard is `*`.
fn match_name(glob: &[u8], name: &[u8]) -> bool {
    wildcard(
        glob,
        name,
        |byte| *byte == b'*',
        |byte, value| byte == value,
    )
}

/// One entry of the exclusion list.
#[derive(Debug)]
pub(super) struct Exclusion {
    pub(super) pattern: Pattern,
    pub(super) reason: String,
}

/// Reads the exclusion list. Blank lines and lines starting with `#` are
/// comments.
///
/// Returns the entries read and one violation for each line that has no
/// reason or an unsupported pattern.
pub(super) fn parse_list(text: &str) -> (Vec<Exclusion>, Vec<String>) {
    let mut list = Vec::new();
    let mut violations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (pattern, reason) = line.split_once('#').unwrap_or((line, ""));
        let (pattern, reason) = (pattern.trim(), reason.trim());
        if reason.is_empty() {
            violations.push(format!(
                "exclusion line {number}: `{pattern}` has no reason"
            ));
            continue;
        }
        match Pattern::parse(pattern) {
            Ok(pattern) => list.push(Exclusion {
                pattern,
                reason: reason.to_owned(),
            }),
            Err(error) => violations.push(format!("exclusion line {number}: {error}")),
        }
    }
    (list, violations)
}

/// The regular expression for the tool's `--ignore-filename-regex`,
/// matching every excluded file under `root`, or `None` for an empty list.
///
/// The expression uses only groups, alternation, `?`, `*`, `.` and
/// bracket expressions, which read the same in the tool's and LLVM's
/// regular-expression syntax.
///
/// # Errors
///
/// Returns why `root` cannot be written into the expression: not absolute
/// UTF-8, or holding `[`, `]`, `\` or `^`.
pub(super) fn ignore_regex(root: &Path, list: &[Exclusion]) -> Result<Option<String>, String> {
    if list.is_empty() {
        return Ok(None);
    }
    let text = root
        .to_str()
        .filter(|text| text.starts_with('/'))
        .ok_or_else(|| {
            format!(
                "workspace root {} is not an absolute UTF-8 path",
                root.display()
            )
        })?;
    let mut regex = String::from("^");
    for c in text.trim_end_matches('/').chars() {
        match c {
            '[' | ']' | '\\' | '^' => {
                return Err(format!(
                    "workspace root {text} holds {c:?}, which the exclusion expression cannot carry"
                ));
            }
            '.' | '+' | '*' | '?' | '(' | ')' | '{' | '}' | '|' | '$' => {
                regex.push('[');
                regex.push(c);
                regex.push(']');
            }
            other => regex.push(other),
        }
    }
    regex.push_str("/(");
    let alternatives: Vec<String> = list.iter().map(|entry| entry.pattern.regex()).collect();
    regex.push_str(&alternatives.join("|"));
    regex.push_str(")$");
    Ok(Some(regex))
}

/// Whether `relative` is a test file under the tool's default rule: a
/// `tests`, `examples` or `benches` directory anywhere in the path, or a
/// file named `tests.rs` or ending in `_tests.rs` or `-tests.rs`.
pub(super) fn is_test_file(relative: &str) -> bool {
    let parts: Vec<&str> = relative.split('/').collect();
    let Some((name, directories)) = parts.split_last() else {
        return false;
    };
    if directories
        .iter()
        .any(|directory| matches!(*directory, "tests" | "examples" | "benches"))
    {
        return true;
    }
    if *name == "tests.rs" {
        return true;
    }
    ["_tests.rs", "-tests.rs"].iter().any(|suffix| {
        name.strip_suffix(suffix).is_some_and(|stem| {
            !stem.is_empty()
                && stem
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        })
    })
}
