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
    "fast-import",
    "fetch",
    "filter-branch",
    "for-each-ref",
    "format-patch",
    "fsck",
    "gc",
    "grep",
    "hash-object",
    "help",
    "http-push",
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
    "replay",
    "rm",
    "send-pack",
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
        // Options inherited from the git that runs a shell alias.
        for pair in scope.git_config.chunks(2) {
            if let [flag, value] = pair {
                if let Some((name, expansion)) = value
                    .strip_prefix("alias.")
                    .and_then(|alias| alias.split_once('='))
                {
                    aliases.push((name.to_owned(), expansion.to_owned()));
                }
                options.extend([flag.clone(), value.clone()]);
            }
        }
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
            if inline.is_none() && scope.config_changed {
                self.block(format!(
                    "git {name} may be an alias written earlier in this command: run git config in a separate command first, so the guard can read the alias"
                ));
                return;
            }
            let found = match (inline, place) {
                (Some(found), _) => found,
                (None, Ok(place)) => self.repo.git_alias(place, name),
                (None, Err(error)) => Err(error.clone()),
            };
            match found {
                Ok(Some(expansion)) => {
                    if let Some(script) = expansion.strip_prefix('!') {
                        self.shell_alias(script, scope, place, depth);
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
            "pull" => {
                self.fetch_refspecs(rest, "pull");
                if !has(&["--ff-only"]) {
                    self.not_on_main(place, scope, "pull");
                }
            }
            "fetch" => self.fetch_refspecs(rest, "fetch"),
            "config" if writes_config(rest) => scope.config_changed = true,
            "rebase" => self.rebase(rest, scope, place),
            "reset" => self.reset(rest, scope, place),
            "update-ref" => self.update_ref(rest, scope, place),
            "branch" => self.branch(rest),
            "checkout" | "switch" => {
                self.force_create(rest);
                // `git checkout <tree> -- <paths>` restores files and stays on
                // the branch; a `--` with no path after it still switches.
                let paths = rest
                    .iter()
                    .position(|word| word.literal() == Some("--"))
                    .is_some_and(|end| end + 1 < rest.len());
                if name == "switch" || !paths {
                    scope.branch_changed = true;
                }
            }
            "fast-import" | "filter-branch" | "replay" | "send-pack" | "http-push" => {
                self.block(format!(
                    "git {name} can rewrite or update main without the guard seeing which refs it changes: use git push, git branch or git update-ref with literal refs"
                ));
            }
            _ => (),
        }
    }

    /// A `!` alias: git runs it with a shell in the repository it resolved,
    /// with `GIT_DIR`, `GIT_WORK_TREE` and its `-c` options passed on.
    fn shell_alias(
        &mut self,
        script: &str,
        scope: &mut Scope,
        place: &Result<Place, String>,
        depth: usize,
    ) {
        let place = match place {
            Ok(place) => place,
            Err(error) => {
                self.block(format!(
                    "a git shell alias: {error}; run git from the repository with literal paths"
                ));
                return;
            }
        };
        let mut child = scope.clone();
        child.cwd = Some(place.dir.clone());
        child.env = place
            .env
            .iter()
            .map(|(name, value)| (name.clone(), Some(value.clone())))
            .collect();
        child.git_config = Vec::new();
        let mut options = place.options.iter();
        while let Some(option) = options.next() {
            if option == "-c" {
                if let Some(value) = options.next() {
                    child.git_config.extend([option.clone(), value.clone()]);
                }
            } else if let Some(dir) = option.strip_prefix("--git-dir=") {
                child.env.push(("GIT_DIR".to_owned(), Some(dir.to_owned())));
            } else if let Some(tree) = option.strip_prefix("--work-tree=") {
                child
                    .env
                    .push(("GIT_WORK_TREE".to_owned(), Some(tree.to_owned())));
            }
        }
        self.script(script, &mut child, depth + 1);
        scope.branch_changed |= child.branch_changed;
        scope.config_changed |= child.config_changed;
    }

    /// `git pull` and `git fetch` refspecs: none may write `main`.
    fn fetch_refspecs(&mut self, rest: &[Word], what: &str) {
        let has = |flags: &[&str]| {
            rest.iter()
                .any(|word| word.literal().is_some_and(|text| flags.contains(&text)))
        };
        if has(&["--stdin"]) {
            self.block(format!(
                "git {what} --stdin: the guard cannot read refspecs that could write main; give them as arguments"
            ));
            return;
        }
        if has(&["--all", "--multiple"]) {
            return;
        }
        let words = positionals(rest, FETCH_VALUES);
        for refspec in words.get(1..).unwrap_or(&[]) {
            let Some(text) = refspec.literal() else {
                self.block(format!(
                    "write the git {what} refspec literally so the guard can check that it does not write main"
                ));
                continue;
            };
            let text = text.strip_prefix('+').unwrap_or(text);
            if let Some((_, destination)) = text.split_once(':') {
                if destination.contains('*') {
                    self.block(format!(
                        "git {what} with a pattern destination can write main: fetch into refs/remotes/ or name the branch"
                    ));
                } else if names_main(destination) {
                    self.block(format!(
                        "this git {what} writes main: fetch into refs/remotes/; main changes only through an approved squash merge (ADR 0002, ADR 0003)"
                    ));
                }
            }
        }
    }

    fn rebase(&mut self, rest: &[Word], scope: &Scope, place: &Result<Place, String>) {
        let has = |flags: &[&str]| {
            rest.iter()
                .any(|word| word.literal().is_some_and(|text| flags.contains(&text)))
        };
        if has(&["--abort", "--quit", "--show-current-patch", "--edit-todo"]) {
            return;
        }
        let words = positionals(rest, REBASE_VALUES);
        let branch = words.get(usize::from(!has(&["--root"])));
        match branch.map(|word| word.literal()) {
            Some(Some(branch)) if names_main(branch) => self.block(
                "this git rebase rewrites main: rebase your task branch; main changes only through an approved squash merge (ADR 0002, ADR 0003)",
            ),
            Some(Some(_)) => (),
            Some(None) => self.block(
                "write the branch given to git rebase literally so the guard can check that it is not main",
            ),
            None => self.not_on_main(place, scope, "rebase"),
        }
    }

    fn reset(&mut self, rest: &[Word], scope: &Scope, place: &Result<Place, String>) {
        let before_paths = rest
            .iter()
            .position(|word| word.literal() == Some("--"))
            .map_or(rest, |end| rest.get(..end).unwrap_or(rest));
        let moves = positionals(before_paths, &[]).iter().any(|word| {
            !matches!(
                word.literal(),
                Some(
                    "HEAD"
                        | "@"
                        | "origin/main"
                        | "refs/remotes/origin/main"
                        | "@{u}"
                        | "@{upstream}"
                )
            )
        });
        if moves {
            self.not_on_main(place, scope, "reset");
        }
    }

    fn update_ref(&mut self, rest: &[Word], scope: &Scope, place: &Result<Place, String>) {
        let has = |flags: &[&str]| {
            rest.iter()
                .any(|word| word.literal().is_some_and(|text| flags.contains(&text)))
        };
        if has(&["--stdin"]) {
            self.block(
                "git update-ref --stdin: the guard cannot read refs that could include main; give the ref as an argument",
            );
            return;
        }
        match positionals(rest, &["-m"]).first().map(|word| word.literal()) {
            Some(Some(reference)) if names_main(reference) => self.block(
                "this git update-ref writes main: main changes only through an approved squash merge (ADR 0002, ADR 0003)",
            ),
            Some(Some("HEAD")) if !has(&["--no-deref"]) => {
                self.not_on_main(place, scope, "update-ref HEAD");
            }
            Some(None) => self.block(
                "write the ref given to git update-ref literally so the guard can check that it is not main",
            ),
            _ => (),
        }
    }

    /// `git branch` with force, move, copy or delete may not touch `main`.
    fn branch(&mut self, rest: &[Word]) {
        let changes = rest.iter().filter_map(Word::literal).any(|text| {
            matches!(text, "--force" | "--move" | "--copy" | "--delete")
                || (text.starts_with('-')
                    && !text.starts_with("--")
                    && text.contains(['f', 'm', 'M', 'c', 'C', 'd', 'D']))
        });
        if !changes {
            return;
        }
        for word in positionals(rest, BRANCH_VALUES) {
            match word.literal() {
                Some(name) if names_main(name) => {
                    self.block(
                        "this git branch forces, moves, copies or deletes main: main changes only through an approved squash merge (ADR 0002, ADR 0003)",
                    );
                    return;
                }
                Some(_) => (),
                None => {
                    self.block(
                        "write the branch names given to git branch literally so the guard can check that they are not main",
                    );
                    return;
                }
            }
        }
    }

    /// `git checkout -B` and `git switch -C` reset the branch they name.
    fn force_create(&mut self, rest: &[Word]) {
        let mut index = 0;
        while let Some(word) = rest.get(index) {
            index += 1;
            let Some(text) = word.literal() else {
                continue;
            };
            if text == "--" {
                return;
            }
            let value = match text {
                "-B" | "-C" | "--force-create" => {
                    index += 1;
                    rest.get(index - 1).map(Word::literal)
                }
                _ => text
                    .strip_prefix("--force-create=")
                    .or_else(|| text.strip_prefix("-B").filter(|tail| !tail.is_empty()))
                    .or_else(|| text.strip_prefix("-C").filter(|tail| !tail.is_empty()))
                    .map(Some),
            };
            match value {
                Some(Some(name)) if names_main(name) => self.block(
                    "this git checkout -B or switch -C resets main: main changes only through an approved squash merge (ADR 0002, ADR 0003)",
                ),
                Some(None) => self.block(
                    "write the branch given to -B or -C literally so the guard can check that it is not main",
                ),
                _ => (),
            }
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

/// Options of `git fetch` and `git pull` whose value is the next word.
const FETCH_VALUES: &[&str] = &[
    "--depth",
    "--deepen",
    "--shallow-since",
    "--shallow-exclude",
    "--refmap",
    "-j",
    "--jobs",
    "--upload-pack",
    "-o",
    "--server-option",
    "--negotiation-tip",
    "--recurse-submodules-default",
    "-s",
    "--strategy",
    "-X",
    "--strategy-option",
];

/// Options of `git rebase` whose value is the next word.
const REBASE_VALUES: &[&str] = &[
    "--onto",
    "-s",
    "--strategy",
    "-X",
    "--strategy-option",
    "-x",
    "--exec",
    "-C",
];

/// Options of `git branch` whose value is the next word.
const BRANCH_VALUES: &[&str] = &[
    "-u",
    "--set-upstream-to",
    "--contains",
    "--no-contains",
    "--merged",
    "--no-merged",
    "--points-at",
    "--sort",
    "--format",
];

/// The words after the options, skipping the value of each option in
/// `with_value`; words after `--` are kept.
fn positionals<'w>(rest: &'w [Word], with_value: &[&str]) -> Vec<&'w Word> {
    let mut found = Vec::new();
    let mut options_done = false;
    let mut index = 0;
    while let Some(word) = rest.get(index) {
        index += 1;
        match word.literal() {
            Some("--") if !options_done => options_done = true,
            Some(text) if !options_done && text.len() > 1 && text.starts_with('-') => {
                if with_value.contains(&text) {
                    index += 1;
                }
            }
            _ => found.push(word),
        }
    }
    found
}

/// Whether a `git config` command may write configuration: anything but a
/// lookup or a listing.
fn writes_config(rest: &[Word]) -> bool {
    let reads = rest.iter().filter_map(Word::literal).any(|text| {
        matches!(
            text,
            "--get"
                | "--get-all"
                | "--get-regexp"
                | "--get-urlmatch"
                | "--get-color"
                | "--get-colorbool"
                | "-l"
                | "--list"
        )
    });
    let words = positionals(rest, &["--file", "-f", "--blob", "--type", "--default"]);
    let first = words.first().and_then(|word| word.literal());
    let writes = rest.iter().filter_map(Word::literal).any(|text| {
        matches!(
            text,
            "--add"
                | "--unset"
                | "--unset-all"
                | "--replace-all"
                | "--rename-section"
                | "--remove-section"
                | "-e"
                | "--edit"
        )
    });
    if writes {
        return true;
    }
    if reads || matches!(first, Some("get" | "list")) {
        return false;
    }
    words.len() != 1
}

/// Whether a ref or refspec side names the branch `main`.
fn names_main(reference: &str) -> bool {
    let reference = reference.strip_prefix('+').unwrap_or(reference);
    reference.strip_prefix("refs/heads/").unwrap_or(reference) == "main"
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
