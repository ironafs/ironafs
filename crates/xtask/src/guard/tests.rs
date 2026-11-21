// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the guard's rules, with allowed and blocked cases and the
//! spellings that try to get past them.

use std::path::{Path, PathBuf};

use super::{Place, Repo, check_input, evaluate};

const FEATURE: &str = "/repo/feature";
const MAIN: &str = "/repo/main";
const COMMIT_OK: &str = "git commit -m 'fix it' -m 'Assisted-by: claude-code:claude-opus-5-5'";
const DISCLOSED: &str = "Body.\n\nWritten by claude-code:claude-opus-5-5 for the maintainer.";

struct Fake {
    push_target: Option<String>,
    fetch_config: Vec<&'static str>,
    git_aliases: Vec<(&'static str, &'static str)>,
    gh_aliases: Vec<(&'static str, &'static str)>,
    files: Vec<(PathBuf, String)>,
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            push_target: None,
            fetch_config: vec!["+refs/heads/*:refs/remotes/origin/*"],
            git_aliases: Vec::new(),
            gh_aliases: Vec::new(),
            files: vec![
                (
                    PathBuf::from(FEATURE).join("msg.txt"),
                    "fix it\n\nAssisted-by: none\n".to_owned(),
                ),
                (
                    PathBuf::from(FEATURE).join("bad.txt"),
                    "fix it\n".to_owned(),
                ),
                (PathBuf::from(FEATURE).join("body.md"), DISCLOSED.to_owned()),
                (
                    PathBuf::from(FEATURE).join("plain.md"),
                    "Body.\n".to_owned(),
                ),
                (
                    PathBuf::from(FEATURE).join("push.sh"),
                    "#!/bin/sh\ngit push origin main\n".to_owned(),
                ),
                (
                    PathBuf::from(FEATURE).join("commit.sh"),
                    format!("{COMMIT_OK}\n"),
                ),
                (
                    PathBuf::from(FEATURE).join("build.sh"),
                    "cargo build\n".to_owned(),
                ),
            ],
        }
    }
}

impl Repo for Fake {
    fn branch(&self, place: &Place) -> Result<Option<String>, String> {
        if place
            .options
            .iter()
            .any(|option| option == "--git-dir=/repo/main/.git")
            || place
                .env
                .iter()
                .any(|(_, value)| value == "/repo/main/.git")
        {
            return Ok(Some("main".to_owned()));
        }
        match place.dir.to_str() {
            Some(FEATURE) => Ok(Some("feature".to_owned())),
            Some(MAIN) => Ok(Some("main".to_owned())),
            Some("/repo/detached") => Ok(None),
            _ => Err("not a git repository".to_owned()),
        }
    }

    fn push_target(&self, _place: &Place, _branch: &str) -> Result<Option<String>, String> {
        Ok(self.push_target.clone())
    }

    fn git_alias(&self, _place: &Place, name: &str) -> Result<Option<String>, String> {
        Ok(lookup(&self.git_aliases, name))
    }

    fn fetch_refspecs(&self, _place: &Place) -> Result<Vec<String>, String> {
        Ok(self
            .fetch_config
            .iter()
            .map(|refspec| (*refspec).to_owned())
            .collect())
    }

    fn gh_alias(&self, name: &str) -> Result<Option<String>, String> {
        Ok(lookup(&self.gh_aliases, name))
    }

    fn read_file(&self, path: &Path) -> Result<String, String> {
        self.files
            .iter()
            .find(|(known, _)| known == path)
            .map(|(_, text)| text.clone())
            .ok_or_else(|| format!("no file {}", path.display()))
    }
}

fn lookup(aliases: &[(&str, &str)], name: &str) -> Option<String> {
    aliases
        .iter()
        .find(|(alias, _)| *alias == name)
        .map(|(_, expansion)| (*expansion).to_owned())
}

#[track_caller]
fn allowed_with(repo: &Fake, command: &str) {
    let fixes = evaluate(command, Path::new(FEATURE), repo);
    assert_eq!(fixes, [] as [String; 0], "{command:?} was blocked");
}

#[track_caller]
fn allowed(command: &str) {
    allowed_with(&Fake::default(), command);
}

#[track_caller]
fn blocked_with(repo: &Fake, command: &str, fix: &str) {
    let fixes = evaluate(command, Path::new(FEATURE), repo);
    assert!(
        fixes.iter().any(|found| found.contains(fix)),
        "{command:?} gave {fixes:?}, not one containing {fix:?}"
    );
}

#[track_caller]
fn blocked(command: &str, fix: &str) {
    blocked_with(&Fake::default(), command, fix);
}

const ON_MAIN: &str = "is on main";
const TO_MAIN: &str = "pushes to main";
const NEEDS_MESSAGE: &str = "give the commit message";
const NEEDS_DISCLOSURE: &str = "disclosure line";

#[test]
fn guard_allows_reading_and_harmless_commands() {
    for command in [
        "git status",
        "git log --oneline -3 main",
        "git diff main...HEAD",
        "git fetch origin && git rebase --help",
        "grep -rn 'git push origin main' docs",
        "echo \"git push origin main\"",
        "cargo build && cargo nextest run --workspace",
        "SHA=$(git rev-parse HEAD); echo $SHA",
        "ls | grep git",
        "gh pr view 85 --comments",
        "gh issue list --state open",
        "gh api repos/o/r/pulls/1",
        "gh api graphql -f query='query { viewer { login } }'",
        "for f in a b; do echo $f; done",
        "git -C /repo/main log -1",
        "git merge-base main HEAD",
        "git switch main",
        "git pull --ff-only",
        "bash build.sh",
        "command -v git",
    ] {
        allowed(command);
    }
}

#[test]
fn guard_allows_a_branch_commit_with_its_assisted_by_trailer() {
    allowed(COMMIT_OK);
    allowed("git commit -am 'fix it' -m 'Assisted-by: none'");
    allowed("git commit -F msg.txt");
    allowed("git commit --file=msg.txt");
    allowed("git commit -F - <<'EOF'\nfix it\n\nAssisted-by: none\nEOF");
    allowed(
        "git commit -m \"$(cat <<'EOF'\nfix it\n\nAssisted-by: claude-code:claude-opus-5-5\nEOF\n)\"",
    );
    allowed("git commit -m 'fix it' --trailer 'Assisted-by: claude-code:claude-opus-5-5'");
    allowed("git -C /repo/detached commit -m 'x' -m 'Assisted-by: none'");
    allowed("git commit --dry-run");
}

#[test]
fn guard_blocks_a_commit_without_a_valid_assisted_by_trailer() {
    blocked("git commit -m 'fix it'", "Assisted-by");
    blocked("git commit -F bad.txt", "Assisted-by");
    blocked(
        "git commit -m 'fix it' -m 'Assisted-by: none\nSigned-off-by: A <a@example.org>'",
        "remove Signed-off-by",
    );
}

#[test]
fn guard_blocks_a_commit_whose_message_it_cannot_read() {
    for command in [
        "git commit",
        "git commit --amend --no-edit",
        "git commit -C HEAD",
        "git commit -m \"$MESSAGE\"",
        "cat msg.txt | git commit -F -",
        "git commit -t template.txt",
    ] {
        blocked(command, NEEDS_MESSAGE);
    }
    blocked("git commit -F missing.txt", "could not read");
}

#[test]
fn guard_blocks_commits_on_main_however_the_directory_is_given() {
    blocked(&COMMIT_OK.replace("git ", "git -C /repo/main "), ON_MAIN);
    blocked(&format!("cd /repo/main && {COMMIT_OK}"), ON_MAIN);
    blocked(&format!("cd .. && cd main && {COMMIT_OK}"), ON_MAIN);
    blocked(
        &COMMIT_OK.replace("git ", "git --git-dir=/repo/main/.git "),
        ON_MAIN,
    );
    blocked(&format!("GIT_DIR=/repo/main/.git {COMMIT_OK}"), ON_MAIN);
    blocked("git -C /repo/main merge feature", ON_MAIN);
    blocked("git -C /repo/main cherry-pick abc123", ON_MAIN);
    blocked("git -C /repo/main revert abc123", ON_MAIN);
    blocked("git -C /repo/main am patch.mbox", ON_MAIN);
    allowed("git -C /repo/main merge --ff-only origin/main");
    allowed("git -C /repo/main merge --abort");
    allowed(&format!("(cd /repo/main && git log); {COMMIT_OK}"));
}

#[test]
fn guard_blocks_porcelain_that_moves_main() {
    for command in [
        "git -C /repo/main pull origin main",
        "git -C /repo/main pull --rebase",
        "git -C /repo/main rebase origin/main",
        "git -C /repo/main rebase --continue",
        "git -C /repo/main reset --hard abc1234",
        "git -C /repo/main reset abc1234",
        "git -C /repo/main update-ref HEAD abc1234",
    ] {
        blocked(command, ON_MAIN);
    }
    for command in [
        "git rebase origin/main main",
        "git rebase --onto abc1234 origin/main refs/heads/main",
        "git update-ref refs/heads/main abc1234",
        "git update-ref -d main",
        "git branch -f main abc1234",
        "git branch --force main",
        "git branch -M main",
        "git branch -m feature main",
        "git branch -C feature main",
        "git checkout -B main abc1234",
        "git switch -C main",
        "git switch --force-create=main abc1234",
        "git fetch origin feature:main",
        "git fetch . +feature:refs/heads/main",
        "git fetch origin 'refs/heads/*:refs/heads/*'",
        "git send-pack origin feature:main",
        "git fast-import",
        "git filter-branch -- --all",
        "git update-ref --stdin",
        "git fetch origin $REFSPEC",
    ] {
        blocked(command, "main");
    }
    for command in [
        "git -C /repo/main pull --ff-only",
        "git -C /repo/main reset --hard origin/main",
        "git -C /repo/main reset -- file.rs",
        "git -C /repo/main reset",
        "git -C /repo/main rebase --abort",
        "git pull origin main",
        "git rebase origin/main",
        "git rebase -i --onto origin/main abc1234",
        "git reset --hard abc1234",
        "git update-ref refs/heads/feature abc1234",
        "git branch -f topic abc1234",
        "git branch -d topic",
        "git fetch origin",
        "git fetch origin main",
        "git fetch origin main:refs/remotes/origin/main",
    ] {
        allowed(command);
    }
}

#[test]
fn guard_checks_the_configured_fetch_refspecs() {
    let heads = Fake {
        fetch_config: vec!["+refs/heads/*:refs/heads/*"],
        ..Fake::default()
    };
    for command in [
        "git fetch",
        "git fetch origin",
        "git fetch --all",
        "git fetch --multiple origin upstream",
        "git fetch origin feature",
        "git pull --ff-only",
        "git remote update",
        "git remote update origin",
    ] {
        blocked_with(&heads, command, "remote.<name>.fetch");
    }
    let main = Fake {
        fetch_config: vec!["refs/heads/main:refs/heads/main"],
        ..Fake::default()
    };
    blocked_with(&main, "git fetch origin", "remote.<name>.fetch");
    for command in [
        "git fetch --refmap='+refs/heads/*:refs/heads/*' origin main",
        "git fetch --refmap '+refs/heads/*:main' origin main",
    ] {
        blocked(command, "main");
    }
    blocked(
        "git config remote.origin.fetch '+refs/heads/*:refs/heads/*' && git fetch origin",
        "separate command",
    );
    for command in [
        "git fetch origin",
        "git fetch --all",
        "git remote update",
        "git remote -v",
        "git fetch origin '+refs/heads/*:refs/remotes/origin/*'",
        "git fetch origin 'refs/tags/*:refs/tags/*'",
    ] {
        allowed(command);
    }
}

#[test]
fn guard_blocks_a_commit_after_a_branch_change_in_the_same_command() {
    blocked(
        &format!("git checkout -b topic && {COMMIT_OK}"),
        "separate command",
    );
    blocked(&format!("git switch main; {COMMIT_OK}"), "separate command");
    blocked(
        &format!("git switch main -- && {COMMIT_OK}"),
        "separate command",
    );
    blocked(
        &format!("git checkout main -- && {COMMIT_OK}"),
        "separate command",
    );
    blocked("git checkout -B main --", "resets main");
    blocked("git switch -C main --", "resets main");
    allowed(&format!("git checkout -- file.rs && {COMMIT_OK}"));
    allowed(&format!("git checkout main -- file.rs && {COMMIT_OK}"));
}

#[test]
fn guard_blocks_a_commit_where_it_cannot_tell_the_branch() {
    blocked(&format!("cd $HOME && {COMMIT_OK}"), "current branch");
    blocked(
        &COMMIT_OK.replace("git ", "git -C /nowhere "),
        "current branch",
    );
}

#[test]
fn guard_allows_pushing_a_task_branch() {
    allowed("git push -u origin feature");
    allowed("git push origin HEAD");
    allowed("git push");
    allowed("git push origin feature:feature --force-with-lease");
    allowed("git push --dry-run origin main");
    allowed("git push origin --tags");
    allowed("git push -o ci.skip origin feature");
}

#[test]
fn guard_blocks_every_spelling_of_a_push_to_main() {
    for command in [
        "git push origin main",
        "git push origin HEAD:main",
        "git push origin +HEAD:refs/heads/main",
        "git push origin @:main",
        "git -C /repo/feature push origin feature:main",
        "git push origin :main",
        "git push --delete origin main",
        "git push -d origin main",
        "git push https://github.com/o/r.git main",
        "git push --repo=origin origin main",
        "git push -fu origin main",
        "git -C /repo/main push",
        "git -C /repo/main push origin HEAD",
        "git push origin -- main",
    ] {
        blocked(command, TO_MAIN);
    }
    blocked("git push --all origin", "--all");
    blocked("git push --mirror origin", "--mirror");
    blocked("git push origin 'refs/heads/*:refs/heads/*'", "pattern");
    blocked("git push origin $BRANCH", "literally");
    blocked("git push origin :", "pattern");
}

#[test]
fn guard_blocks_a_bare_push_whose_configured_target_is_main() {
    let repo = Fake {
        push_target: Some("refs/heads/main".to_owned()),
        ..Fake::default()
    };
    blocked_with(&repo, "git push", TO_MAIN);
    blocked_with(&repo, "git push origin", TO_MAIN);
}

#[test]
fn guard_sees_through_wrappers_shells_and_paths() {
    for command in [
        r"\git push origin main",
        "/usr/bin/git push origin main",
        "git.exe push origin main",
        "env GIT_TRACE=1 git push origin main",
        "command git push origin main",
        "nohup git push origin main",
        "time nice -n 5 git push origin main",
        "timeout 60 git push origin main",
        "sudo -u me git push origin main",
        "bash -c 'git push origin main'",
        "sh -lc \"git push origin main\"",
        "zsh -c 'cd /tmp; git push origin main'",
        "eval git push origin main",
        "eval 'git push origin main'",
        "echo $(git push origin main)",
        "echo `git push origin main`",
        "bash <<'EOF'\ngit push origin main\nEOF",
        "if true; then git push origin main; fi",
        "{ git push origin main; }",
        "! git push origin main",
        "g() { git push origin main; }; g",
        "git -c core.pager=cat --no-pager push origin main",
    ] {
        blocked(command, TO_MAIN);
    }
    blocked("$GIT push origin main", "command name");
    blocked("bash -c \"$SCRIPT\"", "literally");
    blocked("echo git push origin main | bash", "standard input");
}

#[test]
fn guard_sees_assignments_after_shell_keywords() {
    for command in [
        "if GIT_DIR=/repo/main/.git git commit -m 'x' -m 'Assisted-by: none'; then :; fi",
        "! GIT_DIR=/repo/main/.git git commit -m 'x' -m 'Assisted-by: none'",
        "time GIT_DIR=/repo/main/.git git commit -m 'x' -m 'Assisted-by: none'",
    ] {
        blocked(command, ON_MAIN);
    }
}

#[test]
fn guard_blocks_wrappers_it_cannot_read() {
    for command in [
        "sudo FOO=1 git push origin main",
        "sudo --user me git push origin main",
        "sudo --frobnicate git push origin main",
        "nice --adjustment 5 git push origin main",
        "nice --wobble 5 git push origin main",
        "exec FOO=1 git push origin main",
        "nohup FOO=1 git push origin main",
        "timeout --frobnicate 5 git push origin main",
        "time --frobnicate git push origin main",
        "command FOO=1 git push origin main",
        "env --frobnicate git push origin main",
        "xargs --max-args 1 git push origin main",
        "stdbuf -oL FOO=1 git push origin main",
    ] {
        let fixes = evaluate(command, Path::new(FEATURE), &Fake::default());
        assert!(
            fixes
                .iter()
                .any(|fix| fix.contains(TO_MAIN) || fix.contains("wrapper")),
            "{command:?} gave {fixes:?}"
        );
    }
    for command in [
        "sudo --user=me git push origin main",
        "sudo -Eu me git push origin main",
        "nice --adjustment=5 git push origin main",
        "nice -5 git push origin main",
        "timeout --signal=KILL 5 git push origin main",
        "timeout -k 2 5 git push origin main",
        "exec -a name git push origin main",
        "stdbuf -oL git push origin main",
        "stdbuf --output=L git push origin main",
        "setsid git push origin main",
        "caffeinate -i git push origin main",
        "coproc git push origin main",
        "coproc P { git push origin main; }",
        "xargs -0 -n 1 git push origin main",
    ] {
        blocked(command, TO_MAIN);
    }
    blocked(&format!("sudo -D /repo/main {COMMIT_OK}"), ON_MAIN);
    blocked(&format!("env --chdir=/repo/main {COMMIT_OK}"), ON_MAIN);
    blocked(&format!("env -C/repo/main {COMMIT_OK}"), ON_MAIN);
    allowed("env --chdir=/repo/feature cargo test");
    allowed("env --unset=FOO cargo test");
    allowed("env -uFOO cargo test");
    allowed("sudo -u me git status");
    allowed("nice -n 5 cargo build");
    allowed("timeout 60 cargo nextest run");
}

#[test]
fn guard_expands_git_aliases() {
    let repo = Fake {
        git_aliases: vec![("ci", "commit -v"), ("pm", "!git push origin main")],
        ..Fake::default()
    };
    blocked_with(&repo, "git ci -m 'fix it'", "Assisted-by");
    allowed_with(&repo, "git ci -m 'fix it' -m 'Assisted-by: none'");
    blocked_with(&repo, "git pm", TO_MAIN);
    blocked("git -c alias.p=push p origin main", TO_MAIN);
}

#[test]
fn guard_runs_shell_aliases_where_git_runs_them() {
    let repo = Fake {
        git_aliases: vec![("cm", "!git commit -m 'x' -m 'Assisted-by: none'")],
        ..Fake::default()
    };
    blocked_with(&repo, "git -C /repo/main cm", ON_MAIN);
    blocked_with(&repo, "git --git-dir=/repo/main/.git cm", ON_MAIN);
    blocked_with(&repo, "GIT_DIR=/repo/main/.git git cm", ON_MAIN);
    allowed_with(&repo, "git cm");
    blocked(
        "git -c alias.p=push -c 'alias.q=!git p origin main' q",
        TO_MAIN,
    );
}

#[test]
fn guard_blocks_an_alias_defined_earlier_in_the_same_command() {
    for command in [
        "git config alias.pm '!git push origin main' && git pm",
        "git config --global alias.pm push; git pm origin main",
        "git config set alias.pm push; git pm origin main",
        "(git config alias.pm push); git pm origin main",
        "gh alias set pm 'pr merge 5' && gh pm",
        "gh alias import aliases.yml; gh pm",
    ] {
        blocked(command, "separate command");
    }
    allowed("git config --get alias.pm && git pm origin feature");
    allowed("git config user.name; git status");
    allowed("gh alias list && gh copilot explain x");
}

#[test]
fn guard_reads_a_script_a_shell_runs() {
    for command in [
        "bash push.sh",
        "sh -x push.sh",
        "bash -o pipefail push.sh",
        "source push.sh",
        ". ./push.sh",
    ] {
        blocked(command, TO_MAIN);
    }
    blocked("bash missing.sh", "could not read");
    blocked("source missing.sh", "could not read");
    blocked("bash \"$SCRIPT\"", "literally");
    blocked("cd $HOME && bash build.sh", "could not read");
    allowed("bash build.sh");
    allowed(". build.sh");
}

#[test]
fn guard_honours_exported_git_variables() {
    for command in [
        format!("export GIT_DIR=/repo/main/.git; {COMMIT_OK}"),
        format!("declare -x GIT_DIR=/repo/main/.git && {COMMIT_OK}"),
        format!("typeset -x GIT_DIR=/repo/main/.git; {COMMIT_OK}"),
        format!("GIT_DIR=/repo/main/.git; export GIT_DIR; {COMMIT_OK}"),
        format!("export FOO=1 GIT_DIR=/repo/main/.git; {COMMIT_OK}"),
    ] {
        blocked(&command, ON_MAIN);
    }
    blocked(
        &format!("export GIT_DIR=$X; {COMMIT_OK}"),
        "which repository",
    );
    allowed(&format!("(export GIT_DIR=/repo/main/.git); {COMMIT_OK}"));
    allowed(&format!(
        "export GIT_DIR=/repo/main/.git; unset GIT_DIR; {COMMIT_OK}"
    ));
    allowed(&format!("export PATH=/usr/bin; {COMMIT_OK}"));
}

#[test]
fn guard_passes_a_command_s_assignments_to_the_shell_it_runs() {
    for command in [
        format!("GIT_DIR=/repo/main/.git sh -c \"{COMMIT_OK}\""),
        "GIT_DIR=/repo/main/.git bash commit.sh".to_owned(),
        "env GIT_DIR=/repo/main/.git bash commit.sh".to_owned(),
        "GIT_DIR=/repo/main/.git bash < commit.sh".to_owned(),
        format!("GIT_DIR=/repo/main/.git eval \"{COMMIT_OK}\""),
        "GIT_DIR=/repo/main/.git source commit.sh".to_owned(),
    ] {
        blocked(&command, ON_MAIN);
    }
    allowed("bash commit.sh");
    allowed("FOO=1 bash commit.sh");
}

#[test]
fn guard_blocks_unparsable_commands() {
    blocked("echo 'unterminated", "could not parse");
    blocked("case $x in a) git push;; esac", "could not parse");
}

#[test]
fn guard_requires_the_disclosure_line_on_published_text() {
    for command in [
        "gh issue create --title t --body 'Body.'",
        "gh pr create --title t --body-file plain.md",
        "gh pr create --fill",
        "gh pr create --title t",
        "gh issue comment 3 --body 'Thanks.'",
        "gh pr comment 3 -b 'Thanks.'",
        "gh pr comment 3 --editor",
        "gh pr review 3 --request-changes -b 'Fix.'",
        "gh issue edit 3 --body 'Body.'",
        "gh pr edit 3 -F plain.md",
        "gh release create v1 --notes 'Notes.'",
        "gh issue create --title t --body 'Written by Claude Code (claude-opus-5-5).'",
        "gh issue comment 3 --body \"$TEXT\"",
        "gh issue comment 3 --body-file - < plain.md",
        "gh issue close 3 --comment 'Done in abc123.'",
        "gh pr close 5 -c 'A substantive closing judgement.'",
        "gh issue reopen 5 --comment 'Reopening: the fix is wrong.'",
        "gh pr reopen 5 --comment=\"$TEXT\"",
        "gh pr review 3 --comment -b 'Looks fine.'",
        "gh pr review 3 -c --body 'Looks fine.'",
        "gh issue edit 3 --body=\"$TEXT\"",
        "gh release edit v1 --notes=\"$TEXT\"",
    ] {
        blocked(command, NEEDS_DISCLOSURE);
    }
    blocked(
        "gh issue comment 3 --body-file missing.md",
        "could not read",
    );
}

#[test]
fn guard_accepts_a_well_formed_disclosure_line() {
    for command in [
        format!("gh issue create --title t --body '{DISCLOSED}'"),
        "gh pr create --title t --body-file body.md".to_owned(),
        "gh issue comment 3 --body-file - < body.md".to_owned(),
        format!("gh pr comment 3 -F - <<'EOF'\n{DISCLOSED}\nEOF"),
        "gh pr review 3 --comment -b 'Review by glm:glm-5.3, relayed by claude-code:claude-opus-5-5.'"
            .to_owned(),
        "gh issue comment 3 --body 'Drafted with codex:gpt-6.2-sol, edited by A.'".to_owned(),
        "gh pr review 3 --approve".to_owned(),
        "gh pr edit 3 --title 'New title' --add-label bug".to_owned(),
        "gh issue close 3".to_owned(),
        "gh pr close 3 --delete-branch".to_owned(),
        format!("gh issue close 3 --comment '{DISCLOSED}'"),
        format!("gh pr reopen 3 -c '{DISCLOSED}'"),
        "gh pr ready 3".to_owned(),
        "gh --version".to_owned(),
        "gh pr create --web".to_owned(),
        "gh release create v1 --generate-notes".to_owned(),
    ] {
        allowed(&command);
    }
}

#[test]
fn guard_requires_a_pinned_head_and_message_file_for_gh_pr_merge() {
    allowed("gh pr merge 85 --squash --match-head-commit abc123 --subject 's' --body-file msg.txt");
    allowed("gh pr merge 85 --disable-auto");
    for command in [
        "gh pr merge 85 --squash",
        "gh pr merge 85 --squash --body-file msg.txt",
        "gh pr merge 85 --squash --match-head-commit abc123 --body 'text'",
        "gh pr merge 85 --squash --match-head-commit abc123 --body-file -",
    ] {
        blocked(command, "--match-head-commit");
    }
    for command in [
        "gh pr merge 85 --merge --match-head-commit abc123 --body-file msg.txt",
        "gh pr merge 85 --rebase --match-head-commit abc123 --body-file msg.txt",
        "gh pr merge 85 --match-head-commit abc123 --body-file msg.txt",
        "gh pr merge 85 -s -r --match-head-commit abc123 --body-file msg.txt",
    ] {
        blocked(command, "--squash");
    }
    allowed("gh pr merge 85 -s --match-head-commit abc123 --body-file msg.txt");
    blocked("gh -R o/r issue create --title t --body 'x'", "gh option");
    blocked("gh --repo o/r pr merge 5", "gh option");
}

#[test]
fn guard_blocks_gh_api_writes_it_cannot_check() {
    for command in [
        "gh api -X PUT repos/o/r/pulls/1/merge",
        "gh api --method=PUT /repos/o/r/pulls/1/merge",
        "gh api repos/o/r/issues/1/comments -f body=x",
        "gh api repos/o/r/issues -F title=t",
        "gh api -X PATCH repos/o/r/git/refs/heads/main -f sha=abc",
        "gh api -X PUT repos/o/r/contents/README.md --input body.json",
        "gh api graphql -f query='mutation { mergePullRequest(input: {}) { clientMutationId } }'",
        "gh api graphql --input query.json",
        "gh api -X POST $ENDPOINT",
    ] {
        blocked(command, "gh api");
    }
    allowed("gh api -X PATCH repos/o/r -f description=x");
    blocked(
        "gh api --method=\"$M\" repos/o/r/pulls/1/merge",
        "literally",
    );
}

#[test]
fn guard_expands_gh_aliases() {
    let repo = Fake {
        gh_aliases: vec![
            ("m", "pr merge 85 --squash"),
            ("sh", "!git push origin main"),
        ],
        ..Fake::default()
    };
    blocked_with(&repo, "gh m", "--match-head-commit");
    blocked_with(&repo, "gh sh", TO_MAIN);
    allowed_with(&repo, "gh copilot explain x");
}

fn hook_input(tool: &str, command: &str) -> String {
    format!(
        r#"{{"session_id":"s","cwd":"{FEATURE}","hook_event_name":"PreToolUse","tool_name":"{tool}","tool_input":{{"command":{command:?}}}}}"#
    )
}

#[test]
fn check_input_reads_the_bash_command_and_its_directory() {
    assert_eq!(
        check_input(&hook_input("Bash", "git status"), &Fake::default()),
        [] as [String; 0]
    );
    let fixes = check_input(
        &hook_input("Bash", "git push origin main"),
        &Fake::default(),
    );
    assert!(fixes.iter().any(|fix| fix.contains(TO_MAIN)), "{fixes:?}");
}

#[test]
fn check_input_allows_other_tools() {
    assert_eq!(
        check_input(
            &hook_input("Read", "git push origin main"),
            &Fake::default()
        ),
        [] as [String; 0]
    );
}

#[test]
fn check_input_blocks_input_it_cannot_read() {
    for (input, fix) in [
        ("not json", "could not parse the hook input"),
        ("{}", "no tool_name"),
        (
            r#"{"tool_name":"Bash","cwd":"/r"}"#,
            "no tool_input.command or cwd",
        ),
        (
            r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#,
            "no tool_input.command or cwd",
        ),
    ] {
        let fixes = check_input(input, &Fake::default());
        assert!(
            fixes.iter().any(|found| found.contains(fix)),
            "{input}: {fixes:?}"
        );
    }
}
