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
| [0003](0003-branch-work-and-ai-disclosure.md) | Branch work and AI disclosure | accepted 2026-11-19 (#10) |
| [0004](0004-architecture.md) | The architecture: core, OS layers, operations, negotiation and formats | accepted 2026-11-20; amended 2026-11-20 (#32) and 2026-11-21 (#49) |
| [0005](0005-ironafs-os.md) | `ironafs-os`, the crate for the operating-system calls that need `unsafe` | proposed |

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

- **Status:** `proposed`, `accepted`, `amended` (with the pull request),
  `partly superseded by NNNN` or `superseded by NNNN`, with dates.
- **Context:** the facts the decision rests on, with sources.
- **Decision:** what is decided, stated so it can be checked. Each numbered
  decision is marked firm or working (below).
- **Consequences:** what changes because of it, and what it costs.
- **Alternatives considered:** each, and why it was not taken.
- **Review:** every review round, with each finding and its disposition.

## Firm and working decisions

Platforms, interfaces and evidence change while IronAFS is built, so most
decisions are expected to move. From ADR 0004 on, each numbered decision is
marked one of two ways:

- **Firm:** a wire or on-disk format, a security invariant, or the
  don't-copy rule (ADR 0001). A firm decision changes only by a superseding
  ADR.
- **Working:** everything else. A working decision names a **"Revisit if"**
  trigger: the evidence that would reopen it, such as a version floor that
  moves, an interface that ships, or a named assumption that fails. It can
  be amended in place (below).

The decisions of ADRs 0001 to 0003 carry no mark and count as firm.

## Changing a decision

An accepted ADR is not rewritten, except by an amendment to a working
decision. Anything else needs a later ADR that supersedes it, and the old
one's status says `superseded by NNNN` (or `partly superseded by NNNN`).

### Amending a working decision

When a working decision's trigger fires, or new evidence contradicts it,
it is amended by a pull request:

- **An issue first.** The evidence is raised as a design issue (the
  design-question template), with each fact labelled
  ([docs/workflow.md](../workflow.md) §3c). A task that finds the
  contradiction stops and points to that issue; it does not work around
  the decision.
- **Same class, same review.** The amendment is reviewed as its ADR was:
  the same class and the same required seats from families that wrote
  neither the amendment nor the decision, at the tier
  [docs/workflow.md](../workflow.md) §2 ("Model routing") sets for that
  ADR. An amendment to an architecture ADR, or one touching security, is
  reviewed on the strongest row, as the ADR was.
- **The text changes in place.** The decision is rewritten to say what now
  holds, with a new "Revisit if" trigger. The ADR's status line gains
  `amended YYYY-MM-DD (#N)`, and its Review section gains an entry: the
  decision's id, what changed, the evidence, and the review rounds.
- **Limits.** An amendment never changes or weakens a firm decision,
  directly or through a decision a firm one depends on, and never turns a
  working decision into a firm one. Those need a superseding ADR. An
  amendment that would change most of an ADR's decisions is a new ADR.
- **The sweep applies.** The amendment pull request runs the supersession
  sweep below for the changed decision, and lists it in the Review entry.
- **The owner decides,** as for any ADR, when the pull request is merged.

A superseding ADR, or an amendment, comes with a **supersession sweep**:

- search the whole repository for each superseded rule, its key wording and
  the old ADR's number, and update or annotate every hit; then read every
  file that states an old rule in full, since a phrase search misses
  paraphrases;
- check each changed rule separately for each actor it applies to: the
  maintainer's session, contributors and their agents, and bots;
- list the sweep in the new ADR's (or the amended ADR's) Review section, so a
  reviewer can check it.

Small corrections that change no decision, such as fixing a link, are made in
place and noted in that ADR's Review section.
