<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Test strategy

Status: draft (2026-11-21). Raised in issue #63.

Kind: procedure. Class: P. It sets the test obligations every contributor
and reviewer applies, and what each test tier runs and reports; a later
change that weakens an obligation on class S or R code is class S
([README](README.md), "Review class per kind").

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (T1, T2 and so
on) are in section 12; outside documents are registered in the
[README](README.md)'s register of editions (rows E3 to E8).

## 1. What this implements

Ids are ADR 0004's. Quotations are word for word.

ADR 0004, overview, constraint 4 (a constraint of the whole ADR):

> **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
> local IPC, configuration, requests from the operating system, and SMB
> messages before authentication. No panics, checked arithmetic and byte
> limits apply to all of it (coding standards rules 1 to 3).

ADR 0004, C3 (**Working**), points 2 and 6:

> 2. The subtle parts of the cache manager are pure types: coherency
>    states, fencing, rights, the chunk map, the lock table. They get
>    exhaustive model tests.

> 6. One deterministic simulator drives points 1 to 3 through the injected
>    interfaces. It is a deliverable of M2a (O45).

ADR 0004, C4 (**Working**), point 7, its third sentence:

> The simulator is a test-only workspace crate, `ironafs-sim`, which is
> never shipped.

ADR 0004, O22 (**Firm**), its opening:

> These hold in every phase, on every OS and at every setting. No acceptance
> gate waives them (O46).

ADR 0004, O45 (**Working**), point 3's opening and point 6:

> Invariants are checked after every step:

> 6. Event sequences fed to the simulator are fuzzed (O47).

ADR 0004, O46 (**Firm**), point 1's first sentence and point 3's first
sentence:

> A cross-context or cross-user leak, a privilege escalation, or a breach
> of any O22 invariant fails acceptance, in every phase and on every OS.

> A phase gate that cannot show point 1 holds on a platform does not ship
> that platform's layer in the affected mode.

ADR 0004, O47 (**Working**), points 1 to 3:

> 1. **Model-based tests** compare the client against a small reference model
>    of AFS semantics as IronAFS specifies them: the last closer wins;
>    other clients see changes after a close and a break; handles in the
>    writer's own context see unstored bytes, and other contexts see the
>    server's version (C18).
> 2. **Property tests** use our own seeded generator and shrinker (rule 21).
> 3. **Fuzzing** covers every decoder: XDR, Rx, RPC replies, the directory
>    format, FUSE messages, local IPC, the configuration parser, DNS
>    replies; every SMB request and compound decoder, before and after
>    authentication; rxkad and rxgk token and challenge decoders; and the
>    journal, recovery-store, pin and marker decoders.

ADR 0004, O48 (**Working**), point 5:

> 5. Every claim about mount behaviour is proven on its own venue
>    ([docs/workflow.md](../workflow.md) §5).

ADR 0004, O49 (**Working**), points 1 to 3:

> 1. Benchmarks in CI count; they do not time. They count: RPCs per long
>    listing of N entries, packets per MiB at 1% loss, and bytes fetched on a
>    warm restart.
> 2. Timed benchmarks run nightly on fixed hardware and are recorded as
>    numbers.
> 3. Pull requests run only on GitHub-hosted runners.

Coding standards, rule 7, its last sentence:

> Tests are never retried; a retry that passes hides a nondeterminism bug.

Coding standards, rule 15, "Deterministic, always", its second item:

> Random inputs come from a seed that is printed on failure.

Also cited, without a quotation this page must keep word for word: C3,
point 1; C4, points 1 and 4; C37, point 7; O30, point 4; O41, point 4;
O45, points 1, 2 and 4; O47, points 4 to 6; O48, points 1 to 4; O49,
points 4 to 7; L40; coding standards rules 1 to 3, 5, 9, 16, 20 and 21;
[docs/workflow.md](../workflow.md) §1, §3b, §3c and §5. All are Working
unless marked.

**The owner's decisions this page records** (issue #63; T3). Each is
stated where it applies:

1. Pure crates are `#![no_std]` with `alloc`. Their unit tests in `src/`
   name only `core` and `alloc`; tests that need `std` live in `tests/it/`
   (5.1, point 1). The decision is being written into C4, point 1, by an
   amendment (issue #53); this page follows the decision now.
2. Pure-crate unit tests may use `ironafs-test-support`'s `Property` and
   `Source`; its other helpers only from `tests/it/` (5.1, point 2).
3. The mutation tool is cargo-mutants, pinned (5.4, point 4).
4. The coverage tool is cargo-llvm-cov 0.9.1, pinned, a checksummed
   prebuilt binary in the nightly job only, behind `cargo xtask coverage`,
   which reads only its JSON export; line, region and function coverage,
   branch coverage "not available" (5.4, point 3).
5. Coverage floors: report only for the first month, then a ratchet on the
   uncovered-line count of hand-written code in the pure decoder and
   state-machine crates; no absolute floor (5.4, point 3).
6. The budgets of 5.5 are adopted now and revised after a month of
   measurements.
7. Timed benchmarks run by hand on one named machine until fixed hardware
   exists (5.6, point 5).
8. Published corpora (NIST, Wycheproof, RFC test cases) are a separate
   conformance layer, never a specification's golden vectors (5.3,
   point 5).
9. The inventory is kept by test names (5.4, point 5).
10. The differential listing check's oracle is the generator (5.2.5, L5).
11. No seed, input or replay command is published from scheduled jobs
    (5.5, point 2).
12. The freshness check's gap while the repository is idle is accepted
    until M4, when a scheduled watcher in the private CI repository takes
    over (5.5, point 3).

Section 9 shows where each quoted decision is kept and which test fails if
it is broken.

## 2. Scope

1. **Covered:** the test obligations of each kind of code; how the suite
   grows by construction; how each category of test shows it can fail;
   the inventory ratchet; no retries and the flaky-test rule; the replay
   and trace rules; the generator's form; the tiers with their budgets
   and venues; counted and timed benchmarks; the layout of tests in pure
   crates; and how scheduled failures are classified and reported.
2. **Not covered, and where it is covered:**
   - each component's own vectors, corpora, scenarios and acceptance
     cases: that component's specification, section 10;
   - the simulator's world, fault list, invariants and scenarios: the
     simulator's specification (planned) and its issues (#79, #96);
   - the seams' shapes, the scheduling model, crash and restart, strict
     replay and the trace's encoding: the seams and scheduling
     specification (planned, issue #71). This page states only the test
     obligations that rest on them;
   - the review seats: [docs/workflow.md](../workflow.md) §2;
   - adopting the fuzzing engine's binding: its own `add-dependency`
     record (O47, point 5);
   - the flaky-test rule's wording in the coding standards: rule 15, as
     amended by its own issue (#76). This page states the rule; the
     coding standards are where contributors read it;
   - restoring a running client's virtual machine from a snapshot: a
     documented residual (C37, point 7). No test here claims the client
     stays correct across one.

## 3. The design in short

1. **Tests come from the written contract.** Every specification's
   section 10 names its tests; this page says which kinds each must have,
   as checklists with stable ids (5.2).
2. **Volume comes from tables and generators,** not from hand-written
   cases: vector tables derived from specifications, tests generated per
   message from the codec definitions, and a simulator matrix of
   scenario, topology, profile and seed (5.3).
3. **Every category of test shows it can fail:** a planted defect, a
   mutation, an exercise count, a generator label or a paired control
   (5.4, point 1). None of these replaces rule 15's red-then-green on each
   changed contract.
4. **The suite never shrinks silently.** Tests, table rows, corpus
   entries, scenarios and seeds are reconciled by identity with what ran;
   one that disappears is named in the pull request that removes it.
5. **Nothing is retried.** A test that passes and fails on one revision is
   a bug with an issue.
6. **Four tiers:** every pull request on hosted runners; nightly with
   fresh seeds; weekly with long fuzzing and the interoperability
   harness; and the phase gate and pre-release run on each operating
   system's venue (5.5).
7. **Counts in CI, times by hand or on fixed hardware** (5.6).
8. **IronAFS's own code first.** The generator, shrinker, simulator,
   schedule explorer, benchmark harness and JSON checker are IronAFS's.
   Standalone tools that run but are not linked are pinned, each by the
   owner's decision under rule 21.

## 4. Terms

- **Pure crate:** a crate of C4, point 1.
- **Refusal path:** code that denies a request, rejects input, lowers
  trust or fails closed.
- **Guard:** a check on class S or R code that a refusal path rests on.
- **Planted defect:** a deliberate change to code under test that a named
  test must catch, shown in the pull request and reverted.
- **Mutant:** a planted defect produced by a tool.
- **Exercise count:** how often a property's or invariant's antecedent
  held in a run.
- **Label:** a tag a property gives each generated input (for example "at
  cap" or a union arm), counted per run.
- **Paired control:** the same check run with its condition present and
  absent, which must give opposite results.
- **Inventory:** the sorted list of test names per crate, test binary,
  feature set and target.
- **Tier:** a set of jobs with one trigger, budget and venue (5.5).
- **Manifest:** a checked-in list, by id, of what a tier must run: fuzz
  targets, scenarios, shards, required jobs.
- **Venue:** the machine class where a claim is proven (workflow §5).
- **Reproducer:** the smallest input, seed or reduced case that shows a
  defect.
- **Restricted result:** a scheduled-job failure that may be
  security-sensitive (5.5, point 9); every other failure is **ordinary**.

## 5. Specification

### 5.1 Layout

1. **Unit tests** live in `src/<module>/tests.rs` (rule 15). In a pure
   crate they name only `core` and `alloc` paths, never `std`, not even
   under `cfg(test)`. A test that needs `std` is an integration test in
   `tests/it/` `[T3: owner's decision 1]`.
2. **Test support in pure crates.** A pure crate's unit tests may use
   `ironafs-test-support`'s `Property` and `Source`, whose use names no
   `std` path. Every other helper of that crate is used only from
   `tests/it/`, where the crate under test is linked as a dependency like
   any other user, so the helper and the test see the same copy of its
   types `[T3: owner's decision 2]`.
3. **Integration tests:** `tests/it/main.rs` and one module per subject
   (rule 20). Tests generated from codec definitions go in
   `tests/it/generated.rs` and its children `tests/it/generated/<subject>.rs`,
   written by an `xtask` generator and checked in (rule 20: generated code
   is produced by `xtask` and checked in).
4. **Fixtures:** `tests/fixtures/<subject>/`, one file per case, each with
   its expected result beside it or in a table file (rule 15).
5. **Regressions** (5.3, point 7):
   - a test named `regress_<subject>_<condition>_<outcome>`, beside its
     subject;
   - a generator or property failure, as the pasteable Rust test the
     failure report prints, in the crate's `tests/it/regressions.rs`, never
     as a seed or data file;
   - a byte input as a fixture under `tests/fixtures/regress/`;
   - a fuzz input as a tracked seed under `fuzz/seeds/<target>/`.

   Names carry no issue number: source holds no task id other than a
   `TODO(#123)` (`AGENTS.md`, "No history in source"). The pull request
   names the issue.
6. **Fuzz targets:** `fuzz/fuzz_targets/<crate>_<subject>.rs` (rule 20).
   Tracked seeds are under `fuzz/seeds/<target>/`, built from the golden
   vectors and the reject corpus. `fuzz/corpus/` and `fuzz/artifacts/` are
   the engine's working directories and are ignored by git, so a crash
   input is never committed or published by accident.
7. **Simulator scenarios** and their seed lists live in `ironafs-sim`'s
   library, so the replay command and the tests call the same function.
8. **Benchmarks:** `benches/<name>/main.rs` with `harness = false`
   (rule 20).
9. Every fixture and seed directory is covered by `REUSE.toml`.
   Third-party corpora keep their own licence, a `LICENSES/` entry and a
   `docs/provenance.md` row, in the same commit (ADR 0001;
   [README](README.md), "Test vectors and fuzz targets").

### 5.2 Obligations by kind of code

Each list is a checklist. The implementer answers each item in the pull
request, and the first reviewer checks each answer against the diff. An
item that does not apply says why in one line. Ids are stable.

#### 5.2.1 Decoders and encoders (every wire, IPC, configuration and on-disk codec)

- **D1 Golden vectors** from the specification, derived by hand or from a
  reduced black-box capture, never a standard's example bytes reused as
  the vector ([README](README.md), "Test vectors and fuzz targets"). Each
  is checked both ways where an encoder exists; a read-only parser's are
  independent fixtures (rule 15).
- **D2 Reject corpus:** at least one case per rejection rule of the
  specification, asserting the exact error kind and, where the error
  carries them, its offset and field; never only `is_err()` (rule 15).
- **D3 Round trip,** where an encoder exists: every accepted value
  survives encode then decode; every canonical input survives decode then
  encode; a tolerated non-canonical input decodes to its value and
  re-encodes canonically.
- **D4 Caps:** for every length and count, the value at its cap is
  accepted and the cap plus one rejected; a huge declared length is
  rejected before anything is allocated; the message's cap is tested at
  its boundary.
- **D5 Truncation:** where every proper prefix is incomplete (wire and IPC
  messages), each prefix fails with the truncation error; elsewhere, the
  prefixes the specification names; never a panic. A pull request runs
  every prefix up to 4 KiB, then every field boundary and 256 seeded cut
  points; the nightly tier runs every prefix `[OWN: every prefix of a
  large message is quadratic in its length; the pull-request tier keeps
  the cases where defects cluster]`.
- **D6 Single-byte changes** of valid inputs: each padding and reserved
  byte changed, and each discriminant set to an undefined value, gives the
  specified error; a discriminant set to another defined arm decodes as
  that arm or fails as the specification says.
- **D7 Arithmetic:** each sum, product and offset of the specification's
  section 6 at its overflow edge.
- **D8 Secrets:** the `Debug` text of a secret-bearing value holds no
  value of a secret field (rule 8), checked with a planted marker value.
- **D9 Fuzz target** (O47, point 3), asserting: no panic; the same result
  for the same input; on success with canonical input, re-encoding gives
  the input (D3); on error, the offset is within the input; and the
  decoder's memory accounting, where its specification defines one, stays
  within the message's budget for every input, accepted or not. The
  target's maximum input length is the message cap plus one, and its
  tracked seeds include inputs at the cap and at the cap plus one, so the
  engine starts at the boundary instead of having to find it.
- **D10 Stack:** the largest valid input decodes on a thread with the
  small stack its specification names.

#### 5.2.2 Pure state machines (Rx, the security classes, the cache types, the lock table, the token store, configuration layering, event rate limiting)

- **M1 Reference model:** a small, obviously correct model in `tests/it/`
  (an ordered map and a few rules, no optimisation), driven by the same
  random operation sequence as the code; every observable output is
  compared after every operation (O47, point 1).
- **M2 Transition coverage:** every (state, event) pair of the
  specification's section 5.6 has a named test or a non-zero M3 count. A
  check lists the pairs against both and fails on an uncovered pair
  `[OWN: a table of pairs is the specification; a pair no test reaches
  is an untested requirement]`.
- **M3 Exhaustive enumeration** over a finite abstraction the
  specification justifies, where the sequences enumerated (or the distinct
  states, when deduplicated) number at most about one million: every
  sequence up to a stated depth over a stated alphabet, for example the
  coherency states with break, fetch, expiry and restart events, or three
  owners and two byte ranges for the lock table. The bound, the total and
  the count per (state, event) pair are printed (C3, point 2).
  `ironafs-test-support`'s exhaustive sweep refuses a larger domain rather
  than sampling it.
- **M4 Invariants** are asserted after every step of M1 and M3, not only
  at the end.
- **M5 Time and randomness** only through `Clock` and `Rng` (rule 7);
  tests use the seeded implementations, and cover far dates (2039, 2107)
  and clock regressions wherever time is an input.
- **M6 Determinism:** the same inputs give the same outputs, asserted
  across two runs in separate processes for Rx and the cache types.

#### 5.2.3 Seams, drivers and bindings (`Clock`, `Rng`, `Rpc`, `Disk`, the network driver, `ironafs-os`, `ironafs-gss`)

- **S1 One conformance body per seam trait,** a single generic function in
  `ironafs-test-support`, run against every implementation of the seam:
  the seeded fake, the simulator's and the production driver. A rule that
  holds in one and not another fails on that side. A body that runs
  against fewer implementations than exist, or passes with no case run,
  fails. An implementation that can inject faults runs the body with
  faults off and asserts that none fired `[OWN: one body is what makes
  the simulator's answers the production driver's answers]`.
- **S2 Fault conformance:** each fault the simulator injects (no space, a
  read error, a sync failure, loss, and a sync or rename that took effect
  but returned an error) is also produced against the production driver
  where the venue allows it, and surfaces as the same error kind.
- **S3 Driver thinness:** a driver holds no decision a pure crate could
  hold. Reviewers check it; `check-deps` keeps drivers out of pure crates
  (C4, point 4).
- **S4 The fault feature.** Fault and crash-point entry points in a
  production driver exist only behind one Cargo feature, off by default,
  reached through one gate function, so a new fault site cannot bypass
  the gate. A check over the release build of every shipped binary proves
  the entry points absent (8, point 2). The feature's definition, and any
  change to what enables it, is class S.
- **S5 Bindings:** each `ironafs-os` site is tested through its safe
  interface on every operating system that compiles it (rule 15).
  `ironafs-gss` runs one conformance body against a fake and, on its
  venue, against the system library. Log sinks and packages are tested on
  their venue, as packaged.

#### 5.2.4 The client composition (`ironafs-client`, `ironafsd`)

- **C1 Simulator scenarios** for every behaviour the composition adds:
  each new code path names the scenario family that drives it and an
  invariant with a non-zero exercise count over it.
- **C2 Reference semantics:** the AFS semantics model of O47, point 1, is
  checked by the simulator against the client.
- **C3 Startup order:** a planted failure at each startup step stops
  startup before the next step.
- **C4 Contract suite:** the `ironafs-vfs` contract tests run against the
  scripted core and against the composed real core.

#### 5.2.5 Operating-system layers (FUSE, FSKit, SMB, Cloud Files)

- **L1 Codec tests** of the layer's own codec, per 5.2.1.
- **L2 Contract tests** over the scripted core, in CI on each operating
  system that builds the layer.
- **L3 Venue tests:** each claim about mount or operating-system behaviour
  is proven on its venue (O48, point 5), and the evidence names the
  machine, the system build or kernel, and the negotiated features. FUSE's
  initialisation is tested with each optional feature refused.
- **L4 Acceptance cases:** each case of O48, point 4, is a named test per
  operating system, with a simulator half and a venue half; the venue half
  runs at each phase gate that ships the layer (5.2.8, N3). The smoke of
  L40 is one of them.
- **L5 Differential listing check:** a seeded generator builds a tree of
  awkward but legal names, sizes and modes in the test cell; the tree is
  read back through the mount and printed in a stable, byte-sorted text
  form; the print is compared with the generator's own description of the
  tree, which is the oracle `[T3: owner's decision 10]`; and a separate
  check proves that the print holds every case the generator claims to
  produce, so an empty or truncated tree cannot pass.

#### 5.2.6 The command line (`afs` and its compatibility aliases)

- **X1 Process tests** through `CARGO_BIN_EXE_afs` (rule 20).
- **X2 Exit codes:** a table in the command line's specification; each
  code has a test that produces it.
- **X3 Output that is a contract** (the compatibility aliases' output) is
  compared with a snapshot beside its test; a snapshot changes only with a
  class P review.
- **X4 `--json`:** every command's JSON is checked against its documented
  schema by IronAFS's own checker in `ironafs-test-support` (rule 21: no
  third-party schema validator); the schema version field is asserted; a
  planted extra field and a planted missing field each fail.
- **X5 "Could not run"** is reported as such, never as success, shown with
  a planted failure for every check that can fail to run.
- **X6 Projection:** a second local user sees only its own state.
- **X7 Environment:** tests set the locale and terminal width they rely
  on; output never depends on the developer's environment.
- **X8 Real core:** the effective configuration, a failed reload that
  keeps the last good one, and X6 are asserted through real IPC to a
  composed daemon.

#### 5.2.7 Durable formats (the state directory, its records, the journal, the recovery store, the audit checkpoint)

Everything in 5.2.1, plus:

- **F1 Crash points:** a crash at every write, sync and rename of every
  durable update, through the simulated disk, with the outcome checked
  after restart. The points are those the run reached, not a fixed count.
  The test asserts properties, not a total: every acknowledged byte is
  recoverable; no failed update is promoted; an update that became durable
  but was not yet acknowledged is recovered whole or not at all; a store
  whose reply was lost is never reported confirmed and never replayed
  automatically (C28, points 3 and 4); the last good record is never
  damaged; and the writes, syncs and renames happened in the specified
  order, so a missing sync fails even though it removes its own crash
  point. A rename whose directory sync did not complete is run with both
  outcomes, and so is each fault of S2 that took effect but returned an
  error.
- **F2 Restart and host crash** are run as separate ends: a daemon restart
  in the same boot still reads every written byte, including bytes whose
  sync failed, and never treats them as durable; a host crash keeps a
  seeded draw of what was not durable. Besides the seeded draws, each crash
  point runs every legal outcome class by script (unsynced bytes absent, a
  prefix, torn or whole; a name present or absent; a rename applied or
  not), with a count per class; a class at zero fails the case manifest
  check (5.3, point 3).
- **F3 Compatibility corpora:** when a format version freezes, files
  written by that version go under `tests/fixtures/<format>/v<N>/` and are
  never edited. Every later version reads each file, or refuses it exactly
  as the format's section 5.3 says.
- **F4 Newer versions:** a file of an unknown newer version is refused
  with the specified error and left unchanged ([README](README.md),
  "Formats IronAFS defines").
- **F5 Storage qualification:** writes ship on a storage configuration
  only after O41, point 4's crash and power-loss tests pass on it.

#### 5.2.8 Security properties

- **N1 Every refusal path has a test that proves the refusal:** the exact
  error or event, and that nothing was done (no connection made, no byte
  served, no record touched), asserted where the act would happen (the
  simulated wire, disk or IPC peer), with a positive control: the request
  the guard admits does act. A refusal-coverage check lists every refusal
  error variant and every refusal or audit event of the catalogue, and
  each specification's refusal cases by their section 10 id, and fails if
  one is asserted by no test `[OWN: one error variant can serve several
  guards, so the cases are counted, not only the variants]`.
- **N2 Every guard added or changed on class S or R code** is shown by a
  planted defect: the diff of the plant, the named assertion going red,
  the plant reverted, and the unplanted run green. A plant that does not
  build, or a run that selected no test, is missing evidence, never a
  caught defect (workflow §3c). A checked-in record lists each guard's
  planted defect (its anchor, the change, the test that must fail), and
  an `xtask` probe applies each, runs the named test and restores the
  file; the probe reports "no result", never "caught" or "survived", when
  the anchor did not match the stated number of times, the file did not
  change, the baseline was not green or the restore was not verified.
- **N3 O22's invariants** each map to simulator invariants or named
  acceptance tests, in a table in the specification that carries the
  invariant. The phase gate checks executed results, not the mapping: a
  pass of each mapped case at the reviewed revision, on each operating
  system and mode the phase ships (O46, point 3). An unmapped point, or a
  missing or failed result, keeps that mode out. The uid-reuse residual of
  O46, point 1, is the one recorded limitation; O48, point 4's case tests
  its bounds per host type.
- **N4 Adversary scenarios:** forged callbacks, guessed Rx identities, a
  hostile server's oversized replies, and a local process sending
  malformed IPC (framing and handshake in both directions), each a
  scenario or a test that names its adversary.
- **N5 Secret hygiene:** logs, support bundles, `--json` output and error
  text are scanned in tests for planted secret values (synthetic marker
  bytes in test tokens and keys); a match fails.
- **N6 Embargoed reproducers** stay private until disclosure (O30), then
  join the public regression corpus (5.3, point 7).

#### 5.2.9 Concurrency

- **K1 Schedule exploration** in the simulator: random schedules from the
  seed, plus a priority-change scheduler (random distinct priorities for
  n tasks, with d − 1 priority change points among k steps), with n, k and
  d printed. Its published probability bound is cited only for that
  algorithm, and only once the paper is registered `[OWN: a random walk
  over schedules rarely reaches a bug that needs a few ordering
  constraints at once; changing priorities at a few chosen points reaches
  them far more often]`.
- **K2 Bounded exhaustive schedules** for small scenarios (two or three
  tasks, a few yield points): every interleaving up to a stated number of
  preemptions, with the count printed.
- **K3 Driver threads** (the thin drivers of C39): stress tests under
  nextest's deadline, where a hang is a failure. No model-checker crate
  (rule 21).
- **K4 Lock order** is a simulator invariant, not only a review item.
- **K5 Interleaving witness:** a scenario that asserts a property of a
  race sweeps a seed range, never one seed. For each seed it records
  whether its two named meeting events (each naming the same contested
  object) occurred inside the critical window, and in which order. A seed
  where either event is missing is "not exercised", never a pass. The
  scenario fails unless each order it claims is witnessed on at least a
  stated floor of seeds, and it reports the exercised and not-exercised
  counts `[OWN: a uniform choice among runnable tasks often gives a
  schedule in which the two tasks never meet inside the window, so a race
  test on one seed can pass without the race happening]`.

### 5.3 Volume by construction

1. **Vector tables.** A specification's golden vectors and reject corpus
   are tables. One test loops over the table, runs every row, reports each
   row by its id, and then fails listing every failing id.
2. **Generated per-message tests.** The `xtask` generator that emits the
   codecs of `ironafs-proto`, `ironafs-ipc` and `ironafs-events` also
   emits, for every message: D3 both ways; D4 for every variable-length
   field at its cap and the cap plus one; D5 over its prefixes; D6 over
   every padding and reserved byte and every discriminant; the memory
   budget check; and D10. The generated files are checked in, and CI
   regenerates them and fails on any difference. The generator's own tests
   plant codec defects (a padding byte not checked, a cap off by one) and
   show the generated tests turn red. A census check, with its own walker
   of the codec definitions rather than the generator's code, lists every
   obligation above by id (message, field, check) and reconciles it with
   the generated tests that ran; a missing one fails, shown by a planted
   omission in the generator. Golden vectors are never generated: their
   bytes must be independent of the code.
3. **The simulator matrix.** A run is a scenario, a topology, a profile
   and a seed.
   - Scenario families cover reads, callback breaks, forged callbacks,
     partitions, restarts, clocks and suspend, the executor, volume moves
     and releases, and far dates; from M2b also credential contexts and
     revocation; from M3 writes, crashes and competing writes; with the
     lock service, locks (O45, point 2).
   - Topologies: one fileserver; a location server and two fileservers;
     replicated read-only volumes; dual-stack addresses.
   - Profiles: anonymous; the authenticated test profile with two
     contexts; four contexts across two accounts.
   - A scenario states which topologies and profiles apply; the tier runs
     the product of what applies, each with its checked-in seeds.
   - **Case manifest.** Each invariant of O45, point 3, each
     competing-write case of O45, point 4, and each fault case of O48,
     point 2, has a case id, a phase, a profile and a venue in a
     checked-in manifest. A check fails a phase's run that leaves one of
     its cases at an exercise count of zero.
   - Event-sequence fuzzing feeds the simulator's inputs from fuzzer bytes
     through the generator (5.4, point 9) and checks the same per-step
     invariants (O45, point 6).
4. **IronAFS's own corpora:** reduced captures from the test cell (ADR
   0001; O48, point 3), the reject corpora, and the compatibility corpora
   of F3.
5. **Published conformance corpora** for mainstream cryptography (NIST's
   validation vectors, Project Wycheproof, an RFC's test cases) run as a
   separate conformance layer through the interface IronAFS calls
   (`ironafs-crypto`), so a change of implementation cannot pass
   silently. They are never a specification's golden vectors and never
   the only check of a requirement; each enters under its own licence as
   the [README](README.md) sets out ("Test vectors and fuzz targets")
   `[T3: owner's decision 8]`. rxkad's cipher has no published corpus; its
   vectors come from its specification and from observed exchanges.
6. **Fuzz seed replay.** Every pull request runs each fuzz target's
   tracked seeds through the target's assertions as an ordinary test,
   without the engine, so the targets stay building and their seeds stay
   passing on the stable toolchain.
7. **The regression corpus.** Every bug fix adds its minimal reproducer in
   one of the forms of 5.1, point 5, shown red before the fix. Its name
   says what it checks, not which issue found it. It runs on every pull
   request and is never deleted while the code it tests exists.

### 5.4 Quality of the tests

1. **Every category of test shows it can fail.**

   | Category | Evidence |
   |---|---|
   | Golden and reject tables | a row with a wrong expected value fails, in the table runner's own test; D2 asserts exact kinds |
   | Generated tests | the generator's planted codec defects (5.3, point 2) |
   | Property tests | a planted failing property shrinks to its known minimum; required labels (point 2) |
   | Model tests | a planted divergence in the model is caught within the case budget |
   | Simulator invariants | one test-only mutation switch per invariant, caught within the pull-request seeds; exercise counts above zero |
   | Race scenarios | the interleaving witness (K5); a planted schedule that never interleaves fails |
   | Guards on class S and R code | N2's planted defect |
   | `xtask` checks | `--self-test`, which plants a failure and confirms it is caught (workflow §3b) |
   | Clippy bans | each banned path resolves, since a path that names nothing bans nothing (a planted misspelling fails the check), and each ban fires on a planted call |
   | Harness checks (a fixture, a scripted fault, a capture filter) | a paired control (point 7) |
   | Integration and venue tests | a planted regression in the pull request that adds the test |

   This shows the harness can fail. Each changed contract still needs
   rule 15's proof: an expectation derived independently of the code, red
   on a compiling wrong implementation, then green.
2. **Generator reach.** A property declares the labels it requires and a
   minimum count for each. The run fails if a required label was reached
   fewer times than its minimum, and the report prints every label's
   count `[OWN: a generator that never reaches the cases a property
   claims to test passes without testing them; this is the property-test
   form of the simulator's exercise counts]`.
3. **Coverage** `[T3: owner's decisions 4 and 5]`.
   - Measured nightly with rustc's coverage instrumentation, which reports
     function, instantiation, line and region coverage and needs the
     profiler runtime and the LLVM tools component `[E4: CONFIRMED]`,
     through cargo-llvm-cov 0.9.1, pinned: a prebuilt binary whose
     checksum is verified before it runs, installed only in the nightly
     coverage job, which holds no secret and has a read-only token, and
     run only behind `cargo xtask coverage`. `xtask` reads only the tool's
     JSON export `[E5: CONFIRMED that the tool exports JSON, runs tests
     through nextest, and offers prebuilt binaries]`.
   - Line, region and function coverage of library code, per crate, each
     figure naming its target; code behind a `cfg` the target does not
     build is "not measured", not uncovered. Branch coverage is reported as
     "not available": the tool's branch option needs a nightly compiler
     `[E5: CONFIRMED]`, and the workspace's toolchain is stable. Doc-tests
     are not measured, since the tool needs a nightly compiler for them
     `[E5: CONFIRMED]`, and the report says so.
   - Test code is not counted as covered code. The tool leaves `tests`
     directories and files named `tests.rs` out of its report by default
     `[E5: CONFIRMED]`, which matches rule 15's unit-test files; other
     exclusions are file patterns in one checked-in list, each with a
     reason, never source attributes or `cfg(coverage)` code.
   - The report has four tables: hand-written code in the pure decoder and
     state-machine crates; generated codec files; `ironafs-client`, the
     drivers, `ironafs-os`, `ironafs-gss` and the operating-system layers,
     per target; and `xtask`, `ironafs-sim` and `ironafs-test-support`.
   - **Floors.** For the first month of nightly figures, nothing fails on
     a figure. After it, the first table only has a ratchet: each crate's
     uncovered-line count is checked in, with one line for each uncovered
     function and its reason, from a closed set (unreachable by design,
     diagnostic text, not built on this target). The count may only fall;
     a rise, or an uncovered function without its line, fails the nightly
     job, which reports it through 5.5, point 9. A baseline is raised only
     by a reviewed pull request that names the new entries. The other
     tables are report only. No percentage floor exists anywhere.
   - Coverage is never used to pass a pull request. It shows what no test
     reaches, not that a reached line is checked: that is mutation
     testing's job.
4. **Mutation testing** `[T3: owner's decision 3]`.
   - The tool is cargo-mutants, pinned through the `add-dependency`
     checklist's tool-pinning items. It is a standalone tool, not linked
     (rule 21). It runs tests through nextest when told to, splits a run
     into shards, and can limit a run to the mutants a diff touches
     `[E6: CONFIRMED]`. Its exclusions live in its configuration file,
     never as source attributes.
   - Nightly, sharded, over the pure crates and `ironafs-client`. On a
     pull request, limited to the diff over the pure crates it touches,
     reported and not blocking until the nightly baseline is clean.
   - Every invocation passes `--retries 0` to nextest (point 6).
   - Each surviving mutant is killed by a new test or listed in
     `tests/mutants-accepted.txt` with a one-line reason (an equivalent
     mutant, or logging only); the first reviewer checks each new line.
   - Results are classed as the tool classes them: caught, missed, timed
     out and unviable `[E6: CONFIRMED]`. A mutant that does not build, or
     a run that selected no test, is never "caught". A diff run with no
     mutants says so; a missing shard fails the job.
   - Mutation testing does not replace N2's planted defects for class S
     guards.
5. **The inventory ratchet** `[T3: owner's decision 9]`.
   - CI writes each crate's test names from nextest's machine-readable
     list, which gives each test binary's id, kind, build platform and
     test cases, each with whether it is ignored and whether it matches
     the filter `[E3: CONFIRMED]`. The inventory is keyed by crate, test
     binary, feature set and target, and compared with the checked-in
     `tests/inventory/<crate>.txt`.
   - The inventory is reconciled with the run's own results: a listed
     test that did not run and pass, ignored and filtered tests included,
     fails the job, except a test marked quarantined with its issue
     (point 6), which stays listed, may be skipped, and is printed by name
     as skipped.
   - A test in the run and missing from the inventory fails, so the file
     stays exact; `cargo xtask inventory update` adds new tests.
   - Removal is checked against `main`'s inventory, not only the pull
     request's own files: every name gone from it needs a line in
     `tests/inventory/removed.txt` (name, reason, pull request), a file
     that only grows. A quarantine mark in `main`'s inventory stays until
     the test runs again, and a new mark cites its issue. The job prints
     removed and newly quarantined names first, then per-crate counts and
     the change against `main`.
   - Table rows, corpus entries, simulator cases, seeds, fuzz targets and
     shards are reconciled the same way, by id from each harness's report.
   - Completeness is by identity, never by count (workflow §3b). A test
     moved without change shows the same names under new paths and the
     same total.
6. **No retries, and the flaky-test rule** (rule 7; rule 15).
   - Every profile and every command runs with retries at zero; a check
     fails any profile or command with retries above zero.
   - A test that passes and fails on the same revision is a bug: an issue
     is opened with the seed, the machine and both runs, and the pull
     request that exposed it waits until the test is fixed or the issue is
     linked as a blocker the owner accepts.
   - Quarantine means only an ignored or skipped test, never a retry, and
     needs an issue and the owner's decision. A quarantined test stays in
     the inventory, marked (point 5), so it cannot vanish.
   - A diagnostic re-run is a separate run and never changes the first
     result. A failing simulator seed is never flaky: it is a reproducer.
   - A timeout override cites measured times. Heavy tests join a
     thread-capped test group instead of getting a longer timeout. A crate
     that runs the simulator has a terminating timeout override, so a run
     that never ends fails.
7. **Paired controls.** A harness check (a fixture, a scripted fault, a
   capture filter) is shown working by a paired run with the condition
   present and absent, giving opposite results. A scripted fault or crash
   point that never fired fails its run, and each crash point also runs
   disarmed, as a control that must reach the point `[OWN: a harness
   check that cannot fail looks the same in a report as one that passed]`.
8. **Replay and traces.** The simulator's replay rests on the seams and
   scheduling specification; this page sets what the tests must show.
   - **Two-process replay.** The determinism test runs one seed in two
     separate processes and compares the whole traces byte for byte; a
     comparison inside one process would miss state that leaks between
     runs. It asserts that the trace is not empty; that it renders the
     fields a leak would change (task ids, draw counts, any value that
     would differ between processes); and that two pinned seeds choose
     different task or event identities, compared on those identities.
     Planted leaks, one at a time and two together, turn it red.
   - **Golden traces.** For a few fixed seeds of named scenarios, the whole
     canonical trace is committed as a fixture, per target the replay
     claim covers, and every run compares it exactly. The committed set
     for a covered target is exactly the scenario's named golden seeds, so
     a missing, empty or partial fixture fails, and so does a run on a
     covered target with no committed set. A difference is resolved as a
     defect, or as a deliberate change by regenerating the fixtures in the
     same commit, with the simulator's version line. A fixture is capped
     at 256 KiB per seed; a seed over the cap falls back to a 64-bit FNV-1a
     digest of its whole trace plus the exact bytes of its first and last
     events, and the fallback is recorded beside the fixture `[OWN: exact
     bytes need no cryptographic code and give a readable difference on
     failure; the digest only detects a change]`.
   - **A fixed error vocabulary.** Error text in a trace is a stable code
     from a closed list (the seam's error kind and cause, or an injected
     fault's kind and id), never an operating-system message, a
     locale-dependent string, a pointer or a thread id. An error outside
     the list is a simulator error. Ordinary scheduled failures are
     reported with a failure kind from a fixed list too (5.5, point 9).
9. **The generator.** `ironafs-test-support` holds one generator for
   property tests, the simulator and fuzz-driven strategies.
   - It is value-stable: a seed gives the same values on every target and
     every release, pinned by a known-answer vector in its documentation
     that a test checks. Its range reduction is its own and documented.
     A change to the values a seed gives is a deliberate change with its
     own review, since it moves every recorded seed.
   - It is driven from a seed or from a byte slice, with a documented byte
     layout, so a fuzz input drives the same strategies.
   - A large byte input is a seed plus a length, filled cheaply and shrunk
     by length with the seed kept, so a mebibyte input costs no stored
     choices `[OWN: storing every byte as a choice makes generation and
     shrinking of large inputs slow, and bulk data rarely needs
     byte-level shrinking]`.
   - A failure prints its seed when the first case fails and, after
     shrinking, a pasteable Rust test that replays the minimal input. A
     failure reaches the tree only as that test in the crate's
     `tests/it/regressions.rs` (5.1, point 5), never as a seed or data
     file the runner writes, so a regression is reviewed code.

### 5.5 Tiers and budgets

The budgets are adopted now and revised after a month of measurements
`[T3: owner's decision 6]`. They are counts of cases, seeds, prefixes and
steps, except where a row says wall time or fuzzing time, which are times
by nature.

| Tier | Trigger | What runs | Budget | Venue |
|---|---|---|---|---|
| Pull request | every push to a pull request, and `main` | build; Clippy for Linux, macOS and Windows targets; nextest (unit, integration, generated, regression, fuzz seed replay); doc-tests; native build and tests on each operating system whose `cfg` code exists (rule 15); the layer smoke once the layer exists; the inventory ratchet; simulator scenarios on fixed seeds; counted benchmarks; every `xtask` check and its `--self-test`; the pure crates' build against `core` and `alloc` only; mutation testing over the diff (report only); freshness (advisory) | property tests 256 cases each; simulator 8 fixed seeds per applicable (scenario, topology, profile), 256 per race scenario; D5 per 5.2.1; each job under 20 minutes of wall time | GitHub-hosted runners only (O49, point 3) |
| Nightly | schedule | property tests with a fresh seed; fresh simulator seeds; event-sequence fuzzing; each fuzz target for its stated time; mutation testing, sharded; the coverage report; cross-target replay comparison; timed benchmarks once fixed hardware exists | property tests 25,600 cases each; simulator 64 fresh seeds per applicable (scenario, topology, profile); fuzzing 10 minutes per target; every D5 prefix | hosted Linux runners; timed benchmarks on the fixed machine (O49, point 2) |
| Weekly | schedule | each fuzz target for its longer time, resuming from the carried corpus (point 6); corpus minimisation; the interoperability harness with every fault case of O48, point 2 | fuzzing 60 minutes per target | hosted Linux runners with the test cell (L40) |
| Phase gate and pre-release | each phase gate, and before a release candidate | the per-OS acceptance cases and the layer gates (L13, L22, L41); the interoperability matrix; the compatibility corpora on every supported version; a soak against the simulator and the real cell; freshness (required) | as the gate's manifest lists; the soak 24 hours | to M3: hosted Linux runners with the containerised cell (O48, point 1) and a Linux guest (L40, point 2), and disposable Windows guests for the SMB spike. From M4: the private CI repository's isolated runners at reviewed revisions (O49, point 4), disposable guests, and labs (O49, point 5) |

1. **A tier passes only on proof that it ran.** Each tier has a checked-in
   manifest (fuzz targets, scenarios, shards, required jobs, by id). Each
   job prints what ran by id, and a job that ran nothing fails
   (workflow §3b). The required aggregate job (`ci-ok`) depends on every
   required job of the pull-request manifest and fails unless each
   succeeded; an advisory check (freshness on a pull request, mutation
   testing over the diff) is listed apart and is never its dependency, so
   an advisory result can neither fail nor satisfy it.
2. **Seeds in scheduled jobs** `[T3: owner's decision 11]`. Every fresh
   seed is drawn from the operating system's random source, never derived
   from public run data (the date, the run number, the revision), since
   any failure might turn out restricted. No seed, input or replay command
   from a scheduled job is published, for any outcome, passing runs
   included: the job's runner captures test output instead of streaming
   it to the log, and what leaves the runner is built from an allowlist
   (job, target or scenario, revision, test name, outcome, failure class
   and failure kind), never from a test's free text, which can hold a
   seed, an input or a replay command. Rule 15's printed seed therefore
   reaches the person who runs a test by hand, never a public scheduled
   log. A failing seed reaches tracked files only as a reviewed regression
   (5.3, point 7). The fixed seeds of the pull-request tier are public by
   design.
3. **Freshness** `[E7: CONFIRMED that scheduled workflows run on the
   default branch only, may be delayed under load, and are disabled in a
   public repository after 60 days without repository activity]`. A
   freshness check fails when a tier's last complete run is older than two
   days for the nightly tier, or eight for the weekly tier. A run is
   complete when its first attempt reported every manifest item as run,
   pass or fail; staleness and failure are separate alarms. Re-run
   attempts never count, and a result list cut short by a page or run
   limit is refused, never read as complete. The check runs on pushes,
   on pull requests and on its own schedule, never only inside the
   schedule it watches. On a pull request it is advisory; at a phase gate
   or release candidate it is required. It cannot alert while the
   repository is idle; that gap is accepted until M4, when a scheduled
   watcher in the private CI repository takes over `[T3: owner's
   decision 12]`.
4. **No re-run to green.** A scheduled failure is reported (point 9); a
   re-run never replaces the first result. Scheduled workflows never
   cancel a run in progress, so a dispatched run cannot cancel a running
   nightly.
5. **Fuzzing.** The engine runs only in scheduled jobs, time-boxed per
   target; pull requests replay the tracked seeds (5.3, point 6). Each
   target runs as its own invocation, so one target's crash never skips
   the rest; each is recorded as ran, crashed or not run. A check matches
   registered targets, fuzz binaries, tracked seed directories and run
   lines one to one, each line with its time and input-length bounds, and
   checks that a job's fuzzing time plus a build reserve fits its timeout.
   When a tier cannot run every target, targets rotate, each with a
   maximum age; one past its age fails the freshness check.
6. **The weekly corpus.** The weekly run resumes from the working corpus
   the previous weekly run left. It is held in a cache that only scheduled
   runs on the default branch write; a pull request's cache is scoped to
   its own merge reference and cannot be restored by the default branch
   or written to its scope `[E8: CONFIRMED]`. The nightly fuzz job
   restores it on every run, because an entry not read for seven days is
   evicted `[E8: CONFIRMED]`. A missing corpus starts from the tracked
   seeds and the run reports it. Poisoning: only reviewed code on the
   default branch writes the corpus, and the corpus only chooses where the
   engine starts; the tracked seeds are always added, so a lost or altered
   corpus cannot lower the floor below them, and an input in it that
   crashes a target is a finding like any other. The corpus holds no crash
   input: those are in the engine's crash directory, which is never cached
   and never published in plaintext. Minimised inputs reach
   `fuzz/seeds/<target>/` only through a reviewed pull request.
7. **Third-party test servers** (O48, point 1) are pinned by image digest,
   and the `xtask` command re-verifies the digest at every start, so a
   replaced or poisoned image fails closed.
8. **Build caches.** Warnings are denied through Cargo's setting, not
   through compiler flags that invalidate the cache; no cache in a release
   or publish job; no cached binaries. Pre-release runners are single-use,
   one job each.
9. **Classifying and reporting scheduled failures.** Test jobs run with a
   read-only token and hold no secret. The runner classifies each failure
   when it captures it, failing closed:

   | Failure | Class |
   |---|---|
   | a fuzz crash | restricted |
   | a failure of a decoder property, or of a decoder's generated test | restricted |
   | a simulator invariant of authorisation or cache consistency (O22; O45, point 3) | restricted |
   | a test of a guard on class S or R code (N2) | restricted |
   | a panic, abort or other failure the runner cannot classify | restricted |
   | any other test whose binary or scenario a checked-in list marks ordinary | ordinary |

   A restricted result leaves only the job, the target, the revision, the
   class and a digest in any public log, artefact or issue, and goes to the
   maintainer through the private route of O30 and workflow §6. An ordinary
   failure files or updates a public issue holding the job, the target or
   scenario, the revision, the test's name and its failure kind from a
   fixed list (assertion failed, timed out, simulator error, could not run),
   and the statement that replay details are not published. A separate
   reporter job, the only one with issue write access, reads the result
   file as untrusted data and runs no code from the test jobs.

### 5.6 Benchmarks

1. **Counted benchmarks** are simulator tests with exact count assertions,
   run on every pull request (O49, point 1). A count is read from the
   simulated wire or disk, never from a log line. Each asserts its result
   first (for example, the bytes read equal the server model's), then its
   counts, each with a lower and an upper bound or an equality, so an
   improvement that comes from skipping work fails too. A planted skip of
   the work fails each benchmark.

   | Id | Count | Source |
   |---|---|---|
   | BC1 | RPCs for a long listing of N entries, N in 1, 100 and 10,000 | O49, point 1 |
   | BC2 | packets sent per MiB read at 1% loss | O49, point 1 |
   | BC3 | bytes fetched on a warm restart | O49, point 1 |
   | BC4 | RPCs for a second read of a cached file under a valid callback: 0 | `[OWN]` |
   | BC5 | bytes fetched after a break that changed one chunk | `[OWN]` |
   | BC6 | keepalive datagrams per server per idle simulated hour | `[OWN]` |
   | BC7 | extra RPCs to stat each entry after a listing with bulk status | `[OWN]` |
   | BC8 | retransmissions per call with no loss: 0 | `[OWN]` |

   BC4 to BC8 count the work that the cache and the transport exist to
   save, so a regression that keeps results correct but repeats work is
   seen `[OWN]`.
2. **Timed benchmarks** use IronAFS's own small harness in
   `ironafs-test-support` (rule 21): warm-up, a fixed iteration count, the
   median and spread over repeated samples, written as one JSON line per
   benchmark (name, revision, toolchain, an opaque machine label,
   iterations, median, 10th and 90th percentiles). The harness reads time
   only through an injected `Clock` (rule 7); the benchmark binary passes
   the production awake clock, so the harness needs no exception to
   rule 7. Each binary is `benches/<name>/main.rs` with `harness = false`.
3. **Pull requests never run timed benchmarks:** times on shared runners
   are not stable (O49).
4. **Records.** Results are kept as dated records with their input
   definitions pinned, so each number can be traced to its revision and
   inputs. Before a timed run, its pinned inputs and what each outcome
   decides are written down, and the reading is recorded whichever way it
   falls `[OWN: a threshold chosen after seeing the number decides
   nothing]`.
5. **Until fixed hardware exists,** timed benchmarks run by hand on one
   named machine, are recorded, and have no threshold `[T3: owner's
   decision 7]`. Once the fixed machine exists: a median more than 10%
   slower than the median of the previous seven nightly runs on the same
   machine, seen again on a re-run, opens an issue; it never blocks a
   pull request. Numbers from different machines are never compared.

## 6. Arithmetic

Not applicable: a procedure. Each codec's specification lists its own
arithmetic, which D7 tests.

## 7. Errors and failure

1. A check that cannot run, cannot parse its input or selects no test
   fails, and never reports success (rule 16). Every new `xtask` check
   exits 0 for pass, 1 for fail and 2 for "could not run, verified
   nothing".
2. A missing inventory, manifest, seed list, corpus, baseline or golden
   fixture fails its job.
3. A nightly or weekly failure never blocks unrelated pull requests. It
   opens an issue (5.5, point 9), and the next pull request that touches
   the failing area waits until it is fixed or the owner accepts the issue
   as a blocker.

## 8. Security

1. Test inputs are synthetic: no real cell's names, keys or captures
   holding credentials ([README](README.md), "Sources and provenance";
   ADR 0001).
2. Test-only switches (the simulator's mutation switches, the observer,
   the authenticated test profile, the fault feature of S4) are compiled
   out of every shipped artefact, and a check over the release build of
   each shipped binary proves it. Test-only environment variables carry a
   prefix that a shipped build refuses.
3. A failure that may be a vulnerability is a restricted result (5.5,
   point 9): its reproducer, seed and replay command never reach a public
   log, artefact or issue, and embargoed tests never run in a public job
   (O30, point 4; O49, point 6).
4. Every tool run in CI is pinned by version and, where it is downloaded,
   by checksum; no tool is fetched unpinned.
5. The danger here is a false pass: a test that cannot fail, a tier that
   ran nothing, or a report that counts instead of naming. 5.4, points 1,
   2, 5 and 7, and 5.5, point 1, close them.

## 9. How the invariants are kept

| Quoted decision | Kept by | What fails if it is broken |
|---|---|---|
| Constraint 4 | 5.2.1, D2, D4, D5, D9; 5.3, point 2 | the reject corpus; the fuzz target's assertions; a decoder with no fuzz target (a check lists decoders and targets) |
| C3, point 2 | 5.2.2, M1 to M4 | a planted divergence in a cache type, caught by M1 or M3; the transition-coverage check on an uncovered pair |
| C3, point 6; O45, point 3 | 5.3, point 3; 5.4, points 1 and 8 | an uncaught invariant mutation switch; a case-manifest entry at count zero; the two-process replay test |
| C4, point 7 | `check-deps`; 8, point 2 | a shipped crate depending on `ironafs-sim` or `ironafs-test-support` |
| O22; O46, points 1 and 3 | 5.2.8, N1 to N3 | the refusal-coverage check; an unmapped point, or a missing or failed gate result, keeps the mode out |
| O45, point 6 | 5.3, point 3 | the event-sequence fuzz job missing from the nightly manifest fails the freshness check |
| O47, point 1 | 5.2.2, M1; 5.2.4, C2 | a planted divergence between client and model |
| O47, point 2 | 5.4, points 2 and 9 | a property whose required label is under its minimum; the generator's known-answer test |
| O47, point 3 | 5.2.1, D9; 5.5, point 5 | the fuzz manifest check on a decoder or target without its pair |
| O48, point 5 | 5.2.5, L3 and L4 | a gate result with no venue named is missing evidence |
| O49, point 1 | 5.6, point 1 | a planted skip of work changes a count |
| O49, points 2 and 3 | 5.5 table; 5.6, points 3 and 5 | a timed benchmark in a pull-request workflow (reviewed in the workflow file) |
| Rule 7, no retries | 5.4, point 6 | the retries check on a profile or command with retries above zero |
| Rule 15, printed seeds | 5.4, point 9; 5.5, point 2 | a property failure report with no seed, in the generator's own tests |

## 10. Tests

The procedure's own machinery has these tests, written by the issues that
build each part (10.6). Each has its paired passing case.

- `inventory_check_fails_on_removed_test`, `_on_unlisted_test`,
  `_on_unrecorded_removal_against_main`,
  `_on_dropped_quarantine_against_main`, `_on_quarantine_without_issue`,
  `_on_listed_test_not_run`; `inventory_check_passes_on_marked_quarantine`;
- `retries_check_fails_on_nonzero_retries`;
- `ci_ok_green_on_stale_advisory_freshness`,
  `ci_ok_red_on_missing_required_job`;
- `codec_census_fails_on_omitted_check`;
  `generator_plants_turn_generated_tests_red`;
- `decoder_without_fuzz_target_fails`;
  `fuzz_manifest_fails_on_missing_seeds`, `_on_missing_run_line`,
  `_on_budget_over_timeout`;
- `transition_coverage_fails_on_uncovered_pair`;
- `case_manifest_fails_on_unexercised_case`;
  `crash_outcome_class_at_zero_fails`;
- `refusal_coverage_fails_on_unasserted_variant`;
- `property_label_below_minimum_fails`;
- `race_without_interleaving_witness_fails`;
- `replay_test_red_on_planted_leaks`;
  `golden_trace_fails_on_missing_seed_fixture`;
- `mutation_probe_not_applied_reports_no_result`;
- `counted_bench_fails_on_skipped_work`;
  `bench_harness_records_required_fields`;
- `nightly_freshness_fails_when_stale`;
  `freshness_ignores_rerun_attempt`, `freshness_refuses_truncated_page`,
  `freshness_fresh_on_failed_complete_run`,
  `freshness_incomplete_when_manifest_item_not_run`;
- `scheduled_log_holds_no_planted_seed`, for a passing run, an ordinary
  failure and a restricted failure;
- `unclassified_failure_is_restricted`;
- `fault_feature_absent_in_shipped_build`;
- `server_image_digest_mismatch_fails`;
- the `--self-test` of every new `xtask` command.

### 10.1 Golden vectors

Not applicable: a procedure.

### 10.2 Round-trip properties

Not applicable.

### 10.3 Reject corpus

Not applicable.

### 10.4 Fuzz target

Not applicable.

### 10.5 Acceptance cases

How each step of the procedure is checked:

1. **Start:** a pull request deletes a test and adds none. **Result:** the
   inventory job fails and names the test.
2. **Start:** a codec change stops checking a padding byte. **Result:** a
   generated D6 test fails at that byte's offset.
3. **Start:** a new refusal error variant with no test. **Result:** the
   refusal-coverage check fails and names it.
4. **Start:** a race scenario whose seeds never bring its meeting events
   into its window. **Result:** it fails, reporting the witness counts.
5. **Start:** a nightly run whose fuzz job skipped one manifest target.
   **Result:** the run is incomplete; the freshness check counts the
   previous complete run, and fails once that is too old.
6. **Start:** a scheduled property test fails and prints a seed and a
   replay command. **Result:** neither appears in the job's log, its
   artefacts or the issue the reporter files.
7. **Start:** a nextest profile sets `retries = 1`. **Result:** the retries
   check fails.

### 10.6 Gaps

Every test of section 10 is written by the issue that builds its part, and
none is written yet: the inventory ratchet (#72); mutation testing and the
planted-defect probe (#73); coverage (#74); the scheduled workflows, the
freshness check, the runner and the reporter (#75); the flaky-test rule
and the retries check (#76); the fuzz crate and its manifest check (#45);
the simulator (#79, #96); the timed-benchmark harness (#38); and, planned,
the refusal-coverage check, the differential listing check, the seam
conformance bodies and the release-build check of the fault feature.

## 11. Revisit if

From the working decisions this page rests on:

- **C3:** "the M2a simulator cannot replay a failing schedule from its
  seed, or a measured workload needs more concurrent waits than a bounded
  pool of threads can hold."
- **C4:** "a boundary below forces a dependency that point 4 forbids, the
  first implementation of a crate shows it has no content of its own, or
  no log sink outside Linux still needs a site in `ironafs-os` (then the
  safe log transport could live in a crate without `unsafe`)."
- **O45:** "the simulator cannot reproduce a failure seen against a real
  cell, or keeping its server models in step with observed behaviour
  costs more than the tests return."
- **O47:** "the fuzzing engine's platform limits change, or A12 is
  settled differently when the first fuzz target lands."
- **O48:** "the test cell cannot be run in containers on hosted runners,
  or a case here cannot be automated on a supported OS."
- **O49:** "GitHub's hosted runners gain an OS target the plan needs, or a
  self-hosted runner cannot be isolated as required."
- **L40:** "the maintained OpenAFS 1.8 line moves on (the fixture follows
  it, and the minimum tested patch level is published), or a layer's
  documented freshness bound changes."

This page's own triggers:

- the pull-request tier passes 20 minutes of wall time at its median, or a
  scheduled tier cannot finish its manifest in its capacity;
- the month of measurements is complete (the budgets of 5.5 and the
  coverage ratchet's start);
- a pinned tool is abandoned, changes its licence, or changes a fact
  registered in rows E3 to E8;
- a golden trace fixture passes its size cap;
- fixed hardware for timed benchmarks exists (5.6, point 5);
- the private CI repository exists (5.5, points 3 and 9).

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| T1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; C3, C4, C37 ([part 1](../adr/0004-architecture-1-core.md)); L13, L22, L40, L41 ([part 2](../adr/0004-architecture-2-os-layers.md)); O22, O30, O41, O45 to O49 ([part 3](../adr/0004-architecture-3-operations.md)) | 1, 2, 5, 9, 11 |
| T2 | DECISION | [Coding standards](../coding-standards.md), rules 1 to 3, 5, 7, 8, 15, 16, 20 and 21; [docs/workflow.md](../workflow.md) §1, §2, §3b, §3c, §5 and §6; [ADR 0001](../adr/0001-licence-and-dont-copy.md) | 1, 5, 7, 8 |
| T3 | DECISION | The owner's decisions recorded in issue #63 (section 1's list); the `no_std` decision is also in issue #53 | 1, 5 |
| E3 | DOCUMENTATION | nextest, "Machine-readable output: test lists", register row E3 | 5.4 |
| E4 | DOCUMENTATION | The rustc book, "Instrumentation-based code coverage", Rust 1.100.0, register row E4 | 5.4 |
| E5 | DOCUMENTATION | cargo-llvm-cov, its README and its 0.9.1 release page, register row E5 | 5.4 |
| E6 | DOCUMENTATION | cargo-mutants user guide, pages on nextest, sharding and the output directory, register row E6 | 5.4 |
| E7 | DOCUMENTATION | GitHub Docs, "Events that trigger workflows", section `schedule`, register row E7 | 5.5 |
| E8 | DOCUMENTATION | GitHub Docs, "Dependency caching reference", cache access restrictions and eviction, register row E8 | 5.5 |
| O | OWN | Every point tagged `[OWN]`, with its reason in place | 5 |

Each outside page was read on 2026-11-21 (INSPECTED, workflow §3c), for
facts about the tools only; none is quoted, and none is a source of a wire
fact. No other implementation's source, tests or test data were read for
this page, and none are cited.

### 12.1 Observed behaviour

None.

## 13. Open questions

- **Q1. An encrypted crash payload.** Whether a fuzz crash found by a
  scheduled job before M4 may leave the job as ciphertext encrypted to the
  maintainer's public key, or stays only as its digest until the private
  CI repository exists, is decided with the scheduled workflows (#75).
  This page holds either way: no seed, input or replay command is
  published in plaintext, and a restricted result shows only its digest
  (5.5, points 2 and 9).
- **Q2. Specifications this page points to.** The simulator's
  specification and the seams and scheduling specification (#71) are not
  yet reviewed. Where they set the mechanism (the trace's encoding, strict
  replay, the yield points), this page sets only what the tests must show;
  a difference found when they are reviewed is resolved in whichever is
  wrong, in the same pull request.
- **Q3. Is a cache entry readable outside the repository's workflows?**
  The weekly corpus (5.5, point 6) assumes a cache entry is not published.
  If it is readable by anyone who can read the public repository, the
  corpus is carried another way, settled with the scheduled workflows
  (#75); the tracked-seed floor holds either way.
- **Q4. The priority-change scheduler's bound** (K1) is cited only once
  its paper is registered; until then no probability is claimed for it.

## 14. Review

Not yet reviewed. The record will be in `docs/reviews/`.
