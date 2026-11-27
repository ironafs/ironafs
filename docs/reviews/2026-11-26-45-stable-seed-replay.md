<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Stable seed replay

Class I, part of [#45](https://github.com/ironafs/ironafs/issues/45).
Review range `2285fc45b604119616c4bcc11d31b954febeac30..a40687734652a28839619c1494e3c2a8e87db502`. Patch
SHA-256 `ba336f18af46f93e706d5c905459793a6b35859a7a3d324d3e53825ba83259f9`.

| Seat | Model and effort | Result | Quota headroom |
| --- | --- | --- | --- |
| Implementer | codex `gpt-6.2-sol` medium | Complete | Not reported |
| First independent reviewer | gemini `gemini-4-carbon-medium` | PASS, zero findings | Not reported |

The GLM Coding Plan launcher stopped at its credential precondition. The
first-review fallback excludes the author family. The agy receipt attests
the requested Gemini model pin and terminal check code. The envelope omits
a separate served-model field.

## Consistency

The controller compared every moved assertion with its previous wrapper.
Config uses its existing invariant checker and repeats the parse. XDR shares
its traced decode helper at the original budget and retains all 17 type
dispatches. Fdpass keeps the modeled receive, stand-in adoption and drop
checks, with an equivalent parity expression used for stable Clippy. Each
wrapper and each new stable case calls the same per-input helper.

The stable cases read the canonical fixture paths from the source checkout.
The fdpass cases use file reads because the OS source guard rejects file
embedding. Miri keeps the existing isolated fdpass model and Linux wrapper
stand-ins. The new file-dependent module runs in the ordinary stable suite.

## Evidence at the reviewed implementation

- **EXECUTED.** On arm64 macOS with Rust 1.100.0, ordinary nextest discovered
  and passed one unignored test for each of eight XDR, eight config and
  fourteen fdpass seeds. The independent comparison used the existing
  fuzz inventory admission, the tracked blob for each path, and the source
  test mapping. All 1,070 workspace tests and 46 doctests passed.
- **EXECUTED.** Against the unmodified base, the replay selection was empty.
  With one registration removed, ordinary nextest reported 29 successes
  and the exact coverage audit failed. A swapped mapping also failed with
  equal path and test counts. Two temporary test-only controls verified that
  a panic and a changed second XDR result fail the affected replay case.
  All control edits were restored.
- **EXECUTED.** Format checks, REUSE, host and cross-target workspace Clippy,
  pinned-nightly fuzz-wrapper Clippy and source guards passed. Isolated
  Miri passed 14 original fdpass cases on macOS and 24 on the emulated
  aarch64 Linux target.

Written by codex:gpt-6.2-sol.
