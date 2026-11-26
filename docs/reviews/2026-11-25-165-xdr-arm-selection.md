<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# XDR default-arm selection specification (#165)

Class S specification change. [The XDR profile](../specs/xdr.md#55-encoding)
checks a default arm's discriminant during sizing, before reservation or
message writes. Encode and decode select the arm by the same rule.
An encoder refuses a default-arm value whose discriminant selects an
explicit arm. The error names only the discriminant field.

## Review

The first round covered `a8fb807e246e8539e0a412ef6a1635e4418d6774` through
`5315fc53d109f1f885375450c1058fe54b6074e8`, patch SHA-256
`de767be39ed777cf6f530aa091beb2427cc001562c97fc2b7b558a8044434be2`.
The second round covered only the corrections through
`c4551bea9f6f4b704b693c3c9054c421948efa5b`. The controller verified the
initial and scoped pins, completion records, and original check codes.
The lanes did not report quota headroom.

| Round | Seat | Model and effort | Verdict |
| --- | --- | --- | --- |
| 1 | Design | `glm:glm-5.4`, high | PASS, no findings |
| 1 | Independent design | `gemini:gemini-4-carbon-high`, high, uncalibrated on IronAFS | PASS, no findings |
| 1 | Security | `gemini:gemini-4-carbon-medium`, medium | FAIL, one major and two minors |
| 2 | Security fix verification | `gemini:gemini-4-carbon-medium`, medium | PASS, no findings |

- **Refusal symmetry and P4, major** (`origin: pre-existing`,
  `kind: contract`). The draft weakened the shared refusal rule and
  omitted P4's smallest-case, one-guard, and representability conditions.
  The correction restores the rule and the P4 conditions, keeps the
  32-bit sizing limit, and puts the additional encode-only refusal in P1.
- **Allocation guarantee, minor** (`origin: pre-existing`,
  `kind: contract`). The draft banned the output reservation during sizing
  but failed to exclude other allocator requests. The correction puts
  every allocator request, reservation call, and message write after
  successful sizing. The initial best-effort wipe remains first.
- **Capacity controls, minor** (`origin: pre-existing`,
  `kind: contract`). The draft pointed to standalone sizing for an order
  exercised by both encode entry points and left the test buffer's
  capacity unspecified. The correction requires both entry points and
  caller buffers below and at or above the 12-byte candidate size, plus
  a fresh zero-capacity buffer.

The controller checked the reciprocal #168 criterion, the validation
and arm-selection rules for the discriminant, and the P1 and P4 fixture
obligations. The design reviewers did not inspect every unchanged range.
The security verification left the instrumentation of the owned-buffer
route for the implementation evidence below.

## Evidence

- INSPECTED sections 5.1, 5.4, 5.5, 7 and 10 of the XDR profile and the
  reciprocal ordering criterion in #168. Section 5.5 uses the existing
  sizing pass, refusal list, and buffer order. #168 places the check in
  the global sizing precedence.
- EXECUTED an independent big-endian word calculation for G14 and G15.
  Both vectors matched the fixture and were 12 bytes. A default-arm
  value carrying G14's discriminant and payload would produce G14's
  explicit-arm bytes if the encoder skipped the mismatch check.
- EXECUTED `cargo xtask check` on macOS at `c4551be`. The 995 existing
  workspace tests and the repository checks passed.
- PROPOSED under #59. Run `cargo nextest run -p ironafs-xdr` with the G14
  and G15 controls and the invalid equal-width value. Assert the
  field-only error and the specified buffer outcomes. Check the shared
  private encode path for reservation and write order, following the
  existing encoder buffer tests. The implementation review also checks
  the open-discriminant representation and case selection.
