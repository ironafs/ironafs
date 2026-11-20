<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0004, part 1: Core

Status: proposed (2026-11-20). Part of ADR 0004 ([overview](0004-architecture.md)); raised in issue #20.

## Context

The core is everything the three operating-system layers share: the AFS
protocols, the cache, tokens, locks and the write journal. This part decides
how it is built, where it runs, and what it promises the layers. The
overview gives the goals, the anti-goals, the constraints, the named
assumptions (A1 to A12) and the evidence rules. The facts this part rests
on:

1. **What an operating system says about a caller differs by system and by
   operation.**
   - Every FUSE request carries the caller's uid, gid and pid `[DES-022]`.
     The kernel passes no group list `[DRV-065]`.
   - libfuse documents that a write the kernel sends later from its page
     cache carries no valid caller `[DES-023]` `[DRV-066]`.
   - On macOS 27 an FSKit request carries the caller's real and effective
     user and group ids and nothing else `[DRV-025]`. Earlier FSKit
     operations take no caller context; that is inferred from the API,
     Apple does not say so `[DRV-026]`.
   - With FSKit's data-cache handler, every open and close goes through
     that handler `[DRV-M04]`, and its close reports no error `[DRV-046]`.
2. **AFS-3 has limits a client cannot remove.**
   - File locks are whole-file and advisory `[DES-050]`.
   - Server addresses in the volume location database are IPv4 `[DES-058]`.
   - Volume-package error codes are small positive numbers that collide
     with errno values on many platforms, so a caller must tell them apart
     by context `[DES-055]`.
   - The store call takes no condition on the server's current version of
     the file (assumption A9).
3. **The security classes are uneven.**
   - Existing cells' clients use rxkad with Kerberos 5 `[DRV-089]`. The
     community Rx specification does not specify the security classes
     `[DRV-084]`.
   - OpenAFS's 2013 advisory on brute-forceable single-DES service keys
     describes rxkad-k5 and rxkad-kdf as the fixes `[DES-049]`.
   - rxgk rests on two expired drafts `[DRV-085]` `[DRV-086]`. OpenAFS 1.9.0
     uses it only between servers and for local keys `[DES-048]`; client
     tokens are still in progress `[DRV-M25]`. So we know of no released
     OpenAFS fileserver that offers rxgk to clients to test against.
   - An Rx security class cannot authenticate or protect an abort packet
     ([Rx specification draft](https://registrar.central.org/rx-spec.txt),
     "Packet Types").
4. **A cache shared by several local users can be poisoned.** The rxgk
   integration draft says a malicious user may be able to poison the cache
   for other users, says a cache manager that caches for several users
   should combine each user's token with a token of its own, and says a
   cache manager must not give users access to combined keys it uses
   itself
   ([draft -08](https://datatracker.ietf.org/doc/html/draft-wilkinson-afs3-rxgk-afs-08),
   sections 7, 9 and 14.3). A client may fall back from rxgk for one
   fileserver only on that draft's own signal, an empty token in a
   successful combine reply `[DES-062]`, and the combine call must run over a
   secured connection `[DES-061]`.
5. **Clocks differ in whether they count suspend.** Linux `CLOCK_BOOTTIME`
   counts it and `CLOCK_MONOTONIC` does not `[DES-012]`. On macOS
   `CLOCK_MONOTONIC_RAW` counts sleep and `CLOCK_UPTIME_RAW` does not
   `[DES-013]` `[DES-014]`. On Windows `GetTickCount64` counts it, by
   implication of Microsoft's wording `[DES-016]`, and
   `QueryUnbiasedInterruptTimePrecise` does not `[DES-015]`. Rust's
   `Instant` leaves the question open and differs by platform
   `[DES-017]`.
6. **Cell discovery through DNS is specified,** as SRV records with AFSDB
   as the older form `[DES-008]` `[DES-010]`. How a program should reach the
   host's resolver differs by system `[DES-001]` `[DES-002]` `[DES-004]`
   `[DES-006]`.

What forces these decisions now: every layer in part 2 is written against
the contract decided here, and the formats in part 4 record what this part
keeps.

## Decision

The rule throughout: **semantics in the core, presentation in the layer**
(C2).

**Shape of the core**

### C1. One system daemon per machine holds the core

**Working.** Revisit if: the M4 gate shows an FSKit extension cannot reach a
system daemon over local IPC, or an operating system ships a supported way
to keep a cache and tokens alive without a daemon.

1. One daemon, `ironafsd`, runs per machine on every operating system. It
   holds the core: protocol clients, cache, tokens, locks and journal.
2. A layer is in the daemon's process (FUSE on Linux) or in its own process
   that talks to the daemon over the adapter endpoint (C45): the FSKit
   extension, the Windows SMB front end, and the Cloud Files processes if
   the replica is built.
3. How the daemon is hosted on each operating system, and under which
   account, is in L7, L14 and L23.

Why: an extension the operating system starts and stops would take the
cache and every user's tokens with it. One daemon also gives one place
where authorisation is decided (C2).

### C2. Semantics in the core, presentation in the layer

**Working.** Revisit if: an operating-system interface is found that cannot
be served unless its layer decides one of the things listed in point 1.

1. The core decides authorisation, coherency, sharing, locking and when a
   write is committed.
2. A layer translates requests and results, and applies invalidations to
   its operating system's caches. It decides none of point 1.
3. Presentation is the layer's: names, attributes, error numbers, and
   cache hints such as FUSE's open flags and timeouts, computed from the
   rights and volume type the core reports.

Why: goal 3 of the overview. A rule decided once in the core is tested
once; a rule decided in each layer is three rules.

### C3. Protocol code without I/O, synchronous operation code, no async runtime

**Working.** Revisit if: the M2a simulator cannot replay a failing schedule
from its seed, or a measured workload needs more concurrent waits than a
bounded pool of threads can hold.

1. Rx, XDR, the RPC codecs and the security classes are pure state
   machines: bytes and time in, actions out. They open no socket and read
   no clock.
2. The subtle parts of the cache manager are pure types: coherency states,
   fencing, rights, the chunk map, the lock table. They get exhaustive
   model tests.
3. The operation code that uses them is written in ordinary blocking style
   against four injected interfaces: `Rpc`, `Clock`, `Rng` and `Disk`.
4. Small blocking drivers on standard-library threads perform the real
   I/O (C39).
5. No async runtime is used, neither a third-party one nor one of our own.
6. One deterministic simulator drives points 1 to 3 through the injected
   interfaces. It is a deliverable of M2a (O45).

Why: the simulator is what goal 3 buys, and it needs every source of
nondeterminism behind a seam (coding standards rule 7). A third-party
runtime is a large dependency outside rule 21 and ties every crate to one
executor. Writing the whole cache manager as one state machine would turn
every filesystem operation into a hand-written continuation.

### C4. Crates and their dependency rules

**Working.** Revisit if: a boundary below forces a dependency that point 4
forbids, or the first implementation of a crate shows it has no content of
its own.

1. **Pure crates** have no I/O and no `unsafe`, and are fuzzed where they
   decode:

   | Crate | Holds |
   |---|---|
   | `ironafs-xdr` | the XDR codec, with size caps |
   | `ironafs-rx` | the Rx state machine, the security-class interface, the null class |
   | `ironafs-rxkad`, `ironafs-rxgk` | the security classes |
   | `ironafs-proto` | volume location, file service, callback and protection types and their generated codecs; the AFS directory format; wire time conversion (C38) |
   | `ironafs-vfs` | the contract (C7), including the name codec (L24, L25, L29) |
   | `ironafs-cache` | pure state: coherency, fencing, rights, chunk map, location entries, lock table, node table, eviction, journal records |
   | `ironafs-auth` | token types and the token store; `aklog` logic behind a Kerberos interface |
   | `ironafs-ipc` | the IPC framework and both roles (C44, C45) |
   | `ironafs-config` | the configuration parser, schema, layering and validation (O17, O18) |
   | `ironafs-events` | the event catalogue, redaction, rate limiting, metrics and health (O1 to O8, O11) |
   | `ironafs-crypto` | the cryptography interface (C5) |

2. **One composition crate,** `ironafs-client`, is the core: operation
   code over the cache types, the RPC clients, the store queue and
   journal, the callback service, location and failover, the lock service
   and the invalidation set. It may use threads. It makes no socket or
   operating-system call.
3. **Drivers and bindings** are thin:

   | Crate | Holds |
   |---|---|
   | `ironafs-net` | the UDP driver and the `rx` thread |
   | `ironafs-fuse` | the FUSE codec (pure) and the `/dev/fuse` driver |
   | `ironafs-smb` | the SMB 3.1.1 front end, a separate process (L18, L19) |
   | `ironafs-cloudfiles` | the Windows replica, later and optional (L27) |
   | `ironafs-gss` | Kerberos and GSS through each system's own library |
   | `ironafs-os` | small operating-system bindings (C5) |
   | `ironafsd`, `ironafs-cli` (the `afs` binary), `xtask` | the binaries and the repository's automation, including the generators for `ironafs-proto`, `ironafs-ipc` and `ironafs-events` |

4. **Rules the dependency graph keeps:**
   - no pure crate depends on a driver or on `ironafs-client`;
   - no layer crate depends on `ironafs-client` or `ironafs-cache`. A
     layer may depend on `ironafs-vfs`, `ironafs-events`, `ironafs-ipc`,
     and its own operating-system bindings and cryptography;
   - `ironafs-rx` knows nothing of AFS.
5. The macOS extension is written in Swift, uses codecs generated from the
   IPC definition (C44) and contains no Rust.
6. Where the first-party DNS client of C33 lives is settled with its
   specification; we propose a pure codec beside `ironafs-proto` and its
   socket in `ironafs-net`.

Why: point 4 makes "a layer cannot bypass the core" a property the build
checks, not a convention. The layout follows coding standards rule 20.
Keeping automation in `xtask` is a somewhat common pattern that needs only
Cargo and rustc `[LAY-045]`.

### C5. The cryptography boundary and the operating-system bindings

**Working.** Revisit if: a mainstream crate offers rxkad's cipher, a FIPS
mode is scheduled (O31), or an operating system ships a safe interface
that removes a site from the `ironafs-os` list.

1. `ironafs-crypto` is an interface in the pure layer. Protocol crates
   call only it. Implementations are supplied when the client is composed:
   a mainstream crate chosen under the dependency rule, IronAFS's own
   implementation of rxkad's cipher, or later an operating system's
   validated module.
2. rxkad's cipher (fcrypt) is written by IronAFS from the specification
   that C6 requires. It is never treated as approved cryptography.
3. The Kerberos encryption-type operations rxgk needs run in the client,
   so they sit behind `ironafs-crypto`. The GSS exchange and its
   pseudo-random function `[DES-060]` stay in the operating system's GSS
   library.
4. Kerberos and GSS come from each system's own library (`ironafs-gss`).
5. `ironafs-os` holds the few calls that have no safe route: the clock
   pairs of C37, peer credentials (C43), the service controls, the log
   calls of O8 and the dump controls of O26. It needs `unsafe`, so it gets its own ADR with a
   closed list of named sites and safe interfaces on top, before any of it
   is written. Since Rust 1.82 a foreign declaration can be marked safe
   to call, with the proof left to whoever writes the declaration
   `[DES-018]`; that ADR states the proof for each.

Why: the constraint on dependencies allows crates for mainstream
cryptography only, and rxkad's cipher is not mainstream. An interface
keeps the choice of implementation out of the protocol code. `unsafe` is
forbidden workspace-wide until an ADR names the sites (`AGENTS.md`).

### C6. The protocol code is written from specifications

**Firm.**

1. Each protocol component is written from a behaviour specification in
   `docs/specs/` that names its sources, under
   [ADR 0001](0001-licence-and-dont-copy.md). No specification, no code.
2. The sources are: the community Rx specification draft `[DRV-084]`; XDR
   ([RFC 4506](https://www.rfc-editor.org/rfc/rfc4506)); the rxgk drafts
   `[DRV-085]` `[DRV-086]`; published AFS-3 documentation; RFC 5864 for
   discovery `[DES-010]`; and packet captures of real servers, reduced to
   fixtures as ADR 0001 requires.
3. **Specification gates.** These are written and reviewed before the code
   that depends on them:
   - before the cache: what evidence identifies a volume's incarnation
     (C25), the directory format, the callback procedures, and the table
     of wire time fields (C38);
   - before rxkad: a specification route that covers rxkad, rxkad-k5 and
     rxkad-kdf, and the rule that turns an access mask and mode bits into
     an effective right (C22). Where behaviour can be learned only from
     another implementation's source, ADR 0001's separate
     specification-from-source session is the only route;
   - before rxgk: the exact wire profile and its open points (N25).
4. A fact about the AFS protocols that this ADR could not tie to a public
   source is an input to a specification, never something to code from.
   Each decision below says where it leans on one.

Why: this is ADR 0001's rule applied to the core. No public specification
of rxkad has been found
([driverless-clients note](../research/2026-11-18-driverless-afs-clients.md),
sections 8 and 9), so the gate is where that risk is met.

**The contract**

### C7. The contract crate and its three kinds of operation

**Working.** Revisit if: building a layer shows an operating-system request
that fits none of the three kinds, or needs a caller the kind does not
carry.

1. The core's interface and its types live in `ironafs-vfs`: callers,
   handles, node ids, attributes, ACLs, errors, invalidations and the name
   codec.
2. **Caller operations** carry a caller capability (C8). They are: lookup,
   get attributes, access, rights, open (with disposition and share
   access, so an SMB create is one atomic call), open directory, create,
   make directory, unlink, remove directory, rename, link, symlink, read
   link, set attributes, filesystem statistics, lock, get and set ACL,
   extended-attribute reads, and `control` for the `fs` family of
   commands.
3. **Handle operations** carry a handle capability returned by open, and
   act with the handle's authority (C13): read, write, read directory,
   commit, lock on an open file, and subscription to change notification.
   Get and set information, disposition and rename also have handle forms.
   They use the access granted at open. A rename through a handle needs
   fresh authority on the target directory.
4. **Binding operations** carry no caller and change no data: forget,
   release, release directory, reclaim, cancel, volume sync, pin and
   unpin. They can only release or flush what the binding already holds.
5. A handle that comes from the operating system (a descriptor passed to
   an `fs` command) is accepted only after the core has checked that it
   belongs to an IronAFS mount or share and to the volume's current
   incarnation (C25).
6. Share access and byte-range locks are checked, reserved and enforced
   in the core's lock service. A layer only forwards them. The lock rules
   are in L33.

Why: FUSE's delayed writes arrive without a usable caller `[DES-023]`, and
the FSKit handler calls that take the caller's context in the macOS 27.2
SDK do not include read or write `[MAC-014]`. Three kinds mean no operation pretends to have a
caller it lacks, and an operation without a caller can do nothing but
release.

### C8. Caller capabilities come from the core, from what the OS attests

**Firm.**

1. A caller capability is an opaque value the core mints for one binding
   and one credential context (C40), when the layer presents the operating
   system's own statement of the caller: the uid in a FUSE request header
   `[DES-022]`, FSKit's context `[DRV-025]`, or the authenticated SMB session
   (L17, L18).
2. A layer cannot construct a caller capability or a handle capability,
   and cannot use one minted for another binding.
3. Local uid 0 has no AFS powers. A local administrator role exists only
   for control operations (O13, O16).

Why: a layer that could name a caller could act for anyone. This keeps a
bug in a layer, or a compromised front-end process, from becoming another
user's authority (C45).

### C9. One commit operation

**Working.** Revisit if: a layer's gate shows a commit point that the
reasons below cannot express, or that the deadline rule makes an
application lose data it was told was saved.

1. A write reaches the server through one operation, `commit(handle,
   reason, durability)`. The reasons are flush, fsync, close, interval,
   write-through and sync. A commit covers the handle's dirty data up to a
   stated boundary and returns the store's error.
2. Each layer maps its own events to `commit` (L6, L12, L19).
3. A caller's commit has an absolute deadline. If it passes while the
   store is still making progress, the caller gets a "not yet stored"
   error, the store continues in the background as a journal record
   (C29), and `afs pending` shows its outcome.
4. `release` never fails.

Why: FUSE does not return an error from a release to `close()`, and may
flush many times or not at all `[DES-029]`. FSKit's cached close takes no
error `[DRV-046]`. A store error must therefore surface at an operation the
application waits on.

### C10. Removing and replacing open files

**Working.** Revisit if: the specification for the file service shows an
AFS-3 operation that renames only when the destination is absent, or the
M3 tests show the reserved name cannot be made safe against a concurrent
replacement.

1. **Unlink of an open file, POSIX layers.** The intent is journaled and
   the file is renamed to a reserved, collision-safe dot-name in the same
   directory, under the unlinker's authority. While IronAFS tracks the
   entry it refuses to open or reuse it by name.
2. After the last local close, the entry is removed under the same
   credential context, and only when its identity and liveness are
   verified and the removal is safe against a concurrent replacement of
   the name. Otherwise it stays, and `afs pending` reports it. An
   interrupted sequence is reconciled from the journal; a reserved name
   whose object cannot be confirmed is quarantined, never removed.
3. The unlink is refused when the caller may not rename.
4. **Windows.** SMB's delete-pending and delete-on-close states defer the
   server's removal to the last close, with no rename. They are modelled
   apart from POSIX unlink (L19).
5. **Rename over an open destination** is one AFS rename. Local handles
   still open on the replaced file then fail with `Stale`. Bytes of the
   replaced file not yet stored are kept as recovery data (C29).
6. **Rename with "do not replace"** is refused with the operating
   system's "invalid argument" error.
7. Opens made on other machines are not visible to this client.

Why: we know of no AFS-3 call by which a client tells a fileserver which
files it holds open, so "gone from the directory, alive until closed" has
to be emulated; that is a specification item too. That
AFS-3's rename replaces a destination in one operation, and that it has no
rename that refuses to replace, are specification items (C6), not cited
facts. Emulating "do not replace" with a check and then a rename would
sometimes replace a file, which is the silent wrong answer the overview
rules out.

### C11. Node identity

**Working.** Revisit if: the M2a kernel tests show a kernel mishandles two
node ids for one file under different anchors, or a layer needs node ids
that outlive the daemon.

1. A node id is 64 bits. It maps to a key of cell, volume, vnode and
   uniquifier, plus an **anchor**: how the volume was entered (the dynamic
   root, a cell root, or a mount point).
2. A node id is never reused within one daemon instance, and is valid for
   that instance only. A daemon restart ends the FUSE mount and the FSKit
   items (L7).
3. The anchor keeps every directory single-parented when one volume is
   mounted in two places. A reverse index from file id to node ids sends
   each callback break and each local change to every alias.
4. The node table is bounded. A layer pins the nodes it has handed to its
   operating system and unpins them when the system forgets them. Status
   and data for a pinned node can still be evicted.
5. A file's read-write, read-only and backup volumes have different ids
   ([OpenAFS administration guide, "About Volumes"](https://docs.openafs.org/AdminGuide/HDRWQ177.html)),
   so their paths are different nodes.
6. What Windows is shown as a file's identity is the stable key, not the
   node id. SMB's per-open ids are generated per open by the front end
   (L19).
7. Identifier widths are in N38.

Why: a kernel assumes a directory has one parent, and AFS lets a volume be
mounted anywhere. The alias handling is reasoned, not tested; the M2a tests
on each supported kernel settle it.

### C12. The dynamic root

**Working.** Revisit if: a supported desktop's indexer or file manager
still contacts cells from a listing of `/afs`, measured in M2a.

1. `/afs` is a dynamic root. It lists configured and discovered cells
   without contacting them.
2. The attributes of a mount point that has not been resolved are
   synthesised.
3. A lookup of a name that is not a cell is answered from a negative
   cache, so an indexer or a mistyped path does not cause a DNS query each
   time.

Why: points 1 and 2 are the dynamic-root and fake-stat behaviours
OpenAFS's client documents `[DES-054]`, which users of `/afs` already rely
on. Point 3 is our own addition.

**Authority**

### C13. Handles carry authority

**Firm.**

1. A handle is bound to its credential context (C40) and that context's
   credential generation, the node, the open mode, the binding, the rights
   at open, and the volume's incarnation (C25).
2. Where the front end can identify the open, each read or write checks
   that the handle is live and takes the intersection of the rights at
   open with the current effective rights. A right that was lost is
   refused; rights that remain keep working.
3. Where the front end cannot tell opens apart (FSKit), each operation
   checks only liveness, and any reduction of rights revokes the handle.
   It must be reopened.
4. A descriptor passed to another process keeps the handle's authority and
   nothing more: that handle's access mode, live grant and generation.
5. A new file gets only its effective AFS rights, including the creator's
   rights where the server grants them.

Why: reads and writes can arrive without a caller (context, point 1), so
the open is the last moment the caller is known. Binding authority to the
handle keeps those operations safe, and matches what a POSIX descriptor
already means.

### C14. Suspension, resumption and revocation

**Working.** Revisit if: the M2b tests show that renewing a token
interrupts I/O in progress, or the ten-minute bound in point 5 proves too
long or too costly against the test cell.

1. **Suspended** means the handle's operations wait, within their
   deadlines. A handle is suspended when what its authority rests on
   becomes uncertain:
   - a token for the same context, cell and AFS identity replaces the old
     one (what `aklog`, renewal and login services produce); or
   - a callback break makes the file's rights, or its directory's ACL,
     stale.
2. The core then fetches the file's rights under the current credential.
   If they still cover what the handle needs, it resumes with the same id
   and stores in flight authenticate again. If not, C13 applies.
3. **Revoked** means every later operation fails. A handle is revoked when
   the token is deleted, expires without a replacement, or is replaced by
   a different AFS identity; when the volume's incarnation changes; or
   when its binding dies. Expiry revokes handles on public data too; a
   fresh open may then get a public grant of its own.
4. After write access is revoked, writes stop. Data not yet committed goes
   to a recovery record private to the original context and is never
   replayed automatically (C28). The failure is reported by the next
   write, flush or close that can carry an error, and in `afs status`.
5. The core's own cached rights decisions are at most ten minutes old.
   This cannot bound how long a group change takes to spread between
   servers. When fresh authorisation is needed and cannot be had, the
   operation fails.

Why: renewal must neither interrupt I/O nor extend authority, so the
handle waits while rights are established again instead of being cut or
trusted blindly.

### C15. What revocation guarantees, by access path

**Firm.**

1. **Ordinary reads and writes** are refused from the next call.
2. **Mapped pages.** On revocation the layer asks the kernel to drop the
   file's resident pages, so the next fault reaches the daemon and fails.
   Pages already resident in a live mapping may stay readable to that
   mapping until it is unmapped, like data the process already copied.
   This is documented, not hidden.
3. **A second context** that maps the same file is served only through
   its own authorised open. One context's fault is never served from pages
   another context loaded.
4. **Descriptors held by other processes** are revoked with the handle.
5. **Gate, per supported kernel and per operating system.** Resident
   pages, a new fault, two contexts mapping one file, and a failed purge
   while the first mapping is still live are each tested. Where a kernel
   would serve one context's fault from another's pages without asking the
   daemon, mapping is refused on that kernel, and programs cannot be run
   from `/afs` there.
6. If a purge fails at run time for a node that may be mapped, the core
   cannot show the pages are gone. On Linux it aborts the FUSE connection
   and remounts; on macOS the item is revoked. That stops every future
   request and fault. It does not remove pages already resident in live
   mappings.

Why: the first anti-goal of the overview. Stating the bound exactly is
what lets a site decide whether it is acceptable. The kernel mechanisms
are FUSE's inode invalidation `[DRV-070]`, the control filesystem's abort
`[DES-036]` and FSKit's revoke action `[DRV-030]`; their use is in L4, L7
and L13.

### C16. Fencing: record first, never serve a fenced reply, check at submission

**Firm.**

1. **Record first, notify after.** Break counters, staleness and
   credential generations change in the core before any event is queued.
   Every later operation sees the change, whether or not a layer has
   applied it.
2. **A reply that fails its fence is never served.** A fetch records the
   break counters of its object, volume and server when it is sent. If any
   changed while it was in flight, the reply is discarded and the fetch
   retried a bounded number of times, then fails with `Stale`. It is never
   handed to the request that started it.
3. **Every delivery rechecks.** A grant is bound to the file, the
   operation, the binding, the credential generation and the rights
   generation. The last check is at **submission**: the binding checks the
   reply's context, generation, grant and node state in the same step that
   writes it to the kernel or to the SMB connection. A revocation that
   lands earlier stops the reply. A reply already submitted counts as
   delivered.
4. **Local changes fence too.** A store, create, rename or remove made
   through this client updates the core from the server's reply and
   publishes data, attribute and entry invalidations to every alias of the
   node, so other users' kernel caches see the change.
5. **While a purge is pending** in a binding, the node serves nothing in
   that binding that needs the purged state. There is no exception for the
   user who was last served.

Why: the "silent wrong answer" anti-goal. A check made before a wait says
nothing about the moment the bytes leave, so the check that counts is the
one in the same step as the write.

### C17. The invalidation stream

**Working.** Revisit if: measurements in M2a show coalescing per node loses
an event a layer needs, or a layer's operating system offers a purge that
cannot fail.

1. Events are **coalesced per node**, not queued one by one. Memory is
   bounded by the node table, never by the rate of breaks. Overflow
   becomes one overflow event that gates every node.
2. Events are **acknowledged per binding.** A layer acknowledges an event
   only after it has applied it and has drained, or fenced out, every
   reply it issued earlier that could fill the cache again.
3. Coherency events go to every binding that holds the node. Rights
   events name a credential context.
4. Each in-process layer has one notifier thread, which writes kernel
   notifications and holds no core lock.
5. **When a purge cannot be completed:**
   - FUSE: the node answers `Stale` and its handles are revoked;
   - FSKit: the item is revoked;
   - SMB: the front end completes pending change notifications with
     "rescan", and closes the session if it cannot;
   - the Windows replica, if built: the placeholder stays marked stale and
     the event stays unacknowledged until dehydration succeeds.

Why: a callback break must never wait on a layer, and a slow layer must
not grow the daemon's memory. libfuse documents a deadlock when
entry-invalidation and delete notifications are sent from a related
operation's path or under a lock it needs `[DES-032]`; with writeback
caching on, inode invalidation first writes back dirty pages and waits for
them `[DRV-069]`, and L4 keeps writeback caching off. FSKit's cache-state
call must likewise be made without the module's own locks held, and can
call back into the module `[DRV-032]`. Sending every notification from the notifier thread is our
own rule on top.

**The cache**

### C18. Cache state is private to each credential context

**Firm.**

1. Every private cache entry carries the id and credential generation of
   the credential context (C40) that fetched it, and is read only under
   them. Private entries are: data chunks, status, rights, directory
   snapshots, lookups and negative lookups, the volume and location data
   used to reach a file, fetches in flight, dirty data and journal
   records.
2. Nothing private crosses contexts. Two accounts, two AFS identities of
   one account, and (where the operating system can tell them apart, C41)
   two sessions of one account are separate contexts.
3. The one exception is **deliberate delegation of an open handle**
   (C13, point 4). Handle operations run in the opener's context,
   including enumeration through a passed directory handle. Every
   operation by path runs in the context the daemon established for that
   request, never one inferred from the node.
4. Where a layer cannot tell two contexts apart on a caller operation, it
   serves each context its own item or refuses the ambiguous operation. It
   never serves one context's item to another.
5. **Dirty data** is visible only inside the writer's context. Other
   contexts, the same user's other sessions included, see the server's
   version until the store is accepted, as two machines would.
6. Nothing ever moves from a private namespace to the shared pool (C21).
7. **Lifetime.** Renewal keeps the generation only when the daemon has
   its own record that the AFS identity is unchanged, never on the token's
   opaque contents. Loss of the credential, the end of the session,
   removal of the account, or a change of policy retires the generation:
   access stops at once, late fetches are fenced off, and the files are
   reclaimed afterwards. A recycled uid or SID is a new account instance
   and inherits nothing. After a daemon restart no private state is
   attached to a context without positive evidence that the account
   instance, the session, its credentials and the policy continue.
8. **The same boundary holds above the daemon.** Whatever a front end or
   its operating system reuses stops being reused when the grant behind it
   ends. Where a front end cannot enforce that, it does not reuse. The
   per-layer rules are in L5, L11, L19 and L28.

Why: a cache shared across users answers "may this reader read?" but not
"who vouches for these bytes?". Under rxkad the user holds the session key
of the connection that fetched the data, because the user's own `aklog`
built the token (C42). So a user could put bytes in front of another user
that the server never sent. The rxgk integration draft names this attack
(context, point 4). Its remedy needs rxgk, which OpenAFS 1.9 uses only
between servers and for local keys `[DES-048]` `[DRV-M25]`.

### C19. Nothing unauthenticated feeds an authenticated path

**Firm.**

1. Bytes, status, lookups, routing and grants obtained without
   authentication (Rx's null class) are never used to answer a request
   made under a credential, and never enter the cache of a context that
   has one.
2. Absence is cached only from an authorised directory result that shows
   it, never from an abort.
3. Capability observations and fallbacks obey the same rule (N19, N20).

Why: an on-path attacker can answer an unauthenticated fetch, and no
security class protects an abort packet (context, point 3). Mixing either
into authenticated state would let the weakest path decide what an
authenticated user sees.

### C20. Unauthenticated data is not cached at all

**Working.** Revisit if: measurements of anonymous reads of public cells
show the refetch cost matters to users, and a design is shown that keeps
C19.

1. Data from an unauthenticated call lives only in that request's buffers:
   no cache entry, no shared fetch, no reuse.
2. Anonymous reads therefore fetch every time.

Why: it is the simplest rule that makes C19 true. The cost falls on
anonymous use of public cells only.

### C21. The shared pool admits only data no local user could have forged

**Firm.**

The shared pool is planned for M6, off by default, and enabled by an
administrator for named cells and volumes. When it exists:

1. An entry is admitted only if its bytes, the status reply that
   authorised them, and every directory and location reply used to reach
   it were fetched over rxgk at its auth or crypt level, under a token
   that combines the user's token with a token private to the daemon
   (context, point 4).
2. **Key custody.** No local user can determine the daemon's own key, the
   combined keys derived from it, or the callback keys. None is exported
   through a token interface, IPC or a front end.
3. Every input that selected the cell, volume, server or file, including
   symbolic links, mount points and each cross-cell hop, meets the same
   rule. A chain that cannot stays private.
4. **No rxkad fetch, no unauthenticated fetch and no fetch under a user's
   rxgk token alone ever qualifies.**
5. A missing, empty-identity or printed daemon token, or a retry at a
   weaker level, fails closed to private. A fallback for one fileserver is
   allowed only on the protocol's own signal `[DES-062]`, never on an RPC
   error, and never below the configured floor (N2, N3).
6. A reader opens a shared entry only with a shared-pool grant, minted
   from the reader's own protected status reply (C22). Only status fields
   that are the same for every caller are shared. Each caller's rights
   stay private.
7. Entries enter the pool only complete and verified. The pool is never
   filled from a writer's dirty data.
8. A fetch meant for the pool that falls back is a separate, private
   fetch in the requester's context.

Why: the test is key custody, not transport integrity: data may be shared
only if no local user held a key that could have produced it. The draft
says a cache manager must not give users the combined keys it uses
(context, point 4). Until a
fileserver offers rxgk to clients `[DES-048]`, the pool is empty and each
active context caches its own copy. That cost is accepted (Consequences).

### C22. Grants: per file, from the reader's own status reply

**Firm.**

1. Cached data is read only with a **grant**. A read grant is minted only
   from a status reply for that file, fetched in the reader's own context
   and generation, carrying the server's access mask for that caller. Each
   item of a bulk status reply counts on its own.
2. The decision that a mask and mode bits allow reading follows the ACL
   specification (C6). Where it cannot be made reliably, the reader's own
   data fetch decides.
3. The client never infers a file's rights from its directory.
4. A grant binds the context and generation, the file id, the volume's
   incarnation, the data version, the effective rights and an expiry. A
   private grant opens only its own namespace; a shared-pool grant opens
   only pool entries.
5. A grant is checked before the store is looked up, before joining a
   fetch, after any wait, and immediately before delivery (C16). A reader
   without a valid grant learns nothing, not even whether the data is
   present.
6. A lookup hit (positive or negative), a listing, a resumed enumeration
   and a step through a cached mount point each need the reader's own
   current grant on every directory consulted.
7. A directory snapshot belongs to the handle that opened it. Its cookies
   bind the context, generation, handle, directory incarnation and
   version.
8. **A shared fetch shares data, never authority,** and only inside one
   context and generation. Identical requests already authorised there may
   share one RPC. A reader in another context never joins a fetch.

Why: the first goal of the overview. A cached answer about one user says
nothing about another, and the server is the only authority on rights.

### C23. How long a grant lasts

**Working.** Revisit if: the M2 tests show the network-generation rule
discards replies on ordinary roaming often enough to matter, or the ACL
specification shows a revocation source this list lacks.

1. A grant ends at the earliest of: the credential's expiry, the end of
   the callback's validity, and a local authorisation timeout (C14,
   point 5).
2. A grant also binds the **network generation**: the client's addresses
   and routes toward the server. It is captured when the authorising
   request is sent and must still match when the grant is minted and at
   delivery. A change discards the reply, authorises again and fences
   outstanding grants.
3. A grant is revoked on a callback break, on an ACL or mode change seen
   for the file or its directory, on a change of identity or group
   membership seen through the client, and on a policy change.
4. A change of address the client cannot see, such as a NAT in between,
   is a documented residual. The local timeout bounds it.

Why: AFS rights can depend on the client's address, through machine
entries in protection groups. That is a specification item (C6), not a
cited fact; binding the network generation is the cautious reading.

### C24. Weak evidence only lowers trust

**Firm.**

1. **Authoritative evidence** is an observation protected by a key no
   local user can determine, bound to the file or volume and to the event
   it establishes. In practice that is rxgk with the daemon's combined
   token (C21).
2. Anything weaker, including a reply protected only by a user's key and
   everything the callback service receives before rxgk (C31), may
   **suspend** callback promises, revoke grants and mark entries suspect.
   A suspect entry is not delivered until its reader has revalidated it
   under the reader's own credentials.
3. Weak evidence never evicts another context's data or the pool's,
   never retires their handles or journal records, and never establishes
   that two contexts see the same volume incarnation.
4. Acting on another context's entries, on the pool, or on shared
   incarnation state needs authoritative evidence.
5. Suspect bytes and journal records are kept, not destroyed.

Why: unauthenticated input must not be a lever. If a forged break could
destroy state, any host on the path could empty every user's cache or
discard pending writes. If it could raise trust, it could make stale data
look fresh.

### C25. Volume incarnation: when in doubt, fetch again

**Working.** Revisit if: the specification gate (C6) shows no reply field
tells a restored volume from the original, or shows one that makes point 3
unnecessarily broad.

1. We assume a restored or recreated volume can repeat a data version
   with different bytes. So the client keeps, per cell, server and volume, the
   evidence that the volume is still the same one, with where each
   observation came from.
2. Cached chunks are reused only when that evidence matches.
3. When it is ambiguous (after the volume is seen offline, a server
   reset, a move, a release, or a restart of the client), data is fetched
   again. The observing context starts a fresh local epoch for the file or
   volume, so no old key in that scope can be used, and fetches in flight
   are fenced.
4. A data version lower than the cached one is ambiguity about that file.
   The observing context drops its own chunks for it and stops using the
   file's shared entries. Fencing the file for others needs authoritative
   evidence (C24).
5. An incarnation change invalidates the volume's handles, directory
   snapshots, Windows identities and pending journal records.
6. Read-only sites marked as an old release, or not yet released, are
   excluded before replicas are ranked.

Why: data version alone identifies content only within one incarnation of
a volume. Which reply fields carry the evidence, and whether any of them
reveals a restore made while the client was down, is a specification gate
before the cache is written.

### C26. Coherency and what survives a restart

**Working.** Revisit if: the specification gate of C25 shows a restore
discriminator. Then clean chunks persist under point 4.

1. Each object is in one of four states: unknown, fetching, valid under a
   callback, or unverified. A fetch is installed as valid only if no break
   counter changed while it ran (C16).
2. Callback expiry is counted from when the request was sent, on the boot
   clock (C37).
3. Callbacks never persist across a restart.
4. Clean chunks are not reused across a restart, and are not kept for
   that purpose, until the specification shows how a restore made while
   the client was down can be detected. Once it does, chunks persist with
   their full key, generation and provenance; an entry is reused only if
   all still match, and every object starts as "known version,
   unverified" and is revalidated on first use.
5. The client's host UUID persists.

Why: a cache that survives a restart is only safe if a volume restored in
the meantime can be told apart. Until that is shown, a cold start is the
honest answer. The host UUID is kept so that a fileserver can recognise
the client after a restart; whether and how it does so is a specification
item.

### C27. The store on disk, eviction and budgets

**Working.** Revisit if: M2a measurements show the filesystem-as-index
layout or the eviction policy misses its targets on a supported
filesystem.

1. **The data store** holds chunks keyed by context and generation (or the
   pool), cell, volume and its incarnation, the local file epoch, vnode,
   uniquifier, data version and chunk index. A new data version makes old
   chunks unreachable without touching them.
2. **No embedded database.** The filesystem is the index. A clean chunk is
   written to a temporary name and renamed, with a header and a checksum
   and no sync. A bad chunk is a miss, never an error.
3. **Eviction** is byte-weighted S3-FIFO
   ([Yang et al., SOSP 2023](https://dl.acm.org/doi/10.1145/3600006.3613147))
   inside each namespace and the pool. Dirty and in-use chunks are pinned.
   A free-space floor keeps the cache from filling the disk.
4. **Budgets.** Every physical byte is counted once and charged to a
   local account or to the pool. Each account has a floor and a soft cap
   on resident bytes, and hard caps on pinned bytes, metadata, dirty and
   journal bytes, fetches in flight and admission rate. Each is reserved
   before the resource is allocated. Back-pressure falls on the account
   that uses the resource. Idle contexts are evicted first.
5. Residency figures per namespace are for administrators only.
   Documented residuals: latency and admission failures show that someone
   is using the cache, and a reader authorised for a pool file can tell
   whether a range is warm.
6. **On Windows** with the SMB surface, the store, rights and journal are
   the same as on Linux. With the replica, the NTFS sync roots are the
   data cache and the core keeps only status and directory data.
7. **The journal lives outside the clean cache tree,** because it cannot
   be regenerated. The cache is readable only by the daemon's account and
   carries a [`CACHEDIR.TAG`](https://bford.info/cachedir/) as a hint to
   backup tools. Encryption at rest and where the cache may live are in
   O27 and O41.
8. Chunk size (we propose 1 MiB to start), pool sizes and eviction
   constants are left to the builder and to measurement.

Why: a private cache multiplies copies on a multi-user host, so the
budgets are what keeps one account from starving another. A filesystem
index needs no dependency and no recovery code. Content-hash deduplication
across contexts was rejected (Alternatives).

**The write journal**

### C28. What the journal never does

**Firm.**

The journal is the only place a write that the application believes saved
can live while it is not on the server.

1. **Nothing is stored over a version of a file the client has not seen
   without the user's action.**
2. An **immediate store**, made while the application's commit is in
   progress and the server is reachable, is unconditional: the last closer
   wins, as with every AFS client.
3. A **deferred store** (a record left by a failed or timed-out commit, a
   restart or an upgrade) is never stored automatically. It is kept for
   explicit recovery.
4. **A lost reply** (the store was sent and no answer came) is ambiguous.
   The record is quarantined as a conflict and the server's file is left
   as it is. It is never taken as applied and never replayed blindly.
   Completing an operation already sent before a timeout is not a new
   store: its outcome is read back.
5. **A record is replayed only under a token for the same AFS identity
   and cell,** and only on an explicit instruction.
6. A record is written durably, with a checksum, **before** the write is
   acknowledged to the application.
7. Recovery data stays in a store only the daemon can read. It is never
   written as a file beside the original unless the user who owns it asks.
8. An upgrade keeps quarantined records as they are. It never forces a
   retry or a discard.

Why: assumption A9. With no conditional store, no check the client makes
beforehand, a held callback included, can stop a second writer committing
between the check and the store. And a version and size that look right
after a lost reply do not prove which write produced them. So the only way
never to overwrite unseen data is never to store late without a person
deciding.

### C29. Journal behaviour and recovery

**Working.** Revisit if: assumption A9 is settled the other way (AFS-3, or
a later profile (N1), offers a conditional store or an operation
id), or M3 use shows explicit recovery is needed often enough to burden
users.

1. **A record holds** the credential context's AFS identity and cell; the
   file's full id and the volume's incarnation; the data version the write
   was made over; the exact change (range and bytes, truncate, create,
   rename, remove); and its order among that file's records. The record
   format is in N32 to N35.
2. Records of one file are ordered. After a confirmed store, the next
   record's base is the version that store produced.
3. A record for a rename, link or unlink names both parent directories,
   the source and target names and their file ids, and is ordered with the
   other records of both directories. Recovery checks each id before it
   acts.
4. **Commands.** `afs pending list` shows what waits and why. `retry`
   stores a record over the server's version after the user has seen the
   difference. `discard` drops it. `show` and `export` give the bytes. A
   user acts on their own records and an administrator on all. Every
   discard is an audit event.
5. A record's owner is the context that wrote it. After that context
   ends, its account may view or export the bytes; replaying still needs a
   current token for the same AFS identity (C28).
6. After a restart, records wait for their user's token (C42).
7. **On a failed or uncertain store,** the dirty data is detached from
   everything readable and the affected handles fail with the write error.
8. Budgets are per account (C27). A background store that keeps making
   progress may run past the commit deadline (C9); one that stalls fails,
   and its data stays in the journal.
9. A record is made durable by writing the file, syncing it, and syncing
   its directory. Removal is the reverse.

Why: explicit recovery is the cost of C28. These points make that cost
visible and bounded: a user can always see, keep, export or drop what did
not reach the server.

**The network**

### C30. One socket per address family; connections per context

**Working.** Revisit if: the M2 tests show a NAT or firewall that maps the
callback port differently from the calling port, or one `rx` thread limits
measured throughput.

1. The client uses one UDP socket per address family, on port 7001 by
   convention `[DES-011]`, for outgoing calls and for the callback service.
2. One `rx` thread owns the sockets. It never takes a cache lock.
3. Rx connections are keyed by server, service, security class and level,
   and credential context with its generation. Status, data and stores
   have separate queues.
4. The installer opens the callback port in the host firewall.

Why: with one socket, the NAT mapping an outgoing call creates is the one
the server calls back on. Connections per context mean one user's slow
store never holds a channel another user's read needs, and no connection
is ever shared across contexts (C18).

### C31. What the callback service does with what it receives

**Working.** Revisit if: a fileserver offers rxgk-protected callbacks to
clients (then a protected break is authoritative evidence under C24), or
the test cell shows a callback procedure this table lacks.

Before rxgk, nothing the callback service receives is authenticated. One
table governs all of it:

| Received | Effect |
|---|---|
| A break naming files, from a known server's address | those files' promises are suspended in every context, and refetched on next use |
| A break naming files, from an address matching no known server | the same, for the named files only |
| A break or reset naming no files | every promise from the servers that source could be moves to a new suspect epoch; affected entries are fenced until revalidated |
| More than the rate limit can process | the suspect epoch of the affected servers (of every server, if the source is unknown) advances **before** work is dropped or acknowledged; drops are counted |
| A probe | evidence that the server can reach the client (C32), never authority over any data |

1. A break changes the core's counters before the reply goes back, and
   never waits on a layer (C16).
2. **A break from a known server is never discarded.** Over that server's
   rate limit, it marks the server's callbacks suspect instead.
3. A break's source is matched to a server through the addresses the
   volume location database lists `[DES-058]` for servers the client holds
   callbacks from.
4. The service answers only an allowlist of callback procedures, drawn
   from published protocol material and tests against the test cell.
   Remote debugging queries get no data (O15).

Why: C24. A forged break can cost a refetch and nothing more, and a real
break is never lost to a rate limit. The longest a dropped break can leave
data stale is bounded by C32.

### C32. Callback trust and bounded revalidation

**Working.** Revisit if: M2 measurements against the test cell and through
NAT show the intervals below serve stale data or cost too many
revalidations.

1. Callback trust is kept per server and per local endpoint generation.
   It is earned by evidence that the server can reach the client: an
   attributed inbound call to the callback service (a probe or a break)
   from that server, no older than **E** (ten minutes by default,
   configurable), and since the last network change or resume.
2. Without fresh evidence the server is in **bounded revalidation**: its
   data is served only if its status was validated within the last **T**
   seconds (we propose 30 to 60). Otherwise it is validated before
   delivery. If that validation cannot complete, the request fails at its
   deadline; stale data is not served.
3. With trust, cached data is served while its callback is valid and the
   credential and grants are valid, with no further cap since last
   contact. A break, or a known loss of callback continuity, ends that.
4. The client sends keepalive probes (we propose every 60 to 100 seconds)
   to servers that hold its callbacks, to keep a NAT mapping open.
5. **Resume is checked when data is served,** not only on a notification
   from the operating system. Each serve compares the clocks of C37, and a
   jump marks every callback suspect before the answer is given.
6. The state is shown in `afs status` and in the logs.

Why: a callback promise is worth nothing if the server cannot reach the
client, which is the normal case behind NAT, a firewall or after a laptop
sleeps. Trust earned by observed inbound traffic separates "the server
would have told us" from "we would never have heard".

### C33. Finding a cell's servers

**Working.** Revisit if: the oldest supported Linux release ships
systemd 256 or later (then the resolver's own record interface is an
option `[DES-003]`), the M2 tests show an out-of-scope query on any
supported configuration, or the status of RHEL's local resolver changes
`[DES-007]`.

1. **Order.** Configuration first, then DNS SRV, then AFSDB
   `[DES-008]` `[DES-010]`. Configuration always wins. SRV priority comes
   before every other ranking input, and a sole SRV target of "." means
   the service is not offered `[DES-009]`.
2. **Windows** uses `DnsQueryEx` with the system's server list and
   interface selection `[DES-004]` `[DES-005]`. **macOS** uses
   `DNSServiceQueryRecord`, asking for intermediate results so a
   conclusive negative answer is seen `[DES-006]`. On both, IronAFS
   leaves the choice of server and route, a VPN's included, to the
   operating system.
3. **Linux and other Unix builds** use one first-party DNS client, the
   same for glibc and musl builds. It queries only the host's own
   established local resolver, for example systemd-resolved's stub
   `[DES-002]`, the dnsmasq that NetworkManager can run as a local caching
   name server
   ([NetworkManager.conf](https://networkmanager.dev/docs/api/latest/NetworkManager.conf.html)),
   or RHEL 10's dnsconfd, which Red Hat's encrypted-DNS documentation
   describes as a local DNS cache and split-DNS daemon `[DES-007]`. The
   client detects the resolver at run time and never requires a
   particular one.
4. **On a flat list of name servers** with no local resolver, discovery
   runs only after an administrator confirms that list is the intended
   DNS policy. The listed servers are then asked in order, with documented
   timeouts. The confirmation is rechecked when the network or VPN policy
   changes.
5. **Otherwise** the client uses only explicitly configured servers. For
   a cell reachable only through a VPN, the cell's name is not sent unless
   a managed route keeps the query inside the intended scope.
6. Cached records and the rankings derived from them expire together, are
   dropped on a network change, and late replies from an earlier network
   state are discarded.
7. **The location cache** keeps volume location entries with lifetimes
   and refreshes at once on "moved" or "offline". Read-only replicas are
   ranked by measured round-trip time and administrator preference. Busy
   and offline answers are retried with jittered backoff up to the
   deadline. A dead server fails fast until a probe succeeds.
8. Discovery can be turned off globally and per cell. Status reports
   where each answer came from and, separately, what the resolver said
   about validation.

Why: looking up a cell name tells the network which cell a user wants, so
the query must go where the host's own policy sends it. On Linux a
first-party client to the local resolver keeps routing in the host's
hands, is one fuzzed decoder for every Unix build, and needs no `unsafe`
call into the C library's resolver (C5). The record interface that would
avoid raw DNS arrived only in systemd 256 `[DES-003]`. Sending the stub
plain DNS is a route systemd documents, with the caveat that some
resolution concepts do not map to it `[DES-002]`.

### C34. Addresses and IPv6

**Working.** Revisit if: assumption A11 fails, or a negotiated extension
(N22) supplies fileserver addresses other than IPv4.

1. Every address inside the core is an address of either family, never an
   IPv4-only type. That covers configuration, the location cache, server
   identity, callback trust, logs and IPC.
2. The callback service listens on both families.
3. Database servers found through configuration or DNS are tried over
   IPv6 when they answer there, with a clean fallback to IPv4 and a record
   per server of what works.
4. Fileservers are reached at the IPv4 addresses the volume location
   database gives `[DES-058]`.
5. On an IPv6-only network the client reaches those addresses through
   NAT64 and 464XLAT (assumption A11).

Why: AFS-3 is an IPv4 service today `[DES-058]` `[DRV-M24]`. Keeping both
families in every internal type means a later extension adds data, not a
redesign.

### C35. Security classes and their order

**Working.** Revisit if: an OpenAFS release offers rxgk between clients
and fileservers, an independent peer and wire profile are agreed, or the
rxkad specification route of C6 fails.

1. The first authenticated class is **rxkad with Kerberos 5, including
   rxkad-k5 and rxkad-kdf**, in M2b, behind the specification gate of C6.
2. M2a is unauthenticated and read-only (Rx's null class), to prove Rx,
   the cache and the Linux layer first.
3. **rxgk** is planned for M6. Its work, and what waits on it (the shared
   pool, C21), is tested against a simulator and independent peers until a
   server offers it (N25).
4. Cells that depend on single-DES service keys are outside what this
   client supports.
5. For rxkad the documentation says plainly what the class does not give:
   protection against a party that holds the user's session key (C18).

Why: existing cells' clients use rxkad `[DRV-089]`, and OpenAFS's advisory
on single-DES service keys describes rxkad-k5 and rxkad-kdf as the fixes
`[DES-049]`; we assume a client without them cannot serve the cells that
applied those fixes. OpenAFS offers no client-to-fileserver rxgk yet
`[DES-048]` `[DRV-M25]`.

### C36. Performance choices made at the start

**Working.** Revisit if: assumption A8 fails, M2 measurements show the
window, readahead or prefetch settings miss their targets, or a FUSE
passthrough mode ships that keeps every read under the daemon's control.

1. After a listing, status is fetched with InlineBulkStatus, falling back
   to BulkStatus when a server refuses it (assumption A8). It runs on
   demand, with bounded prefetch. Each item's success and rights are
   checked on their own and stay in the context that fetched them (C22).
2. The client advertises a large Rx receive window.
3. Readahead is adaptive and charged to the reader's own connection.
4. Concurrent fetches of the same data in one context share one RPC
   (C22).
5. FUSE requests of up to 1 MiB are negotiated where the kernel allows
   (L2).
6. **FUSE passthrough is not used.**

Why: a peer that advertises no window is assumed to have one of 16 packets
`[DES-056]` and, without other information, packets of 1444 bytes
([Rx specification draft](https://registrar.central.org/rx-spec.txt),
acknowledgement trailer); by our arithmetic that caps one call at about
2 MB/s at a 10 ms round trip. Passthrough needs `CAP_SYS_ADMIN` `[DRV-007]` and lets reads
bypass the daemon `[DRV-008]`, which would defeat revocation (C15).

**Time and threads**

### C37. Three clocks behind one seam

**Working.** Revisit if: the M2 sleep, hibernation, Modern Standby and
VM-pause tests show a clock below does not behave as its documentation
says, or a supported target lacks a valid pair.

1. The `Clock` interface gives three clocks:
   - a **boot clock that counts suspend,** for every deadline a server
     grants: callbacks, tokens, location lifetimes, locks;
   - an **awake clock that excludes suspend,** for round-trip and
     retransmission timers;
   - **wall time,** only for AFS timestamps and for token expiry as the
     protocol states it.
2. The pairs, behind `ironafs-os` (C5):
   - Linux: `CLOCK_BOOTTIME` and `CLOCK_MONOTONIC` `[DES-012]`;
   - macOS: `CLOCK_MONOTONIC_RAW` and `CLOCK_UPTIME_RAW`, read with
     `clock_gettime_nsec_np` `[DES-013]` `[DES-014]`;
   - Windows: `GetTickCount64` `[DES-016]` and
     `QueryUnbiasedInterruptTimePrecise` `[DES-015]`.
3. Rust's `Instant` is used for neither.
4. A detected suspend, a regression of a monotonic clock, or a large jump
   of wall time marks callbacks suspect (C32). Wall time can only lower
   trust.
5. Startup is refused without a valid pair. If a later read fails,
   outstanding deadlines expire, callbacks become suspect, one event is
   logged per episode, and no new deadline is set until a valid sample
   returns.
6. The client never sets the system clock.
7. Documented residual: a VM restore that rolls back every observable
   clock together can escape detection.

Why: a callback or a token granted for an hour is not valid after a night
asleep, and a retransmission timer must not fire a burst on waking. No
single clock does both, and `Instant` does not say which it is
`[DES-017]`.

### C38. Time is 64-bit inside; wire times are converted in one place

**Working.** Revisit if: the wire time table (C6) shows a field whose
published definition this rule cannot represent.

1. Inside IronAFS, wall time is one signed 64-bit type (seconds and
   nanoseconds since 1970). Boot and awake time are 64-bit nanoseconds. No
   32-bit time exists outside the wire codecs.
2. Every wire time field is converted in one module of `ironafs-proto`.
   For each field it states the width, signedness, epoch and granularity,
   from the specification.
3. Conversion never sign-extends by accident and never wraps. A value the
   wire field cannot carry is refused with a stated error; for example, a
   file time the server cannot store is refused at store, not truncated.
4. Arithmetic on server-granted times is done in 64 bits after
   conversion.
5. Tests cover the 32-bit boundaries of 2038 and 2106 `[DES-019]`, times
   before 1970 and zero, and simulator runs with the clock set in 2039 and
   in 2107.

Why: a signed 32-bit time wraps in 2038 and an unsigned one in 2106
`[DES-019]`, inside the life of this software. One conversion table is one
place to review.

### C39. Threads and locking

**Working.** Revisit if: M2 load tests show the fixed set starves a class
of work, or lock-order checks find an RPC wait under a lock that cannot be
restructured.

1. `ironafsd` has a fixed set of threads: one `rx`; one or two for
   callbacks; one for housekeeping (expiry, lock renewal, token expiry,
   location refresh, probes, eviction); a pool for stores; one notifier
   per in-process layer (C17); and the layer's own.
2. **Ingress is separate from execution.** A layer's reader threads only
   read requests, record their arrival time, and themselves handle
   cancellation and cleanup. They hand work to a bounded executor without
   blocking. A request that finds the executor full gets a retryable
   error and is counted.
3. Workers wait on one-shot reply slots with a deadline counted from
   arrival and a cancel token. Every queue is bounded in bytes.
4. **No lock is held across an RPC wait, a disk write or a kernel
   notification.** Lock order is server, volume, object, chunk.
5. A watchdog restarts a daemon that stops making progress. On Linux it
   is the service manager's `[DES-042]`, and the mount is then aborted and
   replaced as L7 describes.

Why: a slow fileserver can fill the executor, but it must never stop an
interrupt, a forget or a release from being served; that is how a hang
that needs a reboot is avoided. Byte limits are coding standards rule 3.

**Identity and tokens**

### C40. One credential context, built from what the OS attests

**Firm.**

1. Tokens, Rx connections, caller capabilities, grants, cache namespaces,
   journal records and recovery are all keyed by one daemon-issued
   **credential context** id.
2. A context's local parts are what the operating system **attests** for
   a request or a connection, never what a caller asserts (C41).
3. The AFS identity, the effective identity list and the security class
   and level each call actually ran at complete the key. The level is the
   one used, never the configured one.
4. A token is bound to the context the operating system attests for the
   connection that delivered it, never to a context named inside the
   message.
5. The daemon never reads a user's credential cache, and never prompts.

Why: every isolation rule in this part reduces to "which context is
this?". If any part of the answer came from the caller, a caller could
choose another user's.

### C41. What each operating system attests; no PAGs

**Working.** Revisit if: Linux or macOS reports a per-session marker with
each filesystem request that a process cannot forge, or the Windows spike
(L22) shows the logon session cannot be had from the SMB session.

1. **Windows.** We expect the authenticated SMB session to give the
   logon session; the Windows spike (L22) confirms it. The elevated and
   the limited token of one interactive logon have separate, linked logon
   sessions `[WIN-028]`; IronAFS treats them as one context. Two logons of one account are two contexts, each with its own
   tokens.
2. **Linux and macOS.** The kernel reports the caller's uid and gid with a
   request `[DES-022]` `[DRV-025]`, so every session of one uid is one
   context.
3. There are **no process authentication groups (PAGs).** Two logins of
   one account cannot hold different AFS identities for the same cell on
   Linux or macOS: the later token replaces the earlier. This is
   documented.

Why: a PAG needs a per-session marker the daemon can trust. A FUSE request
carries a uid, gid and pid `[DES-022]` and FSKit's context carries user and
group ids only `[DRV-025]`; neither carries a session. OpenAFS tracks PAGs
using artificial groups on some platforms `[DRV-075]`; the FUSE kernel
module passes no group list `[DRV-065]`, so even that marker would not reach
the daemon.

### C42. Tokens live in the daemon's memory

**Working.** Revisit if: a site asks for tokens to survive a restart and a
design is shown that keeps C40 and C43.

1. AFS tokens and session keys live only in `ironafsd`'s memory, under
   their context and cell. They are not written to disk.
2. **`aklog` runs as the user:** in the `afs` command, in a login service
   or in the Windows session agent. It reads the user's own Kerberos
   credentials through the operating system's library, builds the token,
   and sends it over the control endpoint (C45).
3. **After a daemon restart** tokens are gone. Each login service and
   session agent watches the daemon's instance id and runs `aklog` again
   when it changes, for a context that is still authorised, from that
   session's own valid Kerberos credentials. Journal records wait for the
   token (C29).
4. Sign-out, or a deliberate removal of a token, ends that authorisation.
   A restart cannot undo it.
5. A context that cannot get its token back shows "no token" in
   `afs status`, with a notification. A context with no agent must run
   `aklog` itself. This is documented.
6. Renewal and revocation follow C14.

Why: a token on disk is a secret that backups, images and other
administrators can reach. Having the user's own process build it means the
daemon never needs access to anyone's Kerberos credentials (C40).

### C43. A token sender authenticates the daemon first

**Firm.**

1. No credential material is read or sent until the sender has shown that
   the endpoint belongs to the daemon.
2. The check binds the daemon's instance id from the handshake (C44). A
   failed check aborts before anything secret moves.
3. The automatic `aklog` after a restart (C42) goes through the same
   check, so a process that takes the endpoint's place during a restart or
   an upgrade receives nothing.
4. The planned mechanisms, each to be confirmed in its layer's gate:
   - **Linux:** the control socket lives in a directory the service
     manager creates under `/run`, owned by the daemon's service account
     and not writable by others
     ([`RuntimeDirectory=`](https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html),
     with `RuntimeDirectoryMode=`), so no other user can create a socket
     there while the daemon is down. The
     sender checks that the peer's uid, from
     [`SO_PEERCRED`](https://man7.org/linux/man-pages/man7/unix.7.html),
     is the daemon's service account.
   - **macOS:** the same under a root-owned directory the launch daemon
     creates, checked with `LOCAL_PEERCRED`
     ([`unix(4)`](https://man.freebsd.org/cgi/man.cgi?query=unix&sektion=4&manpath=macOS+26.7)),
     as another macOS project does for its own socket `[MAC-057]`.
   - **Windows:** the daemon creates its pipe as the first instance,
     refusing remote clients
     ([`CreateNamedPipe`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipew)),
     with an access-control list that lets only its service identity
     create instances. Services can run under a virtual account with its
     own SID `[DES-074]`. The sender checks the pipe server's process
     ([`GetNamedPipeServerProcessId`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeserverprocessid))
     before sending.

Why: the endpoint's name is public and the daemon is sometimes down. A
process that listened in its place would be handed every user's token.

**Local IPC and errors**

### C44. Local IPC: one framework, its encoding and compatibility rules

**Firm.**

1. There is one IPC framework: one framing, one handshake and one set of
   message definitions in XDR
   ([RFC 4506](https://www.rfc-editor.org/rfc/rfc4506)). `xtask`
   generates the Rust and Swift codecs from those definitions. There is
   one set of fuzz targets.
2. The handshake carries a version range, the role (C45) and the daemon's
   instance id.
3. IPC is untrusted input in both directions (overview, constraint 4), and
   a long-lived format under the `wire-format` skill.
4. **Compatibility, control endpoint.** Within a major version, only new
   messages and optional fields are added. The daemon accepts the current
   and the previous minor version, so an older `afs` keeps working during
   an upgrade.
5. **Compatibility, adapter endpoint.** Versions must match exactly,
   because adapters ship with the daemon.
6. What an IPC message says about a principal and a negotiated suite is
   in N36, N39 and N41. The message sets themselves are in the specification for
   IPC, written before the code.

Why: the `afs` command, login services and agents outlive a daemon
upgrade, so their channel needs a stated promise; an adapter does not. One
definition and generated codecs keep the Swift and Rust sides from
drifting.

### C45. Two endpoints; adapters bind only by the daemon's registration

**Firm.**

1. **The control endpoint** is open to any local user. The caller is the
   peer the operating system reports. It carries tokens, `fs` operations,
   status, diagnostics, configuration and a small set of user-level
   diagnostic events, never audit events (O9). The Windows session
   agents use this endpoint.
2. **The adapter endpoint** is open only to adapter processes the daemon
   itself registered. It carries the three kinds of operation (C7), the
   invalidation stream with its acknowledgements (C17), and event records.
3. A binding is made only from a **daemon-owned registration.** The
   daemon starts the adapter, or, for the FSKit extension that macOS
   starts, checks its code signature and the peer the system reports. It
   hands the adapter a one-time binding capability for one mount or root.
   A binding without one is refused.
4. **Compromise boundary.** An adapter process holds only capabilities
   the daemon issued. A compromised adapter can read and forge the traffic
   of the sessions it currently serves. It cannot create a capability for
   anyone, and cannot reach another session's opens: the daemon records
   which opens belong to which capability and checks every request against
   that record before it reaches the core. The SMB front end's keys and
   sessions are in L18.

Why: two endpoints put the operating system's own access control in front
of the role that may act for any user, as a second barrier behind the
binding capability. The adapter role is the most powerful thing on the
machine after the daemon, so it is never reachable by name alone.

### C46. Errors

**Working.** Revisit if: a layer's error mapping cannot express a contract
error without losing what the user needs to act on it.

1. The contract has one semantic error type. Its values include: not
   found, exists, access, no token (`TokenRequired`), token expired
   (`TokenExpired`), token revoked (`TokenRevoked`), below the security
   floor (`SecurityFloor`), read-only, no space, quota exceeded,
   cross-device, stale, busy, offline, timed out, not yet stored (C9),
   interrupted, not supported, would block, unreachable and conflict. The
   token errors and `SecurityFloor` reach applications as a permission
   error, with the cause in the side channel (N4, N6, O35).
2. Each layer maps it to its system's errors: errno, an `NSError` with a
   POSIX code, or NTSTATUS.
3. **An AFS abort code is classified by the RPC and the server that
   returned it, never by its number alone.**
4. **A failure never looks like success:** never an empty or short
   listing, zero bytes or a made-up attribute.

Why: the volume error codes collide with errno values on many platforms
and must be told apart by context `[DES-055]`. Point 4 is the "silent wrong answer" anti-goal
applied to the error path.

## Consequences

1. **Users of an existing AFS client will notice differences.**
   - No PAGs on Linux and macOS: one uid has one AFS identity per cell
     (C41).
   - Tokens do not survive a daemon restart; they come back through the
     session's own Kerberos credentials (C42).
   - A write that could not be stored is never stored later on its own.
     It waits in `afs pending` for its owner (C28, C29).
   - A commit that passes its deadline reports "not yet stored" instead of
     blocking without limit (C9).
   - The cache starts cold after a daemon restart, until C26's gate is
     met.
   - Anonymous access is not cached (C20).
   - A removed file that is still open stays visible under a reserved
     name until it is closed; it counts against quota and can block the
     removal of its directory (C10).
   - Rename with "do not replace" fails (C10).
   - Where a kernel cannot isolate mapped pages, programs cannot be run
     from `/afs` (C15).
   - Without proof that a server can reach the client, cached data is
     revalidated every T seconds, and a request fails instead of returning
     stale data (C32).
   - On a Linux host with only a flat list of name servers, DNS discovery
     of cells is off until an administrator confirms it (C33).
2. **Cost on multi-user hosts.** Until fileservers offer rxgk to clients,
   every active credential context fetches and stores its own copy of what
   it reads, and a new login starts cold. The budgets of C27 bound it;
   they do not remove it.
3. **What cannot be promised.** Pages resident in a live mapping stay
   readable after revocation (C15). Root and the daemon's own account see
   everything in the cache. A change of the client's address behind a NAT
   is seen only at the local timeout (C23). Occupancy and timing of the
   cache reveal that it is in use (C27).
4. **Work that must come first.** The specification gates of C6; the ADR
   for `ironafs-os` (C5); the dependency decisions for the cryptography
   crate and the Kerberos libraries (the `add-dependency` checklist); the
   specification for IPC (C44). Each becomes an issue in its milestone.
5. **Coding standards.** Rule 18 is written for async code. It is
   reworded for threads, deadlines and cancel tokens when this ADR is
   accepted (C3).
6. **The simulator is not optional.** C3's cost is only repaid if the
   simulator exists from M2a; without it the injected interfaces are
   overhead.
7. **What this rules out:** an async runtime; an embedded database for
   the cache; any cache entry shared across users under rxkad; automatic
   replay of old writes; a layer that reaches the cache without the
   contract; tokens on disk.

## Alternatives considered

- **An async runtime** (a third-party one, or one of our own): a large
  dependency outside coding standards rule 21 that ties every crate to one
  executor. Our own would carry the same cost with no gain over C3.
- **The cache manager as one hand-written state machine:** every
  filesystem operation becomes a continuation written by hand. C3 keeps
  pure state where it pays (the protocol code and the cache's state types)
  and blocking code elsewhere.
- **The core inside each operating-system extension** (no daemon): the
  cache and every user's tokens would die with the extension, and each
  system would host the core differently (C1).
- **Linking an existing C Rx:** ruled out by ADR 0001.
- **One cache shared by all local users, with rights checked per file:**
  it answers whether a reader may read, not who vouches for the bytes.
  Under rxkad a user holds the key of the connection that fetched them
  (C18).
- **A shared cache split into an integrity-protected pool and an
  unprotected one:** transport integrity is the wrong test; key custody is
  the right one. Under rxkad the protected pool would be empty, so this
  collapses into the previous alternative.
- **Private namespaces with content-hash deduplication:** the timing of
  deduplication across users can reveal that another user holds the same
  content, the side channel described by
  [Harnik, Pinkas and Shulman-Peleg, 2010](https://doi.org/10.1109/MSP.2010.187),
  and each user still pays a cold fetch.
- **Automatic replay of deferred stores when a callback was held
  throughout:** a held callback does not stop a second writer committing
  between the check and the store (C28, assumption A9).
- **Persisting tokens across restarts:** a secret on disk, for a
  convenience the session agents already give (C42).
- **PAGs emulated from process or group information:** a FUSE request
  carries no session, and anything a process can set about itself is an
  assertion, not attestation (C40, C41).
- **Reusing clean cache across restarts from the first release:** unsafe
  until a restored volume can be told apart (C26).
- **The C library's resolver for DNS discovery on Linux:** it sends the
  query to the server its configuration names and applies no routing of
  its own `[DES-001]`, which is no better than the first-party client on a
  host with a local resolver; it also needs an `unsafe` binding and
  differs between glibc and musl builds (C33).
- **systemd-resolved's record interface:** it needs systemd 256
  `[DES-003]`, far newer than the oldest candidate release ships (CentOS
  7.9 ships systemd 219 `[PLT-018]`).
- **One IPC endpoint for both roles:** the adapter role would then be
  protected by the binding capability alone (C45).
- **An embedded database as the cache index:** a dependency, a recovery
  path and a format to keep, for what the filesystem already does (C27).
- **LRU eviction:** S3-FIFO's FIFO queues do not reorder on a hit (Yang
  et al., section 4), and its history can be kept per namespace (C27).
- **FUSE passthrough for speed:** reads would bypass the daemon, so
  revocation could not be enforced (C36).
- **rxgk first:** OpenAFS offers no client-to-fileserver rxgk yet
  `[DES-048]` `[DRV-M25]`, and existing
  cells could not be used at all until rxkad was done (C35).
