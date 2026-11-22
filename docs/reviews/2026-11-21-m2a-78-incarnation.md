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
    questions Q1 to Q11.
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

## Second-family pass (Gemini, then DeepSeek)

After the stop rule, a second pass on the reviewed head `11cec86`,
read-only:

| Seat | Model | What it read |
|---|---|---|
| gemini | gemini-4-carbon-medium | the diff only, inlined in two parts, with no file access |
| deepseek | deepseek-flash | the repository at `11cec86`: it checked each Gemini finding against the text (CONFIRMED, REFUTED or UNVERIFIABLE) and added its own |

Gemini's finding on row E15's hash was not carried forward: the row
holds a 64-digit SHA-256 value (checked). The items DeepSeek did not refute, F1 to F13,
were each checked against the text by the implementer before any
change. The fixes are in the commit on top of `11cec86` that adds this
section; **they have not been reviewed.**

| Item | Severity | Disposition | Origin | Kind |
|---|---|---|---|---|
| F1: row E8's corroboration cell named only vl-fs-read.md's Q7 and Q8 for the facts this specification added | minor | fixed: the cell adds this specification's section 13, Q1 and Q9 | pre-existing (`67ee757`) | consistency between parts |
| F2: the Q10 summary here, 5.6, point 12, and Q10 said a reset never arrives on a context's own call; an abort saying the server is restarting does | minor | fixed: all three say the abort ends the owning context's own call but is weak input that ADR 0006, decision 1, point 5, bars from retiring anything, and that the callback reset is held back by the conservative reading of C24, point 2 | introduced by an earlier fix round (`7daee8f`; the summary in `11cec86`) | correctness |
| F3: a verify-by-refetch reply was not checked against the fence tuple and epoch it was sent under before its branches, and the T7 or T8 reply was installed "with the new fence tuple", which could hide a break that arrived while it was in flight | major | fixed: 5.6, point 6, checks the reply's send-time epoch and fence tuple first, in one step under the volume's lock with the branch it leads to; a reply that fails retries and triggers nothing; 5.6, point 3, step 3, installs the T7 or T8 bytes with their send-time tuple, only the volume's break counter taking step 2's value. New AC8 variant | introduced by an earlier fix round (`fb15722`) | correctness |
| F4: a file-service reply carried no site, so a failover to another site with no new VL reply was not T2, and a record made "new" from a file-service reply had no value for the VL fields | major | fixed: a file-service observation carries the site its call was sent to (5.4, point 3); version ids and type gain a "not observed" state, which differs from any later value (fail safe); `entry_flags` gains "not kept" and provenance a "none" code (234 bytes); an ambiguous outcome clears recorded values the observation does not carry. New AC3 variant. The finding's proposal to carry the routing VL entry's ids too was not taken: that entry may not be the context's own observation | pre-existing (`67ee757`) | correctness |
| F5: 5.4, point 4, made every reply during a release ambiguous again, invalidating handles each time, and a flag clearing had no trigger | major | fixed: any change of the release state, either way, is T3; while the record's release state is not "none", nothing fetched from the volume is cached, so an equal flag need not be ambiguity again. AC4 and the comparison property updated | pre-existing (`67ee757`) | correctness |
| F6: T8 fired only on a verify by refetch, so a status call that saw a lower version first applied the file-level rule and the volume's epoch never changed | major | fixed: T8 and 5.6, point 8, cover a status reply in the context's own call that shows a version lower than a chunk under wide suspicion; no bytes installed. New AC16 variant | introduced by an earlier fix round (`7daee8f`) | correctness |
| F7: K contexts' ambiguity raises the shared volume break counter K times, which fences every context's fetches in flight and could end other contexts' reads with `Stale` | major (UNVERIFIABLE) | specification item, not changed: whether it is reachable depends on C16, point 2's retry bound and on how often a context can be made to observe ambiguity, which the callback specification (#66) and a rate rule for these raises would settle; a separate suspicion counter is a mechanism of its own | introduced by an earlier fix round (`7daee8f`) | missing mechanism |
| F8: AC2 never compared a second observation with the record | minor | fixed: AC2 refreshes the VL entry with equal values between the reads and asserts the field-by-field comparison | pre-existing (`67ee757`) | other (test) |
| F9: ADR 0006 does not exist at the reviewed head | major | base dependency, resolved at landing: ADR 0006 is on `main` (`1d75375`); not changed in this round | pre-existing (base) | consistency between parts |
| F10: vl-fs-read.md called `site.partition` an attribute only, while this specification compares it | minor | fixed: vl-fs-read.md's use column names the comparison | pre-existing (`67ee757`) | consistency between parts |
| F11: the table's caps, room-making, reclaim and holder changes had no stated atomic step, and `last_used` was set only at the outcome | major | fixed: new 5.2, point 7: each is one atomic step on the table, including 5.6, point 3, steps 1 to 3 for an ambiguous outcome; `last_used` is set at the lookup. A2 and the model test (interleavings) updated | pre-existing (`67ee757`), widened by the reclaim of `fb15722` | correctness |
| F12: the threat row said a forged value costs the observing context only a refetch | minor | fixed: it names the incarnation change's effects on that context's handles, snapshots and journal records, and their explicit recovery | pre-existing (`67ee757`) | other (security analysis) |
| F13: "new" and "confirmed" did not set `last_outcome`, and "new" did not set `last_used` | minor | fixed in 5.6, point 1 | introduced by an earlier fix round (`7daee8f`) | correctness |

### Scoped round

The fixes above were committed as `de31e4d` and verified on
`11cec86..de31e4d`: `glm-5.3` (high) passed F7's disposition as a
specification item; `deepseek-flash` found every F item fixed, and four
new findings in F4's fix. Each was checked against the text first; all
four hold. They share one cause: F4's fix took a file-service reply's
site from the VL entry the call was routed by, an interface
vl-fs-read.md does not define. Rather than define it here (a new
mechanism), the site part of F4's fix is withdrawn and the failover
case becomes section 13, Q11. The fixes are in the commit on top of
`de31e4d` that adds this subsection; **they have not been reviewed.**

| Item | Severity | Disposition | Origin | Kind |
|---|---|---|---|---|
| S1: the file-service site's partition and release state came from the routing VL entry, which the location cache may hold from another context, against 5.1.2's "no value read from another context" | major | fixed: a file-service observation carries no compared value (5.4, point 3); a record made "new" from one holds every compared value "not observed" (`site_server`'s tag and `site_release` gain that state; still 234 bytes) | introduced by an earlier fix round (`de31e4d`, F4) | correctness |
| S2: the file-service site's server was the connection's address, while a VL entry may name the same server by identity, which 5.4, point 3, reads as a difference: a spurious T2 on every refresh | major | fixed by S1's withdrawal: no server is taken from a file-service reply | introduced by an earlier fix round (`de31e4d`, F4) | correctness |
| S3: one provenance entry covered the VL values and the file-service site, which could not both be recorded | minor | fixed by S1's withdrawal: a file-service observation sets only the `volsync` provenance entry; 5.4, point 2, says a provenance entry none of whose values the observation carries is "none" | introduced by an earlier fix round (`de31e4d`, F4) | consistency between parts |
| S4: vl-fs-read.md, 5.6, point 3, was cited for routing a call by a site of an entry, which it does not define | minor | fixed by S1's withdrawal; the missing routing and attribution interface is section 13, Q11, and the failover with no new VL reply is a stated residual (5.1.1, reason 3; T2; 5.6, point 7; section 8; 10.6). The AC3 failover variant is removed | introduced by an earlier fix round (`de31e4d`, F4) | missing mechanism |

The scoped round (`de31e4d..f0c735a`) was verified: `glm-5.3` (high)
passed the Q11 residual; `deepseek-flash` passed with three minors that
the withdrawal introduced. Under the stop rule they were fixed and
recorded, with no further full round, in the commit on top of
`f0c735a` that adds these rows; **those fixes have not been reviewed.**

| Item | Severity | Disposition | Origin | Kind |
|---|---|---|---|---|
| S5: section 3, point 6, and 5.1.1, reason 3, still said a replica switch costs a refetch, without T2's qualifier that the context's own VL entry shows it | minor | fixed: both now say so, and section 3, point 6, names the unseen failover (Q11) | introduced by an earlier fix round (`f0c735a`) | consistency between parts |
| S6: section 8 and Q11 bounded the shared epoch by "its next own VL observation", but a refresh naming the site the record holds confirms it | minor | fixed: both bound it by an own observation that shows a different site, and say a refresh naming the held site leaves the sharing in place for as long as the record lasts | introduced by an earlier fix round (`f0c735a`) | correctness |
| S7: a record made from a file-service observation holds `site_release` "not observed", which 5.4, point 4, read as not "none", so the volume would stay uncached until the context's next VL observation | minor | fixed, a narrower fix than the finding's framing: 5.4, point 4's rule now applies to a release flag only; "not observed" caches as "none" does, which is safe because the context's next own VL observation is ambiguity whatever it shows (5.4, point 3), so nothing cached under that epoch outlives it. The comparison property and a new AC6 variant say so | introduced by an earlier fix round (`f0c735a`) | correctness |

## Decision 29 amendment

The controller's decision 29 (2026-11-21) set the cache fence: the
callback specification's five values and the local volume epoch as the
sixth; the credential and rights generations in the request's ticket
only; the fence table's lock a leaf in ADR 0004 C39, point 4's order.
The callback specification carries it (#119, its 5.6, points 2 and 3,
read at `63313e5`), and so does the fence code of #91 (read on its
branch). This specification was brought into line with both, in the
commit on top of `aa4fc26` that adds this section:

- section 4: the local volume epoch is a value of the fence tuple, and
  its change counts among wide suspicions; a chunk recorded under
  another epoch is also unreachable by its key, so it is fetched anew,
  not verified by refetch;
- 5.6, point 1: a new paragraph on the fence table's copy of the epoch:
  every outcome that leaves a record ("new", "confirmed" and
  "ambiguous") writes it, in the same step, under the volume's lock,
  with the fence table's lock taken inside; the adoption rule at the
  install whose own observation wrote the epoch, with the eviction latch
  and the volume break counter condition, as the callback specification,
  5.6, point 3, states them;
- 5.6, point 3, step 3: the in-flight fetches fail through the epoch's
  place in the tuple, not through a separate record; the T7 or T8 reply
  is installed with its send-time tuple in which the volume's break
  counter and the epoch take their new values;
- 5.6, point 6: the refetch's fence check names the epoch as one value
  of the tuple, and the fence table's lock as taken inside the volume's;
- section 2 says which of the callback specification's duties are
  restated here.

**Not reviewed.** No lane has read this amendment.

**Raised for the callback specification and the fence code, not
changed here:**

- A request on an unrecorded volume (5.2, point 5) or from an
  unauthenticated context (5.4, point 1; AC13) makes no observation that
  writes an epoch. By the callback specification, 5.6, point 3, at
  `63313e5`, a fetch such a request sends while its context holds no
  copy records no epoch and fails its check, and adoption cannot rescue
  it, since its own observation writes none (adoption does rescue the
  first fetch of a volume, whose "new" outcome writes the epoch). So
  C16, point 2, would end it with `Stale` instead of the uncached read
  this specification and C20, point 1, describe. Whether a reply that is
  never cached is fenced on the other five values only is the callback
  specification's to state.
- The callback specification, 5.6, point 3, has the copy written when
  the epoch is set or changed; the fence code writes it on "confirmed"
  too. That write is needed: without it, once the table evicts a copy
  while the record lives, the context records no epoch until the next
  epoch change, and every fetch fails. This specification follows the
  code.
- Evicting an evidence record (5.2, point 4) leaves the fence table's
  copy of its abandoned epoch in place. That is safe, because a chunk
  under the abandoned epoch is unreachable by its key, but it costs one
  failed install after the next "new" outcome, and the callback
  specification at `63313e5` did not say so.

### Alignment with the callback specification at `35ad5cb`

The callback specification took up all three items raised above (its
5.6, point 3: the write on "confirmed"; an evicted evidence record's
last copy; "A reply that is never cached"). A reviewer of it also found
the adoption rule's proof false: an epoch can be written with no
ambiguity and no volume break (an outcome "new" after the evidence
record was evicted), so the counter condition does not show which
observation wrote the epoch adopted. That argument, which this
specification repeated, is withdrawn. Changed here, in the commit on top
of `e19011f` that adds this subsection:

- 5.6, point 1: the paragraph on the fence table's copy is split into
  its write, its adoption and the reply that is never cached:
  - every outcome that sets, confirms or changes the epoch writes the
    copy, in the same step, under the volume's lock, the table's lock
    inside;
  - a write is refused only when the fence table is exhausted, when
    every check already fails;
  - an evicted record's last copy stays until the next "new" outcome
    replaces it, at the cost of one failed install;
  - the install whose own observation wrote or confirmed the epoch
    makes that write and the adoption one step under the volume's lock,
    citing the callback specification's 5.6, point 3, and its section
    13, Q11 (the table does not check which epoch it adopts);
  - an unrecorded volume and an unauthenticated context hold no copy;
    their replies are never cached and are fenced on the other five
    values.
- 5.2, point 5: the one-request epoch an unrecorded volume took is
  removed. It existed only in this specification and not in the fence
  code. The unrecorded request takes no epoch and is fenced as above.
  5.4, point 1, says the same of an unauthenticated call.
- 5.6, point 3, step 3: an epoch changed for any cause, a record made
  again after eviction among them, fails the fetches in flight.

Section 4 already listed the epoch among wide suspicions. **Not
reviewed.**

In the list above, the bullet "an unrecorded volume and an
unauthenticated context hold no copy" described the text of `374d924`,
which the scoped round below corrects.

### Scoped round on the alignment

The alignment (`aa4fc26..374d924`) was verified: `glm-5.3` (high)
passed with one minor; `deepseek-flash` failed with one major and one
minor. Each was checked against the callback specification at
`35ad5cb`, 5.6, point 3, and the fence code of #91 first; all three
hold. They are fixed in the commit on top of `374d924` that adds this
subsection; **not reviewed.**

| Item | Severity | Disposition | Origin | Kind |
|---|---|---|---|---|
| D1 (DeepSeek): 5.6, point 3, step 3, failed every fetch in flight on an epoch change, and 5.6, point 1, grounded the five-value rule in "holds no copy", which is false when an evicted record's last copy remains; the callback specification grounds it in the reply never being cached | major | fixed: step 3 now fails, on an epoch change, the fetches whose reply is to be cached, and says a reply that is never cached is checked on the other five values; 5.6, point 1's bullet is now "A reply that is never cached", grounded in that, whether or not the context holds a copy; 5.2, point 5, and 5.4, point 1, cite it | introduced by an earlier fix round (`374d924`) | consistency between parts |
| D2 (DeepSeek): this record said no adoption can rescue a request with no evidence record, said the callback specification did not cover an evicted record's last copy, and said it took up two items | minor | fixed: the raised item names requests whose own observation writes no epoch, notes that adoption rescues a volume's first fetch, and dates the statements to `63313e5`; the alignment says all three items were taken up | introduced by an earlier fix round (`e19011f`, `374d924`) | other (record) |
| G1 (GLM): "hold no record, so no copy" contradicted the "Write" bullet's lingering copy | minor | fixed with D1 | introduced by an earlier fix round (`374d924`) | consistency between parts |

## Q10: the server reset

C25, point 3 (Working), asks the observing context to take a fresh
local epoch after a server reset. A reset reaches the client only
through the callback service, which arrives on no context's call, or
through an abort saying the server is restarting, which ends the owning
context's own call. Neither is an observation. C24, points 3, 4 and 6
(Firm), keep such weak input from changing another context's or shared
state. For the owning context's own epoch, the abort is barred by ADR
0006, decision 1, point 5 (Firm: an abort retires nothing, and an
incarnation change retires handles and quarantines journal records),
and the callback reset by the conservative reading of C24, point 2,
whose list of what weak input may do has no epoch change and is not
called closed. The specification keeps C24: a reset leads to a refetch
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
(rows E10, E11 and E55) with their hashes:

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
- At the commit with the second-family fixes: `cargo xtask check`
  passes (EXECUTED, macOS, 2026-11-21 UTC). Those fixes were not
  verified by any lane; see "Second-family pass".
- At the commit with the scoped round's fixes: `cargo xtask check`
  passes (EXECUTED, macOS, 2026-11-21 UTC). Not verified by any lane;
  see "Scoped round".
- At the commit with the scoped round's minor fixes: `cargo xtask
  check` passes (EXECUTED, macOS, 2026-11-22 UTC). Not verified by any
  lane.
- At the commit with the decision 29 amendment: `cargo xtask check`
  passes (EXECUTED, macOS, 2026-11-22 UTC). Not verified by any lane.
- At the commit aligning it with the callback specification at
  `35ad5cb`: `cargo xtask check` passes (EXECUTED, macOS, 2026-11-22
  UTC). Not verified by any lane.
- At the commit with the scoped round on the alignment: `cargo xtask
  check` passes (EXECUTED, macOS, 2026-11-22 UTC). Not verified by any
  lane.

## Landing

This change was brought up to date with `main` (`dde6ea4`) on
2026-11-22. Its branch was cut before the XDR, Rx, directory format,
seams, VL and FS read, DNS client and test strategy specifications and
ADR 0006 landed, and it carried older copies of `xdr.md`, `rx.md`,
`template.md`, `vl-fs-read.md` and their review records. Every file
starts as `main` has it; this change's own files are
`volume-incarnation.md` and this record. Three files were combined by
hand, keeping all of `main`'s text:

- `docs/specs/vl-fs-read.md` takes this change's one amendment (the
  second-family round's F10: `site.partition`'s use column names this
  specification) on `main`'s copy, which differs from the branch's
  older copy only in its register ids and that line;
- `docs/reviews/README.md` gains this record's row;
- `docs/specs/README.md` gains the specification's index row, in the
  index's title order, and this change's register rows.

The register rows were merged or renumbered, because `main` already
registers two of the three documents. This branch's E8 and E9 are
`main`'s E10 and E11. E10 keeps this change's addition (the facts of
`vos_create(1)` and `vos_examine(1)` that this specification uses, its
Q1 and Q9, and `volume-incarnation.md` in "Cited by"). Row E14 (four
more reference pages of the same set) was merged into E10: E10 gained
the pages `vos_restore(1)`, `vos_release(1)`, `vos_move(1)` and
`vos_backup(1)` with their SHA-256 values, row E14's corroboration
text and the sections used. Row E15 (another page of the same
Administration Guide edition) was merged into E11: E11 gained the page
"Dumping and Restoring Volumes" with its SHA-256 value, row E15's
corroboration text, the section used and `volume-incarnation.md` in
"Cited by". Row E16 (the dump stream format document) became E55, a
new row; its "as row E5" became "as row E7", `main`'s id for the
registry. Ids were renamed in `volume-incarnation.md` (section 12 and
12.2) and in this record's "Clean-room note"; "The change", "Findings"
and "Second-family pass" above still describe the rows as they were
reviewed (E8, E14 to E16). Nothing else in the reviewed text changed.

Its citations of `vl-fs-read.md` (sections 1, 5, 7, 12.2 and 13) and of
the callback specification were re-read against `main`'s copy and the
callback branch at `e9eee07`: neither renumbered nor reworded a cited
section (`main`'s `vl-fs-read.md` and the callback specification differ
from the copies this change was aligned with only in register ids). It
cites no section of `rx.md` or `xdr.md`.
