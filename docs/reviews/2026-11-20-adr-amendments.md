<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: firm and working ADR decisions, and amendments

Issue #28, class I. The change marks each ADR decision firm or working, lets
a working decision be amended in place with new evidence
(`docs/adr/README.md`, `docs/adr/template.md`), and makes every brief
start with a re-check of the claims and decisions it rests on
(`docs/workflow.md` §3). Written by Claude Code (`claude-opus-5-5`).

## Rounds

Round 1, review of `48fe0ec..9751eba`.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | FAIL: 1 major |

- Major: the README said an amendment is reviewed at the tier §2 sets for
  its class, but §2's model routing puts architecture ADRs on the strongest
  row, so an amendment to ADR 0004 could have been reviewed below the tier
  ADR 0004 itself was. ADR 0004's "How this ADR is organised", point 2 said
  "the normal tier for its class". Fixed in `02908bb`: an amendment to an
  architecture ADR, or one touching security, is reviewed on the strongest
  row; ADR 0004's point 2 now points to the README (a correction that
  changes no decision, noted in ADR 0004's Review section).

Round 2, verification of `9751eba..02908bb`.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: fixed, no findings |

## Supersession sweep

Every place that states that an accepted ADR is not rewritten, or how ADR
decisions change: `docs/adr/README.md` (changed); ADR 0004, "How this ADR
is organised", point 2 (now points to the README); ADR 0003, point 3
("never rewritten" is about git history, not ADRs). No other hit in
`AGENTS.md`, `CONTRIBUTING.md`, `AI.md`, `README.md`, `.github/` or
the skills.
