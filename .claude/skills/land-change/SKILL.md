---
name: land-change
description: Use before EVERY git commit, push or merge in this repository — branch commits, squash merges and the root commit. Branch work follows its rules without asking (ADR 0003); a squash merge or the root commit is presented, with its exact final message, to the human you work for, who must approve it before it is made (ADR 0002). Triggers on "commit", "merge", "land", "push", "open a PR", or any step that would create a commit.
---

# Landing a change

The human you work for approves every merge to `main` and the root commit
before it is made (ADR 0002). Branch work needs no approval (ADR 0003, and
section 0 below). For the maintainer the human is the owner; for a
contributor, it is the contributor. The only `Signed-off-by:` an agent ever
writes is the owner's, in the maintainer's session, after the owner approves
the exact final message of a squash merge or the root commit. A
contributor's line is only ever copied, unchanged, from what they wrote
themselves. This procedure makes sure that what lands is exactly what was
approved.

"The human" below means that person. Their sign-off uses their own identity
(`git config user.name` and `user.email`), never anyone else's.

## 0. Branch work (no approval needed)

On a task branch, never on `main`, commit, push and open a draft pull request
without asking (ADR 0003):

- Commit before each review round, and cite the commit range in the review
  record.
- Once a commit is under review or pushed, it is frozen: fixes go on top as
  new commits. Never amend, rebase or force-push it.
- Before every push, scan every outgoing commit and its message for secrets
  (`gitleaks git --log-opts="origin/main..HEAD" --redact .`). Do not push if
  it finds one or cannot complete.
- Mark the pull request ready for review once the reviews its class requires
  are recorded.

## 1. Checks before committing or presenting

**Every commit, branch commits included:**

- The staged change is in the scope of the task, and nothing else is staged.
- Nothing private is in the diff or the message: no paths outside the
  repository, hostnames, cell names, keys, other projects, or anything from
  `CLAUDE.local.md`.
- `cargo fmt --check` and `cargo clippy --all-targets` pass. Add `--target`
  for every OS when a `cfg` is touched.

**A squash merge, in addition** (the maintainer's session only):

- The class's checks are met (`docs/workflow.md` §1–§3). Each is labelled
  with its evidence (§3c), such as EXECUTED, with the revision and the result.
- `cargo nextest run --retries 0` and `cargo test --doc` pass wherever the
  tree has tests of that kind (watched: a run that does not finish is a
  failure). nextest exits 4 when it finds no tests; that is recorded as "no
  tests yet", not a failure. `reuse lint` and `cargo deny check` pass.
- Every review seat has its record (in `docs/reviews/` or the ADR's Review
  section). Every credited seat's verdict covers the substantive files at the
  revision being landed.
- Confirmed correctness and contract findings are resolved, whatever their
  severity.

**The root commit, instead** (the maintainer's session only, once; ADR 0002):
a non-author review of the candidate tree, pinned to its tree id; the owner's acceptance of the ADRs
it holds; provenance and privacy checks; build, clippy, fmt, `reuse lint` and
`cargo-deny`; and approval of the exact tree id and message. The
code-specific class R gates (design review before code, reproduction test) do
not apply to the scaffold.

**A contributor's pull request**, before anything is pushed:

- Prepare the PR. The title is the short commit subject. The description is
  the body, ending with the trailer block from the PR template.
- The contributor writes their own `Signed-off-by:` line in the description.
  The agent never writes it; if it is missing, ask them to add it.
- State that the code was written from the contributor's own understanding
  and the specs, with nothing ported or translated (ADR 0001).
- For class P, S or R, the design or contract is discussed first, in an issue
  or a spec-only draft PR (`CONTRIBUTING.md`), before code is written.
- Present the branch, the title and the description to the contributor. Push
  and open the PR only after their approval (step 4).

## 2. The message

- **A branch commit:** a short, lowercase, casual subject and an optional
  body, then `Assisted-by:` lines for each model whose writing remains in it
  (or `Assisted-by: none`). Never `Signed-off-by:`, `Class:` or
  `AI-Reviewed-by:`.
- **A squash merge or the root commit:** a subject, then what changed and why,
  including findings refuted with their evidence. Then one trailer block as
  the last paragraph, with no blank lines inside it:

  ```
  Class: <E|I|P|S|R>
  Assisted-by: <agent>:<served model id>        (one per model whose writing remains; or "none")
  AI-Reviewed-by: <agent>:<served model id>     (only seats whose verdict covers this tree)
  Signed-off-by: <an outside contributor, if any, exactly as they wrote it>
  Signed-off-by: <the owner, written only after they approve this exact message>
  ```

  - Agent ids are in `AI.md`; the model ids are the served ones, from each
    lane's output.
  - For Claude Code, record the served id: the `fable` alias serves
    `claude-mythos-5-2`.
  - Never add `Co-authored-by:` for a model.

## 3. Present and wait

Show the human, in one message:

- **What exactly lands.** Stage it first, then show:
  - `git diff --cached --stat`, with a one-line summary per file and new and
    deleted files named;
  - the tree id from `git write-tree`.
- **The exact, complete message**, including the `Signed-off-by:` lines.
- **For a merge**, all of:
  - the PR number;
  - the target branch;
  - the base commit the checks ran on;
  - the PR head commit;
  - for an outside contributor, confirmation that their sign-off line in the
    body is unchanged and matches the PR author.

Then stop. Do not merge, make the root commit, or (for a contributor's PR)
push, until the human approves exactly that. If anything changes after
approval, present it again: the tree, the base, the head, or a single
character of the message.

## 4. Do exactly what was approved

Write the approved message to a file once and use that file. Before acting,
re-check that the tree id (or the base and head) is still the approved one.

- **A contributor's PR:** push the branch and open the PR with the approved
  title and description (`gh pr create --title ... --body-file ...`).
- **A squash merge** (maintainer only; contributors open a PR instead):
  immediately before merging, check again that `main` is at the approved base,
  the PR head is the approved head, and the message file's SHA-256 is the
  approved one. Then:
  `gh pr merge <n> --squash --match-head-commit <approved head> --author-email <the owner's public address> --subject "<approved subject>" --body-file <approved body file>`.
  Merge one PR at a time. After the read-back (section 5), post a comment on
  the PR logging the approval: the head, the base and the message's SHA-256,
  with the disclosure line.
- **The root commit** (maintainer only, once; no `main` exists yet):
  1. On an orphan branch, start from an empty index (`git rm -r --cached .`).
  2. Add exactly the reviewed files, by explicit path list.
  3. Check that `git write-tree` gives the approved tree id.
  4. Commit with the message and identity fixed exactly:
     `GIT_AUTHOR_NAME=... GIT_AUTHOR_EMAIL=... GIT_COMMITTER_NAME=...
     GIT_COMMITTER_EMAIL=... git commit --cleanup=verbatim -F <message file>`,
     using the approved public identity for both author and committer.
  5. Before pushing, inspect the new commit with `git cat-file -p <sha>`:
     - it has no parent;
     - its tree is the approved tree id;
     - its author and committer are the approved identity;
     - its message is byte-for-byte the approved file:
       `git cat-file commit <sha> | sed '1,/^$/d' > msg.landed && cmp msg.landed <message file>`.
  6. Push only that verified commit: `git push <new repo> <sha>:refs/heads/main`.

## 4a. Embargoed security fixes

An embargoed fix (`docs/workflow.md` §2) never touches a public branch,
issue or PR before disclosure.

- **Before disclosure,** the fix is prepared on an unpushed local branch or
  in a GitHub security advisory's temporary private fork. The fork is for
  private collaboration only. Never use the advisory's own merge button: it
  bypasses status checks, branch protection and the approved message.
- **Every check runs privately** first, and the results are recorded.
- **At disclosure,** open the PR and land it through the normal squash path
  above: the exact change and message presented, approved, then merged and
  read back. Publish the advisory together with the fix.

## 5. Read back

- **A merge:** take the squash commit's sha from GitHub (`gh pr view <n>
  --json mergeCommit`). Fetch it, and compare its tree, message, author and
  committer with what was approved.
- **A local commit:** compare `git log -1 --format='%H%n%T%n%an <%ae>%n%B'`.
- Report what landed. Any mismatch is fixed by a new, approved commit, never
  by rewriting `main`.

## Never

- Commit "to save a step", or bundle an unapproved change into an approved
  one.
- Write any `Signed-off-by:` yourself, other than the owner's in the
  maintainer's session after their approval of the exact message. Never
  write a contributor's line for them.
- Push to `main` directly. The root commit is the only exception.
- Amend, rebase or force-push a commit that is pushed or under review.
- Post an issue, pull request description, comment or review written by a
  model without its disclosure line (ADR 0003).
