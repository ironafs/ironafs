<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review of the M2a Working amendments (#53)

Class S, documents only. The [ADR 0004 review entry](../adr/0004-architecture.md#review)
records the original decisions, first two rounds, and supersession sweep.
The [owner's November 21 decision](https://github.com/ironafs/ironafs/issues/53#issuecomment-7096109906)
settled the remaining Rule 21 question. The change adds the one dated
pointer to C5, point 1.

| Round and revision | Seat | Effort | Verdict |
|---|---|---|---|
| 1 at `3ba1a4c` | glm:glm-5.3 | high | PASS, four minors |
| 1 at `3ba1a4c` | deepseek:deepseek-flash (uncalibrated on IronAFS) | max | FAIL, one blocker, four majors, four minors |
| 2 at `b3c0c85` | deepseek:deepseek-flash (uncalibrated on IronAFS) | max | FAIL, one blocker and four minors |
| 3 on `7873bac..22291d5` | glm:glm-5.4 | max | PASS, zero findings |
| 3 on `7873bac..22291d5` | deepseek:deepseek-flash (uncalibrated on IronAFS) | max | PASS, zero findings, same-session continuation |
| 3 on `7873bac..22291d5` | gemini:gemini-4-carbon-medium | medium | PASS, two editorial findings |
| Editorial verification on `22291d5..d18aedd` | gemini:gemini-4-carbon-medium | medium | PASS, zero findings |
| C34 citation edit on `b1c7556`, patch `da0a4782` | gemini:gemini-4-carbon-medium | medium | PASS, zero findings, same-session continuation |

Quota headroom was unreported for the final dispatches. The Round 3 and
editorial receipts matched the requested scope, served models and check
codes. DeepSeek's first turn reached its turn cap. The continuation
retained the original session and check code.

The C34 citation in `config-schema.md` now cites the owner’s dated
decision without describing the amendment’s merge status. The reviewer
verified the citation edit. The controller checked the date in ADR
0004’s review entry and the rule in issue #53, point 4.

## Findings and convergence

The ADR review entry names the earlier findings and dispositions. The
Rule 21 blocker was a contract question present in Round 1 and carried
over to Round 2. The owner resolved it with the dated pointer. DeepSeek's
blocker and major counts ran from one and four, to one and zero, to
zero and zero. GLM and the security seat finished with zero blockers
and zero majors. The security seat's two Round 3 findings concerned the
now-stale wording about the owner's decision and the sweep date
(`origin: introduced by fix round 2`, `kind: other`). The editorial
verification confirmed both were addressed. There are zero open
confirmed correctness or contract findings.

GLM and DeepSeek both asked whether C5, point 1, narrows the schema
digest input to control-only messages. C44, point 1, specifies one
definition set. The [IPC specification](../specs/local-ipc.md) §5.6.1,
point 7, defines exactly the canonical bytes over that whole set and
requires both endpoints to carry the same digest.

## Evidence

- INSPECTED. At `7873bac`, ADR 0005 retains the C4, point 3 row and
  C5, point 5. ADR 0006 retains C30 through C32, and main's C13, point 2
  correction is present. The branch's ADR 0005 matches main byte for
  byte. The November 25 issue mapping and allowlist recheck is in the
  ADR review entry.
- EXECUTED. On arm64 macOS, format, Clippy and `cargo xtask check`
  passed at `22291d5`, with 1,006 workspace tests. After the review
  history correction, `reuse lint` and `git diff --check` passed.
