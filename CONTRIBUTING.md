<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Contributing

IronAFS is in its design phase, so the most useful contributions right now are
comments on the [decisions](docs/adr/README.md) and the
[research](docs/research/). Open an issue for design feedback or questions;
the maintainer routes any review it needs. Code contributions follow the
same path as the owner's own changes, described in
[docs/workflow.md](docs/workflow.md).

Everyone taking part follows the [code of conduct](CODE_OF_CONDUCT.md).
Pull requests are reviewed with AI models from several vendors (see
[AI.md](AI.md)), so what you submit is sent to them for review.
Security problems go to [SECURITY.md](SECURITY.md), not to public issues.

## Pull requests

- Before you start on an issue, say so in a comment, so nobody else picks it
  up (`docs/workflow.md` §6). If you stop, comment again so it is free.
- One change per pull request. It is squash-merged: the PR title becomes the
  commit subject (short, lowercase), and the PR description becomes the
  commit body (what changed and why).
- For class P, S or R changes (`docs/workflow.md` §1), open an issue or a
  spec-only draft PR first. The maintainer arranges the required design or
  contract review before code is written.
- End the description with the trailer block from the PR template: `Class:`,
  `Assisted-by:` and `AI-Reviewed-by:` for each model review whose final
  verdict covers the change as it lands (ADR 0003, point 7).
  [AI.md](AI.md) explains each one.
- Put `Assisted-by:` on your branch commits too, and never
  `Signed-off-by:`: your sign-off goes in the description. If a model wrote
  an issue, a comment or a review you post, say so in a last line, such as
  `Written by <agent>:<model>.` (ADR 0003).
- Sign off with your own `Signed-off-by: Your Name <email>` line. It
  certifies the [Developer Certificate of Origin 1.1](https://developercertificate.org/)
  for your change. The owner's sign-off is added after yours at merge.

## Before you open a pull request

`rustup` installs the pinned toolchain and targets from
`rust-toolchain.toml` on first use. Install [cargo-nextest](https://nexte.st/)
separately, then run `cargo xtask check`. Run `cargo xtask hooks install` to
opt in to the `pre-push` hook and the `commit-msg` hook for branch commit
trailers. The `pre-push` hook refuses any update of `main`, scans the
commits you push and their messages for secrets with
[gitleaks](https://github.com/gitleaks/gitleaks), which you install
separately, and then runs `cargo xtask check`. It refuses the push if
gitleaks finds a secret or cannot run. Running the install command again
updates hooks that an earlier version of xtask installed. Use
`cargo xtask hooks install --force` to replace a hook that xtask did not
install. This moves the existing hook to `pre-push.xtask-backup` or
`commit-msg.xtask-backup` in the hooks directory and prints the backup path.
The command refuses if that path is already in use. Use
`cargo xtask hooks uninstall` to remove the managed hooks.

If you use Claude Code, the project's `.claude/settings.json` runs
`cargo xtask guard` before each Bash command the agent runs. It blocks a
commit or push on `main`, a branch commit without `Assisted-by:`, published
text (an issue, pull request, comment, review or release notes) without its
disclosure line, and `gh pr merge` without `--match-head-commit` and
`--body-file`, and says what to fix. It fails closed: a command it cannot
read, or a guard that cannot build, blocks the command. It checks only that
a disclosure line is present and well formed, never who wrote the text.

To run the checks individually, use the following commands.

- `cargo fmt` to apply formatting, or `cargo fmt --all --check` to check it.
- `cargo clippy --all-targets --workspace`, with
  `CARGO_BUILD_WARNINGS=deny`. If you touched code behind an OS `cfg`, run
  clippy with `--target` for Linux, macOS and Windows too.
- `cargo nextest run --workspace --retries 0 --no-tests fail` and
  `cargo test --doc --workspace`.
- `reuse lint` and `cargo deny check`. The local batch reports missing
  optional tools by name. CI requires both checks.
  A new dependency needs the owner's decision first (ADR 0001).

## AI-assisted contributions

Welcome. Using a model is not a problem; hiding it is. Coding agents read
`AGENTS.md` (Claude Code through `CLAUDE.md`), which holds these rules in a form
agents follow.

- Disclose every model whose writing remains in the change with
  `Assisted-by: <agent>:<model id>`, or write `Assisted-by: none`.
- You are the author and answer for all of it. Read and understand
  everything you submit.
- Never let an agent write your `Signed-off-by:`. Add it yourself.
- Don't copy (ADR 0001). Everyone is welcome, including people who know
  OpenAFS, AuriStor or the kernel's AFS and Rx code well. When you write
  IronAFS code:
  - write it from your own understanding and the specifications;
  - don't have another implementation's source open;
  - port or translate nothing;
  - never paste that source into a model's prompt.

  Say so in your pull request, and mention any familiarity that helps
  reviewers. If you are under an NDA or employment duty covering non-public
  AFS source, ask first. If a model's output looks copied from anywhere, say
  so. Permissive third-party material keeps its notices and gets a row in
  [docs/provenance.md](docs/provenance.md). A change whose origin or licence
  is unclear will not be merged.

## Licence

Your contributions to IronAFS's own files are made under `GPL-3.0-or-later`
and certified with your DCO sign-off. Approved third-party material keeps its
own licence (ADR 0001). Every file starts with the SPDX header set out in
[LICENSING.md](LICENSING.md#file-headers), or has a `REUSE.toml` entry, and
`reuse lint` passes.
