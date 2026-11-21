<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0003: Branch work and AI disclosure

Status: accepted (2026-11-19), by the owner, at its merge (#10). Raised in
issue #7. Supersedes the parts of
[ADR 0002](0002-ai-use-and-attribution.md) named under "What it supersedes".

## Context

- **ADR 0002 required approval of every commit** the maintainer's session
  made, branch commits included, and kept branch commits free of trailers.
  In practice that meant asking before each working commit, and committing
  less often than review needs: a review round should read a pinned commit,
  not a working tree that may still change.
- **Common practice for coding agents** is different. GitHub's own coding
  agent commits and pushes to its own branch and opens draft pull requests
  freely. A human must review and merge, and the agent cannot approve or
  merge (GitHub, "Risks and mitigations" for the Copilot coding agent).
  GitHub's guidance on governing agents adds that agents stay under the same
  rulesets and code owners as everyone else.
- **The Linux kernel** (Documentation/process/coding-assistants.rst): an
  agent never adds `Signed-off-by:`, only a human can certify the DCO, the
  human reviews everything, and the model is credited with `Assisted-by:`.
- **Disclosure stopped at commits on `main`.** Issues, pull request
  descriptions and comments written by a model were posted under the owner's
  GitHub account with nothing saying so. So were branch commits written by a
  model and signed with the owner's key. A reader would assume the owner
  wrote them.
- **The owner decided** on 2026-11-19 to follow GitHub's norm for branch
  work, and to disclose model writing wherever it is published.

## Decision

1. **Branch work needs no approval.** On a task branch, never on `main`, the
   maintainer's session commits, pushes and opens draft pull requests
   without asking. It marks a pull request ready for review once the
   reviews its class requires are recorded. Contributors work on their own
   branches as before.
2. **Every merge to `main` still needs the owner's approval** of the exact
   squash message, the base and the head (ADR 0002). In addition:
   - the base and the head are checked again immediately before merging, and
     any change to either, or to the message, needs a new approval;
   - the `main` ruleset requires a pull request to be up to date with its
     base before it can merge, so GitHub refuses a stale base (#3);
   - for the maintainer's own pull requests, the merge sets the author email
     to the owner's public address; a contributor's pull request keeps the
     author GitHub attributes (ADR 0002, "Commit identity");
   - after the merge, a comment on the pull request logs the approval: the
     head, the base and the SHA-256 of the approved message.
3. **History is pinned and never rewritten.**
   - The work is committed before each review round, and the review record
     cites the commit range it read.
   - A commit is frozen once it is submitted for review or pushed. Fixes go
     on top as new commits.
   - Before each push, every commit not yet pushed, and its message, is
     scanned for secrets. The push is refused if the scan finds one or cannot
     complete.
   - A secret in a frozen commit: it is revoked first if it was pushed. The
     branch is then replaced by a new one built from before that commit, the
     old branch is deleted, and the reason is recorded. This is the one
     exception to "never rewritten": nothing is built on a leaked secret.
4. **Branch commits carry `Assisted-by:`**, with the same ids as the squash
   (`AI.md`): one line per model whose writing remains in that commit, or
   `Assisted-by: none`. They never carry `Signed-off-by:`, `Class:` or
   `AI-Reviewed-by:`; those belong to the squash. The maintainer's session
   always adds it; contributors are asked to, and the opt-in `commit-msg`
   hook checks it.
5. **Signing.** The maintainer's session signs branch commits with the
   owner's key, as delegated signing. The signature says the commit came from
   the owner's environment; it does not say who wrote it, or that the owner
   reviewed it. `Assisted-by:` says which model wrote it, and the owner's
   sign-off at the merge is the human certification. Squash commits are
   signed by GitHub.
6. **Disclosure beyond commits.** Wherever a model's writing that remains is
   published under a person's account, it carries a visible line:
   - in issues (title and body), pull request descriptions, comments, review
     summaries and inline review comments, release notes, and substantive
     edits of any of these;
   - in the form `Written by <agent>:<model>.`, using the ids of `AI.md`.
     When a model writes for someone else's account, it adds for whom
     (`Written by claude-code:claude-opus-5-5 for the maintainer.`). When
     there is more than one role, it names each:
     `Review by glm:glm-5.3, relayed by claude-code:claude-opus-5-5.`, or
     `Drafted with codex:gpt-6.2-sol, edited by <name>.`;
   - kept accurate when the text is edited.

   Routine actions (applying an existing label, linking, closing with a
   pointer to the evidence) need no line. A model's substantive judgement,
   such as a proposed class or a triage decision, is attributed in its
   comment. In a pull request description, the line sits above the trailer
   block, which stays the final paragraph.
7. **`AI-Reviewed-by:` stays precise.** It names a review seat only when that
   seat's final verdict covers the landed change: the change it reviewed, plus
   verified fixes, or fixes of its own minor findings that are recorded.
   Earlier rounds, same-family checks and superseded verdicts are in the
   review record, not the trailers. A mechanical check cited as evidence is
   recorded with how it was run.
8. **Bot pull requests are proposals.** Dependabot creates and rebases its own
   branches, so points 3 and 4 cannot apply to them. Before the maintainer
   sponsors one, its candidate head is pinned and scanned, and any review or
   approval lapses if the head moves. The squash credits the bot in its body
   ("Proposed by dependabot[bot]"). `Assisted-by:` stays for model writing.

### What it supersedes

In ADR 0002: the rule that the owner approves the maintainer's branch
commits; the rule that branch commits carry no trailers; and, under
`Signed-off-by:`, "every commit the maintainer's session makes … is presented
to the owner before it is made", which now applies to merges and the root
commit only. Under `AI-Reviewed-by:`, it adds that a recorded fix of the
seat's own minor findings keeps the seat's credit (point 7). Everything else
in ADR 0002 stands, including the merge
approval, contributors' own sign-offs, the root-commit procedure and the
embargo rules of `docs/workflow.md` §2.

## Consequences

- Review rounds read pinned commits, and fixes are visible as their own
  commits.
- Pushed branches are visible to anyone who can read the repository; that
  includes everyone once it is public. Work in progress is public then.
- Every published text written by a model says so, under the same ids as
  the commit trailers.
- The checks that enforce this are #2 (trailers in CI and the `commit-msg`
  hook) and #8 (hooks that stop an agent breaking these rules). Until they
  land, the rules are followed by hand.
- `CLAUDE.md`, `AGENTS.md`, `AI.md`, `CONTRIBUTING.md`, the pull request
  template and the `land-change` skill describe these rules.

## Alternatives considered

- **Approve every branch commit (ADR 0002).** Rejected: it slowed the work
  without protecting `main`, which only changes through approved merges.
- **Approve each push and each pull request, with free local commits.**
  Rejected by the owner in favour of the common practice; it adds an approval
  per pull request without a matching gain.
- **A separate machine account for agent work.** It would make the owner's
  approval a GitHub review that the ruleset can require, and keep the owner's
  key off model-written commits. Not taken: the owner prefers approving in the
  session. It stays an option.

## Review

Round 1, 2026-11-19, a design review of issue #7 before any wording.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex | gpt-6.2-sol | high | FAIL: 6 major, 2 minor |

All were accepted and are part of this ADR:

- **Files:** `CLAUDE.md`, `AGENTS.md` and the pull request template also
  carried the old rules (points 1 and 4, Consequences).
- **Merge base:** `--match-head-commit` guards only the head; the base is
  checked again, and the ruleset refuses a stale base (point 2).
- **Secret scan:** of every outgoing commit, not only staged changes; a
  commit is frozen once submitted for review (point 3).
- **Disclosure scope:** titles, review comments, edits and release notes;
  the trailer ids; mixed roles (point 6).
- **`CONTRIBUTING.md`:** `AI-Reviewed-by:` was described more broadly than
  ADR 0002 allows (point 7).
- **Bot branches:** the push and history rules cannot apply to them (point 8).
- *(minor)* **Signing:** what the owner's key on a model-written commit means
  (point 5).
- *(minor)* **Triage:** labels need no line; judgements are attributed
  (point 6).

Round 2, 2026-11-19, review of the drafted change (`41c3ab1..bf7f65d`).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | FAIL: 4 major, 4 minor |

All were accepted and fixed:

- **Who it applies to** (major): branch work without approval was written
  for every session; it is the maintainer's session's. A contributor's agent
  still follows its contributor's approval (`land-change` section 0).
- **ADR 0002's notes** (major): its annotations now say, at each place, that
  only the maintainer's branch commits changed, including its owner
  decision 3.
- **Secrets** (major): the scan before a push covers only commits not yet
  pushed, and a pushed secret is revoked and its branch replaced (point 3),
  so the rules no longer block each other.
- **Author email** (major): only on the maintainer's own merges; a
  contributor's pull request keeps its author (point 2).
- *(minor)* **`AI-Reviewed-by:`**: one standard everywhere, "covers the
  change as it lands" (point 7), recorded as a change to ADR 0002.
- *(minor)* **ADR numbers**: references to "ADR 0003" as the architecture
  decision now say "the architecture ADR".
- *(minor)* **Status**: this ADR is accepted when the owner approves its
  merge; the status values now include `partly superseded by NNNN`.
- *(minor)* **Disclosure form**: `Written by <agent>:<model>.`, adding for
  whom when written for someone else's account (point 6).

The findings came from rules updated in one file and missed in another, and
from rules written for one actor that read as applying to all. So the ADR
process now requires a **supersession sweep** (`docs/adr/README.md`). For
this ADR it was:

- searched for each superseded rule ("branch commits carry", "no trailers",
  "every commit the maintainer's session makes", "approves every commit",
  "before it is made", "as submitted", "for the maintainer") and for "ADR
  0003" across the repository;
- every hit is updated, annotated as superseded, still true (approval of
  every commit that lands on `main`), or review history;
- each rule checked for the maintainer's session, contributors and their
  agents, and bots.

Round 3, 2026-11-19, verification of those fixes (`bf7f65d..22b19c2`).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | FAIL: 5 minor |

All eight round-2 fixes were confirmed. Its five minor findings were places
the sweep had missed, all fixed and checked by the controller, and the review
stopped (no blocker or major):

- `AI.md` still showed only the "for the maintainer" form;
- ADR 0002's "Honest limit" and its "carry no trailers" clause lacked their
  supersession notes;
- the `land-change` description, introduction and heading, and `CLAUDE.md`,
  did not say "the maintainer's";
- the sweep list in `docs/adr/README.md` had absorbed the paragraph after it.

The sweep missed them because it searched for exact phrases. A sweep also
reads each file that states the old rule in full, not only the matching
lines.

