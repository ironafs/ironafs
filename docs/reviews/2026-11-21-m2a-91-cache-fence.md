<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: the fence table and the fence check

## The change

Class S. Issue #91, its second part: the cache's part of the fence of
ADR 0004, C16, point 2, with C25, point 3 and C32, point 5, in
`ironafs-cache`, under the callback specification's 5.6, points 1 to 4,
and the incarnation specification's 5.6, points 3 and 4. The branch
starts from the first part's head, `f46815f`, the base of the reviewed
range.

- `FenceTuple` and `Suspicion`: six values. They are the object's and
  the volume's break counters, the server key's suspect epoch, the
  socket's pause generation, the resume generation, and the context's
  local volume epoch.
- `FenceTable`, one per daemon, behind one lock:
  - each context's break counters, from one sequence that never repeats
    a value;
  - a copy of each context's local volume epoch, and its adoption by the
    install that wrote it;
  - the resume generation and the clock watch;
  - fixed memory, reserved once from a byte budget;
  - exhaustion that fails every check and refuses new tuples.
- `FenceTable::check`, the one check used at install and at serve.
- `EpochSource`: the client-wide suspect-epoch counter.
- Unit tests, a property test against a reference model, with and
  without eviction, and an exhaustive sweep of every four-operation
  sequence of a small domain.
- **Written by** `claude-code:claude-opus-5-5` (a Claude Code subagent):
  the first version and every fix.

## The specifications, reconciled before the first fix

The first version found three disagreements between the specifications
on what the fence compares. The controller had them reconciled in a
research note, which is not published, and a third family attacked that
note before the fix (below). The controller's decision, in plain words:

- The cache's fence has six values: the callback specification's five
  plus the local volume epoch the incarnation specification records.
- The credential and rights generations are the request's ticket's,
  checked at submission (C16, point 3), not the cache's fence.
- The seams specification's overload record is replaced by the callback
  specification's rule. Rx holds what it cannot admit until the affected
  suspect epochs advance at the tick, and the pause generation covers
  what it cannot hold.
- The text of the seams, contract, callback and incarnation
  specifications that says otherwise is to be amended in their own pull
  requests.

## Rounds

The required seats (class S): review by two non-author families. GLM and
GPT took them, read-only, on 2026-11-21 (UTC). Neither ran cargo.
DeepSeek attacked the reconciliation note, an input to round 1's fix.

| Round | Range | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|
| 1, review | `f46815f..a5ffbf4` | glm | glm-5.3 (calibrated on IronAFS, #48) | high | PASS: 1 minor |
| 1, review | `f46815f..a5ffbf4` | gpt | gpt-6.2-sol (uncalibrated on IronAFS) | high | FAIL: 4 major, 3 minor |
| 1, attack on the reconciliation | the note, read against `a5ffbf4` | deepseek | deepseek-flash (uncalibrated on IronAFS) | max | FAIL: 2 major, 3 minor |
| 2, verification | `a5ffbf4..e4da942` | glm | glm-5.3 | high | PASS: 0 findings |
| 2, verification | `a5ffbf4..e4da942` | gpt | gpt-6.2-sol | high | FAIL: 3 major, 1 minor |
| 3, scoped check | `e4da942..74c29eb` | glm | glm-5.3 | high | PASS: 0 findings |
| 3, scoped check | `e4da942..74c29eb` | gpt | gpt-6.2-sol | high | PASS: 3 minor |

GLM and DeepSeek ran outside peak hours.

## The stop decision

Fix round 2 was the last before the stop rule. The scoped check of its
fix passed at both seats, with three documentation minors from GPT. So
the rule applies: fix, record, stop. The minors are fixed in `4b74bad`.
**That fix is not re-reviewed.**

## Findings

Each finding is quoted as the seat wrote it. In DeepSeek's locations,
"[the reconciliation note]" stands for the unpublished note's file.

### Round 1, GPT

> major crates/ironafs-cache/src/fence/breaks.rs:251 | The advertised
> install-and-serve check is incomplete under the checked-in seams
> spec. Section 5.4.7 requires the ticket to capture the
> callback-overload counter and check it at install and submission. It
> also requires pending overload to be folded before taking a ticket.
> Section 5.8.2 names object, volume and server break values,
> overload, credential generation and rights generation. The
> five-value tuple has no overload or rights input. Its ContextKey
> carries credential generation, so the caller must supply the current
> context to check it.

Refuted in part, fixed in part. The overload part is refuted: the seams
specification hands the overload record's form to the callback
specification, which replaced it with Rx's hold, the tick's advance and
the pause generation (callbacks 5.6, point 7; Rx 5.10, points 5 and 6).
The rights generation is refuted: C16, point 2 names break counters
only, and C16, point 3 checks the credential and rights generations at
submission. Fixed in `e4da942`:
- the module documentation says what the tuple leaves out, and why;
- `record` and `check` require the request's current context and
  credential generation;
- the local volume epoch became the sixth value.
GPT accepted the refutation in round 2.

> major crates/ironafs-cache/src/fence/breaks.rs:247 | The clock
> detector can advance its baseline before the one-shot Clocks::Moved
> result reaches check. If one caller consumes Moved and a second
> obtains Steady and enters check first, the second can still read the
> old resume generation. The documented table lock covers the table
> operations and leaves this ordering unspecified.

Fixed in `e4da942`. The table owns the clock watch and the resume
generation, and `check` takes it by `&mut`, so the observation, the
raise and the comparison are one step. Tests:
`check_raises_the_resume_generation_once_per_move` and
`record_consults_the_clocks_first`.

> major crates/ironafs-cache/src/fence/breaks.rs:45 | The tables are
> bounded by record count, but they have no byte budget as required by
> coding standards rule 3. At both permitted maxima, the field payload
> of the two indexes alone is about 1.875 GiB on 64-bit targets,
> before outer keys and B-tree allocation overhead.

Fixed in two rounds. `e4da942` charged records in bytes, which round 2
found was not a true bound on allocation. `74c29eb` reserves the memory
once (round 2, below). A suggestion to size `MAX_NODES` from the budget
was declined: `MAX_NODES` is the node table's bound (C11), and the fence
table has its own. GPT accepted that.

> major crates/ironafs-cache/tests/it/fence_model.rs:274 | The
> eviction oracle records breaks and generation changes but never
> records eviction. With roomy=false, it therefore accepts both a
> correct refusal and a false match after eviction alone. The unit
> tests cover object eviction and recreation, while the volume test
> checks only absence. The exhaustive sweep covers 50,625
> four-operation sequences with eviction disabled and can select only
> the first two tickets. The clock model is serial and reduces pending
> jumps to a boolean.

Fixed in two rounds. `e4da942`:
- the model saw evictions and compared every result exactly;
- the sweep evicted;
- a planted same-key reuse after eviction failed it.
`74c29eb` made the model decide evictions itself (round 2, below).

> minor crates/ironafs-cache/src/fence/breaks.rs:199 | Exhausting the
> sequence during record creation does not latch table exhaustion. A
> record call can spend the final value on its object, evict another
> object, then return Exhausted when it cannot create the volume.
> Checks and records for still-present pairs can continue until a
> break needs a value.

Fixed in `e4da942`: an operation checks that its values are left before
it evicts or makes anything, and a refusal changes nothing but the
exhaustion latch. Test: `record_refused_changes_nothing_but_the_latch`.

> minor crates/ironafs-cache/src/fence.rs:152 | PauseGeneration
> accepts any u64 and compares by equality. Its documentation leaves
> the driver's non-reuse and exhaustion requirements unstated. The
> documented stamp uniqueness also needs to be scoped to the lifetime
> of its issuing table or source because new instances start their
> sequences at 1.

Fixed in `e4da942`. The documentation now says:
- the driver never reuses a pause value on a socket, and a socket bound
  again is a new key;
- stamps are unique within their issuing table or source.
What the driver does when it cannot raise the pause generation is Rx's
(#55).

> minor crates/ironafs-cache/tests/it/fence_model.rs:127 | The
> consistency pass found copied context and file fixture constructors
> in the integration tests. The change otherwise follows the
> neighboring module layout and typed-error style and reuses Property,
> Exhaustive and the cache identity types.

Fixed in `e4da942`: one fixture per test layer.

### Round 1, GLM

> minor crates/ironafs-cache/src/fence/breaks.rs:202-209 (ensure at
> 121-133) | record() mutates the table on a refusal path:
> objects.ensure() draws the last sequence value, inserts a new object
> record and (breaks.rs:126-130) can evict the kind's oldest record,
> and only then volumes.ensure() fails, so record() returns
> Err(FenceError::Exhausted) after having changed state. Reachable
> when next == Some(u64::MAX), the (context,file) object record is
> absent, and the volume record must be made.

Fixed in `e4da942`, with GPT's first minor.

### Round 1, DeepSeek on the reconciliation

> major [the reconciliation note]:432-453, 500-508, 564-568 | The
> lock/atomic amendment and summary say only the sequence counter and
> the lock-free pause/resume generations need SeqCst atomics, but the
> note's own recommended sixth value (the local volume epoch) is
> written under the core's volume lock (volume-incarnation.md 5.6.3:
> 'in one step under the core's lock for the volume') and is read by
> the fence check under the object lock set and fence-table lock, not
> the volume lock. The amendment is scoped to values 'step 3 reads';
> it does not say whether the epoch is copied into the table under the
> volume lock, read under the volume lock, or made a SeqCst atomic.

Fixed in `e4da942`. The epoch is copied into the table: the incarnation
code writes it with `set_volume_epoch` in its own step, under the
volume's lock, taking the table's lock inside it, and the check reads it
under the table's lock with the other values. A separate atomic was
rejected because it would be a second structure outside the lock and the
memory bound.

> major [the reconciliation note]:506-508;
> crates/ironafs-cache/src/fence/breaks.rs:251-274 | The lock design's
> safety argument is stated only for 'a check at submission', and the
> contract amendment covers only 'step 3's reads', but callbacks
> 5.6.4/C32.5 runs the same fence check on every serve: 'On every
> serve of an entry from K, before the answer is given, the cache runs
> the fence check'. BreakCounters::check takes &self and does not
> enforce that the serve caller holds the table lock; the note never
> requires it.

Fixed in `e4da942`: `check` takes the table by `&mut`, so a serve cannot
check without the lock.

> minor [the reconciliation note]:130-131 | The absolute 'So no call
> is ever dropped before the fence moved' is overbroad: rx.md 5.10.5
> says a datagram that would not fit rx.in.held_bytes 'is not held: it
> is point 6's'; 5.10.6 has the machine tell the driver to pause, and
> the driver raises the pause generation before it stops reading. The
> datagram's content is not retained at the moment of the pause.

Right, and a wording matter for the note and the seams amendment (#71):
a datagram Rx cannot hold is fenced because the pause generation rises
before reading stops. No code change.

> minor [the reconciliation note]:170-186, 214 | The sixth-value rules
> cover an 'unrecorded' volume (5.2.5) but not a volume with no record
> at send that gets the 'new' outcome from this fetch's own
> observation (volume-incarnation.md 5.4.2, 5.6.1). A tuple recorded
> with no/Stamp::NONE epoch will fail the check at the install that
> creates the record.

Fixed in `e4da942`, and made fail-closed in `74c29eb`. A tuple with no
epoch fails, except through `adopt_volume_epoch` at the install whose
own observation wrote the epoch.

> minor crates/ironafs-cache/src/fence/breaks.rs:192-217 (note's §5
> 'Correct' list at 500-503) | BreakCounters::record checks the
> break-counter table's exhausted flag but not whether
> ResumeGeneration::current() is Stamp::NONE. After the resume
> generation is exhausted, record returns a tuple with resume: NONE;
> check then compares NONE with NONE via Stamp::matches, which is
> false, so the call is sent under a tuple that can never match.

Fixed in `e4da942`: `record` refuses with `Exhausted` when the resume
generation is exhausted. Test:
`record_refuses_when_the_resume_generation_is_exhausted`.

### Round 2, GPT

> major crates/ironafs-cache/src/fence/table.rs:234,394 | The fence
> samples pause before it checks the clocks. `ServerValues` is passed
> by value, so even a caller holding the table lock must load the
> pause generation before `observe_clocks()`. The Rx driver can raise
> it during the clock read. `check()` then compares the old pause
> value and can return clear. Callbacks §5.6.2 requires the pause load
> after the clock comparison (`callbacks.md` at b85db41:798).

Fixed in `74c29eb`. `Server` carries the driver's atomic, and the table
loads it after the clock observation in `record` and `check`. Test:
`record_and_check_load_the_pause_generation_after_the_clocks`, with a
clock watch that raises the pause during its comparison.

> major crates/ironafs-cache/src/fence/table.rs:35,269,338,366 |
> `None` means the table lacks an epoch copy. Adoption treats it as
> proof the fetch started before any evidence epoch existed. For
> example, let fence pressure evict the copy of E1 while the evidence
> record survives. A fetch then records `None`. Evidence reclaim can
> abandon E1 while the fetch is in flight. The fetch's own reply can
> now create E2 as a `new` observation, write it into the unchanged
> fence record, adopt it, and pass all six comparisons. Reclaim and
> `new` do not require a volume break (`volume-incarnation.md` at
> 11cec86:433,495,547).

Fixed in `74c29eb`. Evicting a record that holds an epoch copy latches
the table, and a latched table refuses every adoption. So a tuple with
no epoch adopts only when no copy can have existed. Test:
`adoption_is_refused_once_an_epoch_copy_was_evicted`, which failed
before the fix.

> major crates/ironafs-cache/src/fence/table.rs:90,103,510 | G3
> remains open. The three-times-entry charge is not an allocation
> upper bound. One object and one volume fit `MIN_FENCE_BYTES` and
> allocate three B-tree roots with occupancies of 1, 1 and 2. Rust
> 1.100 reserves 11 key and value slots in each leaf. Those arrays
> alone exceed the charge for this state, before node headers or
> temporary allocations.

Fixed in `74c29eb`, by construction:
- one slab and two sorted indexes, reserved once with
  `try_reserve_exact` and sized from the budget (at most 16 MiB);
- no operation allocates.
A worst-case charge per B-tree node was not used, for two reasons. It
rests on the standard library's node layout, which is not a stable
contract. And measuring allocation would need a global allocator, which
needs `unsafe`, forbidden in the workspace. A hash was not used either:
hashing keys read from the network would need a key from the randomness
seam.

> minor crates/ironafs-cache/tests/it/fence_model.rs:239,280,483 | The
> oracle takes eviction decisions from the implementation and can
> treat a just-issued tuple as evicted. If `record()` evicts its own
> existing object while recreating the volume, the model marks that
> new tuple's object as changed and can accept the failure. The
> targeted unit test protects an in-use volume
> (`crates/ironafs-cache/src/fence/table/tests.rs:166`). The
> four-operation sweep has four selector values, but a check or
> adoption can address at most three previously recorded tickets.

Fixed in `74c29eb`. The model mirrors the value sequence and the
eviction rule, predicts recorded values, and asserts that the table
holds exactly its records. The sweep picks among three tuples, all four
operations can leave.

### Round 3, GPT

> minor crates/ironafs-cache/src/fence/table.rs:58 | The latch
> documentation promises one refetch for the first fetch of each
> volume without a copy. The same context and volume can lose its copy
> again, including between a failed attempt and its retry.

Fixed in `4b74bad`: the documentation says a refetch follows whenever a
fetch is sent while the context holds no copy, including after an
eviction between an attempt and its retry, within C16, point 2's bound.

> minor crates/ironafs-cache/src/fence/table.rs:73 | The documentation
> says network keys cannot steer table cost. The sorted position of a
> key determines how many entries an insertion shifts, as shown at
> crates/ironafs-cache/src/fence/table.rs:620.

Fixed in `4b74bad`: a key's place in the order moves the shift's cost
within the stated bound, never past it.

> minor crates/ironafs-cache/tests/it/fence_model.rs:406 | The model
> checks the object stamp, volume stamp, and volume epoch on every
> recording. It exercises the suspect epoch, pause generation, and
> resume generation through later selected checks, although the module
> documentation claims it predicts every recorded value.

Fixed in `4b74bad`: the documentation says which values a recording's
prediction covers, and that later checks cover the rest.

### Uncertain items, and what was done

- No seat ran cargo. The execution evidence is the implementer's, below.
  GLM recomputed the sweep's size independently each round.
- The planted faults are described in the pull request and were
  reverted, so the seats verified by reading that the tests detect them.
- Caller-side rules the crate cannot enforce are documented, and their
  specification text is owed by other issues:
  - `set_volume_epoch` in the incarnation step;
  - `adopt_volume_epoch` only at the install whose observation wrote the
    epoch;
  - the suspect epoch read under the table's lock;
  - the driver's raise of the pause generation.
- Two guards in eviction, and a `NoMemory` branch in making a record,
  cannot fire under the table's invariants (GLM). The model would catch
  any drift. No change.
- DeepSeek's questions about the advance tick's atomicity, and about the
  window before a tick, are the callback specification's (#66).

## Open questions

- Spec amendments, owed in their own pull requests:
  - callbacks (#66): the sixth value, the ticket split, the counter's
    read-modify-write wording, and the suspect epoch's lock;
  - incarnation (#78): step 3 and the wide-suspicion list;
  - seams (#71): the overload record replaced;
  - contract (#67): the ticket, the lock-or-atomic rule, and the push of
    fence-epoch advances to out-of-process bindings.
- The callback specification's Q10, put to this issue, stays undecided:
  may a read whose fetch keeps failing on its object's counter alone be
  answered from a fetch sent after the read began?
  `Suspicion::is_narrow` is there for the policy.

## Evidence

- Every unit test failed first, against a compiling stub or a planted
  fault, and every model test failed on planted faults, each reverted.
  The pull request lists them (EXECUTED by the implementer, 2026-11-21
  UTC, development Mac).
- At `74c29eb`, without pipes, each exited 0:
  - `cargo fmt --all --check`;
  - `cargo clippy --all-targets --workspace`;
  - `cargo nextest run --workspace --retries 0 --no-tests fail` (536
    tests);
  - `cargo test --doc --workspace`;
  - `cargo xtask check`, `cargo xtask check-pure` and its `--self-test`.
- The sweep ran 279 841 sequences.
- At `4b74bad`, a documentation-only change: `cargo fmt --all --check`,
  `cargo clippy --all-targets --workspace` and `cargo nextest run
  --workspace --retries 0 --no-tests fail` (536 tests) exited 0. `cargo
  xtask check` at the head of this branch is recorded in the pull
  request.
