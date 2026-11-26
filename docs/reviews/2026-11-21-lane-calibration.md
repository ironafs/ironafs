<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: calibrating the review lanes on IronAFS ranges

## What this is

Class I. Issue #48 (plan id M2a-42). Before the first class S code
review, each review lane runs at its pinned model and effort on IronAFS
commit ranges with planted, recorded defects and on a clean range. The
lane must reach the owner's threshold before it takes a required seat
([workflow](../workflow.md) §2, "Changing a pin" and "Uncalibrated
pins"). Nothing in this record lands code. The ranges are throwaway
local branches, never pushed and never merged.

This record covers the first suite of code ranges and two lanes: GLM at
`glm-5.3`, high, and Gemini at `gemini-4-carbon-medium`. Both passed.
What is still to run is listed under "Not calibrated yet".

**Re-check first.** The pins in [workflow](../workflow.md) §2, "Model
routing", and the "Changing a pin" rule were re-read at source before
the suite was built. They agree with the issue's seats and order, so
there was nothing to raise.

## The suite

- **Six code ranges**, each one commit (one range has two) on a local
  branch based on `39c853e`. Five carry planted defects, **12 in all, 6
  of them critical**. The sixth range is clean.

  | Range | Branch | Crate | Planted |
  |---|---|---|---|
  | R1 | `wire-time-conversion` | `ironafs-time` | 2 (1 critical) |
  | R2 | `source-raw-payload` | `ironafs-test-support`, `xtask` | 4 (2 critical) |
  | R3 | `check-keep-going` | `xtask` | 2 (1 critical) |
  | R4 | `seeded-clock-present` | `ironafs-test-support` | 2 (1 critical) |
  | R5 | `secret-bytes` | `ironafs-crypto` (new) | 2 (1 critical) |
  | R6 | `instant-checked-sub` | `ironafs-time` | 0 (clean) |

- **The defects are part of ordinary feature work.** Each range is a
  plausible change with doc comments and tests. The commit messages are
  neutral, and the tests pass with the defect present. Every head passes
  `cargo fmt`, clippy with warnings denied, the crate's tests and doc
  tests, and the `xtask` checks for what it touches. So no defect is
  caught by a tool; a lane has to find it by reading.
- **Planted by Claude** (`claude-code:claude-opus-5-5`), a family other
  than the lanes'. The answer key gives each defect's place, its nature,
  whether it is critical, and what a finding must say to hit it. It also
  lists known valid observations that are not planted.
- **Neutral briefs.** Every range got the same code-review brief, filled
  in with the range, its class and the ordinary question ("is this change
  correct and safe to merge?"). Nothing in a brief describes or hints at
  a defect. The lanes ran from worktrees at paths named after the
  branches. The key and the score sheet stayed outside every lane's
  reach. No GLM transcript refers to them. Gemini had no file access; its
  diff and commit messages were inlined.
- **A document-review set** was built with the suite and is in the same
  key: excerpts of four ADR 0004 part 1 sections, three with planted
  contradictions and missing failure paths (12 defects, 3 critical) and
  one clean. It has not been run yet.
- **A held-out second suite** was written and hashed before any run, for
  the miss rule below. Its contents stay unpublished while it can still
  be used. Neither lane was sent to it.

## Frozen before dispatch

Both answer keys were hashed at 09:18:59 UTC on 2026-11-21. The first
lane run started at 09:19:59 UTC. Each key was checked against its digest
again before scoring, and was unchanged.

| Key | SHA-256 |
|---|---|
| Suite 1 (code ranges and document set) | `f82393cdd0141683bde81422b0596466ca69f1d26b9a3656e2c311b02380d2e0` |
| Suite 2 (held out) | `bbc96119c48d478c842f8cf095beb18d9f7046ba6796b646737c66bde77d607d` |

**The defect list is not published yet.** The issue publishes it only
after the ranges are retired, and suite 1 is still needed for the seats
under "Not calibrated yet". When it is retired, the key is added to this
record, and its digest above lets anyone check it is the key used.

## The pass rule

The owner's threshold, from the issue, for each seat, served model and
effort:

- **Pass:** every critical planted defect found; at least 9 of the 12
  planted defects found; every run finished; no false blocker, and at
  most one false major, over all six ranges.
- **Miss rule** (registered before any run): a lane that misses exactly
  one critical defect gets one run on the held-out suite; anything worse
  fails. No other re-run.
- **Held-out pass rule** (the issue sets none; decided by the controller
  before any run): every critical defect in the held-out suite found, the
  run finished, no false blocker, at most one false major.
- A changed threshold or pin uses fresh held-out cases. A lane below the
  threshold takes no required seat until it is re-pinned.

How the runs were scored:

- **Hit:** a finding that names the defect's place (the file, and the
  construct or a line in or next to it) and its nature (the mechanism, in
  any words). The lane's severity does not matter. One finding hits at
  most one defect, and several findings on one defect count once.
- **False finding:** a finding on any of the six ranges that matches no
  planted defect, is not a listed valid observation, and that the
  maintainer refutes with evidence. It is a false blocker or a false
  major by the lane's own severity. The clean range counts like the
  others.
- **Valid, unplanted:** a finding that matches no planted defect and is
  not refuted. It is recorded, and is neither a hit nor false.
- **Finished:** the lane's did-it-run checks all pass: the served model
  and effort read from the lane's own output, a completion event, the
  per-run check code echoed, output that parses with a verdict, every
  cited file present. A run that fails any of them is void, never a pass.
- **Refusal:** the lane declines all or part of the brief. Neither lane
  could run cargo (GLM's allowlist holds only reading tools and `git
  diff`, `log` and `show`; Gemini had no shell). Each said so in its
  uncertain items. That is the design of the seat, not a refusal.

Scoring was done against the frozen key by a Claude Opus 5.5 agent
working for the controller, on 2026-11-21. Every finding that hit
nothing was checked against the head commit. None was refuted, so no
false finding was counted, and no ruling needed a refutation seat.

## Results

### GLM, `glm-5.3`, high: PASS

Seat: the first reviewer (class I and P required reviews; ordinary class
S reviews at the same pin). Served model and effort, from the lane's own
log: `glm-5.3`, high, on every run. Runs on 2026-11-21, 09:19 to 10:07
UTC. Tools: read-only, as in every GLM review seat.

| Range | Finished | Refusal | Tokens (input / cache read / output) | Time, turns | Findings (B/M/m) | Hits | Missed | False | Valid, unplanted |
|---|---|---|---|---|---|---|---|---|---|
| R1 | yes | none | 36.5k / 292k / 19.6k | 235 s, 25 | 2/0/2 | R1-1\*, R1-2 | none | 0 | 2 |
| R2 | yes | none | 60.5k / 757k / 27.6k | 454 s, 36 | 1/2/2 | R2-1\*, R2-3, R2-4\* | R2-2 | 0 | 2 |
| R3 | yes | none | 37.6k / 205k / 12.4k | 189 s, 25 | 1/1/1 | R3-1\*, R3-2 | none | 0 | 1 |
| R4 | yes | none | 46.3k / 380k / 20.3k | 337 s, 30 | 1/1/1 | R4-1\*, R4-2 | none | 0 | 0 |
| R5 | yes | none | 44.8k / 375k / 16.0k | 302 s, 30 | 1/1/1 | R5-1\*, R5-2 | none | 0 | 1 |
| R6 (clean) | yes | none | 23.1k / 220k / 7.3k | 106 s, 28 | 0/0/0 | (clean) | n/a | 0 | 0 |

(\* critical. Where a range has more findings than hits plus valid
unplanted ones, the rest were further findings on a defect already hit.)

| Measure | Result | Needed |
|---|---|---|
| Critical found | 6 of 6 | 6 |
| Planted found | 11 of 12 | at least 9 |
| Every run finished | yes, 6 of 6 | yes |
| False blockers | 0 | 0 |
| False majors | 0 | at most 1 |
| Refusals | 0 | recorded |
| Cost | 248.8k input, 2,229k cache read, 103.2k output tokens; 1,623 s | recorded |

The lane's own guard stopped the first attempt at R5 before anything was
sent, because the diff touches a file whose name looks like a credential
file. The owner allowed the range, and it was then dispatched once. That was not a re-run: nothing had been sent before.

### Gemini, `gemini-4-carbon-medium`: PASS

The owner made the agy lane available for these seats on 2026-11-21.
Pin on the command line: `gemini-4-carbon-medium`, the routing table's
Gemini pin for class I and P required reviews. The runs finished on
2026-11-21 between 10:12 and 10:29 UTC. No shell and no file access; the diff and the commit
messages were inlined.

**Served model not echoed.** The agy lane's JSON output has no model
field, so the served model is known only from the pin on the command
line. That is recorded as such, as for every Gemini run.

| Range | Finished | Refusal | Tokens (input / output, of which thinking) | Time, turns | Findings (B/M/m) | Hits | Missed | False | Valid, unplanted |
|---|---|---|---|---|---|---|---|---|---|
| R1 | yes | none | 15.5k / 18.0k (16.6k) | 217 s, 1 | 2/1/0 | R1-1\*, R1-2 | none | 0 | 1 |
| R2 | yes | none | 37.4k / 23.1k (20.0k) | 255 s, 2 | 1/2/0 | R2-1\*, R2-3, R2-4\* | R2-2 | 0 | 0 |
| R3 | yes | none | 16.0k / 17.5k (16.6k) | 194 s, 1 | 1/1/0 | R3-1\* | R3-2 | 0 | 1 |
| R4 | yes | none | 52.4k / 24.2k (21.3k) | 327 s, 2 | 2/1/1 | R4-1\*, R4-2 | none | 0 | 2 |
| R5 | yes | none | 15.3k / 15.4k (13.8k) | 151 s, 1 | 1/1/0 | R5-1\*, R5-2 | none | 0 | 0 |
| R6 (clean) | yes, see below | none | 13.6k / 5.0k (4.1k) | 54 s, 1 | 0/0/0 | (clean) | n/a | 0 | 0 |

| Measure | Result | Needed |
|---|---|---|
| Critical found | 6 of 6 | 6 |
| Planted found | 10 of 12 | at least 9 |
| Every run finished | yes, 6 of 6 (R6 by the controller's decision below) | yes |
| False blockers | 0 | 0 |
| False majors | 0 | at most 1 |
| Refusals | 0 | recorded |
| Cost | 150.0k input, 103.2k output (92.4k thinking) tokens; 1,197 s | recorded |

**R6, the clean range, counted as finished.** Its thinking used 4,117
tokens. The lane's did-it-run checks also compare a run's thinking with
the lane's other runs, to catch an empty or refused run, and 4,117 is
just under a quarter of the median of the planted runs (4,147). The
controller counted the run as finished:

- R6 has the smallest diff in the suite (2.9 KB, against 6.3 to 10.7 KB);
- per byte of diff it thought about as much as the others (1.40 tokens,
  against 1.55 to 3.37);
- its answer goes through the change it checked, by name and line;
- an empty finding list is the expected result on a clean range.

A fresh run would have been a re-run, which the rule does not allow. The
owner may override this decision; voiding R6 would make Gemini fail
"every run finished".

## What the pass covers

- **`glm-5.3`, high** passed the code suite. The routing table uses this
  pin for the first reviewer (class I and P required reviews,
  refutations and verifications on I and P work) and for ordinary class S
  and R reviews.
- **`gemini-4-carbon-medium`** passed the code suite. The routing table
  uses this pin for class I and P required reviews, refutations and
  verifications on I and P work. Its class S and R pin is
  `gemini-4-carbon-high`, which was not run. Gemini never takes a
  security seat ([workflow](../workflow.md) §2).
- Both passes are on code. Neither lane has run the document set.

Verdicts from these two pins on code reviews no longer need "uncalibrated
on IronAFS" beside them ([workflow](../workflow.md) §2). Every other pin
keeps it.

## Not calibrated yet

| Pin | Seats | Status |
|---|---|---|
| GPT read-only, `gpt-6.2-sol`, high and xhigh | class I and P required reviews; second family and ordinary reviews on class S | not run: the owner is conserving GPT quota. GPT seats stay as they were, with "uncalibrated on IronAFS" beside each verdict, until calibrated |
| GPT read-only, `gpt-6.2-astra`, xhigh | security reviews and the rest of the strongest row | not run, same reason |
| GLM, `glm-5.3`, max | security reviews and the rest of the strongest row | not run |
| Gemini, `gemini-4-carbon-high` | class S and R ordinary reviews | not run |
| DeepSeek | backup second family | calibrated only if it is needed (the issue) |
| Claude seats | none on M2a's required seats, which review Claude-written work | calibrated only before a seat on work Claude did not write (the issue) |
| Document-review set | every lane above, at its document-review seat | built and hashed, not yet run |

The suite stays frozen for these runs. A pin that is changed, or a
threshold that is changed, uses fresh held-out cases instead.

## Evidence

- The two key digests above, recorded at 09:18:59 UTC, one minute before
  the first run; the keys were unchanged at scoring.
- Each range's head passed fmt, clippy with warnings denied, its crate's
  tests and doc tests, and the `xtask` checks for what it touches
  (EXECUTED by the planter, macOS, 2026-11-21). The defects in R1, R2
  and R4 were also shown by probe tests, deleted before the commit
  (EXECUTED, same day).
- Per-run lane output, scored against the key. Raw transcripts are not
  published ([README](README.md)).
