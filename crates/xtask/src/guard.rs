// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Claude Code `PreToolUse` guard for Bash commands.

mod gh;
mod git;
mod self_test;
mod shell;

#[cfg(test)]
mod tests;

use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Output, Stdio};

use shell::{Item, Stdin, Word};

use crate::{Command, Error, json};

/// Largest hook input read; a longer one is blocked.
const MAX_INPUT: u64 = 16 << 20;

/// Largest message or body file read.
const MAX_FILE: u64 = 1 << 20;

/// Deepest nesting of `sh -c`, `eval` and alias expansions followed.
const MAX_DEPTH: usize = 8;

/// Where a `git` command runs: its directory, the global options before its
/// subcommand, and the `GIT_*` variables set for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Place {
    pub(super) dir: PathBuf,
    pub(super) options: Vec<String>,
    pub(super) env: Vec<(String, String)>,
}

/// What the guard asks of the repository and the file system. The real
/// implementation runs `git` and `gh`; tests use a fake.
pub(super) trait Repo {
    /// The current branch, or `None` on a detached head.
    fn branch(&self, place: &Place) -> Result<Option<String>, String>;
    /// The remote branch a bare `git push` of `branch` updates, if configured.
    fn push_target(&self, place: &Place, branch: &str) -> Result<Option<String>, String>;
    fn git_alias(&self, place: &Place, name: &str) -> Result<Option<String>, String>;
    fn gh_alias(&self, name: &str) -> Result<Option<String>, String>;
    fn read_file(&self, path: &Path) -> Result<String, String>;
}

/// Reads a Claude Code `PreToolUse` hook input from `input` and allows the
/// tool call or blocks it. With `self_test`, checks planted commands instead.
///
/// # Errors
///
/// Returns [`Error::Blocked`], which exits with status 2, for a blocked
/// command and for any input the guard cannot read: it fails closed.
pub(super) fn run(
    self_test: bool,
    input: &mut impl Read,
    output: &mut impl Write,
) -> Result<(), Error> {
    if self_test {
        return self_test::run(output);
    }
    let mut text = String::new();
    let fixes = match input.take(MAX_INPUT + 1).read_to_string(&mut text) {
        Ok(read) if read as u64 > MAX_INPUT => {
            vec!["the hook input is larger than the guard reads".to_owned()]
        }
        Ok(_) => check_input(&text, &System),
        Err(error) => vec![format!("could not read the hook input ({error})")],
    };
    if fixes.is_empty() {
        Ok(())
    } else {
        Err(Error::Blocked(fixes))
    }
}

/// Checks one hook input: a JSON object with `tool_name`, and for Bash,
/// `tool_input.command` and `cwd`. Other tools are allowed.
fn check_input(text: &str, repo: &impl Repo) -> Vec<String> {
    let input = match json::parse(text) {
        Ok(input) => input,
        Err(error) => return vec![format!("could not parse the hook input ({error})")],
    };
    match input.get("tool_name").and_then(json::Value::as_str) {
        Some("Bash") => (),
        Some(_) => return Vec::new(),
        None => return vec!["the hook input has no tool_name".to_owned()],
    }
    let command = input
        .get("tool_input")
        .and_then(|tool| tool.get("command"))
        .and_then(json::Value::as_str);
    let cwd = input.get("cwd").and_then(json::Value::as_str);
    match (command, cwd) {
        (Some(command), Some(cwd)) => evaluate(command, Path::new(cwd), repo),
        _ => vec!["the hook input has no tool_input.command or cwd".to_owned()],
    }
}

/// The guard's view of the real repository, through `git` and `gh`.
struct System;

impl System {
    fn git(place: &Place, args: &[&str]) -> Result<Output, String> {
        Command::new("git")
            .current_dir(&place.dir)
            .args(&place.options)
            .args(args)
            .envs(place.env.iter().map(|(name, value)| (name, value)))
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|error| format!("git: {error}"))
    }

    /// A value printed by git, `None` for exit status 1 (absent).
    fn git_value(place: &Place, args: &[&str]) -> Result<Option<String>, String> {
        let output = Self::git(place, args)?;
        match output.status.code() {
            Some(0) => String::from_utf8(output.stdout)
                .map(|text| Some(text.trim_end_matches(['\n', '\r']).to_owned()))
                .map_err(|_| "git printed text that is not UTF-8".to_owned()),
            Some(1) => Ok(None),
            _ => Err(format!(
                "git {} failed with {}",
                args.join(" "),
                output.status
            )),
        }
    }
}

impl Repo for System {
    fn branch(&self, place: &Place) -> Result<Option<String>, String> {
        Self::git_value(place, &["symbolic-ref", "--quiet", "--short", "HEAD"])
    }

    fn push_target(&self, place: &Place, branch: &str) -> Result<Option<String>, String> {
        let reference = format!("refs/heads/{branch}");
        let target = Self::git_value(
            place,
            &["for-each-ref", "--format=%(push:remoteref)", &reference],
        )?;
        Ok(target.filter(|target| !target.is_empty()))
    }

    fn git_alias(&self, place: &Place, name: &str) -> Result<Option<String>, String> {
        Self::git_value(place, &["config", "--get", &format!("alias.{name}")])
    }

    fn gh_alias(&self, name: &str) -> Result<Option<String>, String> {
        let output = match Command::new("gh")
            .args(["alias", "list"])
            .stdin(Stdio::null())
            .output()
        {
            Ok(output) => output,
            // Without gh, a gh command cannot run either.
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("gh alias list: {error}")),
        };
        if !output.status.success() {
            return if String::from_utf8_lossy(&output.stderr).contains("no aliases") {
                Ok(None)
            } else {
                Err(format!("gh alias list failed with {}", output.status))
            };
        }
        let listing = String::from_utf8_lossy(&output.stdout);
        Ok(listing.lines().find_map(|line| {
            let (alias, expansion) = line.split_once(": ")?;
            (alias == name).then(|| expansion.trim_matches(['\'', '"']).to_owned())
        }))
    }

    fn read_file(&self, path: &Path) -> Result<String, String> {
        let mut text = String::new();
        std::fs::File::open(path)
            .and_then(|file| file.take(MAX_FILE + 1).read_to_string(&mut text))
            .map_err(|error| error.to_string())
            .and_then(|read| {
                if read as u64 > MAX_FILE {
                    Err("larger than 1 MiB".to_owned())
                } else {
                    Ok(text)
                }
            })
    }
}

/// Checks a Bash command and returns what to fix; empty means allowed.
///
/// The guard fails closed: a command it cannot parse, a command name or
/// argument it needs that expands at run time, or a repository it cannot
/// query is blocked with a fix that says how to write it instead.
pub(super) fn evaluate(command: &str, cwd: &Path, repo: &impl Repo) -> Vec<String> {
    let mut guard = Guard {
        repo,
        fixes: Vec::new(),
    };
    let mut scope = Scope {
        cwd: Some(cwd.to_path_buf()),
        branch_changed: false,
    };
    guard.script(command, &mut scope, 0);
    guard.fixes
}

struct Guard<'r, R> {
    repo: &'r R,
    fixes: Vec<String>,
}

/// What earlier commands in the same script changed.
#[derive(Debug, Clone)]
struct Scope {
    /// The shell's directory, or `None` once a `cd` went somewhere unknown.
    cwd: Option<PathBuf>,
    /// Whether a `git checkout` or `git switch` ran earlier.
    branch_changed: bool,
}

/// What the text of a published message or a commit message came to.
enum Text {
    Absent,
    Known(String),
    Unknown,
    Unreadable(String),
}

impl<R: Repo> Guard<'_, R> {
    fn block(&mut self, fix: impl Into<String>) {
        let fix = fix.into();
        if !self.fixes.contains(&fix) {
            self.fixes.push(fix);
        }
    }

    fn too_deep(&mut self, depth: usize) -> bool {
        let deep = depth > MAX_DEPTH;
        if deep {
            self.block("the guard follows at most 8 nested shells, evals or aliases: run the commands directly");
        }
        deep
    }

    fn script(&mut self, script: &str, scope: &mut Scope, depth: usize) {
        if self.too_deep(depth) {
            return;
        }
        let items = match shell::parse(script) {
            Ok(items) => items,
            Err(error) => {
                self.block(format!(
                    "the guard could not parse this command ({error}): write it as plain commands, without case or other syntax it does not read"
                ));
                return;
            }
        };
        let mut outer = Vec::new();
        for item in items {
            match item {
                Item::Enter => outer.push(scope.clone()),
                Item::Leave => {
                    if let Some(saved) = outer.pop() {
                        // A subshell's `cd` ends with it; a branch switch does not.
                        scope.cwd = saved.cwd;
                    }
                }
                Item::Command(command) => {
                    self.command(&command.words, &command.stdin, scope, depth);
                }
            }
        }
    }

    /// A child shell: directory changes stay inside it.
    fn child_script(&mut self, script: &str, scope: &mut Scope, depth: usize) {
        let mut child = scope.clone();
        self.script(script, &mut child, depth + 1);
        scope.branch_changed |= child.branch_changed;
    }

    fn command(&mut self, words: &[Word], stdin: &Stdin, scope: &mut Scope, depth: usize) {
        let mut env = Vec::new();
        let mut rest = words;
        while let Some((first, tail)) = rest.split_first() {
            let Some((name, value)) = assignment(first) else {
                break;
            };
            if name.starts_with("GIT_") {
                env.push((name.to_owned(), value));
            }
            rest = tail;
        }
        self.run(rest, stdin, scope, &mut env, depth);
    }

    fn run(
        &mut self,
        words: &[Word],
        stdin: &Stdin,
        scope: &mut Scope,
        env: &mut Vec<(String, Option<String>)>,
        depth: usize,
    ) {
        let Some((name, args)) = words.split_first() else {
            return;
        };
        let Some(name) = name.literal() else {
            self.block("write the command name literally: the guard cannot tell which command an expansion runs");
            return;
        };
        let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
        let base = base.strip_suffix(".exe").unwrap_or(base);
        match base {
            "{" | "}" | "!" | "if" | "then" | "else" | "elif" | "fi" | "do" | "done" | "while"
            | "until" | "builtin" | "exec" | "nohup" => self.run(args, stdin, scope, env, depth),
            "time" => self.run(skip_flags(args, &[]), stdin, scope, env, depth),
            "command" => {
                if !matches!(args.first().and_then(Word::literal), Some("-v" | "-V")) {
                    self.run(skip_flags(args, &[]), stdin, scope, env, depth);
                }
            }
            "nice" => self.run(skip_flags(args, &["-n"]), stdin, scope, env, depth),
            "sudo" => self.run(
                skip_flags(
                    args,
                    &["-u", "-g", "-h", "-p", "-C", "-D", "-U", "-T", "-r", "-t"],
                ),
                stdin,
                scope,
                env,
                depth,
            ),
            "timeout" => {
                let rest = skip_flags(args, &["-s", "-k", "--signal", "--kill-after"]);
                self.run(rest.get(1..).unwrap_or(&[]), stdin, scope, env, depth);
            }
            "xargs" => {
                // xargs appends words read at run time to its command.
                let mut rest =
                    skip_flags(args, &["-n", "-I", "-L", "-P", "-s", "-E", "-d", "-a"]).to_vec();
                if !rest.is_empty() {
                    rest.push(Word::Dynamic(String::new()));
                }
                self.run(&rest, stdin, scope, env, depth);
            }
            "env" => self.env(args, stdin, scope, env, depth),
            "sh" | "bash" | "zsh" | "dash" | "ksh" => self.shell(args, stdin, scope, depth),
            "eval" => {
                let text: Option<Vec<&str>> = args.iter().map(Word::literal).collect();
                match text {
                    Some(parts) => self.script(&parts.join(" "), scope, depth + 1),
                    None => self
                        .block("write the text given to eval literally so the guard can read it"),
                }
            }
            "cd" | "pushd" => {
                scope.cwd = match args.first().map(Word::literal) {
                    Some(Some(dir)) if !dir.starts_with('-') => {
                        scope.cwd.as_ref().map(|cwd| normalize(&cwd.join(dir)))
                    }
                    _ => None,
                };
            }
            "popd" => scope.cwd = None,
            "git" => self.git(args, stdin, scope, env, depth),
            "gh" => self.gh(args, stdin, scope, depth),
            _ => (),
        }
    }

    fn env(
        &mut self,
        args: &[Word],
        stdin: &Stdin,
        scope: &mut Scope,
        env: &mut Vec<(String, Option<String>)>,
        depth: usize,
    ) {
        let mut local = scope.clone();
        let mut index = 0;
        while let Some(word) = args.get(index) {
            match word.literal() {
                Some("-i" | "-0" | "--ignore-environment" | "--null" | "--") => (),
                Some("-u" | "--unset") => index += 1,
                Some("-C" | "--chdir") => {
                    index += 1;
                    local.cwd = match (local.cwd, args.get(index).and_then(Word::literal)) {
                        (Some(cwd), Some(dir)) => Some(normalize(&cwd.join(dir))),
                        _ => None,
                    };
                }
                Some(split) if split.starts_with("-S") || split.starts_with("--split-string") => {
                    self.block(
                        "env -S hides the command from the guard: write the command directly",
                    );
                    return;
                }
                _ => match assignment(word) {
                    Some((name, value)) => {
                        if name.starts_with("GIT_") {
                            env.push((name.to_owned(), value));
                        }
                    }
                    None => break,
                },
            }
            index += 1;
        }
        self.run(
            args.get(index..).unwrap_or(&[]),
            stdin,
            &mut local,
            env,
            depth,
        );
        scope.branch_changed |= local.branch_changed;
    }

    fn shell(&mut self, args: &[Word], stdin: &Stdin, scope: &mut Scope, depth: usize) {
        let mut index = 0;
        let mut command_mode = false;
        while let Some(option) = args.get(index).and_then(Word::literal) {
            if option == "--" {
                index += 1;
                break;
            }
            if option.len() < 2 || !(option.starts_with('-') || option.starts_with('+')) {
                break;
            }
            if option.starts_with('-') && !option.starts_with("--") && option.contains('c') {
                command_mode = true;
            }
            if matches!(option, "-o" | "+o" | "-O" | "+O") {
                index += 1;
            }
            index += 1;
        }
        let operands = args.get(index..).unwrap_or(&[]);
        if command_mode {
            match operands.first().map(Word::literal) {
                Some(Some(script)) => self.child_script(script, scope, depth),
                Some(None) => {
                    self.block(
                        "write the script given to sh -c literally so the guard can read it",
                    );
                }
                None => (),
            }
            return;
        }
        if !operands.is_empty() {
            // Running a script file is outside what the guard reads.
            return;
        }
        match self.stdin_text(stdin, scope) {
            Text::Known(script) => self.child_script(&script, scope, depth),
            Text::Absent => (),
            Text::Unknown | Text::Unreadable(_) => self.block(
                "the guard cannot read a script piped to a shell's standard input: run the commands directly",
            ),
        }
    }

    /// The text a command reads on standard input, if the guard can see it.
    fn stdin_text(&self, stdin: &Stdin, scope: &Scope) -> Text {
        match stdin {
            Stdin::Terminal => Text::Absent,
            Stdin::Text(Some(text)) => Text::Known(text.clone()),
            Stdin::File(Some(path)) => self.read_relative(scope.cwd.as_deref(), path),
            Stdin::Text(None) | Stdin::File(None) | Stdin::Pipe => Text::Unknown,
        }
    }

    fn read_relative(&self, base: Option<&Path>, path: &str) -> Text {
        let Some(base) = base else {
            return Text::Unknown;
        };
        match self.repo.read_file(&normalize(&base.join(path))) {
            Ok(text) => Text::Known(text),
            Err(error) => Text::Unreadable(format!(
                "could not read {path} ({error}): write the file in an earlier command"
            )),
        }
    }
}

/// Splits `NAME=value`; the value is `None` when it expands at run time.
fn assignment(word: &Word) -> Option<(&str, Option<String>)> {
    let (text, dynamic) = match word {
        Word::Literal(text) => (text.as_str(), false),
        Word::Dynamic(prefix) => (prefix.as_str(), true),
    };
    let (name, value) = text.split_once('=')?;
    let valid = name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    valid.then(|| (name, (!dynamic).then(|| value.to_owned())))
}

/// Skips leading options, and the value after each option in `with_value`.
fn skip_flags<'w>(args: &'w [Word], with_value: &[&str]) -> &'w [Word] {
    let mut index = 0;
    while let Some(option) = args.get(index).and_then(Word::literal) {
        if option == "--" {
            index += 1;
            break;
        }
        if option.len() < 2 || !option.starts_with('-') {
            break;
        }
        if with_value.contains(&option) {
            index += 1;
        }
        index += 1;
    }
    args.get(index..).unwrap_or(&[])
}

/// Resolves `.` and `..` without touching the file system.
fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => (),
            Component::ParentDir => {
                normal.pop();
            }
            other => normal.push(other),
        }
    }
    normal
}
