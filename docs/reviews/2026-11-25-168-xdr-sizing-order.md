<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# XDR sizing error order specification (#168)

Class S specification change. [The XDR profile](../specs/xdr.md#55-encoding)
gives both encode entry points the same sizing visit. It orders field
checks, array caps and whole-message admission, and defines the
standalone size query and the first-error rule.

## Review

The controller checked the scoped and cumulative patches, the served
models, and the completion records for the pinned revisions.

| Round | Range | Scope SHA-256 |
| --- | --- | --- |
| 1 | `2c1fda3d86c3e6b518493be521d6d1a5d8ee52fe..2d067350fbc9ddf2b14a903fe03194b6e45891e7` | `58819f9f8ded9c7015bd7ee517af4581e40ac972c482c986349c21271a2a84d3` |
| 2 | `2d067350fbc9ddf2b14a903fe03194b6e45891e7..2cf1506cfa6de4cfbe420b7506d58ca59c57f942` | `5fad5ac885184f07c8a91e2def8ef3635d02eea748991dd94687725f7bdadaf7` |
| 3 | `2cf1506cfa6de4cfbe420b7506d58ca59c57f942..93e5964c61f0ee652d1e027b143be0acc0c89158` | `c22eb505f72035941b32172dfeec11c24e54740a9cedf3a3f92cb0dc31bebc2e` |

| Round | Seat | Model and effort | Verdict |
| --- | --- | --- | --- |
| 1 | Design | `glm:glm-5.4`, high | PASS, one minor |
| 1 | Independent design | `gemini:gemini-4-carbon-high`, high, uncalibrated on IronAFS | Inconclusive, print deadline on the initial turn and continuation |
| 1 | Security | `gemini:gemini-4-carbon-medium`, medium | FAIL, three majors and one minor |
| 2 | Design fix verification | `glm:glm-5.4`, high | PASS, no findings |
| 2 | Independent design | `gemini:gemini-4-carbon-high`, high, uncalibrated on IronAFS | PASS, two minors |
| 2 | Security fix verification | `gemini:gemini-4-carbon-medium`, medium | FAIL, two majors and one minor |
| 2 | Judgement | `glm:glm-5.4`, max | A for both security majors |
| 3 | Independent design fix verification | `gemini:gemini-4-carbon-high`, high, uncalibrated on IronAFS | PASS, no findings |
| 3 | Security fix verification | `gemini:gemini-4-carbon-medium`, medium | PASS, no findings |

### First-round findings

- **Runtime scope and first refusal, major** (origin: pre-existing,
  kind: contract). Define the full-stack refusal, restore cap depth on
  normal and exceptional exits, and retain the first sizing refusal.
- **Sizing fallback and element width, major** (origin: pre-existing,
  kind: contract). Require a context-taking sizing method, allow a
  single size report for an eligible handwritten leaf, and check
  widths through the visit.
- **Phase evidence, major** (origin: pre-existing, kind: contract).
  Observe the sizing, reservation and writing sites in the shared
  path for both public entry points. Check forwarding of the write
  error through both.
- **Nested cap fixture, minor** (origin: pre-existing, kind: contract).
  Put the hyper inside the inner array so its charge crosses both
  caps and the inner cap's name wins.
- **Fixture metadata, minor** (origin: pre-existing, kind: contract).
  Specify the test interface, field-path names, caps, bounds,
  directions and decode budget in section 10.

### Second-round findings and judgement

- **Standalone sizing, major and two design minors** (origin:
  introduced by fix round 1, kind: contract). The security seat's
  position A was that the bridge and standalone paths could disagree
  and that a default query calling through the old bridge could
  recurse. Position B was the draft's structured-codec-only root rule
  and its bridge through the existing leaf `encoded_len`. GLM upheld
  the gap, with the proposed recursion treated as a constraint on the
  correction. The controller checked [5.5 point 2](../specs/xdr.md#55-encoding),
  the existing `Encode` trait, and the Rust rule that trait defaults
  can be overridden. The correction gives every codec the same root
  visit, keeps declared layout limits, separates the leaf report
  from the public query, and reserves message-cap and `usize` admission
  checks for the encode entry points.
- **Guard ownership and latch return, major** (origin: introduced by
  fix round 1, kind: contract). Position A was that the draft allowed a
  caller-owned guard and left earlier refusal precedence ambiguous at
  visit exit. Position B was the draft's scope-guard and latch text.
  GLM upheld both parts. The controller checked the draft's cap and
  visit scopes and the documented behavior of Rust's
  [forget](https://doc.rust-lang.org/core/mem/fn.forget.html#safety) and
  [catch_unwind](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html).
  The context keeps its guards private and returns the first latched
  refusal ahead of any later error, success or zero-width check.
- **Owned-buffer observation, minor** (origin: introduced by fix
  round 1, kind: contract). Observe zero capacity at sizing entry and
  refusal, before the owned buffer drops. Inspect the wrappers, wipe
  and sizing/refusal path for additional allocation sites.

### Convergence

The security seat's blocker counts were 0, 0, 0 and its major counts
were 3, 2, 0. Its open confirmed contract findings were 4, 3, 0.
GLM design cleared its one fixture finding in round 2. The independent
design seat had no round 1 verdict and cleared its two round 2
findings in round 3. The first fix exposed gaps in the sizing API
contract and observation scope. The second fix narrowed those points
and introduced no confirmed finding in verification. There is no open
confirmed correctness or contract finding in the specification.

## Evidence

- INSPECTED the decoder and P4 ranges, the reciprocal #165 default-arm
  rule, the sizing and error sections and section 10 at `93e5964`.
  The default-arm check stays ahead of the discriminant charge.
  Decoder and wipe, reserve and write rules retain their prior text.
- EXECUTED `cargo xtask check` and `cargo clippy --all-targets --workspace`
  on macOS at `93e5964`. The 1,006 existing workspace tests and the
  repository checks passed. Clippy completed without warnings.
- PROPOSED under #59. Run the specified sizing cases on a 64-bit venue
  with `cargo nextest run -p ironafs-xdr`. Use the synthetic size
  reports and field lengths to cover the limit and overflow cases.
  Check both encode entry points, the standalone query, error metadata,
  private scope cleanup, and the caller-buffer and phase observations.
