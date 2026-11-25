<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review of injected benchmark timing (#155)

Class I. This change supplies the timing module in
[testing §5.6, point 2](../specs/testing.md#56-benchmarks). The benchmark
harness in #38 uses it to measure one operation. This change is part of
#155.

## Evidence

- EXECUTED at `cd5aef9` on arm64 macOS with the new tests and the old
  library. The focused integration test run exited 101 because
  `measure_awake` and `TimingError` were absent.
- EXECUTED after implementation at `7bd1f49` on the same host. All four
  focused tests passed. They proved exact 125 ns and 7 ms elapsed times,
  identical results from a repeated seed, one call per operation, and
  exclusion of suspend. Controls passed for a backward reading and for
  failed start and end reads. The package passed 203 tests and 14 doctests.
- EXECUTED. `cargo xtask check` passed on the implemented source, including
  999 workspace tests, doctests, formatting, lints and the repository
  checks. There is no timed benchmark invocation in these tests.

## Review

GLM 5.4 High reviewed `cd5aef9..7bd1f49` and passed with no findings.
The completed envelope matched the served model and check code. The
review inspected the code and focused test cases. The execution evidence
above came from the implementer and controller. Scope was the four
changed files and the public clock and elapsed-time interfaces.

The consistency pass checked `TimingError::fmt`, `TimingError::source`,
`measure_awake`, the seeded record helper and the four integration
cases. The error follows the adjacent wrapped-error pattern. The timing
function uses `Clock::awake` and `AwakeInstant::checked_duration_since`.
The tests use the existing `SeededClock` and its failure and suspend
controls without a second clock implementation. There are no departures
from the existing layout or clock rules.
