<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# #173 seam self-test fixture errors

Class I. Base `94f8d5b93f1f44f12b5b2ca0035bf4f178ca86f0`. Signed source `42e51d1ac28fc4ca085e7f0af6df92a7b4eaae3c`, tree `8c16c4d461dbd3e534475e8cb339b2eb97f03084`. The change covers `crates/xtask/src/seams/self_test.rs`.

The fixture builders retain the existing source-reader result for each fixture. Before the controls, plants or recorded limits reach the seam checks, the owning case reports any unreadable fixture through the existing self-test failure. The diagnostic gives the fixture path and the lexer reason. The separate plant for an expected lexer error still exercises the parser directly. The production workspace walk and lexer are outside the diff.

## Independent review

| Seat | Served model | Effort | Route | Result |
|---|---|---|---|---|
| Implementer | `claude-opus-5-5` | High | Anthropic Console through Claude Code | One-file change |
| First independent source | `gpt-6.2-sol` | High | OpenAI Responses API | PASS, no findings |
| Second independent source | `glm-5.4` | High | Z.ai Coding Plan | PASS |
| Supplemental independent source | `deepseek-flash` | Default | DeepSeek API through headless Claude Code | PASS, no findings |

The source seats read the same pinned diff. The controller ran the commands and checked the terminal receipts. The full module has three fixture builders that call `source_file`, plus the direct call in the intentional unreadable-source plant. Headroom was unavailable. GPT-6.2 Sol High is uncalibrated on IronAFS for this seat.

## Executed evidence

The controller gave only the `src/random.rs` pure-control fixture an unterminated string in each isolated check. The original fixture literals match across the source revisions and the temporary replacement was the same. Before the fix, the command exited 0 and reported the pure-crate control passed. On signed source it exited 1 under that control with `src/random.rs: not readable as Rust: line 1: unterminated string`. After restoration the clean self-test exited 0 and the intentional unreadable-input plant still passed. All three checks used `cargo --locked xtask check-seams --self-test`.

On signed source, the locked workspace build, formatting, Clippy with warnings denied and `cargo xtask check` passed. Workspace Nextest passed 1,189 tests and 57 doctests passed. The final review record and its index entry follow the reviewed source.
