<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: the simulator's VL and fileserver models, part 2: callback calls, A14 and the model front end

## The change

Class I. Issue #96 (plan id M2a-38), its second pull request, based on
part 1 ([record](2026-11-21-m2a-96-simmodels.md)). The contract is the
callback service's specification (`docs/specs/callbacks.md`, issue #66):
its allowlist (5.1.2), C31's table restated (5.6, point 9) and what the
sources say on assumption A14 (5.8). Also ADR 0004, O45, points 1 and 3,
and C16, points 3 and 6, and the seams specification's `accept` and
`respond` (5.4, point 1) and scheduling model (5.6). The models are
written from those documents and nothing else (ADR 0001).

- **A path for server-initiated calls in the scheduler:**
  - a `World` trait replaces the bare server closure;
  - the world may start calls toward a task's `accept`;
  - the world's network admits or loses each attempt, and a lost attempt
    is sent again;
  - a call with no answer by its timeout fails;
  - the world may hold a task's call and answer or drop it later.
  Every decision is a trace line, and latencies come from the seeded
  network stream.
- **A callback envelope:** the break, the three resets and the probe, by
  their IronAFS names. It is the simulator's own encoding, since 5.1.3
  and 5.5 mark every layout MISSING.
- **The fileserver sends breaks, resets and probes on demand.** Forged
  calls come from the network's own address, and the cell can lose the
  first attempts of a kind of call.
- **A14 as 5.8 states it.** A break that never reaches its client makes
  that client's next call wait for a reset. If no reset reaches it, the
  call is dropped.
- **Two invariant checks of O45, point 3:** continuity after a failed
  break, and break order at in-process and out-of-process bindings.
- **A model front end:** its own task applies epoch advances, and
  discards stale replies, one item per step.
- **Trace format 3:** this branch adds kinds of line after part 1's move
  to 2.
- **Shared code with the client: none.** Part 1's check that
  `ironafs-sim` depends on no client codec crate still covers it.
- **Written by** `claude-code:claude-opus-5-5` (a Claude Code subagent,
  test first).

Left on #96: the test strategy's additions (M2a-76), which move as the
issue says.

## Decisions

Made by the controller on the implementer's questions:

- **The trace version moves to 3.** The two parts land separately, so
  each new kind of line gets its own move, by the rule `VERSION_LINE`
  states.
- **The network's own address.** A forged call is started by
  `sim.forge` sent to `NETWORK`, an address no server holds. That keeps
  part 1's rule that bytes to an absent server are dropped unread.
- **Reset before the answer.** This is the model's way of telling a
  client that a break was lost (A14): the client's next call is held
  until a reset has reached it, and is dropped if none does.
  - The reset is the one mechanism the sources name for a server to
    tell a client of a loss (the capabilities draft, §6.1).
  - Which reply or code a real server uses is A14's open question,
    answered by the test cell's case TC2.
  - So this is the model's choice pending TC2, not a wire fact, and no
    abort code is invented.

## Rounds

The required seat (class I, Claude-written code): GLM, the first
reviewer. It ran read-only, without running the tests, on 2026-11-21
(UTC).

| Round | Range | Seat | Served model | Effort | Verdict | Quota headroom |
|---|---|---|---|---|---|---|
| 1, review | `e62f61f..4ab48e5` | glm | glm-5.3 (calibrated on IronAFS, #48) | high | PASS: 3 minor, 5 uncertain | not reported |

## Findings

GLM, round 1:

- **Minor, fixed (`995c18f`):** origin: pre-existing; kind:
  correctness. A failed reset dropped the held calls only if the
  fileserver itself had started that reset. So a reset started by a
  scenario dropped nothing when it failed. And with two resets in
  flight, the first failure dropped the calls even if the other reset
  then succeeded.
  - The server now counts the resets in flight per client, whoever
    started them. A held call waits on any of them. The calls are
    dropped only once every reset in flight has failed.
  - The two new tests failed first.
- **Minor, fixed (`2c51afa`):** origin: pre-existing; kind: consistency
  between parts. The world path copied the cell's serving policy for
  envelope bytes instead of sharing it.
  - Both paths now call one function. Only the hold depends on the
    path.
  - A test compares the two paths on fifteen inputs, with a server up
    and down. It failed against a copy changed on purpose before the
    refactor, and passes after.
- **Minor, fixed (`d7c7910`):** origin: pre-existing; kind: missing
  mechanism. The scheduler's guard against a world releasing a call it
  never held had no test.
  - The test failed with the guard removed.

Of GLM's five uncertain items:

- Two noted that the lane could not run the tests or read the issue.
  The evidence below, and the pull request's map of each acceptance
  item to its scenario, cover them.
- One asked whether the golden fixtures are deterministic across
  targets. The version-3 fixtures differ from part 1's in their version
  line only, and CI's Ubuntu run checks the Linux set.
- One asked whether the scenarios' timing works for every seed. Each
  scenario's trace assertions show it does, for seeds 1 to 3: every
  attempt lost, the expiry, then the hold, the reset and the reply, in
  that order.
- One noted that a task could build a simulator `Incoming` it never
  accepted. Tasks are the code under test, and the client will reach the
  scheduler only through the simulator's `Rpc` adapter. So this is a
  limit below, not a defect.

The review stopped there, under the stop rule: only minor findings,
fixed and recorded, so no verification round.

## Limits

- The client side of the scenarios is a toy written in the tests. The
  client's callback service and cache are M2a-27's and M2a-39's.
- The keepalive procedure is the keepalive specification's, which is not
  written yet. The A14 run uses a status fetch as the client's call to
  the server.
- The network loses requests, never answers.
- The retransmission count, interval and timeout (3, 1 s, 5 s) are the
  model's own values: no specification states a server's.
- `ironafs_sim::sched::Incoming` has public fields. Making them private,
  so that only `accept` builds one, is a later hardening.

## Evidence

- At `4ab48e5` and at the fix head `d7c7910` with this record, all exit 0
  (EXECUTED, macOS 27, Apple silicon, 2026-11-21 UTC):
  - `cargo xtask check`;
  - `cargo nextest run --workspace --retries 0 --no-tests fail`, with 466
    and 470 tests passed;
  - `cargo test --doc --workspace`;
  - `cargo clippy --all-targets --workspace`, with no warnings.
- Red before green:
  - the scheduler's push tests, against stubs of `accept` and `respond`;
  - the callback envelope's tests, against stub decoders;
  - the fileserver's delivery and A14 tests, and the cell's world tests,
    against stubs;
  - the continuity, break-order and front-end tests, against stubs;
  - the seven scenarios: five against a cell hook that never starts a
    callback call, and the two front-end ones against a front end that
    returns at once;
  - the version-3 literals, before the constant moved;
  - the three fixes' tests, as above.
- The envelope's round-trip entries for the new simulator procedures
  were first seen failing only as a compile error.
- Each scenario runs twice per seed, for seeds 1 to 3, and asserts
  identical traces.
