<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# GLM-5.4 High document-review calibration (#48)

## Summary

- Class: I
- Issue: [#48](https://github.com/ironafs/ironafs/issues/48)
- Scope: ordinary Class S pre-code design review of documents
- Served model: `glm-5.4`
- Effort: High
- Route: Claude Code on the Z.ai Coding Plan; inlined packet; builtin tools and MCP disabled
- Fixture author: Anthropic Opus 5.5 High via Antigravity, a separate model family from the reviewing seat
- Registered (UTC): 2026-11-27T16:31:11.174978+00:00
- Historical measured base: `b787a58dadc95f2f00f5e13195a665944dad9c69`
- Source comparison snapshot following PR #206: `c2a764d91a4a49f550cc8ee2910a362d71731691`
- Registration SHA-256: `4e5efff368ecbf801f983c0c4ed5f273eca84654b68b65c32100a74d66758c3b`
- Frozen primary scoring manifest (commitment) SHA-256: `d0e7c5fa4020a5f198b86cdde2615fe0cda402b38608525698bea1cd1b2975be`
- Source status: private measured primary pass.

## Protocol

1. Before dispatch, six primary cases were frozen: five seeded cases and one independently verified clean control. The scoring-key SHA-256 and the exact launch limits were frozen at the same time.
2. Each reviewer session saw one packet only, plus a common neutral brief and an opaque terminal code. Every session was separate. The controller checked the served model, the launcher effort, terminal completion and the exact readbacks.
3. Findings were matched to plants by document location and nature, whatever severity the reviewer stated. Each finding could credit only one plant. Unplanted findings that the evidence refuted were scored at their reported severity. Valid unplanted findings are reported separately.
4. Registered threshold: a pass needs all six runs complete, at least nine of twelve plants found, all six critical plants found, zero false blockers and no more than one false major. If the other gates are met, exactly one critical miss can trigger one frozen held-out attempt.

## Results

| Run | Clean control | Complete | Planted hits | Critical hits | False blockers | False majors | False minors | Valid unplanted | Refusals | Input | Cache read | Output | Model wall seconds |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | yes | yes | 0/0 | 0/0 | 0 | 0 | 0 | 0 | 0 | 1923 | 0 | 18340 | 205.323 |
| 2 | no | yes | 3/3 | 2/2 | 0 | 0 | 0 | 0 | 0 | 1754 | 0 | 11056 | 95.597 |
| 3 | no | yes | 3/3 | 2/2 | 0 | 0 | 0 | 0 | 0 | 1933 | 0 | 22933 | 208.670 |
| 4 | no | yes | 1/2 | 0/0 | 0 | 0 | 0 | 0 | 0 | 1224 | 0 | 15440 | 149.910 |
| 5 | no | yes | 2/2 | 1/1 | 0 | 0 | 1 | 0 | 0 | 1192 | 0 | 8539 | 82.906 |
| 6 | no | yes | 2/2 | 1/1 | 0 | 0 | 0 | 1 | 0 | 2204 | 0 | 24778 | 225.660 |
| **Total** | | **6/6** | **11/12** | **6/6** | **0** | **0** | **1** | **1** | **0** | **10230** | **0** | **101086** | **968.066** |

Aggregate score: 6/6 complete, 11/12 planted, 6/6 critical, 0 false blockers, 0 false majors, 1 false minor, 1 valid unplanted, 0 refusals. This aggregate measurement meets the registered pass threshold. That is a calibration measurement only. It does not adopt a pin and it does not complete #48.

Adjudication notes:

- One noncritical candidate appeared only as an uncertainty. It earned no hit under the frozen rubric.
- One evidence-refuted minor finding and one valid unplanted major finding were recorded.
- The clean control produced no findings and no uncertainties.

## Resources

- Input tokens: 10230
- Output tokens: 101086
- Cache read tokens: 0
- Cache creation tokens: 0
- Model wall seconds (sum): 968.066
- Quota headroom: not reported
- Billing: the Z.ai Coding Plan billed amount was not reported

## Effort attestation

The exact launcher arguments and the lane receipts attest High effort. The served model appears in the provider transcript. The provider does not separately echo the effort level, so the effort attestation stops at the launcher and lane receipts.

## Historical result

An earlier six-packet document screen found 12/12 planted and 6/6 critical. Its intended clean control turned out to contain a genuine defect, which made that run inconclusive. That inconclusive result retains its original registration date of 2026-11-26. This measurement does not replace or reinterpret it.

## Sealed hold-out

There were no critical misses, so the held-out trigger did not fire. No held-out payload was opened, and the hold-out remains sealed.

## Existing scopes

The earlier GLM calibrations keep their own scopes: code and source-gathering at High, and code evidence at Max. This result does not cover any other model or effort, and it does not cover the open remainder of #48.

## Source carry-forward

After #206, every source blob named by the five seeded primary fixtures and the clean wait-evaluation control still matches the frozen base. This recheck only compared sources against the historical measured base. It is a separate check from running a benchmark on current main, and no calibration was rerun on current main.

## References

- [Issue #48](https://github.com/ironafs/ironafs/issues/48)
- [Issue #48 comment](https://github.com/ironafs/ironafs/issues/48#issuecomment-7202775241)
- [Workflow: model routing](../workflow.md#model-routing)
- [GLM-5.4 pin record](2026-11-24-glm54-pin.md)
- [GLM-5.4 Max pin record](2026-11-24-glm54-max-pin.md)

## Record integration review

Class I. The reviewed change adds this record and one index row. All three seats reviewed `3ccda71e0f0499c9d483826e629a58697beb2128..9950122c213797c83bdb3142a433880860503dd3` at tree `6bf97aba29aa14599ed38a24e721df8a96bb810e`.

| Seat | Served model | Effort | Route | Result |
|---|---|---|---|---|
| First independent source | `gpt-6.2-sol` | High | OpenAI Responses API | PASS, no findings |
| Second independent source | `deepseek-flash` | Default | Headless Claude Code on DeepSeek Anthropic-compatible API, inline and tool-free | PASS, two editorial minors |
| Supplemental source | `gemini-4-carbon-medium` | Medium | Google Antigravity | PASS, one editorial minor |

The three seats represent separate families from the Anthropic author. They checked the recorded measurement, its threshold, the source comparison, its scope and the index addition against redacted evidence. The controller verified the exact identities, terminal receipts, numeric totals and relative file links. The protected fixture and held-out contents remain outside this review. Quota headroom was unavailable.


DeepSeek found two inherited labeling omissions. The results table and resources list now identify the measured time as model wall time. The historical section now gives the original registration date of November 26, 2026. Both changes clarify the existing measurement and source identities. Gemini flagged the undefined phrase “wait control”. The source comparison now names the clean wait-evaluation control, which the frozen registration used as its single primary clean case.

The original record was drafted by `agy:claude-opus-5-5-high` via Antigravity. Opus 5.5 High supplied the final status wording through the Anthropic Messages API.

## Disclosure

This record was written by `agy:claude-opus-5-5-high` for the maintainer.
