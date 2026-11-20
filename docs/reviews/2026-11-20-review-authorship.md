<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: who counts as an author for review seats

Issue #26, class I. The change defines when a model family wrote a change,
for the rule that no required review seat goes to an author family:
`docs/workflow.md` §2 ("Seats by class"), `AI.md` ("Who writes and who must
review") and the `review-lanes` skill. Written by Claude Code
(`claude-opus-5-5`).

## Rounds

Round 1, review of `8a07bbc..bbaf7ee`.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: 3 minor |

All three were fixed in `7c2ddd5`:

- The exception for a family that gathered or synthesised research was not
  limited to research whose text stays out of the change. It now applies
  only while none of that family's text is in the change.
- The `review-lanes` skill still stated the old rule ("wrote neither the
  design nor the code"). It now points to §2's definition.
- "The text or code under review" could let a design's author claim it was
  not an author of the code built from that design. A family that wrote a
  design is now also an author of the code built from it.

Round 2, verification of `bbaf7ee..7c2ddd5`.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: all three fixed, no findings |

## Sweep

Every file that states the authorship rule was checked: `AI.md`,
`README.md`, `docs/workflow.md`, `docs/adr/README.md`, ADR 0002, and the
`land-change`, `phase-lifecycle` and `review-lanes` skills. Only
`docs/workflow.md` §2 defines the rule; the others use "wrote" or
"non-author" and follow it. The `review-lanes` skill's restatement was
updated in round 1. The first-reviewer order, the security-review families
and the embargo rules are unchanged.
