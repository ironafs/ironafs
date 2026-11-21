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
| [2026-11-19-trailer-checks](2026-11-19-trailer-checks.md) | Issue #2: commit trailer checks in CI and the `commit-msg` hook |
| [2026-11-19-research-refute-check](2026-11-19-research-refute-check.md) | Issue #16: the refute check of the driverless-clients research note (three families and an adjudicator), the macOS options note, and the reviews of the Windows Cloud Files and UNC notes |
| [2026-11-19-judgement-seat](2026-11-19-judgement-seat.md) | Issue #14: the judgement seat and the strongest tiers |
| [2026-11-19-model-routing](2026-11-19-model-routing.md) | Issue #17: model routing by seat, class and role |
| [2026-11-20-pre-public-tidy](2026-11-20-pre-public-tidy.md) | Issue #3: wording tidy before going public |
| [2026-11-20-adr-0004-claims](2026-11-20-adr-0004-claims.md) | Issue #20: the factual claims behind ADR 0004, each re-checked at its source |
| [2026-11-20-windows-smb-first-docs](2026-11-20-windows-smb-first-docs.md) | Issue #21: the public description of Windows as SMB first |
| [2026-11-20-review-authorship](2026-11-20-review-authorship.md) | Issue #26: who counts as an author for review seats |
| [2026-11-20-adr-amendments](2026-11-20-adr-amendments.md) | Issue #28: firm and working ADR decisions, and amendments |
| [2026-11-20-kernel-code-research](2026-11-20-kernel-code-research.md) | The research note on kernel code against user space, and README corrections |
| [2026-11-20-readme-adr-0004](2026-11-20-readme-adr-0004.md) | The README follows ADR 0004: goals, sources for the Why section, planned platforms and order of work |
| [2026-11-20-workflow-milestones](2026-11-20-workflow-milestones.md) | Milestones follow ADR 0004, O50 |
| [2026-11-20-workflow-implementer](2026-11-20-workflow-implementer.md) | Issue #40: Claude implements from M2a; label rules and triage |
| [2026-11-21-m2a-01-xdr-spec](2026-11-21-m2a-01-xdr-spec.md) | Issue #42: `docs/specs/`, its template and the XDR profile with caps |
| [2026-11-21-m2a-64-corpora](2026-11-21-m2a-64-corpora.md) | Issue #64: rules for published conformance corpora in `docs/specs/README.md` |

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
