<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# IronAFS: notes for Claude Code

The rules for every agent are in AGENTS.md:

@AGENTS.md

## Claude Code specifics

- Skills in `.claude/skills/`:
  - `rust-review`: the Rust house rules and the consistency pass. Use it
    before committing Rust changes.
  - `add-dependency`: use it before adding or upgrading any crate.
  - `wire-format`: use it for any wire message or on-disk format.
  - `phase-lifecycle`: use it at the start and exit of a phase of work.
  - `land-change`: use it before every commit or merge. It presents the change
    and its exact message to the human you work for, and waits for their
    approval.
  - `review-lanes`: for the maintainer's controller session only. It needs
    the maintainer's local review tooling.
- `.claude/settings.json` blocks common credential commands. Its patterns
  match invocations, not everything that could read a secret, so the "no
  secrets" rule in AGENTS.md is what holds.
- Never stage changes to `CLAUDE.md` or `AGENTS.md` unless the task is to
  change them.
