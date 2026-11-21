// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A small reader for nextest's `JUnit` report: which test of which binary
//! ran, and whether it passed.

use std::fmt;

#[cfg(test)]
mod tests;

/// One test case the report holds.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Case {
    /// The test binary's id, from the enclosing `testsuite`'s name.
    pub(crate) binary: String,
    pub(crate) name: String,
    pub(crate) passed: bool,
}

/// Where and why the input is not a report this reader accepts.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParseError {
    offset: usize,
    reason: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at byte {}", self.reason, self.offset)
    }
}

/// Elements a passing test case may hold: its captured output.
const OUTPUT: &[&str] = &["system-out", "system-err"];

/// Deepest nesting of elements accepted.
const MAX_DEPTH: usize = 64;

/// Reads a `JUnit` report as nextest writes it: `testsuites`, holding one
/// `testsuite` per test binary, each holding its `testcase`s. A case passed
/// only if it holds nothing but captured output; a failure, an error, a
/// skip or anything else this reader does not know counts as not passed.
/// Elements of a suite other than its cases are skipped.
///
/// # Errors
///
/// Returns the offset and reason of the first thing that is not such a
/// report.
pub(crate) fn parse(text: &str) -> Result<Vec<Case>, ParseError> {
    let mut reader = Reader { text, offset: 0 };
    let mut cases = Vec::new();
    // The open elements, outermost first.
    let mut stack: Vec<String> = Vec::new();
    let mut suite: Option<String> = None;
    let mut seen_root = false;
    while let Some(event) = reader.next_event()? {
        match event {
            Event::Start {
                name,
                attributes,
                empty,
            } => {
                let depth = stack.len();
                if depth >= MAX_DEPTH {
                    return Err(reader.error("nesting too deep"));
                }
                match depth {
                    0 if name == "testsuites" && !seen_root => seen_root = true,
                    0 => return Err(reader.error("expected one testsuites element")),
                    1 if name == "testsuite" => {
                        suite = Some(
                            attribute(&attributes, "name")
                                .ok_or_else(|| reader.error("testsuite without a name"))?,
                        );
                    }
                    1 => return Err(reader.error("expected a testsuite")),
                    2 if name == "testcase" => {
                        let binary = suite
                            .clone()
                            .ok_or_else(|| reader.error("testcase outside a testsuite"))?;
                        let case_name = attribute(&attributes, "name")
                            .ok_or_else(|| reader.error("testcase without a name"))?;
                        cases.push(Case {
                            binary,
                            name: case_name,
                            passed: true,
                        });
                    }
                    _ if depth >= 3 && stack.get(2).is_some_and(|open| open == "testcase") => {
                        // Only the case's own children decide; deeper
                        // elements are inside one of them already.
                        if depth == 3
                            && !OUTPUT.contains(&name.as_str())
                            && let Some(last) = cases.last_mut()
                        {
                            last.passed = false;
                        }
                    }
                    _ => {}
                }
                if empty {
                    if depth == 1 {
                        suite = None;
                    }
                } else {
                    stack.push(name);
                }
            }
            Event::End { name } => {
                if stack.pop().as_deref() != Some(name.as_str()) {
                    return Err(reader.error("mismatched end tag"));
                }
                if stack.len() == 1 {
                    suite = None;
                }
            }
        }
    }
    if !seen_root || !stack.is_empty() {
        return Err(reader.error("unexpected end of report"));
    }
    Ok(cases)
}

fn attribute(attributes: &[(String, String)], key: &str) -> Option<String> {
    attributes
        .iter()
        .find_map(|(name, value)| (name == key).then(|| value.clone()))
}

enum Event {
    Start {
        name: String,
        attributes: Vec<(String, String)>,
        /// Written `<name/>`: no end tag follows.
        empty: bool,
    },
    End {
        name: String,
    },
}

struct Reader<'a> {
    text: &'a str,
    offset: usize,
}

impl Reader<'_> {
    fn error(&self, reason: &'static str) -> ParseError {
        ParseError {
            offset: self.offset,
            reason,
        }
    }

    fn rest(&self) -> &str {
        self.text.get(self.offset..).unwrap_or_default()
    }

    /// Moves past `end`, which must follow somewhere in the rest.
    fn skip_past(&mut self, end: &str, reason: &'static str) -> Result<(), ParseError> {
        let at = self.rest().find(end).ok_or_else(|| self.error(reason))?;
        self.offset += at + end.len();
        Ok(())
    }

    /// The next start or end tag, skipping text, comments, character data,
    /// processing instructions and declarations; `None` at the end.
    fn next_event(&mut self) -> Result<Option<Event>, ParseError> {
        loop {
            let Some(at) = self.rest().find('<') else {
                self.offset = self.text.len();
                return Ok(None);
            };
            self.offset += at;
            let rest = self.rest();
            if rest.starts_with("<!--") {
                self.skip_past("-->", "unterminated comment")?;
            } else if rest.starts_with("<![CDATA[") {
                self.skip_past("]]>", "unterminated character data")?;
            } else if rest.starts_with("<?") {
                self.skip_past("?>", "unterminated processing instruction")?;
            } else if rest.starts_with("<!") {
                self.skip_past(">", "unterminated declaration")?;
            } else if let Some(after) = rest.strip_prefix("</") {
                let end = after
                    .find('>')
                    .ok_or_else(|| self.error("unterminated tag"))?;
                let name = after.get(..end).unwrap_or_default().trim();
                if !is_name(name) {
                    return Err(self.error("invalid element name"));
                }
                let name = name.to_owned();
                self.offset += 2 + end + 1;
                return Ok(Some(Event::End { name }));
            } else {
                self.offset += 1;
                return self.start_tag().map(Some);
            }
        }
    }

    fn skip_whitespace(&mut self) {
        let trimmed = self.rest().trim_start_matches([' ', '\t', '\r', '\n']);
        self.offset = self.text.len() - trimmed.len();
    }

    fn take_name(&mut self) -> Result<String, ParseError> {
        let rest = self.rest();
        let length = rest
            .find(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '_' | ':' | '.')))
            .unwrap_or(rest.len());
        let name = rest.get(..length).unwrap_or_default();
        if !is_name(name) {
            return Err(self.error("invalid name"));
        }
        let name = name.to_owned();
        self.offset += length;
        Ok(name)
    }

    fn start_tag(&mut self) -> Result<Event, ParseError> {
        let name = self.take_name()?;
        let mut attributes = Vec::new();
        loop {
            self.skip_whitespace();
            let rest = self.rest();
            if rest.starts_with("/>") {
                self.offset += 2;
                return Ok(Event::Start {
                    name,
                    attributes,
                    empty: true,
                });
            }
            if rest.starts_with('>') {
                self.offset += 1;
                return Ok(Event::Start {
                    name,
                    attributes,
                    empty: false,
                });
            }
            let key = self.take_name()?;
            self.skip_whitespace();
            if !self.rest().starts_with('=') {
                return Err(self.error("expected '='"));
            }
            self.offset += 1;
            self.skip_whitespace();
            let Some(quote @ ('"' | '\'')) = self.rest().chars().next() else {
                return Err(self.error("expected a quoted value"));
            };
            self.offset += 1;
            let end = self
                .rest()
                .find(quote)
                .ok_or_else(|| self.error("unterminated value"))?;
            let raw = self.rest().get(..end).unwrap_or_default();
            if raw.contains('<') {
                return Err(self.error("'<' in a value"));
            }
            let value = decode(raw).ok_or_else(|| self.error("invalid entity"))?;
            self.offset += end + 1;
            attributes.push((key, value));
        }
    }
}

fn is_name(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_' || first == ':')
}

/// Replaces the predefined entities and character references.
fn decode(raw: &str) -> Option<String> {
    let mut text = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        text.push_str(rest.get(..at)?);
        let after = rest.get(at + 1..)?;
        let end = after.find(';')?;
        let entity = after.get(..end)?;
        let decoded = match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let code = if let Some(hex) = entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                {
                    u32::from_str_radix(hex, 16).ok()?
                } else {
                    entity.strip_prefix('#')?.parse().ok()?
                };
                char::from_u32(code)?
            }
        };
        text.push(decoded);
        rest = after.get(end + 1..)?;
    }
    text.push_str(rest);
    Some(text)
}
