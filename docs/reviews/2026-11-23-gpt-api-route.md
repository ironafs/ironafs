<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: GPT read-only seats may also run on the OpenAI API

## The change

Class P. The owner's decision and ruling of 2026-11-23.

- Until now, `docs/workflow.md` §2 said that only CLIs are used and that
  API-platform lanes are not.
- **The decision:** GPT read-only seats may also run on the owner's OpenAI
  API credit, through the maintainer's `openai_lane.py` (the Responses API,
  with read-only tools confined to the repository, a cost cap per run and a
  did-it-run gate). The owner's ruling, after the first review: it is the
  same models by another route, not a new lane. So a read-only seat takes
  whichever route has quota, at the same pins (`gpt-6.2-sol`,
  `gpt-6.2-astra`) and effort; trailers credit both routes as `codex`; the
  review record names the route; either route counts as the OpenAI lane
  wherever a rule names it, embargoed security work included; code is
  written only through the codex CLI.
- **Where it is written:** `docs/workflow.md` §2 (the lanes paragraph, the
  design row, the judgement seat's record bullet); `AI.md` (the OpenAI row
  and the two GPT rows of "What each model can do here");
  `docs/reviews/README.md` (a GPT seat's record names its route); the
  `review-lanes` skill; ADR 0002 (a correction changing no decision: the
  `codex` row covers both routes, and some lanes, not one, are
  usage-billed).
- **Not changed:** the list of agent ids; every family and seat rule;
  implementation lanes.
- **Written by** `claude-code:claude-opus-5-5`.

## The evidence

Executed by the controller on 2026-11-23: one run of the lane on
`gpt-6.2-astra` (after adding that model to the lane's tables): the lane's
gate passed and its run file recorded `model_requested=gpt-6.2-astra`,
`model_reported=gpt-6.2-astra` and the effort. The lane's offline
self-test passed, 126 of 126. The OpenAI API's model and pricing pages list
both `gpt-6.2-sol` and `gpt-6.2-astra`.

## Rounds

| Round | Range | Seat (served model, effort) | Verdict |
|---|---|---|---|
| 1 | `d840644..f75f5e1` | GLM (`glm-5.3`, high), reading the repository | FAIL: 1 major, 2 minors |
| 1 | same | Gemini (`gemini-4-carbon-medium`), inlined | FAIL: 1 major, 2 minors |
| Fix 1 | `f75f5e1..ca13d46` | the owner's ruling applied | |
| 2 (verification) | `d840644..ca13d46` | GLM (`glm-5.3`, high) | PASS: 3 minors |
| 2 (verification) | same | Gemini (`gemini-4-carbon-medium`) | PASS: 1 minor (the same as GLM's third) |
| Fix 2 | the minors, below | | stop rule: only minors, fixed and recorded |

Quota headroom: not reported by either lane.

## Findings

Round 1, as the seats wrote them (shortened only where marked):

- **G1, GLM, major** (`origin:` the change; `kind:` contract): "ADR 0002
  declares 'The owner's lanes use a closed list, one id per
  docs/workflow.md §2 lane' … This change adds a new §2 lane with agent id
  `openai` … but does not amend ADR 0002's closed list." Disposition:
  MOOT by the owner's ruling (no new id); both seats confirmed it fixed.
- **G2, GLM, minor** and **M2, Gemini, minor** (`kind:` consistency):
  `AI.md`'s GPT-6.2 Sol row named only codex. FIXED in `ca13d46`.
- **G3, GLM, minor** (`kind:` consistency): ADR 0002's owner decision 2
  said one lane is usage-billed. FIXED in fix 2 (ADR 0002 correction).
- **M1, Gemini, major** (`kind:` contract): the embargo rule ("only to the
  Anthropic and OpenAI lanes") would silently take in the API route.
  Disposition: the owner ruled that either route counts as the OpenAI
  lane, embargoed work included; `ca13d46` says so in §2. GLM: MOOT;
  Gemini: FIXED.
- **M3, Gemini, minor:** `AI.md`'s design row lacked the "`gpt-6.2-sol`
  only" limit. MOOT: the API route serves `gpt-6.2-astra` too (above).

Round 2:

- **GLM, minor** (`origin:` fix 1; `kind:` missing mechanism): "the review
  record names the route" was in no record checklist. FIXED in fix 2
  (`docs/reviews/README.md`, the skill, §2's judgement-seat bullet).
- **GLM and Gemini, minor** (`kind:` consistency): ADR 0002's `codex` row
  described the codex CLI only. FIXED in fix 2 (ADR 0002 correction).
- **GLM, minor:** G3 still open. FIXED in fix 2.

## Credit

Both seats' last verdicts passed, with minor findings fixed as recorded
above and nothing else changed: GLM (`glm-5.3`) and Gemini
(`gemini-4-carbon-medium`).
