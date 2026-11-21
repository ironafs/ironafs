// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The guard's rules for `git`: no commit or push on `main`, and every
//! commit message ends with the branch-commit trailers (ADR 0003).

use super::shell::{self, Item, Stdin, Word};
use super::{Guard, Place, Repo, Scope, Text, normalize};
use crate::trailers::{self, Rules};

/// Git's own commands. Git never lets an alias replace one of these, so
/// only other names are looked up as aliases.
const BUILTINS: &[&str] = &[
    "add",
    "am",
    "apply",
    "archive",
    "bisect",
    "blame",
    "branch",
    "bundle",
    "cat-file",
    "check-ignore",
    "checkout",
    "cherry",
    "cherry-pick",
    "clean",
    "clone",
    "commit",
    "commit-tree",
    "config",
    "count-objects",
    "describe",
    "diff",
    "diff-tree",
    "fetch",
    "for-each-ref",
    "format-patch",
    "fsck",
    "gc",
    "grep",
    "hash-object",
    "help",
    "init",
    "interpret-trailers",
    "log",
    "ls-files",
    "ls-remote",
    "ls-tree",
    "merge",
    "merge-base",
    "mktree",
    "mv",
    "name-rev",
    "notes",
    "pull",
    "push",
    "range-diff",
    "read-tree",
    "rebase",
    "reflog",
    "remote",
    "reset",
    "restore",
    "rev-list",
    "rev-parse",
    "revert",
    "rm",
    "shortlog",
    "show",
    "show-ref",
    "sparse-checkout",
    "stash",
    "status",
    "submodule",
    "switch",
    "symbolic-ref",
    "tag",
    "update-ref",
    "var",
    "verify-commit",
    "version",
    "worktree",
    "write-tree",
];

/// Global options whose value is the next word.
const VALUE_OPTIONS: &[&str] = &[
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--super-prefix",
    "--config-env",
];

const NEEDS_MESSAGE: &str = "give the commit message with -m, -F <file> or a here-document, so the guard can check its Assisted-by: trailer";

impl<R: Repo> Guard<'_, R> {
    pub(super) fn git(
        &mut self,
        args: &[Word],
        stdin: &Stdin,
        scope: &mut Scope,
        env: &[(String, Option<String>)],
        depth: usize,
    ) {
        let mut dir = scope.cwd.clone();
        let mut options = Vec::new();
        let mut known = true;
        let mut aliases = Vec::new();
        let mut index = 0;
        let subcommand = loop {
            let Some(word) = args.get(index) else {
                return;
            };
            let Some(text) = word.literal() else {
                self.block("write the git subcommand literally so the guard can read it");
                return;
            };
            index += 1;
            if text == "-C" {
                dir = match (dir, args.get(index).and_then(Word::literal)) {
                    (Some(dir), Some(path)) => Some(normalize(&dir.join(path))),
                    _ => None,
                };
                index += 1;
            } else if text == "-c" || VALUE_OPTIONS.contains(&text) {
                match args.get(index).and_then(Word::literal) {
                    Some(value) => {
                        if text == "-c"
                            && let Some((name, expansion)) = value
                                .strip_prefix("alias.")
                                .and_then(|alias| alias.split_once('='))
                        {
                            aliases.push((name.to_owned(), expansion.to_owned()));
                        }
                        if text == "-c" {
                            options.extend(["-c".to_owned(), value.to_owned()]);
                        } else {
                            options.push(format!("{text}={value}"));
                        }
                    }
                    None => known = false,
                }
                index += 1;
            } else if text.starts_with('-') {
                options.push(text.to_owned());
            } else {
                break text;
            }
        };
        let env: Option<Vec<(String, String)>> = env
            .iter()
            .map(|(name, value)| value.clone().map(|value| (name.clone(), value)))
            .collect();
        let place = match (dir, env) {
            (Some(dir), Some(env)) if known => Ok(Place { dir, options, env }),
            _ => Err("the guard cannot tell which repository this git command uses".to_owned()),
        };
        let rest = args.get(index..).unwrap_or(&[]);
        self.git_subcommand(subcommand, rest, stdin, scope, &place, &aliases, depth);
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the repository context of one git invocation, passed through alias expansion"
    )]
    fn git_subcommand(
        &mut self,
        name: &str,
        rest: &[Word],
        stdin: &Stdin,
        scope: &mut Scope,
        place: &Result<Place, String>,
        aliases: &[(String, String)],
        depth: usize,
    ) {
        if self.too_deep(depth) {
            return;
        }
        if !BUILTINS.contains(&name) {
            let inline = aliases
                .iter()
                .find(|(alias, _)| alias == name)
                .map(|(_, expansion)| Ok(Some(expansion.clone())));
            let found = match (inline, place) {
                (Some(found), _) => found,
                (None, Ok(place)) => self.repo.git_alias(place, name),
                (None, Err(error)) => Err(error.clone()),
            };
            match found {
                Ok(Some(expansion)) => {
                    if let Some(script) = expansion.strip_prefix('!') {
                        self.child_script(script, scope, depth);
                    } else if let Some(words) = self.alias_words(&expansion) {
                        let mut words = words.into_iter();
                        if let Some(Some(first)) = words.next().map(|word| word.literal().map(str::to_owned)) {
                            let expanded: Vec<Word> = words.chain(rest.iter().cloned()).collect();
                            self.git_subcommand(&first, &expanded, stdin, scope, place, aliases, depth + 1);
                        } else {
                            self.block(format!("could not read the git alias {name}: run the command it stands for"));
                        }
                    }
                }
                Ok(None) => (),
                Err(error) => self.block(format!(
                    "could not look up whether git {name} is an alias ({error}): run git from the repository with literal paths"
                )),
            }
            return;
        }
        let has = |flags: &[&str]| {
            rest.iter()
                .any(|word| word.literal().is_some_and(|text| flags.contains(&text)))
        };
        match name {
            "commit" => self.commit(rest, stdin, scope, place),
            "merge" if !has(&["--ff-only", "--abort", "--quit"]) => {
                self.not_on_main(place, scope, "merge");
            }
            "cherry-pick" | "revert"
                if !has(&["--abort", "--quit", "--skip", "-n", "--no-commit"]) =>
            {
                self.not_on_main(place, scope, name);
            }
            "am" if !has(&["--abort", "--quit", "--skip", "--show-current-patch"]) => {
                self.not_on_main(place, scope, "am");
            }
            "push" => self.push(rest, scope, place),
            "checkout" | "switch" if !has(&["--"]) => scope.branch_changed = true,
            _ => (),
        }
    }

    /// The words of an alias that expands to a single command.
    pub(super) fn alias_words(&mut self, expansion: &str) -> Option<Vec<Word>> {
        if let Ok([Item::Command(command)]) = shell::parse(expansion).as_deref() {
            Some(command.words.clone())
        } else {
            self.block(format!(
                "could not read the alias \"{expansion}\": run the command it stands for"
            ));
            None
        }
    }

    /// The current branch of `place`, blocking `what` if it cannot be known.
    fn current_branch(
        &mut self,
        place: &Result<Place, String>,
        scope: &Scope,
        what: &str,
    ) -> Result<Option<String>, ()> {
        if scope.branch_changed {
            self.block(format!(
                "{what}: switch branches in a separate command first, so the guard can see which branch it runs on"
            ));
            return Err(());
        }
        let branch = match place {
            Ok(place) => self.repo.branch(place),
            Err(error) => Err(error.clone()),
        };
        branch.map_err(|error| {
            self.block(format!(
                "{what}: could not read the current branch ({error}); run it in the repository, with a literal path"
            ));
        })
    }

    fn not_on_main(&mut self, place: &Result<Place, String>, scope: &Scope, what: &str) {
        if let Ok(Some(branch)) = self.current_branch(place, scope, what)
            && branch == "main"
        {
            self.block(format!(
                "this git {what} is on main: work on a task branch (git switch -c <branch>); main changes only through an approved squash merge (ADR 0002, ADR 0003)"
            ));
        }
    }

    fn commit(
        &mut self,
        rest: &[Word],
        stdin: &Stdin,
        scope: &Scope,
        place: &Result<Place, String>,
    ) {
        let mut messages = Vec::new();
        let mut file = None;
        let mut reused = false;
        let mut trailers = Vec::new();
        let mut index = 0;
        while let Some(word) = rest.get(index) {
            index += 1;
            let Some(text) = word.literal() else {
                continue;
            };
            let mut value = |attached: Option<&str>| {
                if let Some(attached) = attached {
                    Some(attached.to_owned())
                } else {
                    let next = rest.get(index).and_then(Word::literal).map(str::to_owned);
                    index += 1;
                    next
                }
            };
            if text == "--" {
                break;
            } else if let Some(long) = text.strip_prefix("--") {
                let (name, attached) = match long.split_once('=') {
                    Some((name, attached)) => (name, Some(attached)),
                    None => (long, None),
                };
                match name {
                    "dry-run" => return,
                    "message" => messages.push(value(attached)),
                    "file" => file = Some(value(attached)),
                    "trailer" => trailers.push(value(attached)),
                    "reuse-message" | "reedit-message" | "fixup" | "squash" | "template" => {
                        value(attached);
                        reused = true;
                    }
                    "author" | "date" | "cleanup" | "pathspec-from-file" if attached.is_none() => {
                        value(None);
                    }
                    _ => (),
                }
            } else if let Some(cluster) = text.strip_prefix('-') {
                for (offset, flag) in cluster.char_indices() {
                    let tail = cluster.get(offset + flag.len_utf8()..).unwrap_or("");
                    let attached = (!tail.is_empty()).then_some(tail);
                    match flag {
                        'm' => messages.push(value(attached)),
                        'F' => file = Some(value(attached)),
                        'C' | 'c' | 't' => {
                            value(attached);
                            reused = true;
                        }
                        'S' | 'u' => (),
                        _ => continue,
                    }
                    break;
                }
            }
        }

        self.not_on_main(place, scope, "commit");
        let base = place.as_ref().ok().map(|place| place.dir.as_path());
        let message = if reused {
            Text::Unknown
        } else if !messages.is_empty() {
            messages
                .into_iter()
                .collect::<Option<Vec<String>>>()
                .map_or(Text::Unknown, |parts| Text::Known(parts.join("\n\n")))
        } else {
            match file {
                Some(Some(path)) if path == "-" => self.stdin_text(stdin, scope),
                Some(Some(path)) => self.read_relative(base, &path),
                Some(None) | None => Text::Unknown,
            }
        };
        let trailers: Option<Vec<String>> = trailers.into_iter().collect();
        match (message, trailers) {
            (Text::Known(message), Some(trailers)) => {
                if let Err(violation) =
                    trailers::validate(&with_trailers(&message, &trailers), Rules::Commit)
                {
                    self.block(format!(
                        "commit message, {violation}. A branch commit ends with Assisted-by: <agent>:<model>, or Assisted-by: none, and no Signed-off-by:, Class: or AI-Reviewed-by: (ADR 0003)"
                    ));
                }
            }
            (Text::Unreadable(fix), _) => self.block(fix),
            _ => self.block(NEEDS_MESSAGE),
        }
    }

    fn push(&mut self, rest: &[Word], scope: &Scope, place: &Result<Place, String>) {
        let mut positionals = Vec::new();
        let mut every = None;
        let mut delete = false;
        let mut options_done = false;
        let mut index = 0;
        while let Some(word) = rest.get(index) {
            index += 1;
            let text = match word.literal() {
                Some(text) if !options_done => text,
                _ => {
                    positionals.push(word);
                    continue;
                }
            };
            match text {
                "--" => options_done = true,
                "--all" | "--mirror" | "--branches" => every = Some(text),
                "--delete" => delete = true,
                "--dry-run" => return,
                "--repo" | "--push-option" | "--receive-pack" | "--exec" => index += 1,
                _ if text.starts_with("--") => (),
                _ if text.len() > 1 && text.starts_with('-') => {
                    for (offset, flag) in text.char_indices().skip(1) {
                        match flag {
                            'd' => delete = true,
                            'n' => return,
                            'o' => {
                                if text.len() == offset + 1 {
                                    index += 1;
                                }
                                break;
                            }
                            _ => (),
                        }
                    }
                }
                _ => positionals.push(word),
            }
        }
        if let Some(flag) = every {
            self.block(format!(
                "git push {flag} pushes every branch, main included: push your task branch by name"
            ));
            return;
        }
        let refspecs = positionals.get(1..).unwrap_or(&[]);
        if refspecs.is_empty() {
            if let Ok(Some(branch)) = self.current_branch(place, scope, "git push") {
                let target = match place {
                    Ok(place) => self.repo.push_target(place, &branch),
                    Err(error) => Err(error.clone()),
                };
                match target {
                    Ok(target) => self.push_destination(&target.unwrap_or(branch)),
                    Err(error) => self.block(format!(
                        "git push: could not read where this branch pushes ({error}); name the remote and branch"
                    )),
                }
            }
            return;
        }
        for refspec in refspecs {
            let Some(text) = refspec.literal() else {
                self.block("write the git push refspec literally so the guard can read it");
                continue;
            };
            let text = text.strip_prefix('+').unwrap_or(text);
            if text.contains('*') || text == ":" {
                self.block(
                    "git push with a pattern or a matching refspec (:) can update main: push your task branch by name",
                );
                continue;
            }
            let source_only = !delete && !text.contains(':');
            if source_only && matches!(text, "HEAD" | "@") {
                if let Ok(Some(branch)) = self.current_branch(place, scope, "git push") {
                    self.push_destination(&branch);
                }
                continue;
            }
            let destination = match text.split_once(':') {
                Some((_, destination)) if !delete => destination,
                _ => text,
            };
            self.push_destination(destination);
        }
    }

    fn push_destination(&mut self, destination: &str) {
        let branch = destination
            .strip_prefix("refs/heads/")
            .unwrap_or(destination);
        if branch == "main" {
            self.block(
                "this git push pushes to main: push your task branch (git push -u origin <branch>); main changes only through an approved squash merge (ADR 0002, ADR 0003)",
            );
        }
    }
}

/// The message as `git commit --trailer` leaves it: the trailers join the
/// final paragraph when it is already a trailer block, or start one.
fn with_trailers(message: &str, trailers: &[String]) -> String {
    let body = message.trim_end();
    if trailers.is_empty() {
        return body.to_owned();
    }
    let last = body.rsplit("\n\n").next().unwrap_or(body);
    let is_block = body.contains("\n\n")
        && last.lines().all(|line| {
            line.split_once(':')
                .is_some_and(|(key, _)| !key.is_empty() && !key.contains(' '))
        });
    let added: Vec<String> = trailers
        .iter()
        .map(|trailer| match trailer.split_once('=') {
            Some((key, value)) if !trailer.contains(':') => format!("{key}: {value}"),
            _ => trailer.clone(),
        })
        .collect();
    let separator = if is_block { "\n" } else { "\n\n" };
    format!("{body}{separator}{}", added.join("\n"))
}
