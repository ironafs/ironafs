<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review records

IronAFS is built with AI models from five families ([AI.md](../../AI.md)).
These records are the evidence of how each change was reviewed: which models
reviewed it, what they found, and what was done about the findings. Records
are redacted summaries: findings are listed one by one where it matters, and
some rounds group small fixes together. A refuted finding is recorded with
the evidence that refutes it.

## Records

| Record | What was reviewed |
|---|---|
| [2026-11-18-first-public-tree](2026-11-18-first-public-tree.md) | The first commit of the public history: mechanical checks, an independent pre-publication audit, and every review round |
| [2026-11-19-workflow-pr-limits](2026-11-19-workflow-pr-limits.md) | Issue #4: the limit on open pull requests, and settling and closing issues |
| [2026-11-19-ci-and-xtask](2026-11-19-ci-and-xtask.md) | Issue #1: the CI workflow and the `xtask` checks |
| [2026-11-19-research-refute-check](2026-11-19-research-refute-check.md) | The refute check of the driverless-clients research note (three families and an adjudicator), and of the macOS options note (two families) |

ADRs carry their review records in their own Review sections
([docs/adr/](../adr/README.md)).

## The format

One file per reviewed change, `YYYY-MM-DD-<slug>.md`, added in the same squash
commit as the change ([ADR 0002](../adr/0002-ai-use-and-attribution.md)). Each
record lists:

- **every seat:** agent id, served model, effort and verdict;
- **every finding:** what it was, and whether it was fixed, refuted (with the
  evidence) or deferred (with where it is tracked).

Records are written from the lanes' output and redacted: no paths outside the
repository, hostnames, cell names, keys or other projects. Raw transcripts are
not published, because they can contain secrets and personal data.
