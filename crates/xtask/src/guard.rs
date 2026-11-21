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
    /// Every configured `remote.<name>.fetch` refspec, of every remote.
    fn fetch_refspecs(&self, place: &Place) -> Result<Vec<String>, String>;
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

    fn fetch_refspecs(&self, place: &Place) -> Result<Vec<String>, String> {
        let listing = Self::git_value(place, &["config", "--get-regexp", r"^remote\..*\.fetch$"])?;
        Ok(listing
            .unwrap_or_default()
            .lines()
            .filter_map(|line| line.split_once(' ').map(|(_, refspec)| refspec.to_owned()))
            .collect())
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
        config_changed: false,
        env: Vec::new(),
        git_config: Vec::new(),
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
    /// Whether an earlier command wrote git configuration or a gh alias,
    /// so an alias looked up now may not be the one that runs.
    config_changed: bool,
    /// `GIT_*` variables exported or assigned earlier; `None` for a value
    /// that expands at run time.
    env: Vec<(String, Option<String>)>,
    /// `-c` options of the git that runs a shell alias, which the git
    /// commands in that alias inherit.
    git_config: Vec<String>,
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
                        // A subshell's `cd` and exports end with it; a branch
                        // switch and written configuration do not.
                        scope.cwd = saved.cwd;
                        scope.env = saved.env;
                    }
                }
                Item::Command(command) => {
                    self.command(&command.words, &command.stdin, scope, depth);
                }
            }
        }
    }

    /// A child shell, started with the `GIT_*` environment `env`: directory
    /// changes and exports stay inside it.
    fn child_script(
        &mut self,
        script: &str,
        scope: &mut Scope,
        env: &[(String, Option<String>)],
        depth: usize,
    ) {
        let mut child = scope.clone();
        child.env = env.to_vec();
        self.script(script, &mut child, depth + 1);
        scope.branch_changed |= child.branch_changed;
        scope.config_changed |= child.config_changed;
    }

    fn command(&mut self, words: &[Word], stdin: &Stdin, scope: &mut Scope, depth: usize) {
        let mut env = scope.env.clone();
        self.assigned(words, stdin, scope, &mut env, depth);
    }

    /// Runs `words` after the `NAME=value` assignments that lead them.
    fn assigned(
        &mut self,
        words: &[Word],
        stdin: &Stdin,
        scope: &mut Scope,
        env: &mut Vec<(String, Option<String>)>,
        depth: usize,
    ) {
        let mut rest = words;
        let mut assigned = Vec::new();
        while let Some((first, tail)) = rest.split_first() {
            let Some((name, value)) = assignment(first) else {
                break;
            };
            if name.starts_with("GIT_") {
                assigned.push((name.to_owned(), value));
            }
            rest = tail;
        }
        if rest.is_empty() {
            // A bare assignment sets a shell variable. The guard counts it as
            // exported: an `export NAME` or `set -a` may follow.
            scope.env.extend(assigned);
            return;
        }
        env.extend(assigned);
        self.run(rest, stdin, scope, env, depth);
    }

    /// Runs the command a wrapper such as `sudo` or `nice` runs, after its
    /// options. An option it does not know, or an assignment where the
    /// command should be, is blocked: the guard cannot tell what runs.
    fn wrapped(
        &mut self,
        name: &str,
        args: &[Word],
        stdin: &Stdin,
        scope: &mut Scope,
        env: &mut Vec<(String, Option<String>)>,
        depth: usize,
    ) {
        let Some(wrapper) = WRAPPERS.iter().find(|wrapper| wrapper.name == name) else {
            return;
        };
        let (options, rest) = match wrapper.options(args) {
            Ok(found) => found,
            Err(fix) => {
                self.block(fix);
                return;
            }
        };
        let rest = rest.get(wrapper.operands..).unwrap_or(&[]);
        let mut local = scope.clone();
        for (option, value) in options {
            if wrapper.chdir.contains(&option) {
                local.cwd = match (local.cwd, value) {
                    (Some(cwd), Some(dir)) => Some(normalize(&cwd.join(dir))),
                    _ => None,
                };
            }
        }
        if wrapper.keyword {
            self.assigned(rest, stdin, &mut local, env, depth);
        } else if rest.first().and_then(assignment).is_some() {
            self.block(format!(
                "{name} NAME=value: the guard does not read assignments after a wrapper, so it cannot tell which command runs; set the variable with env, or before {name}"
            ));
            return;
        } else {
            self.run(rest, stdin, &mut local, env, depth);
        }
        scope.branch_changed |= local.branch_changed;
        scope.config_changed |= local.config_changed;
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
            // Shell keywords: the words after them are a command, which may
            // start with assignments.
            "{" | "}" | "!" | "if" | "then" | "else" | "elif" | "fi" | "do" | "done" | "while"
            | "until" => self.assigned(args, stdin, scope, env, depth),
            "coproc" => {
                // `coproc NAME { …; }` names the coprocess. Before a simple
                // command bash takes no name: in `coproc x git …`, `x` is the
                // command, which is read as a runner.
                let body = match args.get(1).and_then(Word::literal) {
                    Some("{") => args.get(1..).unwrap_or(&[]),
                    _ => args,
                };
                self.assigned(body, stdin, scope, env, depth);
            }
            "command" if matches!(args.first().and_then(Word::literal), Some("-v" | "-V")) => (),
            "xargs" => {
                // xargs appends words read at run time to its command.
                let Some(wrapper) = WRAPPERS.iter().find(|wrapper| wrapper.name == "xargs") else {
                    return;
                };
                match wrapper.options(args) {
                    Ok((_, rest)) => {
                        let mut rest = rest.to_vec();
                        if !rest.is_empty() {
                            rest.push(Word::Dynamic(String::new()));
                        }
                        self.wrapped("command", &rest, stdin, scope, env, depth);
                    }
                    Err(fix) => self.block(fix),
                }
            }
            _ if WRAPPERS.iter().any(|wrapper| wrapper.name == base) => {
                self.wrapped(base, args, stdin, scope, env, depth);
            }
            "env" => self.env(args, stdin, scope, env, depth),
            "sh" | "bash" | "zsh" | "dash" | "ksh" => self.shell(args, stdin, scope, env, depth),
            // `source` and `eval` run in this shell, with the command's
            // assignments. The guard keeps them afterwards, as a POSIX shell
            // does, which can only block more.
            "source" | "." => {
                if let Some(script) = self.script_file(args.first(), scope) {
                    scope.env.clone_from(env);
                    self.script(&script, scope, depth + 1);
                }
            }
            "export" | "declare" | "typeset" | "local" | "readonly" => {
                for word in args {
                    if let Some((name, value)) = assignment(word)
                        && name.starts_with("GIT_")
                    {
                        scope.env.push((name.to_owned(), value));
                    }
                }
            }
            "unset" => {
                for name in args.iter().filter_map(Word::literal) {
                    scope.env.retain(|(known, _)| known != name);
                }
            }
            "eval" => {
                let text: Option<Vec<&str>> = args.iter().map(Word::literal).collect();
                match text {
                    Some(parts) => {
                        scope.env.clone_from(env);
                        self.script(&parts.join(" "), scope, depth + 1);
                    }
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
            _ if NOT_RUNNERS.contains(&base) => (),
            _ => self.runner(args, stdin, scope, env, depth),
        }
    }

    /// A command the guard does not know, read as if it may run a command
    /// given in its arguments, as `watch`, `strace` or `valgrind` do: the
    /// first argument named `git` or `gh` is checked as that command, with
    /// the assignments just before it, and an earlier argument that is a
    /// command line naming git or gh is checked as a script. Commands in
    /// [`NOT_RUNNERS`] are not read.
    fn runner(
        &mut self,
        args: &[Word],
        stdin: &Stdin,
        scope: &mut Scope,
        env: &mut Vec<(String, Option<String>)>,
        depth: usize,
    ) {
        let start = args
            .iter()
            .position(|word| word.literal().is_some_and(is_git_or_gh));
        for word in args.get(..start.unwrap_or(args.len())).unwrap_or(&[]) {
            if let Some(text) = word.literal()
                && text.contains(char::is_whitespace)
                && names_git_or_gh(text)
            {
                let env = env.clone();
                self.child_script(text, scope, &env, depth);
            }
        }
        let Some(start) = start else {
            return;
        };
        let first = args
            .get(..start)
            .unwrap_or(&[])
            .iter()
            .rposition(|word| assignment(word).is_none())
            .map_or(0, |last| last + 1);
        self.assigned(args.get(first..).unwrap_or(&[]), stdin, scope, env, depth);
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
            let text = word.literal();
            // The directory of `-C dir`, `-Cdir` or `--chdir=dir`.
            let chdir = match text {
                Some("-C" | "--chdir") => {
                    index += 1;
                    Some(args.get(index).and_then(Word::literal))
                }
                Some(text) => text
                    .strip_prefix("--chdir=")
                    .or_else(|| text.strip_prefix("-C"))
                    .map(Some),
                None => None,
            };
            if let Some(dir) = chdir {
                local.cwd = match (local.cwd, dir) {
                    (Some(cwd), Some(dir)) => Some(normalize(&cwd.join(dir))),
                    _ => None,
                };
                index += 1;
                continue;
            }
            match text {
                Some("-i" | "-0" | "--ignore-environment" | "--null" | "--") => (),
                Some("-u" | "--unset") => index += 1,
                Some(unset) if unset.starts_with("--unset=") || unset.starts_with("-u") => (),
                Some(split) if split.starts_with("-S") || split.starts_with("--split-string") => {
                    self.block(
                        "env -S hides the command from the guard: write the command directly",
                    );
                    return;
                }
                Some(option) if option.len() > 1 && option.starts_with('-') => {
                    self.block(format!(
                        "env {option}: the guard does not know this wrapper option, so it cannot tell which command runs; write the command without it"
                    ));
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

    fn shell(
        &mut self,
        args: &[Word],
        stdin: &Stdin,
        scope: &mut Scope,
        env: &[(String, Option<String>)],
        depth: usize,
    ) {
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
                Some(Some(script)) => self.child_script(script, scope, env, depth),
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
            if let Some(script) = self.script_file(operands.first(), scope) {
                self.child_script(&script, scope, env, depth);
            }
            return;
        }
        match self.stdin_text(stdin, scope) {
            Text::Known(script) => self.child_script(&script, scope, env, depth),
            Text::Absent => (),
            Text::Unknown | Text::Unreadable(_) => self.block(
                "the guard cannot read a script piped to a shell's standard input: run the commands directly",
            ),
        }
    }

    /// The text of a script file a shell or `source` runs; blocks when the
    /// guard cannot read it.
    fn script_file(&mut self, operand: Option<&Word>, scope: &Scope) -> Option<String> {
        let operand = operand?;
        let Some(path) = operand.literal() else {
            self.block("write the script file a shell runs literally so the guard can read it");
            return None;
        };
        match self.read_relative(scope.cwd.as_deref(), path) {
            Text::Known(script) => Some(script),
            Text::Unreadable(fix) => {
                self.block(fix);
                None
            }
            Text::Absent | Text::Unknown => {
                self.block(format!(
                    "could not read {path}: the guard cannot tell which directory the shell is in; run the script with a literal path from the repository"
                ));
                None
            }
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

/// Commands that print, search or describe their arguments and never run
/// them, so `echo git push origin main` is not read as a push.
const NOT_RUNNERS: &[&str] = &[
    ":", "[", "apropos", "echo", "egrep", "false", "fgrep", "grep", "help", "info", "man",
    "printf", "rg", "test", "true", "type", "whatis", "whereis", "which",
];

/// Whether a word names the `git` or `gh` program, by any path.
fn is_git_or_gh(text: &str) -> bool {
    let base = text.rsplit(['/', '\\']).next().unwrap_or(text);
    let base = base.strip_suffix(".exe").unwrap_or(base);
    matches!(base, "git" | "gh")
}

/// Whether a command line given as one word names `git` or `gh` as a word.
fn names_git_or_gh(text: &str) -> bool {
    text.split(|ch: char| ch.is_whitespace() || ";&|()`".contains(ch))
        .any(is_git_or_gh)
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

/// A command that runs another command given after its options.
struct Wrapper {
    name: &'static str,
    /// Options that take no value.
    switches: &'static [&'static str],
    /// Options whose value is attached (`-n5`, `--user=me`) or the next word.
    valued: &'static [&'static str],
    /// Options that change the directory the command runs in.
    chdir: &'static [&'static str],
    /// Operands before the command, such as `timeout`'s duration.
    operands: usize,
    /// Whether `-<digits>` is an option, as in `nice -5`.
    numeric: bool,
    /// A shell keyword: assignments may follow it, as after `time`.
    keyword: bool,
}

const fn wrapper(
    name: &'static str,
    switches: &'static [&'static str],
    valued: &'static [&'static str],
) -> Wrapper {
    Wrapper {
        name,
        switches,
        valued,
        chdir: &[],
        operands: 0,
        numeric: false,
        keyword: false,
    }
}

/// The wrappers the guard reads through, with every option it knows. Any
/// other option is blocked, rather than guessing which word is the command.
const WRAPPERS: &[Wrapper] = &[
    wrapper("builtin", &[], &[]),
    wrapper("nohup", &[], &[]),
    wrapper(
        "setsid",
        &["-c", "-f", "-w", "--ctty", "--fork", "--wait"],
        &[],
    ),
    wrapper("command", &["-p"], &[]),
    wrapper("exec", &["-c", "-l"], &["-a"]),
    wrapper("caffeinate", &["-d", "-i", "-m", "-s", "-u"], &["-t", "-w"]),
    wrapper(
        "stdbuf",
        &[],
        &["-i", "-o", "-e", "--input", "--output", "--error"],
    ),
    Wrapper {
        keyword: true,
        ..wrapper(
            "time",
            &[
                "-p",
                "-a",
                "-v",
                "-q",
                "--append",
                "--verbose",
                "--quiet",
                "--portability",
            ],
            &["-o", "-f", "--output", "--format"],
        )
    },
    Wrapper {
        numeric: true,
        ..wrapper("nice", &[], &["-n", "--adjustment"])
    },
    Wrapper {
        operands: 1,
        ..wrapper(
            "timeout",
            &["-v", "--verbose", "--preserve-status", "--foreground"],
            &["-s", "-k", "--signal", "--kill-after"],
        )
    },
    Wrapper {
        chdir: &["-D", "--chdir"],
        ..wrapper(
            "sudo",
            &[
                "-A",
                "-b",
                "-B",
                "-E",
                "-H",
                "-i",
                "-k",
                "-n",
                "-P",
                "-s",
                "-S",
                "--askpass",
                "--background",
                "--bell",
                "--preserve-env",
                "--set-home",
                "--login",
                "--reset-timestamp",
                "--non-interactive",
                "--preserve-groups",
                "--shell",
                "--stdin",
            ],
            &[
                "-u",
                "-g",
                "-h",
                "-p",
                "-C",
                "-D",
                "-U",
                "-T",
                "-r",
                "-t",
                "-R",
                "--user",
                "--group",
                "--host",
                "--prompt",
                "--close-from",
                "--chdir",
                "--other-user",
                "--command-timeout",
                "--role",
                "--type",
                "--chroot",
            ],
        )
    },
    Wrapper {
        chdir: &["-C"],
        ..wrapper("doas", &["-n", "-s"], &["-u", "-C"])
    },
    wrapper(
        "xargs",
        &[
            "-0",
            "-o",
            "-p",
            "-r",
            "-t",
            "-x",
            "--null",
            "--open-tty",
            "--interactive",
            "--no-run-if-empty",
            "--verbose",
            "--exit",
        ],
        &[
            "-n",
            "-I",
            "-L",
            "-P",
            "-s",
            "-E",
            "-d",
            "-a",
            "--max-args",
            "--max-lines",
            "--max-procs",
            "--max-chars",
            "--delimiter",
            "--arg-file",
            "--eof",
            "--replace",
            "--process-slot-var",
        ],
    ),
];

impl Wrapper {
    /// The options before the command, with their values, and the words
    /// from the command on; or the fix for an option it does not know.
    fn options<'w>(&self, args: &'w [Word]) -> Result<Unwrapped<'w>, String> {
        let unknown = |option: &str| {
            format!(
                "{} {option}: the guard does not know this wrapper option, so it cannot tell which command runs; write the command without it",
                self.name
            )
        };
        let mut found = Vec::new();
        let mut index = 0;
        while let Some(word) = args.get(index) {
            let Some(option) = word.literal() else {
                if word_is_option(word) {
                    return Err(format!(
                        "write the options of {} literally so the guard can tell which command runs",
                        self.name
                    ));
                }
                break;
            };
            if option == "--" {
                index += 1;
                break;
            }
            if option.len() < 2 || !option.starts_with('-') {
                break;
            }
            index += 1;
            if let Some(long) = option.strip_prefix("--") {
                let (name, attached) = match long.split_once('=') {
                    Some((name, attached)) => (name, Some(attached)),
                    None => (long, None),
                };
                let name = option.get(..name.len() + 2).unwrap_or(option);
                if self.valued.contains(&name) {
                    let value = attached.or_else(|| {
                        index += 1;
                        args.get(index - 1).and_then(Word::literal)
                    });
                    found.push((name, value));
                } else if self.switches.contains(&name) {
                    found.push((name, None));
                } else {
                    return Err(unknown(option));
                }
                continue;
            }
            let cluster = option.get(1..).unwrap_or("");
            if self.numeric && cluster.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            for (offset, flag) in cluster.char_indices() {
                let flag = format!("-{flag}");
                if let Some(known) = self.valued.iter().find(|known| **known == flag) {
                    let tail = cluster.get(offset + flag.len() - 1..).unwrap_or("");
                    let value = if tail.is_empty() {
                        index += 1;
                        args.get(index - 1).and_then(Word::literal)
                    } else {
                        Some(tail)
                    };
                    found.push((*known, value));
                    break;
                } else if let Some(known) = self.switches.iter().find(|known| **known == flag) {
                    found.push((*known, None));
                } else {
                    return Err(unknown(option));
                }
            }
        }
        Ok((found, args.get(index..).unwrap_or(&[])))
    }
}

/// A wrapper's options with their values, and the words from its command on.
type Unwrapped<'w> = (Vec<(&'w str, Option<&'w str>)>, &'w [Word]);

/// Whether a word that expands at run time starts like an option.
fn word_is_option(word: &Word) -> bool {
    matches!(word, Word::Dynamic(prefix) if prefix.starts_with('-'))
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
