<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: open pull requests, and settling issues

Issue #4, class I. The change adds rules to `docs/workflow.md` §6 for picking
a task, a limit on open pull requests, parking, abandoned pull requests,
settling and closing issues, cleanup and a queue review, and a line on
claiming an issue to `CONTRIBUTING.md`. Written by Claude Code
(`claude-opus-5-5`).

Round 1, review of the change against the issue's brief.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: 4 minor |

It confirmed that the change meets the brief, avoids its failure cases, and
contradicts nothing in the workflow, `CONTRIBUTING.md`, `AGENTS.md` or the
issue and pull request templates. All four minor findings were fixed:

- the author is asked, with seven days to answer, before an abandoned pull
  request is parked or closed; the text now says so in that order;
- the 14 days count only the author's own pushes and comments;
- a ready issue is triaged and not an `option`, unless the owner asks;
- the limit is per author, so the maintainer's pull requests never use up a
  contributor's, and a pull request opened over the limit stays a draft.

Round 2, verification of those fixes (the diff of the change after them).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: 4 minor |

All four round-1 fixes were confirmed. Its four new minor findings were fixed,
and checked by the controller against the text:

- an issue settled without a repository change (a question answered, a
  refuted report, a decision recorded in the issue) closes by hand; changes
  to the repository always land by pull request;
- parking also covers an author who has not answered;
- an author can always close one of their own pull requests to free a slot;
- a "trigger" is defined: the event or date that makes an `option` worth
  deciding, or an issue worth doing.

The review stopped there: no round found a blocker or a major finding.
