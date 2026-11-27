<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# XDR fixture coverage split

Class I, test-only. The code base is
`d0e1df4c252f3a60bfe5bf8408b5837e5a61c58f`, the signed code head is
`a04d0c749e20078d54cbf933fde14977fb747fd7`, and the code tree is
`d882641393155fe8ab829ccc230dd0eb6a6ecb0a`. The change touches no
production code, policy, dependencies or assets.

## Review

| Seat | Model, effort and route | Result |
| --- | --- | --- |
| Scope advisory | codex `gpt-6.2-sol` high, OpenAI Responses API | READY |
| Scope advisory | glm `glm-5.4` high, Z.ai Coding Plan | READY |
| Implementer | codex `gpt-6.2-sol` medium, codex CLI | Complete |
| First independent code reviewer | glm `glm-5.4` high, Z.ai Coding Plan | PASS, zero findings |
| Code advisory | codex `gpt-6.2-sol` high, OpenAI Responses API | Requested exact private helper source and closed that evidence gap on follow-up |

Both scope advisories checked the exact proposal before code was written.
The owner's scope approval was conditional on those two checks. The
required independent code review covered the exact range above and
concluded in the same GLM session after the initial turn limit. The
controller checked the exact review marker and served model. Quota headroom
was unreported. GLM inspected the source read-only. The controller ran all
compile and execution checks. For the same-family code advisory, the
controller supplied the exact private helper source unchanged and checked
focused executions. Only the GLM code review counts toward the Class I
independent review requirement.

## Consistency

The canonical `Shaded` fixture moved byte-for-byte from the unit-test
directory to [tests/it/shaded.rs](../../crates/ironafs-xdr/tests/it/shaded.rs).
The [integration suite](../../crates/ironafs-xdr/tests/it/main.rs) and the
[excluded fuzz binary](../../fuzz/fuzz_targets/ironafs-xdr_decode.rs) use
that one source.

The [unit tests](../../crates/ironafs-xdr/src/encode/tests.rs) retain their
helpers, phase and wipe checks, pointer and capacity assertions, and
existing buffer cases. The local `RefusedElement` follows the sizing fixture
style and uses the existing `Sizer::check` path. The local `Array` keeps the
same sizing and encoding pattern, including the real `Sizer::array` call.
Two titles now name the representative field and element refusals.

The named [golden test](../../crates/ironafs-xdr/tests/it/golden.rs)
exercises the real invalid arm directly and inside an array with a 3-byte
cap through `encode`, `encoded_len` and `encode_into`. Its local `Shades`
fixture follows the adjacent `Classified` and `MessageType` style and
implements sizing and encoding. The local `check` and `refuses` helpers
use `track_caller` and share the exact public error and buffer assertions.
Both cases include the new 32-byte prefill.

## Evidence

- **EXECUTED.** On arm64 macOS at the exact code tree above, the workspace
  aggregate exited zero with 1,070 Nextest tests, doctests, repository
  policy checks and self-tests, REUSE and cargo-deny. All 98 XDR unit tests
  passed, including the two renamed tests, and all 56 integration tests
  passed by exact name. All eight stable XDR seed replays passed.
- **EXECUTED.** At the same tree, XDR Clippy passed with warnings denied
  for arm64 and x86_64 on Linux, macOS and Windows. Formatting for the
  excluded fuzz crate and build and lint for the XDR fuzz target passed.
  Resolved Cargo metadata matched the base in both workspaces.
- **EXECUTED.** For compatibility with #130, the controller first
  reproduced its original XDR dependency-check failure. After combining
  #130 with this test-only change, the local disposable tree
  `1255a9c0586e64e319fd9b94e03f3eca91f65205` passed `check-deps` and
  `check-pure` with the guard byte-identical. The combined aggregate
  passed 1,141 tests plus the checks. #130 retains its separate Class S
  review and owner gate.

Written by codex:gpt-6.2-sol for the maintainer.
