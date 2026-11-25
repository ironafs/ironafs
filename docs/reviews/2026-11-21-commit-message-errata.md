<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Errata: commit messages on `main`

The messages of the squash merges on `main` were audited on 2026-11-21, from the root commit to the merge of #89; two later merges are added below. History is not rewritten to correct them: the commits are cited by their ids in review records, issues and pull requests, each message carries the owner's sign-off for that exact text, and ADR 0002 and ADR 0003 forbid rewriting pushed commits. This record states what is missing instead.

## The disclosure line

ADR 0003, point 6, asks every pull request description written by a model to carry a visible line `Written by <agent>:<model>.`, adding "for the maintainer" when it writes for the owner's account, placed above the trailer block; the description becomes the squash message. Five squash merges made after ADR 0003 was accepted lack that line:

| Commit | Pull request | Subject | Writing disclosed by its `Assisted-by` trailers |
|---|---|---|---|
| `8a07bbc` | #25 | adr 0004: the architecture | `claude-code:claude-mythos-5-2`, `claude-code:claude-opus-5-5`, `claude-code:claude-sonnet-5-5` |
| `48fe0ec` | #27 | workflow: a family is an author only of the text it wrote | `claude-code:claude-opus-5-5` |
| `801a40e` | #29 | adr: firm and working decisions; amend working ones with new evidence | `claude-code:claude-opus-5-5` |
| `05fa895` | #30 | adr 0004: part status lines say accepted | `claude-code:claude-opus-5-5` |
| `1f44f21` | #32 | adr 0004: rxgk is planned for M6 against the clean-room peer (O50.9) | `claude-code:claude-opus-5-5` |

In each, the message was written for the maintainer by a model its `Assisted-by` trailers name, so the trailers already disclose the model writing. The line itself is what is missing. The root commit (`ce1cb25`) and `41c3ab1` (#6) also have no such line; both were made before ADR 0003 was accepted.

Every squash merge on `main` up to #89 carries its `Class:`, `Assisted-by:` and the owner's `Signed-off-by:` trailers; no other omission was found.

## The sign-off outside the trailer block

Two squash merges made on 2026-11-21 carry the owner's `Signed-off-by:` trailer after a blank line, so it stands apart from the trailer block that holds `Class:`, `Assisted-by:` and `AI-Reviewed-by:`:

| Commit | Pull request | Subject |
|---|---|---|
| `98d0f24` | #109 | ironafs-crypto: the random source interface and the production rng |
| `86c78e4` | #105 | xtask: cell pinned definition, engine runner, teardown and verified install |

Each message file was built from the pull request description, which ended in a newline, and the sign-off was appended after it; the read-back compared each commit with that same file. The words are the text the owner approved, plus the sign-off; the blank line is the only difference from the message shown for approval. `cargo xtask trailers main` reads the newest commit on `main` and failed on the push of each. The sign-off is now appended by a step that strips trailing blank lines and refuses a message whose final paragraph does not hold `Class:`, `Assisted-by:` and the sign-off together. Each pull request carries a comment that corrects its approval log.

## Wrapping

Most squash messages up to #89 are wrapped at 72 columns, the usual git style; three (`661bdb4`, #23; `9db9478`, #24; `f81aa7d`, #39) already have some unwrapped paragraphs. On GitHub's commit page a wrapped message reads as a narrow column. From 2026-11-21 squash messages are written like the pull request descriptions they come from: a short subject, one line per paragraph and per list item, and the trailer block one trailer per line. The text of the earlier messages is unaffected.
