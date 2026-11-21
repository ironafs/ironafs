<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: the agent command guard and the pre-push secret scan

## The change

Class I. Issue #8: the rules agents and committers follow are checked by
hooks, not memory. The change enforces existing rules and changes none of
them.

- `cargo xtask guard`, run by a Claude Code `PreToolUse` hook on Bash
  (`.claude/settings.json`). It parses the command line with its own shell
  reader and blocks:
  - a commit, push, pull, rebase, reset, ref update, branch move or fetch
    that moves `main`;
  - a branch commit whose message lacks a valid `Assisted-by:` trailer or
    carries `Signed-off-by:`, `Class:` or `AI-Reviewed-by:` (ADR 0003);
  - an issue, pull request, comment, review, closing comment or release
    text without its disclosure line (ADR 0003, point 6);
  - `gh pr merge` without `--squash`, `--match-head-commit` and
    `--body-file` (the `land-change` skill, section 4), and `gh api`
    writes it cannot check.

  It follows wrappers (`sudo`, `env`, `nice`, `timeout`, `xargs` and
  others), `sh -c`, `eval`, `source`, script files, substitutions,
  here-documents, git and gh aliases and `GIT_*` variables. It fails
  closed: input it cannot parse, a word it needs that expands at run
  time, a repository it cannot query or a wrapper option it does not know
  blocks the command, with a fix. `cargo xtask guard --self-test` runs 19
  planted commands through the rules.
- `cargo xtask pre-push`, run by the opt-in `pre-push` hook: it refuses
  any update of `main`, scans the outgoing commits and their messages with
  gitleaks, then runs `cargo xtask check`. `hooks install` updates hooks
  an earlier xtask installed.
- `AGENTS.md` and `CONTRIBUTING.md` describe both, and call the guard a
  guardrail, not a proof.
- **Written by** `claude-code:claude-opus-5-5`, test first.

## Rounds

The required seat (class I, Claude-written code): one non-author family,
GLM, the first reviewer. Its pin, glm-5.3 at high effort, passed the
lane calibration of #48 (11 of 12 planted defects found, 6 of 6
critical). It ran read-only, without running the tests, on 2026-11-21
(UTC).

| Round | Range | Seat | Served model | Effort | Verdict | Quota headroom |
|---|---|---|---|---|---|---|
| 1, review | `e6f3ece..13ef79c` | GLM | glm-5.3 | high | FAIL: 2 major, 6 minor | not reported |
| 2, fix verification | `eee5211..0b0cb6b` | GLM | glm-5.3 | high | PASS: 6 minor | not reported |

`eee5211` merges `main` into the branch without conflicts; the fixes of
round 1 are `18bc844..0b0cb6b`. Round 2 found all eight round-1
findings fixed. Its six minor findings were fixed in
`e524408..02da253` and recorded here under the stop rule, with no further
round.

## Findings

Round 1 (the defects are quoted from GLM's verdict):

- **Major, fixed in `18bc844`** (origin: pre-existing; kind: correctness).
  "run() takes the first word after a wrapper (sudo/nice/timeout/time/exec/builtin/nohup)
  as the command name; if that word is a NAME=value assignment or an
  option value the guard's flag table does not know, the match falls to
  `_ => ()` and the entire remainder of the line is never examined."
  Wrappers now come from a table listing every option each one knows,
  short and long, attached and clustered. An unknown option, or an
  assignment where a wrapper's command should be, blocks. Shell keywords
  and `time` take leading assignments as a command does.
- **Major, fixed in `9065b38`** (origin: pre-existing; kind: correctness).
  "Only commit, merge, cherry-pick, revert, am, push and checkout/switch
  are checked." `git pull` on `main` is blocked unless `--ff-only`;
  `git rebase` on `main`, or of `main` from any branch, is blocked; so are
  `git reset` on `main` to anything but `HEAD` or its upstream,
  `git update-ref` of `main`, `git branch` forcing, moving, copying or
  deleting `main`, `git checkout -B main` and `git switch -C main`, and
  fetch refspecs whose destination is `main` or a pattern.
  `fast-import`, `filter-branch`, `replay`, `send-pack` and `http-push`
  always block: the guard cannot see which refs they change.
- **Minor, fixed in `0c87597`** (origin: pre-existing; kind: contract).
  "`gh pr close 5 --comment '...'` publishes comment text with no
  disclosure check." Closing and reopening comments now need the line. The
  guard cannot tell a pointer to evidence from a judgement, so it asks
  for the line on every one; adding it is always allowed. Found while
  fixing: `gh pr review --comment -b` and `--body="$TEXT"` went
  unchecked, and gh options before the subcommand were allowed. All three
  are fixed.
- **Minor, fixed in `0c87597`** (origin: pre-existing; kind: contract).
  "The merge check verifies only --match-head-commit and --body-file, not
  --squash." `gh pr merge` now needs `--squash`.
- **Minor, fixed in `07d627b`** (origin: pre-existing; kind: correctness).
  "A git alias beginning with `!` is re-checked via child_script against
  the shell scope, discarding the resolved Place." A shell alias now runs
  in the directory git resolved, with its `GIT_*` variables and `-c`
  options.
- **Minor, fixed in `07d627b`** (origin: pre-existing; kind: correctness).
  "Alias existence is sampled before the script runs." A `git config`
  that may write, or a `gh alias` other than `list`, now blocks a later
  alias use in the same Bash call.
- **Minor, fixed in `18bc844` and `07d627b`** (origin: pre-existing;
  kind: correctness). "A shell with a script-file operand returns without
  reading it." Script files given to a shell, `source` or `.` are read and
  checked; an unreadable one blocks. `coproc`, `stdbuf`, `setsid`,
  `caffeinate` and `doas` are followed.
- **Minor, fixed in `07d627b`** (origin: pre-existing; kind: correctness).
  "`export GIT_DIR=... / declare -x GIT_DIR=...` before a git command in
  the same script are ignored." `GIT_*` variables from `export`,
  `declare`, `typeset`, `local`, `readonly` and bare assignments now apply
  to later commands; `unset` removes them.

Round 2 (each fix began with the planted command, which the guard let
through or wrongly blocked before the fix):

- **Minor, fixed in `02da253`** (origin: introduced by fix round 1; kind:
  correctness). `coproc NAME cmd args` was said to hide the command. Bash
  5.3 takes no name before a simple command: in `coproc x git push origin
  main`, `x` is the command, so the guard's reading was right, but `x`
  then went unread. The fix for the next finding covers it.
- **Minor, fixed in `02da253`** (origin: pre-existing; kind: correctness).
  Commands that run another command, such as `watch`, `strace`,
  `taskset`, `ionice` and `valgrind`, were not followed. The guard now
  reads through any command it does not know: the first argument named
  `git` or `gh` is checked as that command, with the assignments just
  before it, and an earlier argument that is a command line naming git or
  gh (`watch -n2 'git push origin main'`) is checked as a script. Commands
  that only print, search or describe their arguments (`echo`, `printf`,
  `grep`, `rg`, `man`, `which` and a few more) are not read, so
  `echo git push origin main` is allowed. This reads through rather than
  blocking every unknown command, so ordinary work (`watch git status`)
  still runs. A self-test case plants `watch -n2 git push origin main`.
- **Minor, fixed in `2feb6e9`** (origin: pre-existing; kind:
  correctness). A command's own assignments were dropped when it ran a
  shell: `GIT_DIR=<main>/.git sh -c 'git commit …'` was checked against
  the task branch. A shell, its script file or standard input, `eval` and
  `source` now start from those assignments. For `eval` and `source`
  they remain afterwards, as in a POSIX shell, which can only block more.
- **Minor, fixed in `af45ad0`** (origin: pre-existing; the round-1 fetch
  check did not cover it; kind: correctness). A fetch without refspecs
  uses the configured `remote.<name>.fetch`, which was not read. `git
  fetch`, `git pull` and `git remote update` now read every remote's
  configured fetch refspecs, through the same `-C`, `-c` and `GIT_*`
  context, and block one that can write `main`. A destination under
  `refs/remotes/` or `refs/tags/` is allowed, including a pattern, so the
  default refspec and an explicit `refs/heads/*:refs/remotes/origin/*`
  (blocked before) pass. `--refmap` values are checked too. A `git config`
  earlier in the same Bash call blocks a fetch.
- **Minor, fixed in `9071864`** (origin: introduced by fix round 1;
  kind: correctness). `env --chdir=/x`, a valid spelling, was blocked as
  an unknown option. `env` now takes `--chdir=`, `-C<dir>`, `--unset=`
  and `-u<name>`, and follows the directory.
- **Minor, fixed in `e524408`** (origin: pre-existing; kind:
  correctness). A `--` anywhere skipped the checkout and switch checks.
  `git switch main --` and `git checkout main --` switch branches (checked
  with git 2.55), so a commit after them in the same call was checked on
  the old branch. Now only a `checkout` with a path after `--` counts as
  staying on the branch, and `-B`/`-C` naming `main` blocks before the
  `--` (`git checkout -B main --` resets `main`; `git checkout -B main --
  <path>` is refused by git).

GLM also listed what it could not check:

- the tests and the self-test, which it did not run; the evidence below
  covers them;
- whether `git checkout -B main -- <path>` and `git switch main --` are
  accepted, and bash's `coproc NAME` form: checked above;
- GNU `env --chdir=`: documented by coreutils; macOS's `env` takes only
  `-C`;
- whether real gh parses `gh pr merge -m` and `-r` as the guard does: the
  guard reads them as valued flags and blocks, which fails closed;
- round 1's uncertain items, below.

## Gaps left

The guard reads only the command line. These are known and left as they
are; `AGENTS.md` and `CONTRIBUTING.md` call the guard a guardrail, not a
proof, and the rules hold whether or not it runs.

- An executable run by path (`./x.sh`) is not read: the guard cannot tell
  a script from a binary by its name. Nor are commands that run code from
  their own languages or files (`python -c`, `make`), or a command given to
  a runner in a word that expands at run time.
- Git configuration written other than through `git config` (an editor,
  `echo >> .git/config`) is not seen by the same-call alias and fetch
  checks.
- Git runs a `!` alias from the work tree's top level; the guard uses the
  resolved directory, which differs only for a relative `cd` in the alias.
- Not verified by running: whether bash ends `$((…))` earlier than the
  guard's quote-blind parenthesis count; the gitleaks argument shape and
  exit codes, taken from its version 8 documentation; whether Claude
  Code allows the command when the hook's 300 s timeout fires; that
  `%(push:remoteref)` follows `remote.<name>.push` and `remote.pushDefault`;
  that git builtins always win over aliases; and whether cargo's progress
  output on the hook's first build is shown.
- Issue #8's live demonstration of each block, in a Claude Code session
  that loads the settings, is still owed and is recorded on the issue.

## Evidence

- At `02da253`: `cargo xtask check`, `cargo nextest run --workspace
  --retries 0 --no-tests fail` (391 tests passed), `cargo clippy
  --all-targets --workspace` (no warnings) and `cargo xtask guard
  --self-test` (19 planted commands ended as planted), each exit 0
  (EXECUTED, macOS, 2026-11-21 UTC).
- Red before green: round 1's 35 planted bypasses were all allowed at
  `13ef79c` and all blocked at `0b0cb6b`. Round 2's planted commands
  were run through the guard before and after each fix: 33 that must
  block were allowed before (4 checkout and switch, 6 shell assignment,
  12 fetch, 11 runner), 5 that must pass were blocked (3 `env` spellings,
  2 fetches into `refs/remotes/` and `refs/tags/`), and 2 `env` spellings
  that must block did so only as unknown options. After the fixes all 40
  give the right answer; each cycle's tests failed first (EXECUTED,
  macOS, 2026-11-21 UTC).
