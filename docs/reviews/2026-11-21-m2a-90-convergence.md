<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: convergence rules for review rounds

## The change

Class I. Issue #54.

- `docs/workflow.md` §2 and §3, and the `review-lanes` skill, now hold the
  rules for long review series:
  - a convergence note before any round after the second;
  - fix rounds add no mechanism;
  - moving a finding into a specification item counts as a deferral that a
    refutation seat must agree to;
  - a single seat's tail of one or two majors goes to the judgement seat;
  - converged parts are split off as changes of their own;
  - findings are tagged by origin and kind;
  - range, freezing and baseline rules for lanes;
  - a refutation seat on the controller's own rulings;
  - runtime claims are settled by a seat that runs code;
  - "uncalibrated on IronAFS" beside verdicts from uncalibrated pins.
- **Deliberate change:** the stop rule no longer ends patching after two
  fix rounds unconditionally. A further fix round needs a convergence note
  that shows the counts falling, few findings introduced by the fixes and
  the kinds narrowing. Otherwise the change is split, its mechanism moves
  into specification items, it is redesigned, or its blocker is recorded.
- The owner's review-depth rule, approved on 2026-11-21, added in
  `b3c8acc` to §3, steps 1 and 3:
  - text is classed by what it governs, not by its form;
  - an issue brief gets two reviews before it is filed: one non-author
    family, plus a focused pass by a second non-author family;
  - a design point is deferred only if the current work is correct
    whichever way it is decided;
  - review runs per brief, and acceptance defects found later, are
    counted.
- The fixes also touched `AI.md` (the owner's checklist) and this
  directory's README (the record format), so that both agree with the new
  rules.
- **Written by** `claude-code:claude-opus-5-5`.

## Rounds

| Round | Range | Seat | Served model | Effort | Verdict | Quota headroom |
|---|---|---|---|---|---|---|
| 1, review | `f81aa7d..8ccfdec` | GLM | glm-5.3 (uncalibrated on IronAFS) | high | FAIL: 1 blocker, 3 major, 6 minor | not reported |
| 2, fix verification | `8ccfdec..a82aeaf` | GLM | glm-5.3 (uncalibrated on IronAFS) | high | FAIL: 1 minor | not reported |
| 3, review of the added rule | `9782a22..b3c8acc` | GLM | glm-5.3 (uncalibrated on IronAFS) | high | PASS: 4 minor | not reported |

Round 2 found all ten round-1 findings fixed. Its one minor finding was
fixed in `496ce0d`. Round 3 reviewed the owner's review-depth rule, which
was added after round 2. Its four minor findings were fixed in `29e936b`
and recorded here under the stop rule, with no further round.

**Convergence note.** This note was required before round 3 but was
written after it: a departure from the new rule, recorded here.

- Counts of blockers and majors per round: 1 and 3, then 0 and 0, then
  0 and 0.
- Introduced by the last fix: round 2's one finding was introduced by fix
  round 1. Round 3's four findings came from text new in `b3c8acc`; none
  came from a fix.
- Kind: no drift. All findings were contract or consistency findings, and
  they narrowed from loopholes in closure to wording and definitions.

## Findings

Round 1 (all fixed in `a82aeaf`):

- **Blocker, fixed** (origin: pre-existing; kind: contract). Moving a
  finding into a specification item closed it for the merge without any
  reviewer, and confirmed minor findings were not covered at all. Now such
  a move, at any severity, is a deferral. It closes the finding only when
  the refutation seat agrees that the finding asks for mechanism the
  change need not decide.
- **Major, fixed** (origin: pre-existing; kind: contract). The effect of
  the judgement seat's ruling on a single seat's tail was undefined. The
  seat now rules whether each major holds as a correctness or contract
  defect in what the change decides, and the text states what follows from
  each outcome. The merge step names the closure.
- **Major, fixed** (origin: pre-existing; kind: contract). "A part that
  every required seat has passed" was undefined. Convergence is now
  defined by pinned conclusive verdicts, and the part becomes a change of
  its own.
- **Major, fixed** (origin: pre-existing; kind: consistency between parts).
  A finding closed as a specification item could leave no seat creditable
  under ADR 0002, while CI requires one. Merge now needs a required seat
  whose final verdict covers the landed tree under ADR 0002's rule, which
  is unchanged; failing that, a verification round on the final diff.
- **Minor, fixed** (origin: pre-existing; kind: consistency between
  parts). The merge-base range contradicted diff-scoped fix verification.
  Fix verifications now use the fix's own commits.
- **Minor, fixed** (origin: pre-existing; kind: consistency between
  parts). `AI.md`'s owner checklist did not know the new closures.
- **Minor, fixed** (origin: pre-existing; kind: consistency between
  parts). Per-dispatch cost in public records contradicted `AI.md` and ADR
  0002's owner decision 2. The row now records quota headroom, or "not
  reported", and no cost figures.
- **Minor, fixed** (origin: pre-existing; kind: consistency between
  parts). This directory's README did not list the new record items.
- **Minor, fixed** (origin: pre-existing; kind: contract). It was unclear
  when a fix round counts. A fix round now counts once its verification
  concludes, and the convergence note is brought up to date before each
  further fix round.
- **Minor, fixed** (origin: pre-existing; kind: contract). A tail has no
  second position for the judgement brief. Position B is now the change's
  own text, quoted from its source and never written by the controller.

Round 2:

- **Minor, fixed in `496ce0d`** (origin: introduced by fix round 1; kind:
  contract). When the seats' pinned shas differ, "split from that sha" had
  no single referent. The part is now split from the latest sha that every
  required seat's verdict covers for its files, where a verdict covers a
  later sha only if every change to those files since then was a verified
  fix. The split change's record carries the part's findings.

Round 3:

- **Minor, fixed in `29e936b`** (origin: introduced in `b3c8acc`; kind:
  contract). "Marked ready" named a step that no rule performs. The rule
  now applies before a brief is filed or dispatched.
- **Minor, fixed in `29e936b`** (origin: introduced in `b3c8acc`; kind:
  contract). "The same revision" could not be pinned for issue text. It
  is now the draft file, pinned by its sha256 prefix.
- **Minor, fixed in `29e936b`** (origin: introduced in `b3c8acc`; kind:
  contract). The antecedent of "each one" was ambiguous. The text now
  says "each prerequisite".
- **Minor, fixed in `29e936b`** (origin: introduced in `b3c8acc`; kind:
  missing mechanism). Brief reviews and their counts had no record. They
  go in the `docs/reviews/` record for the batch of briefs filed
  together. The second pass is also stated to be by a non-author family,
  which was a cheap item from round 3's uncertain list.

Cheap items from round 2's uncertain list, fixed in the same commit: the
skill's final-diff verification is scoped to class I and above, and the
skill's judgement question names correctness or contract defects, as the
workflow does.

Left as they are:

- A controller refutation of a confirmed minor finding still needs no
  refutation seat. This rule predates this change, and the issue scoped
  the refutation seat to blockers and majors.
- The convergence criteria are qualitative, as adopted.
- From round 3's uncertain list:
  - the focus of the second pass is not defined;
  - no routing row names the pin for a brief review;
  - no process row in §1 shows how much of the closure rules text may
    drop;
  - a deferral can exist before any issue depends on it, and nothing
    adds the blocker link when a dependent issue is created later;
  - the task issue form has no fields for prerequisites.
- The tree was searched for the old stop rule and the old checklist
  wording. Only the files changed here held them.
