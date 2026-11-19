---
name: phase-lifecycle
description: Use at the start of any phase or milestone of IronAFS work (a new ADR, a spike, a protocol layer, a platform layer) and at its exit — how a phase runs from definition of done through research, design, plan, execution by class and exit. Triggers on "start the next phase", "what's next", "plan the spike", "start the architecture ADR", "milestone".
---

# Running a phase

A phase is a body of work with one goal, such as the architecture ADR, the
first spike, or the Rx core. `docs/workflow.md` governs each change inside
it. This skill governs the phase around them.

## 1. Define done

Before any research, write down what is true when the phase is finished, and
how that will be shown: commands, results, venues. Record it in the
phase's milestone description (`docs/workflow.md` §6).

## 2. Research

- Use primary sources, with today's date: specifications, upstream
  documentation, measured behaviour.
- **Refute first.** Run every mechanical claim (a version, a flag, an API
  behaviour) before relying on it.
- Write the result into one dated file in `docs/research/`, marking each claim
  with its evidence (`docs/workflow.md` §3c).
- ADR 0001 applies. Published specifications and measured behaviour are
  valid inputs for protocol code. Notes made from reading another
  implementation's source are background only: facts that can only come from
  source go through ADR 0001's separate spec step.

## 3. Design

- A design fork (two credible options) gets the opposing case argued by a
  second model family in the same round.
- Decisions go into an ADR, reviewed before the owner accepts it.
- Lane dispatch and reviews are the maintainer's controller's job
  (`review-lanes`). A contributor raises a design through an issue or a
  spec-only draft PR, and the maintainer arranges the reviews.

## 4. Plan

- The plan is a list of four-part briefs (`docs/workflow.md` §3, step 3).
  Each names its class, the machine it runs on, and that machine's limits.
- Integration checks are tasks in their own right, not afterthoughts.
- One plan review by a non-author family, scoped to contracts, arranged by
  the maintainer. Rounds after the first review only what changed.

## 5. Execute

- **Changes run by class** (`docs/workflow.md` §1–§3).
- **Implementers stop when the brief is wrong.** They report it, not patch
  around it.
- **Implementer reports carry:**
  - for a bug fix, or a change whose class requires reproduction, the
    failing-then-passing test runs; otherwise, the acceptance evidence named
    in the brief;
  - for each new guard, the answer to "what input defeats it?";
  - any departures from the brief.
- **Parallel implementers** work in separate worktrees. The controller runs the
  compiler and the tests one at a time.

## 6. Exit

- Check every "done" item from step 1, with its evidence.
- If the phase froze anything (a wire format, a cache format, a release), it
  takes the class R path.
- Update `docs/specs/` and the ADR index, close the milestone, and say in
  each open issue where its work went.
