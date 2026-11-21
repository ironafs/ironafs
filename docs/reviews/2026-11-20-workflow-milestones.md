<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: milestones follow ADR 0004, O50

## The change

Class P.

- `docs/workflow.md` §6 listed the first three milestones as M0, M1 and
  "M2, FSKit spike". ADR 0004, part 3, O50 orders the phases differently,
  and is the decision.
- §6 now lists M0 to M7 as O50 orders them, keeping each phase's gate:
  - M2a: the core and Linux, read-only and unauthenticated, with the
    Windows SMB identity spike beside it;
  - M2b: rxkad, behind its specification route;
  - M3: Linux writes and locks;
  - M4: macOS, starting with its gate;
  - M5: Windows, starting with the rest of its gate;
  - M6: rxgk and other security work;
  - M7: proposed later work, each item scheduled only once its issue is
    approved (O51).
- One line defines the bare "M2" that ADR 0004 uses in several places:
  M2a and M2b together.
- **Written by** `claude-code:claude-opus-5-5`.
  - An independent read-only review by Claude Mythos 5.2 (same family,
    not a required seat) found the gaps in an earlier draft of this fix.
  - That draft was on another branch, together with a separate,
    undecided change to who implements code. This change is split out of
    it.

## Rounds

These were the same rounds as the README change's (its own record).

| Round | Range | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|
| 1, review | `34dfa0a..1c77a32` | GPT read-only | gpt-6.2-sol | high | FAIL: 1 major, 1 minor on this change |
| 2, fix verification | `1c77a32..6835eea` | GPT read-only, same session | gpt-6.2-sol | high | both fixed; no findings on this change |
| 3 | unchanged | GPT read-only, same session | gpt-6.2-sol | high | PASS |

## Findings

- **Major, fixed:** M7 cited O51 but left out its condition: each item is
  scheduled only once its issue is approved. Without it, an item's place
  in M7 could read as approval to schedule it.
- **Minor, fixed:** the list kept the macOS gate but dropped M2b's
  specification gate, and M5's start with the rest of the Windows gate.

The review stopped there: no blocker or major finding remains.
