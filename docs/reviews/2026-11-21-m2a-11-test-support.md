<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: `ironafs-test-support`, the seeded generator and shrinker

## The change

Class I. Issue #44 (plan id M2a-11).

- A new crate, `crates/ironafs-test-support`, a development dependency
  only: IronAFS's own seeded generator (SplitMix64, written from its
  published description and checked against its reference outputs), a
  `Source` whose draws record a choice sequence, a shrinker over that
  sequence, and a `Property` runner that prints the seed and a replay line.
- Later commits on the branch add boundary values, more draws (`option_of`,
  `weighted`, `i64_in`, strings), element spans so collections of records
  shrink to the minimum, `bulk_bytes` (a seed and a length for multi-MiB
  inputs), `Source::from_bytes` for fuzz input, golden tests of the
  seed-to-value mapping, a shrink time limit, the seed printed before
  shrinking, and failures printed as pasteable regression tests.
- Two named departures, each stated in the code and in its commit:
  - coding standards rule 20: the crate exists before a second crate needs
    it, because the briefs that use it are planned now (`3c18dd0`);
  - coding standards rule 7: the shrink time limit reads the real clock,
    once, in `Property`'s run loop; the tests use a fake clock (`fedf04d`).
- Not in this change, and not reviewed here: #44's exhaustive sweep for
  small domains and its wire-message helpers.
- **Written by** `claude-code:claude-opus-5-5` (Claude Code subagents,
  test first).

## Rounds

All rounds ran on 2026-11-21 (UTC). The GLM rounds ran read-only, without
running the tests; the Claude rounds ran read-only on the branch, with
probe tests in a scratch copy outside the repository.

The required seat (class I, Claude-written code): GLM, the first reviewer.
Its rounds cover the whole branch, `1f44f21..3d067f6`, with no gap.

| Round | Range | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|
| 1, review | `1f44f21..35fdc2b` | glm | glm-5.3 | high | PASS: 2 minor |
| 2, fix review | `35fdc2b..a12f79a` | glm | glm-5.3 | high | PASS: no findings |
| 3, fix review | `a12f79a..8b96228` | glm | glm-5.3 | high | PASS: no findings |
| 4, review of the follow-up | `8b96228..e0b8892` | glm | glm-5.3 | high | PASS: 1 minor |
| 5, fix verification | `e0b8892..3d067f6` | glm | glm-5.3 | high | PASS: no findings |

An extra same-family self-check (Claude, not a required seat, and not
credited in the trailers): Claude Mythos 5.2, as a Claude Code subagent.
Its effort was not recorded.

| Round | Range | Seat | Served model | Verdict |
|---|---|---|---|---|
| 1, review | `1f44f21..35fdc2b` | Claude, extra | claude-mythos-5-2 | no blocker; 3 major, 6 minor |
| 2, fix verification | `35fdc2b..a12f79a` | Claude, extra | claude-mythos-5-2 | FAIL: 1 major left (a residue of its first major), 1 minor |
| 3, fix verification | `a12f79a..8b96228` | Claude, extra | claude-mythos-5-2 | PASS: 2 minor |

The self-check did not review the follow-up (`8b96228..3d067f6`); GLM did.

## Findings

GLM, round 1:

- **Minor, fixed (`3c18dd0`):** the shrinker's documentation said binary
  search finds the smallest failing value; for a property whose failures
  are not monotone it finds a threshold. The shrinker now tries 0 to 3 one
  by one before searching, and the documentation says what the search
  finds.
- **Minor, fixed (`a12f79a`):** `cases(0)` passed without running a case.
  It is now rejected.

GLM, round 4:

- **Minor, fixed (`3d067f6`):** in the byte-input property test, the
  assertion on `pick` held for every item of the slice, so it could not
  fail. The test now checks that the picked value is one of the items and
  that every item is reached; making `pick` always return the first item
  fails it. GLM verified the fix in round 5.

Claude self-check, round 1:

- **Major, fixed (`3c18dd0`, then `8b96228`):** a collection followed by
  other draws did not shrink to the minimum, because deleting an element's
  choices shifted every later draw. `vec_of` now records a "one more"
  choice before each element. Round 2 found the residue: an element of 3,
  5 or more choices still did not delete cleanly with fixed chunk sizes.
  Each element now records its span, and a shrink pass deletes whole
  spans; round 3 confirmed records of 2, 4 and 10 fields and nested
  vectors reach the minimum on 30 of 30 seeds.
- **Major, fixed (`a12f79a`):** a failure report dropped the panic
  location. The quiet panic hook records it, and `check` runs the minimal
  input once more with its panic shown.
- **Major, fixed (`a12f79a`):** the crate's own tests failed when
  `IRONAFS_TEST_SEED` was set. Seed-specific assertions now ignore the
  variable.
- **Minors, fixed (`c6738be`, `a12f79a`, `8b96228`):** an empty seed
  variable panicked (now unset) and a non-Unicode one was ignored (now
  rejected); draws were purely uniform (one in 16 is now a boundary
  value); errors printed differently from panics (both use `Display`, and
  `check_described` takes a description); silencing panics is per thread
  (documented); `cases(0)` (as GLM's finding); missing draws and
  constructors (added, the string draw in `8b96228`).

Claude self-check, round 2:

- **Minor, accepted as documented:** for a property such as "`i32` at
  least 10 and even", the result was 12 or 14 on most seeds, not 10. The
  shrinker finds a local minimum; the public documentation says so, with
  a tested example (`e0b8892`).

Claude self-check, round 3:

- **Minor, fixed (`38812d1`):** the fresh string length target counted
  characters against a byte cap, and about half the characters were not
  ASCII although the documentation said "mostly ASCII". The target is now
  in bytes, and three draws in four are ASCII.
- **Minor, fixed (`e0b8892`):** the local-minimum case is now stated in
  the public documentation.

The review stopped there: no blocker or major finding remains, and each
minor finding is fixed or recorded above.

## Evidence

- The implementer's runs on each commit: fmt, clippy, the crate's tests
  (also with `IRONAFS_TEST_SEED` set to 5 and set empty), doc-tests,
  `reuse lint` and `cargo xtask check` passed (EXECUTED by the
  implementer, macOS, 2026-11-21 UTC, as reported; not re-run for this
  record).
- The large-input property, 32 cases of up to 8 MiB through `bulk_bytes`,
  took 2.26 to 2.50 s in a debug build on the development Mac, with and
  without `IRONAFS_TEST_SEED` set (EXECUTED by the implementer,
  2026-11-20 local time; recorded in the test's doc comment).
- At `3d067f6`, before this record was added: fmt, clippy, nextest,
  doc-tests, `reuse lint`, `cargo deny check` and `cargo xtask check`
  (EXECUTED, macOS; the results are in the pull request).
