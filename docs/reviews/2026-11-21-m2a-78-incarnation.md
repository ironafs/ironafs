<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: volume incarnation evidence and the restore discriminator, the version from documents only

## The change

Class S. Issue #78 (plan id M2a-06).

- `docs/specs/volume-incarnation.md`, which covers:
  - which values in the replies M2a decodes carry incarnation evidence,
    and what the documentation says a move, a release, a restore and a
    re-creation do to each;
  - the gate's answer, INSPECTED: no value is known to tell a restore
    made while the client was down, so no clean chunk is reused across
    a restart;
  - the bounded per-context evidence record, with its floor, idle
    reclaim and pinning;
  - the comparison and its four outcomes, and the triggers of ambiguity;
  - how weak input reaches other contexts: only as suspicion, through
    the fence tuple;
  - verify by refetch after a server-wide or volume-wide event;
  - the restart rule;
  - the arithmetic, errors, threats, acceptance cases, and open
    questions Q1 to Q10.
- `docs/specs/README.md`: register of editions rows E14 to E16, an
  addition to row E8, and the index row.
- **Written by** `claude-code:claude-opus-5-5` (drafting, and the fixes
  of every round).

**This is the documents-only version.** Every fact here about what a
server does on a move, release, restore or re-creation is a lead from
documentation and is UNVERIFIED. Every wire layout it touches is MISSING
in `vl-fs-read.md`. The reviews judged the rules that do not depend on
those facts: fail-safe behaviour, bounds, how the rules fit ADR 0004
C24 to C26 and ADR 0006, and the labels. They did not judge any wire
value. Issue #78 stays open. It closes only when the revision backed by
the observation lab's stage B record (#51) is accepted (ADR 0004, C6,
point 3). The restore experiment against the test cell is to record
the answer as EXECUTED.

## Rounds

All rounds ran on 2026-11-21 (UTC), read-only.

The required seats (class S, Claude-written text): design review by two
non-author families. GLM took the first-reviewer seat and DeepSeek the
second family. `glm-5.3` at high effort passed the first calibration of
the review lanes on IronAFS material (#48). DeepSeek was not part of
that calibration. Each lane's served model was read from its own
output.

| Round | Range | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|
| 1, review | `5243a7c..67ee757` | glm | glm-5.3 | high | PASS: 5 minor |
| 1, review | `5243a7c..67ee757` | deepseek | deepseek-flash | max | FAIL: 2 major, 9 minor |
| 2, fix verification (resumed session) | `67ee757..7daee8f` | glm | glm-5.3 | high | PASS: 4 minor |
| 2, fix verification (resumed session) | `67ee757..7daee8f` | deepseek | deepseek-flash | max | FAIL: 2 major, 8 minor |
| 3, fix verification (resumed session) | `7daee8f..fb15722` | glm | glm-5.3 | high | PASS: 3 minor |
| 3, fix verification (resumed session) | `7daee8f..fb15722` | deepseek | deepseek-flash | max | FAIL: 2 major, 2 minor |

Round 1's findings were fixed in `7daee8f` and round 2's in `fb15722`.
After round 3 the workflow's stop rule was reached. Round 3's two
DeepSeek majors were defects that round 2's own fix introduced, in its
room-making rules. They and every round-3 minor were fixed minimally in
`814906d`. **That last fix was made after the final verification and has
not been reviewed.** So DeepSeek's round-3 FAIL is its last verdict, and
DeepSeek is not credited as a reviewer of the final text. GLM passed
every round; its round-3 findings were three stale cross-references,
fixed in `814906d`.

## Findings

### Round 1, GLM (all fixed in `7daee8f`)

- **Minor:** 5.2 and 5.4, point 2, gave different outcomes for a
  context that cannot keep a record because the table is full. Fixed
  with DeepSeek's same finding: a single "unrecorded" outcome.
- **Minor:** section 3 called a value that cannot be read ambiguity,
  while 5.4 makes an undecodable reply a non-observation. Fixed: such a
  reply is not an observation; it changes no record and serves nothing.
  Section 3, 5.4, 5.7 and section 7 now agree.
- **Minor:** another context's ambiguity sent chunks to the
  verify-by-refetch rule, but that rule's stated scope covered only
  fence-tuple suspicion. Fixed: another context's ambiguity now raises
  the volume's break counter, which is a wide suspicion by definition.
- **Minor:** the threat row bounded forced refetches by the advance
  tick, but a break naming a volume renews suspicion at the callback
  rate bucket. Fixed: the cost is one refetch per use, at most double
  the bytes read, with renewal bounded by the callback specification.
- **Minor:** the claim-record source row named a section it is not
  used in. Fixed.

### Round 1, DeepSeek (all fixed in `7daee8f`)

- **Major:** the record had no suspect state, reason or last outcome,
  though 5.6, points 4 and 11, used them, and a confirmed outcome could
  be read as clearing suspicion. Fixed: display fields were added;
  suspicion that governs serving rides on the fence tuple; a confirmed
  outcome clears nothing.
- **Major:** C25, point 3, names a server reset among the cases that
  need a fresh local epoch, but the text gave it no trigger and pointed
  at a paragraph about aborts. Fixed:
  - new 5.6, point 12: a reset reaches no context's own call, and C24
    keeps weak input from changing epochs, so a reset leads to verify by
    refetch;
  - new trigger T8: a lower data version under a wide suspicion is the
    reader's own volume-level ambiguity;
  - the remaining residual is in section 8 and new Q10.
- **Minor:** a journal record could outlive its evidence record without
  being quarantined. Fixed: a record whose context holds a handle,
  snapshot or journal record on the volume is pinned. New 5.8, point 3,
  covers records from an earlier run, quoting C28, point 3.
- **Minor:** the summaries called an unreadable value ambiguity, and
  7.1 contradicted 7.3. Fixed with GLM's same finding.
- **Minor:** the outcome at the total cap was contradictory. Fixed:
  "unrecorded".
- **Minor:** trigger T6 (restart) could never fire. Fixed: T5 and T6
  are marked as not triggers, and keep their numbers.
- **Minor:** a C25 point was labelled Firm. Fixed: Working.
- **Minor:** wrong internal references, and an unlabelled claim about
  the size of the volume sync record. Fixed: that size is now labelled
  a guess, MISSING.
- **Minor:** a value an observation does not carry erased the recorded
  copy. Fixed.
- **Minor:** the record's size bound could not be reproduced. Fixed:
  every field now has its width.
- **Minor:** register row E14 understated the specification's reliance
  on the creation-date option. Fixed.

### Round 2, GLM (all fixed in `fb15722`)

- **Minor:** the incarnation change raised the break counter after the
  event, against C16, point 1. Fixed with DeepSeek's same finding.
- **Minor:** an ambiguous outcome could drop the holder count and make a
  held volume evictable. Fixed: the holder count is kept.
- **Minor:** the scope still said "three outcomes". Fixed.
- **Minor:** a cross-reference was stale after renumbering. Fixed.

### Round 2, DeepSeek (all fixed in `fb15722`)

- **Major:** the order in 5.6, point 3, queued the event before the
  break counter was raised (C16, point 1, Firm). Fixed: the epoch, the
  counter and the fence change in one core step; only then is the event
  queued.
- **Major:** the "unrecorded" state had no way to make room, because
  only a context's own records could be evicted. Fixed: a per-context
  floor outside the shared cap; reclaim of unpinned shared-part records
  from a context idle for 600 seconds, as C27, point 4's budget policy;
  removal of a retired generation's records; a stated bound.
- **Minor:** the key lacked the cell's protocol identifier. Fixed.
- **Minor:** the suspicion display could be wrong. Fixed: it shows the
  last suspicion found, and current suspicion is computed from the fence
  tuple.
- **Minor:** section 3 said any difference changes the epoch after a
  reset. Fixed: only a lower version, or other bytes under an equal one.
- **Minor:** the reply that caused T7 or T8 would fail its own fence.
  Fixed: it is exempt and is installed under the new epoch.
- **Minor:** the cost of a fenced fetch was understated. Fixed: C16,
  point 2's bounded retries, which can end in `Stale`.
- **Minor:** the reasoning over-read C24 for a context's own epoch.
  Fixed: C24, points 3, 4 and 6, bar changes across contexts; for a
  context's own epoch, point 2's list is open and is read
  conservatively.
- **Minor:** an overflow of the holder count had no result. Fixed: busy,
  never wrapped.
- **Minor:** an acceptance case treated a directory listing as a plain
  read. Fixed.

While fixing the room-making major, C18, point 9, was found to detach
journal records from a retiring generation. A retired generation's
detached records are now treated as records from an earlier run:
unconfirmed, and kept quarantined. Both lanes judged this sound in
round 3.

### Round 3, GLM (fixed in `814906d`, not re-reviewed)

- **Minor:** three cross-references were stale after round 2's
  renumbering and reorder: the T5 row, and the section 9 rows for C25,
  point 5, and C16, point 1. Fixed.

### Round 3, DeepSeek (fixed in `814906d`, not re-reviewed)

- **Major:** the room-making order had no branch for a context past its
  floor while the shared part still had room, so such a context became
  "unrecorded". Fixed: each branch is explicit, the new record takes the
  evicted slot, and "unrecorded" is reached only when nothing can be
  evicted. A new acceptance-case variant covers it.
- **Major:** the floors sat outside every cap, and the number of
  contexts is not bounded in ADR 0004. Fixed: at most 1 024 floors at
  once, so the whole table is at most 24 MiB; a context past that has no
  floor. That residual is stated.
- **Minor:** the threat row said a starving context had to keep its
  records pinned; keeping them active is enough. Fixed: the row now
  states the floor as the fairness bound and gives the residual.
- **Minor:** an ambiguous outcome did not update the record's last use.
  Fixed.

## Q10: the server reset

C25, point 3 (Working), asks the observing context to take a fresh
local epoch after a server reset. A reset reaches the client only
through the callback service or an abort, never on a context's own
call. C24 (Firm) keeps such weak input from changing another context's
or shared state. The specification keeps C24: a reset leads to a refetch
of each chunk before it is served, and an epoch changes only on a
reader's own observation (T7, T8). The residual: a restore hidden by a
reset that raised every version it touched is read as ordinary changes.
New bytes are served, but open handles and journal records are not
invalidated on the reset alone.

Neither lane raised Q10 as a defect. Both judged the departure the right
reading of C24. DeepSeek questioned only the rationale's claim that C24
bars a context's own epoch change; that was fixed in round 2. **The
controller's decision** (2026-11-21): C24 is kept as the specification
reads it, and the amendment of C25, point 3's wording to match is
taken up in #53. The owner may override it.

## Clean-room note

No source code, test data or interface definition of any AFS or Rx
implementation was read for this change. The community Rx draft was not
opened. The public documents used are listed in the register of editions
(rows E8 and E14 to E16) with their hashes:

- reference pages and an Administration Guide page, published under the
  IBM Public License or with no licence stated;
- the AFS Assigned Numbers Registry's dump stream format document.

All are cited for facts only, as leads, with no passage allowlisted.

One public Internet-Draft that contains code under another licence,
draft-benjamin-extendedcallbackinfo-02, was read only before its
excluded sections:
- a scan of its headings, which shows the names of its type definitions
  and constants;
- its front matter, sections 1 and 2;
- the prose of section 4.5 before 4.5.1.

Nothing from it is cited in this specification. Neither review lane was
asked to open any excluded section.

## Open items for the owner

- **Q3:** C25, point 3's "seen offline" against ADR 0006, decision 1,
  point 5. The recommendation is an amendment of ADR 0006, decision 1,
  point 6, so that an off-line abort may mark a volume's chunks for
  verify by refetch in the owning context only.
- **Q5:** the in-run residual of a restore in place that no event
  reveals. The next step waits on what servers send on a restore (Q4).
- **Q10:** decided by the controller as above; the owner may override.

## Evidence

- At the commit that adds this record: `cargo xtask check` passes
  (EXECUTED, macOS, 2026-11-21 UTC).
- `814906d` was not verified by either lane (stop rule); see "Rounds".
