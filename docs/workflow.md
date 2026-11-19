<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Workflow

How changes are made, checked and reviewed in IronAFS. Change it by commit,
with the reason in the commit body.

**The idea in one paragraph.** Every change gets a *class* that says how much
damage a plausible mistake in it could do. The class decides everything else:
what must be checked, which models review it, and who may merge it. Routine
changes stay fast; risky ones get more eyes. Claude Code runs the work as the
controller; other model families write and review code so that no model is
the only one checking its own work.

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
| `unsafe` code, FSKit or Cloud Files bindings (no C Rx: ADR 0001) | S | Memory safety rests on hand-checked invariants |
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
| **Controller** | the maintainer's Claude Code session (following the maintainer's private instructions) | Plans, briefs, routes, verifies findings, lands changes after the owner's approval. Writes code only for work only it can run. |
| **Implementer** | codex CLI, `gpt-6.2-sol` | `codex exec` with a written brief; `high` effort for S, `medium` for I/P mechanical work. `gpt-6.2-astra` only after sol fails the same problem twice. |
| **Implementer when codex is out of quota** | agy CLI (Gemini) | Same brief; a session allowlist that denies git. Brief paths must sit inside `--add-dir`. |
| **First reviewer of codex-written code** | GLM 5.3 (`glm-review`) | Headless Claude Code on Z.ai's endpoint, read-only. |
| **Second family when the class needs two** | Gemini Carbon via agy (`gemini-review`), or DeepSeek (`deepseek-review`) | DeepSeek off-peak only (not Mon–Fri 01–04 or 06–10 UTC). |
| **Design, rulings, research, refutations on work codex did not write** | GPT read-only: `codex exec -s read-only -m gpt-6.2-sol - < brief.md`; `-m gpt-6.2-astra -c model_reasoning_effort=xhigh` for the strongest-tier seats (§2, "Strongest tiers") | GLM second when a second view is needed. |
| **Claude seats** (review, reproduction, hard judgement, security) | Claude Code subagents: Opus by default; Mythos 5.2 (`claude-mythos-5-2`, Claude Code's `fable` alias) for hard problems and for every Claude security review | Never a required seat on Claude-written work. |

**A family never reviews its own work.** Codex is OpenAI-family, so a GPT
seat never reviews codex-written code. A same-family review (for example
`/code-review` on Claude-written work) may run as an extra self-check, but it
never counts as a required seat. A finding raised by one family is
refuted, if at all, by a family that neither wrote the change nor raised it.

### Seats by class

**Rule:** every required seat comes from a family that did not write the
change. A family *wrote* a change if it wrote its design or its code. Drafting
the commit message, a brief or the review record does not count. Outside
contributors' declared tools count. The controller writes code only for work
only it can run, so most code comes from codex.

**First reviewer, by author.** The table is an order of preference. Skip any
family that wrote the design or the code. Each row's own order comes first;
if every family it names is excluded, the seat goes to the first eligible
family in this order: GLM, GPT read-only, Gemini, DeepSeek, then a Claude
seat.

| Author | First reviewer |
|---|---|
| codex (OpenAI) | GLM |
| agy (Google) | GLM |
| Claude | GPT read-only, or GLM |
| outside contributor | GLM, or GPT read-only if the contributor declared GLM |
| the owner, with no model writing | GLM, or GPT read-only |

**Second family** (when the class needs two): any of Gemini, DeepSeek, GPT
read-only or a Claude seat whose family did not write the change and is not
already the first reviewer.

**Security review** (class S and R): GPT read-only, a Claude seat (always
Mythos 5.2), or GLM,
whichever family wrote neither the design nor the code. Choose it when the
design is assigned, so that one stays free. Never Gemini, which declined
security-framed briefs in our runs.

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
  disputed after the fix rounds. **A pending or inconclusive judgement blocks
  the merge**, like an open confirmed finding (§3, step 8), and a reproduced
  failure blocks whatever the ruling says (§4).
- **Who sits it.** A family that is independent of the dispute: it wrote
  neither the work nor either position, and has not taken a side in it. The
  controller's family is excluded when the controller argued a position.
  Among the independent families, the first available at its strongest tier
  in this order: GLM, GPT read-only, DeepSeek, Gemini, a Claude seat. Then
  two filters:
  - **Security disputes** (whether a security finding holds, or anything
    that would overturn part of a security review): only the
    security-review families (GPT read-only, a Claude seat, GLM), never
    Gemini or DeepSeek.
  - **Embargoed work:** only the Anthropic and OpenAI lanes, and nothing
    about the dispute leaves them (brief, source lookups, record).

  If no family passes, the owner rules, and the record says so; under an
  embargo that record stays private until disclosure. A ruling never removes
  a seat the class requires.
- **How it rules.** The brief inlines both positions as A and B. The whole
  brief, and anything it points to, carries no family, model or lane name
  and no trailer, transcript or review record of the dispute; the
  controller checks it for them before sending. The seat checks each
  disputed fact at a primary source and labels it CONFIRMED, with the link,
  or UNVERIFIED. A seat without tools (Gemini) gets the primary-source
  passages inlined with their citations; when they cannot be inlined, another
  family takes the seat. The verdict is A, B, a named alternative, or
  INCONCLUSIVE.
- **After it rules.** The controller confirms every factual premise the
  ruling rests on at its source before acting on it. A premise that fails
  or stays UNVERIFIED leaves the dispute open: it goes back to the seat with
  the evidence, or to the owner. The owner also decides when the ruling is
  itself contested, or when the question is the owner's to decide.
- **The record** (§4, "After the run") gives both positions, the seat, the
  served model and effort, the deciding facts with their sources, and the
  controller's check.

### Strongest tiers

Each family's strongest tier is the default, without waiting for the
standard tier to fail, for five kinds of seat: the judgement seat, security
reviews, class R designs, format freezes and architecture ADRs. This table
governs the model for those seats over every other table and command in this
file, `AI.md` and the skills. If a family's strongest tier is unavailable,
that family is skipped for the seat; the seat is never run on a lower tier.

| Family | Strongest tier |
|---|---|
| GPT | `gpt-6.2-astra`, `xhigh` |
| Claude | Mythos 5.2 (`claude-mythos-5-2`) |
| GLM | `glm-5.3`, `max` |
| DeepSeek | the lane's pinned model at `max`, off-peak |
| Gemini | `gemini-4-carbon-high` |

Every other seat stays on the standard tier. A standard seat moves up to the
strongest tier only after it fails the same problem twice (the implementer's
rule, above), so the quota these seats need is there when they run.

`AI.md` explains these rules for readers new to the project. This file is
authoritative.

## 3. How a change runs

1. **Class it** (§1) and name what it touches.
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
   carries the reuse rule (§3a).
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
   commit range pinned by sha, the question, what is already known.
7. **Fix rounds**: fix confirmed correctness and contract findings; review
   only the fix's changes. After two fix rounds on one change, stop patching:
   split it, redesign it, or record the blocker.
8. **Merge** when the class's checks are met, no confirmed correctness or
   contract finding is open, and no judgement is pending or inconclusive: a pull request, squash-merged after the owner's
   approval, with the trailer block and sign-off of ADR 0002 (the root commit
   of the public history is the one exception; ADR 0002 gives its checklist). R also needs the owner's
   decision.

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
  details. A brief that touches credentials says "run no credential command"
  in its first lines.
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
- Linux FUSE: a container or Linux VM. macOS FSKit: a macOS 26/27 machine or
  VM. Windows Cloud Files: a Windows VM.
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
  open, triaged (it has its `class:*` label), not `owner-decision`, not
  `option` unless the owner asks, and with every issue blocking it closed.
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
  (the `phase-lifecycle` skill). The first three:
  - M0, public foundation;
  - M1, architecture;
  - M2, FSKit spike.
- **Labels:**
  - **Class:** `class:E`, `class:I`, `class:P`, `class:S`, `class:R` (§1).
    Every PR and task issue has exactly one.
  - **Type:** `type:bug`, `type:design`, `type:task`, `type:research`.
  - **Area:** `area:rx`, `area:xdr`, `area:cache`, `area:auth`, `area:cli`,
    `area:fuse`, `area:fskit`, `area:cloudfiles`, `area:docs`,
    `area:process`.
  - **Status:**
    - `owner-decision`: waits on the owner;
    - `option`: an idea not yet decided. Its body names its trigger: the
      event or date that makes it worth deciding;
    - `debt`: known shortcuts to repay;
    - `parked`: paused, with a two-line comment saying why and what unblocks
      it.
  - **Priority:** `p1`, `p2`, `p3`.
  - **For contributors:** `good first issue`, `help wanted`.
- **Embargoed security work is the one exception** to "issues hold
  everything". It stays in the private advisory or an unpushed branch (§2,
  "Embargoed security fixes"), and gets its public issue and its review
  record at disclosure.
- **Templates:** bug report, design question and task
  (`.github/ISSUE_TEMPLATE/`). The chooser hides blank issues from public
  contributors; maintainers can still open one, and a task still follows the
  four-part brief. Security reports go to `SECURITY.md`, never to an issue.
- **Triage:** the maintainer confirms each new issue's class and applies
  exactly one `class:*` label before anyone plans from it.
- **Dependabot** opens update PRs weekly for Cargo and GitHub Actions. They
  are proposals: the maintainer classifies each one, checks it against the
  `add-dependency` checklist, and sponsors it with their sign-off and
  approval. If its class needs a review before code, the update is redone
  from that reviewed contract in a new PR.
- **`CODEOWNERS`** requests the maintainer's review when a pull request is
  ready for review (GitHub waits while a PR is a draft). A spec-only draft
  that wants early review mentions the maintainer.
