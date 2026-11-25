<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The `Rpc` and `Disk` seams and the scheduling model

Status: draft (2026-11-21). Raised in issue #71. Amended with the
interface types (#71's second part) to state the two `xtask` checks as
built, with their limits: 5.2, point 5 (the token rules' place among
the confinement checks, and the pure-crate rule: where a seam's name may
appear, a variant with a seam's name exempt only at its declaration and
through `Self::`, a production implementation's import and `impl`, the
refused renames and shadows, `path` attributes and `include`), 5.3,
point 2 (the call-site check now covering
`ironafs-time`, which `cfg(test)` code it leaves out, the sampling
function found by its full path, the exact fresh-sample form and its
`Clock` parameter test, the second net's two configuration files, and
which plants are judged by line), and section 9's plants.

Kind: behaviour (interface shapes and orderings, no byte layout). Class:
S. Every operation, the cache's durable writes and the simulator's gate
rest on it, and a mistake here is a concurrency, durability or
consistency failure ([README](README.md), "Review class per kind").

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (S1, S2 and so
on) are in section 12.

## 1. What this implements

Ids are ADR 0004's. Quotations are word for word.

ADR 0004, C3 (**Working**). Its trigger: "Revisit if: the M2a simulator
cannot replay a failing schedule from its seed, or a measured workload
needs more concurrent waits than a bounded pool of threads can hold."
Points 1 and 3 to 6:

> 1. Rx, XDR, the RPC codecs and the security classes are pure state
>    machines: bytes, time and randomness in, actions out. They open no
>    socket, read no clock and draw no random numbers.

> 3. The operation code that uses them is written in ordinary blocking style
>    against four injected interfaces: `Rpc`, `Clock`, `Rng` and `Disk`.
> 4. Small blocking drivers on standard-library threads perform the real
>    I/O (C39).
> 5. No async runtime is used, neither a third-party one nor one of our own.
> 6. One deterministic simulator drives points 1 to 3 through the injected
>    interfaces. It is a deliverable of M2a (O45).

ADR 0004, C4 (**Working**), point 1, its first sentence; point 4, its
first and third items; point 7, its third sentence:

> **Pure crates** have no I/O and no `unsafe`, and are fuzzed where they
> decode:

> - no pure crate depends on a driver or on `ironafs-client`;

> - `ironafs-rx` knows nothing of AFS.

> The simulator is a test-only workspace crate, `ironafs-sim`, which is
> never shipped.

ADR 0004, C16 (**Firm**), points 2 and 3, the parts this page carries:

> If any changed while it was in flight, the reply is discarded and the
> fetch retried a bounded number of times, then fails with `Stale`. It is
> never handed to the request that started it.

> The last check is at **submission**: the binding checks the reply's
> context, generation, grant and node state in the same step that writes
> it to the kernel or to the SMB connection.

ADR 0004, C24 (**Firm**), point 1, its first sentence:

> 1. **Authoritative evidence** is an observation protected by a key no
>    local user can determine, bound to the file or volume and to the event
>    it establishes.

ADR 0004, C27 (**Working**), point 2, its last two sentences:

> A clean chunk is written to a temporary name and renamed, with a header
> and a checksum and no sync. A bad chunk is a miss, never an error.

ADR 0004, C28 (**Firm**), points 3, 4 and 6:

> 3. A **deferred store** (a record left by a stalled or failed commit, a
>    restart or an upgrade, or an upload of a change seen by the Windows
>    replica, L27, point 3) is never stored automatically. It is kept for
>    explicit recovery.
> 4. **A lost reply** (the store was sent and no answer came) is ambiguous.
>    The record is quarantined as a conflict and the server's file is left
>    as it is. It is never taken as applied and never replayed blindly.
>    Completing an operation already sent before a timeout is not a new
>    store: its outcome is read back.

> 6. A record is written durably, with a checksum, **before** the write is
>    acknowledged to the application.

ADR 0004, C29 (**Working**), point 9:

> 9. A record is made durable by writing the file, syncing it, and syncing
>    its directory. Removal is the reverse.

ADR 0004, C32 (**Working**), point 1, its second sentence; point 4, its
last sentence; point 5, its last two sentences:

> It is earned only by a reply from that server to the
> client's own keepalive call (point 4), received on the client's
> callback socket (C30), no older than **E**, and since the last network
> change or resume.

> Trust lapses when no reply has arrived within
> E.

> Each serve compares the clocks of C37, and a
> jump marks every callback suspect before the answer is given.

ADR 0004, C37 (**Working**), point 4, its last three sentences, and point
5, its last sentence:

> A detected suspend, a regression of a monotonic clock, or a
> jump of wall time past the threshold marks callbacks suspect (C32).
> Wall time can only lower trust. A regression of a monotonic clock, or
> a failed sample, also treats every boot-clock deadline (callbacks,
> tokens, location lifetimes, locks) as possibly expired: a lock counts
> as lost until it is established again with the server, and a token is
> checked again before its next use.

> If a later read fails,
> outstanding deadlines expire, callbacks become suspect, one event is
> logged per episode, and no new deadline is set until a valid sample
> returns.

ADR 0004, C39 (**Working**), points 3 and 4:

> 3. Workers wait on one-shot reply slots with a deadline counted from
>    arrival and a cancel token. Every queue is bounded in bytes.
> 4. **No lock is held across an RPC wait, a disk write or a kernel
>    notification.** Lock order is server, volume, object, chunk.

ADR 0004, C46 (**Working**), points 3 and 4:

> 3. **An AFS abort code is classified by the RPC and the server that
>    returned it, never by its number alone.**
> 4. **A failure never looks like success:** never an empty or short
>    listing, zero bytes or a made-up attribute.

ADR 0004, N7 (**Firm**), point 2, its third sentence:

> When an applicable restriction rises (a pin, a floor, a policy),
> connections below it are closed, and sends queued on them are fenced: a
> request not yet sent runs through this function again; a mutation
> already sent follows the lost-reply rule (C28).

ADR 0004, N32 (**Firm**), point 6, its first sentence:

> 6. The journal's specification fixes an independently verifiable commit
>    boundary, made durable after each record (C29, point 9).

ADR 0004, O45 (**Working**), point 5:

> 5. The rule the simulator imposes on the code: threads live only in thin
>    drivers, and the core never spawns or sleeps (C3, C39).

Coding standards, rule 7, its second sentence:

> No direct `Instant::now`, `SystemTime::now` or random-number calls
> outside an injected clock and random source.

Also cited as context, not as invariants this page keeps, so with no
quotation and no section 9 row (the specifications README asks a
quotation, its citation rule 2, and a section and test, rule 4, only of
an invariant the page keeps): C9,
point 3; C26, points 2 and 4; C30, points 2 and 3; C31; C37, points 1
and 3; O41, points 1, 3, 4 and 6; O45, points 2 and 3; O47, point 3;
O48, point 1; O50, point 11, all Working; and ADR 0005, decision 2, the
closed list of `ironafs-os` sites (**Firm** that it is closed, Working
in each row).

**This page's reading of C3, C4 and O45.** The owner decided the
scheduling model (issue #71): one standard-library thread per task with a
baton, as a test-only thin driver in `ironafs-sim`, never in the core.
This page reads C3, points 3 to 6, C4, point 7, and O45, point 5, as
allowing it. The threads are the simulator's driver, not the core's; the
live stacks are bounded; the scheduler alone chooses the next task; every
blocking point yields through the simulated driver; and the core runs
ordinary blocking operation code and never spawns or sleeps. C4, point 2,
says the composition crate "may use threads": this page reads that as
running on threads that `ironafsd` or `ironafs-sim` spawned and sharing
state between them, never spawning one. It is a reading, not an
amendment. If the model cannot meet C3's trigger, C3 reopens (section
11).

**What replay rests on.** A baton replays a failing schedule byte for
byte only if the scheduler controls every observable transition and each
task is deterministic between its yields. Both are requirements here
(5.6, point 11), each with the check that enforces it.

Section 9 shows where each quoted decision is kept and which test fails
if it is broken.

## 2. Scope

1. **Covered:** the `Rpc` and `Disk` interfaces, with their crates,
   shapes, error sets and deadlines; the one blocking primitive,
   `Clock::wait`, and the waiting primitives built on it; the scheduling
   model under the simulator (tasks, yield points, the hand-off protocol,
   choosing, time, crash and restart, stuck detection, replay, trace
   rules); the crates that perform real RPC and real file I/O; the rule
   that confines I/O; the production mapping of C39's threads; and the
   obligations each seam's conformance tests carry.
2. **Not covered, and where it is covered:**
   - the `Clock` and `Rng` reads and the time types: `ironafs-time`'s
     documentation (issue #58). This page adds two operations to `Clock`,
     `sample` and `wait`, with the types they use, and moves the episode
     and the doubt it records from the code that holds the clock into the
     `Clock` implementation (5.3);
   - the simulator's world, its fault list, its invariants, its streams
     and its scenarios: the simulator's specification (planned) and its
     implementation (issue #79). This page fixes only the scheduling
     model, the persistence model of the simulated disk, and the trace
     rules that replay rests on;
   - the Rx state machine, its security-class interface and the
     connection key (C30, point 3): the Rx specification (issue #55) and
     `ironafs-rx` (issue #90);
   - the state directory's layout, its record formats and its start
     commit: the specifications of those records;
   - the contract between the core and the layers (C7), the request
     table and how a production binding makes its submission check:
     the contract and its specification (the contract's crate is issue
     #67);
   - the clock of an operating-system request's deadline: the contract
     specification (section 13, Q3);
   - the store path, its commit and stall deadlines and streamed calls:
     M3 (section 13, Q4);
   - restoring a virtual machine from a snapshot while the client runs:
     C37, point 7, and the simulator's specification.

## 3. The design in short

1. **Four seams, no fifth.** `Clock` and `Rng` are in `ironafs-time`,
   `Rpc` in `ironafs-rx`, `Disk` in `ironafs-cache`. Each is a trait with
   no logic; the types it names are values, or the `Watch` trait of
   `ironafs-time`. A seam in a pure crate never names a concrete waiting
   primitive, so no pure crate depends on `ironafs-sync`.
2. **One blocking primitive.** Every blocking point of the core is
   `Clock::wait` on a watched flag with a deadline and a cancel token, an
   `Rpc` call or a `Disk` call. The waiting primitives (cancel tokens,
   one-shot slots, byte-bounded queues, locks) are concrete types in a
   small standard-library crate, `ironafs-sync`, whose only blocking step
   is `Clock::wait`. The native waiting lives in the `Clock`
   implementation, never in the core.
3. **The core exposes loops; others run them.** The core's loops are
   ordinary functions. `ironafsd` spawns the threads that run them in
   production, `ironafs-sim` under test.
4. **A baton under the simulator.** Each task is a real thread, and a
   baton lets exactly one run. A task gives the baton back only at a
   yield point, under a hand-off protocol that tolerates every order of
   grant and park. The scheduler draws the next task or world event from
   the seed and logs the decision with the identity it chose.
5. **Pure crates never call a seam.** They hold formats, decisions and
   plans. Operation code in `ironafs-client` carries them out, and
   reaches the disk only through `Disk`.
6. **One owner per call.** The `rx` loop alone moves a call between its
   states. It arbitrates the call's deadline, cancel and abandon inside
   one decision guard per call, so each send and each settlement is
   ordered against them. `NotSent` means transmission is ruled out;
   anything after a possible send is ambiguous, an abort included.
7. **A disk that can fail as disks fail.** `Disk` is fine-grained enough
   to express a torn write, a lost rename, no space, a read error, a sync
   error, a sync or rename that took effect yet returned an error, and a
   crash at any call. The simulated disk models two ends of a run: a
   daemon restart (the page cache survives) and a host crash (memory is
   lost).
8. **Replay is strict.** A failing schedule replays from its seed, byte
   for byte, in a separate process; golden traces catch a behaviour
   change between commits; errors in a trace come from a fixed
   vocabulary.

## 4. Terms

- **Core:** `ironafs-client` and the pure crates of C4, point 1.
- **Seam:** one of the four interfaces of C3, point 3.
- **Driver:** a crate that performs real I/O for a seam (C4, point 3), or
  the simulator's implementation of one.
- **Task:** one thread under the simulator. Each executor worker, each
  store-pool worker, each fixed loop of C39, point 1, and each model
  front-end request source is one task. An operation runs on the worker
  task that took it from the executor.
- **Yield point:** a place where a task hands the baton back (5.6,
  point 3).
- **Step:** what one task does between two yield points, or one world
  event.
- **Decision:** the scheduler's choice of the next step.
- **World event:** a step of a simulated server, the network, the disk or
  the clocks. It runs on the scheduler's thread, with no stack of its
  own.
- **Grant:** the scheduler's permission for one task to run until its
  next yield. **Hand-off:** a yield or an end that returns the baton.
- **Watch:** a flag that a wait reads, with a list of registered wakers.
  A **signal** is a watch that a producer raises; a **cancel token** is a
  watch that an owner fires.
- **Plan:** a pure value listing `Disk` steps in order.
- **Incarnation:** one life of the daemon under test, from start to crash
  or stop.
- **Daemon restart:** the daemon's process ends while the operating
  system keeps running (the same boot). **Host crash:** the machine
  stops, and memory is lost.
- **Cut:** the instant a simulated crash takes effect.
- **Call owner:** the `rx` loop, which alone moves a call between its
  states (5.4, point 4).
- **Sent:** the transport accepted the first packet of a call's request,
  inside the call's decision guard (5.4, point 4).
- **Contract request:** a request from the operating system, through a
  layer, that the core must complete once.
- **Completion owner:** the part of the ingress side that completes a
  contract request at its deadline, independently of the worker serving
  it (5.7, point 4).

## 5. Specification

### 5.1 Layout

Not applicable: this page fixes no byte format. The trace's canonical
encoding is the simulator's specification's.

### 5.2 Crates and dependency rules

1. **Placement of the interfaces.**

   | Item | Crate | Why |
   |---|---|---|
   | `Clock` (with `sample` and `wait`), `Rng`, instants, deadlines, `Deadline`, `Waited`, `Stamp`, `Doubt`, the `Watch` and `Waker` traits, `WatchRef`, `Registration`, `WatchFull` | `ironafs-time` | C4, points 1 and 7. Traits and values only, so the crate stays pure, `no_std` and free of dependencies [ADR 0004 C4.1: Working] |
   | `Rpc`, `RpcError`, `Reply`, `ReplyEvidence`, `Incoming`, `Response`, `Peer`, `ServiceId`, `ConnId`, `AddrFamily`, `EndpointGeneration`, `SecurityObject` | `ironafs-rx` | its vocabulary is peers, services, connections, security objects and bytes, so "`ironafs-rx` knows nothing of AFS" holds [ADR 0004 C4.4: Working] |
   | `Disk`, `DiskError`, `DiskOp`, `EntryKind`, `StorePath`, `StoreDir`, `StoreName`, `ReadRange`, `Durability`, `Space`, `Plan` | `ironafs-cache` | the state directory's owner (C4, point 7), beside the plans that use it [ADR 0004 C4.7: Working] |

   A trait in a pure crate is a definition, not a call: `Clock` already
   sits in `ironafs-time` the same way. C46, point 3's classification of
   abort codes is AFS logic and stays in `ironafs-client`. `ironafs-rx`'s
   public items name no type of `ironafs-proto` [OWN: keeps C4, point 4].
   Byte counts in the seams are plain `u64` (section 6), so no seam names
   a counting type from another crate.

   **Pure crates name no waiting primitive.** A pure crate is `no_std`,
   and its dependencies are pure crates only; `xtask`'s dependency check
   allows `ironafs-rx` only `ironafs-xdr`, `ironafs-time` and
   `ironafs-crypto` [ADR 0004 C4.1: Working; the owner's `no_std`
   decision, which `xtask`'s pure-crate check enforces]. So `Rpc` takes
   its cancel token as a `WatchRef`, a shared handle on `ironafs-time`'s
   `Watch` trait, exactly as `Clock::wait` takes a `&dyn Watch`; the
   concrete `Cancel` stays in `ironafs-sync` and hands out that handle
   (5.3, point 5) [OWN: the smallest change that keeps the trait in its
   pure crate; a non-pure home for `Rpc` would need a further C4
   amendment and would put the trait outside the crate whose types it
   names, and a concrete `Cancel` needs a mutex, which `core` lacks].

2. **Placement of the waiting primitives.** `Signal`, `Cancel`,
   `OneShot`, `Queue` and `Lock` are concrete types with a mutex inside,
   and `core` has no mutex, so they cannot live in a `no_std` crate. They
   live in **`ironafs-sync`**, a small standard-library crate with no I/O
   and no `unsafe`, used by `ironafs-client`, `ironafs-net` and
   `ironafs-sim` [OWN: the driver `ironafs-net` needs `Queue` for its
   ingress and must not depend on the core above it; the simulator must
   not pull the whole client for a mutex]. It depends on `ironafs-time`
   (each primitive is a `Watch`, and waits through `Clock::wait`), and no
   pure crate depends on it. Its one standard-library mutex per primitive
   is the allow-listed exception of point 5. C4 has no row for it;
   section 13, Q2, names the amendment.

3. **Implementations.**

   | Seam | Production | Simulator |
   |---|---|---|
   | `Clock` (with its `Waker`) | `ironafs-os` (C37) | `ironafs-sim` |
   | `Rng` | the cryptographic random source behind `ironafs-crypto` (C5, point 1) | `ironafs-sim` |
   | `Rpc` | `RxRpc` in `ironafs-client`, over the `rx` thread in `ironafs-net` | the same `RxRpc`, over the simulated `rx` task |
   | `Disk` | `ironafs-disk` (point 4) | `ironafs-sim`'s simulated disk |

   The seeded `Clock` and `Rng` in `ironafs-test-support` serve unit
   tests. The seeded clock's `sample` keeps the doubt counts and episodes
   of 5.3, point 2, from its scripted reads. Its `wait` advances the
   deadline's own clock by the time remaining, with checked arithmetic,
   then evaluates 5.3, point 4, once more, the deadline's latched doubt
   included; an overflow returns `ClockFailed`, never `DeadlinePassed`.

4. **Real RPC** is performed by `ironafs-net`: the UDP sockets and the
   `rx` thread (C4, point 3; C30, points 1 and 2). **Real file I/O** is
   performed by a new thin driver crate, **`ironafs-disk`**, over
   `std::fs`, calling `ironafs-os` only for what the standard library
   cannot do safely: a stronger flush where O41, point 4, qualifies one,
   free space, the root-relative, no-follow lookups of 5.5, point 2, and
   the sync of the root's parent at construction (5.5, point 6) [OWN: a
   separate crate keeps the safe file logic out of the `unsafe` crate's
   audit, and gives the production driver and the simulated disk one
   shared conformance body (5.9)]. C4, point 3, has no row for it;
   section 13, Q2, names the amendment. ADR 0005's closed list names the
   free-space query on Linux only, and none of the other calls; section
   13, Q17, names that amendment.

5. **Confinement.** The core may use from the standard library only what
   cannot block, cannot observe the host and cannot carry hidden order.
   Each rule is checked on resolved paths (the lint's disallowed-paths
   lists, which see through re-exports and type aliases), by the
   dependency check, or, where neither reaches, by a token rule of
   `xtask`'s `check-seams` that fails closed (its rules are stated where
   they are used, with their limits), and each has a planted self-test, including one
   through a re-export and one through a wrapper in another workspace
   crate [OWN: a seam the scheduler cannot see breaks replay]:
   - pure crates call no seam and hold no seam value. They hold formats,
     decisions and plans, and reach the disk only through a `Plan` that
     operation code runs through `Disk`. This is checked at the call
     sites, separately from the pure-crate build, by `xtask`'s
     `seams::pure_crate_calls_no_seam`, on each pure crate's non-test
     library source as tokens. A seam's name (`Clock`, `Rng`, `Rpc`,
     `Disk`) may appear only:
     - after `trait` in its home crate (its definition), and that trait's
       body holds no block, so it provides no method and calls nothing;
     - in a `pub use` item of its home crate's library root, not
       followed by `as`;
     - in the pure crate that point 3 names as the seam's production
       implementation, for that seam only, in a private `use` of the
       trait from its home crate, not renamed, and as the trait of an
       `impl Name for Type` item (today `ironafs-crypto`'s `Rng`; any
       other use of a seam there is refused);
     - as an enum variant, in two places only: its declaration in an
       `enum` body, and `Self::Name` in a file that declares a variant
       `Name`. Every other path to such a variant (`Owner::Name`,
       `home_crate::Owner::Name`, `::Owner::Name`) is refused.

     The check resolves no path. A token rule cannot tell which item a
     path such as `Owner::Name` reaches (a module, a renamed crate or an
     alias can stand where an enum's name does), so no path to a seam's
     name is exempt, and neither exemption can name a seam: a
     declaration in an `enum` body is a variant, and `Self` is a type,
     so `Self::Name` is one of its variants or associated items, never a
     trait, and an associated item named like a seam is refused where it
     is declared. Code outside the declaring file that needs the variant
     uses a method or an accessor of the enum instead. Today the one
     variant with a seam's name is `DeadlineError::Clock`, which pure
     crates reach only through `Self::` in its own file. The check also
     refuses, in a pure crate, a `use` or `extern crate` item that names
     a home crate with `as` or `*`, an item named like a home crate
     (`mod`, `enum`, `struct`, `union`, `type`, `trait`, `fn`, `const`,
     `static`), and a dependency whose name in cargo's resolved graph
     differs from its package's name (with `-` read as `_`, whatever the
     manifest's spelling); with no path exempt, these only keep a home
     crate's name meaning that crate. In every crate the checks read, an
     attribute holding `path = ...` (directly or inside `cfg_attr`) and
     the identifier `include` anywhere (as `check-pure` refuses it, so
     a `use` cannot rename the macro) are refused, so no compiled file is
     out of their sight; a macro that assembles either from fragments is
     not seen (the macro limit of 5.3, point 2);
   - no pure crate depends on `ironafs-sync`, `ironafs-net`,
     `ironafs-disk` or `ironafs-sim` (the dependency check's pure-crate
     rule, which already refuses any dependency outside the pure set);
   - the core names none of: `std::fs`; `std::net`; `std::io`'s standard
     streams; `std::thread` (all of it: `spawn`, `Builder`, `scope`,
     `sleep`, `park`, `yield_now`, `current`, `available_parallelism`);
     `std::sync::{Mutex, RwLock, Condvar, Barrier, Once, OnceLock,
     LazyLock, mpsc}`; `thread_local!`; `std::env`; `std::process`;
     `std::time::{Instant, SystemTime}`;
     `std::collections::hash_map::RandomState` and the hashed collections
     with their default hasher (an ordered map is used, or a hasher
     seeded through `Rng`);
   - the core declares no `static` with interior mutability, atomics
     included, so no state outlives an incarnation;
   - one exception, allow-listed by path: the internal mutex of each
     `ironafs-sync` primitive, held only to read or write its flags and
     storage, never across a wait. Under the baton it is never contended,
     because only one task runs;
   - the core's dependency closure is workspace crates that pass the
     same lint, plus third-party crates the dependency decisions allow
     (none of which the core calls for I/O); a new transitive dependency
     or feature fails the check until it is reviewed;
   - no shipped crate depends on `ironafs-sim` or `ironafs-test-support`;
   - `ironafs-net`, `ironafs-disk` and `ironafs-sim` do not depend on one
     another.

   What a lint cannot see (an address, or a hashed order reaching a
   decision by arithmetic) is caught at run time by strict replay (5.6,
   point 11).

### 5.3 Waiting: the one blocking primitive

1. **`Clock` gains two operations** beside its three reads, and the types
   they use:

   ```rust
   /// In `ironafs-time`, which is `no_std` with `alloc`.
   /// The doubt counts of point 2, one per monotonic clock.
   pub struct Doubt { pub boot: u64, pub awake: u64 }

   /// One sample as C37, point 4, takes it (an awake read and a wall read
   /// bracketed by two boot reads, all succeeding), with the doubt counts and
   /// the episode count after this sample's own checks (point 2).
   pub struct Stamp {
       pub boot: BootInstant, pub awake: AwakeInstant, pub wall: WallTime,
       pub doubt: Doubt, pub episodes: u64,
   }

   /// A deadline on one monotonic clock, with a doubt count for that
   /// clock latched when it was built. Its fields are private to
   /// `ironafs-time`, so outside the crate it is built only by the two
   /// constructors below, and never by a literal, a destructured copy or
   /// an edited count.
   pub struct Deadline { /* private: which clock, the instant, the latch */ }
   /// `Clock` is a failed sample, which the sampling function of point 2
   /// reports; the constructors themselves return only `FromAfterNow`
   /// and `Overflow`.
   pub enum DeadlineError { Clock(ClockError), FromAfterNow, Overflow }
   impl Deadline {
       /// Pure: calls no seam. `now` is the sample the caller has just
       /// taken, and `from` the request's recorded arrival or a reply's
       /// `first_sent` (point 2 says who may call this, and how that is
       /// checked). `Err(FromAfterNow)` if `from` is newer than `now` on
       /// the boot clock: a higher boot doubt count, or a later boot
       /// instant. The instant is `from`'s boot instant plus `d`
       /// (`Err(Overflow)` if it overflows, section 6); the latched count
       /// is the lower of `from`'s and `now`'s boot doubt counts.
       pub fn boot_after(now: &Stamp, from: &Stamp, d: Duration)
           -> Result<Self, DeadlineError>;
       /// The same on the awake clock, its instant and its doubt count.
       pub fn awake_after(now: &Stamp, from: &Stamp, d: Duration)
           -> Result<Self, DeadlineError>;
       /// Read-only views, for `Clock::wait` implementations and the call
       /// owner's arbitration.
       /// `ClockKind::Boot` or `ClockKind::Awake`, never `Wall`.
       pub fn clock(&self) -> ClockKind;
       pub fn boot_at(&self) -> Option<BootDeadline>;
       pub fn awake_at(&self) -> Option<AwakeDeadline>;
       pub fn latched_doubt(&self) -> u64;
   }

   pub enum Waited { Cancelled, ClockFailed, DeadlinePassed, Signalled }

   /// A flag with a bounded list of registered wakers (point 3).
   pub trait Watch: Send + Sync {
       fn is_set(&self) -> bool;
       /// Refused, never a panic, when the list is full.
       fn register(&self, waker: Arc<dyn Waker>) -> Result<Registration, WatchFull>;
       fn deregister(&self, registration: Registration);
   }
   pub trait Waker: Send + Sync { fn wake(&self); }
   /// A shared handle on a watch, for a token that outlives one borrow.
   pub type WatchRef = Arc<dyn Watch>;

   pub trait Clock: Send + Sync {
       fn boot(&self) -> Result<BootInstant, ClockError>;
       fn awake(&self) -> Result<AwakeInstant, ClockError>;
       fn wall(&self) -> Result<WallTime, ClockError>;
       /// Takes one sample and applies point 2's checks.
       fn sample(&self) -> Result<Stamp, ClockError>;
       /// Waits by the protocol of point 3; the outcome by point 4.
       fn wait(&self, signal: &dyn Watch, deadline: &Deadline, cancel: &dyn Watch) -> Waited;
   }
   ```

   `wait` is an operation of `Clock`, not a fifth seam: every wait is
   bounded by a deadline on one of the clocks, and the simulator must own
   both virtual time and the wake-up to keep the baton exact [OWN, a
   reading of C3, point 3]. No deadline counts on wall time (C37, point
   1). There is no wait without a deadline: a loop with nothing due waits
   to a horizon and loops. A `Registration` is a number the watch assigns
   from its own counter, so deregistration depends on no address.

2. **The `Clock` keeps the episodes and the doubt.** The `Clock`
   implementation remembers its last valid sample and keeps two doubt
   counts, one per monotonic clock, and an episode count. `sample` and
   every read move them as follows [ADR 0004 C37.4, C37.5: Working]:
   - **a failed read** of any clock: both doubt counts move, and an
     episode is counted if none is open (an episode of failed reads lasts
     until a valid sample returns, so one event is logged for it, C37,
     point 5); `sample` returns the error, so no new `Stamp` exists, and
     the sampling function that builds every deadline (below) returns
     that error before it reaches a constructor, so nothing is built
     until a valid sample returns (C37, point 5);
   - **a regression** of either monotonic clock against the last valid
     sample: the boot count moves, and one episode is counted. The awake
     count does not move: C37, point 4, expires boot-clock deadlines on a
     regression, and an awake-clock deadline that a regression pushes
     later costs time, never safety, since awake deadlines time
     retransmission and round trips [OWN];
   - **a detected suspend** (the boot clock grew more than the awake clock
     by over the threshold, after sampling skew): one episode; no count
     moves, since boot deadlines expire across the suspend by the boot
     clock itself (5.6, point 7);
   - **a jump of wall time** past the same threshold (between two valid
     samples, wall time moved by more or less than the boot clock did, by
     over the threshold after sampling skew): one episode; no count
     moves, since no deadline is on wall time, and wall time can only
     lower trust (C37, point 4).

   **No new deadline while samples fail.** `ironafs-time` is pure and
   calls no seam (5.2, point 5), so its constructors take the fresh
   sample as `now` rather than taking it themselves. C37, point 5's
   rule, and C37, point 4's for a regression, rest on three things, and
   only these:
   - **The latch (by construction).** The latch is the lower of `now`'s
     and `from`'s counts, and a clock's counts only grow, so the latch
     is never above the clock's count when the deadline was built. A
     failed read or a regression after either stamp was taken has moved
     the count above the latch, so the deadline is failed at its first
     evaluation (point 4), and while samples still fail the evaluation
     finds the clock unreadable anyway. `from` gives two things: the
     instant counted from (C39, point 3, "a deadline counted from
     arrival"), and its count to the latch. The count is needed: after a
     boot-clock regression of R between the arrival and `now`, `from`'s
     instant is on the timeline before the regression, so a deadline
     counted from it, read on the clock after it, would end up to R
     later than `d` after the arrival; the regression has moved the
     count past `from`'s, so the deadline is failed instead (C37, point
     4, treats every boot-clock deadline as possibly expired on a
     regression, and a callback lifetime counted from `first_sent` must
     never lengthen, 5.4, point 4). A stale stamp therefore cannot hide
     a failure or a regression.
   - **The value check (pure, in the constructors).** A `from` newer
     than `now` on the deadline's clock, by a higher doubt count or a
     later instant, is refused with `Err(DeadlineError::FromAfterNow)`.
     Without a regression, no true arrival or `first_sent` is newer than
     a sample taken after it; across a regression a true one can be (its
     instant is on the earlier timeline), and refusal is then the safe
     outcome. Either way the sampling function's caller treats
     `FromAfterNow` as `ClockFailed` and never retries with another
     `from`. So a forged or fresh `from` with a higher count than `now`
     cannot raise the latch, and a regression larger than the time since
     the arrival cannot stretch the deadline either. What a value check
     cannot see is a `from` whose count is equal to `now`'s and whose
     instant is not later, but which is not the true arrival or send (a
     sample taken after a failure or regression that followed the true
     arrival, or a forged stamp with those values). It is not refused,
     its latch equals the current count, and it hides that failure or
     regression; only the call-site check below, which pins where `from`
     comes from, bounds it.
   - **The call-site check (`xtask`'s
     `seams::deadline_built_from_a_fresh_sample`).** It reads the source
     of every workspace crate as tokens (each non-test module;
     `#[cfg(test)]` items, test modules their parent gates, a file whose
     leading inner attributes hold `#![cfg(test)]`, and `tests/` are left
     out; both checks read the same tokens. A `#[cfg(test)]` on anything
     but an item, such as a field, a variant, an arm or a parameter, and
     a `#![cfg(test)]` inside a module leave out nothing, since the token
     reader cannot find their ends for certain: what they gate is read
     as non-test code). It matches the two constructors by name, so a `use`
     rename of the type changes nothing. In `ironafs-time` the
     constructors' names may appear only where they are defined, so no
     helper there can hand the constructors to other crates. It fails
     unless all of these hold:
     - **One named sampling function per crate.** `xtask` keeps a list
       naming, by full path, at most one sampling function per crate (in
       the core, `ironafs_client::time::deadline_after`). The function
       is found by that path: at the top level of the module file it
       names, under the crate's library directory (`time.rs` or
       `time/mod.rs` there, so `src/time.rs` here, and never a file
       elsewhere in the package that ends the same way, such as an
       example's; the library root for a function at the crate root),
       not in a nested module, an
       `impl` or a trait; a function of that name anywhere else is not
       the sampling function. Every path
       that resolves to `Deadline::boot_after` or
       `Deadline::awake_after` is the callee of a direct call inside
       that function's body. A path used as a value (a function pointer,
       a closure that calls it and leaves the function, a method
       reference passed on) fails.
     - **`now` is that function's own fresh sample.** The body binds the
       result of one `.sample()` call on its `Clock` parameter (a
       parameter whose type, after its first `:`, contains the identifier
       `Clock`, so `&dyn Clock` and `&impl Clock` pass and a generic
       `clock: C` bounded elsewhere is refused; a type naming a different
       trait called `Clock` is not told apart) with `let`, returns
       `Err(DeadlineError::Clock)` if it fails, and passes that binding,
       never reassigned, as the constructor's `now`, with no other
       expression flowing into it. Written as
       `let now = clock.sample().map_err(DeadlineError::Clock)?;`, and
       each constructor called after it as
       `Deadline::boot_after(&now, from, ...)`, with `from` the function's
       own `from` parameter. The binding's name and `from` appear nowhere
       else in the body (no parameter, pattern, shadowing, use or
       assignment of either), the `Clock` parameter's name appears in it
       only as the sample's receiver (so no binding can stand in for the
       parameter), and the body holds no `|` (no closure, and
       so no bitwise or either), no nested function and no constructor
       inside a macro call.
     - **`from` is the recorded arrival or `first_sent`.** At every call
       of a sampling function, the `from` argument is a field access
       ending in `.arrival` on the request record (the arrival time C39,
       point 2's ingress records) or the `first_sent` of a `Reply` (5.4,
       point 1), unwrapped from its `Option`. Anything else (a fresh
       `sample()`, a `Stamp` literal, a local built from either) fails.
     - **The allowance sits on the sampling function only.** Both
       constructors are also on the lint's disallowed-paths list, which
       flags every use of the paths and knows nothing of the rule above;
       it is a second net only. The check fails if an allowance for
       either path sits anywhere but on the named function: on a second
       function, a module, an `impl` or the crate. The list is the root
       `clippy.toml`'s. Clippy reads the nearest `clippy.toml` whole, and
       inside `ironafs-time` the paths name nothing, so `ironafs-time` has
       its own file: the root file without the two constructor bans, and
       nothing else different apart from comments. There the call-site
       check alone holds the rule. `xtask`'s `check-bans` proves both bans
       fire on planted calls and fails if the two files differ. Tests that
       build deadlines outside `ironafs-time` carry the allowance on a
       helper each. An allowance is looked for in attributes only; a
       manifest's `[lints]` table cannot carry one, since `check-deps`
       requires every member to inherit the workspace's lints and the
       workspace table is the root manifest's.
     - **Planted self-tests,** each of which must fail the check with its
       own kind of failure, on its planted line where that failure has a
       line (no fresh sample, a sample from a parameter that is not the
       clock, a parameter sharing the fresh sample's name, a duplicate
       and a missing sampling function are judged by kind alone): a `now` kept from an
       earlier sample; a `now` from a parameter that is not the clock; the
       clock parameter rebound before the sample; a sampling function
       with no `from` parameter; one defined in another module or in a
       nested module; a
       parameter or pattern binding sharing the fresh sample's name; a
       constructor before the binding; a constructor whose `from` is
       another parameter; a constructor called outside the sampling
       function, and in `ironafs-time` outside its definition; a second
       function in the same crate with its own allowance; a module-level
       allowance; the constructor carried out of the sampling function by
       a closure and by a function pointer, or inside a macro call; a
       `from` that is a fresh sample; a `from` that is a forged `Stamp`
       literal with a high count; a `#[path]` module, also inside
       `cfg_attr`; an `include!`, and `include` renamed by a `use`; an
       inner `#![cfg(test)]` in a module, which hides nothing after it; a
       source the token reader cannot read.
     - **Limits, which review covers** (the `rust-review` consistency
       pass): the check reads source as written and expands no macro, so
       a macro invocation whose tokens name either constructor fails it
       (a token scan), but a macro that assembles the path from
       fragments is not seen; and it checks `from` by field name, not by
       type flow, so it trusts that only the ingress writes a request's
       `arrival`, once, and that only the call owner writes a `Reply`'s
       `first_sent`, once, from the `pre` sample of its hand-off (5.4,
       point 4). `Reply`'s fields are public, so a wrong or forged
       `Reply` whose `first_sent` is later than the true send but not
       later than `now` passes both checks and would start a callback
       lifetime late (C26, point 2); review covers who builds a `Reply`.
       The planted self-tests include a forged `from` with a count equal
       to `now`'s and an earlier instant, written through each of these
       two limits, to show that the check does not catch it (they are
       expected passes, recorded as the limits, not failures).
     - **Scope.** The check sees only the workspace's own crates. Every
       crate of the workspace is unpublished (`publish = false` in the
       workspace manifest), so no crate outside it builds on
       `ironafs-time`'s constructors through a registry; a crate that
       copied or vendored them would be outside the check, and the
       guarantee is the workspace's only. The constructors stay `pub`
       because Rust has no visibility between `pub(crate)` and `pub`
       that would reach the sampling functions in other crates.

   So, while samples fail, the code has no new `now` and builds no
   deadline. This rests on the check, not on a type: any crate may
   implement `Clock` (`ironafs-os`, `ironafs-sim`,
   `ironafs-test-support`), so a "fresh stamp" or "arrival stamp" type
   would need a public constructor that any crate could call, and would
   add nothing the check does not give. A way past the check (one of the
   limits above) can build a deadline from a stale `now` or `from`,
   which the latch fails at once; from a forged one newer than `now`,
   which the value check refuses; or from a forged or wrong `from` with
   a count equal to `now`'s and an instant not later, which nothing in
   the type or the value check catches, so that deadline can hide a
   failure or a regression after the true arrival. That last case is the
   residual the call-site check and review exist for [OWN].

   A compile-fail doc-test in `ironafs-time` shows that a literal, a
   destructuring and a field write of a `Deadline` do not compile
   outside the crate [OWN].

   **Doubt is latched per deadline.** A `Deadline` records a latched
   count for its clock, never above that clock's count when it was built
   (above). Every later evaluation of it (a wait, the call owner's
   arbitration in 5.4, point 4, a completion claim in 5.7, point 4) finds
   it failed once that clock's count is above the latch, even if the
   sample taken now is good. So a failure that clears
   before another waiter samples still expires every deadline outstanding
   when it happened, and a regression still expires every boot deadline
   [ADR 0004 C37.5: Working]. **Episodes reach the core** through the
   episode count of each `Stamp`: a serve that finds it moved since the
   last serve marks callbacks suspect before the answer (C32, point 5),
   and callback trust compares it (5.4, point 1).

3. **The wait protocol.** A `Watch` holds a flag and a bounded list of
   registered `Waker`s under its own internal mutex. The `Clock`
   implementation supplies one `Waker` per waiting task: a condition
   variable in `ironafs-os`, a task record in `ironafs-sim`.
   - **The waker's latch.** Each `Waker` holds a wake generation, a
     counter under the waker's own mutex. A wake increments it and
     notifies, both under that mutex.
   - **Wait:** register the waker with the signal and with the cancel
     token; then, in a loop: read the waker's generation g; evaluate
     point 4; if an outcome is decided, deregister from both watches and
     return it; otherwise take the waker's mutex and block (releasing it
     atomically, as a condition variable does) only if the generation
     still equals g, until woken or until 5.7, point 1's slice. Every
     wake, spurious or not, is followed by an evaluation.
   - **Raise and fire:** set the flag first, then wake every registered
     waker. A raise before registration is seen by the evaluation that
     follows registration. A raise between the evaluation and the block
     has moved the generation past g, so the block does not start. A
     raise after the block starts notifies it. No wake is lost on either
     side of the check-to-sleep boundary.
   - **Registration's lifetime.** A registration lasts from the start of
     one `wait` to its return. Every exit path deregisters from both
     watches, a crash marker's unwind included (5.6, point 8). A
     registration is never kept between two waits.
   - **Lock order.** A watch's mutex is taken before a waker's, never the
     reverse. Neither is held across a block.
   - **Signal and cancel differ.** A signal can be raised and lowered by
     its owner (a queue lowers its signal when it is emptied); a cancel
     token is fired once and stays fired. Raising and firing are
     idempotent.
   - Under the simulator the check-to-sleep gap cannot occur: a raise
     happens only in another task's step or a world event, never between
     a task's evaluation and its yield, which form one step; the task
     record plays the latch.
   - A registration list that is full is a defect. `register` refuses it.
     Under the simulator the simulated `Clock::wait` that is refused
     reports it to the scheduler, which fails the run (5.6, point 9); in
     production the wait falls back to polling at the slice and an event
     is logged. Its bound is one entry per waiting task.

   [OWN: the protocol closes the lost-wake gap that a flag and a
   condition variable leave between the check and the sleep.]

4. **Wait evaluation.** Each evaluation first checks the cancel token. If
   it has fired, return `Cancelled` without observing the signal or the
   clock. Otherwise take a snapshot of the signal's flag with neither the
   clock's state lock nor the waker's mutex held. Then observe the
   deadline's clock and its doubt count together once.

   A failed clock read or a doubt count above the deadline's latched
   count (point 2) gives `ClockFailed`. Otherwise a reading at or past the
   deadline gives `DeadlinePassed`. Otherwise a raised snapshot gives
   `Signalled`. With no outcome yet, follow the wait protocol in point 3.

   `Signalled` requires a raised snapshot followed by a valid, unexpired
   clock observation. While the deadline's clock does not go back, the
   snapshot therefore precedes the deadline. If the awake clock goes back
   below a deadline it had already passed, the wait can return
   `Signalled`: the regression moves only the boot doubt count (point 2).
   A raised flag gives no evidence of which publication raised it. The
   result's owner decides which result wins and whether it met the
   deadline, for `Rpc` the call owner (5.4, point 4 and C46, point 4).

5. **The primitives,** in `ironafs-sync`, flags and bounded storage over
   `Clock::wait`, the same code in production and under the simulator:
   Each primitive implements `ironafs-time`'s `Watch`.
   - `Signal`: a raisable watch.
   - `Cancel`: cloneable; `fire` is idempotent and wakes every wait
     registered with it. `watch()` returns a `WatchRef` on the same
     token, which is what a seam in a pure crate takes (5.2, point 1).
   - `OneShot<T>`: one `put`, one `take(clock, deadline, cancel)`, with
     `cancel` a `&dyn Watch`. A
     second `put` is refused, never a panic. `put` raises the slot's
     signal.
   - `Queue<T>`: bounded in bytes (C39, point 3). `push` never blocks: a
     full queue refuses the item, and the caller returns a retryable
     error and counts it (C39, point 2). `pop` waits.
   - `Lock<T>`: mutual exclusion with a level from C39, point 4's order.
     It never waits on a seam. A build with the lock observer on records
     holders and checks the order.

6. **The lock rule.** No core `Lock` is held at any yield point (5.6,
   point 3). This is stricter than C39, point 4. [OWN: under the baton
   only one task runs, so if no lock is held at a yield point none is
   ever contended, and taking a held lock can only be a defect, caught at
   once (5.6, point 9) instead of hanging.] It is checked at every yield
   point, the submission to a binding included.

### 5.4 The `Rpc` interface

1. **Shape.**

   ```rust
   /// In `ironafs-rx`.
   pub trait Rpc: Send + Sync {
       /// Opens an Rx connection. Sends nothing and blocks on nothing.
       fn open(&self, peer: Peer, service: ServiceId,
               security: Box<dyn SecurityObject>) -> Result<ConnId, RpcError>;
       /// One client call: the encoded request out; reply bytes and evidence back.
       fn call(&self, conn: ConnId, request: &[u8], reply_cap: u64,
               deadline: Deadline, cancel: &WatchRef) -> Result<Reply, RpcError>;
       /// Fences the connection before it returns, then settles every call
       /// of it by point 4, with cause `Closed`.
       fn close(&self, conn: ConnId);
       /// Server side, for the callback service (C31).
       fn accept(&self, service: ServiceId, deadline: Deadline,
                 cancel: &WatchRef) -> Result<Incoming, RpcError>;
       fn respond(&self, call: Incoming, reply: Response<'_>) -> Result<(), RpcError>;
   }

   pub struct Reply {
       pub bytes: Vec<u8>,
       /// When the request was first handed to the transport (C26, point 2),
       /// a `Clock::sample` taken inside the hand-off (point 4). `None` if the
       /// sample failed:
       /// no lifetime is counted from it (C37, point 5).
       pub first_sent: Option<Stamp>,
       pub via: ReplyEvidence,
   }

   /// Where, on what and when the reply arrived (C32, point 1).
   pub struct ReplyEvidence {
       pub family: AddrFamily,
       pub local_generation: EndpointGeneration,
       pub epoch: u32,
       pub conn_id: u32,
       pub call_number: u32,
       /// A `Clock::sample` the loop took when it read the datagram that
       /// completed the reply, before feeding it to the endpoint. `None` if
       /// the sample failed.
       pub received: Option<Stamp>,
   }
   ```

   - `request` is the call as `ironafs-proto` encodes it, opcode
     included. `ironafs-client` maps each connection key (C30, point 3)
     to a `ConnId`.
   - `cancel` is a `WatchRef` (5.3, point 1), not `ironafs-sync`'s
     `Cancel`, so the trait stays in the pure, `no_std` `ironafs-rx`
     (5.2, point 1). The caller passes its `Cancel`'s `watch()`. The
     implementation clones the handle into the submission, so the call
     owner reads the caller's token itself, inside the decision guard
     (point 4), and never through a message.
   - `SecurityObject` is the security-class interface of `ironafs-rx`. It
     is a pure state machine (C3, point 1): any credential work that
     needs I/O is done before `open`, by the driver that obtains the
     token, so `open` sends nothing.
   - `Incoming` carries the peer, the call identity the callback service
     checks (C31, C32) and the request bytes under the service's cap.
     `Response` is reply bytes or an abort code.
   - **Reply evidence and trust.** `ReplyEvidence` is what C32, point 1's
     trust rule needs. A keepalive's reply earns trust only through it,
     and only if: both stamps exist; the boot clock grew no more than the
     awake clock between them, beyond C37, point 4's threshold (no
     suspend between send and receipt); the two stamps carry the same
     episode count (no detected suspend, regression, failed read or wall
     jump between them, 5.3, point 2); the send follows the last network
     change; and the receipt is within E of the send. A reply sent before
     a suspend and read from the socket buffer after it never counts: the
     suspend lies between its stamps. Trust is aged from `received`, not
     from when it is granted: at the grant, and at every use after it, a
     fresh sample must show no more than E on the boot clock since
     `received`, and the same episode count as `received`; otherwise
     trust is not granted, or has lapsed. So any episode, a wall jump
     included, ends trust at the next use
     [ADR 0004 C32.1, C32.4, C37.4: Working; OWN for the stamps].

2. **Errors.** The seam does not know whether an opcode changes state. It
   reports only what the call owner knows about transmission:

   ```rust
   pub enum RpcError {
       /// The call owner ruled out transmission. Safe to retry.
       NotSent(NotSentCause),
       /// Transmission may have occurred and no conclusive outcome exists.
       NoReply(NoReplyCause),
       /// The peer ended the call with an abort that matched the live call.
       /// The call is over; whether the procedure took effect is not
       /// established by the abort (the last bullet below).
       Aborted { code: u32, via: ReplyEvidence },
   }

   pub enum NotSentCause {
       Deadline, Cancelled, Unreachable, Admission, Closed, ClockFailed,
       Security(SecurityFailure),
   }

   pub enum NoReplyCause {
       Deadline, Cancelled, PeerLost, NetworkChanged, Closed, ClockFailed,
       ReplyTooLarge, Security(SecurityFailure),
   }
   ```

   - A security failure before any packet left (the object refused to
     protect the request) is `NotSent(Security)`; one after (the reply
     failed its check) is `NoReply(Security)`.
   - A reply over `reply_cap` (coding standards rule 3) is
     `NoReply(ReplyTooLarge)`: the peer ran the call.
   - `NotSent(Unreachable)` settles a queued call when the transport
     refuses its first packet with an error that a retry to the same
     destination cannot clear (no route, the address family unavailable,
     the socket closed). Which send errors are of that kind is the
     `ironafs-net` design's list, and the model network scripts both
     kinds. "Would block" and every other send error leave the call
     queued (point 4); the owner tries again when the socket can take it
     or at the endpoint's next timer, whichever is first, never in a
     loop with no wait, so a call that can never be sent ends at its
     deadline as `NotSent(Deadline)`.
   - There is no retryable "busy" result. A local refusal is
     `NotSent(Admission)` (point 8). A peer's Rx BUSY is not evidence
     that the call was rejected; how an unprotected BUSY may act is the
     Rx specification's and ADR 0006's, and the call then ends by the
     rules of point 4.
   - `ironafs-client` maps these per RPC to the contract's errors (C46,
     point 1): for a fetch, `NoReply` is "timed out" or "unreachable",
     except `NoReply(ReplyTooLarge)`, which is never retried as either: the
     peer answered, and a retry meets the same cap. It fails the request
     at once with its own event, and the server is not marked
     unreachable; which contract error it becomes is the contract
     specification's (issue #67). For a store, any `NoReply` is C28, point
     4's lost reply, never success and never resent.
   - **An abort ends the call, not the question.** `ironafs-client`
     classifies the code by the RPC and the server (C46, point 3). For a
     procedure that changes state, an abort is a lost reply (C28, point
     4: ambiguous, read back) unless that procedure's specification
     states that the code is returned only before any change, **and** the
     abort is authoritative evidence (C24, point 1): protected by a key no
     local user can determine and bound to this call. A match with the
     live call's identity is not provenance: an unprotected abort with
     that identity can come from anyone on the path. Before rxgk no abort
     is authoritative, so every abort of a state-changing call is
     ambiguous until then [ADR 0004 C28.4: Firm; C24.1: Firm]. For a
     read, an abort is its classified error.

3. **Deadlines.** `call` and `accept` take a `Deadline`. Operation code
   passes its request's deadline, counted from arrival (C39, point 3).
   The Rx machine's retransmission, round-trip and keepalive timers run
   on the awake clock inside the endpoint (C37, point 1), whatever the
   clock of the call's deadline; a call whose deadline is on the boot
   clock still retransmits at awake-clock cadence, and ends at its boot
   deadline through point 4, across a suspend too. Which clock a
   request's deadline is on is the contract specification's (section 13,
   Q3); this interface works for either.

4. **One call lifecycle, one owner.** The `rx` loop owns every call's
   state and alone changes it, one transition at a time [OWN: one owner
   is what makes every send and every settlement totally ordered against
   the caller's abandon, as C28, points 3 and 4, need]:

   | State | Entered when | Settled by the owner as |
   |---|---|---|
   | queued | the submission is admitted (point 8) | `NotSent(cause)`, if the arbitration below ends it here, or on close; nothing is ever sent later |
   | handed | the transport has **accepted** the first packet; a hand-off whose `post` sample is past the deadline is **handed late** | a complete reply, an abort, or `NoReply(cause)`; a late hand-off only `NoReply(Deadline)` |
   | settled | the owner records an outcome and puts it in the call's slot | final; a later event changes nothing |

   - **The submission carries its own ends.** It holds the call's
     deadline, the caller's cancel token (the `WatchRef` of point 1), an
     abandon flag (a `Cancel` the caller and the owner share) and its
     connection's fence (a `Cancel` per connection, shared by every
     submission on it), beside one shutdown fence for the run or the
     daemon. Setting the flag is the abandon; `close` fires the
     connection's fence inside a hold of the connection's send lock (the
     next bullet but one) before it returns, and shutdown fires its fence
     before it posts anything. A message on the control lane (point 8)
     only wakes the owner sooner. The owner never depends on a message
     arriving to know that a call, or its connection, has ended.
   - **One decision guard per call.** Each call has a decision guard: a
     small mutex of `ironafs-sync` (not a core `Lock`, so C30, point 2,
     holds) over the call's state and its abandon flag. The owner makes
     each arbitration below, and the transition it allows, in one hold
     of the guard, reading the cancel token and a fresh sample of the
     deadline's clock inside it. The caller sets the abandon flag, and
     reads the state, inside the guard. The guard is held for the check,
     the state write and, at the queued-to-handed transition only, the
     transport hand-off of the first packet; never across a wait, a
     yield or I/O that can block.
   - **One send lock per connection.** Each connection has a send lock,
     a small mutex of `ironafs-sync` like the guard (not a core `Lock`).
     Every datagram the `rx` loop hands to the transport for that
     connection (a call's first packet, its later data packets,
     retransmissions, acknowledgements, aborts) is handed by a
     non-blocking send inside a hold of the connection's send lock, after
     reading the connection's fence and the shutdown fence unfired in
     the same hold; a datagram that finds a fence fired is dropped, never
     sent. `close` takes the same lock, fires the fence inside that hold,
     and releases it. The lock is held only for those reads, one clock
     sample and one non-blocking send, never across a wait, a yield or
     I/O that can block. The lock order is a call's guard, then its
     connection's send lock; `close` takes only the send lock and the
     caller's abandon only the guard, so neither can deadlock with the
     owner [OWN].
   - **The hand-off is inside the guard.** The owner hands the prepared
     first packet to the transport with a non-blocking send inside the
     same hold of the guard that made the arbitration (the fences' part
     of it read inside the nested hold of the send lock that makes the
     send), and the call
     becomes handed only when that send returns accepted. If the send
     would block, or fails with an error that point 2 leaves queued, the
     call stays queued, nothing of it is on the transport, and the next
     attempt arbitrates again from the start; an error that point 2 calls
     unreachable settles it `NotSent(Unreachable)` in the same hold. So
     no send happens after a deadline, cancel, abandon or fence that was
     ordered before it. Once accepted, the operating system may put the
     packet on the wire later: that is the send accepted before the end,
     never a new send, and this page claims nothing about when it reaches
     the wire (the close fence below). An abandon ordered after it finds
     the call handed, and the caller gets `NoReply`: for a store, C28,
     point 4's lost reply, never a deferred store sent on its own (C28,
     point 3).
   - **Acceptance is bracketed against the deadline.** A non-blocking
     send's contract is only that it does not wait, and the owner's
     thread can be preempted between its clock sample and the send for
     any time [UNVERIFIED: that no interface on the three platforms makes
     a datagram's acceptance conditional on a clock; the design does not
     depend on it]. So the deadline is enforced on both sides of
     acceptance, inside the same hold of the guard:
     - **Margin before.** The arbitration's fresh sample `pre` must be at
       least the hand-off margin before the deadline (a configuration
       value; proposed 50 ms, far above an unpreempted hold, which is one
       clock sample and one non-blocking send; measured in M2a). A call
       inside the margin is settled `NotSent(Deadline)` from queued, as
       if the deadline had passed.
     - **Sample after.** The owner samples the deadline's clock again,
       `post`, as soon as the send returns accepted, before the hold
       ends. If `post` is at or before the deadline, the call was handed
       in time. If `post` is after it (a preemption overran the margin),
       the call is **handed late**: handed, since the packet cannot be
       withdrawn, with its verdict fixed in the same transition as
       `NoReply(Deadline)`, so no later reply or abort can win and the
       caller never gets success or `NotSent`. The **late-hand-off
       event** names the call and the overrun, and its **counter** counts
       it. A late hand-off of a state-changing call is a lost reply (C28,
       point 4). A clock that cannot be read for `post`, or whose doubt is
       latched by then, counts as late.
     - `first_sent` is `pre`, the earliest the packet can have left, so a
       callback lifetime counted from it, and the "sent after the last
       suspend" check, can only be shorter or fail, never longer.
   - **The residual for stores, closed in M3** (decided by the owner;
     issue #71). The clock bracket cannot by itself stop a store's first
     send from following the failure of its commit, since what makes a
     store deferred (C28, point 3) is its commit stalling or failing (C9,
     point 3; C29, point 8), not the call's deadline. The store path (M3)
     closes it: a store's first hand-off is eligible only while the
     journal holds the store as immediate, and the transition that ends
     that state is made inside the same call's decision guard, totally
     ordered against the hand-off. A call still queued at it is settled
     `NotSent`, and its data stays the deferred record; a call handed
     before it was sent while the store was immediate (C28, point 2). M2a
     makes no store call. The hand-off margin, the late-hand-off event
     and its counter are M2a's.
   - **Arbitration, in the owner's own transitions.** Inside the guard,
     immediately before the queued-to-handed transition and immediately
     before recording a reply or an abort, the owner evaluates, in this
     order: abandon flag set; the connection's fence or the shutdown
     fence fired (cause `Closed`); cancel fired; the deadline's clock
     unreadable or its doubt latched (5.3, point 2); the deadline passed
     (before the queued-to-handed transition: the deadline within the
     hand-off margin). The first that holds settles the call: from queued
     as `NotSent` with that cause, from handed as `NoReply` with that
     cause (the reply or abort is dropped, and the endpoint abandons the
     call as the Rx specification says). Only if none holds is the packet
     handed or the reply recorded. A reply that arrives after the
     deadline, cancel, abandon or fence never wins, and a queued call is
     never sent after any of them.
   - **The close fence.** No datagram of a connection is accepted by
     the transport after `close` of that connection has returned,
     whatever order the owner takes its messages in. It holds because
     the hold of the send lock in which `close` fires the fence and every
     hold that hands a datagram for that connection exclude one another:
     a hand-off hold that ended before `close`'s hold began was accepted
     before `close` returned; a hand-off hold that begins after it reads
     the fence fired and sends nothing. So the connections a rising
     restriction closes send nothing more once `close` returns ("sends
     queued on them are fenced", N7, point 2) [ADR 0004 N7.2: Firm].
     **What this gives N7, point 5, and what it does not.** This page
     reads point 5's "leaves" as the transport's acceptance of a
     datagram, the last step the client controls, and that is where the
     simulator's point 5 check observes it (O45): no datagram of a
     connection is accepted after the `close` that a rise applies to it
     has returned. The fence does not control the wire: a datagram
     accepted before `close` returned may reach the wire after it (the
     accepted-before-the-end case of the hand-off above), and one
     accepted between the restriction's rise and `close`'s return is
     before the fence. The simulator's check therefore counts from the
     return of each `close` the rise makes, not from the rise itself
     [OWN: a reading of N7, point 5; a non-blocking send cannot recall a
     datagram it has handed over].
   - **The caller's side.** The caller's wait ending (deadline passed,
     cancel fired, clock failed) decides nothing by itself. The caller
     sets the abandon flag inside the guard, posts the wake on the
     control lane, and takes the same slot again for the owner's verdict,
     with a short horizon deadline and a cancel token that never fires.
     The owner answers from the call's state: queued, `NotSent`; handed,
     `NoReply`; settled, the outcome already recorded (a reply recorded
     before the flag was set, and before the deadline, wins).
   - If even the verdict's horizon passes (the owner is stuck, a
     defect), the caller returns `NoReply`, never `NotSent`, and an event
     is logged. The call is still fenced: the flag is set, so the owner,
     when it runs again, sends nothing for it if it is queued, and lets no
     later reply or abort win if it is handed.
   - `close` and shutdown settle each call of the connection by the same
     rule, cause `Closed`: a queued call `NotSent(Closed)`, which for a
     request not yet sent means it runs through the connection's choice
     again (N7, point 2), and a handed one `NoReply(Closed)`, which for a
     mutation is the lost-reply rule. A first packet accepted in a
     hand-off hold that ended before `close`'s hold of the send lock
     began is ordered before the close, and before `close` returns: the
     call is handed, and is settled `NoReply(Closed)`.
   - `NotSent` is returned only from the queued state. Every other path
     that may follow a send is `NoReply`, a reply or an abort. A
     cancelled store is a lost reply (C28, point 4): its outcome is read
     back, never assumed.
   - Under the simulator a transition is one step, so the guard is never
     contended; it is the same code.

5. **What a call may block on.** `call` and `accept` block only on their
   own `OneShot`, through `Clock::wait`. The hand-off to the `rx` loop is
   a `Queue::push`, which never blocks. `open` and `respond` never
   block. `close` waits at most for one hold of its connection's send
   lock already in progress (point 4: a fence read, one clock sample and
   one non-blocking send), never for I/O that can block or for a wait.
   No `Rpc` method takes a core `Lock` (C30, point 2).

6. **Bulk data.** A call carries its whole request and reply in memory,
   within their caps. With 1 MiB chunks (C27, point 8, a proposal) this
   is bounded. Streamed calls are M3's (section 13, Q4).

7. **One implementation, two loops.** `RxRpc` holds the sending side of
   the `rx` loop's ingress and a `Clock`. A call admits a submission with
   a fresh `OneShot` and takes it. The `rx` loop owns the `ironafs-rx`
   endpoint (the pure machine and its call table):
   - in production, `ironafs-net`'s `rx` thread reads the sockets with a
     timeout from the endpoint's next timer, feeds datagrams, expiries
     and submissions to the endpoint, sends what it emits and fills
     slots; point 9 says how a submission wakes it;
   - under the simulator, a simulated `rx` task does the same over the
     model network. It waits with `Clock::wait` on its ingress signal, to
     the endpoint's next timer. A delivery is a world event that pushes
     the datagram onto the task's inbound queue, a `Queue` bounded in
     bytes that models the socket's receive buffer (the scenario states
     its cap), and raises that signal, without a yield. A delivery to a
     full inbound queue is dropped and counted, as a full socket buffer
     drops it: the endpoint never saw it, so it is never acknowledged,
     and to the sender it is network loss. The task feeds what it pops to
     the endpoint when next granted, and a slot it fills raises the
     waiting caller's signal.

   Both loops run one call owner: the submission, the decision guard and
   the arbitration of point 4 are one piece of code. `ironafs-net` and
   `ironafs-sim` do not depend on each other, and `ironafs-net` does not
   depend on `ironafs-client` (5.2, points 2 and 5), so that code lives in
   a crate both may use; which one is section 13, Q2's.

   **Callback overload** (C31; C16, points 1 and 6). An incoming call for
   the callback service that the endpoint has received but cannot hand
   to `accept` (the service's accept queue is full in bytes, or the
   source is over its rate limit) is dropped by the loop, never
   acknowledged as accepted. Before the drop, in the same transition, the
   loop records the overload in a shared overload record (an
   `ironafs-sync` primitive, so no core `Lock`): a counter and the bounded
   set of source addresses, which overflows to "unknown source".
   - **The record is part of the fence from the drop.** A fetch's ticket
     records the record's counter with the break counters (C16, point
     2), and every comparison of a ticket, at the fetch's install and at
     submission (5.8, point 2), reads the record: a counter moved since
     the ticket, with a recorded source matching the ticket's server or
     an unknown source, fails the comparison as a moved break counter of
     that server would.
   - **No ticket over a pending overload.** The core keeps the record's
     counter as of its last fold. At a read's admission and at every
     ticket's creation, before the ticket captures any counter or any
     callback trust, the core reads the record: if its counter is ahead
     of the last fold, the core folds it then, in the same step (the
     matching servers' callbacks marked suspect), and only then takes the
     ticket. So a ticket never pairs the record's new counter with
     callback trust from before the drop [ADR 0004 C16.1: Firm].
   - **Out of process** (C16, point 6). Moving the record raises the
     callback service's signal. That task, at its next step, pushes a
     fence-epoch advance on the ordered stream of every binding in its
     own process that holds data from the matching servers (of every
     binding, if a source is unknown); the stream keeps space for an
     advance, so it is never dropped.

   The record's exact form and its matching are the callback service
   specification's. The production loop is tested on loopback and in the
   interoperability cell (O48, point 1).

8. **Admission.** The ingress has lanes, each bounded in bytes (C39,
   point 3): status, data and stores separately (C30, point 3), and a
   control lane for abandon wakes, closes and shutdown. A call posts at
   most one abandon wake (only the caller that sets its flag posts it),
   each connection at most one close, and the run or the daemon one
   shutdown. The control lane's bound is the call table's size plus the
   connection count plus one, so it is never full while the call table is
   within its own bound, shutdown included; and a lost wake would cost
   only time, since the flag, not the message, ends the call. Besides the
   lanes, the bytes of every unsettled call (its request plus its
   `reply_cap`) are reserved against one in-flight budget at admission
   (C27, point 4's "fetches in flight"). A refusal by a lane or by the
   budget is `NotSent(Admission)`, counted. The callback service's input
   arrives on the sockets, never through these lanes.

9. **Waking the production loop.** A submission must reach an `rx`
   thread blocked in a socket read within a stated bound, not only at the
   next timer. The mechanism, and how one thread reads one socket per
   address family (C30, point 2), are the `ironafs-net` design's, chosen
   by measurement (section 13, Q14). The conformance body measures the
   bound on an idle loop and under sustained receive load.

### 5.5 The `Disk` interface

1. **Shape.**

   ```rust
   /// In `ironafs-cache`.
   pub enum ReadRange { Whole { max: u64 }, Part { offset: u64, len: u64 } }
   pub enum Durability { Unsynced, Synced }
   /// `Other` is a link or any entry that is neither a file nor a directory.
   pub enum EntryKind { File, Dir, Other }

   pub trait Disk: Send + Sync {
       fn read(&self, path: &StorePath, range: ReadRange) -> Result<Vec<u8>, DiskError>;
       /// Creates exclusively, readable only by the daemon's account, and
       /// writes all of `bytes`. `Synced` syncs on the same handle before
       /// closing. Never replaces a file.
       fn create(&self, path: &StorePath, bytes: &[u8], d: Durability) -> Result<(), DiskError>;
       /// Creates a directory exclusively. Durable only after its parent's `sync_dir`.
       fn create_dir(&self, dir: &StoreDir) -> Result<(), DiskError>;
       /// Replaces `to` with `from`, both in `dir`. Durable only after `sync_dir(dir)`.
       fn rename(&self, dir: &StoreDir, from: &StoreName, to: &StoreName) -> Result<(), DiskError>;
       fn sync_dir(&self, dir: &StoreDir) -> Result<(), DiskError>;
       /// Removes a file. Durable only after `sync_dir`.
       fn remove(&self, path: &StorePath) -> Result<(), DiskError>;
       /// Removes an empty directory. Durable only after its parent's `sync_dir`.
       fn remove_dir(&self, dir: &StoreDir) -> Result<(), DiskError>;
       /// At most `max` entries after `after`, in byte order of their names.
       fn list(&self, dir: &StoreDir, after: Option<&StoreName>, max: u32)
           -> Result<Vec<(StoreName, EntryKind)>, DiskError>;
       fn space(&self) -> Result<Space, DiskError>;
   }

   pub enum DiskError {
       NotFound, Exists, NoSpace, TooLarge, IsLink, IsDir, NotEmpty, Io { op: DiskOp },
   }
   pub enum DiskOp {
       Read, Create, Write, Sync, CreateDir, Rename, SyncDir, Remove, RemoveDir, List, Space,
   }
   ```

   - **`read`:** `Whole { max }` returns the whole file, or `TooLarge`
     and nothing if it is longer than `max`. `Part` returns the bytes
     from `offset` to the smaller of `offset + len` and the file's end:
     fewer than `len` only past the end, and none if `offset` is at or
     past it. The caller compares the length with what the content's own
     header states.
   - **`list`** returns names in byte order, each with its kind, read
     without following a link. A caller that needs every name pages with
     `after`, so a startup sweep always finishes, whatever the
     directory's size. A sweep removes a `File` with `remove` and a `Dir`
     with `remove_dir` after emptying it, and leaves an `Other` in place
     with an event: it never follows or removes a link.
   - **Which errors each call returns:** `NotFound` from `read`,
     `rename` (its source), `remove`, `remove_dir`, `list` and
     `sync_dir`; `Exists` from `create` and `create_dir`; `NoSpace` from
     `create` and `create_dir` only (a no-space condition in any other
     call is that call's `Io`, with an uncertain effect, point 4);
     `TooLarge` from `read`; `IsLink` from any call whose path meets a
     link (point 2); `IsDir` from `read`, `rename` and `remove` when a
     named entry is a directory; `NotEmpty` from `remove_dir`; `Io { op }`
     from the call named.
   - The shape holds whole buffers: `create` takes at most one chunk or
     one record image, under the caller's cap. Streaming is M3's
     (section 13, Q4).

2. **Paths and confinement.** A `StorePath` is a `StoreDir` and a
   `StoreName`, relative to the root the driver was opened on: a bounded
   number of components from a restricted character set with one case,
   no `.` or `..`, so on-disk names are case-independent (O41, point 4).
   It is validated when built; that checks only the name. The driver
   also holds an open handle on the root and resolves every component of
   every path relative to it, refusing a symbolic link or other
   redirection at each component, at lookup and at every mutation
   (`IsLink`), through the platform's root-relative, no-follow calls in
   `ironafs-os` [UNVERIFIED for Windows: which calls give the same
   guarantee; settled with that layer]. A rename's two names are in one
   directory, so no rename crosses directories [OWN: one directory's
   sync then covers it]. That a rename onto an existing name replaces it
   in one step on each platform's call is UNVERIFIED for Windows (section
   13, Q16), and the conformance body checks it there.

3. **Sync on the same handle.** There is no "sync this path" call.
   `create(.., Synced)` syncs on the handle that wrote. Each `StoreDir`
   holds one directory handle for the whole incarnation, opened when the
   store opens the directory, and every `sync_dir` of that directory goes
   through it, never through a handle opened afresh [OWN: where an
   operating system reports writeback errors per open handle, a handle
   opened after the error may never be told of it; UNVERIFIED per
   platform, checked in O41, point 4's qualification]. On Linux a held
   handle is told at its next sync that a writeback error occurred since
   its previous sync, not which write or how many [UNVERIFIED], so a
   failed `sync_dir` marks every namespace change in the directory that
   no earlier successful `sync_dir` covered, never one change (points 4
   and 7). The held handle also keeps the directory confined. On macOS a
   plain directory sync is not a write barrier; the macOS driver uses the
   full-flush control on directories as on files [UNVERIFIED until O41,
   point 4's tests run there].

4. **After a failure,** per call. "Uncertain" means the simulated disk
   draws the effect (point 7), and the caller treats both outcomes as
   possible. `Disk` can therefore report as failed an operation that took
   effect.
   - `create` failed: where it failed decides what is left, and the
     error does not say where (`NoSpace` can come from the exclusive open
     or from a write). At the open: the entry is absent. At a write: the
     entry exists, empty or holding any prefix of `bytes`. The caller
     handles all three: the temporary name is never renamed into place;
     the caller removes it, taking `NotFound` as done, or leaves it to
     the startup sweep of temporary names.
   - `Io { op: Sync }` from `create(.., Synced)`: the contents'
     durability is uncertain.
   - `Io { op: Rename }` or `Io { op: Remove }`: whether it was applied
     is uncertain; if applied, its durability is as for a success.
   - `Io { op: SyncDir }`: the durability of every namespace change in
     that directory not covered by an earlier successful `sync_dir` is
     uncertain. A change an earlier successful `sync_dir` covered stays
     durable.
   - **A failed sync means the data is not durable,** and it is never
     trusted as durable afterwards: not after a later successful sync of
     the same file or directory, and not after a restart (point 7). It is
     never retried as though the data were still pending: the next
     attempt writes a new temporary file [OWN: an operating system may
     report a failed writeback once and then report success to a later
     sync].
   - Any error in a plan: the plan stops; the steps before it keep their
     effects, whatever their durability; nothing is acknowledged. A
     durable store is then uncertain and is read again before its next
     commit.
   - `NoSpace` on a clean chunk: the chunk is not cached (C27, point 3).
     On a durable record: the native no-space error (O41, point 6).
   - **Any `read` error on a clean chunk is a miss** (C27, point 2):
     `NotFound`, `TooLarge`, `IsLink`, `IsDir` and `Io { op: Read }`
     alike, as 5.8's `if let Ok` does. The read falls through to the
     network, never fails on it. `IsLink` and `IsDir` also log an event,
     since nothing the cache writes is a link or a directory there.

5. **No deadline and no cancel.** `Disk` calls take neither: a local
   file call cannot be interrupted portably, and O41, point 1, requires
   local, non-removable storage. A contract request whose worker is held
   in a `Disk` call is still completed at its deadline by its completion
   owner (5.7, point 4); the daemon watchdog (C39, point 5) covers a
   hang. No `Lock` is held across a `Disk` call (5.3, point 6).

6. **Plans.** A pure crate expresses each multi-step change as a `Plan`;
   operation code runs it step by step through `Disk` and stops at the
   first error. Fixed here:
   - **clean chunk** (C27, point 2): `create(dir/tmp, Unsynced)`,
     `rename(dir, tmp, final)`;
   - **durable replace** (C29, point 9; per copy where a record keeps
     two): `create(dir/tmp, Synced)`, `rename(dir, tmp, final)`,
     `sync_dir(dir)`. **Durable removal:** `remove`, then `sync_dir`;
   - **directories** a store uses are created by that store at startup:
     `create_dir`, then `sync_dir` of the parent, before any plan writes
     in them. A directory sync comes after the change it covers.
   - **the root** is the state directory itself, which exists before the
     daemon starts (O41, point 1). It is outside the trait: no
     `StoreDir` names it or its parent. The driver's construction, in
     `ironafs-disk` and in the simulated disk alike, takes the state
     directory's lock, opens the root, and syncs the root's parent at
     every start, whether or not this start found the root present; a
     failure refuses the start. So a start that stopped before that sync
     leaves no later start skipping it. The simulated disk starts each
     run from a durable root [OWN: the root's own entry is the
     installation's, and a refused start is the only safe answer to a
     parent it cannot sync].
   - **where a durable record may live.** A failed `sync_dir` after a
     `create_dir` leaves the directory's creation marked (point 7): no
     later successful sync, in this incarnation or the next, makes it
     durable, and every change inside it needs it. A later incarnation
     cannot tell such a directory from a durable one. So a durable record
     is acknowledged only in the root, or in a directory whose
     `create_dir` and parent `sync_dir` both succeeded in this
     incarnation. Within an incarnation, a failed parent sync after a
     `create_dir` takes that directory out of use for durable records:
     the store draws a new name where its layout allows one (as point 9
     does), or refuses durable writes for the incarnation with an event.
     Which layout keeps this is section 13, Q18's [OWN: the fresh-commit
     rule below, applied to a directory's own name].

   **Serialisation.** Running a plan does not serialise. Plans of one
   store run one at a time, under that store's one-writer rule; plans of
   different stores never touch the same directory entry (the layout's
   rule).

   **Acknowledgement.** A change is acknowledged only after its plan's
   last step returned success and, for a record, the record and its
   commit boundary are durable by point 7's rules: the journal's boundary
   (N32, point 6) or both copies of a two-copy record. A plan with any
   failed step is never acknowledged [ADR 0004 C28.6: Firm].

   A plan is data, so its crash points can be enumerated (point 7).

7. **The persistence model** of the simulated disk [OWN: weaker than "an
   ordered prefix per directory", so code that relies on more than a
   filesystem promises fails under it]. It separates names from files.
   - **Files** have identities. Each identity has durable bytes and,
     until a sync on its handle succeeds, volatile bytes. Bytes belong to
     the identity and follow it through a rename.
   - **Each directory** has a durable namespace (name to identity) and
     an ordered log of the namespace changes made in it: create (a name
     bound to a new identity), `create_dir`, rename (one name to another,
     replacing the target's binding), remove and `remove_dir`.
   - **Dependency.** A change in the log depends on every earlier change
     in the same log that touches one of its names or its identity: for
     example, `rename(dir, tmp, final)` depends on the `create(dir/tmp)`
     before it.
   - **Coverage.** A successful `sync_dir(dir)` covers every unmarked
     change logged in `dir` before it; a successful sync on a file's
     handle makes its unmarked bytes durable. Covered changes always
     survive a host crash.
   - **What the log keeps.** Every change not yet resolved, in log order.
     A successful `sync_dir` folds into the durable namespace, and drops
     from the log, only the covered changes that come before every
     unresolved change (every marked change and every change not
     covered), applied in log order. So with no marked change the log is
     emptied by each successful `sync_dir`; while a marked change is
     unresolved, the durable namespace is the checkpoint just before the
     oldest unresolved change, and the log keeps the whole ordered suffix
     from it, covered changes flagged as covered. Its length is bounded
     by the scenario's step budget.

   **A host crash** resolves each directory independently. Every covered
   change survives. Of the changes not covered, it keeps a subset closed
   under dependency among them (a change is kept only if every
   non-covered change it depends on is kept), drawn from the seed's fault
   stream or scripted, and applies all the kept changes, covered and not,
   together in their original log order, each atomically, to the durable
   namespace; then the log is empty. An ordered prefix is one such
   subset; so is keeping nothing. Each surviving name's identity then has
   its durable bytes plus, for bytes no successful sync covered, any of:
   none (empty), a prefix, any subset of its sectors (a torn write), or
   all. A sector of the file's extent that is not kept reads as zeros or
   as seeded junk, drawn; an extended tail may hold junk after a kept
   prefix. A reader never sees a lost sector as merely absent bytes.

   | Situation at the crash | Legal outcomes |
   |---|---|
   | a file's unsynced bytes (`create(.., Unsynced)`, or `Synced` whose sync failed) | empty, a prefix, any subset of its sectors, or whole, under whatever name survives |
   | a create or `create_dir` no `sync_dir` of its directory covered | the name present or absent |
   | `rename(dir, a, b)` that `sync_dir(dir)` did not cover | applied (`b` names the file, `a` absent) or not (`a` and `b` as the kept earlier changes leave them); never both names. The file is under neither name only if its own create was not kept, so a first install may lose both the temporary name and the rename: nothing was acknowledged, because the acknowledgement follows the `sync_dir` (point 6) |
   | `remove` or `remove_dir` that `sync_dir` did not cover | applied or not, subject to dependency |

   **A daemon restart** resolves nothing. Every namespace change and
   every byte the old incarnation wrote stays exactly as volatile state
   left it, synced or not, and the next incarnation reads it all, as it
   would read a page cache. Nothing becomes durable by the restart:
   - bytes and changes never synced stay unsynced, and a host crash
     later in the run still resolves them by the table;
   - **bytes whose sync failed stay readable,** and so do changes logged
     before a `sync_dir` that failed and not covered by an earlier
     successful `sync_dir`. The model marks them **sync-failed**. A failed
     sync marks only what was not yet durable: bytes an earlier
     successful sync made durable, and covered changes the log still
     keeps, are never marked. **The mark survives every daemon restart,**
     and a later successful sync on any handle of that identity, in this
     incarnation or the next, does not make marked bytes durable. **Only
     a host crash clears a failed sync:** it resolves the marked bytes as
     unsynced bytes, and clears the mark.
   - **What a later successful sync covers.** For a file: the bytes
     written to that identity after the failed sync returned, never the
     marked ones (a sector rewritten after the failure is unmarked for
     its new bytes). For a directory: each change logged after the failed
     one that is not marked and does not need a marked change. A change
     needs an earlier change when it reads a binding that change made: a
     rename's source name, a remove's name, or a create inside a
     directory a marked `create_dir` made. A change that needs a marked
     change is marked too. A replacing rename does not need the change
     that last bound its target, so a fresh rename onto a marked name is
     covered. At a host crash the last kept change to a name, in log
     order, decides it.

   **What recovery uses as evidence.** The daemon cannot see the model's
   marks, and after a daemon restart a record on disk looks the same
   whether its syncs succeeded or failed. A matching checksum proves the
   bytes are whole, never that they are durable, and a commit boundary
   found on disk is no proof either (its own `sync_dir` may have failed).
   So the next incarnation relies on no record it did not commit itself:
   before it acknowledges anything that rests on such a record, or
   publishes it, it makes a **fresh commit** of the record's content: the
   durable replace plan of point 6 run to a new identity, with every sync
   of that plan succeeding in this incarnation. Syncing the old identity
   again is not a fresh commit, even when that sync succeeds. Until then
   the record may be read only to decide what to commit. The same holds
   within an incarnation after a failed sync: the plan stops, and the
   change is committed again only by a new plan to a new identity
   [OWN].

   A test that models only host crashes cannot catch a restart that
   trusts bytes whose sync failed, and one that models only daemon
   restarts cannot catch a commit that relies on unsynced bytes; the
   simulator models both, and each crash decision (5.6, point 8) names
   its kind.

   These outcomes are the model's. That a platform's filesystem gives no
   worse is O41, point 4's qualification, by crash and power-loss tests
   per platform; the simulator proves the code's order of calls and its
   acknowledgement rule, not the filesystem. **Named residual:** on
   journaled ext4 before Linux 6.8 a failed journal checkpoint write is
   reported by no later sync [UNVERIFIED at source], and a filesystem
   query cannot tell ext2, ext3 and ext4 or the journal mode apart, so no
   startup probe detects the failure itself; a model of failed syncs
   cannot cover a failure the kernel never reports. Each Linux target's
   qualification (O41, point 4) either lists the ext family with this
   residual named or leaves it unqualified, and O41, point 3's startup
   probes refuse unqualified storage.

8. **What M2a enables.** Which disk faults each phase enables is the
   simulator's specification's, under O45, point 2, and O50, point 11. By
   the owner's decision, M2a enables: a read error and no space on the
   read path; the daemon restart over unsynced bytes; and every cut of
   the durable replace plan (its write, sync and rename steps) for the
   state directory's records written in M2a (the format marker, the
   generation record, the host identity, the audit checkpoint and the
   last good policy), with every legal outcome, each run as both a daemon
   restart and a host crash, and including the variants after an injected
   sync failure (a failed file sync before the rename, a failed directory
   sync after it). The rest of the disk-fault gate is M3's: injected write
   and sync errors, torn writes and lost unsynced data anywhere else, and
   crashes at other `Disk` calls. Outside those cuts an M2a crash applies
   every volatile change as if it had been synced.

9. **Restart and the clean store** (C26, point 4). Before any clean
   chunk is read in an incarnation, every chunk written by an earlier
   incarnation is excluded: the cache keeps clean chunks under a
   directory named for the incarnation (a name of at least 128 bits drawn
   from `Rng`), created as point 6 says. `Exists` from that `create_dir`
   means an older incarnation's directory has the name, and a new name is
   drawn, a bounded number of times; after that the clean cache stays off
   for the incarnation and an event is logged. An older directory is
   never adopted. A paged sweep (point 1) removes older ones: each one's
   files with `remove`, then the emptied directory with `remove_dir`; no
   sync is needed, since the clean tree is never durable. Records outside
   the clean tree (C27, point 7) are untouched. The exact layout
   is the cache format specification's; this page requires only the
   exclusion.

### 5.6 The scheduling model under the simulator

1. **Tasks.** Each executor and store-pool worker, up to its configured
   count, each other loop of C39, point 1, and each model front-end
   request source is a task: one standard-library thread spawned by
   `ironafs-sim` with a stated stack size. An operation runs on the
   worker task that took it from the executor. Model servers, the
   network, the disk and the clocks are world events.

2. **Bounded live stacks.** The scenario caps live tasks per
   incarnation; one more is a simulator error. C39, point 1's fixed set
   plus the model front end fits, so executor exhaustion (C39, point 2)
   is reachable.

3. **Yield points: a closed list.** A task hands the baton back only at
   one of these, or at its end:

   | Kind | Where |
   |---|---|
   | a wait on a reply slot | `OneShot::take`, so `Rpc::call` and `Rpc::accept` |
   | a wait on a deadline or a timer | every other `Clock::wait`, `Queue::pop` included |
   | a `Disk` call | every `Disk` method |
   | a hand-off to the bounded executor | a reader's `push` onto the executor's queue: the push never blocks, and the yield follows it |
   | a submission to a binding | the binding's `submit` (5.8): the yield precedes the check |

   Other `Queue::push` calls do not yield. Clock reads and `Rng` draws do
   not yield.

4. **The hand-off protocol.** All scheduling state is under one scheduler
   mutex, in `ironafs-sim`. Per task: an id, its incarnation, a grant
   count, a seen count and a state (starting, running, parked with its
   wake condition, or ended with how). One holder field names the task
   that holds the baton, or none.
   - **Ids** are assigned by the scheduler from a counter when it decides
     to create a task, before the thread is spawned, never from thread
     start order.
   - **Gate.** A task thread's first act, before any task code, is to
     wait on its own condition variable, with the mutex released, until
     its grant count exceeds its seen count.
   - **Grant.** Under the mutex the scheduler increments the task's grant
     count, sets the holder and notifies the task. A grant posted before
     the thread reaches its wait is kept in the count and seen when it
     arrives: no grant is lost. The scheduler then waits on its own
     condition variable, mutex released, until the holder is none.
   - **Yield.** In one critical section the task checks the lock rule
     (5.3, point 6), records its wake condition, sets itself parked,
     clears the holder and notifies the scheduler; then it waits as at
     the gate. On every wake, spurious or not, it re-checks its counts
     and waits again unless a new grant is there; then it sets its seen
     count to the grant count and runs.
   - **End.** The task body runs inside an unwind catcher. On any exit
     (return, crash marker, panic) the wrapper sets the task ended,
     clears the holder and notifies the scheduler. A panic other than the
     crash marker fails the run with its message; the baton is never lost
     to it. The scheduler joins ended threads with the mutex released.
   - Only the holder runs task code. Every other task thread is at its
     gate or in its yield wait.

5. **The enabled list,** in a fixed order: tasks whose wake condition
   holds, by task id; then world events due now, by kind and creation
   order. A wake condition holds when the signal is raised, the cancel
   token has fired, the deadline's virtual clock is failing under a
   scripted fault, the deadline's latched doubt has moved (5.3, point 2)
   whether or not the clock is still failing, the deadline has passed in
   virtual time, or a `Disk` call is ready to complete. Wake conditions
   are read from the watches' flags and the virtual clocks' doubt counts
   while no task runs, so the read is deterministic. A parked waiter whose
   deadline's doubt has moved stays enabled until it runs and its wait
   returns `ClockFailed`, so a clock fault that clears before the waiter
   is chosen never takes it off the list.

6. **Choosing.** The scheduler draws an index from the seed's schedule
   stream and logs the decision: the index and the whole enabled list,
   each entry by its identity (task id, or event kind and creation
   number), so the chosen entry and every unchosen one are on record. It
   grants the baton to that task, or runs that event, until the next
   hand-off. A one-entry draw is still logged.

   **Fairness.** Each entry has an age: the counted decisions since it
   last became enabled or last ran, whichever is later; running resets
   it to 0. An entry of age k or more is overdue. While any entry is
   overdue, the scheduler draws nothing and runs the overdue entry of
   greatest age, ties broken by enabled-list order, one per decision,
   each such decision still logged. Every scenario that checks a deadline
   or liveness property states m, the most entries that can be enabled at
   once (its live-task cap plus its bound on due world events); more is a
   simulator error. Once an entry is overdue, only entries at least as old
   run before it, and each of the others, at most m − 1, runs once and
   returns to age 0, so every enabled entry runs within k + m counted
   decisions. A **counted decision** is a draw, or an overdue entry run
   without a draw.

   **Deadline completions are mandatory.** A completion owner's deadline
   timer (5.7, point 4) is not an ordinary entry: at the first decision at
   which one is due, the scheduler runs it without a draw, ahead of
   overdue entries, and a decision that runs only such timers advances no
   virtual time. Several due at the same instant run one per decision, in
   enabled-list order, all at that instant; their number is bounded by
   the request table's size. These decisions are logged but are not
   counted decisions. A worker's submission can still meet the deadline
   completion in either order: it wins if it runs at a decision before the
   deadline's instant, and loses at or after it. With point 7's split
   advance, virtual time never passes an open contract request's deadline
   with that request open, unless a test switch disables the rule (10.5,
   point 7) or a suspend jumps the boot clock past it (point 7).

   **Race scenarios: a seed sweep and an interleaving witness.** A uniform
   draw among enabled entries often gives a schedule in which two tasks
   never interleave inside the window that matters, so a race property
   asserted on one seed can pass without the race happening. So every
   scenario that asserts a property of a race (two events that meet, in
   each order) states:
   - its **critical window:** the trace events that open and close it;
   - a **seed range,** never one seed (proposed: 256 seeds on a pull
     request, more nightly), every seed run;
   - its two **meeting events:** the named trace events that race (for
     example the owner's arbitration and the caller's abandon, or a world
     event and a task's step), each naming the contested object: call,
     request, file and version;
   - its **interleaving witness:** the order of the two meeting events
     inside the critical window, read from the record of the venue that
     ran them: under the simulator the run's trace, whose events carry
     their decision numbers, so a world event racing a task is witnessed
     with no hand-off between tasks; in a production-code test through a
     hook (10.5, point 2), the hook's own ordered log. A seed is
     **exercised** for an order when both meeting events occur inside the
     window in that order. A seed in which either is missing from the
     window is **not exercised**, never a pass of that order. The count of
     task hand-offs inside the window is reported, not required.

   The scenario fails unless each order it claims is witnessed on at
   least a stated floor of seeds (proposed: 8), and its report gives the
   exercised and not-exercised counts. A scenario that forces an order by
   script still asserts its witness, so a script that stopped forcing it
   fails.

7. **Step cost and time.** Every step advances the three virtual clocks
   by the scenario's step cost, so time moves while work stays runnable,
   and a loop that yields reaches its deadlines. A step's effects happen
   at the instant it starts; its cost is charged after it. **The advance
   is split at each open deadline:** if an open contract request's
   deadline lies after the step's start and at or before the end of its
   cost, the clocks advance only to that deadline; the deadline timers due
   there run at that instant by point 6's mandatory rule; then the rest of
   the cost is charged the same way. So no step carries virtual time past
   an open request's deadline, and a request is completed at its
   deadline's instant, never at the step boundary after it.

   **A suspend is a jump, not a cost.** The clock fault "suspend" (boot
   and wall advance by S, awake does not) is a world event whose effect,
   at the instant it starts, moves the boot and wall clocks forward by S.
   The jump is not split: the host runs nothing while it sleeps, so no
   completion can happen inside it, and C37, point 1, counts suspend on
   the boot clock so that its deadlines expire across a sleep. For an
   open contract request whose deadline is on the boot clock and falls
   inside the jump, the first instant the host runs at or after the
   deadline is the resume instant R = start + S:
   - immediately after the jump, before the suspend event's cost is
     charged and before any other entry runs, the deadline timers now due
     run by point 6's mandatory rule, one per decision, all at R,
     advancing no virtual time; only then is the event's cost charged,
     split as above. A worker's submission therefore never wins a request
     whose deadline the suspend crossed: it either ran before the suspend
     started, or runs after R and loses;
   - the completion is the timeout error, since the completion claim
     (5.7, point 4) evaluates the deadline as passed;
   - an awake-clock deadline is never crossed by a suspend. Other
     boot-clock waits the jump crossed (callbacks, tokens, location
     lifetimes, locks; an `Rpc` call on a boot deadline) are enabled at R
     by point 5 and end `DeadlinePassed` when chosen;
   - **expiry does not depend on detection.** These outcomes follow from
     the boot clock alone, for a suspend of any length. Detection is C37,
     point 4's: the core marks callbacks suspect only if the next sample
     after R shows the boot clock grew more than the awake clock by over
     the threshold, after sampling skew. A shorter suspend marks nothing
     suspect, yet every boot-clock deadline it crossed has still expired
     at R.

   Completion at R is later than the deadline. Whether that is a
   departure from O45, point 3 ("every OS request completes within its
   deadline") depends on the clock of a request's deadline (section 13,
   Q3).

   A task that never yields gets no further step, so time does not move
   for it; the watchdog of point 9 ends the run. With an empty enabled
   list, time jumps to the earliest pending deadline or event. With
   nothing pending and work open, the run is stuck, and fails. A `Disk`
   call may carry a modelled latency; its wake condition is then "now is
   at or after its start plus the latency".

8. **Crash and restart.**
   - **Kind.** Every crash decision is a **daemon restart** or a **host
     crash** (5.5, point 7), drawn from the fault stream or scripted, and
     recorded with the decision. Both end the incarnation by the rules
     below; they differ only in what the disk keeps.
   - **Cut.** At a crash decision the scheduler records the cut. On a
     host crash the simulated disk resolves durable state from its state
     at the cut; on a daemon restart it keeps the volatile state at the
     cut, sync-failed marks included. The world is frozen: no world event
     runs, and no old task gets any grant but its teardown.
   - **Teardown,** one old task at a time, by task id: the task is granted
     with a crash marker, its pending seam call unwinds from inside, and
     the scheduler waits for it to end before the next. The baton stays
     exclusive during teardown.
   - **Teardown has no observable effect.** Seam calls made while
     unwinding are ignored and counted by kind, never applied, as a real
     crash runs no cleanup; core code therefore never relies on `Drop`
     for I/O. Trace events the old incarnation emits after the cut are
     counted, not appended. The new incarnation shares no in-memory
     object with the old: only the world (the disk as the crash kind
     leaves it, the network, the models) carries over. The core has no
     thread-locals and no mutable statics (5.2, point 5), so none
     survives.
   - A panic while unwinding aborts the process, as does a stack
     overflow [UNVERIFIED at the pinned toolchain: section 13, Q16];
     point 10's supervisor reports either, so the design holds whichever
     way they end.
   - The simulator's test binary is built with unwinding panics, and
     `ironafs-sim` refuses to compile under `panic = "abort"`, through a
     compile error under `cfg(panic = "abort")` [UNVERIFIED at the pinned
     toolchain: section 13, Q16].
   - The next incarnation starts when the last old task has ended.

9. **Failures the scheduler reports.** Each fails the run, with the seed
   and the replay command:
   - **A lock held at a yield:** caught at the yield (point 4), naming
     the yield kind, the task and the lock's level.
   - **A full waker list:** reported by the simulated `Clock::wait` when
     a watch refuses its registration (5.3, point 3), naming the task,
     the watch and the list's bound.
   - **A contended `Lock`,** reachable only if the check above is
     bypassed: the observer build fails at once, without blocking, naming
     both tasks and the level.
   - **A lost baton.** The scheduler waits for the holder to clear under
     a real-time watchdog (an initial setting of 30 seconds, restarted at
     every grant). On expiry the run stops failed, never a pass, reported
     as an unconfirmed lost baton at step s, with the task, its last
     yield, the step, the seed, the replay command and the decision log up
     to the last committed decision. The stop shows that a task ran past
     the watchdog without yielding: a hidden block, a loop that never
     yields, or host starvation. A replay can help triage but proves none
     of them. The watchdog never feeds a decision, so it cannot change a
     trace.
   - **Endless yielding** spends the step budget: "budget reached".

   The watchdog's real-time read and the supervisor's outer limit (point
   10) are the simulator's only real-clock reads: a named departure from
   coding standards rule 7, each one call site with a local `expect`
   attribute and its reason [OWN: a hidden block would otherwise hang CI
   to its job timeout with no diagnosis].

10. **The supervisor.** Every run whose result is evidence (the pull
    request seeds, the determinism checks, replays) runs in a child
    process under a parent. The child writes each decision record (point
    6) to the parent before acting on it, so the parent always holds the
    committed prefix. The parent enforces an outer real-time limit: the
    run budget plus the watchdog interval plus a fixed reporting margin
    (proposed 10 seconds), so the watchdog can always finish and report
    first. A child that aborts, crashes, overflows a stack or passes the
    outer limit is a simulator error, reported with the seed, the
    scenario and the committed prefix: never a pass, and never a breach
    of a client invariant. Host resource failures (a thread that cannot
    be spawned, memory exhausted) are simulator errors too; the faults
    the simulator promises to reproduce are modelled inputs.

11. **Replay.** Byte-identical replay holds under five conditions, each
    enforced:
    - **(a) The scheduler controls every observable transition.** Every
      blocking point of the core is a yield point of point 3: statically
      by 5.2, point 5, and at run time by point 9.
    - **(b) Each task is deterministic between yields.** It reads only
      its arguments, seam results and in-memory state written by tasks in
      decided order; never a thread identity, an address, a hashed order,
      the environment, process state or real time (5.2, point 5). Checked
      by strict replay and its plants (10.5, point 5).
    - **(c) The simulator's own order is canonical.** Ids by point 4; the
      enabled list by point 5; ordered collections only; the trace
      carries no pointer, thread id, thread name or host value, in a
      fixed encoding. **Error text in the trace comes from a fixed
      vocabulary:** an error is recorded as the seam's error kind and
      cause (5.4, point 2; 5.5, point 1), a stable code, never the
      `Display` of an operating-system error, a message from the
      platform's error-string function, or any text that depends on the
      locale, the host or the build. An injected fault is recorded by its
      kind and its own id. The vocabulary is a closed list in
      `ironafs-sim`; an error outside it is a simulator error, never free
      text.
    - **(d) The inputs are pinned:** the same test binary (commit, pinned
      toolchain, target triple, profile and features), the scenario with
      its parameters and effective configuration, and the seed. The
      replay command names all of them.
    - **(e) Host failures are not inputs:** they stop the run (point 10).

    **Two-process replay.** The determinism check runs the same seed in
    two separate child processes and compares their whole traces, byte
    for byte; a comparison inside one process would miss state that
    leaks between runs.

    **Strict replay** feeds the logged indices to the schedule stream.
    Each decision record carries the whole enabled list, every entry's
    identity in order. At every decision the replay compares the full
    ordered list with the record, and after every step it compares the
    trace events that step appended (seam results and reply bytes
    included) with the original run's, in the canonical encoding. The
    first difference stops the run as a divergence at that decision. A
    replay that needs a decision past the end of the log, or ends with
    records unused, is a divergence too, and the replayed trace must
    reach the original's terminal event, byte for byte. A lenient replay
    (an out-of-range index takes entry 0 and is noted) is minimisation's
    mode only and is never evidence of determinism. Byte-identical replay
    is claimed per target triple (section 13, Q7).

    **Golden traces.** Replay compares two runs of one commit, so it
    cannot see a behaviour change between commits. For a few fixed seeds
    of named scenarios (proposed: 4 seeds of the sample scenario of 10.5,
    point 5), the whole canonical trace through the terminal event is
    committed in the repository as a fixture, per target triple the
    replay claim covers, and every run of those tests compares it
    exactly. The scenario names its golden seeds; the committed set for a
    covered target must be exactly those seeds, so a missing, empty or
    partial fixture fails, and so does a run on a target with no
    committed set. A difference fails the test, and is resolved either
    as a defect or as a deliberate change, by regenerating the fixtures in
    the same commit as the change, with the simulator's version line. A
    fixture is capped at 256 KiB per seed (proposed; the real size is
    known after the first run) [OWN: exact bytes need no hash, so no
    cryptographic code is involved, and a failure shows a readable diff;
    section 13, Q15].

12. **Shutdown and quiet.** When the workload has ended, no contract
    request is open, and nothing is pending but loops' horizon waits, the
    scheduler fires the run's shutdown `Cancel`. Every loop's wait ends
    `Cancelled` and the loop returns; the run ends when every task has
    ended. A loop still running when the step budget runs out after
    shutdown is reported by name as "budget reached".

13. **Never in the core.** Threads, the baton, parking, the scheduler and
    the supervisor are in `ironafs-sim` only. The core sees four seam
    values and cannot tell which world it runs in.

### 5.7 Production mapping

1. **`Clock::wait`** follows 5.3, points 3 and 4, with `ironafs-os`'s
   waker (a condition variable per waiting thread). Each block lasts at
   most the smaller of the time left on the deadline's clock and one
   slice (an initial setting of one second, measured on resume and under
   idle load), then the wait evaluates again. The time left comes from
   one `Clock::sample`. A failed read ends every wait, on either clock,
   as `ClockFailed`; a regression ends every wait on a boot deadline as
   `ClockFailed`, and no wait on an awake deadline (5.3, point 2; C37,
   points 4 and 5). Each waiting thread samples at least once per slice,
   and each failure or regression moves the doubt counts that 5.3, point
   2, names, which the outstanding deadlines latch, so every wait affected
   learns of it within one slice. The `Clock` implementation counts each
   episode (a detected suspend, a regression, a failed read, a wall jump)
   in the episode count of every later `Stamp`, which the core reads at
   its next serve and which marks callbacks suspect before the answer
   (C32, point 5). The slice is a
   polling interval, not a guarantee: host scheduling can add to it, and
   whether a condition variable's timeout counts suspend is not relied on
   [UNVERIFIED per platform].

2. `Rpc` is the same `RxRpc` as under the simulator (5.4, point 7).
   `Disk` is `ironafs-disk`, opened once on the state directory's root
   after the state directory's lock is held; taking the lock belongs to
   the driver's construction.

3. `ironafsd` spawns every thread of C39, point 1, and runs on each a loop
   the core exposes. The core spawns none (O45, point 5).

   | Thread (C39, point 1) | Runs | Blocks only on |
   |---|---|---|
   | `rx` | `ironafs-net`'s loop over the endpoint | its sockets, with a timeout and the wake path of 5.4, point 9; never a core lock (C30, point 2) |
   | callbacks | the callback service loop | `Rpc::accept` |
   | housekeeping | the housekeeping loop | `Clock::wait` to the next expiry |
   | store pool, executor | the worker loops | `Queue::pop`, `Rpc`, `Disk` |
   | notifiers | the notifier loop | `Queue::pop`, the binding |
   | layer readers | the layer | its device; `Queue::push` never blocks (C39, point 2) |

   Under the simulator: the same, with the simulated `rx` task and model
   front-end request sources. Shutdown is a `Cancel` given to every loop;
   its spawner joins it.

4. **Completion does not wait on a worker.** A worker held in a `Disk`
   call or an `Rpc` wait cannot answer its request in time. The request's
   owner on the ingress side, which already handles cancellation and
   cleanup (C39, point 2), completes the request with its timeout error at
   the deadline. This page requires of that completion owner:
   - **it is independent of the worker, and is not a thread that blocks
     on the device:** a layer's reader can be held in a device read for
     as long as the kernel sends nothing, so the deadline is served by a
     loop that waits with `Clock::wait` on the earliest open request
     deadline (proposed: the housekeeping loop, over the request table;
     C39, point 1, already gives it expiry);
   - **one completion, arbitrated, with expiry in the claim:** each
     request's entry moves from open to completed once, in one serialised
     step. The worker's submission (5.8, point 2) and the deadline
     completion each try that step; the first wins, and the other finds
     the entry completed and is discarded. The step itself evaluates the
     request's deadline (5.3, point 4, doubt included): if it has passed,
     the step completes the entry with the timeout error, whoever makes
     it, and a worker's success is discarded. So a worker that submits
     after the deadline, before the completion owner's timer has run,
     never delivers success late;
   - its completions to the binding are bounded by the request table's
     size, and shutdown completes every open entry once, with an error,
     never a success, before the binding closes.

   The contract specification names the completion owner and the request
   table. Under the simulator the model front end plays it.

### 5.8 A worked operation

A read of one chunk, on an executor worker task, showing each yield
point and each pure decision. The names of pure functions are
illustrative; the shape is the requirement.

```rust
/// One attempt. `None`: the fetch failed its fence and was discarded.
pub fn read_chunk<R: Rpc, D: Disk, C: Clock>(cx: &Core<R, D, C>, req: &ReadRequest)
    -> Result<Option<Fenced<Bytes>>, VfsError>
{
    // A pure decision under a lock, released before any yield (5.3, point 6).
    let (plan, ticket) = cx.objects.lock().plan_read(req.fid, req.range, req.context)?;
    if let ReadPlan::TryChunk(path) = &plan {
        if let Ok(bytes) = cx.disk.read(path, ReadRange::Whole { max: CHUNK_CAP }) { // yield
            if let Ok(chunk) = cache::chunk::decode(&bytes, &ticket) {               // pure
                return Ok(Some(Fenced::new(chunk.slice(req.range)?, ticket)));    // checked at submission
            }
        } // an unreadable or bad chunk is a miss (C27, point 2)
    }
    let conn = cx.conns.for_key(&ticket.conn_key(), &cx.rpc)?;          // `open`: no wait
    let request = proto::fs::fetch_data::encode(&ticket.fetch_args())?;  // pure
    let reply = cx.rpc.call(conn, &request, FETCH_REPLY_CAP, req.deadline, &req.cancel.watch()) // yield
        .map_err(|e| errors::for_fetch(e, ticket.server()))?;           // C46, point 3
    let data = proto::fs::fetch_data::decode(&reply.bytes)?;            // pure, fuzzed
    // Installed only if no break counter, and no overload, moved since the
    // ticket (C16, point 2; C26, point 1). The callback lifetime counts from
    // `reply.first_sent`'s boot time (C26, point 2); none if `None`.
    let Some(install) = cx.objects.lock().finish_fetch(ticket, &data, reply.first_sent) else {
        return Ok(None);                                                // discarded, never served
    };
    if let Some(p) = install.cache_plan() {                             // the clean chunk plan
        if let Err(e) = disk::run(&cx.disk, &p) { cx.events.cache_write_skipped(e); } // yields
    }
    Ok(Some(install.reply(req.range)?))                                 // never short (C46, point 4)
}

pub fn serve_read<R: Rpc, D: Disk, C: Clock>(cx: &Core<R, D, C>, req: &ReadRequest,
    binding: &dyn Binding) -> Result<(), VfsError>
{
    for _ in 0..cx.limits.fence_retries {                               // C16, point 2: bounded
        let Some(reply) = read_chunk(cx, req)? else { continue };
        match binding.submit(req.id, reply) {                           // yield, then check and start
            Submitted::Started => return Ok(()),                        // counts as delivered (C16, point 3)
            Submitted::Fenced => continue,                              // discarded, never served
        }
    }
    binding.fail(req.id, VfsError::Stale)
}
```

1. **Yields:** `disk.read`, `rpc.call`, each call inside `disk::run`, and
   `binding.submit`. Between them, other tasks and world events run in an
   order drawn from the seed, breaks included.

2. **The fence goes to the binding.** `Fenced<T>` carries the ticket: the
   break counters of object, volume and server, the overload record's
   counter, and the credential and rights generations (C16, point 3).
   Nothing before submission is the fence; an earlier check is only a
   hint. `binding.submit` yields first; when the task next holds the
   baton, in that one step and with no further yield:
   - for a binding in the daemon's process: the core compares the clocks
     (C32, point 5), compares the ticket with the current counters and
     the overload record (5.4, point 7) under the object lock, releases
     it, and, if they match, records the submission start, claiming the
     request's entry (5.7, point 4);
   - for a binding in its own process: the model front end compares the
     reply's fence epoch with the latest it has applied (C16, point 6)
     and, if they are equal, records the submission start.

   A break recorded at any earlier step is effective and stops the reply;
   a reply whose start is recorded counts as delivered (C16, point 3)
   [ADR 0004 C16.3: Firm]. How a production binding makes the check and
   the write to the kernel one serialised step, without holding a core
   lock across the kernel write (C39, point 4), is the contract
   specification's.

3. **A failed fence,** at the install or at submission, discards the
   reply; the read is retried, at most `fence_retries` times in all, then
   fails with `Stale` (C16, point 2). The count is the contract
   specification's.

### 5.9 Conformance and the fault feature

1. **One conformance body per seam trait.** `ironafs-test-support` holds
   one body for each of `Clock`, `Rng`, `Rpc` and `Disk`. Each body runs
   against every implementation of its seam:
   - `Clock` and `Rng`: the fake (the seeded implementation in
     `ironafs-test-support`), the simulator's and the production one;
   - `Rpc`: `RxRpc` over the simulated `rx` task and over the production
     `rx` thread;
   - `Disk`: the simulated disk and `ironafs-disk`. There is no separate
     `Disk` or `Rpc` fake: a unit test that needs one uses the
     simulator's, with its faults off.

   A body that runs against fewer than the implementations listed for its
   seam, or passes with no case run, fails [OWN: one body is what makes
   the simulator's answers the production driver's answers].
2. **Fault conformance.** For the seams that inject faults (`Disk`, and
   `Rpc` through the model network), the body also checks that each
   injected fault surfaces as the seam's ordinary error from 5.4, point
   2, or 5.5, point 1, carries its own id in the trace, and leaves the
   state 5.5, point 4, allows. A scripted fault or crash point that never
   fired fails its run; every crash point also runs disarmed, as a
   control that must reach the point.
3. **The fault feature and its gate** (the test strategy's
   specification, issue #63, lists the obligations). Fault and
   crash-point entry points in a production driver exist only behind one
   Cargo feature, off by default, and are compiled only with it. A crash point does not kill
   the process itself: the test harness does. No shipped binary is built
   with the feature, and a check over the release build of every shipped
   binary proves the entry points absent. The definition of the feature,
   and any change to it or to what enables it, is class S.
4. **What the bodies check, at least:** for `Disk`, the cases of 10.5,
   point 1; for `Rpc`, the call lifecycle of 10.5, point 2, admission per
   lane, a reply over its cap, the wake bound of 5.4, point 9, and
   shutdown with the call table and every connection full; for `Clock`,
   the wait protocol of 5.3, points 3 and 4, and the counts, episodes and
   doubt latch of 5.3, point 2; for
   `Rng`, that a seeded source reproduces its sequence from its seed and
   that a failed source fails the operation that asked.

## 6. Arithmetic

1. `ReadRange::Part`: `offset + len` is checked; an overflow is
   `TooLarge`.
2. Byte counts are 64-bit; queue caps and the in-flight budget are
   checked before adding.
3. Deadline arithmetic is the `ironafs-time` types' (checked, 64-bit
   nanoseconds; C38, point 1). The production block length is the
   smaller of the time left and the slice, and saturates at zero.
4. The seeded clock's advance to a deadline is checked; an overflow
   returns `ClockFailed` (5.2, point 3).
5. Grant and seen counts, ages and decision numbers are 64-bit; the step
   budget keeps them far below the limit.
6. Doubt counts and the episode count are 64-bit and only increase; an
   increment that would overflow is a clock failure.

## 7. Errors and failure

1. `RpcError` and `DiskError` are the two seams' only errors; `Waited` is
   the wait's only outcome. `ironafs-client` maps them per operation (C46,
   points 1 to 3).
2. No seam method or primitive panics: not on a second `OneShot::put`, a
   full `Queue`, a full waker list, or a poisoned mutex (a primitive
   recovers its value; the panic that poisoned it has already failed the
   run).
3. A failure never looks like success (C46, point 4): no short `read`
   inside the file without an error; no empty reply in place of
   `NoReply`; no `NotSent` after a possible send; no abort of a
   state-changing call taken as "not applied" without its procedure's
   table and authoritative evidence (5.4, point 2); no record trusted as
   durable after a failed sync (5.5, point 4).
4. Every simulator error stops the run as failed and is never reported as
   a pass or as a breach of a client invariant (5.6, points 9 and 10).

## 8. Security

1. Reply and file bytes are decoded only by fuzzed decoders (O47, point
   3).
2. A path built from server data cannot escape the state directory:
   `StorePath` validation checks the name, and the driver's root-relative,
   no-follow resolution checks every component at every call (5.5,
   point 2).
3. `create` makes files readable only by the daemon's account (C27,
   point 7).
4. An off-path sender cannot raise callback trust: trust is earned only
   through a keepalive reply's evidence (5.4, point 1), and a callback
   overload fences what it might have hidden (5.4, point 7).
5. The baton, the observer, the supervisor, the crash marker and the
   fault entry points are in no shipped artefact (C4, point 7; 5.9,
   point 3).
6. The danger here is a false pass: a blocking point the scheduler cannot
   see, a task that reads the host, or a test that cannot fail. 5.2,
   point 5, 5.3, point 6, 5.6, points 9 to 11, and the planted tests of
   10.5 close them.

## 9. How the invariants are kept

Test names are the ones the implementing pull requests use; each is
listed with its crate.

| Quoted decision | Kept by | Test that fails if it is broken |
|---|---|---|
| C3, points 1 and 3 | 5.2 to 5.5 | `xtask`: `seams::core_names_no_real_io` (planted, one per banned item, through a re-export and a wrapper) |
| C3, points 4 and 5; O45, point 5 | 5.6, point 13; 5.7, point 3 | `xtask`: `seams::core_spawns_no_thread` (planted) |
| C3, point 6, and its trigger | 5.6, points 4 and 11 | `ironafs-sim`: `replay::sample_read_replays_byte_identically`, `replay::planted_divergence_is_caught_at_its_decision` |
| C4, points 1, 4 and 7 | 5.2, points 1, 2 and 5 | `xtask`: `check-deps --self-test`, `check-pure --self-test`, `seams::pure_crate_calls_no_seam` (planted, `ironafs-time` included: a `Clock` method called inside it fails; a seam taken, bound, implemented, renamed, re-exported or named through a module path, a raw identifier, an uppercase alias or glob of its home crate, a `::`-rooted path through an enum named like the home crate, a home enum's name used without its import, the home crate renamed like a home enum inside it, a module named like a local enum with that variant, and a `::`-rooted path through another crate; a seam renamed in its home crate's root re-export; a variant with a seam's name reached by any path but `Self::`; a seam after a `#[cfg(test)]` field, which hides nothing after it; in the production `Rng`'s crate, the seam held, called, renamed on import or re-exported, and another seam implemented; another pure crate implementing `Rng`; a pure crate's dependency renamed in cargo's resolved graph) |
| C16, points 2 and 3 | 5.8, points 2 and 3 | `ironafs-client`: `read::break_before_submission_is_fenced` (both binding roles) |
| C16, points 1 and 6; C31 | 5.4, point 7; 5.8, point 2 | `ironafs-client`: `rpc::callback_overload_recorded_before_drop`, `read::overload_fences_install`, `read::no_ticket_over_pending_overload`; `binding::overload_pushes_epoch_advance` |
| C24, point 1 | 5.4, point 2 | `ironafs-client`: `errors::unprotected_matching_abort_is_ambiguous`, `errors::abort_before_rxgk_is_never_authoritative` |
| C27, point 2 | 5.5, points 4 and 6 | `ironafs-client`: `read::bad_chunk_is_a_miss` (read errors on), `read::any_clean_read_error_is_a_miss` (each `DiskError` kind) |
| C28, points 3 and 4 | 5.4, points 2 and 4 | `ironafs-client`: `rpc::lifecycle_*` (10.5, point 2), `rpc::send_decided_inside_guard`, `rpc::late_handoff_is_bracketed_and_never_succeeds`, `errors::state_changing_abort_is_ambiguous`, `errors::unprotected_matching_abort_is_ambiguous` |
| C28, point 6; C29, point 9; N32, point 6 | 5.5, points 4, 6 and 7 | `ironafs-sim`: `disk::durable_replace_crash_at_every_cut`, `disk::one_ack_after_record_and_boundary`, `disk::restart_never_trusts_failed_sync`, `disk::marked_log_keeps_covered_suffix`, `disk::failed_sync_never_marks_covered`, `disk::no_ack_in_a_directory_not_made_this_incarnation`, `disk::root_parent_synced_at_construction` |
| C32, points 1 and 4 | 5.4, point 1; 5.3, point 2 | `ironafs-sim`: `clock::keepalive_across_suspend_earns_no_trust`, `clock::keepalive_across_wall_jump_earns_no_trust`, `clock::trust_lapses_at_e_after_received` |
| C32, point 5 | 5.3, point 2; 5.7, point 1; 5.8, point 2 | `ironafs-sim`: `clock::episode_marks_suspect_before_reply` (a suspend, a regression, a failed read and a wall jump) |
| C37, points 4 and 5 | 5.3, points 1, 2 and 4; 5.6, point 5; 5.7, point 1 | `ironafs-sim`: `clock::failed_read_expires_every_deadline`, `clock::regression_expires_boot_deadlines_only`, `clock::latched_doubt_outlives_the_fault`, `clock::no_deadline_without_a_valid_sample`; `ironafs-time`: `deadline::stale_stamp_gives_an_already_failed_deadline`, `deadline::regression_between_from_and_now_fails_the_deadline`, `deadline::from_after_now_is_refused` (a higher count; a later instant on the deadline's clock) and the compile-fail doc-test that a `Deadline` cannot be built, destructured or edited outside the crate; `xtask`: `seams::deadline_built_from_a_fresh_sample` (planted: a kept `now`; a `now` from a parameter that is not the clock; the clock parameter rebound before the sample; a sampling function with no `from` parameter, or defined in another module or a nested one; a parameter or pattern sharing the fresh sample's name; a constructor before the binding; a constructor whose `from` is another parameter; a constructor outside the sampling function, and in `ironafs-time` outside its definition; a second allowed function; a module-level allowance; the constructor carried out by a closure, by a function pointer or inside a macro call; a fresh `from`; a forged high-count `from`; a `#[path]` module, also inside `cfg_attr`; an `include!`, also renamed by a `use`; an inner `#![cfg(test)]` in a module, hiding nothing after it; an unreadable source; and, as recorded limits that pass, an equal-count forged `from` through a forged `.arrival` and through a forged `Reply`); `check-bans` (both constructor bans fire; `ironafs-time`'s `clippy.toml` is the root one without them); the `Clock` conformance body |
| C39, point 3 | 5.3, points 3 and 5; 5.4, points 7 and 8 | `ironafs-sync`: `queue_refuses_over_cap`, `oneshot_cancel_wakes`; `ironafs-os`: `waker_raise_at_check_to_sleep_boundary`; `ironafs-client`: `rpc::admission_lanes`, `rpc::control_lane_never_full_at_shutdown`; `ironafs-sim`: `inbound_queue_full_drops` |
| C39, point 4 | 5.3, point 6; 5.6, point 9 | `ironafs-sim`: `lock_held_at_yield_fails`, `contended_lock_fails` |
| C46, point 3 | 5.4, point 2 | `ironafs-client`: `errors::abort_code_needs_rpc_and_server` |
| C46, point 4 | 5.4, point 4; 5.5, point 1; 7, point 3 | `ironafs-client`: `rpc::verdict_wait_failure_is_no_reply`; the `Disk` conformance body's `part_inside_is_exact`, `part_past_end` and `whole_over_max` |
| N7, point 2; point 5 read as the transport's acceptance (5.4, point 4, the close fence) | 5.4, points 4 and 5 | `ironafs-client`: `rpc::close_fences_queued_sends` (both orders of `close` and the owner's arbitration; `close` against a hold of the send lock; no datagram of the connection accepted by the transport after `close` returns) |
| Coding standards rule 7 | 5.2, point 5; 5.6, point 9 | `xtask`: the disallowed-methods self-test; `ironafs-sim`: `planted_host_read_breaks_replay` |

## 10. Tests

### 10.1 Golden vectors

Not applicable: this page fixes no byte format. The golden traces of 5.6,
point 11, are fixtures of the simulator, not vectors of a format.

### 10.2 Round-trip properties

Not applicable.

### 10.3 Reject corpus

Not applicable. `StorePath` has a table test of refused names (a `.` or
`..` component, a character outside the set, an empty component, one
component too many).

### 10.4 Fuzz target

Not applicable: no decoder.

### 10.5 Acceptance cases

Every case asserts its exact outcome (the failure, the error, the
decision), the child's exit status, and a complete parsed trace with its
terminal event. A truncated trace never compares equal to anything.
Mutations listed with a case are each run and must be caught.

1. **`Disk` conformance,** on `ironafs-disk` and the simulated disk,
   each call's every error of 5.5, point 1, by its exact variant:
   - `create` and `create_dir` exclusive, `Exists` on a second one;
     `create_dir` then its parent's `sync_dir`;
   - a replacing rename within a directory; `NotFound` for a missing
     source; `IsDir` when a name is a directory;
   - `remove` of a file; `IsDir` on a directory; `remove_dir` of an empty
     directory, `NotEmpty` on one holding an entry;
   - `NotFound` from `read`, `rename`, `remove`, `remove_dir`, `list` and
     `sync_dir` on a missing name or directory;
   - `read`: `part_inside_is_exact` (`Part { offset: 0, len: 4096 }` on an
     8192-byte file returns exactly 4096 bytes, and a range ending exactly
     at the end returns all of it); `part_past_end` (fewer bytes only past
     the end, none at or past it); `whole_over_max` (`TooLarge` and no
     bytes); an `offset + len` overflow is `TooLarge`;
   - `list`: an empty directory gives no entries; byte order, paging to
     the end, and each entry's kind (`File`, `Dir`, and `Other` for a
     planted link, never followed);
   - `space` returns a value on the store's own volume;
   - the paged sweep of 5.5, point 9, over a tree with files, an emptied
     directory and a planted link: every file, and every directory that
     holds no link, removed; the link and its directory left in place,
     with the event;
   - `NoSpace` on a full small volume both at the open (the name is then
     absent) and at a write (the name is present, empty or a prefix),
     each followed by the caller's cleanup; an injected sync failure;
   - a symbolic link planted at each component of a path, refused with
     `IsLink` at lookup and at every mutation: `create`, `create_dir`,
     `rename`, `remove`, `remove_dir`, `sync_dir` and `list`.

   On `ironafs-os`'s waker, through a test hook: a raise
   injected just after the evaluation and before the block, and one just
   after the block starts, both ending `Signalled` well inside one slice;
   a failed clock sample ends a wait on each clock as `ClockFailed`; a
   scripted regression ends a wait on a boot deadline as `ClockFailed`
   and not one on an awake deadline.

2. **Call lifecycle,** each case where two events meet run as a race
   scenario (5.6, point 6), over its seed range, with its witness per
   order:
   - an abandon while queued: `NotSent`, and the network model sees no
     packet of that call, ever;
   - an abandon in the step of the first hand-off, before and after it:
     `NotSent` and `NoReply`;
   - a reply's processing by the owner and the caller's setting of the
     abandon flag in adjacent steps, both orders: whichever comes first in
     decided order wins;
   - `close` and shutdown with calls queued and handed;
   - **`close` against a hand-off** (`rpc::close_fences_queued_sends`):
     a queued call, and `close` of its connection returning before the
     owner processes either the submission or the close message, in both
     orders of the two messages: the call settles `NotSent(Closed)`, and
     the network model sees no packet of it, ever. Through the
     production hook, `close` called while the owner holds the
     connection's send lock between its fence read and the send: `close`
     returns only after that hold ends, the call is handed and settles
     `NoReply(Closed)`, and the transport's log shows the acceptance
     before `close`'s return; a retransmission or acknowledgement of a
     handed call due after `close` returns is dropped, and the transport's
     log shows no datagram of the connection after the return.
     Mutations: the fence set only by the control-lane message; the
     fence left out of the arbitration; `close` firing the fence outside
     the send lock; a later datagram sent without the fence read;
   - a send error of each kind: "would block" and a transient error leave
     the call queued, and it is sent at the next attempt or ends
     `NotSent(Deadline)`; an unreachable error settles
     `NotSent(Unreachable)` at once; no attempt is made without a wait
     between;
   - a clock failure during the wait, queued and handed;
   - a security failure before and after sending; a reply over its cap:
     `NoReply(ReplyTooLarge)`, and a fetch that meets it fails at once,
     is not retried and does not mark its server unreachable;
   - the deadline passes, or the cancel fires, while the call is queued
     and before any abandon wake is processed: the owner settles
     `NotSent` at its next transition, and no packet of the call is ever
     sent;
   - a reply delivered after the deadline, or after the abandon flag is
     set, but processed before the abandon wake: `NoReply`, the reply
     dropped;
   - the caller's verdict wait planted to fail while the call is queued:
     the caller returns `NoReply`, and no packet of the call is ever sent;
   - an abort matching a live store call: the client reports a lost
     reply, never "not applied" and never success; an unprotected abort
     matching a live call of a procedure whose test table says the code
     means "not applied": still a lost reply;
   - **the decision guard,** through a test hook in the production `rx`
     loop: the owner held after its arbitration and before the transport
     accepts the packet, while the caller's deadline passes and it tries
     to abandon. The abandon waits for the guard. With the send then
     accepted, the call is handed and the caller gets `NoReply`, and the
     transport's log shows acceptance before the abandon. With the send
     made to return "would block", the call stays queued, the abandon is
     recorded, the owner settles `NotSent`, and the transport accepts no
     packet of the call;
   - **against the actual deadline,** through the same hook: the owner
     paused after its `pre` sample and before the send while a test clock
     moves past the deadline, then the send accepted. The call is handed
     late, its verdict is `NoReply(Deadline)` even when a reply is then
     delivered, the late-hand-off event and its counter appear, and a
     test procedure marked state-changing is a lost reply. With `pre`
     inside the margin the call settles `NotSent(Deadline)` and nothing
     is accepted. Across every interleaving the hook allows, from its
     log: every acceptance lies between its `pre` and `post`; every
     acceptance with `post` after the deadline is recorded as handed late
     and never yields a reply or `NotSent`; no acceptance follows a
     recorded abandon or cancel. Mutations: take `post` after the hold
     ends; mark a late hand-off in time; drop the margin check; set
     `first_sent` from `post`.

3. **Fence.** A break planted during the cache fill (between the disk
   read and the submission) and at the submission's yield, for a binding
   in the daemon's process and one in its own: no submission starts after
   the break is effective, the reply is discarded, and after
   `fence_retries` fenced attempts the read fails with `Stale`.

4. **Crash points,** for the durable replace plan, at every cut of 5.5,
   point 7, with every legal outcome scripted (both keeping and losing
   each unsynced change), each run as a daemon restart and as a host
   crash:
   - after a host crash a reader finds exactly the last acknowledged
     record, byte for byte with its checksum, or a later committed one;
     never a mixture. Cuts in a first install include the one that keeps
     neither the temporary name nor the rename: no record, and none was
     acknowledged;
   - in the run with no fault, exactly one acknowledgement, at a step
     after the last successful `sync_dir` of both the record and its
     commit boundary; a sync failure acknowledges nothing (the count is
     zero). Mutations: skip the file sync; skip the directory sync;
     acknowledge before the directory sync; skip the commit boundary's
     write, or the second copy's;
   - after a daemon restart the next incarnation reads every byte and
     name the old one wrote, unsynced included, and acknowledges or
     publishes nothing on them until its own fresh commit's syncs have
     succeeded; a host crash later in the run then resolves what is still
     unsynced by the table, and the reader still finds only an
     acknowledged or later committed record;
   - **after an injected sync failure** (the records of 5.5, point 8, in
     M2a; every other `Disk` user with M3's gate): after a daemon restart
     the sync-failed bytes are readable and match, a later successful
     sync of that file does not clear the mark, and a host crash may
     still lose them. A `sync_dir` that fails after the rename, a daemon
     restart, the restarted daemon's fresh commit to a new identity with
     every sync succeeding, then a host crash at each cut after it: the
     reader finds the fresh commit's record; with the host crash before
     the fresh commit's `sync_dir`, the earlier acknowledged record, the
     marked rename's record or the fresh one, whole, never a mixture.
     Nothing was acknowledged or published from the marked rename.
     Mutations: the restarted daemon re-syncs the old identity and
     publishes it; it publishes on a matching checksum; it treats a commit
     boundary found on disk as proof;
   - **the model's own rules** (M3's gate): a marked replacing rename onto
     a name, a later covered replacing rename onto it, then a newer
     replacing rename onto it that a second failed `sync_dir` marks, then
     a host crash, every legal subset enumerated: the name's images are
     exactly the covered rename's and the newer marked rename's, never the
     older marked one's. A marked rename, a covered rename of the same
     name, an unrelated create and a second failed `sync_dir`, then a host
     crash: the covered image in every outcome. Each also runs with a
     daemon restart in between, with the same images. Mutations of the
     model: a daemon restart resolved as a host crash; a later successful
     sync that clears a mark; covered changes applied after the kept
     marked ones instead of in log order; a covered change dropped by the
     crash; a successful `sync_dir` that folds a covered change logged
     after an unresolved marked one; a failed `sync_dir` that marks
     covered changes;
   - **handles, root and lost sectors:** a driver construction that finds
     the state directory present still syncs its parent, and one whose
     parent sync fails refuses the start; torn outcomes read lost sectors
     as zeros and as junk. Mutations: a `sync_dir` through a handle opened
     afresh; a construction that skips the parent's sync because the
     directory exists;
   - **a directory whose creation was not made durable**
     (`disk::no_ack_in_a_directory_not_made_this_incarnation`): a
     `create_dir` whose parent `sync_dir` fails; in the same incarnation,
     no durable record is acknowledged in it, and the store uses a fresh
     name or refuses durable writes; then a daemon restart, and the next
     incarnation, finding the directory, acknowledges no durable record
     in it; then a host crash at each cut, with the directory kept and
     lost: no acknowledged record is lost. Mutation: a store that
     acknowledges a record in a directory it found at start.

   The exclusion of clean chunks across a restart (5.5, point 9) is
   checked by its own case, with the mutation "reuse the older clean
   tree".

5. **Replay,** on the operation of 5.8 under a sample scenario (one
   server, loss and duplication, a second client changing the file, a
   disk read error, no space, one restart):
   - two separate processes with the same seed give byte-identical,
     non-empty traces through the terminal event; strict replay of the
     decision log gives the same bytes;
   - not vacuous: replies arrived, faults reached the operation, choices
     were real, and the seeds explored more than one schedule, compared on
     the chosen task and event identities, never on the seed or other
     printed metadata;
   - the golden traces match the committed fixtures, whose seed set is
     exactly the scenario's golden seeds for the target;
   - planted divergences are caught at their decision: a changed chosen
     entry; a changed unchosen entry with the same length and choice; a
     changed reply byte at the first decision that emits reply bytes (the
     plant must be shown to have fired); a log one record short; a log
     with one record extra;
   - a planted read of a host value, past the lint, that differs between
     the two processes breaks replay;
   - a planted seam error carrying an operating-system message is a
     simulator error, and a clean run's errors are all in the
     vocabulary.

   Before operation code is written on the seams, the same replay is shown
   on a minimal scripted operation with a planted failing schedule, which
   must reproduce from its seed (section 13, Q11).

6. **Hand-off protocol,** each planted through a hook in `ironafs-sim`: a
   grant posted before the task thread reaches its gate; a spurious
   notification of every task at every step (the trace is unchanged); a
   signal raised before the waiter registers (no lost wake); a task
   ending during shutdown; a panic in a task (the run fails with its
   message, the baton is returned, and the run ends without the
   watchdog); a race scenario with the scheduler planted to grant each
   task its whole critical window, which reports the missing order as not
   exercised and fails; a race between a task and a world event (a
   delivery during a task's window), witnessed in both orders from the
   trace with no hand-off between tasks.

7. **Stuck and runaway,** each planted:
   - a worker that ignores its request's deadline and loops on horizon
     waits, with the completion owner working: no failure; the completion
     owner completes the request at its deadline, and the worker's later
     submission is discarded (exactly one completion);
   - the same, with the deadline completion disabled by a test switch:
     the run fails at the request's deadline, naming the request;
   - the deadline completion and the worker's submission in adjacent
     steps, both orders: exactly one completion each time;
   - with the deadline check and the mandatory rule both switched off,
     the worker's success submitted after the deadline, before the
     owner's timer runs: exactly one completion, the timeout error; with a
     clock failure that clears before the submission: the clock error.
     This case tests only the completion claim's arbitration;
   - the same late worker under the normal scheduler, over the race seed
     range, with other tasks and timers enabled at the deadline's
     instant: on every seed the completion's virtual time equals the
     deadline, and the decision log shows the timer run without a draw.
     Mutation: the deadline timer treated as an ordinary entry;
   - a deadline strictly inside one step's cost: the clocks advance to
     the deadline inside that step's charge, the timer runs there, the
     rest is charged after. Mutation: a step's cost charged whole;
   - a suspend across an open request's boot-clock deadline (D 15 ms
     after arrival, the suspend at 5 ms, S 10 s), over the race seed
     range: the decision log shows the suspend event, then the deadline
     timer run without a draw at R, before the suspend's cost is charged,
     with no task step or other event between; one completion, the
     timeout error, at R exactly; the awake clock unchanged; the worker's
     `Rpc` wait on the same deadline ends `DeadlinePassed`; a second
     request whose deadline is after R completes normally. Mutations: the
     suspend's cost charged before the due timers run; the jump split at
     D with the completion inside the sleep; a worker step allowed between
     the jump and the timer;
   - a suspend shorter than the detection threshold less the sampling
     skew, with a cached callback whose expiry falls inside it: the
     request completes at R with the timeout error; no episode is counted
     and nothing is marked suspect by detection; the crossed callback is
     expired at R; a callback whose expiry is after R stays valid. A run
     with S above the threshold counts one episode and marks every
     callback suspect. Mutations: expiry gated on detection; detection
     without the threshold;
   - a successful result ready before the suspend, held by a hook at the
     submission's hand-off, released after R: the timer completes the
     request at R first, and the worker's submission is refused (exactly
     one completion). Mutations: the claim accepts a result ready before
     D; the worker runs between the jump and the timer; the hook not
     reached (the case must then fail, not pass);
   - a `Lock` held at a deadline wait: the run fails, naming the yield
     kind;
   - a contended `Lock`, with the yield check switched off: the observer
     fails the run;
   - a real `std::thread::sleep` behind a test-only feature: an
     unconfirmed lost baton, failed, never a pass;
   - a panic in a `Drop` during teardown: the child aborts, and the
     supervisor reports a simulator error with the seed.

8. **Clock.** On each production venue, a boot-deadline wait across a
   longer suspend ends within one slice plus host delay. Under the
   simulator: a suspend and a clock failure, each with a reply ready in
   its slot and with a cache hit: no callback lifetime is set from a
   `first_sent` of `None`, and after a detected suspend every callback is
   suspect before the reply starts. A keepalive sent before a suspend
   whose reply is delivered after it settles the call but does not raise
   trust. A scripted clock failure with tasks waiting on boot and on
   awake deadlines: every one is enabled at that decision and ends
   `ClockFailed`, and no deadline is built until a valid sample returns
   (the core's sampling function returns `Err(Clock)` meanwhile, given a
   `from` stamp from before the failure too). A deadline built after the
   failure clears, with a `from` stamp taken before it, is already failed
   at its first evaluation, and so is one built from a `now` and a `from`
   both kept from before the failure. A boot-clock regression between a
   request's arrival and its deadline's construction: the deadline is
   failed at its first evaluation, or refused, never counted from the
   arrival on the regressed clock (a regression larger than the time
   since the arrival is refused with `FromAfterNow` and ends
   `ClockFailed`). An
   awake-clock regression larger than the time since the arrival: the
   same refusal. A `from` newer than `now` (a fresh or forged stamp with
   a higher count) is refused with `FromAfterNow`.
   A failure that clears before a second waiter's next sample: that
   waiter still ends `ClockFailed`, and the call owner treats the latched
   deadline as failed. A fault that clears before a parked waiter is
   chosen: the waiter stays on the enabled list until it runs. Mutation:
   an enabled-list rule that reads only whether the clock is failing now.
   A keepalive reply received within E of its send but processed more than
   E after its receipt: trust is not granted; trust granted from a fresh
   receipt lapses at E after `received`, not after the grant.
   **Regression and wall jump.** A scripted regression of the awake
   clock, and separately of the boot clock, with tasks waiting on boot
   and on awake deadlines: every boot-deadline wait is enabled at that
   decision and ends `ClockFailed`, every awake-deadline wait runs on to
   its own outcome, one episode is counted, and the next serve marks
   callbacks suspect before its answer. A wall-time jump past the
   threshold, forward and back, with no other fault: one episode, no
   wait ends, the next serve marks callbacks suspect, and a keepalive
   whose send and receipt straddle the jump, or trust granted before it
   and used after it, gives no trust. A wall-time change under the
   threshold counts nothing. Mutations: a regression that moves no count;
   a regression that also fails awake deadlines; a wall jump not counted
   as an episode; trust compared on doubt counts only.

9. **Callback overload.** A flood of callback calls from one known server
   and from unknown sources, past the accept queue's cap and the rate
   limit, with a real break among those dropped: no queue exceeds its
   cap; no dropped call is acknowledged as accepted; the overload record
   moves before the drop's step ends; no read reply that relies on the
   affected servers' callbacks starts after that step without
   revalidation. A fetch whose ticket predates the drop fails at its
   install. For a binding in its own process, a fence-epoch advance is
   pushed on its stream, and a reply sent before the drop and not yet
   started is discarded. A read admitted, and its ticket taken, in the
   step after the drop and before the core's scheduled fold revalidates.
   Mutations: record the overload after the drop; compare tickets at
   submission only; push no advance to a binding in its own process; take
   a ticket without folding a pending overload.

### 10.6 Gaps

None in this page's requirements. The tests above are written by the
issues that implement each crate: the simulator (issue #79), the
production `Disk` driver (issue #99), `ironafs-rx` (issue #90), and the
conformance bodies after both the simulator and the production drivers
exist.

## 11. Revisit if

From the working decisions this page rests on:

- **C3:** "the M2a simulator cannot replay a failing schedule from its
  seed, or a measured workload needs more concurrent waits than a bounded
  pool of threads can hold."
- **C4:** "a boundary below forces a dependency that point 4 forbids, the
  first implementation of a crate shows it has no content of its own, or
  no log sink outside Linux still needs a site in `ironafs-os` (then the
  safe log transport could live in a crate without `unsafe`)."
- **C39:** "M2 load tests show the fixed set starves a class of work, or
  lock-order checks find an RPC wait under a lock that cannot be
  restructured."
- **O45:** "the simulator cannot reproduce a failure seen against a real
  cell, or keeping its server models in step with observed behaviour
  costs more than the tests return."
- **C9:** "a layer's gate shows a commit point that the reasons below
  cannot express, or that the deadline rule makes an application lose
  data it was told was saved."
- **C26:** "the specification gate of C25 shows a restore discriminator.
  Then clean chunks persist under point 4."
- **C27:** "M2a measurements show the filesystem-as-index layout or the
  eviction policy misses its targets on a supported filesystem."
- **C29:** "assumption A9 is settled the other way (AFS-3, or a later
  profile (N1), offers a conditional store or an operation id), or M3 use
  shows explicit recovery is needed often enough to burden users."
- **C30:** "the M2 tests show a NAT or firewall that maps the callback
  port differently from the calling port, or one `rx` thread limits
  measured throughput."
- **C31:** "a fileserver offers rxgk-protected callbacks to clients (then
  a protected break is authoritative evidence under C24), or the test
  cell shows a callback procedure this table lacks."
- **C32:** "M2 measurements against the test cell and through NAT show
  the intervals below serve stale data or cost too many revalidations."
- **C37:** "the M2 sleep, hibernation, Modern Standby and VM-pause tests
  show a clock below does not behave as its documentation says, or a
  supported target lacks a valid pair."
- **C46:** "a layer's error mapping cannot express a contract error
  without losing what the user needs to act on it."
- **O41:** "crash and power-loss tests fail on a file system the matrix
  supports, or a platform's qualified flush sequence changes."
- **O47:** "the fuzzing engine's platform limits change, or A12 is
  settled differently when the first fuzz target lands."
- **O48:** "the test cell cannot be run in containers on hosted runners,
  or a case here cannot be automated on a supported OS."
- **O50:** "a phase's gate fails, a specification route proves
  unavailable, or an rxgk trigger in point 9 opens a scheduling review."
- **ADR 0005, decision 2,** a row: "an operating system ships a safe
  interface that removes it (C5's trigger), a supported release changes
  the item's header or library (point 5.7), or the decision it serves is
  revisited (C23, C27, C33, C34, C37, O8, C43, C45)."

This page's own triggers:

- a blocking point of the core that is not a yield point of 5.6, point 3;
- the lock rule of 5.3, point 6, cannot be kept by some operation;
- a scenario the gate needs exceeds the live-stack bound;
- a lost-baton stop on a pull-request seed that does not reproduce;
- a filesystem whose qualification (O41, point 4) shows an outcome
  outside 5.5, point 7's table;
- a golden fixture over its size cap;
- idle wake-ups from the one-second slice measured as material on a
  laptop;
- a seam in a pure crate that needs more of a waiting primitive than the
  `Watch` trait gives (5.2, point 1).

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| S1 | DECISION | ADR 0004 (accepted 2026-11-20, with its amendments to 2026-11-21), [part 1](../adr/0004-architecture-1-core.md): C3, C4, C9, C16, C24, C26 to C32, C37 to C39, C46 | all |
| S2 | DECISION | ADR 0004, [part 3](../adr/0004-architecture-3-operations.md): O41, O45, O47, O48, O50 | 1, 5.4, 5.5, 5.6, 8, 11 |
| S3 | DECISION | ADR 0004, [part 4](../adr/0004-architecture-4-negotiation.md): N7, N32 | 1, 5.4, 5.5, 9 |
| S4 | DECISION | [Coding standards](../coding-standards.md), rules 3, 7 and 18 | 1, 5.4, 5.6, 9, 13 |
| S5 | DECISION | Issue #71: the scheduling model and the store path's late hand-off, decided by the owner; issue #79: the M2a disk faults, decided by the owner | 1, 5.4, 5.5, 5.6 |
| S6 | OWN | The `Watch` handle a pure seam takes; the episode and doubt counts; the wait protocol and its latch; the doubt latch; the connection fence; the call owner, its decision guard and the clock bracket; the overload record's ordering; sync on the same handle; plans; the persistence model, its marks and the fresh commit; the hand-off protocol; fairness and mandatory deadline completions; the race witness; the supervisor and the watchdog; strict replay and golden traces | 5.3 to 5.9 |
| S7 | DECISION | [ADR 0005](../adr/0005-ironafs-os.md) (accepted 2026-11-21), decision 2: the closed list of `ironafs-os` sites | 1, 5.2, 11, 13 |

No outside document is cited for a fact this page relies on. Platform
behaviour named here (per-handle writeback error reporting, directory
syncs on macOS, the ext4 residual, condition-variable timeouts across a
suspend, datagram acceptance and the clock, a replacing rename on
Windows, the root-relative no-follow calls, the Rust panic and unwinding
rules) is labelled UNVERIFIED and is settled by O41, point 4's
qualification and the conformance bodies, or by registering the edition
that states it before code relies on it (section 13, Q16). No other implementation's source is used.

### 12.1 Observed behaviour

None yet. 10.5, points 1, 4 and 8, produce the first observations on
real platforms.

## 13. Open questions

- **Q2. Two crates C4 does not list.** `ironafs-sync` (5.2, point 2) and
  `ironafs-disk` (5.2, point 4) need rows in C4: `ironafs-disk` as a
  driver in point 3's table, `ironafs-sync` as shared standard-library
  support that is neither pure nor a driver, with point 4 gaining "no
  shipped crate depends on `ironafs-sim` or `ironafs-test-support`" and
  "`ironafs-net`, `ironafs-disk` and `ironafs-sim` do not depend on one
  another". No pure crate depends on `ironafs-sync` (5.2, point 1), so
  the pure set and `ironafs-rx`'s allowed dependencies are unchanged. The
  same amendment names the crate that holds the call owner's code (5.4,
  point 7). `ironafs-sync` cannot, since it would need `ironafs-rx`'s
  types. The options are a small standard-library crate over
  `ironafs-rx`, `ironafs-time` and `ironafs-sync` that `ironafs-net` and
  `ironafs-sim` both use, or `ironafs-client`, with each loop driving it
  through an interface its driver supplies, which must keep section 3,
  point 2's one blocking primitive. Decision needed before the `rx` loop
  is written. C4 is Working, so this is an amendment by its own
  issue ([ADR README](../adr/README.md), "Amending a working decision"),
  with `xtask`'s crate table updated in the same change, as issue #53
  amends C4's other points. Tracked in issue #71 until that issue is
  filed. Blocks: creating `ironafs-sync`, `ironafs-disk` and the `rx`
  loop. It blocks nothing else on this page.
- **Q3. The clock of a request's deadline.** C39, point 3, says "counted
  from arrival"; C37, point 1, gives the boot clock to server-granted
  deadlines and the awake clock to round-trip and retransmission timers,
  and names neither for a request from the operating system. The
  recommendation is the boot clock, so no request outlives a long sleep;
  5.6, point 7, then completes a request whose deadline a suspend crosses
  at the resume instant, after its deadline, which O45, point 3 ("every
  OS request completes within its deadline"), does not allow as written.
  Settled in the contract's specification (issue #67), with the Working
  amendments of C37, point 1, and O45, point 3, it then needs. Blocks:
  the deadline check across a suspend. Tracked in issue #67.
- **Q4. Streaming and the store path's deadlines.** Whole buffers in M2a,
  for `Rpc` and `Disk`. A store needs more than `call` gives: C9, point 3,
  lets the caller's commit deadline pass while the store continues, and
  C29, point 8, ends it only on a stall. The recommendation is to split
  `call` into `start` and `wait`, so a wait can end without abandoning the
  call, with an explicit `abandon` and a stall limit renewed by
  acknowledged progress, under the call owner of 5.4, point 4; decided
  with the store path in M3. M2a makes no store call, so no M2a code
  depends on it. Tracked in issue #71 until M3's store-path issue is
  filed.
- **Q7. Replay across targets.** Byte-identical replay is claimed per
  target triple. A nightly comparison across targets matches scenario,
  seed, features and profile, compares canonical events without the
  target's metadata, and files its first unexplained difference as a
  defect; it does not gate pull requests. Tracked in issue #79.
- **Q11. The spike.** Before operation code is written on the seams, the
  scheduler and strict replay run on a minimal scripted operation with a
  planted failing schedule, which must reproduce from its seed (10.5,
  point 5), under this page's issue; its code carries into the simulator.
  If it cannot, C3's trigger fires and C3 reopens before the rest is
  written. Tracked in issue #71.
- **Q12. Coding standards rule 18** still describes async code, which C3
  forbids; ADR 0004's Consequences say it is reworded for threads,
  deadlines and cancel tokens. Proposed text, class S:
  "18. **Threads, deadlines and cancellation.** No async code (C3).
  Threads are spawned only by the thin drivers and `ironafsd` (C39, point
  1), never by the core. Operation code waits only through `Clock::wait`,
  on a `Deadline`, with a `Cancel`. `Disk` calls take neither, and a
  request stuck behind one is completed at its deadline by its owner on
  the ingress side. Drivers' own readiness waits and joins follow their
  crate's design. No core lock is held at a yield point of the seams
  specification. Network and IPC I/O has a timeout. Shutdown is a
  `Cancel` given to every loop, then a join, and core code never relies
  on `Drop` for I/O." Blocks: nothing on this page; rule 18 is reworded
  before the first core loop is written. Tracked in issue #71 until its
  own class S issue is filed.
- **Q14. Waking the production `rx` loop** (5.4, point 9). A loop over the
  standard library alone can own both sockets non-blocking and sample
  them on a bounded interval (C30, point 2); a readiness wait over the
  sockets and a loopback wake socket needs one more `ironafs-os` site.
  The `ironafs-net` design states the submission-to-service bound first,
  then measures both against it on an idle loop and under sustained
  receive load. If the readiness wait wins, its design covers a wake send
  that fails, "would block" after a ready report, bounded draining so
  receive traffic cannot starve submissions, interrupted waits with the
  deadline recomputed, and the Windows poll flags and errors. The same
  design lists the send errors that settle `NotSent(Unreachable)` (5.4,
  point 2). Blocks: the production `rx` loop. Tracked in issue #71 until
  `ironafs-net`'s issue is filed.
- **Q15. Golden trace fixtures.** Exact trace bytes per seed (5.6, point
  11), capped at 256 KiB per seed. If the first run shows a seed over the
  cap, that seed falls back to a 64-bit FNV-1a hash of the whole trace
  plus the exact bytes of its first and last events, which detects change
  but is not exact, and the fallback is recorded. Tracked in issue #79.
- **Q16. Platform facts to register.** The UNVERIFIED platform facts of
  section 12 are read at source and registered ([README](README.md),
  "Register of editions") before the driver that relies on each is
  written: per-handle writeback error reporting on Linux, the directory
  sync on macOS, the ext4 residual, condition-variable timeouts across a
  suspend, the root-relative no-follow calls on each platform, a
  replacing rename on Windows, that no interface makes a datagram's
  acceptance conditional on a clock (5.4, point 4), and the Rust panic
  and unwinding rules at the pinned toolchain (a panic while unwinding
  and a stack overflow abort; `cfg(panic = "abort")` is available to
  refuse a build). Blocks: each driver that relies on one. Tracked in
  issues #99 (the disk facts), #57 (the condition variable) and #79 (the
  Rust rules and datagram acceptance).
- **Q17. `ironafs-os` sites for `ironafs-disk`.** ADR 0005's closed list
  has the free-space query on Linux only. `ironafs-disk` also needs, per
  platform where the standard library has no safe route: the stronger
  flush O41, point 4, qualifies (macOS), root-relative no-follow lookups
  and mutations, the sync of a directory through a held handle where the
  standard library cannot open one, and free space on macOS and
  Windows. Each is a row added by ADR 0005's own amendment rule, with its
  proof, before `ironafs-disk` calls it; which calls they are is
  UNVERIFIED until Q16 registers them. Blocks: `ironafs-disk` (issue
  #99). Tracked in issue #99.
- **Q18. Where durable records live.** 5.5, point 6, acknowledges a
  durable record only in the root or in a directory created, with its
  parent synced, in this incarnation. The state directory's layout must
  place each durable record so: the M2a records (the format marker, the
  generation record, the host identity, the audit checkpoint and the last
  good policy) in the root, and M3's journal in the root or in
  per-incarnation directories into which each start's fresh commit
  writes. Decision needed: the layout of each record family, by its
  specification. Blocks: the first durable record's layout. Tracked in
  issue #61 (the generation record) for M2a, and in issue #71 until the
  journal's specification issue is filed.

## 14. Review

The review record is [2026-11-21-m2a-71-seams](../reviews/2026-11-21-m2a-71-seams.md).
