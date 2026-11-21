// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `guard --self-test`: runs planted commands through the guard's rules,
//! against a planted repository, and proves that each rule blocks its
//! planted command with its own fix and lets ordinary commands through.

use std::io::Write;
use std::path::Path;

use super::{Place, Repo, evaluate};
use crate::Error;

const CHECK: &str = "guard";
const CWD: &str = "/planted/feature";

/// A repository with a task branch at [`CWD`] and `main` at `/planted/main`,
/// no aliases and no files.
struct Planted;

impl Repo for Planted {
    fn branch(&self, place: &Place) -> Result<Option<String>, String> {
        match place.dir.to_str() {
            Some("/planted/main") => Ok(Some("main".to_owned())),
            Some(CWD) => Ok(Some("feature".to_owned())),
            _ => Err("not a planted repository".to_owned()),
        }
    }

    fn push_target(&self, _place: &Place, _branch: &str) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn git_alias(&self, _place: &Place, _name: &str) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn gh_alias(&self, _name: &str) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn read_file(&self, path: &Path) -> Result<String, String> {
        Err(format!("no planted file {}", path.display()))
    }
}

/// A planted command, and text its fix must contain (empty: allowed).
struct Case {
    name: &'static str,
    command: &'static str,
    fix: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "a commit without Assisted-by:",
        command: "git commit -m 'subject'",
        fix: "Assisted-by",
    },
    Case {
        name: "a commit on main",
        command: "git -C /planted/main commit -m 'subject' -m 'Assisted-by: none'",
        fix: "is on main",
    },
    Case {
        name: "a push to main",
        command: "git push origin HEAD:main",
        fix: "pushes to main",
    },
    Case {
        name: "a push to main inside sh -c",
        command: "sh -c 'git -C ../feature push origin +refs/heads/main'",
        fix: "pushes to main",
    },
    Case {
        name: "a comment without its disclosure line",
        command: "gh issue comment 1 --body 'text'",
        fix: "disclosure line",
    },
    Case {
        name: "gh pr merge without a pinned head and message file",
        command: "gh pr merge 1 --squash",
        fix: "--match-head-commit",
    },
    Case {
        name: "a merge through gh api",
        command: "gh api -X PUT repos/o/r/pulls/1/merge",
        fix: "gh api",
    },
    Case {
        name: "a command the guard cannot parse",
        command: "echo 'unterminated",
        fix: "could not parse",
    },
    Case {
        name: "a branch commit with Assisted-by:",
        command: "git commit -m 'subject' -m 'Assisted-by: none'",
        fix: "",
    },
    Case {
        name: "a push of the task branch",
        command: "git push -u origin feature",
        fix: "",
    },
    Case {
        name: "a comment with its disclosure line",
        command: "gh issue comment 1 --body 'Text.\n\nWritten by claude-code:claude-opus-5-5.'",
        fix: "",
    },
    Case {
        name: "reading",
        command: "git log --oneline main && gh pr view 1 && grep -r 'git push origin main' .",
        fix: "",
    },
];

pub(super) fn run(output: &mut impl Write) -> Result<(), Error> {
    let mut report =
        |line: String| writeln!(output, "{line}").map_err(Error::io("report guard self-test"));
    for case in CASES {
        let found = evaluate(case.command, Path::new(CWD), &Planted);
        let as_planted = if case.fix.is_empty() {
            found.is_empty()
        } else {
            found.iter().any(|fix| fix.contains(case.fix))
        };
        if !as_planted {
            return Err(Error::SelfTest {
                check: CHECK,
                case: case.name,
                found,
            });
        }
        match found.first() {
            Some(fix) => report(format!("caught: {}: {fix}", case.name))?,
            None => report(format!("passed: {}", case.name))?,
        }
    }
    report(format!(
        "guard --self-test: {} planted commands ended as planted",
        CASES.len()
    ))
}
