<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: Claude implements from M2a; label rules

## The change

Class I. Issue #40.

- `docs/workflow.md` §2, `AI.md` and `README.md`: from M2a, Claude Code
  subagents (Opus) are the default implementer, dispatched by the
  controller with a written brief, test first; codex adds capacity on
  separate tasks. The M2a designs are Claude-written, and §2 counts a
  design's family as an author of the code built from it, so Claude
  implementing keeps one author family and leaves GLM and GPT free for
  the required and security seats. GLM becomes the first reviewer of
  Claude-written code. The owner decided this on 2026-11-20.
- `docs/workflow.md` §6: written label rules, triage, the `area:test`,
  `area:os` and `area:ops` labels, and a description for every type and
  area label. The three issue forms set `needs-triage`; `dependabot.yml`
  sets `dependencies`. The labels were created on GitHub with the owner's
  approval before this change lands.
- ADR 0002: a small correction that changes no decision
  (`docs/adr/README.md`): its Context's roles line is marked as the roles
  at acceptance, and the `codex` agent-id row says "implementer for extra
  capacity". Noted in the ADR's Review section.
- **Written by** `claude-code:claude-opus-5-5`.

## Rounds

| Round | Range | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|
| 1, review | `1f44f21..15a22b0` (an earlier version of the branch, on an older `main`) | GLM | glm-5.3 | high | FAIL: 2 major, 1 minor |
| 2, review | `0fa8a2d..f04dd07` (the branch rebuilt on current `main`, plus the label rules) | GLM | glm-5.3 | high | PASS: 2 minor |
| 3, fix verification | `fc3311b..c1658d7` | GLM | glm-5.3 | high | PASS: no findings |

## Findings

Round 1:

- **Major, fixed:** the opening summary of `docs/workflow.md` still said
  other families write code. Rewritten when the branch was rebuilt.
- **Major, fixed in round 3's commit:** ADR 0002 still said "codex
  implements" and listed codex as the implementer. The round-2 brief
  wrongly described round 1's findings as minor, so round 2 did not look
  for it; it was found while writing this record, corrected in place, and
  verified in round 3, with a sweep of the tree for other wording of codex
  as the main implementer (none).
- **Minor, fixed:** `AI.md`'s second-review-family row named only two
  families, contradicting §2. It now names every eligible family.

Round 2:

- **Minor, fixed (`fc3311b`):** the triage step did not apply the
  `area:*` label that the label rules require.
- **Minor, fixed (`fc3311b`):** "every PR has exactly one class label"
  contradicted Dependabot's PRs before triage; the rule now holds from
  triage on.

Round 2's reviewer could not run a YAML parser in its sandbox; the
three issue forms, the chooser configuration and `dependabot.yml` were
parsed separately, and all load.

The review stopped there: no blocker or major finding remains.
