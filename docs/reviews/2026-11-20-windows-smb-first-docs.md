<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: Windows is SMB first, Cloud Files a later companion

Issue #21, class P. The public description (`README.md`, `AGENTS.md`,
`docs/coding-standards.md`, `docs/workflow.md`, the `rust-review` skill)
now matches the owner's Windows decision of 2026-11-19, to be recorded in
ADR 0004 (#20): an online `\\afs` path through Windows' own SMB client to a
local SMB 3.1.1 server of IronAFS's own, behind a gate, with a Cloud Files
replica later as an optional offline companion and the fallback. The
`area:smb` label was created with the change. Written by
`claude-code:claude-opus-5-5`.

| Round | Range | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|
| 1, review | `dda1473..90ec16a` | GPT read-only | gpt-6.2-sol | high | FAIL: 1 major |
| 2, fix verification | `90ec16a..c32dc47` | GPT read-only, same session | gpt-6.2-sol | high | PASS |
| 3, final-state check | `dda1473..4cb25d5` | GPT read-only, same session | gpt-6.2-sol | high | PASS |
| 3, second family | `dda1473..4cb25d5` | DeepSeek (peak window, owner's permission) | deepseek-flash | max | PASS |

- **Major, fixed (`c32dc47`):** the introduction said a local SMB server
  "can present" `\\afs` and called it a supported way, while only the
  transport was measured. It now says the path is planned, behind its gate,
  with only the transport measured.

- **From the second family's notes, fixed:** the README now names the
  measured Windows 11 versions (24H2 and 26H2) instead of "Windows 11".

The review stopped there: no blocker or major finding remains.
