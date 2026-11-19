<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: CI workflow and xtask checks

Issue #1, class I. The change adds the GitHub Actions workflow
(`.github/workflows/ci.yml`) and the dependency-free `xtask` crate
(`cargo xtask check`, and opt-in `cargo xtask hooks install` and
`uninstall`), and updates the documents that name the commands. Written by
codex (`gpt-6.2-sol`), with later fixes by codex and by Claude Code
(`claude-opus-5-5`).

The pins (one action by commit SHA, tool archives by SHA-256, the `reuse`
image by digest) and the security settings were the owner's decisions,
recorded on #1. Each pinned release was at least seven days old.

## Rounds

Round 1, review of `ce1cb25..44f4d51`.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: 2 minor |

Both findings were fixed in `65567a2`, together with a third point the
controller added:

- `hooks install --force` replaced a hook xtask did not write; it now moves
  that hook to `pre-push.xtask-backup`, and refuses if the backup path is
  taken;
- `cargo xtask check` run inside a member crate checked only that crate;
  clippy and nextest now take `--workspace`;
- nextest's "no tests" behaviour is set by the explicit `--no-tests fail`
  argument, as in CI, instead of an environment variable whose name could
  not be confirmed.

Round 2, verification of those fixes (`44f4d51..65567a2`).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: 2 minor |

All three fixes were confirmed. Its two minor findings were fixed in
`5a2d539` and checked by the controller, and the review stopped:

- the README, `CONTRIBUTING.md` and `AGENTS.md` listed the individual
  commands without `--workspace` and `--no-tests fail`;
- an interrupted `--force` could leave an empty backup file that blocks later
  runs; the error now says to inspect and remove it.

Round 3, verification of the owner's change to CI (`35e673b..a0f0522`): the
tests run on Linux only for now, and Clippy lints the Windows and macOS
targets from Linux. There is no OS-specific code yet, and macOS and Windows
runners cost 10 and 2 times the private repository's Actions minutes.
Coding standards rule 15 now says the change that adds the first code behind
a Windows or macOS `cfg` adds that system's runner.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: 1 minor |

Its finding was fixed and checked by the controller: the `wire-format` skill
still said CI checks the vectors on all three systems.

## Evidence

- `actionlint` and `zizmor`: no findings (EXECUTED, on each workflow change).
- The first CI run, on `44f4d51`, passed on Linux, macOS and Windows
  (EXECUTED, GitHub Actions).
- A deliberately broken scratch pull request (#9: one formatting error, one
  unused variable) failed where it should: `lint` at "Check formatting",
  Clippy on all three runners with warnings denied by `build.warnings`, and
  `ci-ok` (EXECUTED; the pull request was then closed and its branch
  deleted).
- `cargo xtask check` passed from the repository root and from inside
  `crates/ironafs-cli` (EXECUTED, macOS).
- The Linux-only workflow passed on `a0f0522`, including Clippy for the
  Windows and macOS targets (EXECUTED, GitHub Actions).
