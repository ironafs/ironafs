<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: the AFS directory format, the version from documents only

## The change

Class S. Issue #56 (plan id M2a-05).

- `docs/specs/directory-format.md`: the AFS directory object as the
  client reads it from a fileserver. The layout (pages, records, the
  directory header, entries, the name hash) from two leads, an expired
  Internet-Draft (I1) and a reference page (F1); the caps, each a named
  constant; the decoder's checks in a fixed order (length, page headers,
  record indexes, chains, spans, names, buckets, uniqueness, `.` and
  `..`); the arithmetic; a closed error set whose locations are
  positions, never values read from the object; golden vectors, round-trip
  properties, a reject corpus and a fuzz target; open questions Q1 to
  Q11.
- `docs/specs/README.md`: register of editions rows E3 (I1) and E4 (F1),
  and the index row.
- **Written by** `claude-code:claude-opus-5-5` (drafting, and the fixes
  of every round).

**This is the documents-only version.** Every layout fact in section 5.1
is UNVERIFIED: no published source found so far may fix the layout on
its own, and nothing is coded from it. The reader's checks are IronAFS's
own and are meant to hold whatever values the layout is confirmed to
have. The reviews below judged that; they did not and could not judge
whether any layout value is right. Issue #56 stays open: it closes only
when the revision backed by captures from the observation lab (#51) is
reviewed and accepted (section 13, Q1; ADR 0004, C6, point 3).

## Rounds

All rounds ran on 2026-11-21 (UTC), read-only.

The required seats (class S, Claude-written text): design review by two
non-author families. GLM took the first-reviewer seat and Gemini the
second family. Gemini's class S pin is `gemini-4-carbon-high`; this
review ran `gemini-4-carbon-medium` by the owner's choice. Both
`glm-5.3` high and `gemini-4-carbon-medium` passed the first calibration
of the review lanes on IronAFS material (#48). The agy lane does not
echo the served model; the model shown for Gemini is the one requested.

| Round | Text or range | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|
| 1, review | `d8e0cd4..513dccc` | glm | glm-5.3 | high | PASS: 4 minor |
| 1, review | `513dccc`, the specification inline | agy | gemini-4-carbon-medium | (in the model name) | FAIL: 3 major, 5 minor |
| 2, fix verification | `513dccc..02088b0`, the specification and the diff inline | agy | gemini-4-carbon-medium | (in the model name) | PASS: 1 minor |

Round 1's twelve findings were fixed in `02088b0`. Round 2's minor
finding was fixed in the commit that adds this record. Under the
workflow's stop rule, minor findings are fixed and recorded, with no
further round.

GLM read the repository. Gemini read the specification inline, without
the README, the ADRs, the claim record or the leads. Neither lane fetched
I1 or F1; their uncertain items say so (see "Not verified by these
rounds").

## Findings

Some findings overlap: GLM 1 and Gemini 1 are one defect; GLM 3 and
Gemini 6 overlap, and so do GLM 4 and Gemini 8.

GLM, round 1, all fixed in `02088b0`:

- **Minor:** a lookup name with an embedded zero octet had no error
  kind: 5.6, point 3 deferred to 5.4, point 10, which says the case
  cannot occur on decode. Fixed: a new kind, `NameHasZero`, for lookup
  only, named in 5.4, point 10, 5.6, point 3 and R25.
- **Minor:** the ledger follows the XDR profile's method, which separates
  a request past the budget from a fallible reservation the allocator
  refuses, but the error set had no kind for the second. Checked against
  [xdr.md](../specs/xdr.md), 5.2, point 9. Fixed: `AllocationFailed`
  added to section 7 and to 5.2, point 3.
- **Minor:** section 7, point 2 said an error carries the record index
  where a check failed and never a value read from the object; for an
  index out of range, the index is such a value. Fixed: a location is a
  position, never a value, and a table gives each kind's location; the
  index errors carry the object offset of the field that held the index.
- **Minor:** the note on which golden vectors rest on the bucket rule
  left out G6, whose high-octet name has a bucket that depends on how
  Q5 is settled. Fixed differently from the suggestion: G6's name is now
  one whose bucket is the same under both readings Q5 leaves open, and
  the note says every vector holding an entry (G2 to G12) rests on the
  bucket rule.

Gemini, round 1, all fixed in `02088b0`:

- **Major:** the same defect as GLM's first finding, and in addition a
  lookup error could not supply the object location every error had to
  carry. Fixed with GLM's first finding: `NameHasZero`, lookup names
  each kind directly in a stated order, and lookup errors carry no
  location, since they describe the caller's name, not the object.
- **Major:** the check order let a decoder walk all chains before
  computing spans, so an entry whose first record lies inside another
  entry's span could fail with an index or termination error instead of
  `SpanOverlap`, after reading octets that 5.4, point 5 says are never
  read. Fixed: points 1 to 5 apply once; points 6 to 13 apply to one
  record index at a time, in walk order, each passing every check before
  the next index is read; an index inside a decoded span is
  `SpanOverlap` before any octet of its record is read, and so is each
  continuation record; R13 covers both orders in which the two entries
  are reached. Departure from the suggestion: the steps keep the
  specification's point numbers instead of a new numbered loop.
- **Major:** `.` and `..` were in the decoded set, so in listings, yet
  lookup said they were not looked up here, which broke 5.6, point 1 and
  property P5. Fixed: lookup finds `.` and `..` like any other entry when
  the object holds them; what they resolve to is the namespace's (#80),
  and the target lookup returns for `..` is not a verified parent.
- **Minor:** `dir.records_max` was defined per object while 5.2 called
  every cap a constant, and the arithmetic width of the product was not
  stated. Fixed: `dir.records_max` is a constant; the records present in
  an object come from its length with checked `usize` arithmetic, and
  every index is checked against them; every read takes a bounds-checked
  slice (6, point 7).
- **Minor:** the relations among the UNVERIFIED layout constants that the
  reader's safety needs were never stated. Fixed: 5.2, point 4 lists them
  as compile-time assertions, and section 3, point 3 qualifies the safety
  claim by them.
- **Minor:** only `BadTag` said which location it carries. Fixed with
  GLM's third finding: the per-kind location table. Departure from the
  suggestion: entry checks carry the record index of the entry's first
  record (for `SpanOverlap` and `DuplicateName`, the entry reached
  second).
- **Minor:** a cumulative ledger sized only for the decoded result could
  refuse a valid maximal directory, once working state and growth were
  counted. Fixed: the ledger counts result and working state; each buffer
  is reserved once, fallibly, for a bound from the length, so nothing
  grows; `dir.budget` is a constant set in the capture-backed revision,
  and a new golden vector, G12, decodes a maximal object within it.
- **Minor:** G6's high-octet name had no defined bucket while Q5 leaves
  open how octets of 0x80 and more count, and the dependency note left
  out vectors. Fixed differently from the suggestion (fixing a
  provisional reading would code a guess for Q5): G6's name has the same
  bucket under both readings. The note's second point was accepted, as
  for GLM's fourth finding.

Gemini, round 2, fixed in the commit that adds this record:

- **Minor:** 5.2, point 3 reserved buffers "before decoding starts",
  which a decoder could read as right after the length check, so that a
  garbage object of `dir.object_max` octets sized several megabytes of
  buffers before its page headers were checked, and the order between
  the header errors and `BudgetExceeded` or `AllocationFailed` was
  unstated; two cross-references were also off by one point. Fixed: 5.4,
  point 4 now makes the reservations only after every page's header has
  passed and before the walk reads any bucket head; 5.2, point 3 says the
  same; the cap table cites 5.4, point 2 for `dir.object_max`, and 5.4,
  point 14 cites 5.6, point 2 for enumeration order.

No finding was refuted or deferred.

## Not verified by these rounds

Both lanes listed what they could not check. None is a defect in the
text; each is settled elsewhere:

- whether I1 and F1 say what the specification attributes to them, and
  whether any layout value is right: the capture-backed revision (#51;
  section 13, Q1);
- the register rows' statements about outside sources (the draft's
  status, the hashes of the texts read, the reference page's licence),
  and that the issues cited cover what the specification says they do;
- Gemini, reading inline, could not check the README's rules, the ADR
  citations or the claim record; GLM read them.

## Open clean-room items (the owner's decisions)

Clean-room policy is the owner's (ADR 0001). These items are open, not
decided:

- **Q11, allowlisting I1.** Whether I1's passages may be allowlisted once
  fixtures agree with them, or fixtures alone are cited. Until decided,
  no passage is allowlisted and every fact from I1 is UNVERIFIED. No code
  waits on it: a confirming fixture is itself a route (ADR 0004, N31,
  point 1).
- **I1's Appendix B.** The draft's text, fetched whole to write this
  version, includes Appendix B, a code component. The specification and
  the register exclude it by name: it is never read for a specification,
  never put in a model's prompt and never cited, and nothing in this
  change is taken from it. What, if anything, that exposure requires
  under ADR 0001 is the owner's decision, still open.
