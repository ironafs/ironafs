<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Issue brief review: the specification status rule and the status correction

This record covers a batch of two issue briefs, both Class S, which came out of the M2a re-ordering of 2026-11-27:

- **The rule brief.** The proposed design issue rewrites the "Status" rule of `docs/specs/README.md` so that `draft` → `reviewed` has one reading: a stopped review never promotes a specification. It also asks for an executed reproduction of the failure. The other status rules and every independent gate are left as they are.
- **The correction brief.** It corrects each specification's status, index row and "Review" section under the landed rule, from a sha256-pinned ruling reviewed before any edit, and stages handoffs to the issues that consume those specifications. It is blocked by the rule issue.

Both were reviewed at base `67b5075abd7893255fc1c6872552c21730e3e2b3`.

- **Rule brief.** Last reviewed revision: SHA-256 `d8664629515214b96cc5726398dbb83dd79241c48336ba926904b64b6e562473`.
  - It is **not filed at this record's commit**: three findings of the OpenAI seat are open (RA1.5, RA3.2 and RA4.2, below).
  - They were left out of the scope of rounds 5 and 6 by the controller's error (see "Controller errors").
  - Under its own "Before filing" paragraph, it is filed only once they are closed and this record holds that.
- **Correction brief.** Last reviewed revision: SHA-256 `7226d24191ab21771def6bfc51f547bc26ad60a3d695c2a373cf419682f1d0d3`.
  - It is **not filed**.
  - Its blocker is recorded: one major (CA6.1) and one minor (CB6.1) on criterion 4's new return-to-ruling clause, introduced by fix round 5.
  - No further fix round is authorised.

Both briefs were written by `claude-code:claude-opus-5-5`.

## History: the earlier specification-status brief

The two briefs were split from one earlier brief, which handled the rule question and the correction together. Its review count:

| Round, brief pin | `codex:gpt-6.2-sol` xhigh, uncalibrated on IronAFS | `glm:glm-5.4` High |
| --- | --- | --- |
| 1, `190b9c61` | FAIL: two blockers, four majors, one minor | FAIL: four majors, five minors |
| 2, `a4b38d84` | FAIL: two fixed, five partly; two new majors, one new minor | FAIL: two majors partly fixed; three new majors, one new minor |
| 3, `8a7a3ad0` | FAIL: six fixed, four partly; two new majors, one new minor | FAIL: all its earlier findings fixed; two new majors, two new minors |

After round 3 (fix round 2) the stop rule applied.

- **The controller's recommendation:** stop patching. Split the rule-text change into its own Class S design, reviewed first. Redesign the acceptance criteria around an executed check. A redesign starts a new count (`docs/workflow.md` §3, step 7, "What counts").
- **Owner, 2026-11-27:** "Lets go with your recommendations".

The two briefs below are that redesign. Their count started at zero. The earlier brief's findings are history and are not carried as findings here.

## Rounds of the redesign count

Every OpenAI row ran `codex:gpt-6.2-sol` at xhigh through the OpenAI Responses API (`openai_lane.py`, research mode, read-only repository tools).

- It is **uncalibrated on IronAFS** for this role.
- Its calibration (record 2026-11-27-48-sol-xhigh-code-calibration) covers only one form of ordinary Class S code review:
  - one tool-free API request per change, on an inlined packet;
  - changes of 80 to 400 lines to `ironafs-vfs`, `ironafs-time` and `ironafs-config`.

Every GLM row ran `glm:glm-5.4` at High on the Z.ai Coding Plan through headless Claude Code, with read-only repository tools, as a focused pass.

- Its calibration (record 2026-11-27-48-glm54-document-calibration) has the scope "ordinary Class S pre-code design review of documents".
- That calibration was measured on the route "inlined packet; builtin tools and MCP disabled". These runs used repository tools, which lies outside that measured form.

How the rounds ran:

- Round 1 reviewed each brief whole.
- Each later round verified the seat's own open items on the diff from the previous revision, with the full new revision and the author's dispositions inlined.
- From round 5 the OpenAI seat was given only a subset of its open items (see "Controller errors").

| Round, rule pin / correction pin | Seat | Rule brief | Correction brief |
| --- | --- | --- | --- |
| 1, `46857964` / `cf77ac8c` | OpenAI, uncalibrated on IronAFS | FAIL: two blockers, six majors | FAIL: two blockers, four majors, one minor |
| 1 | GLM, focused pass | FAIL: one blocker, one major, four minors | FAIL: two majors, six minors |
| 2, `03f39012` / `1dfdbe16` | OpenAI, uncalibrated on IronAFS | FAIL: three fixed, five partly; three new majors, two new minors | FAIL: three fixed, four partly; three new majors |
| 2 | GLM | PASS: six of six fixed; one new minor | PASS: seven fixed, one minor partly; four new minors |
| 3, `3cc81cd8` / `0c5c3c51` | OpenAI, uncalibrated on IronAFS | FAIL: ten fixed, three majors not fixed; two new minors | FAIL: five fixed, four partly, one major not fixed; one new major, one new minor |
| 3 | GLM | PASS: seven of seven fixed; two new minors | FAIL: twelve of twelve fixed; it confirmed the OpenAI seat's open major at source; four new minors |
| 4, `34f9e717` / `50ab2808` | OpenAI, uncalibrated on IronAFS | FAIL: eleven fixed, four partly (three majors, one minor); one new major, two new minors | FAIL: twelve fixed; one new major, one new minor |
| 4 | GLM | PASS: nine of nine fixed; one new minor | PASS: five fixed; two new minors |
| 4, judgement on the OpenAI seat's majors | `gemini:gemini-4-carbon-high`, inlined, tool-free and anonymised, uncalibrated on IronAFS | Three majors held (J1, J2, J3) | One major held (J4) |
| 5, `21f175db` / `c0d1df3a` | OpenAI, uncalibrated on IronAFS, scoped to the judgement's findings | FAIL: J2 and J3 fixed, J1 partly | PASS: J4 fixed, no new finding |
| 5 | GLM | PASS: its one open minor fixed; one new minor | PASS: one minor fixed, one not fixed (not in fix round 4's scope), no new finding |
| 6, `d8664629` / `7226d241` | OpenAI, uncalibrated on IronAFS, scoped to its round-5 items | PASS: J1 fixed, no new finding | FAIL: one new major |
| 6 | GLM | PASS: its one open minor fixed, no new finding | PASS: its open minor fixed; one new minor |

Attestation:

- The OpenAI and GLM lane logs attest the served model of every run (`gpt-6.2-sol`, `glm-5.4`), and every run echoed its terminal marker.
- The judgement runner attests its invoked pin, its input hash, its terminal marker and a completed status.
- Quota headroom was not reported.

## Findings by origin and kind

The appendix lists every finding with its origin, kind, severity and the round that closed it. Each verdict's full answer was read for this record; nothing was counted by pattern.

- **Round 1 (pre-existing):** wrong readings of the existing rules.
  - The rule brief counted a matching `draft` with a stopped review as an inconsistency.
  - Its gates and orders could not be met.
  - Its acceptance evidence could not prove the property, or prescribed mechanism.
- **Round 2 (fix round 1):** landing mechanics.
  - Squash read-back against the approved candidate.
  - Closing keywords and `Part of #N`.
  - Comparing the final change with the approved design.
- **Round 3 (fix round 2):** mostly minors, plus the correction brief's carrying of gates to later consumers. Fix round 2 left the round-2 majors unattempted (see "Controller errors").
- **Round 4 (fix round 3):** closure order and the scope of the keyword scan, put to the judgement seat; filing-record minors; and a residual in the rule brief's reproduction cases.
- **Rounds 5 and 6 (fix rounds 4 and 5):** the wording of the remaining fixes.
  - The rule brief's departure rule.
  - The correction brief's coverage re-check, whose new clause introduced the recorded blocker.

Almost all findings are contract or consistency points about acceptance evidence and order. The exceptions are two correctness findings, CB1.6 and CB3.4, each a wrong claim the correction brief made about a source, and three of kind other (RB3.1, CB3.2 and CB3.3).

## Judgement

The OpenAI seat's majors open after round 4 went to the judgement seat. These were the three majors on the rule brief that the controller named, J1 to J3, and one on the correction brief, J4.

**The seat.** `gemini:gemini-4-carbon-high`, the strongest-row Gemini pin (`docs/workflow.md` §2).

- Anthropic wrote the briefs, OpenAI raised the findings, and GLM had passed the disputed texts.
- The record of that choice does not say whether DeepSeek was considered.

**The brief it was given.** Tool-free, with everything inlined:

- both brief revisions in full;
- `docs/workflow.md` §3, steps 3 and 7, and §6, quoted verbatim;
- each finding as position A, quoted, and the brief's text as position B.

The reviewer was not named.

**The rulings.** The status was complete. It ruled A (holds) on all four, giving for each the defect and its consequence. Its result carries no fix text, although the brief asked for one. The fixes applied in fix round 4 were the findings' own, as quoted below; J2 needed none of its own beyond J3's.

The four findings it upheld:

- **J1** (rule brief, criterion 3). A material departure from the pinned design needed only to have "gone through" a review. It must instead have a conclusive passing non-author verdict, pinned to the revised design, and the final candidate must be compared with that design.
- **J2** (rule brief, criterion 6 and Order). The release sequence to the companion was unresolved, because of the closure cycle in J3.
- **J3** (rule brief, criterion 6 and Order). Manual closure was evidence for criterion 6, yet Order placed closure after it. Premature closure was not said to be reopened.
- **J4** (correction brief, criterion 6). The pre-merge closing-keyword scan was narrowed to this issue alone.

**The controller's checks of the premises, at source.**

- When the ruling arrived, the controller confirmed:
  - J3's cycle against the brief text;
  - J4's negation premise against `docs/workflow.md` lines 575–577: "GitHub ignores negation: "does not close #N" closes #N".
- J1's and J2's premises were checked for this record, afterwards:
  - **J1.** The Class S row requires "the design reviewed before code by a non-author family" (`docs/workflow.md` line 163), and step 2 requires the design "written and reviewed by another family before code" (lines 298–299). The seat had listed lines 158–164 as uncertain, because they were not inlined.
  - **J2.** A task is done when its change is on `main` and its evidence holds there; a pull request doing part of an issue says `Part of #N`; one closed early is reopened (lines 571–580).

All are CONFIRMED.

**Departure from `docs/workflow.md` §3, step 7.** The judgement seat takes "a single seat's tail, one or two majors". On the rule brief the OpenAI seat had three majors open at round 4 (J1 to J3), and four counting RA1.5's residual. The controller sent them to the judgement seat anyway.

## Convergence

Open findings after each round, per brief and seat, computed from the appendix. A finding raised in round *n* and closed in fix round *m* is open in rounds *n* to *m*. A finding counts as open until a fix closes it, whether or not its seat was asked about it again.

**Rule brief**

| Round | A: blockers and majors | A: correctness or contract, any severity | A: all findings | B: blockers and majors | B: correctness or contract, any severity | B: all findings |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 8 | 7 | 8 | 2 | 2 | 6 |
| 2 | 8 | 9 | 10 | 0 | 1 | 1 |
| 3 | 3 | 5 | 5 | 0 | 0 | 2 |
| 4 | 4 | 6 | 7 | 0 | 1 | 1 |
| 5 | 2 | 4 | 4 | 0 | 0 | 1 |
| 6 | 1 | 3 | 3 | 0 | 0 | 0 |

**Correction brief**

| Round | A: blockers and majors | A: correctness or contract, any severity | A: all findings | B: blockers and majors | B: correctness or contract, any severity | B: all findings |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 6 | 6 | 7 | 2 | 7 | 8 |
| 2 | 7 | 7 | 7 | 0 | 2 | 5 |
| 3 | 5 | 7 | 7 | 0 | 1 | 4 |
| 4 | 1 | 2 | 2 | 0 | 1 | 2 |
| 5 | 0 | 0 | 0 | 0 | 0 | 1 |
| 6 | 1 | 0 | 1 | 0 | 0 | 1 |

In the correction brief's round 3, the GLM seat also confirmed the OpenAI seat's open major, CA2.3. It is counted once, under A.

**Introduced by each fix round:**

| Fix round | Rule brief, OpenAI seat | Rule brief, GLM seat | Correction brief, OpenAI seat | Correction brief, GLM seat |
| --- | --- | --- | --- | --- |
| 1 | three majors, two minors | one minor | three majors | four minors |
| 2 | two minors | two minors | one major, one minor, and a minor reopened (CA1.7) | four minors |
| 3 | one major, two minors, and a major reopened (RA1.5) | one minor | one major, one minor | two minors |
| 4 | none new; J1's fix itself partly regressed (counted under RA2.1) | one minor | none | none |
| 5 | none | none | one major | one minor |

The kinds narrowed: from wrong readings of the rules, to landing and closure mechanics, to the wording of single clauses.

The controller's convergence notes are below, with their errors.

- **After round 3:**
  - The rule brief's three round-2 majors were open, not attempted in fix round 2.
  - The correction brief had two majors open: CA2.3, never attempted, and CA3.1, introduced by fix round 2. That count is wrong: CA1.2, CA2.1 and CA2.2 were also open, as partly fixed (five blockers and majors, in the table above).
  - Fix round 2 introduced minors on the rule brief, and one major and minors on the correction brief.
  - The kinds narrowed to landing mechanics, closing keywords and the carrying of gates to later consumers.
- **After round 4:**
  - GLM passed both briefs.
  - On the rule brief the OpenAI seat had two majors partly fixed, one new major and two minors.
  - On the correction brief it had one new major and one minor.
  - The judgement seat upheld all four majors, J1 to J4.
  - This note's rule-brief line left out RA1.5's residual and RA3.2's partly fixed minor.
- **After round 5:**
  - The correction brief passed at both seats, with GLM's minor CB4.2 open.
  - The rule brief had one major left, J1's residual, introduced by fix round 4. That count is wrong: RA1.5 was also open (two, in the table above).
- **After round 6:**
  - The note recorded the rule brief as converged, "no open finding at either seat". That is wrong: RA1.5, RA3.2 and RA4.2 are open.
  - The correction brief had one new major introduced by fix round 5, recorded as its blocker.

Fix rounds past the second, with the owner's permission (`docs/workflow.md` §3, step 7):

- **Fix round 3.**
  - Request: one fix round per brief, limited to the items open after round 3.
  - The controller's check: it wrote the convergence note after round 3, as above, before asking.
  - Recommendation: permit one fix round 3 per brief, scoped to the listed open items only.
  - Owner, 2026-11-27: "permit fix round 3 on both spec-status briefs".
  - The scope recorded with the answer: the listed open items only; if a major remained after verification, it would be recorded as a blocker, with no further request.
- **Fix round 4.**
  - Request: an optional scoped round applying the fixes of the four upheld findings, in the findings' own words, plus GLM's two minors on the filing record's permission entries, then verification.
  - The controller's check: it updated the note with round 4 and the ruling before asking. That note omitted RA1.5's residual and RA3.2.
  - Recommendation: none is recorded. After round 3 the controller had told the owner no further round would be requested, and the request called this round optional.
  - Owner, 2026-11-27: "permit fix round 4 on both spec-status briefs, judgement fixes only".
- **Fix round 5.**
  - Request: the rule brief's J1 residual and GLM's minor on the same sentence, and the correction brief's minor CB4.2.
  - The controller's check: it updated the note with round 5 before asking. The correction brief passed at both seats; the rule brief had one major left, introduced by fix round 4 (wrong: RA1.5 was also open).
  - Recommendation: none is recorded.
  - Owner, 2026-11-27: "Lets do one more round on it".
  - The answer names no brief. The controller applied it to both, as the request had listed items on both. The rule `docs/workflow.md` §3, step 3 states is a permission "for that brief alone", so this reading is the controller's.
  - The correction brief's blocker was introduced by this round.

**Departure from `docs/workflow.md` §3, step 7.** Before each request the controller checked the blocker and major counts, what the last fix introduced, and the kinds. It did not check the per-seat series of open correctness or contract findings of any severity. That series was computed afterwards from the appendix, for this record (the tables above): it is a retrospective account, not the check the step asks for.

The recommendations for fix rounds 4 and 5 were not recorded.

## Controller errors

**The fix-round-2 brief.** It told the author that the OpenAI seat had raised no new findings in round 2. The controller had counted findings by a bold-text pattern. In fact that seat had raised:

- on the rule brief: three new majors and two new minors (RA2.1 to RA2.5);
- on the correction brief: three new majors (CA2.1 to CA2.3).

The author found and reported the discrepancy, but fix round 2 was written to the controller's "minimal" instruction. Consequences:

- RA2.1 to RA2.3 and CA2.3 were not attempted in fix round 2.
- Round 3 reported them not fixed.
- The briefs reached the stop rule with those majors open.
- Fix round 3, past the second, then needed the owner's permission to address them.

**The round-5 brief and the round-4 note.** Both took the OpenAI seat's open items on the rule brief to be the judgement seat's three findings. They left out:

- RA1.5: a round-1 major, fixed in fix round 2, reopened as partly fixed at round 4 after fix round 3 rewrote the unmerged case;
- RA3.2: a minor, partly fixed;
- RA4.2: a new minor.

None of the three was put to the judgement seat, to fix rounds 4 or 5, or to rounds 5 and 6. The round-6 note's "converged" is therefore wrong. The controller found this while writing this record.

- RA1.5 and RA4.2 hold at source against the brief's text. The rule brief's property 6 still has no case that isolates "not on `main`" from stale text.
- Property 6 also has no accepted case for a file holding a frozen version 1 beside a draft version 2, which `docs/specs/README.md` lines 115–116 allow.

The rule brief stays unfiled until these are closed. Under the stop rule that needs either:

- the owner's permission for a further fix round of that brief alone; or
- the blocker recorded, with the work kept gated.

## Appendix: every finding

A is the OpenAI seat and B the GLM seat. R prefixes a finding on the rule brief and C one on the correction brief. FR*n* is fix round *n*.

Neither the author nor the controller refuted any finding. Round 6 raised none on the rule brief.

**Rule brief**

| Id | Round raised | Origin | Severity | Kind | Finding | Closed |
| --- | --- | --- | --- | --- | --- | --- |
| RA1.1 | 1 | pre-existing | blocker | contract | A matching `draft` with a record and a stopped review demanded to fail; the reproduction needs a planted false promotion and separately named checker and input revisions | FR1 |
| RA1.2 | 1 | pre-existing | major | contract | The link-only rule and existing "Review" prose against the ban on editing specifications; a README edit outside "Status" | FR1 |
| RA1.3 | 1 | pre-existing | blocker | consistency between parts | The design gate's evidence named the first edit commit it forbids; two families stated as the Class S design minimum | FR1 |
| RA1.4 | 1 | pre-existing | major | contract | A checker mechanism prescribed before design; a landed-only check blocking a valid candidate before its squash | FR2 (partly fixed in FR1) |
| RA1.5 | 1 | pre-existing | major | contract | Reproduction cases that do not prove coverage or completeness; later: the `main` negative does not isolate non-landing from stale text | **open** (fixed in FR2; partly again at round 4 after FR3's rewrite; not re-checked since) |
| RA1.6 | 1 | pre-existing | major | contract | Independent gates (ADR 0004 C6 point 3, capture-backed revisions) not re-checked or compared; the edit not limited to `draft` → `reviewed` | FR2 (partly fixed in FR1) |
| RA1.7 | 1 | pre-existing | major | contract | Pre-squash review and security evidence not separated from post-squash confirmation | FR2 (partly fixed in FR1) |
| RA1.8 | 1 | pre-existing | major | contract | The order to the companion stated in prose; no PROPOSED label, GitHub field or pickup evidence | FR2 (partly fixed in FR1) |
| RB1.1 | 1 | pre-existing | blocker | consistency between parts | The same as RA1.1: criterion 3 made a `draft` with a record and a row fail | FR1 |
| RB1.2 | 1 | pre-existing | major | consistency between parts | The "Review" section plant not scoped to `reviewed` specifications | FR1 |
| RB1.3 | 1 | pre-existing | minor | consistency between parts | The Outcome and criterion 2 allowed different README diffs | FR1 |
| RB1.4 | 1 | pre-existing | minor | contract | Two non-author families stated as the Class S design requirement | FR1 |
| RB1.5 | 1 | pre-existing | minor | consistency between parts | "Landed" in criteria 2 and 3 made a prerequisite cycle | FR1 |
| RB1.6 | 1 | pre-existing | minor | contract | The blocking relationship in prose only; the companion not in the re-check list | FR1 |
| RA2.1 | 2 | FR1 | major | contract | The final change not compared with the design approved before code (J1); later: approval before the departure's first commit, not before any implementation edit (regressed by FR4) | FR5 (not attempted in FR2; partly in FR3 and FR4) |
| RA2.2 | 2 | FR1 | major | contract | No landed-wording check: the squash not read back against the approved candidate | FR3 (not attempted in FR2) |
| RA2.3 | 2 | FR1 | major | contract | The dependency released at the squash before landed evidence; closing keywords and `Part of #N` (J2) | FR4 (not attempted in FR2; partly in FR3) |
| RA2.4 | 2 | FR1 | minor | contract | The 18-`draft` inventory demanded of the final tree too | FR2 |
| RA2.5 | 2 | FR1 | minor | consistency between parts | Property 2's consistency statement covered rows with wrong dates | FR2 |
| RB2.1 | 2 | FR1 | minor | contract | No accepted case for a new specification's `draft` with a row and no record yet | FR2 |
| RA3.1 | 3 | FR2 | minor | contract | The no-record control against a dangling reference; malformed metadata not refused | FR3 |
| RA3.2 | 3 | FR2 | minor | contract | The stale fix-round count; the convergence note required only by filing, not when round 3 concluded | **open** (partly fixed in FR3; not re-checked since) |
| RB3.1 | 3 | FR2 | minor | other | The header's fix-round count stale | FR3 |
| RB3.2 | 3 | FR2 | minor | consistency between parts | The unmerged-review case undetermined on a candidate tree | FR3 |
| RA4.1 | 4 | FR3 | major | consistency between parts | Manual closure both evidence for criterion 6 and after it in Order (J3) | FR4 |
| RA4.2 | 4 | FR3 | minor | contract | No valid mixed-status control (a frozen version beside a draft version) for the malformed-metadata refusal | **open** (not re-checked since round 4) |
| RA4.3 | 4 | FR3 | minor | contract | "Before filing" did not require the fix-round permission entries | FR4 (the OpenAI seat was not asked again; closed by the GLM seat's verification of the same text, RB4.1) |
| RB4.1 | 4 | FR3 | minor | contract | The same as RA4.3 | FR4 |
| RB5.1 | 5 | FR4 | minor | consistency between parts | Criterion 3 relied on criterion 2 to record the departure's first commit, which criterion 2 did not | FR5 |

**Correction brief**

| Id | Round raised | Origin | Severity | Kind | Finding | Closed |
| --- | --- | --- | --- | --- | --- | --- |
| CA1.1 | 1 | pre-existing | blocker | consistency between parts | The required order impossible: the design review not first; final review behind the post-merge read-back | FR1 |
| CA1.2 | 1 | pre-existing | blocker | contract | Handoffs dropping independent gates; status-independent handoffs and named suppliers (later: the test-14 results supplier) | FR3 (partly fixed in FR1 and FR2) |
| CA1.3 | 1 | pre-existing | major | contract | The class unsettled while the checker's input schema and a red predecessor were prescribed | FR1 |
| CA1.4 | 1 | pre-existing | major | contract | A ruling hash or a passing row taken as proof of review coverage | FR2 (partly fixed in FR1) |
| CA1.5 | 1 | pre-existing | major | contract | The departure census first-only; breach not separated from timing | FR2 (partly fixed in FR1) |
| CA1.6 | 1 | pre-existing | major | contract | Handoff and closure evidence without a finite boundary or write-back | FR2 (partly fixed in FR1) |
| CA1.7 | 1 | pre-existing | minor | contract | The edit allowlist broader than the metadata fields; later: all "Review" sections, not only defective ones (reopened by FR2) | FR3 |
| CB1.1 | 1 | pre-existing | major | consistency between parts | Criteria 6 and 7 made a cycle around the merge | FR1 |
| CB1.2 | 1 | pre-existing | major | contract | The departure table not inside the reviewed ruling; no cut-off | FR1 |
| CB1.3 | 1 | pre-existing | minor | contract | The supplier of criterion 1's prerequisite misattributed | FR1 |
| CB1.4 | 1 | pre-existing | minor | contract | Criterion 3 presumed what the rule issue's check reads | FR1 |
| CB1.5 | 1 | pre-existing | minor | contract | The Class S design review not the first criterion; the reproduction not named | FR1 |
| CB1.6 | 1 | pre-existing | minor | correctness | Directory Q1's gate understated as the decoder's only | FR1 |
| CB1.7 | 1 | pre-existing | minor | contract | "Each affected issue" not derived; "filed later" had no carrier | FR2 (partly fixed in FR1) |
| CB1.8 | 1 | pre-existing | minor | contract | Branch-commit evidence that does not survive the squash | FR1 |
| CA2.1 | 2 | FR1 | major | contract | The detecting run asked to prove corrections outside the checker's scope | FR3 (partly fixed in FR2) |
| CA2.2 | 2 | FR1 | major | contract | The ruling's mapping and table not required to land and be compared | FR3 (partly fixed in FR2) |
| CA2.3 | 2 | FR1 | major | contract | No `Part of #N`; the keyword scan of the PR text only; premature closure (confirmed by GLM at round 3) | FR3 (not attempted in FR2) |
| CB2.1 | 2 | FR1 | minor | consistency between parts | The re-check called listed issues "beyond" the list | FR2 |
| CB2.2 | 2 | FR1 | minor | consistency between parts | The consumer search scoped to specifications left `draft` | FR2 |
| CB2.3 | 2 | FR1 | minor | contract | Detection "on the pre-change tree", which cannot go red | FR2 |
| CB2.4 | 2 | FR1 | minor | consistency between parts | "Review pointers" against "the link and nothing else" | FR2 |
| CA3.1 | 3 | FR2 | major | contract | Later-filed consumers no longer told to re-check and carry their own gates | FR3 |
| CA3.2 | 3 | FR2 | minor | contract | The stale fix-round count | FR3 |
| CB3.1 | 3 | FR2 | minor | consistency between parts | The fallback still named a "before" run | FR3 |
| CB3.2 | 3 | FR2 | minor | other | The same as CA3.2 | FR3 |
| CB3.3 | 3 | FR2 | minor | other | Sources omitted #47, named as a supplier | FR3 |
| CB3.4 | 3 | FR2 | minor | correctness | The merge rule claimed to govern consumers of promoted specifications | FR3 |
| CA4.1 | 4 | FR3 | major | contract | The pre-merge keyword scan limited to this issue (J4) | FR4 |
| CA4.2 | 4 | FR3 | minor | contract | "Before filing" did not require the fix-round permission entries | FR4 |
| CB4.1 | 4 | FR3 | minor | contract | The same as CA4.2 | FR4 |
| CB4.2 | 4 | FR3 | minor | consistency between parts | Coverage not re-verified against the tree that merges | FR5 (not in FR4's scope) |
| CA6.1 | 6 | FR5 | major | consistency between parts | The return to criterion 1 for any changed text also catches the metadata edits the ruling authorises, and cannot meet criterion 2's order | **open: blocker recorded** |
| CB6.1 | 6 | FR5 | minor | consistency between parts | The same clause: the return not reconciled with criterion 2's order evidence, and not limited to specifications ending `reviewed` | **open** |

Open at this record's commit:

- **Rule brief:** RA1.5 (major), RA3.2 and RA4.2 (minors).
- **Correction brief:** CA6.1 (major) and CB6.1 (minor).

No judgement is pending.

## Record review

| Round, record commit | Seat | Verdict |
| --- | --- | --- |
| 1, `e5a4b27` | `glm:glm-5.4` High, focused pass, read-only repository tools with the 30 verdicts, the judgement run and both final briefs given as evidence; uncalibrated on IronAFS for this role | FAIL: one major, three minors |
| 2, `7f1b03f` (the fix commit's diff) | the same seat, its session resumed, the original terminal marker echoed | PASS: all four findings fixed and the fourteenth cell confirmed; every cell of both tables recomputed from the appendix; no new finding |

Quota headroom was not reported. The served model and the lane's terminal marker are attested by the lane log.

Round 1 found:

- **Major, consistency between parts, fixed:** the convergence tables' "correctness or contract" columns counted findings of kind "consistency between parts". Thirteen cells were wrong. The controller's recomputation found a fourteenth (the rule brief, seat A, round 1: RA1.3 is a consistency finding). All fourteen are corrected.
- **Minor, correctness, fixed:** the judgement section credited the seat with "an exact fix" for each finding. Its result carries none; the fixes were the findings' own.
- **Minor, consistency between parts, fixed:** two of the controller's convergence-note counts (after rounds 3 and 5) contradicted the tables and were not flagged as errors.
- **Minor, consistency between parts, fixed:** the kinds summary said every finding was contract or consistency; two are correctness and three are other.

The controller's brief to that seat gave the record's commit as a full hash it had extended by hand from the abbreviation, which does not exist. The seat reviewed the real commit, `e5a4b2741bd303070b92635020a5f1941cc7c9a6`.

## Measures

- **Review runs per brief.**
  - Rule brief: twelve family runs and a share of one judgement.
  - Correction brief: twelve family runs and a share of the same judgement.
  - The earlier brief they were split from had six family runs.
- **Acceptance defects found at pickup or in code review:** none yet. Neither brief is filed, and each is measured when its issue is picked up.

This record was written by `claude-code:claude-opus-5-5` for the maintainer.
