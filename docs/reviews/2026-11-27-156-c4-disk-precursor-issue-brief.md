<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Issue brief review: a phased `ironafs-disk` row in ADR 0004 C4

The proposed Class S design issue asks for a Working-decision amendment of ADR 0004 C4, and the matching `check-deps` change, so that #156's S4 fault feature can be defined in an `ironafs-disk` crate before #99 adds the `Disk` driver. The brief settles the authority only; S4's gate mechanism stays with #156. Both independent families checked the brief at base `21f46b1fade4aee4db506bd5b491d9c8fdd39b67` (round 1) and `d1ea48b9798df666d786de22bd1dbd6f1509e8f7` (rounds 2 to 5). The reviewed revision 5 SHA-256 is `ae5deba6c24d59a8aa2647a31dd9618b1313458de5de078948a92d238f6e5db4`. The filing text differs from it only by naming this record's path in its "Before filing" paragraph, SHA-256 `da215469d124e8dcbce6429b82c4d0cbac8495b957f3561ed1d9e0f8aff25b23`.

The brief was written by `claude-code:claude-opus-5-5`.

## Rounds

Every OpenAI row ran `codex:gpt-6.2-sol` at xhigh through the OpenAI Responses API (`openai_lane.py`, research mode, read-only repository tools); it is uncalibrated on IronAFS for document review (its calibration, record 2026-11-27-48-sol-xhigh-code-calibration, covers tool-free code review on inlined packets only). Every GLM row ran `glm:glm-5.4` at High on the Z.ai Coding Plan through headless Claude Code with read-only repository tools; its IronAFS calibration covers ordinary Class S pre-code document review. Each round after the first verified its seat's own open findings on the diff from the previous revision, given with the full new revision.

| Round, brief pin, scope | Seat | Result |
| --- | --- | --- |
| 1, `5c112885`, whole brief | `codex:gpt-6.2-sol` xhigh, uncalibrated on IronAFS | FAIL: one blocker, five majors, three minors |
| 1, `5c112885`, whole brief | `glm:glm-5.4` High, focused pass | FAIL: two majors, eight minors |
| 2, `e3cec447`, v1 to v2 | `codex:gpt-6.2-sol` xhigh, uncalibrated on IronAFS | FAIL: six fixed, three partly (two majors, one minor); five new majors, two new minors |
| 2, `e3cec447`, v1 to v2 | `glm:glm-5.4` High | PASS: ten of ten fixed, no new finding |
| 3, `61d63ce8`, v2 to v3 | `codex:gpt-6.2-sol` xhigh, uncalibrated on IronAFS | FAIL: eight fixed, two partly (one major, one minor); two new majors, three new minors |
| 3, `61d63ce8`, v2 to v3 | `glm:glm-5.4` High | PASS: no blocker or major; two new minors, one carried |
| 4, `746efbaf`, v3 to v4 | `codex:gpt-6.2-sol` xhigh, uncalibrated on IronAFS | FAIL: every open item fixed; one new major, four new minors |
| 4, `746efbaf`, v3 to v4 | `glm:glm-5.4` High | PASS: no blocker or major; two new minors, both the same as two of the OpenAI seat's |
| 4, judgement on the single major | `gemini:gemini-4-carbon-high`, inlined and tool-free, uncalibrated on IronAFS | The major held; the seat gave the replacement wording |
| 5, `ae5deba6`, v4 to v5 | `codex:gpt-6.2-sol` xhigh, uncalibrated on IronAFS | PASS: verified its five open items (A4.1 to A4.5) fixed, no new finding |
| 5, `ae5deba6`, v4 to v5 | `glm:glm-5.4` High | PASS: verified its two open items (B4.1, B4.2) fixed, no new finding |

The OpenAI and GLM lane logs attest the served model, and every run echoed its terminal marker. The judgement seat's runner attests its invoked pin, input hash and terminal marker. Quota headroom was not reported.

## Findings by origin and kind

The appendix lists every finding with its origin, kind and the round that closed it. In short: round 1's findings (pre-existing) were wrong or missing rules and missing review, sweep and citation criteria; three of the OpenAI seat's items were only partly fixed and closed in fix rounds 2 and 3. Round 2's (introduced by fix round 1) were contradictions and wrong owners for checks. Round 3's (introduced by fix round 2, one GLM minor first raised then on fix round 1's text) were the precision of acceptance evidence. Round 4's (introduced by fix round 3) were one wording point in the dependency-authorization rule, upheld by the judgement seat, and four distinct minors in six reports. Each fix was limited to the findings it answered.

## Judgement

The disputed text was property 5's rule "only the named crates depend on `ironafs-disk` with the feature enabled". The objection: the workspace lint enables every feature itself, so under the rule as written any dependent crate could be in breach without requesting the feature; the rule should restrict which crates may request it. The seat was chosen by `docs/workflow.md` §2: the author's family and the family that raised the finding were excluded, GLM was excluded because it had passed the disputed text, and DeepSeek was outside its off-peak window. Its brief gave the two positions with neutral labels and the primary sources inlined. It ruled that the objection holds and gave wording that restricts requests through a dependency and does not count activation by the lint or by feature unification as a consumer's request. The controller confirmed the premise at source: the workspace lint runs `--workspace --all-features` (`crates/xtask/src/bans.rs`, `workspace_clippy`). Fix round 4 applied the wording verbatim.

EXECUTED by the controller: rustc 1.97.1 refuses both `#[expect(unsafe_code)]` and `#[allow(unsafe_code)]` under a crate-level `forbid(unsafe_code)` (error E0453), which the brief's property 2 relies on.

## Convergence

Open findings after each round, per seat, computed from the appendix (a finding raised in round *n* and closed in fix round *m* is open in rounds *n* to *m*).

| Round | A: blockers and majors | A: correctness or contract, any severity | A: all findings | B: blockers and majors | B: correctness or contract, any severity | B: all findings |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 6 | 7 | 9 | 2 | 7 | 10 |
| 2 | 7 | 7 | 10 | 0 | 0 | 0 |
| 3 | 3 | 5 | 7 | 0 | 1 | 3 |
| 4 | 1 | 4 | 5 | 0 | 1 | 2 |
| 5 | 0 | 0 | 0 | 0 | 0 | 0 |

Introduced by each fix round, OpenAI seat: five majors and two minors; two majors and three minors; one major and four minors; none. GLM seat: one minor (first raised in round 3); two minors; two minors; none. The kinds narrowed from wrong rules to the wording of acceptance evidence.

Fix rounds past the second, with the owner's permission:

- Fix round 3. Request: one scoped round limited to the items open after round 3, verified by both seats on the v3 to v4 diff. The controller wrote the convergence note from the round-3 results before asking, and its check is the note's content: open blockers and majors at the OpenAI seat 6, 7, 3 and none at GLM; the last fix introduced fewer defects than the one before; the kinds narrowing. Recommendation: permit. Owner, 2026-11-27: "permit fix round 3 on the C4 brief".
- Fix round 4. Request: apply the judgement seat's wording verbatim and fix the four distinct open minors, nothing else, with the OpenAI seat verifying the major and the GLM seat the minors on the v4 to v5 diff. The controller updated the note with round 4 and the judgement before asking: open blockers and majors 6, 7, 3, 1; the wording came from the judgement seat. Recommendation: permit. Owner, 2026-11-27: "permit fix round 4 on the C4 brief".

No blocker, major or confirmed correctness or contract finding is open, and no judgement is pending.

## Appendix: every finding

A is the OpenAI seat, B the GLM seat; FR*n* is fix round *n*. Every finding was checked at source by the controller before it went to the author; none was refuted. Round 5 raised none.

| Id | Round raised | Origin | Severity | Kind | Finding | Closed |
| --- | --- | --- | --- | --- | --- | --- |
| A1.1 | 1 | pre-existing | blocker | contract | The precursor's ban on development dependencies stated as if the existing rule forbade them for every shipped crate | FR1 |
| A1.2 | 1 | pre-existing | major | contract | Evidence for the three-crate exclusion and both phases; later: production's policy selection and isolated plants in the checker's test path | FR3 (partly fixed in FR1 and FR2) |
| A1.3 | 1 | pre-existing | major | contract | Owner and proof of the declaration, default-off and activation rules on the real package | FR2 (partly fixed in FR1) |
| A1.4 | 1 | pre-existing | major | contract | Seams Q2, Q17 and 5.2 point 4 left blocking the gate-only crate | FR1 |
| A1.5 | 1 | pre-existing | major | contract | Class S review, security review and closure evidence missing from the acceptance criteria | FR1 |
| A1.6 | 1 | pre-existing | major | other | Amendment history and the full supersession sweep not required | FR1 |
| A1.7 | 1 | pre-existing | minor | correctness | The existing pure-crate feature plant misdescribed | FR1 |
| A1.8 | 1 | pre-existing | minor | correctness | The provider and deferral alternatives misdescribed | FR1 |
| A1.9 | 1 | pre-existing | minor | other | Opening lines, evidence labels and the pre-filing passes | FR3 (partly fixed in FR1 and FR2) |
| B1.1 | 1 | pre-existing | major | missing mechanism | `check-deps` cannot see a driver's development dependencies; no rule named | FR1 |
| B1.2 | 1 | pre-existing | major | contract | The driver-phase edge the conformance body needs left out | FR1 |
| B1.3 | 1 | pre-existing | minor | correctness | A rule attributed to C4 point 7 that it does not state | FR1 |
| B1.4 | 1 | pre-existing | minor | contract | The simulator's legs of the three-crate exclusion not planted | FR1 |
| B1.5 | 1 | pre-existing | minor | correctness | The pure-crate feature cases claimed to exist already | FR1 |
| B1.6 | 1 | pre-existing | minor | correctness | The amended status line placed in the wrong ADR file | FR1 |
| B1.7 | 1 | pre-existing | minor | contract | The sweep's per-actor check omitted | FR1 |
| B1.8 | 1 | pre-existing | minor | other | ADR 0005 decisions cited by the wrong points | FR1 |
| B1.9 | 1 | pre-existing | minor | other | A blocking relationship stated in prose without the source of the no-edge rule | FR1 |
| B1.10 | 1 | pre-existing | minor | contract | The feature's name fixed without saying why that is authority, not S4 mechanism | FR1 |
| A2.1 | 2 | FR1 | major | correctness | A fallback that contradicted the precursor's zero-dependency rule | FR2 |
| A2.2 | 2 | FR1 | major | correctness | `check-deps` named as owner of "no `unsafe`" | FR2 |
| A2.3 | 2 | FR1 | major | contract | The driver-phase allowlist proven only by a green fixture | FR2 |
| A2.4 | 2 | FR1 | major | contract | No negative for an unlisted but graph-legal consumer | FR2 |
| A2.5 | 2 | FR1 | major | consistency between parts | The S4 wait recorded as unchanged while the brief added one | FR2 |
| A2.6 | 2 | FR1 | minor | consistency between parts | Seams 5.2 point 5, the testing §9 row and the diagnostic not aligned; reachability wording | FR2 |
| A2.7 | 2 | FR1 | minor | correctness | The metadata reader's development-dependency list misdescribed | FR2 |
| A3.1 | 3 | FR2 | major | contract | A dependency allowlist used as proof that no shipped build enables the feature | FR3 |
| A3.2 | 3 | FR2 | major | contract | "No `unsafe`" proven by builds that do not compile every branch | FR3 |
| A3.3 | 3 | FR2 | minor | correctness | "It reads no source" wrong for the composition crate | FR3 |
| A3.4 | 3 | FR2 | minor | contract | The base checker's red run asked of a self-test that cannot give it | FR3 |
| A3.5 | 3 | FR2 | minor | consistency between parts | The contracts list missing seams 5.2 point 5 and the testing §9 row | FR3 |
| B3.1 | 3 | FR2 | minor | other | "Target inventory" undefined | FR3 |
| B3.2 | 3 | FR2 | minor | consistency between parts | Testing §9 missing from the re-check, sources and outcome | FR3 |
| B3.3 | 3 | FR1 (first raised in round 3) | minor | contract | Criterion 5 asked a repository diff to show an order stated only on GitHub | FR3 |
| A4.1 | 4 | FR3 | major | contract | The dependency-authorization rule stated in terms of the feature being enabled, not requested (upheld by the judgement seat) | FR4 |
| A4.2 | 4 | FR3 | minor | contract | The target inventory asked to prove more than the build-script claim | FR4 |
| A4.3 | 4 | FR3 | minor | contract | The testing §9 row only re-cited, not restated with the development exception | FR4 |
| A4.4 | 4 | FR3 | minor | consistency between parts | Q17's bullet under the wrong checkbox | FR4 |
| A4.5 | 4 | FR3 | minor | contract | Criterion 5's prerequisite list missing criterion 6's comments | FR4 |
| B4.1 | 4 | FR3 | minor | consistency between parts | The same as A4.4 | FR4 |
| B4.2 | 4 | FR3 | minor | contract | The same as A4.5, and INSPECTED cannot show a future merge | FR4 |

## Measures

- Review runs for this brief: ten family runs and one judgement.
- Acceptance defects found at pickup or in code review: pending, measured when the issue is picked up.

This record was written by `claude-code:claude-opus-5-5` for the maintainer.
