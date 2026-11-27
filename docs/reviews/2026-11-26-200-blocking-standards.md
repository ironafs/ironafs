<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Blocking concurrency in the coding standards

Class S. [#200](https://github.com/ironafs/ironafs/issues/200) supplies
Q12's rules 14 and 18 alignment. Current source base `2285fc45b604119616c4bcc11d31b954febeac30`.
The [filing review](2026-11-26-71-q12-issue-brief.md) covers the issue brief.

## Pre-edit evidence and design review

**INSPECTED.** At the implementation base, rule 14 required a
cancellation-safety heading for async selection. Rule 18 preferred
`JoinSet` to bare spawning, required documentation for `select!`, and
called for explicit async shutdown. ADR 0004 C3.3–6 and
C39.1–4, its Part 1 Consequences point 5, O45.5, and the seams and scheduling
specification require the blocking model. The full readbacks and source
comparison used rules 14, 18 and 21, seams §§3, 5.3–5.7 and Q3/Q12, plus
the landed `Clock::wait`, `Waited`, `Rpc` and `Disk` contracts.
The reviewed inputs are byte-identical to the eight pinned source files
at the original brief's base. The final wording patch SHA-256 is
`94fed64eb6779f40c65d45c418ff5c7ae51791cf0c1c1287abf472a87a06d01e`.

| Scope and patch pin | Non-author reviewer | Verdict | Quota headroom |
| --- | --- | --- | --- |
| Initial exact patch `f543e2eb` | gemini `gemini-4-carbon-high` | FAIL, three majors | Not reported |
| Scoped first correction `55f26ed3` | gemini `gemini-4-carbon-high` | FAIL, one remaining major | Not reported |
| Scoped second correction `94fed64e` | gemini `gemini-4-carbon-high` | PASS, zero findings | Not reported |

The agy receipt attests each invoked model pin and terminal readback. The
envelope has no separate served-model field. The Carbon High ordinary
document pin is uncalibrated on IronAFS. These verdicts cover this exact
standards change. The GLM launcher stopped at its credential precondition.

- **F1, pre-existing contract wording.** The paragraph omitted the thin
  drivers from the production spawner boundary. Fixed to name the daemon
  and thin drivers only, retain the daemon's C39 responsibilities, and
  name the separate simulator tasks. The old preference for `JoinSet`
  supplies context for explicit shutdown.
- **F2, pre-existing contract wording.** “An Rpc call” was ambiguous
  about the permitted method and its wait. The first fix distinguished
  reply slots from the bounded close wait. The first correction introduced
  a narrower ambiguity about other blocking. The second fix
  says `call` and `accept` block only on their own slot through
  `Clock::wait`, `open` and `respond` never block, and `close` gets
  only the send-lock hold allowed by seams §5.4 point 5.
- **F3, pre-existing contract wording.** Clarified explicit shutdown for
  every loop and narrowed the driver note to its own internal readiness
  waits and joins. The wording permits a shared shutdown token.

The confirmed findings narrowed from three wording obligations to one RPC
sentence and then zero. There were no blockers. The first correction
introduced one wording ambiguity in the same RPC contract. The second
introduced none. Review scope and both corrections left rule 14
and the lock paragraph unchanged. The final narrow verification used the
seam blocking and yield clauses. The controller checked the surrounding
signatures and shutdown clauses against the sources pinned for the
initial review. No runtime mechanism or implementation guarantee is
claimed from this documentation comparison.

Written by codex:gpt-6.2-sol.
