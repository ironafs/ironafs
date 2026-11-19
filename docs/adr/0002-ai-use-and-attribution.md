<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0002: AI use, commit attribution and working in the open

Status: accepted (2026-11-18), by the owner, after the review rounds recorded
below. Partly superseded by [ADR 0003](0003-branch-work-and-ai-disclosure.md)
(branch work, branch-commit trailers, and disclosure beyond commits).

## Context

- IronAFS is built with AI models in defined roles (`docs/workflow.md` §2):
  Claude Code controls, codex implements, other families review. The owner
  wants this visible: the repository, ADRs, workflow, change classes and
  model usage are meant to become public.
- Each change on `main` should credit both sides: the human who answers for
  it and the models that did the work. Git has one author field, so the
  models need another place.
- Practice has converged on a few rules:
  - **Linux kernel** (`Documentation/process/coding-assistants.rst`, v7.2):
    an `Assisted-by: AGENT_NAME:MODEL_VERSION [TOOL1] [TOOL2]` trailer.
    "AI agents MUST NOT add Signed-off-by tags": only a human can certify
    the Developer Certificate of Origin (DCO). The human reviews the code
    and takes full responsibility for it.
  - **Fedora** (Council policy, 2025-10-22): AI assistance is allowed and
    disclosed with an `Assisted-by:` trailer. The contributor is always the
    author and is accountable for all of it.
  - **Apache Software Foundation** (Generative Tooling Guidance): name the tool
    in the commit message (`Generated-by:` in the ASF's draft conventions).
  - **GitHub** credits the account a `Co-authored-by: name <email>` address
    maps to. Displayed credit is neither proof of authorship nor a statement
    of responsibility.
- **Copyright.** The US Copyright Office (Copyright and AI, Part 2,
  2025-01-29) concludes that purely AI-generated output is not protected. A
  human's sufficiently creative selection, arrangement or modification of it
  can be. Using AI does not void protection of the human parts. IronAFS can
  license only the rights it holds. AI output can also reproduce or adapt
  third-party code, including the OpenAFS and kernel sources ADR 0001 keeps
  out of IronAFS.
- **Line-level attribution** exists but is early. Agent Trace (Cursor, v0.1.0
  RFC) and git-ai (git notes under `refs/notes/ai`) record which lines came
  from which model. Both need a hook in every agent, and git notes are not
  pushed by default.

## Decision

### Shape of history: one commit per validated change

- Each change is worked on its own branch, with as many commits as it needs.
  Branch commits are working state, not the record. They carry no trailers
  (superseded by ADR 0003: they carry `Assisted-by:`) and no sign-off; the
  full trailer block goes only on commits that land on `main`. Who approves
  a commit before it is made (for the maintainer's session, superseded by ADR
  0003: its branch commits need no approval; all branch commits carry
  `Assisted-by:`):
  - the owner, for the maintainer's own branch commits, every squash merge,
    and the root commit;
  - a contributor, for their own branch commits.
- **The exception for writing a sign-off** (an agent writing a human's line
  after that human approves the exact message) is the owner's, for squash
  merges and the root commit. A contributor writes their own `Signed-off-by:`
  in the PR description.
- A change reaches `main` only through a pull request, and only when its
  class's checks (`docs/workflow.md` §1) are met. It lands as **one squash
  commit**. The controller prepares the branch and the squash message, presents
  them, and merges only after the owner approves that message.
- Editorial (E) fixes may be batched: one branch is one batch, and the body
  lists each fix.
- **The root commit** of the public history is the one exception to the PR
  rule, because there is no `main` to open a pull request against. It is
  class R, and its checklist is:
  - a non-author review of the candidate tree;
  - the owner's acceptance of the ADRs it holds;
  - provenance and privacy checks;
  - the scaffold's own checks: build, clippy, fmt, `reuse lint`,
    `cargo-deny`;
  - approval of the exact tree id and message.

  The code-specific gates (a design review before code, a reproduction test)
  do not apply: it carries a placeholder binary and documents. It has the full
  trailer block.
- Repository settings: squash merge only; the squash commit title is the PR
  title and its message is the **PR description**; branches are deleted after
  merge. A ruleset on `main` allows changes only through pull requests and
  refuses force-pushes and deletion.

### The commit message

The PR description ends with **one trailer block**, the last paragraph of the
message, which git parses as the trailers. It becomes the squash commit's
trailers unchanged.

```
cache: drop stale entries on callback break

<what changed and why; refuted findings with the evidence>

Class: S
Assisted-by: claude-code:claude-opus-5-5
Assisted-by: codex:gpt-6.2-sol
AI-Reviewed-by: glm:glm-5.3
AI-Reviewed-by: gemini:<served model id>
Signed-off-by: Adrian Ana <adrian@adrianana.com>
```

- **`Class:`** exactly one of E, I, P, S, R.
- **`Assisted-by: <agent>:<model-id>`**, the kernel's form without its tool
  list. One line per model whose writing *remains* in the landed change:
  code, tests, docs, or this message. A model whose work was discarded is
  not credited here; it appears in the PR's review record. The model id is
  the one actually served, from the lane's did-it-run line. A change with no
  model writing carries `Assisted-by: none`. The trailer is always present, so
  a missing trailer is an error, never "no AI".
- **`AI-Reviewed-by: <agent>:<model-id>`** for each review seat whose final
  verdict covers the landed tree. The seat reviewed the substantive files at a
  pinned revision, and every change after it was a verified fix (since ADR
  0003: or a recorded fix of that seat's own minor findings). A seat that
  only reviewed an earlier revision is not credited here. Adding the review
  record itself, or other review-only bookkeeping, does not need a new
  review. `Reviewed-by:` is kept for humans.
- **`Signed-off-by:`** (DCO 1.1) is a human's certification.
  - **The owner's:** every commit the maintainer's session makes, and every
    commit that lands on `main`, is presented to the owner before it is made
    (since ADR 0003: every commit that lands on `main`, and the root commit)
    (contributors approve their own branch commits; see "Shape of history").
    The presentation shows the diff summary, the full final message including
    the owner's `Signed-off-by:` line, and, for a merge, the PR's head commit.
    Nothing is committed or merged until the owner approves exactly that.
    Approving it certifies the DCO for the change, not only that they
    reviewed it. The controller then commits, or merges with
    `gh pr merge --squash --match-head-commit <approved sha>` and the approved
    subject and body. Any change to the tree or the message needs a new
    approval.
  - **An outside contributor's:** written by them in the PR description and
    kept in the block. Before approval, the controller checks that the
    approved merge body carries their line unchanged and that it matches the
    PR author. After the merge, it checks the landed commit for the same
    line. Each signer certifies under whichever DCO clause
    applies. The owner signs under (c) only if they pass the contribution on
    unmodified; if they change it, under (a) or (b) for their changes.
- **No `Co-authored-by:` for models.** It is kept for human co-authors.

**Agent ids.** The owner's lanes use a closed list, one id per
`docs/workflow.md` §2 lane:

| id | Lane | Family |
|---|---|---|
| `claude-code` | Claude Code, controller and Claude subagent seats | Anthropic |
| `codex` | codex CLI, implementer and GPT read-only seat | OpenAI |
| `agy` | agy CLI, implementer when codex is out | Google |
| `gemini` | Gemini review via agy | Google |
| `glm` | GLM via headless Claude Code on Z.ai | Zhipu |
| `deepseek` | DeepSeek via headless Claude Code | DeepSeek |

Outside contributors and bots name their own tool, as `<tool>:<model or
version>`; CI checks only the format. The trailers declare who took part.
Reviewer independence (which families wrote and which reviewed) is
established from the review record in `docs/reviews/`, not from the trailers
alone.

**Commit identity.** The squash is made by GitHub's merge after approval,
through `gh pr merge` with the guards below. The committer is GitHub
(`web-flow`, signed by GitHub's key). The author is whoever GitHub
attributes, usually the account that opened the PR; for the owner's own PRs
that is the owner. Neither field is the human statement: the
`Signed-off-by:` lines are. The landed identity is read back and recorded.

**What an approval binds.** The owner approves all of these together:

- the target branch;
- the base commit the checks ran on;
- the PR's head commit;
- the exact subject and body.

The merge runs with `--match-head-commit <head>` and the approved subject and
body, and only while `main` is still at the approved base. Merges run one at a
time while the checks are manual. If the base or the head moves, the change is
presented again. After the merge, the squash commit GitHub created is read
back and compared with what was approved.

### Enforcement

The checks below are **planned**. Until they exist, the controller runs them
by hand before presenting a commit, and the owner checks them before
approving.

- **CI on every pull request**, rerun when the description is edited, checks
  the PR description:
  - exactly one `Class:`, and at least one `Assisted-by:`;
  - well-formed ids;
  - `AI-Reviewed-by:` seats that fit the class;
  - for an outside contributor, their own `Signed-off-by:`.
  The owner's sign-off is checked at merge.
- **CI on every push to `main`** checks the landed commit: the same rules,
  plus a `Signed-off-by:` from a maintainer address. A failure there is fixed
  by a revert or a follow-up commit, never by rewriting `main`.
- **Honest limit:** CI can check that a sign-off is present, not that the
  owner approved it. "No commit without the owner's approval of its message"
  (since ADR 0003: no merge to `main`, and no root commit) is a rule the
  controller follows, backed by the maintainer's instructions to
  their session and the `land-change` skill, not a technical control.

### Review records

Every change with a review seat gets a short record in
`docs/reviews/<YYYY-MM-DD>-<slug>.md`, added in the same squash: each seat's
lane, served model, effort and verdict; each finding with how it was triaged
(fixed, refuted with evidence, deferred). It is written from lane output and
redacted: no paths outside the repository, hostnames, cell names, keys or
other projects. ADRs link to their record.

### Usage record

`docs/ai-usage.md` is generated by `cargo xtask ai-usage` from a named revision
of `main`. It reads that revision's commit trailers and its review records:
the files in `docs/reviews/` and the Review sections of ADRs. A clone can
reproduce it. Each report names the revision
it covers. It reports **declared** activity:

- changes per class;
- changes per agent and model, writing (`Assisted-by:`) and reviewing
  (`AI-Reviewed-by:`) separately;
- changes declared `Assisted-by: none`;
- review rounds, and findings fixed, refuted and deferred.

The committed report is a snapshot of a named earlier revision, so
committing it never makes it stale. CI checks that it matches what the
script produces for that revision. It counts work, not money. The script and
the report are planned; they land with the CI checks.

### What is published, and what is not

- **Public**: ADRs and their review records, `docs/workflow.md` (classes and
  lane roles), research notes, agent instructions (`AGENTS.md` for every
  agent, `CLAUDE.md`, `.claude/skills/`, and `.claude/settings.json` as a
  statement of what agents may not run), and an `AI.md` at the root. `AI.md`
  explains how the project uses AI, the agent ids, how to read the trailers,
  and what the owner checks before signing off. The maintainer's session also
  follows private instructions on how to carry these rules out; the rules
  themselves are all public. `README.md` and `CONTRIBUTING.md` link to it.
- **Private**: the owner's personal notes, other work and machine details. These go in
  `CLAUDE.local.md`, which `.gitignore` excludes, or in the owner's notes.
- **Not published**: raw agent transcripts. They can hold secrets, personal
  data, paths and other projects' content. The redacted review records stand
  in for them.

### Provenance of AI output

AI output gets the same checks as human code under ADR 0001:

- No implementer or review brief ever includes OpenAFS, AuriStor or kernel
  AFS/Rx source, or names their files, functions or internal identifiers
  (ADR 0001, "AI models"). A session that read such source to write a
  behaviour spec does not implement that spec.
- When a model's output looks copied from anywhere, the change says so.
  Permissive third-party material keeps its licence, notices and SPDX
  identifier, and gets a `docs/provenance.md` row.
- A change whose origin or licence is unresolved does not merge.

`CONTRIBUTING.md` asks the same of outside contributors.

### Outside contributors

AI-assisted contributions are welcome under the same rules: the trailer block,
their own `Signed-off-by:`, and full responsibility for the change. No ban, no
quota.

### Other cases

- **A revert** takes the reverted change's class and a full trailer block.
- **Release tags** are signed by the owner (class R).
- **Bot PRs** (dependency updates) carry `Assisted-by: <bot>:<version>` and the
  owner's sign-off, which certifies the change under DCO clause (b) or (c).
  Git may name the bot as author; the owner's sign-off is the statement that
  counts.

### Going public: conditions (class R, owner)

The repository becomes public only when all of these hold:

1. `.gitignore` excludes `CLAUDE.local.md`. Private notes have moved out of
   tracked files.
2. No private material is reachable in history. The public history starts
   from a fresh root (owner decision 1), pushed to a **new, empty**
   `ironafs/ironafs`, because GitHub can keep force-pushed commits reachable
   by id.
3. All branches, tags, PRs, issues, comments and review records are swept for
   paths, hostnames, cell names, secrets and private notes.
4. The public name and email in commit metadata and SPDX headers are
   confirmed.

## Alternatives considered

- **`Co-authored-by:` per model** (Claude Code's default trailer). Rejected: it
  presents a model as a co-author, and GitHub credit implies a person behind
  an account. Each tool also emits its own form.
- **`Generated-by:`** (ASF draft). Rejected: it reads as "the model wrote all
  of it". `Assisted-by:` fits mixed work and matches the kernel and Fedora.
- **Every branch commit on `main`** (merge or rebase merge). Rejected: the owner
  wants `main` short and made only of validated changes.
- **Squash locally** (`git merge --squash`, `git commit -s`, push). Possible,
  and it makes the owner the committer, but it bypasses the PR merge and needs
  direct pushes to `main`. Kept as a fallback if GitHub's merge path proves
  unreliable.
- **The owner types every `Signed-off-by:` by hand** (the kernel's rule:
  agents must not add the tag). Not taken: the owner chose instead to approve
  each commit's exact message, after which the controller adds the line. The
  certification is still the human's, given by explicit approval, and the
  ADR says so publicly. Outside contributors add their own sign-off.
- **Line-level attribution (Agent Trace, git-ai)**. Deferred: a v0.1 RFC, a
  hook in every lane, and notes easily lost on push.
- **Publishing transcripts.** Rejected for the leak risk above.

## Consequences

- The agent instructions carry these rules: the general ones in `AGENTS.md`
  for every agent, and the owner-specific ones (approval, the owner's
  sign-off) in the maintainer's private instructions to their session. The rule that source carries
  no model attribution stays.
- `docs/workflow.md` §3 step 8 points here for how a change lands.
- ADR 0001's Contributions section takes DCO sign-off from the start, under
  this ADR.
- New files: `AI.md`, `CONTRIBUTING.md`, `cargo xtask ai-usage`, `docs/reviews/`,
  the CI checks, and a PR template with the trailer block.
- Repository settings change: squash only, message from the PR description,
  delete branches after merge.

## Owner decisions (2026-11-18)

1. The public repository starts from a fresh root. The first private history
   is kept by the owner as a local backup and never published.
2. The usage record counts work, not money. Most lanes run on flat-rate
   subscriptions, one is usage-billed, and not all of them report per-run
   cost. A per-change figure
   would be invented, and figures for some lanes but not others would mislead.
   `AI.md` names each lane's service and plan type instead.
3. Every commit the maintainer's session makes, and every commit that lands on
   `main`, is presented to the owner (diff summary, full message) and made
   only after approval (since ADR 0003: every commit that lands on `main`, and
   the root commit; the session's branch commits need no approval); the
   approval is what lets the controller add the owner's `Signed-off-by:`. Contributors approve their own branch commits.

## Sources

- Linux kernel, AI Coding Assistants: https://docs.kernel.org/7.2/process/coding-assistants.html
- Fedora AI-Assisted Contributions Policy: https://docs.fedoraproject.org/en-US/council/policy/ai-contribution-policy/
- ASF Generative Tooling Guidance: https://www.apache.org/legal/generative-tooling.html
- ASF RAI, commit messages for AI-assisted code (draft): https://rai.apache.org/commit-messages.html
- GitHub, commits with multiple authors: https://docs.github.com/en/pull-requests/committing-changes-to-your-project/creating-and-editing-commits/creating-a-commit-with-multiple-authors
- US Copyright Office, Copyright and AI Part 2: https://www.copyright.gov/ai/
- Agent Trace: https://github.com/cursor/agent-trace
- git-ai: https://usegitai.com/docs/get-started/how-git-ai-works

## Review

Round 1, 2026-11-18, on the first draft. Both seats read-only.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex (GPT design seat) | gpt-6.2-sol | high | FAIL: 1 blocker, 6 major, 2 minor |
| glm (second family) | glm-5.3 | high | 2 blockers, 7 major, 8 minor (research mode, no verdict field) |

Findings and triage (both seats unless marked):

- **Fixed: sign-off and squash mechanics.** The draft's `git rebase --signoff`
  signed branch commits and rewrote pinned shas. GitHub's default squash
  message stacks branch messages, so git would parse only the last trailer
  block. Checked: the repository's setting is `COMMIT_MESSAGES`. Now the PR
  description is the message and the owner signs at merge.
- **Fixed: over-credit.** Aggregating every model that touched the branch
  credited discarded work and stale reviews. Now credit is for retained
  writing and reviews that cover the landed tree.
- **Fixed: usage record.** A missing trailer read as "no AI". Review counts
  were not reproducible from a clone. GLM: the "no model" share is
  structurally near zero. Now `Assisted-by: none` is explicit, review records
  are in git, the figures are declared activity, and old commits are legacy.
- **Fixed: unenforceable checks.** A local hook cannot see GitHub's squash, or
  which session typed a line. Now the checks are specified for CI on PRs and
  on `main`, run by hand until CI exists, and the limit is stated.
- **Fixed: contributor DCO.** The owner's sign-off cannot stand in for a
  contributor's.
- **Fixed: going public.** History, PRs and issues were not in the checklist;
  `.gitignore` lacks `CLAUDE.local.md` (checked); GLM: the research note's
  trademark section. Now these are conditions, not options.
- **Fixed (GLM): seat ids.** Seat ids did not cover every §2 lane; the example
  had a placeholder. Now there is a closed id list with families.
- **Fixed (codex): copyright and provenance.** The copyright wording
  overstated protection, and there was no provenance rule for AI output.
- **Fixed: minor points.** GitHub co-author wording; kernel format stated as
  "without the tool list" (GLM); E batching (GLM); reverts, tags and bot PRs
  (GLM); merge authority per class (codex); a reference to an `AGENTS.md`
  that does not exist (GLM), dropped.
- **Partly rejected (GLM): `.claude/settings.json` reveals the owner's
  machine.** It lists generic paths (`~/.zshenv`, `~/.codex/auth.json`) as
  *denied* reads. It is kept public as a statement of posture.
- **Rejected (GLM): kernel trailers are `Name <email>` shaped.** The kernel's
  `Assisted-by:` form is `AGENT:MODEL`, with no email.
- **Uncertain, both seats:** neither could check web sources offline. The
  controller checked the kernel format against the v7.2 page.

Round 2, 2026-11-18, verification of the round-1 fixes and the owner's
commit-approval rule, against the working tree.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: 5 minor |

- **Fixed: four wording points.** Who appends the sign-off (Enforcement);
  where a contributor's sign-off comes from; commit identity for both merge
  paths; and the PR template, where a blank line now separates the
  instructions from the trailer block.
- **Fixed in part: commit identity.** GLM said a `gh pr merge` would make the
  owner the committer. Merges through GitHub's API are also committed by
  `web-flow`, so the text now covers both paths with the same identity.
- **Already covered: the old private history.** It is replaced by the fresh
  root (owner decision 1), which is a condition of going public.
- **Confirmed by GLM:**
  - every round-1 fix is in place;
  - the two rejections hold;
  - the approval rule is stated the same way in the ADR, `CLAUDE.md` and
    `AI.md`, and its departure from the kernel's rule is disclosed;
  - the tree to be committed carries no private material.

Round 3, 2026-11-18, verification of the latest changes, against the working
tree.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex | gpt-6.2-sol | xhigh | FAIL (shared with ADR 0001's round 4) |

All fixed:

- **The root commit:** an explicit class R checklist; the exception is also
  stated in `CLAUDE.md` and the workflow.
- **What an approval binds:** the target, the base, the head, and the exact
  subject and body. Merges run one at a time, and the landed squash is read
  back from GitHub.
- **The `land-change` skill:**
  - the staged diff and tree id are presented;
  - the root commit is built in an empty index from an explicit file list;
  - checks are split between branch commits and landing.
- **Sign-off:** branch commits carry none, and the wording now matches across
  this ADR, `CLAUDE.md` and the skill.
- **Contributors' sign-offs** are checked in the final merge body and in the
  landed commit.
- **Credited reviews** cover the substantive files at a pinned revision; the
  review record itself needs no new review.
- **Seat rules:** they now cover every possible author, with one security
  route and a stop rule when too few families are available. `AI.md` no
  longer offers GLM for security reviews. (Superseded by the
  first-public-tree review in `docs/reviews/`, which made GLM an eligible
  security seat.)
- **Minor fixes:**
  - GitHub's author attribution is described accurately, bot PRs included;
  - the usage report reads trailers and review records from the named
    revision;
  - the reference to ADR 0001 now points at its Contributions section;
  - the stale lines in the session plan (`docs/next-session.md`, since made
    private) and `CLAUDE.md` are fixed.

Round 4, 2026-11-18: final verification of both ADRs and the whole tree.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | FAIL: 1 blocker, 8 minor (shared across both ADRs) |

It confirmed every fix from the earlier rounds is present. All findings are
fixed:

- **Blocker:** git's index still listed the deleted dual-licence files. They
  are now removed from the index. `land-change` also builds the root commit
  from an empty index and an explicit file list.
- **Minor:**
  - stale status lines and ADR index rows;
  - the class I seat now defers to the seat table;
  - a seat row for the owner's own unaided changes;
  - the billing wording, which now notes that one lane is usage-billed;
  - the research note's IPL wording, aligned with ADR 0001;
  - the spec session produces the text, and the controller places it.

Round 5, 2026-11-18: the split of the agent instructions (`AGENTS.md`,
`CLAUDE.md`, `docs/maintainer/controller.md`), after the owner accepted this
ADR. The controller file was later moved into the maintainer's private
instructions, before publication.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex | gpt-6.2-sol | high | FAIL: 6 major, 3 minor |

All fixed:

- **Who approves what.** The owner approves the maintainer's commits, every
  squash merge and the root commit. A contributor approves their own branch
  commits and writes their own sign-off. The agent-written sign-off is the
  owner's exception only.
- **Contributors in `land-change`:** a contributor PR path, and the root
  commit's own checklist.
- **`review-lanes`:** marked maintainer-only in its description, which is
  what Claude Code reads when it decides to load a skill.
- **The workflow** labels dispatch and merges as the maintainer's, and tells
  contributors to start P, S and R work with an issue or a draft PR.
- **A rule lost in the split** was restored, in the owner's private notes.
- **Minor:**
  - `AI.md` states the root exception and the author-based first reviewer;
  - the PR title is the commit subject;
  - the handoff records the ADRs as accepted.

