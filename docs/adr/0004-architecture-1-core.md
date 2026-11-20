<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0004, part 1: Core

Status: proposed (2026-11-20). Part of ADR 0004 ([overview](0004-architecture.md)); raised in issue #20.

## Context

The core is everything the three operating-system layers share: the AFS
protocols, the cache, tokens, locks and the write journal. This part decides
how it is built, where it runs, and what it promises the layers. The
overview gives the goals, the anti-goals, the constraints, the named
assumptions (A1 to A14) and the evidence rules. The facts this part rests
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

**Firm** (points 1 and 2, a security invariant). **Working** (point 3).
Revisit point 3 if: an operating-system interface needs a presentation
that the rights and volume type the core reports cannot compute. An
interface that could be served only if its layer decided one of the
things in point 1 needs a superseding ADR.

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
   machines: bytes, time and randomness in, actions out. They open no
   socket, read no clock and draw no random numbers.
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
   | `ironafs-time` | the `Clock` and `Rng` interfaces of C3, the boot, monotonic and wall instant and deadline types (C37), and the 64-bit time types (C38); it has no dependencies |

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
     and its own operating-system bindings under their ADR, and uses
     cryptography only through `ironafs-crypto`;
   - `ironafs-rx` knows nothing of AFS.
5. The macOS extension is written in Swift, uses codecs generated from the
   IPC definition (C44) and contains no Rust.
6. Where the first-party DNS client of C33 lives is settled with its
   specification; we propose a pure codec beside `ironafs-proto` and its
   socket in `ironafs-net`.
7. The `Clock` and `Rng` interfaces and the time types are in
   `ironafs-time` (point 1). The production clocks are in `ironafs-os`
   (C37), and the production `Rng` is the cryptographic random source
   behind `ironafs-crypto` (C5, point 1). The simulator is a test-only
   workspace crate, `ironafs-sim`, which is never shipped. The state
   directory's format marker (N8) is in `ironafs-cache`, beside the
   journal record formats (N32).

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
   validated module. The cryptographic random source comes through the
   same interface. The production `Rng` (C3, point 3) is this source; the
   simulator seeds its own.
2. rxkad's cipher (fcrypt) is written by IronAFS from the specification
   that C6 requires. It is never treated as approved cryptography.
3. The Kerberos encryption-type operations rxgk needs run in the client,
   so they sit behind `ironafs-crypto`. The GSS exchange and its
   pseudo-random function `[DES-060]` stay in the operating system's GSS
   library, behind `ironafs-gss`. `ironafs-rxgk` stays pure (C3): it
   makes no GSS call, and takes the exchange's results and the function's
   output as inputs when the client is composed. The exact interface is
   part of the rxgk profile record (N25).
4. Kerberos and GSS come from each system's own library (`ironafs-gss`).
5. `ironafs-os` holds the few calls that have no safe route: the clock
   pairs of C37, peer credentials (C43), the service controls, the log
   calls of O8, the dump controls of O26 and, on Windows, the query of the
   operating system's FIPS algorithm mode (O31, point 4). It needs
   `unsafe`, so it gets its own ADR with a closed list of named sites and
   safe interfaces on top, before any of it is written. Since Rust 1.82 a foreign declaration can be marked safe
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
   - before `ironafs-proto`: the table of wire time fields (C38), inbound
     and outbound;
   - before the cache: what evidence identifies a volume's incarnation
     (C25), the directory format and the callback procedures;
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
   Every handle operation is checked as C13, point 2, says. A rename or a
   delete disposition through a handle also needs the delete right on the
   source directory, and a rename needs fresh authority on the target
   directory. On Windows the names behind
   these forms follow L25, point 5.
4. **Binding operations** carry no caller and change no data: forget,
   release, release directory, reclaim, cancel, volume sync, pin and
   unpin. They can only release or flush what the binding already holds.
5. A handle that comes from the operating system (a descriptor passed to
   an `fs` command) is accepted only after the core has checked that it
   belongs to an IronAFS mount or share and to the volume's current
   incarnation (C25), and that the handle behind it is live and still in
   its credential generation (C13, points 2 and 4). Where the operating
   system cannot identify the open behind the descriptor, the command is
   refused, except the ACL commands on a metadata-only descriptor that
   L35, point 6, allows.
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
   (L17, L18). Calls on the control endpoint take their caller from the
   peer credentials of C45, point 1, instead.
2. A layer cannot construct a caller capability or a handle capability,
   and cannot use one minted for another binding.
3. Local uid 0 has no AFS powers. A local administrator role exists only
   for control operations (O13, O16).

Why: a layer that could name a caller could act for anyone. This keeps a
bug in a layer, or a compromised front-end process, from becoming the
authority of a user it does not serve (C45). It does not stop a
compromised front end from acting for the users it does serve, on its own
bindings: the FSKit extension forwards uids the daemon cannot verify
`[DRV-025]`, and the SMB front end holds its sessions' keys (L18). C45,
point 4, states that boundary.

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
   error. The commit already under way continues as the same immediate
   store while it keeps making progress (C29, point 8), and `afs pending`
   shows its outcome. A commit that stalls becomes a deferred record
   (C28, point 3).
4. `release` never fails.
5. A commit reports success only when the server has accepted the store
   (point 1). Data held in the journal or in a recovery record is never
   reported to the application as stored. This binds the commit
   operation; what `close()` shows where it cannot carry an error is in
   L6, point 5, and L12, point 4.

Why: FUSE does not return an error from a release to `close()`, and may
flush many times or not at all `[DES-029]`. FSKit's cached close takes no
error `[DRV-046]`. A store error must therefore surface at an operation the
application waits on.

### C10. Removing and replacing open files

**Working.** Revisit if: the specification for the file service shows an
AFS-3 operation that renames only when the destination is absent, or one
that removes an entry only while it still names a given file, or the M3
tests show the reserved name cannot be made safe against a concurrent
replacement.

1. **Unlink of an open file, POSIX layers.** The intent is journaled and
   the file is renamed to a reserved dot-name in the same directory,
   carrying at least 128 random bits, under the unlinker's authority. While IronAFS tracks the
   entry it refuses to open or reuse it by name.
2. After the last local close, the entry is removed under the same
   credential context, after the core re-reads the entry and confirms its
   file id immediately before the removal. A removal that cannot be
   checked yet (for example, with no token after a restart) stays
   pending, and `afs pending` reports it. A confirmed mismatch is
   quarantined, never removed. A record holding recovery data for the
   file (C29) blocks the removal until that data is resolved. An
   interrupted sequence is reconciled from the journal. The window
   between the check and the removal is a stated residual: only a party
   with the delete right on the directory can use it, and that party could
   delete the file directly.
3. The unlink is refused when the caller may not rename.
4. **Windows.** SMB's delete-pending and delete-on-close states defer the
   server's removal to the last close, with no rename. They are modelled
   apart from POSIX unlink (L19).
5. **Rename over a destination that is open on this machine** is done as
   one atomic rename. Handles of the renaming context on the replaced
   file fail with `Stale`, as when it is replaced from another machine
   (point 7), and their unstored bytes become a recovery record bound to
   its file id (C29). Handles of other contexts are suspended (C14,
   point 1) and their entries marked suspect (C24, point 2); each is
   retired, and its dirty overlay detached to its owner's recovery record
   (C29), when its own context's revalidation shows the file is gone or
   replaced, or on authoritative evidence (C24, point 4). Until then
   nothing of theirs is discarded. On SMB, a destination opened without
   delete sharing refuses the rename with a sharing violation (L33).
6. **Rename with "do not replace"** is refused with the operating
   system's "invalid argument" error.
7. Opens made on other machines are not visible to this client.

Why: we know of no AFS-3 call by which a client tells a fileserver which
files it holds open, so "gone from the directory, alive until closed" has
to be emulated; that is a specification item too. AFS-3 has no
remove-if-still-this-file call that we know of, so the removal in point 2
is checked first and the remaining window is stated. That
AFS-3's rename replaces a destination in one operation, and that it has no
rename that refuses to replace, are specification items (C6), not cited
facts. Keeping a replaced file for its open handles would take a second
namespace change, so the replacement would not be atomic, and we know of
no AFS-3 operation that does both. Refusing the rename instead would make
every save by rename fail while any local process, even another user's,
held the file open, and the refusal would tell other users that someone
on this machine has the file open (O16). Point 5 therefore treats a
local opener as point 7 treats a remote one, and keeps its unstored
bytes as recovery data. Emulating
"do not replace" with a check and then a rename would
sometimes replace a file, which is the silent wrong answer the overview
rules out.

### C11. Node identity

**Working.** Revisit if: the M2a kernel tests show a kernel mishandles two
node ids for one file under different anchors, or a layer needs node ids
that outlive the daemon.

1. A node id is 64 bits. It maps to a key of cell, volume, vnode and
   uniquifier, plus an **anchor**: how the volume was entered (the dynamic
   root, a cell root, or a mount point). A mount point anchor is the node
   id of that mount point, whose own key carries its own anchor, so the
   anchor names the whole chain of mounts.
2. A node id is never reused within one daemon instance, and is valid for
   that instance only. A daemon restart ends every layer's binding: the
   FUSE mount (L7), the FSKit items (L14), and the SMB sessions and their
   handles (L19).
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
   node id; its components are in N38, point 7. SMB's per-open ids are
   generated per open by the front end (L19).
7. Identifier widths are in N38.
8. A chain of mounts is bounded in depth and checked for cycles, as L32,
   point 3, states. A chain that exceeds the bound or repeats a mount
   point is refused, never followed.
9. A mount point resolves to a volume in the requesting context (C18,
   point 1). The volume in a node's key is the one the mount point
   resolved to in that context. Two contexts that resolve one mount point
   to different volumes get different nodes.
10. **A full node table.** Each node a layer pins is charged to the
    pinning context's budget (C27, point 4). A new pin over that budget, or
    in a table that is full, is refused with a retryable error (busy).
    Pinned nodes are never evicted to make room.

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
   time. This cache is machine-wide and part of cell discovery (C33), not
   a cache entry of any context: it holds only that a name is not a cell.
   It is filled only from a conclusive negative DNS answer, kept no longer
   than that answer's time to live, never filled from a timeout or an
   error, and cleared on a network change (C33, point 6).

Why: points 1 and 2 are the dynamic-root and fake-stat behaviours
OpenAFS's client documents `[DES-054]`, which users of `/afs` already rely
on. Point 3 is our own addition.

**Authority**

### C13. Handles carry authority

**Firm.**

1. A handle is bound to its credential context (C40) and that context's
   credential generation, the node, the open mode, the binding, the rights
   at open, and the volume's incarnation (C25).
2. Where the front end can identify the open, every handle operation
   (C7, point 3: read, write, read directory, commit, lock, get and set
   information, disposition and rename) checks that the handle is live and
   takes the intersection of the rights at open with the current effective
   rights. A right that was lost is refused; rights that remain keep
   working.
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
   - a token for the same context, cell and AFS identity, whose
     restriction class the daemon has itself verified to be unchanged
     (C18, point 7), replaces the old one (what `aklog`, renewal and login
     services produce). For a token kind whose restriction status the
     daemon cannot verify itself, rather than take from the installing
     process, a replacement counts as a change: the generation is retired
     and no handle of the old generation resumes. A token kind whose
     specification (C6) shows no restricted form keeps the generation on
     renewal. **Test:** the same principal with an unverifiable
     restriction status replaces its token; the generation is retired,
     records are detached and no handle resumes; or
   - a callback break makes the file's rights, or its directory's ACL,
     stale; or
   - a local rename replaces the file, for a handle of a context other
     than the renaming one (C10, point 5; the renaming context's handles
     fail with `Stale` there and are never suspended); or
   - the network generation changes (C23, point 2); or
   - the ten-minute bound of point 5 passes for a node with open handles.
2. The core then fetches the file's rights under the current credential.
   If they still cover what the handle needs, it resumes with the same id
   and stores in flight authenticate again. A handle suspended by a local
   rename resumes only if, in addition, its context's revalidation of the
   entry shows the file is neither gone nor replaced (C10, point 5); a
   rights fetch on the old file id alone does not resume it. If not, or if the rights
   cannot be fetched before the deadline, C13 applies, and the operating
   system's caches for the node are purged as for a revocation (C15,
   point 2; C17).
3. **Revoked** means every later operation fails. A handle is revoked when
   the token is deleted, expires without a replacement, or is replaced by
   a different AFS identity; when the volume's incarnation changes; when
   the cell's protocol identifier or configuration generation changes
   (N27), in which case it fails with `Stale`; or when its binding dies.
   Expiry revokes handles on public data too; a fresh open may then get a
   public grant of its own. A handle opened without a token is revoked
   when its context installs a token for that cell, so no anonymous
   authority survives into the authenticated identity (C19; N4, point 2).
4. After write access is revoked, writes stop. Data not yet committed goes
   to a recovery record owned by the writer's durable owner (C40, point 6;
   C29, point 5) and is never stored automatically (C28). From then on the
   handle has no dirty overlay: its next operation of any kind, read
   included, fails with the write error (as C29, point 7). The failure is
   also shown in `afs status`.
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
   another context loaded. This holds in the hardest case too: two
   contexts, both authorised, each served different bytes by the daemon
   (C18), mapping the same node.
4. **Descriptors held by other processes** are revoked with the handle.
5. **Gate, per supported kernel and per operating system.** Resident
   pages, a new fault, two contexts mapping one file (point 3's case), a
   new fault after revocation, and a failed purge while the first mapping
   is still live are each tested. Where a kernel would serve one context's
   fault from another's pages without asking the daemon, or would serve a
   new fault after revocation without asking it, the layer refuses the
   affected mappings only through a mechanism demonstrated on that kernel
   or build (for example, refusing the open modes the operating system
   reports for a mapping), and programs cannot be run from `/afs` there.
   Where no such mechanism is demonstrated, the row does not qualify for
   that layer (O46, point 3) until one is. The gate's measurement, per
   kernel and build, is that a new fault after revocation reaches the
   daemon or front end and fails (L4, L13, L22, point 7). A layer
   promises mapped I/O only where this gate passed.
6. If a purge fails at run time for a node that may be mapped, the core
   cannot show the pages are gone. On Linux the node answers `Stale` in
   every binding, its handles are revoked, and new opens and mappings of
   it are refused until a purge succeeds. The FUSE connection is aborted
   only when the notification channel itself fails. On macOS the item is
   revoked. That stops every later request on the node, and every later
   fault that reaches the daemon. It does not stop a fault served from
   pages still resident, and it does not remove pages already resident in
   live mappings. A failed purge is local evidence that the operating
   system may still serve the node's old state, because its cache for the
   node is shared by every context that holds it. It revokes every
   context's handles on that node and nothing else: no other node, and no
   bytes, epochs or journal records (C24, point 6). The connection is
   aborted only when the notification channel itself fails; aborting it
   for a failed purge would let weak evidence (a forged break, C31) end
   every context's handles on every node, which C24 forbids. **Test:** two
   contexts hold a node; a weak break plus an injected purge failure; both
   get `Stale` on that node only, and their data and records are kept.
   On SMB, C17, point 5's test covers both branches.
7. **macOS: a failed revoke.** If revoking the item fails too, the
   extension ends that user's mount (L9, L10). Nothing the mount served
   stays reachable through it.
8. **Windows SMB.** The front end declines leases (L19, point 5), so it
   cannot recall pages that Windows has mapped. The stated bound is:
   pages Windows already holds for a mapped view stay readable to that
   view until it is unmapped, as in point 2. Closing the SMB open or
   ending the session stops later faults reaching data; it does not
   remove resident pages. The Windows gate (L22, point 7) measures whether
   a new fault after revocation reaches the front end. Where it does not,
   that build fails point 5's gate, and point 5's rule applies: the
   affected mappings are refused only through a demonstrated mechanism,
   or the row does not qualify.

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
   writes it to the kernel or to the SMB connection. A revocation already
   effective at that binding (point 6) stops the reply. A reply already submitted counts as
   delivered. Point 6 says how a binding in its own process makes this
   check.
4. **Local changes fence too.** A store, create, rename or remove made
   through this client updates the core from the server's reply and
   publishes data, attribute and entry invalidations to every alias of the
   node, so other users' kernel caches see the change. These
   invalidations are among the effects C24, point 6, allows on weak
   evidence: they make readers revalidate and destroy nothing.
5. **While a purge is pending** in a binding, the node serves nothing in
   that binding that needs the purged state. There is no exception for the
   user who was last served.
6. **A binding in its own process** (the FSKit extension, the SMB front
   end, C1, point 2) cannot see the core's state, so point 3's check is
   split across one ordered stream on the adapter endpoint (C45):
   - The daemon sends each reply with the binding's current **fence
     epoch**, on the same ordered stream that carries invalidations
     (C17).
   - A revocation or invalidation that affects the binding advances the
     epoch, and the daemon pushes the new epoch on that stream.
   - **When a revocation is effective.** From the moment the daemon
     records a revocation (point 1) it admits no new request for that
     binding under the revoked authority. The revocation is effective at
     the binding when the front end applies the epoch advance: after
     that, no reply produced under the revoked authority starts to be
     written, and replies that started before count as delivered
     (point 3). The front end applies advances and writes replies in one
     serialised step. The revocation is complete when the binding
     acknowledges the advance (C17's acknowledgement) or the binding is
     torn down; a callback's reply to its server never waits for either
     (C31, point 1). The order of these events is S1.
   - The front end compares a reply's epoch with the latest epoch it has
     applied immediately before it writes the reply to the operating
     system or the SMB connection. A reply with an older epoch is
     discarded. A fenced fetch reply is retried as point 2 says; a fenced
     mutation reply follows C28, point 4: its outcome is read back and the
     mutation is never repeated.
   - **Back-pressure:** the stream is bounded (C39, point 3). When it is
     full, replies wait within their deadlines. An epoch advance is never
     dropped: space for it is kept free.
   - **Partial submission.** Fencing is at the boundary of what the
     transport frames, signs or encrypts as one unit. Before the first
     byte of such a unit, the front end checks every reply the unit
     carries. A unit whose write has started is completed; when a later
     advance must stop the rest of a response, no further unit is written
     and the connection is torn down (SMB) or the request fails (FSKit).
     Part of a signed or encrypted unit is never followed by an error on
     the same stream. Which SMB units these are is S2.
   - **Adapter death:** the binding is torn down, which revokes its
     handles (C14, point 3) and ends any revocation pending on it. A read
     reply not acknowledged as written is undelivered; a mutation whose
     reply was not acknowledged as written is uncertain: its outcome is
     read back and it is never repeated (C28, point 4).
   - **Tests:** the front end applies a revocation's advance after a
     reply has crossed the IPC channel and before it writes the reply:
     the reply is not written. The complementary order: the daemon has
     recorded the revocation and the advance is withheld from the front
     end; a reply already issued starts to be written and counts as
     delivered (point 3), a new request under the revoked authority is
     refused from the moment of recording, and the revocation is not
     reported complete until the binding acknowledges the advance or is
     torn down. Two replies in one encrypted
     compound, the second revoked (SMB only). (Also L13, point 10, and L18,
     point 9.)

Why: the "silent wrong answer" anti-goal. A check made before a wait says
nothing about the moment the bytes leave, so the check that counts is the
one in the same step as the write.

### C17. The invalidation stream

**Working.** Revisit if: measurements in M2a show coalescing per node loses
an event a layer needs, or a layer's operating system offers a purge that
cannot fail.

1. Events are **coalesced per node**, not queued one by one. Memory is
   bounded by the node table, never by the rate of breaks. Overflow
   becomes one overflow event that gates every node. Point 6 says how
   events widen and when a binding overflows.
2. Events are **acknowledged per binding,** in this order: new replies
   that need the purged state are gated (C16, point 5); every reply the
   binding issued earlier that could fill the cache again is drained or
   fenced out (C16, points 3 and 6); the operating system's purge is
   applied; then the event is acknowledged.
3. Coherency events go to every binding that holds the node. Rights
   events name a credential context.
4. Each in-process layer has one notifier thread, which writes kernel
   notifications and holds no core lock.
5. **When a purge cannot be completed:**
   - FUSE: the node answers `Stale` and its handles are revoked, and new
     mappings are refused until a purge succeeds (C15, point 6);
   - FSKit: the item is revoked;
   - SMB: the front end completes pending change notifications with
     "rescan". Only if it cannot, which is a failure of the notification
     channel itself (C15, point 6), does it close the session, ending
     that session's other opens too. **Test:** an unrelated file stays
     open in the same session while a node's purge fails; after a
     successful rescan notification the unrelated open stays usable, and
     after an injected failure of the notification channel it ends with
     the session;
   - the Windows replica, if built: the event stays unacknowledged until
     the mechanism its own ADR specifies has removed the stale state
     (L28).
6. **Widening and overflow.** A node's slot per binding holds the union
   of its pending effects (data, attributes, entries, rights) and a
   revision number. A new event merges into the slot and raises the
   revision; where the union cannot be represented it widens as below.
   An acknowledgement (point 2) names the revision it applied and clears
   only effects up to it; an event that arrives while a purge is being
   applied stays pending under the higher revision. **Test:** a rights
   event and an entry event on one node; a second event during a purge.
   Events that do not fit one slot widen:
   - rights events on one node that name different credential contexts
     widen to all contexts;
   - entry events on one directory that name different entry names widen
     to all entries of that directory. A layer applies an all-entries
     purge by invalidating every entry name it has handed its operating
     system under that directory. A layer that cannot list those names
     treats the purge as one it cannot complete (point 5);
   - a binding **overflows** when its pending events reach their byte
     bound (C39, point 3). It then applies the overflow event, which
     carries a revision like a slot: every node
     it holds is gated as in C16, point 5, and purged as above, or
     handled under point 5 where it cannot be purged. A purge that fails
     under overflow applies C15, point 6, to each node that failed, not to
     the binding. **Test:** during an overflow, a new event arrives for a
     node the overflow has already swept, before the overflow's
     acknowledgement; the node stays gated and the new effect stays
     pending under its own revision after the overflow is acknowledged,
     until that revision is applied and acknowledged.

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
   used to reach a file, fetches in flight, and dirty data. Journal and
   recovery records are not generation-locked entries; point 9 says who
   owns them.
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
   contexts, the same user's other sessions included (where the operating
   system can tell them apart, C41), see the server's version until the
   store is accepted, as two machines would.
6. Nothing ever moves from a private namespace to the shared pool (C21).
7. **Lifetime.** Renewal keeps the generation only when the daemon has
   its own record that the AFS identity is unchanged, never on the token's
   opaque contents. Loss of the credential, the end of the session,
   removal of the account, a change of policy, or a change of the token's
   restriction status or of any other token property that narrows the
   rights the server grants retires the generation. The generation's key
   therefore includes the token's restriction class (N36), as the daemon
   itself verifies it; where it cannot, C14, point 1, treats a
   replacement as a change. Retirement means:
   access stops at once, late fetches are fenced off, journal and recovery
   records are detached (point 9), and the remaining files are reclaimed
   afterwards. A recycled uid or SID is a new account instance
   and inherits nothing (where the operating system can tell them apart,
   C41). After a daemon restart no private state is
   attached to a context without positive evidence that the account
   instance, the session, its credentials and the policy continue.
8. **The same boundary holds above the daemon.** Whatever a front end or
   its operating system reuses stops being reused when the grant behind it
   ends. Where a front end cannot enforce that, it does not reuse. The
   per-layer rules are in L5, L11, L19 and L28.
9. **Journal and recovery records** belong to the durable owner of the
   context that wrote them (C40, point 6), not to its generation. They
   survive the loss of the credential, the end of the session, the
   retirement of the generation and a daemon restart. They are removed
   only by a confirmed store or an explicit discard (C29, point 4).
   Retirement detaches them from the generation before it reclaims that
   generation's clean entries. Who may read or replay them is in C28 and
   C29.

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
   from the reader's own status reply (C22). That reply, and the reply for
   every directory and mount point on the path, must be authoritative
   evidence (C24, point 1), as point 1 requires of the entry. A reply
   protected only by a key the reader holds mints a private grant, never
   a shared-pool grant. Only status fields that are the same for every
   caller are shared. Each caller's rights stay private.
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
   item of a bulk status reply counts on its own. A shared-pool grant is
   minted only from authoritative evidence (C21, point 6).
2. The decision that a mask and mode bits allow reading follows the ACL
   specification (C6). Where it cannot be made reliably, the reader's own
   data fetch decides.
3. The client never infers a file's rights from its directory.
4. A grant binds the context and generation, the file id, the volume's
   incarnation, the data version, the effective rights and an expiry. A
   private grant opens only its own namespace; a shared-pool grant opens
   only pool entries. Where a reader has its own dirty data or private
   chunks for a range, those are read in place of pool chunks (C18,
   point 5).
5. A grant is checked before the store is looked up, before joining a
   fetch, after any wait, and immediately before delivery (C16). A reader
   without a valid grant learns nothing, not even whether the data is
   present.
6. A lookup hit (positive or negative), a listing, a resumed enumeration
   and a step through a cached mount point each need the reader's own
   current grant on every directory consulted.
7. A directory snapshot belongs to the handle that opened it. Its cookies
   bind the context, generation, handle, directory incarnation and
   version. Resuming an enumeration needs a current rights grant on the
   live directory (point 6); the cookie is checked against the handle's
   own snapshot, not against the directory's live version. A cookie
   handed to the operating system is an index into snapshot state the
   daemon holds, never the bound values themselves.
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
   never retires their handles (save point 6's purge-failure case) or
   journal records, and never establishes
   that two contexts see the same volume incarnation.
4. Evicting, retiring or changing the epoch of another context's
   entries, of the pool's, or of shared incarnation state needs
   authoritative evidence.
5. Suspect bytes and journal records are kept, not destroyed.
6. **What weak evidence may do across contexts** is this closed list:
   suspend callback promises, revoke grants, mark entries suspect, and
   purge operating-system caches (C16, point 4; C17). Each only forces
   the reader to revalidate under its own credentials and obtain a fresh
   grant. Each keeps bytes, epochs, handles and journal records. The one
   further effect, after a demonstrated purge failure, is revocation of
   the handles on that node (C15, point 6): the operating system's cache
   for the node is shared, so local evidence, not the weak input, decides
   it, and it keeps bytes, epochs and journal records. Anything else
   across contexts falls under point 4.
7. **Callback trust is raised only by a reply to the client's own call.**
   Before rxgk, a server earns callback trust (C32) only from its reply to
   a call the client made, matched by that call's random identity.
   Nothing the callback service receives, probe or break, raises it; such
   input may only lower trust.

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
   file's shared entries. For other contexts and the pool the
   observation is weak evidence: it marks their entries suspect (C24,
   point 2). Evicting them, or changing the file's shared epoch, needs
   authoritative evidence.
5. An incarnation change invalidates the volume's handles, directory
   snapshots and Windows identities, and quarantines its pending journal
   records for explicit recovery (C28, point 3; C29).
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

1. **The data store** holds chunks keyed by namespace, then cell, volume
   and its incarnation, the local file epoch, vnode, uniquifier, data
   version and chunk index. The namespace is the credential context and
   its generation, plus the security profile and suite (N39) of the
   connection that fetched the entry; or the pool, with the cell, its
   protocol identifier and configuration generation (N27). Through the
   context (C40, point 3) a private namespace also carries the cell's
   protocol identifier and configuration generation. A new data version
   makes old chunks unreachable without touching them.
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
3. A **deferred store** (a record left by a stalled or failed commit, a
   restart or an upgrade, or an upload of a change seen by the Windows
   replica, L27, point 3) is never stored automatically. It is kept for
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

1. **A record holds** its durable owner (C40, point 6), which includes the
   AFS identity and cell; the
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
   difference. `discard` drops it, and only the record's durable owner or
   an administrator (below) may discard it. `show` and `export` give the
   bytes to the record's durable owner (point 5) only. Only the durable
   owner instructs a retry, after seeing the difference; a context with
   the same AFS identity under another account instance may then carry it
   out under C28, point 5, without seeing the bytes or the difference
   (C18, point 2); executing the retry grants no permission by itself,
   and an account that also holds the administrator role keeps that
   role's permissions (O16, point 5). A retry always needs C28, point 5.
   **Test:** two local accounts, neither an administrator, hold one AFS
   identity; the second account's attempt to replay a record before its
   owner has consented is refused; after consent it carries the retry
   out; its requests for the bytes, the difference and a discard are
   each refused; the same holds after a daemon restart (point 6). An administrator may list, discard, and move
   records unread: `afs pending export --for-owner` writes an opaque,
   checksummed archive that carries each record's durable owner, AFS
   identity and cell, and `afs pending import` places those records in
   another install's store under the same owner. Both are audited. An
   administrator cannot show or export a record's bytes. This is an
   access rule, not encryption: a local administrator can read the
   daemon's store on disk, and local administrators are outside the
   threat model (O23). The archive format is specified with the record
   format (N32 to N35). Every discard is an audit event.
   The account-instance check that runs at token installation also runs
   before `afs pending list` returns anything, before any recovery record
   is shown, exported, retried or discarded, and before a context rebinds
   to one (C41, point 4; C40, point 8). The one stated residual (C41, point
   4) is uid reuse. On Linux and macOS it has two halves. The token half: a
   successor account given an earlier account's uid that acts before the
   earlier context is retired acts in that context, with its token.
   Retirement comes from whichever the host offers: a session-end signal
   (logind or the user's IronAFS agent, and only once any linger ends); a
   failed account-instance check (account UUID or equivalent) at the next
   token installation or record operation; or, for the token half, the
   token's expiry or `unlog`. Hosts with neither signal nor account UUID
   have the widest window; hosts with either signal are not affected once
   the signal has retired the earlier context. The records half: on a host
   with no account UUID (or equivalent), recovery records are owned by the
   account's uid and name, as the operating system owns that account's
   files; a later account with the same uid and name is the same owner of
   unresolved records, before or after the earlier context is retired or
   the daemon restarts, until they are resolved (stored or discarded). A
   host with an account UUID checks it before every record operation and
   every rebind, so a recreated account there gets none of them.
5. A record's owner is the durable owner of the context that wrote it
   (C18, point 9; C40, point 6). After that context ends, the same
   account instance may view or export the bytes; replaying still needs a
   current token for the same AFS identity (C28).
6. After a restart, records wait for their durable owner's instruction
   and for a current token for the same AFS identity in the context that
   carries the retry out (C42; C28, point 5), under point 4's rules: the
   owner consents after seeing the difference, and another account
   instance may carry the retry out without seeing either. A returning
   token stores nothing.
7. **On a failed or uncertain store,** the dirty data is detached from
   everything readable and the affected handles fail with the write error.
8. Budgets are per account (C27). A background store that keeps making
   progress may run past the commit deadline (C9); one that stalls fails,
   and its data stays in the journal. Stalled means no acknowledged
   progress within the stall interval, which the journal specification
   fixes.
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
3. **The connection key**, defined here once and cited by N7 and N47:
   the cell, its protocol identifier and configuration generation (N27);
   the server, the service and the transport; the security class and
   level; and the credential context with its generation. These form the
   selector, known before the handshake. Once the handshake has run, the
   suite it established (N39) is recorded with the key, and the
   connection's identity is the selector plus that suite. Status, data
   and stores have separate queues.
4. Host firewall rules for the callback port are set up as O37, point 6,
   states, never by the daemon at run time.

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
| A break naming files, from a known server's address | those files' promises are suspended and their entries marked suspect in every context (C24, point 2); each reader revalidates on its next use. Nothing is evicted |
| A break naming files, from an address matching no known server | the same, for the named files only |
| A break or reset naming no files | every promise from the servers that source could be moves to a new suspect epoch; affected entries are suspect until their reader revalidates them |
| More than the rate limit can process | the suspect epoch of the affected servers (of every server, if the source is unknown) advances **before** work is dropped or acknowledged; drops are counted |
| A probe | answered per the allowlist (point 4); never raises callback trust (C24, point 7) and is never authority over any data |

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

1. Callback trust is kept per server, address family and local endpoint
   generation. It is earned only by a reply from that server to the
   client's own keepalive call (point 4), received on the client's
   callback socket (C30), no older than **E**, and since the last network
   change or resume. The reply is matched by the keepalive call's Rx
   identity (epoch, connection id and call number), which the client
   draws from `Rng` (C3) so that an off-path sender cannot guess it. We
   propose E = 2 × the keepalive interval. Nothing the callback service
   receives raises trust (C24, point 7).
2. Without fresh evidence the server is in **bounded revalidation**: its
   data is served only if its status was validated within the last **T**
   seconds (we propose 30 to 60). Otherwise it is validated before
   delivery. If that validation cannot complete, the request fails at its
   deadline; stale data is not served.
3. With trust, cached data is served while its callback is valid and the
   credential and grants are valid, with no further cap since last
   contact. A break, or a known loss of callback continuity, ends that.
   A loss is known when a call to that server shows it: a reset of the
   client's callback state, or an error. This branch rests on assumption
   A14 (callback continuity). If the specification gate refutes A14, the
   uncapped branch does not ship: data under trust is served only within
   a revalidation cap, whose length is then a project decision, and this
   decision is reopened.
4. The client sends keepalive calls (we propose every 60 to 100 seconds)
   to servers that hold its callbacks, to keep a NAT mapping open and to
   earn point 1's evidence. Trust lapses when no reply has arrived within
   E.
5. **Resume is checked when data is served,** not only on a notification
   from the operating system. Each serve compares the clocks of C37, and a
   jump marks every callback suspect before the answer is given.
6. The state is shown in `afs status` and in the logs.

Why: a callback promise is worth nothing if the server cannot reach the
client, which is the normal case behind NAT, a firewall or after a laptop
sleeps. The client sends and receives on one socket (C30), so a reply to
its own call comes back over the same address-and-port pair a break would
use, and shows that the path through a NAT or firewall is open now. An
off-path sender cannot forge that reply without guessing the call's random
identity, while an inbound probe or break could be forged by any host that
can send UDP (C24). A NAT may drop a UDP mapping after two minutes:
[RFC 4787](https://www.rfc-editor.org/rfc/rfc4787), REQ-5, allows a
mapping timer of two minutes and recommends five or more. So the evidence
is kept to about two keepalive intervals. Keepalives already exist, so
this adds no traffic. An attacker on the path can still keep a server
trusted while dropping its breaks; that is the reach rxkad already gives
them (A13). rxgk-protected callbacks stop forged breaks and probes
(C31's trigger); they do not stop an attacker on the path dropping
breaks. What bounds a dropped break is the server's handling of a break
it could not deliver (assumption A14).

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
6. A callback that arrives from a translated address the client cannot
   match to a server (C31, point 3) is handled as one from an unknown
   address: a break still suspends the files it names. Trust comes from
   replies on the client's own flow (C32, point 1), so an unattributable
   translated source affects only whether breaks are received, not
   whether a server is trusted. The NAT64 test checks that breaks arrive
   through the translator (A11).

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
   protection against a party that holds the user's session key (C18);
   and, under IronAFS's position (O23, point 4; assumption A13), the
   integrity of a request's or a reply's payload against a network
   attacker. Write and reply integrity against a network attacker needs
   rxgk at both ends, and we know of no released OpenAFS fileserver that
   offers rxgk to clients (context, point 3). Sites that need it now protect the
   network path, or do not allow writes from untrusted networks.

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
4. **Suspend detection.** Each awake-clock sample is bracketed by two
   boot-clock reads, in 64-bit nanoseconds with checked arithmetic. If
   the boot clock grew more than the awake clock by over a threshold (we
   propose one second) after allowing for sampling skew, a suspend is
   detected. A detected suspend, a regression of a monotonic clock, or a
   jump of wall time past the threshold marks callbacks suspect (C32).
   Wall time can only lower trust. A regression of a monotonic clock, or
   a failed sample, also treats every boot-clock deadline (callbacks,
   tokens, location lifetimes, locks) as possibly expired: a lock counts
   as lost until it is established again with the server, and a token is
   checked again before its next use.
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
   wire field cannot carry is refused with the "out of range" error
   (C46, point 1); for example, a
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

1. Tokens, Rx connections, caller capabilities, grants and cache
   namespaces are keyed by one daemon-issued **credential context** id.
   What must outlive the context id (journal and recovery records, and
   the markers of N4) is keyed by its durable owner (point 6).
2. A context's local parts are what the operating system **attests** for
   a request or a connection, never what a caller asserts (C41).
3. The cell with its protocol identifier and configuration generation
   (N27), the AFS identity and the effective identity list complete the
   context's key. The security profile is not part of it: the profile
   and suite a call actually ran at (N39) key the connection (C30,
   point 3) and the cache entries it fetched (C27, point 1). The level
   recorded there is the one used, never the configured one.
4. A token is bound to the context the operating system attests for the
   connection that delivered it, never to a context named inside the
   message.
5. The daemon never reads a user's credential cache, and never prompts.
6. **The durable owner.** A context id and its generation do not survive
   a daemon restart. What must survive is keyed by the context's
   **durable owner**: the local account instance (the uid or SID, plus
   the operating system's evidence that it is the same instance of that
   account, C41, points 4 and 5), the cell with its protocol identifier and
   configuration generation (N27), and the AFS identity. It holds no credential
   generation and no security profile.
7. **Which identity keys which store:**
   - the durable owner: the authenticated-intent marker (N4, point 3),
     the reacquisition authorisation (C42, point 7), and the ownership of
     journal and recovery records (C18, point 9; C29);
   - the context id and its credential generation: tokens, caller
     capabilities, handles and grants, and, with the security profile
     and suite of the fetching connection, cache namespaces (C27,
     point 1);
   - the connection key (C30, point 3), with the suite after the
     handshake: Rx connections.
8. **After a restart** a new context takes over a durable owner's records
   and markers only on positive evidence, from the operating system, that
   its account instance is the same (C18, point 7). Without that evidence,
   requests to a cell where that owner holds a marker return
   `TokenRequired`, never anonymous data (N4, point 4), and the owner's
   records stay quarantined. They are never given to a new holder of the
   uid or SID that the host's evidence tells apart from the earlier
   account; on a host with no account UUID, uid and name are that
   evidence, and the records half of the stated residual applies (C41,
   point 4).
   Before a restart, the account-instance check that runs at token
   installation also runs before `afs pending list` returns anything,
   before any recovery record is shown, exported, retried or discarded, and
   before a context rebinds to one (C41, point 4). The one residual (C41,
   point 4) is uid reuse. On Linux and macOS it has two halves. The token
   half: a successor account given an earlier account's uid that acts
   before the earlier context is retired acts in that context, with its
   token. Retirement comes from whichever the host offers: a session-end
   signal (logind or the user's IronAFS agent, and only once any linger
   ends); a failed account-instance check (account UUID or equivalent) at
   the next token installation or record operation; or, for the token half,
   the token's expiry or `unlog`. Hosts with neither signal nor account
   UUID have the widest window; hosts with either signal are not affected
   once the signal has retired the earlier context. The records half: on a
   host with no account UUID (or equivalent), recovery records are owned by
   the account's uid and name, as the operating system owns that account's
   files; a later account with the same uid and name is the same owner of
   unresolved records, before or after the earlier context is retired or
   the daemon restarts, until they are resolved (stored or discarded). A
   host with an account UUID checks it before every record operation and
   every rebind, so a recreated account there gets none of them.

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
4. **When a context ends on Linux and macOS.** A context ends when the
   operating system reports that the uid's last session has ended
   (systemd-logind's `UserRemoved` signal, where logind runs), or when the
   user's IronAFS agent exits at logout (the systemd user manager on
   Linux, the launch agent of L14, point 3, on macOS), or when a token
   install shows a different account instance for the uid. When a token is
   installed the daemon records the uid with its user name, and the
   account's UUID where the operating system has one; a mismatch at the
   next install retires the old generation (C18, point 7) and treats the
   records of the old owner as C40, point 8, says. **Records.** The
   account-instance check that runs at token installation also runs before
   `afs pending list` returns anything, before any recovery record is
   shown, exported, retried or discarded, and before a context rebinds to
   one; a mismatch retires the old generation, refuses the request and
   treats the records of the old owner as C40, point 8, says. A user whose
   services linger keeps the context until the linger ends, and the signal
   retires it only then. Hosts with neither signal rely on token expiry and
   `unlog`; this is documented. **Bound.** An account deleted and recreated
   with the same uid and name, on a system with no account UUID, is
   indistinguishable from the old one. The one residual is uid reuse. On
   Linux and macOS it has two halves. The token half: a successor account
   given an earlier account's uid that acts before the earlier context is
   retired acts in that context, with its token. Retirement comes from
   whichever the host offers: a session-end signal (logind or the user's
   IronAFS agent, and only once any linger ends); a failed account-instance
   check (account UUID or equivalent) at the next token installation or
   record operation; or, for the token half, the token's expiry or `unlog`.
   Hosts with neither signal nor account UUID have the widest window; hosts
   with either signal are not affected once the signal has retired the
   earlier context. The records half: on a host with no account UUID (or
   equivalent), recovery records are owned by the account's uid and name,
   as the operating system owns that account's files; a later account with
   the same uid and name is the same owner of unresolved records, before or
   after the earlier context is retired or the daemon restarts, until they
   are resolved (stored or discarded). A host with an account UUID checks
   it before every record operation and every rebind, so a recreated
   account there gets none of them. C29, point 4, C40, point 8, O22, point
   1, O23's local-user row, O46, point 1, and O48, point 4, state the same
   residual. After a restart, records stay quarantined from any account
   whose instance differs from their owner's (C40, point 8).
5. **Windows.** The account instance is the account's SID, which is not
   reissued to another account. A context ends when its logon session
   ends (L18, point 12). After a restart the SID is C40, point 8's
   positive evidence.
6. **Gate.** The account-instance evidence of points 4 and 5 is confirmed
   on each supported release of each operating system (L13, L22) before
   the release that rebinds records after a restart. Where it is not
   confirmed, records stay quarantined after a restart (C40, point 8).

Why: a PAG needs a per-session marker the daemon can trust. Point 4 uses
only operating-system interfaces: logind's manager sends its user and
session signals when a user logs in or out, and documents lingering
([org.freedesktop.login1](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html)).
A logout hook alone would depend on every site configuring one. The user
name check is cheap and catches the common case of an account removed and
a new one created with the same uid; Linux has no portable account
instance id beyond it. A FUSE request
carries a uid, gid and pid `[DES-022]` and FSKit's context carries user and
group ids only `[DRV-025]`; neither carries a session. OpenAFS tracks PAGs
using artificial groups on some platforms `[DRV-075]`; the FUSE kernel
module passes no group list `[DRV-065]`, so even that marker would not reach
the daemon.

### C42. Tokens live in the daemon's memory

**Working**, except points 4 and 7, which are **Firm** (a security
invariant). Revisit if: a site asks for tokens to survive a restart and a
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
4. Sign-out (C41, point 4, on Linux and macOS; point 5 on Windows), or a deliberate removal
   of a token, ends that authorisation. A restart cannot undo it.
5. A context that cannot get its token back shows "no token" in
   `afs status`, with a notification. A context with no agent must run
   `aklog` itself. This is documented.
6. Renewal and revocation follow C14.
7. **Reacquisition authorisation.** "Still authorised" in point 3 is a
   durable state the daemon keeps per durable owner and cell (C40,
   point 6), stored beside the authenticated-intent marker (N4) but
   separate from it. It holds no key material. Its states are **none**,
   **authorised**, **withdrawn-by-user** and **blocked-by-administrator**:

   | Event | From | To |
   |---|---|---|
   | The user's explicit `aklog`, or the login service's token step at session start | none, withdrawn-by-user | authorised |
   | The same, while blocked | blocked-by-administrator | blocked-by-administrator (the session gets its token; automatic reacquisition stays off) |
   | `unlog` | none, authorised, withdrawn-by-user | withdrawn-by-user |
   | `unlog` | blocked-by-administrator | blocked-by-administrator |
   | An administrator's revocation (N4, point 6) | any | blocked-by-administrator |
   | An administrator's audited restore | blocked-by-administrator | authorised |
   | Sign-out (point 4) | authorised, withdrawn-by-user | none |
   | Sign-out | blocked-by-administrator | blocked-by-administrator |
   | Token expiry | any | unchanged |

   - Every change is committed durably before the operation that caused
     it is acknowledged. Agent requests in flight at that moment are
     fenced: a token they deliver is refused.
   - After a restart, the daemon accepts a token from an agent's
     automatic `aklog` only in the authorised state.
   - **Per durable owner.** Reacquisition authorisation belongs to the
     durable owner, not to one logon. An `unlog` or an administrator's
     revocation in one logon withdraws it for every logon of that owner:
     after a restart neither logon's agent reinstalls a token, and each
     gets `TokenRequired`, never anonymous data. This is documented.
   - **Tests:** revoke a token, restart the daemon: the agent's automatic
     `aklog` does not install a token. An administrator revokes, the user
     unlogs, the user runs `aklog`, the daemon restarts: no automatic
     reinstall. Two Windows logons of one account with one AFS identity:
     one revocation by an administrator, then a restart: no automatic
     reinstall in either.

Why: a token on disk is a secret that backups, images and other
administrators can reach. Having the user's own process build it means the
daemon never needs access to anyone's Kerberos credentials (C40).

### C43. A token sender authenticates the daemon first

**Firm.**

1. No credential material is read or sent until the sender has shown that
   the endpoint belongs to the daemon.
2. The check has two parts on every operating system: the endpoint's
   owner is the daemon's service identity, as the operating system
   reports it (point 4), and the handshake's instance id (C44) is bound
   to that checked connection. A failed check aborts before anything
   secret moves.
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
   - **macOS:** as on Linux. The privileged install or launch step
     creates the socket's directory, owned by the daemon's dedicated user
     (L14) and not writable by others. The daemon creates the socket
     itself. The sender checks that the peer's uid, from
     `LOCAL_PEERCRED`
     ([`unix(4)`](https://man.freebsd.org/cgi/man.cgi?query=unix&sektion=4&manpath=macOS+26.7)),
     is the daemon's user, as another macOS project does for its own
     socket `[MAC-057]`.
   - **Windows:** the daemon creates its pipe as the first instance,
     refusing remote clients
     ([`CreateNamedPipe`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipew)),
     with an access-control list that lets only its service identity
     create instances. Services can run under a virtual account with its
     own SID `[DES-074]`. The sender finds the pipe server's process
     ([`GetNamedPipeServerProcessId`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeserverprocessid))
     and checks that the process's token carries the service's SID before
     sending. A process id alone proves nothing. The check is made again
     on every new connection, so it is repeated after a daemon restart.

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
   A binding without one is refused. A per-user FSKit mount's binding is
   registered for one owner uid and that user's credential context (L9,
   L10). The daemon refuses any operation on it whose FSKit context names
   a real or effective uid other than the owner's, uid 0 included (C8,
   point 3). Items the binding hands out stay bound to the credential
   generation they were served under until the operating system has
   released every reference to them.
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
   interrupted, not supported, would block, unreachable, conflict, and
   out of range (C38, point 3). The token errors and `SecurityFloor`
   reach applications as a permission error, with the cause in the side
   channel (N4, N6, O35). Out of range reaches applications as an
   invalid-argument error (`EINVAL`, an `NSError` with POSIX `EINVAL`, or
   `STATUS_INVALID_PARAMETER`), with the cause in the side channel.
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
   - A rename over a file that is open on the same machine replaces it.
     The renaming user's handles on the replaced file fail with `Stale`,
     as when the file is replaced from another machine; other users'
     handles fail once their own revalidation shows the file replaced.
     Unstored bytes wait as recovery records of their owners (C10).
   - Where a kernel cannot isolate mapped pages, programs cannot be run
     from `/afs`, and the row qualifies only where a mechanism that
     refuses the affected mappings is demonstrated (C15, point 5).
   - Without a recent reply from a server to the client's own keepalive
     call, that server's cached data is revalidated every T seconds, and a
     request fails instead of returning stale data (C32).
   - On a Linux host with only a flat list of name servers, DNS discovery
     of cells is off until an administrator confirms it (C33).
2. **Cost on multi-user hosts.** Until fileservers offer rxgk to clients,
   every active credential context fetches and stores its own copy of what
   it reads, and a new login starts cold. The budgets of C27 bound it;
   they do not remove it.
3. **What cannot be promised.** Pages resident in a live mapping stay
   readable after revocation until the view is unmapped, on Windows too:
   closing the SMB open or ending the session stops later faults, not
   resident pages (C15, points 2 and 8). A failed purge affects one file:
   every context holding it gets `Stale` and reopens; on Linux that node
   also refuses new mappings until a purge succeeds (C15, point 6). On
   SMB, a front end that cannot tell the client to rescan ends the
   session (C17, point 5). On
   macOS, if revoking an item fails after a failed purge, that user's
   whole mount ends (C15, point 7). An attacker on the network path can
   keep a server trusted while dropping its breaks; this is the same
   reach rxkad already gives them (A13). rxgk-protected callbacks stop
   forged breaks and probes; they do not stop an attacker on the path
   dropping breaks. What bounds a dropped break is the server's handling
   of a break it could not deliver (assumption A14; C24, point 7;
   C32). Root and the daemon's own
   account see everything in the cache. A change of the client's address behind a NAT
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
- **Callback trust earned from inbound probes or breaks:** before rxgk
  nothing the callback service receives is authenticated, so a forged
  probe every few minutes could keep a server trusted while its real
  breaks were lost (C24, point 7; C32).
- **Bounded revalidation for every server until callbacks are
  rxgk-protected:** it would add a status round trip for every cached file
  every T seconds, on every server, for no gain against an attacker on the
  path, who can already tamper with rxkad replies (A13; C32).
- **Refusing a rename over a file open on this machine:** every save by
  rename would fail while any local process held the file open, and the
  refusal would tell other users that the file is open (C10, point 5).
- **Aborting the whole FUSE connection when one purge fails:** a forged
  break could then end every user's open files (C15, point 6).
- **The time and randomness seam inside `ironafs-vfs` or
  `ironafs-client`:** `ironafs-rx` knows nothing of AFS and so cannot
  depend on `ironafs-vfs`, and pure crates may not depend on
  `ironafs-client` (C4, points 1, 4 and 7).
