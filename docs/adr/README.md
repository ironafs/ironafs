<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Architecture decisions

An architecture decision record (ADR) is a short document that records one
significant decision: its context, the decision itself, its consequences,
the alternatives considered, and how it was reviewed. ADRs are how IronAFS
decides anything that is hard to change later.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-licence-and-dont-copy.md) | Licence and the don't-copy rule: GPL-3.0-or-later, no code from other AFS implementations | accepted |
| [0002](0002-ai-use-and-attribution.md) | AI use, commit attribution and working in the open | accepted; partly superseded by 0003 |
| [0003](0003-branch-work-and-ai-disclosure.md) | Branch work and AI disclosure | proposed |

## How an ADR is made

1. **A design question** is raised as an issue (the design-question template).
2. **A draft ADR** is written in a pull request, numbered after the last one,
   with status `proposed`.
3. **It is reviewed** by at least one model family that did not write it
   ([docs/workflow.md](../workflow.md) §1 and §2). Each round's findings, and
   how each was handled, are recorded in the ADR's own Review section.
4. **The owner decides.** The status becomes `accepted`, with the date.

## What every ADR contains

Start from [template.md](template.md).

- **Status:** `proposed`, `accepted`, or `superseded by NNNN`, with dates.
- **Context:** the facts the decision rests on, with sources.
- **Decision:** what is decided, stated so it can be checked.
- **Consequences:** what changes because of it, and what it costs.
- **Alternatives considered:** each, and why it was not taken.
- **Review:** every review round, with each finding and its disposition.

## Changing a decision

An accepted ADR is not rewritten. A later ADR supersedes it, and the old one's
status says `superseded by NNNN`. Small corrections that change no decision,
such as fixing a link, are made in place and noted in its Review section.
