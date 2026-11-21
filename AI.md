<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# How IronAFS uses AI

IronAFS is written by one person working with AI models from five families:
Anthropic (Claude), OpenAI (GPT), Google (Gemini), Zhipu (GLM) and DeepSeek.
Nothing about that is hidden. This file, the
[workflow](docs/workflow.md), the [decisions](docs/adr/README.md) with their
review records, and the instructions the agents follow are all in this
repository. The rules here come from
[ADR 0002](docs/adr/0002-ai-use-and-attribution.md).

**Why five families.** Models from different families miss different
things. So no *required* review is done by the family that wrote the change,
and important changes are reviewed by two families that did not write them.
A same-family check may run as an extra; it never counts as a required
review. The review records show the effect: in the reviews of this
repository's first commit, the GPT, GLM, Gemini and Claude (Mythos) seats
each caught defects the others had missed.

The instructions the agents follow:

- `AGENTS.md`, the rules for every agent;
- `CLAUDE.md` and `.claude/`, for Claude Code.

The maintainer's own controller session also follows private instructions:
how they personally run the lanes and land changes. The rules those
instructions carry out are all public, in ADR 0002, `docs/workflow.md` and
this file.

The review lanes run through the maintainer's local tooling, described in
`docs/workflow.md` and the `review-lanes` skill. That tooling is not published,
because it is the maintainer's personal setup.

## Who does what

One human owns the project and approves every commit that lands on `main`. The
models come from five families and have fixed roles. No family does a
required review of its own family's work.

### The families and models

| Family | Models used here (id as recorded in trailers) | Reached through | Agent ids |
|---|---|---|---|
| Anthropic (Claude) | Claude Opus 5.5 (`claude-opus-5-5`), default; Claude Mythos 5.2 (`claude-mythos-5-2`, what Claude Code's `fable` alias serves) for the strongest seats; Claude Haiku 5.5 (`claude-haiku-5-5`) and Sonnet for light work, including Claude Code's own background steps | Claude Code CLI, Claude subscription | `claude-code` |
| OpenAI (GPT) | GPT-6.2 Sol (`gpt-6.2-sol`), default; GPT-6.2 Astra (`gpt-6.2-astra`) for the strongest seats | codex CLI, ChatGPT subscription | `codex` |
| Google (Gemini) | Gemini 4 Carbon (`gemini-4-carbon-medium`; `gemini-4-carbon-high` for class S and R and the strongest seats); Gemini Flash and Argon for light work only | Antigravity CLI (`agy`) | `gemini`, `agy` |
| Zhipu / Z.ai (GLM) | GLM 5.3 (`glm-5.3`); GLM 5.3 Flash (`glm-5.3-flash`) for small helper steps | Claude Code CLI on Z.ai's endpoint, GLM Coding Plan | `glm` |
| DeepSeek | DeepSeek V4.1 Flash (`deepseek-flash`) | Claude Code CLI on DeepSeek's endpoint, DeepSeek API | `deepseek` |

Which model and effort each seat uses is set by the routing table in
[docs/workflow.md](docs/workflow.md) §2, "Model routing": light models only
for helpers and research gathering, standard tiers for required reviews,
the strongest tiers for the named seats.

Model versions change. A trailer always records the model that actually
served the request, read from the lane's own log, not the one asked for. When
the defaults above change, this table changes in the same commit.

### The roles

| Role | Family | Agent id |
|---|---|---|
| Controller: plans, briefs, routes, verifies findings, drafts commits | Anthropic | `claude-code` |
| Implementer | Anthropic | `claude-code` (subagents the controller dispatches) |
| Implementer for extra capacity | OpenAI | `codex` |
| Implementer when codex is out of quota | Google | `agy` |
| Design, research, rulings (read-only) | OpenAI | `codex` |
| First reviewer of implemented code | Zhipu | `glm` |
| Second review family where the class needs two | OpenAI, Google, DeepSeek or Anthropic, whichever wrote none of the change and is not already the first reviewer | `codex` (read-only), `gemini`, `deepseek`, `claude-code` |
| Claude review seats (never the only reviewer of Claude's work) | Anthropic | `claude-code` |

The accounts are flat-rate subscriptions or usage-billed, and not all of them
report cost per run, so no cost is published per change.

## Change classes: how careful each change must be

Every change gets a class, chosen by asking "if this change has a plausible
mistake, what goes wrong?". The higher the class, the more checks and the
more independent reviewers it needs. When in doubt, the higher class wins.

| Class | Name | A plausible mistake… | AFS examples |
|---|---|---|---|
| **E** | Editorial | changes nothing the project claims or does | a typo, a broken link |
| **I** | Internal | changes internals only; no input, output or stored byte differs | a refactor, new tests, a test harness |
| **P** | Public | changes what a user, an admin or a server sees | a new `afs` subcommand, a config key, an error message, `fs` output that scripts parse |
| **S** | Safety | breaks security, data, consistency or concurrency | parsing fileserver replies, callbacks and cache invalidation, token storage, locking |
| **R** | Irreversible | cannot be taken back | an on-disk format we promise to read forever, a release, going public, a licence change |

### Who writes and who must review

No model family reviews its own family's work. A required review always
comes from a family that did not write the change: any of its text or code,
or the design it was built from. Having reviewed an earlier draft, or
gathered research it draws on without its text entering the change, does
not make a family an author (`docs/workflow.md` §2).

| Class | Usually written by | Required reviews before merge | Other checks |
|---|---|---|---|
| **E** | Claude or the owner | none; the owner re-reads it | links resolve |
| **I** | Claude | one other family: GLM or GPT for Claude's or the owner's own work; GLM for codex's or agy's | tests and lints pass |
| **P** | Claude | as I, and the written contract (what users will see) is reviewed before any code | contract first |
| **S** | Claude, from a design | the design is reviewed before code; the code by the first reviewer for its author (GLM, or GPT for Claude's work), plus a second family (Gemini, DeepSeek, GPT or Claude, whichever did not write it) when tokens, the wire or cache coherency are involved; a security review by GPT, Claude or GLM, whichever wrote neither the design nor the code | a test that reproduces the failure |
| **R** | as S | as S | the owner's decision |

If too few independent families are available, a change waits; the bar is
never lowered. Every class ends the same way: the owner sees the change and
its exact commit message, and nothing lands without their approval.

### What each model can do here

| Model (family) | Controller | Writes code | Design and research | Reviews others' code | Security review |
|---|---|---|---|---|---|
| Claude Opus 5.5 (Anthropic) | yes | yes (main implementer, as subagents) | yes | yes (default Claude seat) | no: Mythos takes Claude's security seats |
| Claude Mythos 5.2 (Anthropic) | | promoted implementation only | the strongest seats: judgements, class R designs, architecture ADRs, load-bearing research synthesis | judgements | yes, every Claude security review |
| GPT-6.2 Sol (OpenAI, via codex) | | yes (extra capacity, separate tasks) | yes, read-only | yes, read-only, never codex's own work | no: Astra takes GPT's security seats |
| GPT-6.2 Astra (OpenAI, via codex) | | after two recorded failures on the same defect | the strongest seats: judgements, class R designs, architecture ADRs, load-bearing research synthesis | judgements | yes, the default GPT security seat |
| Gemini 4 Carbon (Google, via agy) | | when codex is out of quota | | yes, second family | no: it declined security-framed briefs in our runs |
| GLM 5.3 (Zhipu) | | | second view | yes, first reviewer | yes, when it wrote neither the design nor the code |
| DeepSeek V4.1 Flash (DeepSeek) | | | research | yes, second family, off-peak (paid) | |

When two seats disagree and the evidence does not settle it, a third
family rules on the evidence, using its strongest model (the judgement seat,
[docs/workflow.md](docs/workflow.md) §2).

The authoritative rules are in [docs/workflow.md](docs/workflow.md) §1 and
§2.

## Reading a commit on `main`

Each commit on `main` is one validated change, squash-merged from a pull
request after the owner's approval. The one exception is the first commit of
the public history, which had no `main` to merge into (ADR 0002 gives its
checklist). Every commit on `main` ends with a trailer block:

```
Class: S
Assisted-by: claude-code:claude-opus-5-5
Assisted-by: codex:gpt-6.2-sol
AI-Reviewed-by: glm:glm-5.3
AI-Reviewed-by: gemini:<served model id>
Signed-off-by: Adrian Ana <adrian@adrianana.com>
```

- `Class:` is the change's class.
- `Assisted-by:` names a model whose writing remains in the change: code,
  tests, docs or the message. The id is the model that actually served the
  request. `Assisted-by: none` means no model wrote any of it.
- `AI-Reviewed-by:` names a review whose final verdict covers the change as
  it landed: what it reviewed, plus verified fixes or recorded fixes of its
  own minor findings. Earlier rounds are in the review record.
- `Signed-off-by:` is the human's [Developer Certificate of
  Origin](https://developercertificate.org/) sign-off. The owner sees every
  merge's diff and exact message before it lands. Only after approving it
  is their sign-off added, so the certification is always the human's.
  Outside contributors write their own.
- Models never get `Co-authored-by:`. That trailer is for people.

To list the models behind each commit:

```sh
git log --format='%h %s%n  %(trailers:key=Assisted-by,valueonly,separator=%x2C )'
```

Working commits on a branch carry `Assisted-by:` too. Issues, pull request
descriptions, comments and reviews written by a model end with a line naming
it, such as `Written by codex:gpt-6.2-sol.`, adding for whom when it is
written for someone else's account
(`Written by claude-code:claude-opus-5-5 for the maintainer.`)
([ADR 0003](docs/adr/0003-branch-work-and-ai-disclosure.md)).

The pull request behind each commit keeps the full story: the working commits,
each review verdict and the fix rounds. Review records are also kept in
[docs/reviews/](docs/reviews/). Raw agent transcripts are not published,
because they can contain secrets and personal data.

## What the owner checks before approving a commit

- The change does what its description says, and its class is right.
- The class's checks passed, and every confirmed finding is fixed or refuted
  with evidence.
- The trailers are true: the right models, served ids, no stale reviews.
- Provenance: nothing from OpenAFS, AuriStor or the kernel's AFS/Rx code
  goes in (the don't-copy rule in [ADR 0001](docs/adr/0001-licence-and-dont-copy.md)),
  and permissive third-party material keeps its notices and is listed in
  [docs/provenance.md](docs/provenance.md), whoever or whatever copied it.

## Copyright

The project's licences cover what is copyrightable in it. The US Copyright
Office's position is that purely AI-generated material is not protected, while
a human's creative selection, arrangement and modification can be. Using AI
does not change who is responsible for the code: the human who signs off is.

## Contributing with AI

Welcome, under the same rules: see [CONTRIBUTING.md](CONTRIBUTING.md).
