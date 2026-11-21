<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Errata: commit messages on `main`

The messages of the squash merges on `main` were audited on 2026-11-21, from the root commit to the merge of #89. History is not rewritten to correct them: the commits are cited by their ids in review records, issues and pull requests, each message carries the owner's sign-off for that exact text, and ADR 0002 and ADR 0003 forbid rewriting pushed commits. This record states what is missing instead.

## The disclosure line

ADR 0003, point 6, asks every pull request description written by a model to end with a line `Written by <agent>:<model>.`, and the description becomes the squash message. Five squash merges made after ADR 0003 was accepted lack that line:

| Commit | Pull request | Subject | Writing disclosed by its `Assisted-by` trailers |
|---|---|---|---|
| `8a07bbc` | #25 | adr 0004: the architecture | `claude-code:claude-mythos-5-2`, `claude-code:claude-opus-5-5`, `claude-code:claude-sonnet-5-5` |
| `48fe0ec` | #27 | workflow: a family is an author only of the text it wrote | `claude-code:claude-opus-5-5` |
| `801a40e` | #29 | adr: firm and working decisions; amend working ones with new evidence | `claude-code:claude-opus-5-5` |
| `05fa895` | #30 | adr 0004: part status lines say accepted | `claude-code:claude-opus-5-5` |
| `1f44f21` | #32 | adr 0004: rxgk is planned for M6 against the clean-room peer (O50.9) | `claude-code:claude-opus-5-5` |

In each, the message was written by the model its `Assisted-by` trailers name, for the maintainer, and the trailers already disclose that. The line itself is what is missing. The root commit (`ce1cb25`) and `41c3ab1` (#6) also have no such line; both were made before ADR 0003 was accepted.

Every squash merge on `main` up to #89 carries its `Class:`, `Assisted-by:` and the owner's `Signed-off-by:` trailers; no other omission was found.

## Wrapping

The squash messages up to #89 are wrapped at 72 columns, the usual git style. On GitHub's commit page that reads as a narrow column. From 2026-11-21 squash messages are written like the pull request descriptions they come from: a short subject, one line per paragraph and per list item, and the trailer block one trailer per line. The text of the earlier messages is unaffected.
