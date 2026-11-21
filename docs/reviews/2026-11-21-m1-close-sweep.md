<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: the documentation sweep at M1's exit

## The change

Class P.

- M1's exit check read the repository's public descriptions against
  ADR 0004 and found text that still pointed forward to "the
  architecture ADR" or contradicted its decisions. This change fixes
  what contradicts the ADR, with minimal edits, and lists what was
  checked and left alone. No decision changed.
- **Written by** `claude-code:claude-opus-5-5`.
- The sweep is also listed in ADR 0004's Review section.

## What changed

| File | Change | Why |
|---|---|---|
| `docs/adr/README.md` | ADR 0004's row says "accepted 2026-11-20; amended 2026-11-20 (#32)" | The row showed only the amendment's date. ADR 0004 landed on 2026-11-20 (8a07bbc, PR #25). |
| `docs/coding-standards.md`, rule 21 | The concurrency item now says blocking code on standard-library threads and no async runtime, citing C3 | It allowed an async runtime "if the architecture ADR shows it is needed". C3, point 5, uses none, neither a third-party one nor one of our own. |
| `docs/workflow.md` §5 | macOS FSKit: "a macOS 27 machine or VM on Apple silicon (ADR 0004, L8)"; one sentence pointing to the matrix (L36 to L39) for release venues | It said "macOS 26/27". L8 sets the floor at macOS 27 on Apple silicon. The Windows line already matches L37 and L38. |
| `docs/research/2026-11-18-project-layout-and-tests.md`, G8 | One dated line: answered by C4, points 3 and 5 | G8 named `ironafs-fskit` and said the ADR decides. |
| `docs/research/2026-11-18-driverless-afs-clients.md`, §3 | One dated line: answered by L4 | It left writeback authorisation to the ADR. |
| `docs/research/2026-11-18-platform-support.md` | Two dated lines: Windows 10's tier (L38, point 2), and where the open questions are decided (C5, C33, L2, L7, L8, L15, L36 to L39, L42) | Both pointed forward to the ADR. |
| `docs/adr/0004-architecture.md`, Review | An entry for this sweep | Its README asks for a sweep to be listed there. |
| `docs/reviews/README.md` | Rows for this record, the M2+ plan's record, and the two records of 2026-11-21 that had none | The index lists every record. |

The research notes are dated records. Their text is unchanged apart from
the added lines, each marked "Update, 2026-11-21".

## Left to another pull request

- **Coding standards rule 20, "Per-OS crates and platform code".** It
  still says "proposed; the architecture ADR decides" and names
  `ironafs-fskit`, which C4 does not have (point 5: the macOS extension
  is Swift, with no Rust). A replacement for the whole paragraph is
  already drafted. It lands with the layout page, after the C4 Working
  amendment (#53, plan id M2a-69) and the dependency checks it cites
  (#46, plan id M2a-13). Editing the paragraph here would collide with
  that text, so it stays until then.

## Checked and unchanged

- `README.md`: brought in line by #36 and #35.
- `AGENTS.md`: says SMB on Windows and `docs/specs/` as planned; agrees.
- `CONTRIBUTING.md`, `AI.md`, `SECURITY.md`: state no architecture
  decision.
- `docs/workflow.md` §2 and §6: brought in line by #39 and #37.
- The skills in `.claude/skills/`: none states an architecture decision.
  `rust-review` keeps its checklist for async code, which applies only if
  such code appears; C3 means none is planned.
- ADR 0001's "the architecture ADR is written on that basis" and
  ADR 0003's mention of it: dated decisions, left as written.
- The dated research notes' other text and the review records: dated,
  left as written.

## Not in this change

- ADR 0001 asks each platform's ADR to check every linked operating
  system component against GPLv3's System Libraries definition. ADR 0004
  does not. The check is planned for ADR 0005 (`ironafs-os`, #43), which
  names those components.
- `docs/adr/README.md` lists ADR 0003 as "proposed", and its own status
  line says the same, though it landed in 796ab3f (#10). That is an ADR
  status question, outside this sweep.

## Review

UNKNOWN until the review runs. This class P change needs one non-author
review; its seat and result are added here before it lands.
