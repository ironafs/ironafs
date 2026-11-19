<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: the judgement seat and strongest tiers

Issue #14, class I. The change adds a judgement seat to `docs/workflow.md`
§2 (a third, independent family rules on a dispute that checked evidence
does not settle), a "Strongest tiers" table used for that seat, security
reviews, class R designs, format freezes and architecture ADRs, and one
promotion rule for every family. It brings `AI.md` and the `review-lanes`
skill into line. Written by Claude Code (`claude-opus-5-5`); GPT read-only
was the first reviewer for Claude-written work.

| Round | Range | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|
| 1, review | `451d356..2a7ecee` | codex | gpt-6.2-sol | high | FAIL: 8 major |
| 2, verification | `2a7ecee..47d352e` | codex | gpt-6.2-sol | high | FAIL: 3 major, 2 minor |
| 3, verification | `47d352e..4b362f2` | codex | gpt-6.2-sol | high | FAIL: 2 major, 1 minor |
| 4, verification | `4b362f2..b0ec50f` | codex | gpt-6.2-sol | high | PASS: 1 minor |

Every finding was checked against the text by the controller; none was
refuted. Round 1 found:

- the seat's interaction with embargoed work, and with security disputes;
- eligibility that ignored families that had taken a side;
- anonymity limited to the two positions rather than the whole brief;
- no INCONCLUSIVE outcome, and no rule for premises that fail the
  controller's check;
- the skill's stop rule, which could close a review while a judgement was
  pending;
- model tables that contradicted the new tiers;
- no evidence path for a seat without tools.

Rounds 2 and 3 found partial fixes. The main ones:

- with no eligible family, the change now waits, rather than the owner
  ruling on a fact;
- every family is filtered before the first in order is taken;
- one promotion rule now applies to every family;
- a disputed fact can be closed as moot when the work stops depending on it;
- technical names stay only when they do not reveal who took a side.

The stop rule allows two fix rounds. After round 3, the owner approved one
more, limited to wording, and round 4 passed. Its one minor finding was
fixed in `17e02c4`, and checked by the controller: a judgement can be
recorded as moot before or after the seat runs.

Round 1 was a fresh session, and rounds 2 to 4 resumed it with the model
and effort passed again: a resumed codex session does not keep its model.
This was found when an unpinned resume in another review served
`gpt-6.2-sol` at `high` in place of `gpt-6.2-astra` at `xhigh`. That run
was recorded as inconclusive for its seat, and the skill now says so.

## The first worked example

The rule was prompted by a research question settled the same day:
whether an AFS client's cache may be shared between local users.

- Two independent research answers agreed that a fully shared store has a
  cross-user integrity problem. They disagreed on the default: tiered pools
  shared by default (A), or private per credential context by default (B).
- A third family (DeepSeek, `deepseek-flash`, effort `max`, with web
  access) was given both answers as A and B, with no names, and ruled B.
- The controller checked the two deciding facts at their sources:
  - rxkad never protects the integrity of the data payload (an
    implementer's statement on the openafs-devel list, 2019-12-10);
  - the rxgk AFS draft describes a malicious user poisoning a multi-user
    cache (draft-wilkinson-afs3-rxgk-afs, section 7), and requires that no
    individual user can determine the key protecting operations that
    affect the cache (version -04, section 6).
