// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validation of the final trailer block in pull requests and commit messages.

use std::env;
use std::fmt;
use std::fs;
use std::io::Write;
use std::process::Stdio;

use crate::{Command, Error, args::TrailerMode};

#[cfg(test)]
mod tests;

// This list and .github/CODEOWNERS must change together.
const MAINTAINERS: &[(&str, &str)] = &[("AdrianIAna", "adrian@adrianana.com")];

/// A trailer rule violation and the line where it can be fixed.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Violation {
    line: usize,
    fix: &'static str,
}

impl Violation {
    fn at(line: usize, fix: &'static str) -> Self {
        Self { line, fix }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.fix)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Rules {
    MaintainerPr,
    ContributorPr,
    Commit,
    Main,
}

/// Reads a message from the input selected by the mode and checks its trailers.
///
/// # Errors
///
/// Returns an error if the input is unavailable or a trailer rule is broken.
pub(super) fn run(mode: TrailerMode, output: &mut impl Write) -> Result<(), Error> {
    let (path, rules) = match mode {
        TrailerMode::Pr => {
            let author = env::var("PR_AUTHOR").map_err(|_| {
                Error::Trailer(Violation::at(
                    1,
                    "set PR_AUTHOR to the pull request author's login",
                ))
            })?;
            return run_pr(
                &author,
                || {
                    env::var("PR_BODY").map_err(|_| {
                        Violation::at(1, "set PR_BODY to the pull request description")
                    })
                },
                output,
            );
        }
        TrailerMode::Commit(path) => (Some(path), Rules::Commit),
        TrailerMode::Main(path) => (path, Rules::Main),
    };
    let message = match path {
        Some(path) => fs::read_to_string(path).map_err(Error::io("read commit message file"))?,
        None => head_message()?,
    };
    validate(&message, rules).map_err(Error::Trailer)
}

fn run_pr(
    author: &str,
    read_body: impl FnOnce() -> Result<String, Violation>,
    output: &mut impl Write,
) -> Result<(), Error> {
    if author == "dependabot[bot]" {
        return writeln!(
            output,
            "notice: the maintainer's squash message for this Dependabot proposal is checked on main"
        )
        .map_err(Error::io("report dependabot trailer check"));
    }
    if author.trim().is_empty() {
        return Err(Error::Trailer(Violation::at(
            1,
            "set PR_AUTHOR to the pull request author's login",
        )));
    }
    let rules = if MAINTAINERS
        .iter()
        .any(|(login, _)| login.eq_ignore_ascii_case(author))
    {
        Rules::MaintainerPr
    } else {
        Rules::ContributorPr
    };
    let body = read_body().map_err(Error::Trailer)?;
    validate(&body, rules).map_err(Error::Trailer)
}

fn head_message() -> Result<String, Error> {
    const COMMAND: &str = "git log -1 --format=%B";
    let output = Command::new("git")
        .args(["log", "-1", "--format=%B"])
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::io(COMMAND))?;
    Error::check_status(COMMAND, output.status)?;
    String::from_utf8(output.stdout).map_err(|_| Error::UnexpectedOutput(COMMAND))
}

pub(super) fn validate(message: &str, rules: Rules) -> Result<(), Violation> {
    let lines = message.trim_end().lines();
    let block_len = lines
        .clone()
        .rev()
        .take_while(|line| !line.trim().is_empty())
        .count();
    if block_len == 0 {
        return Err(Violation::at(
            1,
            "add a trailer block as the final paragraph",
        ));
    }
    let preceding_lines = lines.clone().count() - block_len;
    let mut trailers = Trailers::default();
    let mut fence = None;
    for (index, text) in lines.enumerate() {
        let fenced = in_fenced_code(&mut fence, text);
        if index >= preceding_lines {
            let line = index + 1;
            if fenced {
                return Err(Violation::at(
                    line,
                    "put the trailer block outside fenced code",
                ));
            }
            if text.trim_start().starts_with('>') {
                return Err(Violation::at(
                    line,
                    "put the trailer block outside quoted text",
                ));
            }
            let (key, value) = text.split_once(':').ok_or_else(|| {
                Violation::at(line, "end the message with a trailer block and start each line with a trailer key and a colon")
            })?;
            trailers.add(rules, line, key, value.trim())?;
        }
    }
    trailers.require(rules, preceding_lines + 1)
}

#[derive(Debug, Default)]
struct Trailers<'a> {
    class: Option<&'a str>,
    assistance: Option<&'a str>,
    reviewed: bool,
    signed: bool,
}

impl<'a> Trailers<'a> {
    fn add(
        &mut self,
        rules: Rules,
        line: usize,
        key: &str,
        value: &'a str,
    ) -> Result<(), Violation> {
        let at_line = |fix| Violation::at(line, fix);
        match key {
            "Class" => {
                if rules == Rules::Commit {
                    return Err(at_line("remove Class: from the branch commit"));
                }
                if self.class.is_some() {
                    return Err(at_line("remove the duplicate Class: trailer"));
                }
                if !matches!(value, "E" | "I" | "P" | "S" | "R") {
                    return Err(at_line("set the Class: value to E, I, P, S or R"));
                }
                self.class = Some(value);
            }
            "Assisted-by" => {
                if value != "none" && !valid_model_id(value) {
                    return Err(at_line(
                        "use Assisted-by: none or Assisted-by: <agent>:<model> with each id using only ASCII letters, digits, dots, underscores or hyphens",
                    ));
                }
                if self
                    .assistance
                    .is_some_and(|previous| previous == "none" || value == "none")
                {
                    return Err(at_line(
                        "keep one Assisted-by: none or the named models because none must be the only Assisted-by: trailer",
                    ));
                }
                self.assistance = Some(value);
            }
            "AI-Reviewed-by" => {
                if rules == Rules::Commit {
                    return Err(at_line("remove AI-Reviewed-by: from the branch commit"));
                }
                if !valid_model_id(value) {
                    return Err(at_line(
                        "use AI-Reviewed-by: <agent>:<model> with each id using only ASCII letters, digits, dots, underscores or hyphens",
                    ));
                }
                self.reviewed = true;
            }
            "Signed-off-by" => {
                if rules == Rules::Commit {
                    return Err(at_line("remove Signed-off-by: from the branch commit"));
                }
                let email = trailer_email(value).ok_or_else(|| {
                    at_line("use Signed-off-by: Name <email> with a nonempty name and a valid email address")
                })?;
                self.signed |= rules != Rules::Main
                    || MAINTAINERS.iter().any(|(_, address)| *address == email);
            }
            "Co-authored-by" | "Reviewed-by" => {
                trailer_email(value).ok_or_else(|| {
                    at_line(if key == "Co-authored-by" {
                        "use Co-authored-by: Name <email> with a nonempty name and a valid email address"
                    } else {
                        "use Reviewed-by: Name <email> with a nonempty name and a valid email address"
                    })
                })?;
            }
            _ => {
                return Err(at_line(
                    "unknown trailer key. Use Class:, Assisted-by:, AI-Reviewed-by:, Signed-off-by:, Co-authored-by: or Reviewed-by: as allowed by the mode",
                ));
            }
        }
        Ok(())
    }

    fn require(&self, rules: Rules, line: usize) -> Result<(), Violation> {
        let at_block = |fix| Violation::at(line, fix);
        if rules != Rules::Commit && self.class.is_none() {
            return Err(at_block(
                "add one Class: trailer with a value of E, I, P, S or R",
            ));
        }
        if self.assistance.is_none() {
            return Err(at_block("add an Assisted-by: trailer"));
        }
        if rules == Rules::Main && self.class != Some("E") && !self.reviewed {
            return Err(at_block(
                "add an AI-Reviewed-by: trailer for a non-E change on main",
            ));
        }
        match rules {
            Rules::ContributorPr if !self.signed => Err(at_block(
                "add a Signed-off-by: Name <email> trailer for the pull request author",
            )),
            Rules::Main if !self.signed => Err(at_block(
                "add a Signed-off-by: with a maintainer email address",
            )),
            _ => Ok(()),
        }
    }
}

pub(super) fn valid_model_id(value: &str) -> bool {
    value.split_once(':').is_some_and(|(agent, model)| {
        [agent, model].into_iter().all(|id| {
            !id.is_empty()
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        })
    })
}

/// Extracts the address from a name and an unquoted mailbox in angle brackets.
fn trailer_email(value: &str) -> Option<&str> {
    let (name, email) = value.strip_suffix('>')?.rsplit_once(" <")?;
    let (local, domain) = email.split_once('@')?;
    let valid_name = !name.trim().is_empty()
        && !name
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '<' | '>'));
    let valid_local = local.split('.').all(|part| {
        !part.is_empty()
            && part.chars().all(|ch| {
                !ch.is_control() && !ch.is_whitespace() && !r#"()<>,;:"@[]\"#.contains(ch)
            })
    });
    let valid_domain = domain.split('.').all(|label| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|ch| ch.is_alphanumeric() || ch == '-')
    });
    (valid_name && valid_local && valid_domain).then_some(email)
}

#[derive(Debug)]
struct Fence {
    marker: char,
    width: usize,
}

/// Updates fence state and reports whether this line is part of fenced code.
fn in_fenced_code(open: &mut Option<Fence>, line: &str) -> bool {
    let indented = line.trim_start_matches(' ');
    if line.len() - indented.len() <= 3
        && let Some(marker @ ('`' | '~')) = indented.chars().next()
    {
        let tail = indented.trim_start_matches(marker);
        let width = indented.len() - tail.len();
        if width >= 3 {
            if let Some(fence) = open {
                if fence.marker == marker
                    && width >= fence.width
                    && tail.trim_matches([' ', '\t']).is_empty()
                {
                    *open = None;
                    return true;
                }
            } else if marker == '~' || !tail.contains('`') {
                *open = Some(Fence { marker, width });
            }
        }
    }
    open.is_some()
}
