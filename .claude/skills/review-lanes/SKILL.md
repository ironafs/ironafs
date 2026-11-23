---
name: review-lanes
description: MAINTAINER'S CONTROLLER SESSION ONLY (the maintainer's own session, set up by their private instructions; it needs the maintainer's local lane tooling). Other agents never use it. Use before dispatching any review, design, research or refutation seat in this repository (codex/GPT, GLM, Gemini via agy, DeepSeek, Claude subagents) — which lane takes which seat per change class, the approvals for this repo, model pins and effort, the brief template, the did-it-run checks, and how the result becomes a docs/reviews record. Triggers on "review", "second opinion", "cross-model", "codex", "gemini", "glm", "deepseek", or a class P/S/R change reaching its review step.
---

# Review lanes for IronAFS

**For the maintainer's controller session only** (set up by the maintainer's
private instructions).
It relies on the maintainer's local lane tooling. Other contributors' agents
skip it; a contributor's PR is reviewed by the maintainer's lanes after it is
opened.

This skill says **which** seat runs and **what it must contain** for this
repository. **How** to run each lane is in the global skills: `codex-review`,
`gemini-review`, `glm-review` and `deepseek-review`. Use their commands; do
not copy them here.

## Approvals for this repository

All four outside families are approved for IronAFS source (owner,
2026-11-18). CLIs only; API-platform lanes are not used.

## Which seat, by class

Use "Seats by class" in `docs/workflow.md` §2; it is authoritative, and
`AI.md` explains it. Required seats always come from a family that did
not write the change, as §2 defines it (any of its text or code, or the
design it was built from), counting outside contributors' declared tools. Security reviews go to GPT read-only, a Claude seat, GLM or Gemini
Carbon (at `gemini-4-carbon-medium`), whichever wrote neither the design
nor the code, never to DeepSeek. Embargoed security
work goes only to the Anthropic and OpenAI lanes until disclosure
(`docs/workflow.md` §2). A same-family review is an extra
self-check, never a required seat.

## Model pins and effort

The model and effort for every seat come from the routing table in
`docs/workflow.md` §2, "Model routing", after the seat's family is chosen
(the judgement seat checks each candidate's availability at that pin
first, step 1 below). In short:

| Lane | Light | Standard | Ordinary class S and R reviews | Strongest |
|---|---|---|---|---|
| codex (GPT) | `gpt-6.2-sol` medium | `gpt-6.2-sol` high | `gpt-6.2-sol` xhigh | `gpt-6.2-astra` xhigh |
| gemini | `gemini-3.8-flash-medium` (helpers), `gemini-4-argon-medium` (gathering) | `gemini-4-carbon-medium` | `gemini-4-carbon-high` | `gemini-4-carbon-high`; `gemini-4-carbon-medium` for security reviews and security disputes |
| glm | `glm-5.3-flash` | `glm-5.3` high | `glm-5.3` high | `glm-5.3` max |
| deepseek | `deepseek-flash` default | `deepseek-flash` default | `deepseek-flash` default | `deepseek-flash` max |
| Claude | Haiku (helpers), Sonnet (gathering) | Opus 5.5 | Opus 5.5 | Mythos 5.2 (`fable` alias, serves `claude-mythos-5-2`) |

- Light models never hold a required review seat. Fix verification uses the
  pin of the seat whose finding it verifies.
- Research gathering stays light or standard; a load-bearing question's
  view is formed on the strongest tier, reading the gathered dossier
  inlined, without web access; any other view on the standard tier. Claude Code
  subagents for gathering pass a lighter `model` explicitly instead of
  inheriting the session's.
- Promotion only for implementation, after two recorded failures on the
  same defect at the seat's pin.
- DeepSeek runs off-peak only. Check `agy models` before pinning a Gemini
  id; ids change.
- A resumed codex session does not keep its model: pass `-m` and the effort
  again on every `codex exec resume`, and read the served model from the
  lane's log.

## Judgement seat

The rules are in `docs/workflow.md` §2, "Judgement seat". Use it when two
seats disagree on a point the work depends on and checked evidence does not
settle it, for any finding still disputed after the fix rounds, and for a
single seat's tail of one or two majors (below, "After the run"). For a
tail, A is the finding as the seat wrote it and B is the change's own text
it concerns, both quoted from their sources, never written by you.

1. **Choose the seat.** Exclude the work's authors, both disputing
   families, any family that took a side, and the controller's family if
   the controller argued a position, and any family unavailable at the
   pin `docs/workflow.md` §2, "Model routing", assigns to this seat.
   Security disputes: only GPT read-only, Claude, GLM or
   Gemini Carbon (at `gemini-4-carbon-medium`).
   Embargoed work: only Anthropic and OpenAI, and nothing about the dispute
   goes elsewhere. Then take the first remaining in the order GLM, GPT
   read-only, DeepSeek, Gemini, Claude. None left: the change waits and the
   blocker is recorded.
2. **Write the brief.** Open it with the two lines of the brief template
   below. Inline both positions as A and B. Check the whole
   brief, and anything it points to, for anything that shows who took
   either position (family, model or lane names used that way, trailers,
   transcripts, review records of the dispute); send none. Keep a technical
   name only when it does not reveal who took a side; otherwise use a
   neutral label and a source excerpt. If the evidence cannot be shared
   anonymously, the judgement stays pending.
3. **Give it the evidence path.** Ask for each disputed fact checked at a
   primary source and labelled CONFIRMED with its link or UNVERIFIED. Give
   web access where the lane has it. For Gemini, inline the primary-source
   passages with their citations; if they cannot be inlined, use another
   family.
4. **Ask for a verdict:** A, B, a named alternative, or INCONCLUSIVE, with
   the exact rules or fix that follow.
5. **Check it.** Run the did-it-run checks below. Confirm every factual
   premise of the ruling at its source yourself. A premise that fails or
   stays UNVERIFIED keeps the dispute open: back to the seat with the
   evidence. Meanwhile the owner may choose a course of action, never a
   fact. If the work is changed so it no longer depends on the disputed
   fact, before or after the seat runs, record the judgement as moot once
   you have checked that, and keep every other review and merge gate.
6. **Record** the positions, the seat, the served model and effort, the
   deciding facts and your check. Contested rulings and owner decisions go
   to the owner. Until the dispute is settled, the merge waits.

## Brief template

Every brief starts with "Run no credential command. Run no command that
lists or prints git remotes or configuration." (`docs/workflow.md` §4).
That holds for review, design, research, refutation and judgement briefs,
and for implementer and fix-round briefs. A seat's brief then gives:

1. **What:** a commit range pinned by sha at both ends: for a first
   review, from the merge base (`$(git merge-base main <head>)..<head>`,
   written out as shas); for a fix verification, the fix's own commits.
   For a design in the working tree, the file and its sha256 prefix. And
   one bounded question.
2. **Context:** the one-line repository description, decisions that are fixed
   and must not be argued, and what is already known.
3. **The consistency pass** (`docs/workflow.md` §3a) as a named item when the
   diff touches `crates/`: inventory every new function and compare it with its
   neighbours.
4. **Output rules:** findings with location, defect, consequence and fix; no
   style nits; unverifiable claims go in `uncertain`.
5. **Exclusions and scope**, before the artefact: "Do not read
   `CLAUDE.local.md`, `ironafs.code-workspace` or `.claude/skills/local-*/`;
   read only the files in scope." The scope is a manifest of the expected
   paths, or of the inlined parts.
6. **Verified facts**, when a round follows earlier ones: the things already
   executed (commands, results, revisions), given as evidence, never as a
   list of what not to raise.
7. **No other implementation's source**, also before the artefact: never
   include OpenAFS, AuriStor or kernel AFS/Rx source, or name their files, functions or internal
   identifiers (ADR 0001, "AI models"). The protocol-milestone similarity
   audit is a separate seat, run outside the repository.
8. **The check code goes last**, after the inlined artefact or file list, with
   an end marker the lane must confirm it saw. Placed before the artefact, it
   cannot detect a prompt that was silently cut short.

**A fix-round brief** (for the implementer) opens with the same two
lines, names the findings to fix, quoted as the seat wrote them, and says:
"Fix only what each finding names. Add no new mechanism; a needed
mechanism becomes a specification item or a change of its own, and is
reported, not written."

**An issue brief before it is filed or dispatched** gets one non-author
review and one focused pass by a second non-author family on the same
revision (the draft file and its sha256 prefix). After two fix rounds the
stop rule below applies to the brief: a further fix round needs the
owner's permission for that brief alone (`docs/workflow.md` §3, step 3).
Defer a design point only as that step allows.

**When round 3 concludes**, write the convergence note (`docs/workflow.md`
§3, step 7), and bring it up to date after each later verification round.
Check it against the stop rule below before the next fix round is
written; no fix round past the second is written or dispatched before the
owner's answer.

For Gemini, inline the artefact and forbid tools. Split inlined material at
file boundaries, keeping each part under a measured size (about 100 KB). A
single file over the cap stops the run; it is never sent truncated. Send
nothing private, and no memory notes.

Every run has an explicit timeout, and its transcript is kept as the record
of which files the lane opened.

## Did it run?

Check before believing any verdict. A run that fails any check is void, never
a pass:

- **Served model and effort:** they are the ones pinned, read from the lane's
  own log (the codex transcript header or rollout, the GLM gate line, the
  Gemini envelope plus the pinned id), not from the flags passed. Unknown
  provenance is INCONCLUSIVE; a default effort can silently mean no
  reasoning.
- **Completion:** the turn completed. A stream with no completion event is
  void, even if it holds a well-formed "PASS".
- **Check code and end marker:** both were echoed.
- **Output:** it parses and has a verdict. A lane that hit its output limit
  is retried once, then reported INCONCLUSIVE.
- **Gemini's thinking tokens** are above a measured floor: refusals and empty
  runs have shown about a quarter of a real review's count.
- **Real files:** every finding cites a file that exists in the reviewed
  range or tree. A report citing missing files is refused.
- **Full scope seen:** compare the scope manifest with what the lane was
  given or opened (the transcript's tool reads, or the inlined parts). An
  unexplained gap is INCONCLUSIVE.
- **Unchanged input:** for a working-tree review, the reviewed tree id or file
  hash still matches.
- **Implementer reports:** read which tests actually ran. A sandbox without
  network access can report "passed" with the deciding socket test never
  run. Grant network access only to an implementer run that needs it, set
  per run.
- **Rounds that don't count:** an inconclusive round never becomes the
  baseline that the next, diff-scoped round starts from. Each lane's
  last-reviewed commit advances only on a conclusive round. An inconclusive
  round's findings are still verified and recorded.
- **Frozen during a round:** neither the reviewed files nor the lane
  tooling change while a round runs; the tooling runs from a copy taken at
  dispatch. A change to either voids the round.

## After the run

- Verify every finding against the files before acting. Fix, refute with
  evidence, or defer with where it is tracked. Never accept or reject
  wholesale, and never count votes.
- **Your own rulings get a refutation seat.** Before your refutation,
  downgrade or deferral closes a blocker or major, or a move of a confirmed
  finding of any severity into a specification item (a deferral) closes
  it, a family that wrote none of the change, did not raise the finding
  and is not yours reviews it. The move stands only if that seat agrees
  the finding asks for mechanism the change need not decide. A
  disputed claim about what code does at run time goes to a seat that runs
  it; reading alone never settles it.
- Write the record: in the ADR's Review section for an ADR, otherwise in
  `docs/reviews/YYYY-MM-DD-<slug>.md`. Give the seat, served model, effort,
  verdict, and each finding with its disposition. Redact paths outside the
  repository, hostnames, cell names and keys. Also:
  - tag each finding `origin:` (pre-existing, or introduced by fix round
    N) and `kind:` (correctness, contract, consistency between parts,
    missing mechanism, or other);
  - give each round a row that notes each dispatch's quota headroom
    before and after, where the lane reports it, or "not reported"; no
    cost figures (`AI.md`);
  - put "uncalibrated on IronAFS" beside each verdict from a pin that has
    not passed a calibration on IronAFS material (`docs/workflow.md` §2,
    "Model routing");
  - from round 3 on, the convergence note: each seat's trend of blocker and
    major counts and of open confirmed correctness or contract findings of
    any severity, how many of the last round's findings the last fix
    introduced, and any drift in the kind of defect found;
  - for each fix round past the second: your request, your recommendation,
    and the owner's answer with its date.
- Only seats whose final verdict covers the landed tree go into
  `AI-Reviewed-by:` (ADR 0002).
- Stop rule: stop when a round produces no blocker or major, every
  confirmed correctness or contract finding is resolved, whatever its
  severity, and no judgement is pending or inconclusive. After two fix
  rounds on one change, split it, move the mechanism into specification
  items, redesign it, or record the blocker. A further fix round needs the
  owner's permission for that change alone: you ask, with the latest
  convergence note and a recommendation, and recommend another round only
  when the note shows the counts falling, few findings introduced by the
  fixes, and the kinds narrowing (`docs/workflow.md` §3, step 7). A
  permission covers the fix rounds the owner names (one, unless they say
  more) for that change and never carries to another; before each, check
  the updated note and ask again if it no longer shows convergence. A convergence note alone never
  extends the limit. A scoped fix of an upheld tail, or of a finding still
  disputed after the fix rounds, counts as a fix round. A part split off
  unconverged keeps the parent's count; a redesign starts a new count. If
  too few independent families are available, the change waits.
- A single seat's tail (one or two majors, no blocker, from one seat
  alone) goes to the judgement seat or, through the refutation seat, into
  specification items; never into another full round. The judgement seat
  rules whether each major holds as a correctness or contract defect in
  what the change decides:
  one that holds is fixed by a scoped fix verified at its seat's pin (a
  fix round, under the stop rule above), or
  the change is split, redesigned or its blocker recorded; one that does
  not hold is closed as ruled and recorded, and any mechanism it asks for
  becomes a specification item.
- A part is converged when each required seat's last conclusive verdict
  covered its files at a pinned sha and no finding in them is open. Split
  it from the latest sha every required seat's verdict covers for those
  files (a verdict covers a later sha only if every change to the files
  since was a fix verified at its finding seat's pin), as a change of its
  own, with its own class and merge; its record carries the part's
  findings and their dispositions. The rest keeps the change's fix-round
  count.
- Credit follows ADR 0002 unchanged. For class I and above, if no
  required seat's final verdict covers the landed tree, a verification
  round on the final diff comes before the merge.
