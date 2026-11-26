<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review of checked XDR sizing and default-arm refusal (#59)

Class S. The contracts are the [arm-selection amendment](2026-11-25-165-xdr-arm-selection.md)
and the [sizing-order amendment](2026-11-25-168-xdr-sizing-order.md).
They were reviewed and landed before this implementation began. The
[closed-enum fixtures](2026-11-25-59-shaded-fixtures.md) supply the
independent valid vectors and the undeclared-discriminant control.

| Stage and range | Seat | Effort | Verdict |
|---|---|---|---|
| Implementation, `3248a92..d452bc7` | glm:glm-5.4 | high | PASS, zero findings |
| Independent wire review, `3248a92..d452bc7` | gemini:gemini-4-carbon-high (uncalibrated on IronAFS) | high | PASS, zero findings |
| Security review, `3248a92..d452bc7` | gemini:gemini-4-carbon-medium | medium | FAIL, one major |
| Scoped security verification, `d452bc7..245e510` | gemini:gemini-4-carbon-medium | medium | PASS, zero findings |

The review receipts matched the served models, pinned changes and
check codes. Quota headroom was unreported. The scoped verification
used the exact two-file diff and was limited to the test observation.

## Security finding

- **Caller-buffer wipe coverage**, major, `origin: pre-existing`,
  `kind: contract`. The failure-path tests checked length, address and
  capacity. They left the initialized bytes unobserved. On a disposable
  source copy at `d452bc7`, removing the overwrite and retaining
  `clear` still passed all 98 XDR unit tests. The independent judgement
  attempt at the Mythos 5.2 pin timed out without a verdict. The
  correction at `245e510` observes the actual initialized bytes
  immediately before clearing. It covers the populated buffer after
  sizing refusal and the freshly allocated buffer after a forwarded
  write error. Both known cases fail on the same disposable
  clear-only change and pass with the overwrite present. The
  originating security seat verified the fix. The corrected tests
  establish the coverage directly, so the judgement is moot under
  workflow §2.

## Evidence

- EXECUTED on arm64 macOS. Before the implementation, three new
  regression cases compiled and failed. Both encoder entry points
  accepted an inconsistent default arm, a length-word failure masked
  its declared bound, and a later item masked an earlier enclosing cap
  refusal. All three cases passed after the implementation.
- EXECUTED. The independent 12-byte vectors, G13, R9b, seeded
  properties and small-stack controls passed. The XDR package passed
  144 tests and 13 doctests. Sizing refusal reached neither reservation
  nor writing. The scoped wipe change passed all 98 unit tests and
  detected both failures on the clear-only plant.
- EXECUTED at `245e510`. `cargo xtask check` passed, including 1,024
  workspace tests, doctests, format, workspace Clippy and repository
  checks. Local replay passed all 28 tracked seeds on the implementation
  before the test-only fix. The fixture and seed bytes are identical.

## Consistency

The shared sizing visit reuses the existing error and field metadata,
padding calculations and checked write accounting. Its private
lexical guards restore depth on error and unwind. The caller-buffer
entry point preserves the wipe, sizing, reserve and write order.
The standalone sizing entry point uses the same visit and leaves
platform-width admission to encoding. The observe hooks compile only
for unit tests, which use the existing `Property` runner for the
caught-unwind control.
