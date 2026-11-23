<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Workflow

How changes are made, checked and reviewed in IronAFS. Change it by commit,
with the reason in the commit body.

**The idea in one paragraph.** Every change gets a *class* that says how much
damage a plausible mistake in it could do. The class decides everything else:
what must be checked, which models review it, and who may merge it. Routine
changes stay fast; risky ones get more eyes. Claude Code runs the work as the
controller and, through the subagents it dispatches, writes most of the code;
other model families review it, so that no model is the only one checking
its own work.

## 1. Classes: the ranking

Ask: *if this change has a plausible mistake, what goes wrong?* Pick the
highest class that fits. When unsure, pick the higher one and say so. Split a
change so most of it lands in a lower class.

| Class | A plausible mistake… | Needs before merge |
|---|---|---|
| **E** (editorial) | changes nothing the project claims or does | Links resolve; re-read the diff |
| **I** (internal) | changes internals only: refactors, tests, fixtures, harnesses | Tests and clippy green; one review by the first reviewer for the change's author (§2, "Seats by class") |
| **P** (public) | changes what a user, admin or peer *sees*: a CLI flag or output, a config key, an error message, a documented contract | Everything for I, plus the contract written first and one review by a family that did not write the change |
| **S** (safety) | breaks security, data, consistency or concurrency: untrusted input gets past a check, data is lost or served stale, a token leaks or reaches the wrong user, a task wedges or races | Everything for P, plus the design reviewed by another family *before* coding, the failure reproduced in a test, `/security-review`, and two non-author families when it touches tokens, the wire or cache coherency |
| **R** (irreversible) | can't be undone: a wire or on-disk format we promise to keep, a release, making the repository public, a licence change | Everything for S, plus the owner's decision |

### What that means for an AFS client

| Change | Class | Why |
|---|---|---|
| Fix a typo in a doc | E | Nothing changes |
| Rename a private helper, extract a function unchanged | I | No input, output or stored byte can differ |
| Add tests for the XDR decoder | I | Tests never inherit the class of what they test |
| New `afs` subcommand or `--json` field | P | Users and scripts see it |
| Output of the `fs`/`tokens` compatibility aliases | P | Old scripts parse it |
| Config schema or a registry/plist key | P | Admins depend on it |
| Rx or XDR parsing, anything decoding fileserver replies | S | Hostile or broken servers send this data |
| Callback handling, cache invalidation, data cache | S | A mistake serves stale or wrong data |
| Token storage, per-user token lookup, the local IPC caller check | S | A mistake leaks a token or lets one user act as another |
| `unsafe` code, FSKit, SSPI or other OS bindings (no C Rx: ADR 0001) | S | Memory safety rests on hand-checked invariants |
| SMB message parsing, SMB session authentication and identity mapping | S | Local processes send this data, and a mistake lets one user act as another |
| Locking, async tasks, shutdown | S | Races and deadlocks |
| A cache format on disk we promise to read in later versions | R | Users' caches depend on it forever |
| First public release, going public, relicensing | R | Can't be taken back |

## 2. Roles and lanes

The lanes, dispatch and merges below are run by the maintainer's controller
session. A contributor does not dispatch lanes: they open an issue or a PR,
and the maintainer arranges the reviews the class needs (`CONTRIBUTING.md`).

All lanes were approved for this repository by the owner on 2026-11-18.
**Only CLIs are used**: Claude Code, the codex CLI and the agy CLI. GLM and
DeepSeek run through headless Claude Code on their own endpoints. API-platform
lanes are not used.

| Role | Who | How |
|---|---|---|
| **Controller** | the maintainer's Claude Code session (following the maintainer's private instructions) | Plans, briefs, routes, verifies findings, asks the owner for any fix round past the second (§3, step 7), lands changes after the owner's approval. Writes code itself only for work only it can run; other code comes from the implementers it dispatches. |
| **Implementer** | Claude Code subagents, Opus 5.5 | Dispatched by the controller, one task at a time, with a written brief, test first. The default from M2a: the designs are Claude-written, so Claude implementing keeps one author family and leaves GLM and GPT free for the required and security seats. |
| **Implementer for extra capacity** | codex CLI, `gpt-6.2-sol` | `codex exec` with a written brief, on a task separate from Claude's; `medium` effort for mechanical I/P work, `high` for S and R. Its seats exclude OpenAI and the design's family. |
| **Implementer when codex is out of quota** | agy CLI (Gemini) | Same brief; a session allowlist that denies git. Brief paths must sit inside `--add-dir`. |
| **First reviewer of implemented code** | GLM 5.3 (`glm-review`) | Headless Claude Code on Z.ai's endpoint, read-only. |
| **Second family when the class needs two** | GPT read-only, Gemini Carbon via agy (`gemini-review`), DeepSeek (`deepseek-review`) or a Claude seat, whichever wrote none of the change and is not already the first reviewer | DeepSeek off-peak only (not Mon–Fri 01–04 or 06–10 UTC). |
| **Design, rulings, research, refutations on work codex did not write** | GPT read-only: `codex exec -s read-only -m gpt-6.2-sol - < brief.md`; `-m gpt-6.2-astra -c model_reasoning_effort=xhigh` for the strongest row (§2, "Model routing") | GLM second when a second view is needed. |
| **Claude seats** (review, reproduction, hard judgement, security) | Claude Code subagents: Opus by default; Mythos 5.2 (`claude-mythos-5-2`, Claude Code's `fable` alias) for the strongest row of §2's "Model routing", including every Claude security review | Never a required seat on Claude-written work. |

**A family never reviews its own work.** Codex is OpenAI-family, so a GPT
seat never reviews codex-written code. A same-family review (for example
`/code-review` on Claude-written work) may run as an extra self-check, but it
never counts as a required seat. A finding raised by one family is
refuted, if at all, by a family that neither wrote the change nor raised it.

- **The controller's rulings are reviewed.** When the controller refutes,
  downgrades or defers a blocker or major, or moves a confirmed finding of
  any severity into a specification item (§3, step 7; such a move is a
  deferral), a refutation seat reviews that ruling before it closes the
  finding: a family that wrote none of the change, did not raise the
  finding, and is not the controller's.
- **Runtime claims are settled by running code.** A disputed claim about
  what code does when it runs is settled by a seat that runs it (a
  reproduction, §3c's EXECUTED), never by reading alone.

### Seats by class

**Rule:** every required seat comes from a family that did not write the
change. A family *wrote* a change if it wrote any of its text or code,
including fixes it wrote into it; a family that wrote a design is also an
author of the code built from it. Drafting the commit message, a brief or
the review record does not count. Nor does having reviewed an earlier
draft, gathered or synthesised research the change draws on, or proposed
fixes that another family wrote in, as long as none of that family's text
is in the change; such a family may hold a required seat, and the review
record says what its earlier part was. Outside contributors' declared
tools count. Most code is written by Claude Code subagents the controller
dispatches; codex adds capacity on separate tasks.

**First reviewer, by author.** The table is an order of preference. Skip any
family that wrote the design or the code. Each row's own order comes first;
if every family it names is excluded, the seat goes to the first eligible
family in this order: GLM, GPT read-only, Gemini, DeepSeek, then a Claude
seat.

| Author | First reviewer |
|---|---|
| codex (OpenAI) | GLM |
| agy (Google) | GLM |
| Claude | GLM, or GPT read-only |
| outside contributor | GLM, or GPT read-only if the contributor declared GLM |
| the owner, with no model writing | GLM, or GPT read-only |

**Second family** (when the class needs two): any of Gemini, DeepSeek, GPT
read-only or a Claude seat whose family did not write the change and is not
already the first reviewer.

**Security review** (class S and R): GPT read-only, a Claude seat (always
Mythos 5.2), GLM, or Gemini Carbon (always `gemini-4-carbon-medium`),
whichever family wrote neither the design nor the code. Choose it when the
design is assigned, so that one stays free. Gemini gets the artefact
inlined, without tools, like every Gemini seat; it holds this seat on the
evidence of its security-framed calibration runs
([record](reviews/2026-11-22-gemini-security-seat.md)).

**Embargoed security fixes.** Until a vulnerability is disclosed, its report,
design, fix, tests and review records go only to the Anthropic and OpenAI
lanes (`SECURITY.md`):

- One of the two families writes the design and the fix; the other wrote
  neither and reviews both, including the security review and the
  consistency pass (§3a). That review replaces,
  for the embargo only, the rule that the pass comes from a family other than
  both the author's and the controller's.
- Seats that need another family (GLM, Gemini, DeepSeek) run after
  disclosure, before the fix is recorded as done. If a fix cannot ship safely
  without them, it waits.
- **Where the work lives:** before the repository is public, a local branch
  that is never pushed. Once it is public, an unpushed local branch, or a
  private GitHub security advisory's temporary private fork used only for
  preparing the fix. At disclosure the fix lands through a normal, approved
  squash PR, never the advisory's merge button, and the advisory is published
  with it. No public issue, comment, branch, commit message or review
  record mentions it until disclosure. At disclosure, the public issue and
  the review record are written (§6).

| Class | Required review seats | Also required |
|---|---|---|
| **E** | none (the owner re-reads it) | links resolve |
| **I** | the first reviewer | tests and clippy green |
| **P** | the first reviewer, and the contract reviewed by a non-author family before code | the contract written first |
| **S** | the design reviewed before code by a non-author family; the code by the first reviewer, plus a second family when it touches tokens, the wire or cache coherency; a security review | a test that reproduces the failure |
| **R** | as S | the owner's decision |

**Stop rule:** if too few independent families are available (quota, an
outage), the change waits and the blocker is recorded. The bar is never
lowered to fit.

### Judgement seat

Disagreements are settled on evidence, never by counting.

- **When it runs.** Two seats disagree on something the work depends on (a
  finding, a design choice, a research conclusion), and the controller cannot
  settle it from evidence it has checked itself. Also for a finding still
  disputed after the fix rounds, and for a single seat's tail of one or two
  majors (§3, step 7), where it rules whether each one holds. **A pending
  or inconclusive judgement blocks the merge**, like an open confirmed finding (§3, step 8), and a reproduced
  failure blocks whatever the ruling says (§4).
- **Who sits it.** A family that is independent of the dispute: it wrote
  neither the work nor either position, and has not taken a side in it. The
  controller's family is excluded when the controller argued a position.
  Filter every family first, then take the first that remains in this
  order: GLM, GPT read-only, DeepSeek, Gemini, a Claude seat. The filters
  are independence (above), availability at the pin "Model routing"
  assigns to this seat, and:
  - **Security disputes** (whether a security finding holds, or anything
    that would overturn part of a security review): only the
    security-review families (GPT read-only, a Claude seat, GLM, Gemini
    Carbon at `gemini-4-carbon-medium`), never DeepSeek.
  - **Embargoed work:** only the Anthropic and OpenAI lanes, and nothing
    about the dispute leaves them (brief, source lookups, record).

  If no family passes, the change waits (the stop rule), and the blocker is
  recorded, privately under an embargo. A ruling never removes a seat the
  class requires.
- **How it rules.** The brief inlines both positions as A and B. For a
  single seat's tail, A is the finding as the seat wrote it and B is the
  change's own text it concerns, both quoted from their sources, never
  written by the controller. The whole
  brief, and anything it points to, carries nothing that shows who took
  either position: no family, model or lane name used that way, and no
  trailer, transcript or review record of the dispute; the controller checks
  it before sending. A technical name stays only when it does not reveal
  who took a side; otherwise a neutral label and a source excerpt replace
  it. If the evidence cannot be shared anonymously, the judgement stays
  pending. The seat checks each
  disputed fact at a primary source and labels it CONFIRMED, with the link,
  or UNVERIFIED. A seat without tools (Gemini) gets the primary-source
  passages inlined with their citations; when they cannot be inlined, another
  family takes the seat. The verdict is A, B, a named alternative, or
  INCONCLUSIVE.
- **After it rules.** The controller confirms every factual premise the
  ruling rests on at its source before acting on it. A premise that fails
  or stays UNVERIFIED leaves the dispute open, and it goes back to the seat
  with the evidence. Facts are settled only by evidence, whoever settles
  them. While a factual dispute stays open, the owner may choose a course
  of action within the owner's authority, such as changing the work so it
  no longer depends on the fact; the owner also decides when the ruling is
  itself contested, or when the question is the owner's to decide.
- **Moot.** When the work changes so that it no longer depends on the
  disputed fact, before or after the seat runs, the controller checks that
  and records the judgement as moot; every other review and merge gate
  still applies.
- **The record** (§4, "After the run") gives both positions, the seat, the
  served model and effort, the deciding facts with their sources, and the
  controller's check.

### Model routing

**Seat first, then model.** The family for a seat comes from the rules
above: who wrote the work, the first-reviewer order, the security-review
families (GPT read-only, a Claude seat, GLM, Gemini Carbon; never DeepSeek),
embargoed work (Anthropic and OpenAI only) and the judgement seat's order.
Only then does this table give the model and effort; the one exception
is the judgement seat, whose filters check each candidate family's
availability at the pin this table assigns to that seat. It governs over
every other model pin in this file, `AI.md` and the skills.

| Seat | GPT | Claude | Gemini | GLM | DeepSeek |
|---|---|---|---|---|---|
| **Light:** optional helpers | `gpt-6.2-sol`, medium | Haiku (`claude-haiku-5-5`) | `gemini-3.8-flash-medium` | `glm-5.3-flash` | `deepseek-flash`, default |
| **Light:** research gathering | `gpt-6.2-sol`, medium | Sonnet (`claude-sonnet-5-5`) | `gemini-4-argon-medium` | `glm-5.3`, high | `deepseek-flash`, default |
| Class I and P implementation (`high` where it is not mechanical) | `gpt-6.2-sol`, medium (extra capacity) | Opus (`claude-opus-5-5`), the default implementer | `gemini-4-carbon-medium` (when codex is out of quota) | — | — |
| Class I and P required reviews, P contracts, refutations and verifications on I and P work, research synthesis that is not load-bearing | `gpt-6.2-sol`, high | Opus (`claude-opus-5-5`) | `gemini-4-carbon-medium` | `glm-5.3`, high | `deepseek-flash`, default |
| Class S and R implementation | `gpt-6.2-sol`, high (extra capacity) | Opus (`claude-opus-5-5`), the default implementer | `gemini-4-carbon-high` (when codex is out of quota) | — | — |
| Ordinary class S and R reviews, S design review before code, refutations on S and R work, reproduction, the consistency pass | `gpt-6.2-sol`, xhigh | Opus (`claude-opus-5-5`) | `gemini-4-carbon-high` | `glm-5.3`, high | `deepseek-flash`, default |
| **Strongest:** the judgement seat, security reviews, class R designs, format freezes, architecture ADRs, promoted implementation, synthesis of a load-bearing research question | `gpt-6.2-astra`, xhigh | Mythos 5.2 (`claude-mythos-5-2`) | `gemini-4-carbon-high`, where eligible; `gemini-4-carbon-medium` for security reviews and security disputes | `glm-5.3`, max | `deepseek-flash`, max, where eligible |

- **Precedence.** A seat in the strongest row never runs on a lower tier; if
  no eligible family is available at that tier, the change waits (the stop
  rule). The one exception is Gemini's security seats (security reviews
  and the judgement seat in security disputes): they run at
  `gemini-4-carbon-medium`, the pin their calibration covers. Every
  required review starts at the standard tier or above. Light
  models only take optional helper seats and research gathering.
- **Fix verification** runs at the pin of the seat whose finding it
  verifies, scoped to the fix's diff.
- **Research has two steps.** Gathering (web searches, reading sources,
  collecting facts) runs on a light or standard tier and ends in a dossier
  that gives, for each claim, its source, a short excerpt, its uncertainty
  and any counter-evidence found. Forming a view on a **load-bearing**
  question (one an ADR, a class S or R design or a format freeze will rest
  on) runs on the strongest tier, reading the dossier inlined, without web
  access; any other view-forming runs on the standard tier. Refute and
  source-verification seats keep web access, since checking sources is
  their job; a seat without tools (Gemini) gets the primary excerpts
  inlined; embargoed work keeps its limits on the brief, the dossier, the
  lookups and the record. The controller checks the deciding facts at their
  sources before a design relies on them.
- **Promotion** outside the strongest row: only for implementation, after
  two completed attempts on the same defect fail at the seat's pinned tier,
  each recorded. Nothing else promotes a seat.
- **Changing a pin** needs a recorded benchmark: known defects and clean
  cases, misses, false findings, refusals and cost, a pass threshold, and a
  comparison with the current pin. Check `agy models` and the other lanes'
  model lists before pinning; ids change.
- **Uncalibrated pins.** Until a pin has passed a calibration on IronAFS
  material (known defects planted in IronAFS code or documents, measured as
  "Changing a pin" requires), every verdict from it is recorded with
  "uncalibrated on IronAFS" beside it.
- **Claude seats** run at the lane's default effort, which is recorded with
  the served model; the other lanes pass their effort as pinned.
- **Every dispatch and resume** passes the model and effort explicitly, and
  the served model and effort are read from the lane's own log.

`AI.md` explains these rules for readers new to the project. This file is
authoritative.

## 3. How a change runs

1. **Class it** (§1) and name what it touches. Text is classed by what it
   governs, not by its form: a copy edit is class E, while text that sets a
   contract, a design, or what review or merge require keeps its class's
   gates. Code, wire and on-disk formats, and security keep §2's seats and
   this section's closure rules in full.
2. **Spec first** for P and above: the contract (P) or design (S, R) is
   written and reviewed by another family before code.
3. **Brief the implementer** in four parts:
   - **Outcome:** what is true when it is done.
   - **Contracts:** which ones the change touches, plus the class and the
     files.
   - **Failure cases:** at least one plausible wrong implementation the work
     must avoid.
   - **Acceptance evidence:** the commands and results that prove it.

   Settle open design questions before dispatch, not during it. Every brief
   opens with the two lines of §4, carries the reuse rule (§3a), and then
   starts with a **re-check**: the claims
   and ADR decisions the task rests on are re-read at their sources first.
   A contradiction stops the task and is raised as a design issue, which
   leads to an amendment of a working decision or a superseding ADR
   ([docs/adr/README.md](adr/README.md)); it is never worked around.

   **Issue briefs before they are filed or dispatched:** one non-author
   family reviews the brief, and a second non-author family does one
   focused pass on the same revision (the draft file, pinned by its sha256
   prefix). Each acceptance criterion names the evidence that proves it,
   its prerequisites, and the issue that supplies each prerequisite. Fix
   rounds re-check only the changed text and what depends on it. Step 7's
   closure rules apply, and editorial minors are fixed and recorded. After
   two fix rounds a brief is split, redesigned, or its blocker recorded and
   the work kept gated. Step 7's rule after two fix rounds applies to a
   brief as to a change: a further fix round needs the owner's permission
   for that brief alone, one round at a time. A brief's permission never
   covers the code change implemented from it, nor the reverse. The review
   record in `docs/reviews/` for the batch of briefs filed together holds
   these reviews, two counts (review runs per brief filed, and acceptance
   defects found later at pickup or in code review), and, for a brief past
   two fix rounds, its convergence notes and permissions (step 7).

   **Deferring a design point** is allowed only if the current work is
   correct whichever way the point is decided: its acceptance criteria,
   interfaces and safety invariants hold either way. The deferral is a
   named issue, giving the decision needed and the evidence that settles
   it, linked as a blocker on the first issue that depends on it. For
   unmeasured operating-system behaviour, the measurement is the blocker.
   A specification item (step 7) is such a deferral.
4. **Author self-check**: the implementer runs the `rust-review` checklist,
   including the function inventory on its own diff. It reports any deliberate
   departure from its neighbours, with the reason.
5. **Consistency pass** (§3a) by the controller, before any review seat
   starts. One non-author review seat repeats it as a named item of its brief,
   and the controller compares the seat's inventory with its own.
   - For I, the seat is the first eligible reviewer from the seat table
     (§2), with its authorship check and fallback.
   - For P, S and R, it is a family other than both the author's and the
     controller's.
   - A function the seat missed, or a site it did not compare, is asked about
     again.
6. **Review seats** for the class (§1, §2), each with a bounded brief: the
   commit range pinned by sha (from the merge base for a first review; the
   fix's own commits for a fix verification), the question, what is
   already known. Neither the reviewed files nor the lane tooling change
   while a round runs.
7. **Fix rounds**: fix confirmed correctness and contract findings; review
   only the fix's changes. Round 1 is the first review; each fix round's
   verification is the next round, and a fix round counts once that round
   has concluded.
   - **Fix rounds add no mechanism.** A fix does only what its finding
     names. A mechanism the finding shows is needed becomes a specification
     item (an open item named in the change and tracked in an issue,
     reviewed when its specification is written) or a change of its own.
     A fix-round brief says so.
   - **A specification item never takes a defect in what the change does
     decide.** Moving a confirmed finding into one is a deferral: it closes
     the finding only after the refutation seat (§2) agrees that the
     finding asks for mechanism the change need not decide.
   - **Every finding is tagged** in the review record by origin
     (pre-existing, or introduced by fix round N) and by kind
     (correctness, contract, consistency between parts, missing
     mechanism, or other).
   - **A convergence note is written when round 3 concludes**, and
     brought up to date after each later verification round; the
     controller checks it before the next fix round is written. It gives
     each seat's trend of blocker and major counts and of open confirmed
     correctness or contract findings of any severity, how many of the
     last round's findings the last fix introduced, and whether the kind
     of defect found has drifted. It goes in the review record.
   - **After two fix rounds, stop patching**: split the change, move the
     mechanism into specification items, redesign it, or record the
     blocker. A further fix round needs the owner's permission for that
     change alone. The controller asks for it with the latest note and a
     recommendation, and recommends another round only when the note shows
     the counts falling, few findings introduced by the fixes, and the
     kinds narrowing. Each permission covers exactly one fix round and is
     asked again before the next; it never carries to another change. The
     request, the recommendation, the owner's answer and its date go in the
     review record. A convergence note alone never extends the limit.
   - **What counts.** A scoped fix of an upheld tail, or of a finding still
     disputed after the fix rounds, counts as a fix round. A part split off
     unconverged keeps the parent's fix-round count; a redesign starts a
     new count.
   - **A single seat's tail**, one or two majors and no blocker from one
     seat alone, goes to the judgement seat (§2) or, through the
     refutation seat (the specification-item rule above), into
     specification items; never into another full round. The
     judgement seat rules whether each major holds as a correctness or contract defect in
     what the change decides. One that holds is fixed by a fix scoped to
     it and verified at its seat's pin (a fix round, under the stop rule
     above), or the change is split, redesigned or its blocker recorded.
     One that does not hold is closed as ruled, like a refuted finding,
     and recorded with the ruling; any mechanism it asks for becomes a
     specification item.
   - **Converged parts are split off.** A part is converged when each
     required seat's last conclusive verdict covered the part's files at a
     pinned sha and no finding in them is open. It is split off from the
     latest sha that every required seat's verdict covers for those files
     (a verdict covers a later sha only if every change to the files since
     was a fix verified at its finding seat's pin), as a change of its own,
     with its own class, merge (step 8) and review record, which carries
     the part's findings and their dispositions; the rest continues with
     the change's fix-round count.
8. **Merge** when the class's checks are met, no confirmed correctness or
   contract finding is open (one closed by the refutation seat as a
   specification item, or by a ruling that it does not hold, is closed; a
   specification item stays open in its issue), no judgement is pending or
   inconclusive (one recorded as moot is closed), every fix round past the
   second has the owner's permission in the review record, and, for class
   I and above, at least one required seat's final verdict covers the
   landed tree under ADR 0002's `AI-Reviewed-by:` rule (if none does, a
   verification round on the final diff comes first): a pull request,
   squash-merged after the owner's approval, with the trailer block and
   sign-off of ADR 0002 (the root commit of the public history is the one
   exception; ADR 0002 gives its checklist). R also needs the owner's decision.

## 3a. Reuse and consistency

New code reuses what exists and reads like its neighbours.

- **Reuse.** Before writing a helper, find the item that already does the job
  and call it. A second copy of a step drifts from the first, and the copy is
  where a later fix gets missed. If the existing item is private, widen it
  only as far as the new caller needs.
- **Match the neighbours.** New code follows the module around it: naming,
  error and refusal style, lock and check order, doc comments and test
  layout.
- **Name departures.** A deliberate departure is named, with its reason, in
  the change's description. The implementer names it in its report, and the
  controller carries it into the description.
- **Someone else checks.** An author's own check misses this, so the check is
  done by someone other than the author: the consistency pass in the
  `rust-review` skill. Its scope includes:
  - tests;
  - test doubles and simulators, since a fake that copies a production
    sequence drifts the same way.
- **No tool replaces the pass.** No reliable automatic duplicate detection
  exists for Rust.

## 3b. Checks that earn trust

- **Clippy for every target.** A change that touches code behind an OS
  `cfg` runs `cargo clippy --all-targets --target <triple>` for Linux, macOS
  and Windows before review seats start. Clippy on the development machine
  (macOS) does not see Linux- or Windows-only code.
- **A check passes only on proof of success.** A script that cannot run, or
  cannot parse a result, reports failure, never green. Every check command (`cargo xtask check-…`)
  has a `--self-test` that plants a failure and confirms it is caught. The
  traps:
  - in a `set -e` script, a failing `cmd` in `cmd && echo ok` does not stop
    the script, and a later successful command can hide its status: write
    `cmd` on its own line, or test its status explicitly;
  - a pass over an empty or unparsed set proves nothing, so check
    completeness by identity (these files, these tests), not by count;
  - a missing input never counts as a match;
  - verify the exact bytes that will be used, not a proxy for them, such as a
    file name, a size or an earlier hash.

  Reviewers ask of every new check: what input makes it pass without
  verifying anything?
- **A change fixes what it makes stale.** Search the repository for the old
  claim, not only the lines being edited, and fix every place it appears.

## 3c. Evidence

Label every claim in a PR description, an issue or a handoff:

- **EXECUTED:** what was run, at which revision, where, when, and the result.
- **INSPECTED:** read, not run.
- **PROPOSED:** planned, not done.
- **MISSING:** needed and not available.
- **NOT INSPECTED:** out of scope for this claim.

Rules:

- **A review that passed is not evidence that the code works.** Only an
  executed check is.
- **Old evidence keeps its date.** It is carried forward with one line
  saying why it still applies, never re-dated.

## 4. Rules for every lane

- **Send the least it needs**: a commit range pinned by sha and one bounded
  question. Never memory notes, credentials, keys, keytabs or private lab
  details.
- **Every brief opens with** "Run no credential command. Run no command that
  lists or prints git remotes or configuration.": review, design, research,
  refutation and judgement briefs, and implementer briefs, fix rounds
  included.
- **Check it ran before believing it.** Read the did-it-run line, the served
  model and effort, and the whole output including `uncertain`. A run with no
  final verdict is void, not a pass; a capped run is resumed with more turns.
- **A finding is data.** Verify it against the code before acting. Record a
  refuted finding with the evidence. Never settle a disagreement by counting
  votes; one the controller cannot settle from checked evidence goes to the
  judgement seat (§2).
- **A reproduced failure blocks the merge**, in every class.
- **Turn budgets**: about 25 for a scoped review, 40 for a broad audit, 8–12
  for fix verification.

## 5. Where things run

- Edit and build on the development machine (macOS) (toolchain pinned in `rust-toolchain.toml`).
- Linux FUSE: a container or Linux VM. macOS FSKit: a macOS 27 machine or
  VM on Apple silicon (ADR 0004, L8). Windows (the SMB layer; Cloud Files
  later): Windows 10 and Windows 11 virtual machines. The release venues
  are named in ADR 0004's supported-platform matrix (L36 to L39).
- Every task that touches a platform layer names its venue; a claim about
  mount behaviour is only proven on that venue.

## 6. GitHub: issues, milestones, labels

GitHub is part of the process, not just hosting. Discussions are off; issues
hold everything.

- **Issues are the project's memory.** A task is an issue whose body is its
  four-part brief (§3, step 3). An issue's title is a claim. Its state is its
  last comment plus the commits that reference it.
  - Before planning on an issue, read it in full and say what is done, what
    remains and what blocks it.
  - When work lands, write the state back: landed, remaining or refuted, with
    PR numbers and commits.
  - Close what is done, and say where each part went. Retitling is not
    closing.
  - Relationships (blocked by, parent and sub-issues) use GitHub's fields,
    not prose, and only when the two issues really concern the same thing.
- **Picking a task.** Take the highest-priority issue that is **ready**:
  open, triaged (it has its `class:*` and `area:*` labels and no
  `needs-triage`), not
  `owner-decision`, not `option` unless the owner asks, and with every
  issue blocking it closed.
  - Claim it with a comment before starting, naming the branch.
  - Whoever stops before finishing says so in a comment: the branch, the last
    pushed commit and what remains. The task is then free to pick up.
- **At most two active pull requests per author** at a time. The
  maintainer's own pull requests never use up a contributor's.
  - Dependabot's pull requests and parked ones do not count.
  - The limit stops new claims only. Finishing, repairing or landing a pull
    request within the limit is never blocked by it.
  - A pull request opened over the limit stays a draft until one of its
    author's other pull requests is merged, parked or closed. Its author can
    always close one of their own to free a slot.
- **Parking** is for a blocker outside the author's reach, such as an owner
  decision or another pull request, or for an author who has not answered
  (below). The pull request gets the `parked`
  label and a comment with two lines:
  - `Blocker:` what blocks it, and who acts;
  - `Recheck:` when it is looked at again.

  A `parked` label without that comment still counts towards the limit. A
  failing check the author can fix is not a blocker.
- **No silent abandonment.** When a pull request has had no push or comment
  by its author for 14 days, the author is asked in a comment and has seven
  days to answer. Without an answer, it is parked as above, or closed with a
  comment that says what happens to its task.
- **Settling an issue.** A task is done when its change is on `main` and its
  acceptance evidence holds there.
  - A pull request that completes an issue says `Closes #N`; one that does
    only part of it says `Part of #N`.
  - Before a merge, the pull request's title, description and commit messages
    are searched for closing keywords. GitHub ignores negation: "does not
    close #N" closes #N.
  - After a merge, every issue the pull request mentions is checked. One left
    open in error is closed by hand, naming the merge commit; one closed
    early is reopened with a comment.
  - Closing a pull request unmerged never closes its task: its comment says
    what happens to the task instead.
  - An issue settled without a change to the repository (a question
    answered, a refuted report, a decision recorded in the issue) is closed
    by hand, with the record and why. Changes to the repository always land
    by pull request.
- **After a merge, clean up.** GitHub deletes the merged branch (a
  repository setting). Local branches and worktrees are removed once
  `git fetch` shows the work is on `main`; nothing holding work that is not
  on `main` is deleted blind.
- **The queue is reviewed at least every 30 days.** Each `option` and `p3`
  issue is scheduled, closed with a reason, or kept with its trigger (the
  event or date that would make it worth doing) still valid.
- **Milestones are phases.** A phase's plan is the issues in its milestone
  (the `phase-lifecycle` skill). The phases follow ADR 0004, part 3, O50,
  which gives their order and reasons, and promises no dates:
  - M0, public foundation;
  - M1, architecture;
  - M2a, the core and Linux, read-only and unauthenticated. The small
    Windows SMB identity spike (O50, point 2) runs beside it, and its
    issues sit in this milestone;
  - M2b, rxkad, gated on a specification route under ADR 0001;
  - M3, Linux writes and locks;
  - M4, macOS, starting with the macOS gate (L13);
  - M5, Windows, starting with the rest of the Windows gate (L22, L41);
  - M6, rxgk and other security work;
  - M7, proposed later work (O51). Each item is scheduled only once its
    issue is approved.

  Where ADR 0004 says "M2", it means M2a and M2b together; each test is
  placed in one of them by its issue.
- **Labels.** Every issue has exactly one `type:*` label and, once
  triaged, at least one `area:*` label, normally one. Every PR and every
  triaged task issue has exactly one `class:*` label, from triage on. An
  issue has at most one priority label.
  `needs-triage` and a `class:*` label are never on the same issue.
  - **Class:** `class:E`, `class:I`, `class:P`, `class:S`, `class:R` (§1).
  - **Type:**
    - `type:bug`: something does not work as specified;
    - `type:design`: a design question, ADR or specification;
    - `type:task`: a unit of work, written as a brief;
    - `type:research`: research to answer a question.
  - **Area:**
    - `area:rx`: the Rx transport and its security classes;
    - `area:xdr`: XDR encoding and the interface definitions;
    - `area:cache`: the client core: the cache manager and its state;
    - `area:auth`: tokens, credentials and authentication;
    - `area:cli`: the `afs` commands and their output;
    - `area:fuse`: the Linux FUSE layer;
    - `area:fskit`: the macOS FSKit layer;
    - `area:smb`: the Windows SMB layer;
    - `area:cloudfiles`: the Windows Cloud Files layer;
    - `area:os`: the shared operating-system bindings; each front end keeps
      its own platform label;
    - `area:ops`: events, the shared control IPC and configuration;
    - `area:test`: the simulator, test support, fuzzing, the test cell and
      the harnesses;
    - `area:docs`: documentation;
    - `area:process`: how the project works: workflow, automation, CI and
      repository settings.
  - **Status:**
    - `needs-triage`: new, its class not yet confirmed. The issue forms set
      it; the maintainer removes it when applying the `class:*` label.
    - `owner-decision`: waits on the owner;
    - `option`: an idea not yet decided. Its body names its trigger: the
      event or date that makes it worth deciding;
    - `debt`: known shortcuts to repay;
    - `parked`: paused, with a two-line comment saying why and what unblocks
      it.
  - **Priority:** `p1`, take next; `p2`, soon; `p3`, when convenient.
  - **Dependencies:** `dependencies`, set on Dependabot's pull requests by
    `.github/dependabot.yml`. They still get a `class:*` label at triage.
  - **For contributors:** `good first issue`, `help wanted`.
- **Embargoed security work is the one exception** to "issues hold
  everything". It stays in the private advisory or an unpushed branch (§2,
  "Embargoed security fixes"), and gets its public issue and its review
  record at disclosure.
- **Templates:** bug report, design question and task
  (`.github/ISSUE_TEMPLATE/`). The chooser hides blank issues from public
  contributors; maintainers can still open one, and a task still follows the
  four-part brief. Security reports go to `SECURITY.md`, never to an issue.
- **Triage:** the maintainer confirms each new issue's or PR's class,
  applies exactly one `class:*` label and at least one `area:*` label, and
  removes `needs-triage`, before anyone plans from it.
- **Dependabot** opens update PRs weekly for Cargo and GitHub Actions. They
  are proposals: the maintainer classifies each one, checks it against the
  `add-dependency` checklist, and sponsors it with their sign-off and
  approval. If its class needs a review before code, the update is redone
  from that reviewed contract in a new PR.
- **`CODEOWNERS`** requests the maintainer's review when a pull request is
  ready for review (GitHub waits while a PR is a draft). A spec-only draft
  that wants early review mentions the maintainer.
