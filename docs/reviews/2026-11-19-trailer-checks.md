<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: commit trailer checks

Issue #2, class I. The change adds `cargo xtask trailers` with three modes:
`pr` checks a pull request description, `main` checks a landed commit, and
`commit` checks a branch commit, used by an opt-in `commit-msg` hook. A CI
job runs `pr` on every pull request, again when its description is edited,
and `main` on every push to `main`; `ci-ok` requires it. Written by codex
(`gpt-6.2-sol`).

## Rounds

Round 1, review of `41c3ab1..291dd57`.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | FAIL: 1 major, 1 minor |

Both were fixed in `b041cde`, in codex's resumed session:

- **Human trailers** (major): every unknown key was refused, although ADR
  0002 keeps `Co-authored-by:` and `Reviewed-by:` for humans. Both are now
  accepted in the `Name <email>` form; other unknown keys are still refused.
- **Branch-commit rule** (minor): ADR 0002's Enforcement text now points to
  ADR 0003 for the branch-commit trailers the hook checks.

Round 2, verification of those fixes (`291dd57..b041cde`).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS |

## Evidence

- 29 unit tests, including the messages of the three commits on `main`, a
  human co-author, an unknown key and an empty description (EXECUTED,
  macOS).
- `cargo xtask trailers main` passes on each historical message, and a
  branch message with `Class:` is refused with its line number (EXECUTED).
- `actionlint` and `zizmor`: no findings (EXECUTED).
