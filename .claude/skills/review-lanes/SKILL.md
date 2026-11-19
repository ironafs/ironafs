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
`AI.md` explains it. Required seats always come from a family that wrote
neither the design nor the code, counting outside contributors' declared
tools. Security reviews go to GPT read-only, a Claude seat or GLM, whichever
wrote neither the design nor the code, never to Gemini. Embargoed security
work goes only to the Anthropic and OpenAI lanes until disclosure
(`docs/workflow.md` §2). A same-family review is an extra
self-check, never a required seat.

## Model pins and effort

| Lane | Model | Effort |
|---|---|---|
| codex (GPT) | `gpt-6.2-sol` (`gpt-6.2-astra` only after Sol fails twice) | `high` for I/P and docs; `xhigh` for S/R and security |
| gemini | `gemini-4-carbon-medium`; `gemini-4-carbon-high` for S/R | in the id |
| glm | `glm-5.3` | `high` |
| deepseek | `deepseek-flash` | per its skill; off-peak only |
| Claude | Opus 5.5 by default; Mythos 5.2 (`fable` alias, serves `claude-mythos-5-2`) for hard problems and every Claude security review | default |

Check `agy models` before pinning a Gemini id; ids change.

## Brief template

Every brief starts with "Run no credential command." and then gives:

1. **What:** a commit range pinned by sha (or, for a design in the working
   tree, the file and its sha256 prefix), and one bounded question.
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
  baseline that the next, diff-scoped round starts from.

## After the run

- Verify every finding against the files before acting. Fix, refute with
  evidence, or defer with where it is tracked. Never accept or reject
  wholesale, and never count votes.
- Write the record: in the ADR's Review section for an ADR, otherwise in
  `docs/reviews/YYYY-MM-DD-<slug>.md`. Give the seat, served model, effort,
  verdict, and each finding with its disposition. Redact paths outside the
  repository, hostnames, cell names and keys.
- Only seats whose final verdict covers the landed tree go into
  `AI-Reviewed-by:` (ADR 0002).
- Stop rule: stop when a round produces no blocker or major, and every
  confirmed correctness or contract finding is resolved, whatever its
  severity. After two fix rounds on one change, split it, redesign it, or
  record the blocker. If too few independent families are available, the
  change waits.
