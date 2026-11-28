<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# GPT-6.2 Sol xhigh code-review calibration (#48)

## Summary

- Class: I
- Issue: [#48](https://github.com/ironafs/ironafs/issues/48)
- Scope: ordinary Class S code review
- Served model: `gpt-6.2-sol`
- Effort: xhigh
- Route: OpenAI Responses API through the maintainer's read-only lane, one request per case, no repository access, nothing stored by the provider, standard service tier
- Case author and custodian: Anthropic Opus 5.5 High, a separate model family from the reviewing seat. No OpenAI model wrote, chose, attested or scored a case.
- Measured base: `21f46b1fade4aee4db506bd5b491d9c8fdd39b67`
- Registration SHA-256: `cfd3e23270e55beee3c452abba4c831ce89e43effabef20287065f8d4fae5585`
- Frozen run manifest SHA-256: `3b338ab6f4db5ffee690ec4fdfbbdee4f5cb2591f9cdae8461d96bfc6919b9f5`
- Scoring SHA-256: `461ea2eac57a657d70c015a6dae0337e174d000a556f00b72933f94713520927`
- Source status: private measured primary pass. The cases, key and reviews stay private.

## Protocol

1. Six fresh code changes on the measured base were written for this measurement: five seeded with 12 planted defects (6 critical) and one clean. Each is a production-like change of 80 to 400 lines to `ironafs-vfs`, `ironafs-time` or `ironafs-config`, with tests. At every head, format, Nextest, doctests and Clippy with warnings denied passed.
2. The clean case was reviewed by GLM 5.4 High as an ordinary review, in a copy holding only that change. It found no defect, and the custodian confirmed it clean before registration.
3. Each case was sent once as one self-contained packet: a neutral brief, the diff, the full head text of every changed source file, the touched package's manifest and crate root, and fixed specification sections chosen per package before any case was read. The custodian attested that every planted defect and the clean case can be judged from its packet alone. The reviewer had no repository, no memory and no tool but its verdict.
4. A review counted only if finished: served model, effort, tier and limits as registered, one request, a complete structured verdict, and the per-run and per-packet terminal codes. All responses were locked by hash before scoring.
5. Findings were matched to plants by file and line or containing item, and by the defect's nature. A critical plant counted only at blocker or major severity. Each finding could credit one plant. Unassigned findings were judged as valid unplanted defects or false findings at their stated severity.
6. Registered threshold: six finished reviews, at least nine of twelve plants, all six critical plants, no false blocker and at most one false major. Exactly one critical miss, with every other gate met, could trigger one held-out attempt after a separate owner decision.
7. Before registration, Gemini Carbon High reviewed the registration, packet builder, finished-review check and launcher. The first revision failed with three blockers, four majors and one minor. All were fixed, and the same seat passed the second revision with no findings. The owner approved the dispatch, its six calls and its spend bound before any request.

## Results

| Run | Clean control | Finished | Findings (blocker/major/minor) | Planted hits | Critical hits | False blockers | False majors | False minors | Valid unplanted | Input tokens | Output tokens | Wall seconds |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | no | yes | 0/2/1 | 2/2 | 1/1 | 0 | 0 | 0 | 1 | 15964 | 3203 | 41.0 |
| 2 | no | yes | 0/2/1 | 3/3 | 1/1 | 0 | 0 | 0 | 0 | 21496 | 5310 | 67.6 |
| 3 | no | yes | 0/1/2 | 2/2 | 1/1 | 0 | 0 | 0 | 1 | 13020 | 2569 | 32.2 |
| 4 | yes | yes | 0/0/0 | 0/0 | 0/0 | 0 | 0 | 0 | 0 | 13863 | 3059 | 37.4 |
| 5 | no | yes | 0/2/1 | 3/3 | 2/2 | 0 | 0 | 0 | 0 | 17283 | 3574 | 41.7 |
| 6 | no | yes | 0/2/1 | 2/2 | 1/1 | 0 | 0 | 0 | 1 | 17883 | 3899 | 46.7 |
| **Total** | | **6/6** | **0/9/6** | **12/12** | **6/6** | **0** | **0** | **0** | **3** | **99509** | **21614** | **266.6** |

Aggregate score: 6/6 finished, 12/12 planted, 6/6 critical (each at major severity), 0 false blockers, 0 false majors, 0 false minors, 3 valid unplanted, 0 refusals, 0 retries. This meets the registered pass threshold. It is a calibration measurement only: it does not change a pin and it does not complete #48.

Adjudication notes:

- One valid unplanted major is a second consequence of a planted critical defect, which another finding was credited with.
- One valid unplanted minor is a missing negative test for a planted defect.
- One valid unplanted minor is a wrong comment the case author wrote by mistake in a test.
- The clean control produced no findings.
- No match or judgement was disputed, so no refutation seat sat.

## Scoring audit

Gemini Carbon High audited the scoring against the registered rules, the primary key and the six reviews, inlined and without tools: PASS, no findings. It could not check the cited test lines of two unplanted findings without the code, and recorded that as uncertain.

## Resources

- Input tokens: 99509
- Output tokens: 21614 (reasoning included)
- Wall seconds (sum, lane launcher): 266.6
- Quota headroom: not reported

## Effort attestation

Each request in the frozen transcripts carries effort `xhigh`, the 64,000-token output limit and only the verdict tool. Each provider response names `gpt-6.2-sol` and the standard tier.

## Custody

The controller learned which primary case was clean after the packets were built and hashed. It knew nothing of any plant, did not score, and fixed the dispatch order and retry rule in the registration before dispatch. The registration review saw this and passed it.

## Sealed hold-out

There were no critical misses, so the held-out trigger did not fire. The held-out case was never sent and remains sealed.

## Existing scopes

This result covers `gpt-6.2-sol` at xhigh in ordinary Class S code review only. It does not cover another effort, Astra, design or security review, or the open remainder of #48. The earlier GLM 5.4 High document calibration keeps its own scope.

## References

- [Issue #48](https://github.com/ironafs/ironafs/issues/48)
- [Workflow: model routing](../workflow.md#model-routing)
- [GLM-5.4 High document calibration](2026-11-27-48-glm54-document-calibration.md)

## Disclosure

This record was written by `claude-code:claude-opus-5-5` for the maintainer.
