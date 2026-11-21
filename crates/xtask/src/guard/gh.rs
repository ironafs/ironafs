// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The guard's rules for `gh`: published text carries a disclosure line
//! (ADR 0003 point 6), `gh pr merge` pins the approved head and message
//! (the `land-change` skill, section 4), and `gh api` writes that would get
//! around either are refused.

use super::shell::{Stdin, Word};
use super::{Guard, Repo, Scope, Text};
use crate::trailers;

/// Top-level `gh` commands; any other name may be an alias.
const COMMANDS: &[&str] = &[
    "alias",
    "api",
    "attestation",
    "auth",
    "browse",
    "cache",
    "codespace",
    "completion",
    "config",
    "extension",
    "gist",
    "gpg-key",
    "help",
    "issue",
    "label",
    "org",
    "pr",
    "project",
    "release",
    "repo",
    "ruleset",
    "run",
    "search",
    "secret",
    "ssh-key",
    "status",
    "variable",
    "workflow",
];

/// Flags of `gh issue` and `gh pr` that take a value: (short, long).
const ISSUE_AND_PR_FLAGS: &[(Option<char>, &str)] = &[
    (Some('b'), "body"),
    (Some('F'), "body-file"),
    (Some('t'), "title"),
    (Some('R'), "repo"),
    (Some('a'), "assignee"),
    (Some('l'), "label"),
    (Some('m'), "milestone"),
    (Some('p'), "project"),
    (Some('B'), "base"),
    (Some('H'), "head"),
    (Some('T'), "template"),
    (Some('r'), "reviewer"),
    (Some('c'), "comment"),
    (Some('A'), "author-email"),
    (None, "match-head-commit"),
    (None, "subject"),
];

const RELEASE_FLAGS: &[(Option<char>, &str)] = &[
    (Some('n'), "notes"),
    (Some('F'), "notes-file"),
    (Some('t'), "title"),
    (Some('R'), "repo"),
    (None, "target"),
    (None, "notes-start-tag"),
    (None, "discussion-category"),
];

const API_FLAGS: &[(Option<char>, &str)] = &[
    (Some('X'), "method"),
    (Some('f'), "raw-field"),
    (Some('F'), "field"),
    (Some('H'), "header"),
    (Some('q'), "jq"),
    (Some('t'), "template"),
    (Some('p'), "preview"),
    (None, "input"),
    (None, "hostname"),
    (None, "cache"),
];

/// Path segments of REST endpoints whose writes publish text or change
/// branches: issues, pull requests, comments, merges, refs and contents.
const UNCHECKED_ENDPOINTS: &[&str] = &[
    "issues", "pulls", "comments", "merges", "merge", "git", "contents", "releases",
];

const DYNAMIC_OPTION: &str = "write gh options literally, as --body \"$TEXT\" rather than --body=\"$TEXT\", so the guard can tell which option it is and check the disclosure line";

const SQUASH_FIX: &str = "gh pr merge lands a change on main only as a squash merge: use --squash, without --merge or --rebase (the land-change skill, section 4)";

const MERGE_FIX: &str = "gh pr merge needs --match-head-commit <approved head> and --body-file <approved message file> (the land-change skill, section 4)";

/// Parsed `gh` arguments. A value is `None` when it expands at run time.
struct Flags {
    values: Vec<(&'static str, Option<String>)>,
    switches: Vec<String>,
    positionals: Vec<Word>,
    /// Whether an option's own name expands at run time, as in
    /// `--body="$TEXT"`: the guard cannot tell which option it is.
    dynamic_option: bool,
}

impl Flags {
    fn parse(args: &[Word], table: &[(Option<char>, &'static str)]) -> Self {
        let mut flags = Self {
            values: Vec::new(),
            switches: Vec::new(),
            positionals: Vec::new(),
            dynamic_option: false,
        };
        let mut index = 0;
        let mut options_done = false;
        while let Some(word) = args.get(index) {
            index += 1;
            let text = match word {
                Word::Dynamic(prefix) if !options_done && prefix.starts_with('-') => {
                    flags.dynamic_option = true;
                    continue;
                }
                Word::Literal(text) if !options_done && text.len() > 1 && text.starts_with('-') => {
                    text.as_str()
                }
                _ => {
                    flags.positionals.push(word.clone());
                    continue;
                }
            };
            let (long, attached) = if text == "--" {
                options_done = true;
                continue;
            } else if let Some(long) = text.strip_prefix("--") {
                let (name, attached) = match long.split_once('=') {
                    Some((name, attached)) => (name, Some(attached)),
                    None => (long, None),
                };
                if let Some((_, known)) = table.iter().find(|(_, known)| *known == name) {
                    (Some(*known), attached)
                } else {
                    flags.switches.push(name.to_owned());
                    continue;
                }
            } else {
                let mut chars = text.chars().skip(1);
                let short = chars.next();
                let tail = text.get(2..).unwrap_or("");
                let attached = tail
                    .strip_prefix('=')
                    .or((!tail.is_empty()).then_some(tail));
                if let Some((_, long)) = table.iter().find(|(known, _)| *known == short) {
                    (Some(*long), attached)
                } else {
                    flags
                        .switches
                        .extend(text.chars().skip(1).map(String::from));
                    continue;
                }
            };
            if let Some(long) = long {
                let value = if let Some(attached) = attached {
                    Some(attached.to_owned())
                } else {
                    let next = args.get(index).and_then(Word::literal).map(str::to_owned);
                    index += 1;
                    next
                };
                flags.values.push((long, value));
            }
        }
        flags
    }

    fn value(&self, long: &str) -> Option<&Option<String>> {
        self.values
            .iter()
            .rev()
            .find(|(name, _)| *name == long)
            .map(|(_, value)| value)
    }

    fn has(&self, names: &[&str]) -> bool {
        self.switches
            .iter()
            .any(|switch| names.contains(&switch.as_str()))
    }
}

impl<R: Repo> Guard<'_, R> {
    pub(super) fn gh(
        &mut self,
        args: &[Word],
        stdin: &Stdin,
        scope: &mut Scope,
        env: &[(String, Option<String>)],
        depth: usize,
    ) {
        if self.too_deep(depth) {
            return;
        }
        let Some((first, rest)) = args.split_first() else {
            return;
        };
        let Some(command) = first.literal() else {
            self.block("write the gh command literally so the guard can read it");
            return;
        };
        match command {
            "issue" | "pr" => self.issue_or_pr(command, rest, stdin, scope),
            "release" => self.release(rest, stdin, scope),
            "api" => self.api(rest),
            "alias" => {
                if !matches!(rest.first().and_then(Word::literal), Some("list" | "ls")) {
                    scope.config_changed = true;
                }
            }
            "--version" | "--help" | "-h" => (),
            _ if command.starts_with('-') => self.block(format!(
                "gh option {command} before the command: put options after the gh command (gh issue create -R o/r), so the guard can read which command runs"
            )),
            _ if COMMANDS.contains(&command) => (),
            _ if scope.config_changed => self.block(format!(
                "gh {command} may be an alias set earlier in this command: run gh alias set in a separate command first, so the guard can read the alias"
            )),
            _ => match self.repo.gh_alias(command) {
                Ok(Some(expansion)) => {
                    if let Some(script) = expansion.strip_prefix('!') {
                        // gh runs a shell alias with the command's environment.
                        self.child_script(script, scope, env, depth);
                    } else if let Some(words) = self.alias_words(&expansion) {
                        let expanded: Vec<Word> =
                            words.into_iter().chain(rest.iter().cloned()).collect();
                        self.gh(&expanded, stdin, scope, env, depth + 1);
                    }
                }
                Ok(None) => (),
                Err(error) => self.block(format!(
                    "could not look up whether gh {command} is an alias ({error}): run the command it stands for"
                )),
            },
        }
    }

    fn issue_or_pr(&mut self, group: &str, rest: &[Word], stdin: &Stdin, scope: &Scope) {
        let Some((subcommand, args)) = rest.split_first() else {
            return;
        };
        let Some(subcommand) = subcommand.literal() else {
            self.block("write the gh subcommand literally so the guard can read it");
            return;
        };
        // `gh pr review --comment` is a switch; elsewhere it takes the text.
        let table: Vec<(Option<char>, &'static str)> = ISSUE_AND_PR_FLAGS
            .iter()
            .copied()
            .filter(|(_, long)| subcommand != "review" || *long != "comment")
            .collect();
        let flags = Flags::parse(args, &table);
        if flags.dynamic_option {
            self.block(DYNAMIC_OPTION);
            return;
        }
        let what = format!("gh {group} {subcommand}");
        if flags.has(&["web", "w"]) {
            // The text is written in the browser, by a person.
            return;
        }
        match subcommand {
            "create" => {
                let text = self.published_text(&flags, "body", "body-file", stdin, scope);
                self.require_disclosure(text, &what, false);
            }
            "comment" if !flags.has(&["delete-last"]) => {
                let text = self.published_text(&flags, "body", "body-file", stdin, scope);
                self.require_disclosure(text, &what, false);
            }
            "review" | "edit" => {
                let text = self.published_text(&flags, "body", "body-file", stdin, scope);
                self.require_disclosure(text, &what, true);
            }
            "close" | "reopen" => {
                let text = match flags.value("comment") {
                    None => Text::Absent,
                    Some(None) => Text::Unknown,
                    Some(Some(text)) => Text::Known(text.clone()),
                };
                self.require_disclosure(text, &what, true);
            }
            "merge" if group == "pr" && !flags.has(&["disable-auto"]) => {
                let squash = flags.has(&["squash", "s"])
                    && !flags.has(&["merge", "rebase"])
                    && flags.value("milestone").is_none()
                    && flags.value("reviewer").is_none();
                if !squash {
                    self.block(SQUASH_FIX);
                }
                let pinned = matches!(flags.value("match-head-commit"), Some(Some(head)) if !head.is_empty());
                let message_file = matches!(flags.value("body-file"), Some(Some(file)) if !file.is_empty() && file != "-");
                if !pinned || !message_file || flags.value("body").is_some() {
                    self.block(MERGE_FIX);
                }
            }
            _ => (),
        }
    }

    fn release(&mut self, rest: &[Word], stdin: &Stdin, scope: &Scope) {
        let Some((subcommand, args)) = rest.split_first() else {
            return;
        };
        if matches!(subcommand.literal(), Some("create" | "edit")) {
            let flags = Flags::parse(args, RELEASE_FLAGS);
            if flags.dynamic_option {
                self.block(DYNAMIC_OPTION);
                return;
            }
            let text = self.published_text(&flags, "notes", "notes-file", stdin, scope);
            self.require_disclosure(text, "gh release notes", true);
        }
    }

    fn api(&mut self, rest: &[Word]) {
        let flags = Flags::parse(rest, API_FLAGS);
        if flags.dynamic_option {
            self.block(DYNAMIC_OPTION);
            return;
        }
        let sends = ["raw-field", "field", "input"]
            .iter()
            .any(|name| flags.value(name).is_some());
        let method = match flags.value("method") {
            Some(Some(method)) => method.to_ascii_uppercase(),
            Some(None) => "a run-time method".to_owned(),
            None if sends => "POST".to_owned(),
            None => "GET".to_owned(),
        };
        if method == "GET" || method == "HEAD" {
            return;
        }
        let Some(endpoint) = flags.positionals.first() else {
            return;
        };
        let Some(endpoint) = endpoint.literal() else {
            self.block("gh api: write the endpoint literally so the guard can read it");
            return;
        };
        let endpoint = endpoint.trim_start_matches('/');
        let unchecked = if endpoint == "graphql" {
            flags.value("input").is_some()
                || flags
                    .values
                    .iter()
                    .filter(|(name, _)| matches!(*name, "raw-field" | "field"))
                    .any(|(_, value)| {
                        value
                            .as_ref()
                            .is_none_or(|value| value.contains("mutation"))
                    })
        } else {
            endpoint
                .split(['/', '?'])
                .any(|segment| UNCHECKED_ENDPOINTS.contains(&segment))
        };
        if unchecked {
            self.block(format!(
                "gh api {method} {endpoint}: the guard cannot check this write; use gh issue, gh pr, gh release or git push, which it checks"
            ));
        }
    }

    fn published_text(
        &self,
        flags: &Flags,
        text_flag: &str,
        file_flag: &str,
        stdin: &Stdin,
        scope: &Scope,
    ) -> Text {
        match flags.value(text_flag) {
            Some(Some(text)) => return Text::Known(text.clone()),
            Some(None) => return Text::Unknown,
            None => (),
        }
        match flags.value(file_flag) {
            None => Text::Absent,
            Some(None) => Text::Unknown,
            Some(Some(path)) if path == "-" => match self.stdin_text(stdin, scope) {
                Text::Absent => Text::Unknown,
                text => text,
            },
            Some(Some(path)) => self.read_relative(scope.cwd.as_deref(), path),
        }
    }

    fn require_disclosure(&mut self, text: Text, what: &str, absent_ok: bool) {
        match text {
            Text::Known(text) if has_disclosure(&text) => (),
            Text::Known(_) => self.block(format!(
                "{what}: end the text with a disclosure line such as `Written by <agent>:<model>.` (ADR 0003 point 6); the guard checks only that the line is present and well formed, not who wrote the text"
            )),
            Text::Absent if absent_ok => (),
            Text::Absent | Text::Unknown => self.block(format!(
                "{what}: give the text with --body or --body-file, ending with its disclosure line, so the guard can check it"
            )),
            Text::Unreadable(fix) => self.block(fix),
        }
    }
}

/// Whether a line reads `<Role> by|with <agent>:<model>`, as in
/// `Written by claude-code:claude-opus-5-5.` or
/// `Review by glm:glm-5.3, relayed by claude-code:claude-opus-5-5.`
fn has_disclosure(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim().trim_start_matches(['*', '_']);
        let mut words = line.split_whitespace();
        let (Some(role), Some(joiner), Some(id)) = (words.next(), words.next(), words.next())
        else {
            return false;
        };
        let id = id.trim_end_matches([',', '.', ';', '*', '_']);
        role.chars()
            .next()
            .is_some_and(|first| first.is_ascii_uppercase())
            && role.chars().all(|ch| ch.is_ascii_alphabetic())
            && matches!(joiner, "by" | "with")
            && trailers::valid_model_id(id)
    })
}
