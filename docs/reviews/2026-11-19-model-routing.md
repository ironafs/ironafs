<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: model routing by seat, class and role

Issue #17, class I. The change replaces "Strongest tiers" in
`docs/workflow.md` §2 with "Model routing": the family for a seat is chosen
by the existing rules, then a table gives the model and effort by tier.
`AI.md`, `README.md` and the `review-lanes` skill follow it. Written by
Claude Code (`claude-opus-5-5`).

| Round | Range | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|
| 1, review | `7e18b85..2f243a7` | glm | glm-5.3 | high | PASS: 4 minor |

All four minor findings were fixed and checked by the controller:

- `AI.md` no longer lists "hard problems" as a reason for Mythos or Astra,
  and says Mythos may take promoted implementation only for work only the
  controller can run; `README.md`'s "hardest problems" went the same way;
- the light tier is split into helpers (Haiku, Gemini Flash) and research
  gathering (Sonnet, Gemini Argon), with exact ids, and Claude's effort is
  stated as the lane's default, recorded with the served model;
- the table now covers refutations and verifications by class, research
  synthesis that is not load-bearing, and class I and P implementation that
  is not mechanical;
- forming a view on a load-bearing question (one an ADR, a class S or R
  design or a format freeze will rest on) runs on the strongest tier, and
  any other view on the standard tier.

The review stopped there: no blocker or major finding.
