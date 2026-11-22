<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: Gemini Carbon may hold the security seat

## The change

Class P. The owner's decision of 2026-11-22.

- Until now, security reviews (class S and R) went to GPT read-only, a
  Claude seat or GLM, "never Gemini, which declined security-framed briefs
  in our runs". The owner asked for that to be re-tested, and for Gemini to
  be admitted if it engaged.
- **The decision:** Gemini Carbon becomes an eligible security-review
  family, pinned at `gemini-4-carbon-medium`, the pin the evidence below
  covers (the owner prefers medium over high for Gemini seats). It is also
  eligible for the judgement seat in security disputes, at the same pin.
- **Where it is written:**
  - `docs/workflow.md` §2: "Security review", the judgement seat's filter
    for security disputes, the "Model routing" paragraph, the strongest
    row of its table, and its "Precedence" rule, which names Gemini's
    security seats as the one exception to "never on a lower tier";
  - `AI.md`: the Gemini family row, the class S row and the Gemini row of
    "What each model can do here";
  - the `review-lanes` skill: "Which seat, by class", the model pins
    table's strongest column, and the judgement seat's filter.
- **Not changed:**
  - embargoed security work goes only to the Anthropic and OpenAI lanes
    until disclosure;
  - DeepSeek stays excluded from security reviews and security disputes;
  - Gemini still gets the artefact inlined, without tools, split at file
    boundaries under the measured size cap, and a single file over the cap
    stops the run;
  - every Gemini seat's did-it-run checks stay: the thinking-token floor,
    the sentinel and the structured output;
  - Gemini's other pins (`gemini-4-carbon-high` for the other class S and
    R seats and the strongest row) and the first-reviewer and judgement
    seat orders.
- **Written by** `claude-code:claude-opus-5-5`.

## The evidence

Executed by the controller on 2026-11-22.

- **Runs.** Gemini `gemini-4-carbon-medium` through agy, with no shell or
  file access and the diff inlined, ran the six ranges of the calibration
  suite of issue #48 with the suite's own security-review briefs: the
  framing it had declined before. All six runs completed (status SUCCESS,
  the sentinel echoed, a structured verdict). None declined.
- **Score.** Scored against the suite's hashed answer key (its SHA-256,
  `f82393cd…d2e0`, checked equal before scoring): 10 of 12 planted defects
  found, all 6 critical ones found, no false blocker or major, and the
  clean range passed with no finding. #48's pass rule is every critical
  defect found and at least 9 of 12.
- **Misses.** The same two non-critical defects it missed with neutral
  briefs on 2026-11-21: payload bytes not recorded for replay, and a usage
  text that omits `--keep-going`.
- **Comparison,** on the same suite with neutral briefs: Gemini
  `gemini-4-carbon-medium` 10 of 12 (6 of 6 critical); GLM `glm-5.3` high
  11 of 12 (6 of 6 critical).
- **Thinking tokens per run:** 9,818; 2,675 (the clean range, a short
  PASS); 12,262; 19,314; 24,094; 20,132.

This is the recorded benchmark that "Changing a pin" in
[docs/workflow.md](../workflow.md) §2 asks for, item by item:

- **Known defects, a clean case, misses, false findings, refusals and a
  pass threshold:** as above.
- **Cost:** the agy envelope reports tokens, not cost, so the thinking
  tokens above are the cost information available. No cost figures are
  published per change ([AI.md](../../AI.md)).
- **Comparison with the current pin:** Gemini had no earlier security pin;
  `gemini-4-carbon-medium` is a newly eligible pin for these seats. The
  neutral-brief runs of 2026-11-21 (Gemini and GLM, above) are its
  baselines.
- **What the runs exercised:** `gemini-4-carbon-medium` on security-review
  briefs. No judgement-seat brief was run. Gemini's eligibility for the
  judgement seat in security disputes, at the same pin, is the owner's
  decision.

## The sweep

Every hit of `grep -rn -i "gemini"` over `docs/`, `AI.md`, `AGENTS.md`,
`CONTRIBUTING.md`, `.claude/skills/` and `.github/` was read.

| Where | What it says | Done |
|---|---|---|
| `docs/workflow.md` §2, roles table: implementer when codex is out of quota; second family | Gemini as implementer and second family | unchanged: not a security seat |
| `docs/workflow.md` §2, first-reviewer fallback order | GLM, GPT read-only, Gemini, DeepSeek, Claude | unchanged |
| `docs/workflow.md` §2, "Second family" | Gemini among the second families | unchanged |
| `docs/workflow.md` §2, "Security review" | "Never Gemini, which declined security-framed briefs in our runs" | changed: Gemini Carbon admitted at `gemini-4-carbon-medium`, inlined and without tools, linked to this record |
| `docs/workflow.md` §2, embargoed fixes | seats that need another family (GLM, Gemini, DeepSeek) run after disclosure | unchanged: the embargo stays |
| `docs/workflow.md` §2, judgement seat order | GLM, GPT read-only, DeepSeek, Gemini, Claude | unchanged |
| `docs/workflow.md` §2, judgement seat, security disputes | "never Gemini or DeepSeek" | changed: Gemini Carbon at `gemini-4-carbon-medium` added; never DeepSeek |
| `docs/workflow.md` §2, judgement seat, how it rules | a seat without tools (Gemini) gets the passages inlined | unchanged |
| `docs/workflow.md` §2, "Model routing" paragraph | "never Gemini or DeepSeek" | changed: Gemini Carbon added; never DeepSeek |
| `docs/workflow.md` §2, routing table, light, implementation and ordinary S/R rows | Gemini pins | unchanged |
| `docs/workflow.md` §2, routing table, strongest row | `gemini-4-carbon-high`, where eligible | changed: adds `gemini-4-carbon-medium` for security reviews and security disputes |
| `docs/workflow.md` §2, "Precedence" (no Gemini hit) | a strongest-row seat never runs on a lower tier | changed: Gemini's security seats named as the one exception |
| `docs/workflow.md` §2, research | a seat without tools (Gemini) gets excerpts inlined | unchanged |
| `docs/adr/0001-licence-and-dont-copy.md`, its review table | a past Gemini verdict | unchanged: history |
| `docs/adr/0002-ai-use-and-attribution.md`, trailer example and agent-id table | `gemini` agent id | unchanged |
| `docs/adr/0004-architecture.md`, Review section | Gemini's design seat in round 1 | unchanged: history |
| `docs/reviews/`, twelve records (75 lines) | past Gemini runs, pins and verdicts | unchanged: records of past rounds; none states the security rule |
| `AI.md`, list of families; first commit's seats | Gemini named | unchanged |
| `AI.md`, families table, Gemini row | `gemini-4-carbon-high` for class S and R and the strongest seats | changed: medium also for its security seats; high for the other S and R and strongest seats |
| `AI.md`, roles table, second family | Google among the second families | unchanged |
| `AI.md`, class S row (no Gemini hit before) | security review by GPT, Claude or GLM | changed: "GPT, Claude, GLM or Gemini" |
| `AI.md`, "What each model can do here", Gemini row | Security review: "no: it declined security-framed briefs in our runs" | changed: "yes, at `gemini-4-carbon-medium`, when it wrote neither the design nor the code", linked to this record |
| `AI.md`, trailer example | `gemini` agent id | unchanged |
| `AI.md`, routing summary and judgement-seat line (no Gemini hit) | "the strongest tiers for the named seats"; a third family rules "using its strongest model" | changed in fix round 1: both point to §2's "Model routing" and its "Precedence" rule |
| `.claude/skills/review-lanes/SKILL.md`, description and the global skills list | Gemini via agy, `gemini-review` | unchanged |
| `.claude/skills/review-lanes/SKILL.md`, "Which seat, by class" | "never to Gemini" | changed: Gemini Carbon at `gemini-4-carbon-medium` added; never DeepSeek |
| `.claude/skills/review-lanes/SKILL.md`, pins table, gemini row | strongest `gemini-4-carbon-high` | changed: adds `gemini-4-carbon-medium` for security reviews and security disputes |
| `.claude/skills/review-lanes/SKILL.md`, "Check `agy models`" | pinning ids | unchanged |
| `.claude/skills/review-lanes/SKILL.md`, judgement seat step 1 (security disputes, no Gemini hit before) | "only GPT read-only, Claude or GLM" | changed: Gemini Carbon at `gemini-4-carbon-medium` added |
| `.claude/skills/review-lanes/SKILL.md`, judgement seat order and step 3 | order; inline passages for Gemini | unchanged |
| `.claude/skills/review-lanes/SKILL.md`, inlining and did-it-run checks | Gemini inlined without tools; served model; thinking-token floor | unchanged: these stay for every Gemini seat |
| `AGENTS.md`, `CONTRIBUTING.md`, `.github/` | no hit | nothing to do |

Outside the sweep's paths, `README.md` lists Gemini 4 Carbon's role as
"Second-family reviews". It does not state the security rule and was left
as it is.

## Review

To be filled by the controller: the two seats' verdicts, with range, seat,
served model, effort and findings.

| Round | Range | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|
| 1, review | | | | | |
