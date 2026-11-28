<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Issue brief review: a phased `ironafs-disk` row in ADR 0004 C4

The proposed Class S design issue asks for a Working-decision amendment of ADR 0004 C4, and the matching `check-deps` change, so that #156's S4 fault feature can be defined in an `ironafs-disk` crate before #99 adds the `Disk` driver. The brief settles the authority only; S4's gate mechanism stays with #156. Both independent families checked the brief at base `21f46b1fade4aee4db506bd5b491d9c8fdd39b67` (round 1) and `d1ea48b9798df666d786de22bd1dbd6f1509e8f7` (rounds 2 to 5), whose only change between them is testing.md 10.6 naming #156 and one review record. The reviewed revision 5 SHA-256 is `ae5deba6c24d59a8aa2647a31dd9618b1313458de5de078948a92d238f6e5db4`. The filing text differs from it only by naming this record's path in its "Before filing" paragraph, SHA-256 `da215469d124e8dcbce6429b82c4d0cbac8495b957f3561ed1d9e0f8aff25b23`.

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
| 5, `ae5deba6`, v4 to v5 | `codex:gpt-6.2-sol` xhigh, uncalibrated on IronAFS | PASS: the upheld major and all four of its minors fixed, no new finding |
| 5, `ae5deba6`, v4 to v5 | `glm:glm-5.4` High | PASS: both minors fixed, no new finding |

The OpenAI and GLM lane logs attest the served model, and every run echoed its terminal marker. The judgement seat's runner attests its invoked pin, input hash and terminal marker. Quota headroom was not reported.

## Findings by origin and kind

- Round 1 (origin: pre-existing). Contract: a ban on the precursor's development dependencies stated as if the existing rule forbade all such edges, when it allows them for shipped crates; a driver-phase edge to the conformance body left out; seams Q17 and 5.2 point 4 left blocking the crate. Missing mechanism: which check owns each property. Other: review, sweep and closure criteria, and citations of the ADR 0004 overview, ADR 0005 and the workflow. All fixed in fix round 1.
- Round 2 (origin: fix round 1). Correctness: a fallback that contradicted the precursor's zero-dependency rule, and a wrong owner (`check-deps`) for "no `unsafe`". Contract: the S4 wait recorded as unchanged while the brief added one; the driver allowlist and consumer allowlist proven only by green runs. Fixed in fix round 2.
- Round 3 (origin: fix round 2). Contract: the checker's test path must exercise production's policy selection; a dependency allowlist does not prove that no shipped build enables the feature; "no `unsafe`" must cover source no build compiles. Minors: wording, an evidence item that cannot come from the named command, and contract lists. Fixed in fix round 3.
- Round 4 (origin: fix round 3). Contract: the dependency-authorization rule stated in terms of the feature being enabled, which the all-features lint or Cargo's feature unification does without any consumer's request. Minors: six reports, four distinct (both GLM minors matched OpenAI minors): the target inventory proves only the build-script claim; the testing §9 row must state the rule accurately, not only re-cite it; Q17's bullet under the wrong checkbox; a prerequisite missing from criterion 5. Fixed in fix round 4.
- Round 5: none.

## Judgement

The disputed text was property 5's rule "only the named crates depend on `ironafs-disk` with the feature enabled". The objection: the workspace lint enables every feature itself, so under the rule as written any dependent crate could be in breach without requesting the feature; the rule should restrict which crates may request it. The seat was chosen by `docs/workflow.md` §2: the author's family and the family that raised the finding were excluded, GLM was excluded because it had passed the disputed text, and DeepSeek was outside its off-peak window. Its brief gave the two positions with neutral labels and the primary sources inlined. It ruled that the objection holds and gave wording that restricts requests through a dependency and does not count activation by the lint or by feature unification as a consumer's request. The controller confirmed the premise at source: the workspace lint runs `--workspace --all-features` (`crates/xtask/src/bans.rs`, `workspace_clippy`). Fix round 4 applied the wording verbatim.

EXECUTED by the controller: rustc 1.97.1 refuses both `#[expect(unsafe_code)]` and `#[allow(unsafe_code)]` under a crate-level `forbid(unsafe_code)` (error E0453), which the brief's property 2 relies on.

## Convergence

| Round | OpenAI seat: open blockers and majors | OpenAI seat: open confirmed findings, any severity | GLM seat: open blockers and majors | GLM seat: open confirmed findings, any severity |
| --- | --- | --- | --- | --- |
| 1 | 6 | 9 | 2 | 10 |
| 2 | 7 | 10 | 0 | 0 |
| 3 | 3 | 7 | 0 | 3 |
| 4 | 1 | 5 | 0 | 2 |
| 5 | 0 | 0 | 0 | 0 |

Introduced by each fix round, OpenAI seat: five majors and two minors; two majors and three minors; one major and four minors; none. GLM seat: none; two minors; two minors; none. The kinds narrowed from wrong rules to the wording of acceptance evidence.

Fix rounds past the second, with the owner's permission:

- Fix round 3. Request: one scoped round limited to the items open after round 3, verified by both seats on the v3 to v4 diff. Controller's check of the note before asking: open majors at the OpenAI seat 6, 7, 3 and none at GLM; the last fix introduced fewer defects than the one before; the kinds narrowing. Recommendation: permit. Owner, 2026-11-27: "permit fix round 3 on the C4 brief".
- Fix round 4. Request: apply the judgement seat's wording verbatim and fix the four distinct open minors, nothing else, with the OpenAI seat verifying the major and the GLM seat the minors on the v4 to v5 diff. Controller's check: open majors 6, 7, 3, 1; the wording came from the judgement seat. Recommendation: permit. Owner, 2026-11-27: "permit fix round 4 on the C4 brief".

No blocker, major or confirmed correctness or contract finding is open, and no judgement is pending.

## Measures

- Review runs for this brief: ten family runs and one judgement.
- Acceptance defects found at pickup or in code review: pending, measured when the issue is picked up.

This record was written by `claude-code:claude-opus-5-5` for the maintainer.
