<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# IronAFS: instructions for coding agents

These rules apply to every AI agent working in this repository (Claude Code,
codex, Antigravity, or any other), and to the people directing them.

IronAFS is a modern AFS client in Rust: one shared core with thin per-OS
layers (Linux FUSE, macOS FSKit, and on Windows a local SMB server reached
through Windows' own SMB client, planned) and one `afs` command line. It is meant to work with existing AFS cells. OpenAFS servers are the
first intended test target, and nothing is tested yet. It has no kernel
extensions or drivers of its own.

## Start here

- The open milestone and its issues on GitHub: where work stands and what
  comes next (`docs/workflow.md` §6).
- `docs/workflow.md`: how changes are classed (E/I/P/S/R), made and reviewed.
- `docs/coding-standards.md`: the numbered rules for Rust code, including
  the workspace and test layout (rules 15 and 20).
- `docs/adr/README.md`: the decisions. Read ADR 0001 (licence and "don't
  copy") before writing any code, and ADR 0002 (AI attribution) before any
  commit.
- `AI.md`: how AI is used here, and how commits credit it.
- `CONTRIBUTING.md`: how a change gets in.

Spec first: a new phase starts with an ADR or spec, not code.

## Commands

- build: `cargo build`
- test: `cargo nextest run --workspace --retries 0 --no-tests fail`
  (nextest terminates hung tests; see `.config/nextest.toml`), plus
  `cargo test --doc --workspace` for doc-tests, which run outside nextest
  under a CI step deadline (by hand, a run that does not finish is a
  failure)
- lint: `cargo clippy --all-targets --workspace` (no warnings). When a
  change touches code behind an OS `cfg`, also run it with `--target` for
  Linux, macOS and Windows.
- format: `cargo fmt` to apply, `cargo fmt --all --check` to check
- licences: `reuse lint` and `cargo deny check`
- automation: `cargo xtask check` runs the local checks.
  `cargo xtask hooks install` opts in to running them before each push,
  after refusing any update of `main` and scanning the outgoing commits and
  their messages for secrets with `gitleaks`, and to checking branch commit
  trailers with the `commit-msg` hook.
  `cargo xtask hooks uninstall` removes the managed hooks. Keep repository
  automation in `xtask`.
- guard: in Claude Code, `.claude/settings.json` runs `cargo xtask guard`
  before every Bash command. It blocks a commit, push, pull, rebase or ref
  update that moves `main`, a branch commit without its `Assisted-by:`
  trailer, an issue, pull request, comment, review or release text without
  its disclosure line, and `gh pr merge` without `--squash`,
  `--match-head-commit` and `--body-file`. Each block
  says what to fix. It reads only the command line, so it is a guardrail,
  not a proof: these rules hold whether or not it runs.

## Hard rules

- **Licence.** IronAFS's own material is `GPL-3.0-or-later`, with an SPDX header
  (or a `REUSE.toml` entry) in every file. Third-party material keeps its own
  licence (ADR 0001).
- **Don't copy** (ADR 0001).
  - No code from OpenAFS, AuriStor, or the Linux kernel's AF_RXRPC or kafs:
    nothing copied, ported or translated, and nothing generated from their
    files.
  - Never put their source, or their file and function names, into a model's
    prompt.
  - Work from published specifications, observed behaviour and `docs/specs/` (planned).
  - Permissive third-party material keeps its notices and gets a
    `docs/provenance.md` row in the same commit.
- **Names.** Never use "OpenAFS" or "AuriStor" in a product, crate or package
  name. "AFS client" and "works with OpenAFS cells" are fine as descriptions.
- **Untrusted input.** No `unwrap`, `expect`, `panic` or indexing reachable
  from it. The client parses fileserver data and serves callbacks to
  fileservers. Tests may use them (`clippy.toml`).
- **`unsafe`** is forbidden workspace-wide. A crate that needs it (FSKit, SSPI or
  other OS bindings) gets an ADR first, then named `// SAFETY:` sites.
- **Reuse and neighbours.** New code reuses what exists and reads like the code
  around it (`docs/workflow.md` §3a). Name any deliberate departure, with its
  reason.
- **Dependencies.** Our own code first (coding standards rule 21). A crate
  only for mainstream cryptography or the operating system's own interfaces,
  plus one planned test-only exception (the fuzzing engine's binding), and
  only with the owner's decision and the `add-dependency` checklist, before
  the edit.
- **No history in source.** Source carries no development history, task ids
  (other than a public issue in a `TODO(#123)`), or reviewer or model
  attribution. Those go in commit trailers and
  `docs/reviews/`.
- **No secrets.** Run no command that could print a credential: tokens,
  keychains, `env`, git credential configuration.
- **Public by default.** Everything tracked is written to be public. No paths
  outside the repository, hostnames, cell names, keys or private notes in
  files, PR descriptions or review records.

## Commits (ADR 0002)

- One branch per change. A change lands on `main` by pull request, as one
  squash commit, after its class's checks.
- The PR description becomes the commit message. It ends with one trailer
  block:
  - `Class:`;
  - `Assisted-by: <agent>:<model id>` for each model whose writing remains,
    or `Assisted-by: none`;
  - `AI-Reviewed-by:` for model reviews that cover the final change;
  - each human's `Signed-off-by:`.

  `AI.md` explains each one.
- **A `Signed-off-by:` is a human's certification** under the Developer
  Certificate of Origin. Contributors write their own line, in the PR
  description; an agent never writes it for them. Branch commits carry
  none. The one disclosed exception is the maintainer's: after the owner
  approves the exact final message of a squash merge, or of the root commit
  of the public history (the one commit made without a PR), their session
  adds the owner's line (ADR 0002).
- **Branch commits** carry `Assisted-by:` (or `Assisted-by: none`), never
  `Signed-off-by:`, `Class:` or `AI-Reviewed-by:`. Never rewrite a commit
  once it is pushed or under review; scan outgoing commits for secrets
  before pushing (ADR 0003).
- **Disclose model writing beyond commits.** An issue, pull request
  description, comment or review written by a model ends with a line such as
  `Written by <agent>:<model>.`, adding for whom when it writes for someone
  else's account (ADR 0003).
- Never add `Co-authored-by:` for a model.
