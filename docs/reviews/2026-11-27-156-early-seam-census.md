<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# #156 early seam census

Class I. The code base is `a460cc454d6b2b9107b0dd67c7483176b71bbf8d`.
The signed code head is `b06cc8036994019f208bfb5935984cfef5f91ad0`
and its tree is `7c67f7014e82481ba4cca887f261d7f32d1f44a2`.

`cargo xtask check-seams` lists the four seam declarations, identified
written implementations and unresolved source. Rows include the package,
Cargo target, lexical scope, source location and written configuration
predicates. The inventory surveys written Rust in Cargo targets and their
modules. It carries predicates as source text and reports its incomplete
status. Macro expansion, doctest code blocks and compiled target coverage
lie outside this inventory. Conformance bodies, fault runs and the fault
feature remain open in [#156](https://github.com/ironafs/ironafs/issues/156).

## Review

| Seat | Served model and route | Scope and result |
| --- | --- | --- |
| Implementer | codex `gpt-6.2-sol`, codex CLI | Early census, tests and scoped fixes |
| Earlier independent attempt | gemini `gemini-4-carbon-medium`, Antigravity | Inconclusive on predecessor `e015e90`. The response omitted the exact source pin and completion marker. Its observations informed controller checks. |
| Census and fix advisories | codex `gpt-6.2-sol` high, OpenAI Responses API | Census triage, qualified-function and module-ancestry corrections, missing-file behavior, and the scope and draft of the explicit-root fix. Same-family advisories. |
| Source review and fix verification | glm `glm-5.4` high, Z.ai Coding Plan | Required independent Class I seat. The verified rounds below carry the finding dispositions. |

The GLM pin has [IronAFS code-review calibration](2026-11-24-glm54-pin.md).
The GPT advisory pin is uncalibrated on IronAFS for these advisories.
Quota headroom was unreported. The model seats read source. The controller
ran the checks and verified each served pin and verdict receipt.

| Round | Source pin | Result |
| --- | --- | --- |
| 1, source | `a460cc4..9622cd2` | PASS, two minors. The reviewer identified unread context. |
| 2, alias fix and contract disposition | `9622cd2..5568f8d` | PASS, one new minor. A same-session continuation supplied the exact completion marker and preserved the verdict. |
| 3, block-scope test | `5568f8d..2864e5e` | PASS, zero findings in the test-only diff. |
| Source context follow-up | `a460cc4..2864e5e` | PASS, one pre-existing minor. The same source reviewer completed named reads and recorded the remaining contextual reading limits. |
| 4, approved explicit-root fix | `2864e5e..b06cc80` | PASS, zero findings in the exact two-file correction. |

## Findings and convergence

- **Pre-existing, correctness.** The earlier inconclusive response raised
  lost `cfg` labels on qualified functions and incorrect `super` resolution
  in a module inside a lexical block. The controller reproduced both and
  fixed them before `9622cd2`, with fail-before and pass-after regressions.
  Other observations about syntax beyond the tested cases and exact header
  display remain limits of the early source inventory.
- **Pre-existing, correctness.** Bare dependency imports and direct crate
  aliases produced false unresolved rows. `5568f8d` corrected both and
  pinned the expected implementation rows.
- **Pre-existing, contract.** Rust accepts a missing external module behind
  a disabled configuration. The early census contract requires a nonzero
  result and a source diagnostic for a missing declared module. The
  controller reproduced the hard error with and without a predicate and
  confirmed that restoration recovered the original report. GLM agreed
  that the existing behavior satisfies the contract. Closed as expected
  behavior for this written-source scope.
- **Introduced by fix round 1, other (test coverage).** The import test
  omitted the new lexical-block path. `2864e5e` added a case that failed
  before the import fix and passed with it. Its assertion pins the written
  implementation row and scope. An enclosing scope with a conflicting
  import is outside this individual test case.
- **Pre-existing, correctness.** An ordinary direct dependency-root
  import such as `use ::ironafs_time::Clock` could produce a false
  unresolved row. The controller confirmed the Rust syntax and reproduced
  the census failure. `b06cc80` fixes this form and tests it with a
  same-named local module. Grouped, renamed and conditional explicit
  imports retain their conservative handling.

The initial GLM review found two minors and no majors or blockers. The
first fix verification left one new coverage finding. The second closed
that finding with zero new findings. The source-context follow-up found one
older minor in the same import resolver. The controller checked that the
open findings had narrowed to this one case and recommended one additional
round limited to a census-local correction and regression. On 2026-11-27
the owner approved one explicit crate-root import fix at `2864e5e`. The
final review closed the finding with zero new findings.

## Consistency and evidence

- **INSPECTED.** The census reuses the workspace metadata, source tokens,
  import classification and seam names. The final explicit-root helper
  accepts the shape of an ordinary import. Its token grammar excludes
  generic argument lists accepted by the implementation-header reader.
  It feeds the existing import binding and dependency lookup.
- **EXECUTED.** On arm64 macOS with Rust 1.100.0, the explicit-root
  regression failed at `2864e5e` and passed at `b06cc80`. The final
  `cargo xtask check` passed with 1,078 workspace tests, doctests, Clippy,
  repository checks and self-tests, REUSE lint and cargo-deny.
- **EXECUTED.** At the same code tree, the live workspace report matched
  the preceding candidate byte for byte. It listed four declarations,
  five implementation rows and 128 unresolved diagnostics. Later
  conformance work must reconcile those gaps with the compiled evidence.

Written by codex:gpt-6.2-sol for the maintainer.
