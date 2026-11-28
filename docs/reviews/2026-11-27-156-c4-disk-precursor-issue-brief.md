<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Issue brief review: a phased `ironafs-disk` row in ADR 0004 C4

The proposed Class S design issue asks for a Working-decision amendment of ADR 0004 C4, and the matching `check-deps` change, so that #156's S4 fault feature can be defined in an `ironafs-disk` crate before #99 adds the `Disk` driver. The brief settles the authority only; S4's gate mechanism stays with #156. Both independent families checked the brief at base `21f46b1fade4aee4db506bd5b491d9c8fdd39b67` (round 1) and `d1ea48b9798df666d786de22bd1dbd6f1509e8f7` (rounds 2 to 5). The reviewed revision 5 SHA-256 is `ae5deba6c24d59a8aa2647a31dd9618b1313458de5de078948a92d238f6e5db4`. The filing text differs from it only by naming this record's path in its "Before filing" paragraph, SHA-256 `da215469d124e8dcbce6429b82c4d0cbac8495b957f3561ed1d9e0f8aff25b23`.

The brief was written by `claude-code:claude-opus-5-5`.

| Round and brief pin | Family and effort | Result |
| --- | --- | --- |
| 1, `5c112885` | OpenAI `gpt-6.2-sol` xhigh, Responses API, read-only repository tools | FAIL: one blocker, five majors, three minors |
| 1, `5c112885` | GLM `glm-5.4` High, focused pass | FAIL: two majors, eight minors |
| 2 (fix round 1), `e3cec447` | OpenAI `gpt-6.2-sol` xhigh | FAIL: six fixed, three partly, five new majors, two new minors |
| 2, `e3cec447` | GLM `glm-5.4` High | PASS: all ten fixed, no new finding |
| 3 (fix round 2), `61d63ce8` | OpenAI `gpt-6.2-sol` xhigh | FAIL: eight fixed, two partly, two new majors, three new minors |
| 3, `61d63ce8` | GLM `glm-5.4` High | PASS, three minors |
| 4 (fix round 3), `746efbaf` | OpenAI `gpt-6.2-sol` xhigh | FAIL: all earlier items fixed, one new major, four new minors |
| 4, `746efbaf` | GLM `glm-5.4` High | PASS, two minors |
| 4, judgement on the single major | Google `gemini-4-carbon-high`, inlined and tool-free, uncalibrated on IronAFS | The major held; the seat gave the replacement wording |
| 5 (fix round 4), `ae5deba6` | OpenAI `gpt-6.2-sol` xhigh | PASS, no new finding |
| 5, `ae5deba6` | GLM `glm-5.4` High | PASS, no new finding |

The OpenAI and GLM results attest the served model in their lane logs, and every run echoed its terminal marker. The judgement seat's runner attests its invoked pin, input hash and terminal marker. Quota headroom was not reported. `gpt-6.2-sol` xhigh is calibrated on IronAFS only for tool-free code review on inlined packets (record 2026-11-27-48-sol-xhigh-code-calibration); for this document review it is uncalibrated on IronAFS. GLM 5.4 High's document calibration covers ordinary Class S pre-code document review.

The judgement seat was chosen by `docs/workflow.md` §2: the author's family and the family that raised the finding were excluded, GLM was excluded because it had passed the disputed text, and DeepSeek was outside its off-peak window. Its brief gave the finding and the brief's text as neutral positions with the primary sources inlined. The controller confirmed its premise at source: the workspace lint runs `--workspace --all-features` (`crates/xtask/src/bans.rs`, `workspace_clippy`).

Findings by kind. Round 1: wrong or missing rules (a ban on development dependencies that the existing rule allows, a driver-phase edge the conformance body needs, the Q17 and §5.2 wording) and missing review, sweep and citation criteria. Round 2: contradictions introduced by the first fix and wrong owners for checks. Round 3: the precision of acceptance evidence (the production policy selection in the checker's test path, a dependency allowlist used as proof that no shipped build enables the feature, `unsafe` in source no build compiles). Round 4: one wording point in the dependency-authorization rule (a feature enabled by the all-features lint is not a consumer's request). Each fix was limited to the findings it answered. The controller compiled one claim the brief rests on: rustc 1.97.1 refuses both `#[expect(unsafe_code)]` and `#[allow(unsafe_code)]` under a crate-level `forbid(unsafe_code)` (error E0453).

## Convergence

Open blocker and major counts at the OpenAI seat: 6, 7, 3, 1, 0. At the GLM seat: 2, 0, 0, 0, 0. Findings introduced by each fix round at the OpenAI seat: five majors, two majors, one major, none. The kinds narrowed from wrong rules to evidence wording.

Fix rounds past the second, with the owner's permission:

- Fix round 3. Request: one scoped round limited to the open items after round 3; recommendation: permit, the counts falling at both seats. Owner, 2026-11-27: "permit fix round 3 on the C4 brief".
- Fix round 4. Request: apply the judgement seat's wording verbatim and fix the six open minors, nothing else; recommendation: permit. Owner, 2026-11-27: "permit fix round 4 on the C4 brief".

No blocker, major or confirmed correctness or contract finding is open, and no judgement is pending.

This record was written by `claude-code:claude-opus-5-5` for the maintainer.
