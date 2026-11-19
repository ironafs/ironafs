<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: the first public tree

The candidate tree for the first commit of the public history (class R).
Reviews of the two ADRs are recorded in their own Review sections.

## Mechanical checks

All EXECUTED on 2026-11-18 on the development machine, against the candidate
files (`git ls-files -co --exclude-standard`), re-run after each round of
fixes:

- `reuse lint`: compliant.
- `cargo deny check`: ok.
- `cargo build`, `cargo clippy --all-targets`, `cargo fmt --check`: clean.
- `gitleaks dir` (8.30.1, redacted) on a copy holding only the candidate
  files: no leaks.
- A pattern scan for absolute paths, other email addresses, key and token
  shapes, IP addresses, and the names of the owner's other work: nothing
  beyond the deny list in `.claude/settings.json`.
- Every relative Markdown link resolves.
- No symlinks or executable files. Git's index holds no deleted paths.

## Pre-publication audit

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| claude-code (subagent; a fresh session that saw no earlier review) | claude-mythos-5-2 | default (not reported) | 3 must-fix, about 15 should-fix |

This is a same-family check, run as an extra. It is not the root commit's
required non-author review. All items were fixed, except where noted.

Must fix:

- **Private context disclosed indirectly.** Removed; the lists are now
  generic.
- **References to the owner's other work.** Removed.
- **"Clean room" used for a policy that rejects a strict clean room.** It is
  now "the don't-copy rule" everywhere. ADR 0001's file was renamed before its
  URL becomes permanent.

Should fix:

- **`SECURITY.md`:** it now says which lanes see embargoed vulnerability
  details.
- **`CONTRIBUTING.md`:** it says PRs are reviewed with models from several
  vendors.
- **ADR 0002:** wording that would go stale once public was rewritten.
- **The README trademark notice:** now accurate.
- **Vendor claims:** softened to what was observed.
- **The research note:** its inference about OpenAFS relicensing is marked as
  the note's reading.
- **Smaller fixes:** first-person wording; tool names and "the Mac".
- **`.gitignore`:** extended with secrets, local AFS and Kerberos files, agent
  state, and Rust, Apple and Windows build output.
- **The README** says why there is more process than code.
- **Smaller still:** the PR template asks for the own-understanding statement
  only on code changes; small wording fixes in `CONTRIBUTING.md`, `AI.md` and
  `AGENTS.md`.

Deferred, to be done when the repository is created:

- **Repository settings:** a ruleset on `main`, secret scanning with push
  protection, private vulnerability reporting, Dependabot alerts, a read-only
  Actions token, and wiki, projects and discussions off.
- **Owner checks before creating it:** the account's commit email and 2FA.

## Required non-author review

Round 1, on candidate tree `656ec3d383693d60b317b3e1d45ac940a20841e8` (36
files).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex (GPT) | gpt-6.2-sol | xhigh | FAIL: 1 blocker, 3 major, 9 minor |

All fixed:

- **Blocker: the root-commit procedure could alter the message or the
  identity.** `land-change` now commits with `--cleanup=verbatim` and an
  explicit author and committer. Before pushing, it inspects the commit for
  no parent, the approved tree and identity, and a byte-identical message.
  It pushes only that commit's id.
- **Major: the embargo rule had no workable path.** `docs/workflow.md` §2 now
  defines one: Anthropic and OpenAI only, one writing and the other
  reviewing; other seats after disclosure, or the fix waits; and private
  branches.
- **Major: the security seat could deadlock** when Claude and GPT both took
  part. "Wrote" now means the design or the code, not the message. GLM is an
  eligible security reviewer, and the seat is chosen when the design is
  assigned.
- **Major: `land-change` had a contributor's agent push before approval.**
  Pushing is now a post-approval step.
- **Minor:**
  - `.gitignore`'s `core` patterns would have hidden `core.rs` and `core/`;
    they are now anchored, and this was checked;
  - `allow-build-scripts` is described accurately, with a recorded approval
    for bundled native code;
  - the private repository's retention details were removed;
  - a review disposition that implied CI exists was corrected;
  - this record is pinned to the tree id and gives the effort;
  - the wording about permanent grants now follows Apache-2.0's own terms;
  - `AGENTS.md` and `LICENSING.md` now cover third-party material and
    licence texts;
  - the README trademark line no longer implies working interoperability;
  - an unsourced "first time" claim was removed from the research note.

Round 2, verification of those fixes, on candidate tree
`acc58dc8d9189c095991ce54f401e5fa20050522`.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: all 13 fixes present and correct; 3 new minor |

All three were fixed:

- `AI.md`'s GLM security cell now matches the rule;
- ADR 0002's earlier review note is marked as superseded;
- the root commit's byte-for-byte message check extracts the message from
  the commit before comparing.

## Additions after round 2, at the owner's request

Lessons from building with AI lanes, written as IronAFS's own rules:

- **`docs/coding-standards.md`** (new): numbered rules that never change
  number. Caps before allocation, limits in bytes, a time and randomness
  seam, secrets handling, structure by ownership not size (a written note on
  any file over 3,000 lines), comments, tests, lints and toolchain floors.
- **Workspace lints:** clippy `pedantic` at warn, `undocumented_unsafe_blocks`
  denied, three rustc lints at warn; `clippy.toml` names the project's proper
  nouns.
- **`wire-format`** (new skill): rules for every wire and on-disk format.
- **`phase-lifecycle`** (new skill): how a phase runs, from definition of
  done to exit.
- **`review-lanes`:**
  - the check code goes after the inlined material, with an end marker;
  - size caps, timeouts, and the served model read from the lane's own log;
  - a measured thinking-token floor;
  - findings must cite real files;
  - implementer reports are read for which tests actually ran;
  - inconclusive rounds never become a baseline.
- **`add-dependency`:**
  - the advisories check, and a recorded owner note in place of a headcount;
  - the same rules for dev-dependencies;
  - checks against `Cargo.lock`, and planted-violation proof for bans;
  - exception records, and yearly re-justification.
- **`docs/workflow.md` §3b:** the traps behind "passes only on proof".
- **`rust-review`:** points to the numbered rules, with checklist items for
  limits, the seam, secrets, formats, docs and tests.
- **`.gitignore`:** the maintainer's private skills (`.claude/skills/local-*/`).

Round 3, on the additions, candidate tree
`0997cdf2731cf39b43f0a79fb07dbf8945be4785` (39 files).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex (GPT) | gpt-6.2-sol | xhigh | FAIL: 8 major, 5 minor |

All fixed:

- **Rule 2:** validation uses checked arithmetic and fallible conversions;
  saturation is only for values the format or algorithm defines as clamped.
- **`wire-format`:**
  - value round trips, and byte round trips only for canonical inputs;
  - golden vectors with independent expected bytes, from the spec or from
    reduced synthetic captures;
  - encoder snapshots are regression tests only;
  - CI and the frozen-format index are marked as planned.
- **`phase-lifecycle`:**
  - only notes made from reading source are background-only;
  - lane dispatch is the maintainer's, and contributors use an issue or a
    draft PR;
  - failing-then-passing evidence where the class requires it.
- **`add-dependency`:**
  - the lock file is used for package identity; edge kinds and targets come
    from manifests and `cargo tree`;
  - a reliable package count.
- **`review-lanes`:**
  - exclusions and the scope manifest come before the artefact, private
    local skills included;
  - a "full scope seen" check.
- **Minor:**
  - the lint inheritance for a future `unsafe` crate;
  - `SAFETY` comments for `unsafe impl` as well;
  - planned checks marked as planned;
  - an exact `set -e` statement.

Round 4, verification of those fixes, on candidate tree
`9e8acb1d79fb7f9d59508aa2595343c0595c4f45`.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: all 16 fixes present; 2 new minor |

The run was given a scope manifest of 12 files. Its transcript shows it opened
exactly those 12, and nothing outside them. Both minor findings were fixed:
`rust-review` now requires `SAFETY` comments on `unsafe impl` too, and allows
saturation where "the format or algorithm" defines it, matching rule 2.

## Second addition, at the owner's request: comments and tests

- **Rule 14 (comments) expanded:**
  - what a comment is for, and what to keep;
  - no development narrative;
  - a stated guarantee names what carries it;
  - items cited by name;
  - a change fixes the comments it falsifies;
  - the kinds of comment (`//!`, `///`, `//`, `// SAFETY:`);
  - the public-item doc sections;
  - no bare `TODO`.
- **Rule 15 (tests) expanded:**
  - where tests live, and how they are named;
  - tests from the contract, shown red then green;
  - exact assertions;
  - determinism: no time, sleeps, fixed paths or order dependence;
  - no retries, and hangs are terminated;
  - platforms, evidence, and the runner.
- **`.config/nextest.toml`** (new): `retries = 0`, and slow tests are
  terminated. EXECUTED 2026-11-18 on the development machine (macOS):
  `cargo nextest list` with cargo-nextest 0.9.143 loaded it, exit status 0.
- **`AGENTS.md` and `rust-review`** point to the expanded rules.

Round 5, on the comment and test rules (scope manifest of 11 files; the
transcript shows exactly those 11 opened).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex (GPT) | gpt-6.2-sol | xhigh | FAIL: 4 major, 10 minor |

All fixed:

- **Major: the hang timeout covered only nextest.** Rule 15 and `AGENTS.md`
  now say doc-tests run outside nextest, with a deadline from the CI job.
- **Major: evidence rules disagreed.** Rule 15 now records the expected
  tests, the result, the exit status, the revision and the machine. A test the
  runner terminated is an executed failure; setup and infrastructure failures
  are missing evidence.
- **Major: "red then green" could not apply to new APIs or
  compiler-enforced contracts.** The rule now names the right kind of red
  evidence for each case.
- **Major: lint inheritance was unchecked.** Rule 5 and `rust-review` now
  require `[lints] workspace = true` in every crate.
- **Minor:**
  - retries: the profile setting can be overridden, so runs pass
    `--retries 0`;
  - `///` scope;
  - `TODO(#123)` reconciled with "no task ids";
  - fixtures versus temporary paths;
  - assert the most specific observable result, and how to assert a
    documented panic;
  - `#[track_caller]` only on synchronous helpers;
  - the round-trip requirement in rule 9 depends on an encoder existing;
  - `rust-review`'s target trigger;
  - this record's evidence detail.

At the owner's request, after research, unit tests now always live in
`src/foo/tests.rs`. The basis:

- **The prescription:** the Rust project requires separate test files for
  the standard library and compiler, so that a test edit does not recompile
  the crate.
- **The practice:** most of the ecosystem keeps tests inline, with separate
  files becoming common as files grow.
- **The mechanism:** a test edit then rebuilds only the test target, not
  the crate and its dependents. The research note
  `docs/research/2026-11-18-project-layout-and-tests.md` gives the sources.

Modules use `foo.rs` with a sibling `foo/`, enforced by clippy
`mod_module_files`. EXECUTED 2026-11-18 on the development machine: on a
throwaway copy of the candidate with a planted `foo/mod.rs`, `cargo clippy`
reported "`mod.rs` files are not allowed".

Round 6, verification of round 5 (scope manifest of 11 files; the transcript
shows exactly those 11 opened).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: all round-5 fixes and the `tests.rs` rule confirmed; 1 minor |

The minor finding was that this record said `AGENTS.md` carried the doc-test
deadline, which only rule 15 did. `AGENTS.md` now carries it too.

## Third addition, at the owner's request: layout

A refute-first research pass, run by a Claude subagent (Mythos 5.2) with
public sources only, checked the layout and test decisions and looked for
gaps. Saved as `docs/research/2026-11-18-project-layout-and-tests.md`. It
confirmed the workspace, module and integration-test decisions, and amended
three things:

- **Wording:** always-separate unit tests is IronAFS's choice, not the
  ecosystem convention; a remark about `mod.rs` practice that was never
  measured was dropped.
- **Fuzz targets:** a per-crate `fuzz/` breaks under the workspace's member
  glob (EXECUTED in a scratch workspace), so there is one top-level `fuzz/`
  excluded from the workspace.
- **Test helpers:** Clippy's test allowances do not reach helpers in
  `tests/it/support.rs` (EXECUTED), so `tests/it/main.rs` starts with
  `#![cfg(test)]`.

The added rule 20, "Layout", covers:

- the workspace, crates, dependencies and modules;
- one integration test binary per crate;
- the `fuzz/` crate, `cargo xtask` for automation, and the test-support
  crate;
- build scripts, `Cargo.lock`, and the items deferred with their rule;
- the per-OS crate names, proposed for the architecture ADR.

Rule 15, `AGENTS.md`, `rust-review` and `wire-format` point to it.
References to scripts became `cargo xtask` commands.

Round 7, on the layout research and rule 20 (scope manifest of 11 files; the
transcript shows exactly those 11 opened).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: 1 major, 4 minor |

- **Major: two new files were not yet in git's index.** It is procedural, not
  a content defect: `land-change` builds the root commit from an empty index
  and the explicit candidate list, and the checks run on that list.
- **Minor, all fixed:**
  - rule 20's Windows wording is now a goal, with the ban scoped to
    repository automation;
  - the research note's count of sources read raw;
  - the scope of "no shell scripts";
  - `docs/next-session.md` now lists today's rules, skills and research.

## Fourth addition, at the owner's request: README and GitHub

- **README rewritten:** why IronAFS exists, status with a milestones link,
  goals, a planned-design diagram marked as not yet code, compatibility,
  where to start reading, building with nextest, security and licence.
- **`.github/`:** issue templates (bug, design question, task), with blank
  issues off and security reports pointed to `SECURITY.md`; Dependabot for
  Cargo and Actions, weekly; `CODEOWNERS`.
- **`docs/workflow.md` §6:**
  - issues as memory; milestones as phases;
  - the label set;
  - templates, Dependabot and `CODEOWNERS`.

  Discussions are off (owner decision).
- **`docs/maintainer/controller.md`:** the `issue-memory` duties, and
  creating the labels, milestones and first issues with the repository.
- **The research note on driverless clients:**
  - one accurate disclaimer;
  - the three items from reading source marked "(from source)";
  - a caveat on the line counts;
  - corrected costs;
  - a new section on protocol documentation.

Round 8, on the README, the GitHub files and the late additions (scope
manifest of 16 files; the review confirmed it opened exactly those).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex (GPT) | gpt-6.2-sol | high | FAIL: 1 blocker, 4 major, 9 minor |

All fixed:

- **Blocker: issues and the embargo clashed.** Embargoed security work now
  stays in a private advisory and its temporary fork, or an unpushed local
  branch. The public issue and review record are written at disclosure
  (workflow §2 and §6, controller).
- **Major:**
  - under embargo, the reviewing family also does the consistency pass;
  - the "Bug" form was renamed "Bug report", since GitHub requires a name
    longer than three characters;
  - repository setup now enables private vulnerability reporting and checks
    the button. `SECURITY.md`'s button reference is conditional on it;
  - the research note's lock conclusion is now platform-scoped and left open
    for the spike, citing OpenAFS's documented Windows mode.
- **Minor:**
  - the README's history wording, and its build block (nextest noted as a
    separate install, used once there are tests);
  - the task form's failure field is required, and the class is applied at
    triage;
  - blank-issue, `CODEOWNERS` and Dependabot behaviour are described as
    GitHub actually behaves;
  - a stale next-session item;
  - the macFUSE wording;
  - FSKit at `/afs` marked unconfirmed.
- **Also added:** a numbered repository-creation checklist in
  `docs/maintainer/controller.md`.

Round 9, verification of round 8 (scope manifest of 10 files; the transcript
shows exactly those 10 opened).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: all 14 fixes confirmed; 2 new minor |

Both fixed:

- **Repository creation now has two stages.** Settings available on a
  private repository are applied at creation. Secret scanning with push
  protection, and private vulnerability reporting, are enabled and verified
  after the owner makes the repository public, since a personal private
  repository does not offer them without a paid plan.
- **Disclosure:** workflow §6 and the controller now name both public
  artefacts written then, the issue and the review record.

## Fifth addition, at the owner's request: dependencies

**Coding standards rule 21:** IronAFS's own code first, and a dependency
only when we must.

- **Written in IronAFS:** the protocols (including the FUSE wire protocol),
  the event loop, CLI parsing, errors, logging, configuration parsing and
  test helpers.
- **Crates only for:** mainstream cryptography and the operating system's
  own interfaces.
- **Every crate** is the owner's decision.

It cites the Rust blog's malicious-crate reports and the sudo-rs reduction.
The `add-dependency` skill and `AGENTS.md` follow it.

Round 10, on rule 21, the README goal and the two-stage setup (scope
manifest of 9 files; the review confirmed it opened exactly those).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex (GPT) | gpt-6.2-sol | high | FAIL: 6 major, 5 minor |

All fixed:

- **The `add-dependency` skill** no longer allows crates outside the two
  permitted categories, and lists the exact SPDX ids, with unclear licences
  refused.
- **Test and fuzz manifests** follow rule 21. The fuzzing engine's binding is
  the one planned exception, an owner decision when the first fuzz target
  lands.
- **The excluded `fuzz/` crate** declares its own dependencies and lints,
  under the same policy and checks.
- **"Operating system interfaces"** now covers libraries a platform's vendor
  or distribution ships, even as a separate package. Each platform ADR names
  the library, its linkage and its binding.
- **Repository setup:**
  - rulesets, an effective `CODEOWNERS`, private vulnerability reporting and
    secret scanning are enabled and verified after the owner makes the
    repository public, before any change to `main`, with each feature's
    eligibility stated separately;
  - the Actions token is described as a restricted default, with each
    workflow's own permissions checked;
  - before publication, embargoed work stays on an unpushed local branch.
- **Wording:**
  - the malicious-crate wording is now precise;
  - cancellation safety is labelled as an IronAFS requirement;
  - the README describes Cloud Files as a sync provider.

## Sixth addition, at the owner's request

- **Mythos 5.2** now takes every Claude security review (workflow §2,
  `AI.md`, `review-lanes`).
- **The README** gains "How IronAFS is built": the five families, their
  models and roles, the human approval of every commit, and AI use by
  contributors. `AI.md` opens with why five families.
- **`docs/research/2026-11-18-platform-support.md`:** the support floors and
  RHEL 7, researched from primary sources with executed probes. Its open
  questions go to the architecture ADR.
- **Git hooks** are planned for PR #1, as shims calling `cargo xtask`,
  because rule 20 rules out shell scripts and the checks need tests.

Round 11, verification of the sixth addition (scope manifest of 11 files; the
transcript shows exactly those 11 opened).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: 5 minor |

All fixed:

- the README's review wording scoped to changes that get a model review;
- rules 1–21 named correctly;
- IronAFS's own FUSE layer named in the architecture plan;
- the fuzzing exception stated where the README says "only";
- `docs/next-session.md`'s numbered plan joined into one list.

## Seventh addition: README practice, folder READMEs, licence

**README, following researched practice** (GitHub's documentation, the
standard-readme specification, opensource.guide, the OpenSSF criteria):

- badges with alt text: project status "Concept", licence, and AI-assisted;
- the status as a GitHub "Important" alert;
- a text version of the design diagram;
- a Contributing section;
- the copyright holder named.

**Folder READMEs:** `docs/`, `docs/adr/`, `docs/research/` and
`docs/reviews/` each explain their folder and index its contents.

**The licence: GPL-3.0-or-later** (ADR 0001, review round 6), with a
research note in `docs/research/`.

- `LICENSE` and `LICENSES/GPL-3.0-or-later.txt` come from the REUSE tool.
- Every SPDX header, `REUSE.toml` and Cargo's `license` field changed.
- `deny.toml` ignores the unpublished workspace crates, so its allowlist
  governs dependencies only.
- README, `LICENSING.md`, `CONTRIBUTING.md`, `AGENTS.md` and `rust-review`
  match.
- Contributions are DCO only at launch.

Round 12, a full-tree review of all 51 candidate files, on candidate tree
`39baacfd5f431321da5d58187ac880ada1f3fd79`.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex (GPT) | gpt-6.2-sol | xhigh | FAIL: 8 major, 5 minor |

It opened all 51 files and reproduced the candidate tree. It confirmed that
both licence files are the unmodified GPLv3 text. All findings are fixed:

- **The embargo:** `land-change` gained a procedure for embargoed fixes
  (replaced in round 14).
- **The research notes:**
  - the driverless note's licence conclusion now reflects GPL-3.0-or-later,
    and separates the AuriStor terms check;
  - the licence note is marked as research, and points to ADR 0001 for the
    decisions;
  - the RHEL 7 x86_64 feasibility is marked as inferred, with the tests
    required before claiming it;
  - macFUSE's FSKit backend and its partly closed source are described
    accurately.
- **The release rule:** downloadable releases give equivalent access to the
  source (GPLv3 §6(d)), and a written offer is only for physical media. The
  public summaries say "a work based on it", with a clarification on
  aggregation.
- **`rust-review`** no longer suggests third-party error crates (rules 4 and
  21).
- **`land-change`** runs nextest with `--retries 0` and the doc-tests.
- **Approval scope:** the README and ADR 0002 now say the owner approves the
  maintainer session's commits and everything landing on `main`, while
  contributors approve their own branch commits.
- **The reviewer table** is an order of preference with an authorship check
  and a fallback. Under embargo, one family writes the design and the fix, and
  the other reviews both.
- **Minor:**
  - `AI.md` and the README scope independence to required reviews, and name
    only the families whose findings the records show;
  - `CONTRIBUTING.md`'s licence statement covers third-party material and
    `REUSE.toml`;
  - `docs/provenance.md` lists the GPL text files as FSF material;
  - `AGENTS.md` names the fuzzing exception.

Round 13, verification of round 12, on the working tree after the round-12
fixes (not pinned to a tree id; scope manifest of 15 files, and the
transcript shows exactly those 15 opened). Its verdict is superseded by
round 14, which covers the whole tree.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: 4 minor |

All round-12 fixes are confirmed, with no Apache-2.0 statement about IronAFS
left outside historical records. All four minor findings were fixed:

- the reviewer table: each row's order comes first, then a stated fallback
  order;
- ADR 0002's owner decision 3: approval scoped like the body;
- `AI.md`'s independence sentence: scoped to required reviews;
- the README's aggregation note: no longer limited to unmodified copies.

At the owner's request, the licence goal in ADR 0001, the README and the
licence research note now reads: IronAFS, and every version of it that is
distributed, stays free and open source. Whoever distributes it passes on
the same freedoms and makes the source available, so improvements can flow
back to its users and the wider community.

Round 14, the final full-tree review, on candidate tree
`94d37009a499defa3e424bd1b69562a58ea6bf97` (all 51 files opened).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex (GPT) | gpt-6.2-sol | xhigh | FAIL: 8 major, 4 minor |

All findings are fixed:

- **The embargo:** the advisory's temporary private fork is used only to
  prepare a fix. At disclosure the fix lands through a normal, approved
  squash PR, never the advisory's merge button, and the advisory is
  published with it (`land-change` §4a, workflow §2).
- **"Complete source":** the README, `LICENSING.md`, ADR 0001 and the licence
  note say a distributor must make the corresponding source available by a
  GPLv3 §6 method, and that separate programs shipped alongside are not
  covered.
- **MPL-2.0:** allowed only without an "Incompatible With Secondary Licenses"
  notice; the note describes the MPL duty as making all covered source
  available, not only changed files.
- **Quotations:** short, cited quotations in research notes and review
  records are not incorporated third-party material (ADR 0001).
- **FUSE caller identity:** both research notes say delayed writeback
  writes carry no valid caller identity. The architecture ADR (planned as ADR
  0003 then; now ADR 0004 or later) must show how authorisation
  survives deferred I/O; writeback stays off until then
  (tracked for the architecture ADR).
- **Tests in `land-change`:** the suites apply where tests of that kind exist;
  nextest's "no tests" exit is recorded as such, not as a pass or failure.
- **This record:** rounds 12 to 14 give their tree ids, or say they are not
  pinned.
- **macOS distribution:** a separate check that a first-party FSKit
  extension can be signed, notarized, installed and loaded outside the store
  comes before the macOS layer (tracked for that phase).
- **Minor:**
  - ADR 0001: System Libraries are checked per component in each platform's
    ADR, and the FUSE kernel interface is not a linked library;
  - workflow §3: the class I consistency pass goes to the first eligible
    reviewer from the seat table;
  - the reviews index no longer claims nothing is summarised;
  - VLC for iOS: cites VideoLAN's announcement of its 2013 return.

Also at the owner's request, in the same batch:

- the maintainer's controller instructions are no longer a public file; they
  live in the maintainer's private instructions, and the public files say so;
- ADR 0001 no longer mentions a future service component's licence;
- `docs/adr/template.md` gives the sections every ADR has;
- the session plan (`docs/next-session.md`, named in the rounds above) is no
  longer published. Plans live in the maintainer's private notes until they
  become milestones and issues.

Round 15, verification of round 14 and the owner's batch, on candidate tree
`af663e25339222c0686c56f858bd2436735ad873` (scope manifest of 19 files; the
transcript shows all 19 opened).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | FAIL: 1 major, 1 minor |

Every round-14 item and every owner change was confirmed fixed, except:

- **Major, fixed:** the licence note's comparison table and its MPL
  alternative still described the MPL duty as covering only changed files.
  Both now say all MPL-covered files, changed or not.
- **Minor, fixed:** the README's licence summary now names the GPL's methods
  for making the source available, like `LICENSING.md` and ADR 0001.
- **Observation, fixed:** the private session plan now times the FSKit
  distribution check like ADR 0001 ("before the macOS layer is designed").

Changed after round 15's tree, and checked by the controller against the
files: the session plan was made private (this record and ADR 0002 note it
in their history); the licence note's list of questions for a lawyer was
shortened; `docs/README.md`, `AGENTS.md` and the `phase-lifecycle` skill now
point to milestones and issues instead.

A final check of every file before the root commit, by the controller,
found three gaps, all fixed:

- `.gitattributes` keeps LF line endings on every platform and marks
  `Cargo.lock` as generated;
- the pull request template shows the `AI-Reviewed-by:` and
  `Signed-off-by:` lines, so deleting its comments no longer drops the DCO
  sign-off prompt;
- Dependabot waits seven days before proposing a new release (security
  updates are not delayed).

At the owner's request, the toolchain moved to the current stable Rust,
1.100.0, and `rust-version` to 1.100. Its platform-support page gives the
same floors as 1.97.1 (Linux kernel 3.2+ and glibc 2.17+; aarch64 kernel
4.1+; macOS 11+; Windows 10+; musl 1.2.5), so the platform research stands.
Build, fmt, clippy and cargo-deny pass on it.

A last look at the Cargo and toolchain files, at the owner's request, added:

- the denied lints for the other panicking macros (`todo!`,
  `unimplemented!`, `unreachable!`), string slicing and `dbg!`, so the lints
  match the untrusted-input rule (coding standards rule 1);
- the six target platforms in `rust-toolchain.toml`, so the cross-OS clippy
  runs that `AGENTS.md` asks for work on any machine. Clippy passes for
  Linux (x86_64, aarch64), macOS (aarch64, x86_64) and Windows (x86_64,
  aarch64).

Round 16, verification of every change since round 15, on candidate tree
`0acdc7655333944cff8f4b602fd08903cfd0e241` (the diff from round 15's tree,
inlined; the lane read the touched files and checked them against the rest
of the tree).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | FAIL: 1 major |

Every change in the diff was confirmed correct: no public file points to the
session plan as a live file; the denied lints match rule 1 one to one; the
toolchain, `.gitattributes`, Dependabot and pull request template changes
are valid. The major finding: the working tree already held further edits to
the README and `CONTRIBUTING.md` that the diff did not cover. They are
covered by round 17.

After round 16, at the owner's request:

- **README:** the licence badge is a fixed "GPL-3.0-or-later" badge (the
  dynamic one dropped "or later", and cannot read a private repository); the
  don't-copy sentence names the kernel's AFS and Rx implementations and links
  ADR 0001, like `LICENSING.md`; long lines reflowed.
- **`CONTRIBUTING.md`:** a "Before you open a pull request" section with the
  commands to run; one line reflowed.
- **`.claude/settings.json`:** a `$schema` line for editor validation, and
  read denials for SSH keys, Kerberos and AFS key material, `.env` files,
  PEM and key files and other credential stores, plus
  `git config --get-regexp`.

Round 17, verification of those changes, on candidate tree
`9d76d3afd038c5f517c8afc6956140dcfe411ff2` (the diff from round 16's tree,
inlined; the index and the working tree matched it throughout).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: no findings |

It confirmed the badge and the don't-copy sentence against `LICENSING.md` and
ADR 0001, the new contributor section against `AGENTS.md` and the toolchain
file, and every new settings rule's syntax and purpose. Its open points were
checked by the controller: the schema URL resolves to SchemaStore's Claude
Code settings schema, and the badge renders. The only change after round 17
is this entry.

