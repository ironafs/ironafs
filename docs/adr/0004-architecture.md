<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0004: The architecture

Status: accepted (2026-11-20), by the owner, after the review rounds
recorded below. Raised in issue #20. Amended 2026-11-20 (#32).

## Context

### What IronAFS is

IronAFS is a new AFS client for existing AFS-3 cells, written in Rust. It is
one shared core with a thin layer per operating system (Linux, macOS and
Windows) and one `afs` command line. It has no kernel code of its own and
uses no third-party driver: each layer uses only what its operating system
ships. OpenAFS 1.8 servers are the first test target. Nothing is built or
tested yet; this ADR decides what is built.

It aims first at the behaviour users of an AFS client already rely on
(`/afs`, tokens and `aklog`, the `fs` commands, mount points, ACLs), and then
at what a client written today can add. It is meant to be run by central IT
across many machines.

### Goals

1. **Correct authorisation per user on every operating system.** One local
   user never reads data with another user's AFS rights.
2. **Correct cache coherency.** When a fileserver breaks a callback, the
   operating system drops what it cached.
3. **One core, tested once.** Time, randomness, the network and the disk sit
   behind seams, so one deterministic simulator tests the code every layer
   shares.
4. **Operable across a fleet.** Native logs with stable event ids, audit,
   health, metrics, diagnostics and central configuration on every
   operating system.
5. **Few dependencies, all readable** ([coding standards](../coding-standards.md)
   rule 21).

### Non-goals in this phase

- Server components, and administration tools beyond those users run (`fs`,
  `tokens`, `aklog`, `unlog`; `pts` lookups later). A possible server is
  tracked in issue #22.
- DFS, an NFS translator, or a gateway for other machines. The Windows SMB
  front end (part 2) serves only its own machine, on loopback.
- Writes while disconnected. A read-only offline mode is later, optional
  work (part 3).
- Android, iOS and the BSDs. They may come later behind the same contract.
- Telemetry of any kind, and update checks.
- Set-user-id and set-group-id programs run from AFS, and device files
  (part 2).

### Anti-goals

Each of these is a failure even if the client otherwise works:

- a cross-user data leak, through an operating system's cache or through
  the client's own;
- local privilege escalation through `/afs`;
- a silent wrong answer: stale data served as fresh, a partial listing
  shown as complete, a lost write reported as saved, or a write stored
  later, without the user's action, over a version of the file the client
  has not seen (C28);
- a hang that needs a reboot;
- code copied, ported or translated from another AFS implementation
  ([ADR 0001](0001-licence-and-dont-copy.md));
- a secret in a log, a dump or a support bundle.

### Constraints

1. **No kernel code and no third-party driver.** Third-party kernel
   extensions on Apple silicon need Reduced Security `[DRV-001]`, and the
   interfaces this ADR uses instead are the ones each system ships: FUSE on
   Linux `[DRV-059]`, FSKit on macOS `[DRV-002]` and Windows' own SMB client
   (part 2).
2. **`unsafe` only in named binding crates,** each with its own ADR
   (`AGENTS.md`; coding standards rule 5).
3. **Dependencies:** IronAFS's own code first. A crate is used only for
   mainstream cryptography and for an operating system's own interfaces,
   each by a separate decision (coding standards rule 21).
4. **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
   local IPC, configuration, requests from the operating system, and SMB
   messages before authentication. No panics, checked arithmetic and byte
   limits apply to all of it (coding standards rules 1 to 3).
5. **Sources.** IronAFS is written from published specifications and
   observed behaviour, never from another implementation's code
   (ADR 0001). The available documents are uneven:
   - the community Rx specification draft describes Rx but no security
     class `[DRV-084]`;
   - rxgk rests on two expired Internet-Drafts `[DRV-085]` `[DRV-086]`. The
     base draft has experimental status in the AFS-3 standardisation group
     `[DES-M05]` `[DRV-087]`;
   - existing cells' clients use rxkad with Kerberos 5 `[DRV-089]`, and no
     public specification of rxkad has been found
     ([driverless-clients note](../research/2026-11-18-driverless-afs-clients.md),
     section 8);
   - Microsoft publishes SMB versions 2 and 3 as an Open Specifications
     document
     ([MS-SMB2](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-smb2/5606ad47-5ee0-437a-817e-70c366052962));
     part 2 cites the rest.

### What forces a decision now

- ADR 0001 fixed the licence and the clean-room rule, and left the
  architecture to this ADR: Rx in Rust, IronAFS's own interface
  definitions, no C Rx through FFI.
- The research is done and checked. The notes in
  [docs/research](../research/README.md) were re-checked claim by claim in
  [the claim record](../reviews/2026-11-20-adr-0004-claims.md).
- The platforms moved. FSKit's caller context, which carries the caller's
  user and group ids, exists only from macOS 27 `[DRV-025]` `[DRV-026]`.
  OpenAFS 1.9.0's release notes list no rxgk between a client and a
  fileserver `[DES-048]`, and the 1.9.x notes say client tokens are still
  in progress `[DRV-M25]`. Both change what a new client can promise.
- No Rust crate is written before this ADR is accepted (`AGENTS.md`: a new
  phase starts with an ADR or a specification, not code).

## How this ADR is organised

ADR 0004 is one decision record in five files. This file is the overview.
The decisions are in four parts:

| Part | File | Ids | Covers |
|---|---|---|---|
| 1 | [Core](0004-architecture-1-core.md) | C1, C2, … | the daemon, the contract between core and layers, node identity, handles, invalidation, the cache, the write journal's behaviour, the network, time, threads, identity and tokens, local IPC, errors, crates |
| 2 | [OS layers and Windows](0004-architecture-2-os-layers.md) | L1, L2, … | Linux, macOS and Windows layers, presentation shared by all layers, names, mounting and namespace, the platform matrix |
| 3 | [Operations, security and lifecycle](0004-architecture-3-operations.md) | O1, O2, … | events, audit, metrics, diagnostics, configuration, the threat model, hardening, upgrades, packaging, testing, phasing |
| 4 | [Negotiation and formats](0004-architecture-4-negotiation.md) | N1, N2, … | protocol negotiation and downgrade protection, what the client holds ready for later servers, long-lived formats, identifier widths |

1. **Each decision has an id** with its part's prefix (C, L, O or N). Parts
   cite each other by id, for example "see C12". Ids are never reused or
   renumbered once this ADR is accepted.
2. **Each decision is marked Firm or Working.**
   - **Firm** decisions are the ones a later change would break data,
     interoperability or a security promise: wire and on-disk formats,
     security invariants, and the clean-room rule. A firm decision changes
     only by a superseding ADR
     ([docs/adr/README.md](README.md), "Changing a decision").
   - **Working** decisions are the best answer on today's evidence. Each
     names what would make us look again: a version floor that moves, a
     measurement, an API that ships, or a named assumption that fails. A
     working decision changes by an amendment pull request that brings the
     new evidence and is reviewed as
     [docs/adr/README.md](README.md) sets out (for this ADR, on the
     strongest row of [docs/workflow.md](../workflow.md) §2). That route opens once
     `docs/adr/README.md` allows amendments; until then a working decision
     changes as a firm one does.
3. **Milestone names** (M2a, M2b, M3 to M7) are defined in part 3. They are
   a plan, not a promise.
4. **"Gate"** means a test or a specification that must pass before the
   work behind it starts or ships. Each gate is stated where it applies.
5. **The Review section below covers all four parts.** Part files have no
   Review section of their own.

## Named assumptions

Some things this ADR rests on cannot be shown from a public source today.
Each is named here, with the claims it concerns, what depends on it and
what would settle it. The parts cite them as A1 to A14. An assumption that
fails reopens every working decision that names it.

- **A1. The RHEL 7.9 artifact.** We assume a client built against glibc 2.17
  runs on RHEL 7.9's 3.10.0-1160 kernel `[PLT-015]` at FUSE protocol 7.23
  `[PLT-067]` `[PLT-M04]` `[PLT-M05]`. No Red Hat document states the FUSE
  level, and no public record exists of a run on that kernel.
  - Depends on it: the RHEL 7.9 candidate row of the platform matrix and
    the static-musl candidate (part 2).
  - Settled by: a named RHEL 7.9 guest with current ELS updates passing the
    common smoke test, with the negotiated FUSE flags recorded.
- **A2. Demand for EL7.** We assume enough AFS sites still run EL7 to
  justify a tier 2 candidate row. No public count exists `[PLT-066]`.
  - Depends on it: only the RHEL 7.9 row.
  - Settled by: a request for the row from a site that runs AFS on EL7.
    Without one, the row is not qualified and lapses at its cutoff
    (L39, point 3); no earlier cutoff is set.
- **A3. The Windows SMB routes.** We assume Windows' own SMB client,
  reaching IronAFS's loopback server through route 3 (and route 1 on
  Windows 11 24H2 and later `[WIN-162]`), completes an authenticated, signed SMB 3.1.1
  session with the user's identity intact. The public record covers only
  the client's first packet reaching a listener `[PLT-M08]`
  ([UNC note](../research/2026-11-19-windows-unc-without-a-driver.md)).
  - Depends on it: the whole Windows SMB surface and its rows in the
    platform matrix (part 2).
  - Settled by: the route qualification in the platform matrix, run and
    recorded in public on the named Windows builds, early in M2.
- **A4. Protecting the back-end port.** We assume a Windows Filtering
  Platform rule can stop other local processes from taking route 3's
  back-end port while IronAFS is stopped `[WIN-189]`, and the same for
  route 1's alternative port, and that such a rule persists across a
  reboot and is in force before any local process can bind the port. A
  persistent port reservation cannot do this: Windows gives such
  reservations no security between components `[WIN-187]`.
  - Depends on it: the fail-closed admission criterion for routes 1 and 3
    (part 2, L20).
  - Settled by: a test in M2 in which an ordinary local user tries to bind
    each route's port while IronAFS is stopped, and again early in boot,
    and the containment test of L41, point 9.
- **A5. Identity through Windows' own file server and NFS client.** We
  assume that a local folder shared through Windows' own SMB server loses
  the caller's identity before it reaches the folder's provider, and that
  Windows' NFS client in its usual configuration presents one identity for
  every user. No public measurement of either exists.
  - Depends on it: the rejection of those two routes (part 2).
  - Settled by: a public spike run of each. If either keeps the caller,
    that route is reconsidered.
- **A6. How Cloud Files behaves where Microsoft does not document it.** We
  assume the Cloud Files platform behaves in ways a replica can be built
  on, in areas its documentation leaves open: what happens when the
  process holding a sync root's connection fails, hydration while antivirus
  software scans, in-sync marking, vetoes of delete and rename, Explorer
  refresh, removal of a root, and drift of NTFS access-control lists.
  - Depends on it: the replica ADR that must be accepted before the
    replica is built (L27, point 11), which specifies its mechanisms, and
    when the replica is scheduled (part 2). This ADR fixes only the
    replica's invariants (L27, L28).
  - Settled by: public measurement on the named Windows builds, recorded
    in that replica ADR, before the replica is built. Until then this ADR
    cites only Microsoft's documentation for the replica.
  - Two documented facts belong beside it: hard links on placeholders are
    refused by default `[WIN-M04]`, and Microsoft's two pages disagree on
    whether the partial hydration policy is supported `[WIN-M44]`.
- **A7. Identity and lifecycle in a Cloud Files replica.** We assume a
  replica identifies callers per Windows user and per root `[WIN-059]`.
  Microsoft documents a session id in callbacks and no user or token
  `[DRV-019]`. We also assume that logoff needs handling apart from a service
  stop `[WIN-128]`, and that keeping the cache within a size is the
  provider's job, with no automatic dehydration `[WIN-085]`. Finally we
  assume a logon SID is the same across the linked elevated and limited
  tokens of one logon; Microsoft's documentation does not say.
  - Depends on it: how the replica ADR (L27, point 11) binds a root to
    one logon session and meets L28, point 1's isolation, and its
    cache-size policy.
  - Settled by: a spike, recorded in the replica ADR, that logs callback
    session ids across two users, two logons of one account, an elevated
    and a limited process of one logon, a logoff and a run under disk
    pressure.
- **A8. The order of the bulk status calls.** We assume an AFS-3 fileserver
  accepts InlineBulkStatus after a directory listing, and that BulkStatus
  is the fallback when a server refuses it. Both RPCs exist; their
  order of use is our assumption `[DES-076]`.
  - Depends on it: listing performance (C36).
  - Settled by: a packet capture of a listing against the OpenAFS test
    cell, in M2.
- **A9. No conditional store in AFS-3.** We assume AFS-3 offers no store
  that is conditional on the server's current version of a file. So a
  client cannot atomically refuse to overwrite a version it has not seen
  `[WIN-098]`.
  - Depends on it: the rule that deferred stores are never automatic
    (C28), the replica's rule of no automatic upload (L27, point 3), and
    the case for a
    conditional store in a later server profile (issue #22).
  - Settled by: a published AFS-3 file-service specification, or an
    edition of the older AFS documents registered under N31, point 3,
    whose complete file-service call list shows no conditional store,
    corroborated as N31, point 3, requires.
- **A10. Where File Provider puts a domain.** We assume File Provider
  cannot place a domain at a fixed path such as `/afs`, and puts roots
  under the user's Library `[DRV-M29]` `[DRV-043]` `[MAC-M17]` `[MAC-M18]`. The
  evidence is an absence in Apple's documentation and forum posts.
  - Depends on it: the rejection of File Provider for `/afs` (part 2).
  - Settled by: a statement from Apple, or a test that registers a domain
    on macOS 27.
- **A11. IPv6-only networks.** We assume the client reaches IPv4-only
  fileservers, and receives their callbacks, through NAT64 and 464XLAT on
  an IPv6-only network. RFC 6877 specifies 464XLAT; the client's use
  of it, and the callback path through it, are untested `[DES-067]`.
  - Depends on it: the IPv6 statement in C34. Trust comes from replies on
    the client's own flow (C32, point 1), so a translated source that
    cannot be matched to its server affects only whether its breaks are
    received (C34, point 6).
  - Settled by: the NAT64 test in M2, including the callback path.
- **A12. Fuzz targets in the workspace.** We assume a `fuzz` directory per
  crate does not work under a `crates/*` virtual workspace without an
  exclude, and that a top-level `fuzz` directory outside the workspace does
  `[LAY-039]` `[LAY-040]`. No primary source states either.
  - Depends on it: where fuzz targets live
    ([project-layout note](../research/2026-11-18-project-layout-and-tests.md)).
  - Settled by: running `cargo metadata` and `cargo fuzz` on the pinned
    toolchain when the first fuzz target lands, with the output recorded
    in the pull request.
- **A13. rxkad and the integrity of a reply or a request.** We assume no
  rxkad level protects the integrity of a reply's payload, and IronAFS
  takes the same position on a request's payload, a store's data
  included: a network attacker may be able to change it. The claim record
  holds no public source that states, level by level, what rxkad
  protects. The rxgk base draft describes rxkad as limited to very weak
  cryptography, approximately single-DES `[ADR-034]`.
  - Depends on it: IronAFS's stated position on rxkad (O23, point 4, and
    its rxkad row); what rxkad's documentation says it does not give
    (C35, point 5); what counts as evidence in negotiation (N1, N5); and
    the rule that no rxkad fetch enters the shared pool (C18, C21).
  - Settled by: the specification route for rxkad (O50, point 3), which
    confirms or corrects it.
- **A14. Callback continuity.** We assume an AFS-3 fileserver that cannot
  deliver a break stops treating this client as holding that callback,
  and that the client learns of the loss no later than its next call to
  that server, through a reset of its callback state or an error. No
  public source in the claim record states this.
  - Depends on it: the branch of C32, point 3, that serves cached data
    under callback trust with no further cap; and O34, point 1, which
    serves offline under that trust. An on-path attacker who drops breaks
    is a separate, stated residual (C32, Why); rxgk-protected callbacks
    stop forged breaks, not dropped ones.
  - Settled by: the specification for the callback service (C6), and a
    test against the test cell in which every retransmission of a break
    is dropped while keepalives succeed. If either refutes A14, the
    uncapped branch does not ship: data under trust is served only within
    a revalidation cap, and C32 is reopened.

## Open questions for review

On this point the parts do not yet give one answer, or give one whose cost
the review should weigh. The entry names the decisions involved and the
options. It is not answered here.

1. **DNS discovery on a flat list of name servers** (C33, point 4). Is
   an administrator's confirmation, with the listed servers asked in
   order under documented timeouts, enough to keep IronAFS's own DNS
   client from going around the host's choice of server? Options:
   - as C33 states;
   - no discovery on a flat list: configured servers only.

## Evidence

1. **Facts are cited by claim id.** A tag such as `[DES-076]` names a row
   of [the claim record](../reviews/2026-11-20-adr-0004-claims.md): one
   fact, the primary source it was checked against, the verdict and the
   date. The prefixes are PLT (platform floors), DES (design facts), MAC
   (macOS), DRV (driverless clients), LAY (layout and tests), WIN
   (Windows) and ADR (facts read while this ADR was drafted).
2. **Verdict labels.**
   - CONFIRMED: the source states the claim.
   - AMEND: the claim is partly right. This ADR uses the corrected form
     the record gives.
   - UNVERIFIED: no reachable public source settles the claim. This ADR
     rests on such a claim only through a named assumption (A1 to A14),
     never directly.
   - UNVERIFIED-PRIVATE: the claim rests on a measurement whose record is
     not public. This ADR does not cite it.
   - REFUTED: none was found.
3. **A fact with no row in the record carries a public link** where it is
   stated.
4. **Everything else is a proposal.** A sentence that says "we propose",
   "planned" or "the specification for …" describes work to be done, not a
   fact. No decision in this ADR has been executed: in the terms of
   [docs/workflow.md](../workflow.md) §3c, the cited facts are INSPECTED
   and the decisions are PROPOSED.
5. **Old evidence keeps its date.** A decision is re-checked against its
   sources before the task that builds it starts. A source that changed is
   new evidence under "Working" above.

## Decision index

Each decision's mark is in its part.

**[Part 1, the core](0004-architecture-1-core.md)**

- C1: One system daemon per machine holds the core
- C2: Semantics in the core, presentation in the layer
- C3: Protocol code without I/O, synchronous operation code, no async runtime
- C4: Crates and their dependency rules
- C5: The cryptography boundary and the operating-system bindings
- C6: The protocol code is written from specifications
- C7: The contract crate and its three kinds of operation
- C8: Caller capabilities come from the core, from what the OS attests
- C9: One commit operation
- C10: Removing and replacing open files
- C11: Node identity
- C12: The dynamic root
- C13: Handles carry authority
- C14: Suspension, resumption and revocation
- C15: What revocation guarantees, by access path
- C16: Fencing: record first, never serve a fenced reply, check at submission
- C17: The invalidation stream
- C18: Cache state is private to each credential context
- C19: Nothing unauthenticated feeds an authenticated path
- C20: Unauthenticated data is not cached at all
- C21: The shared pool admits only data no local user could have forged
- C22: Grants: per file, from the reader's own status reply
- C23: How long a grant lasts
- C24: Weak evidence only lowers trust
- C25: Volume incarnation: when in doubt, fetch again
- C26: Coherency and what survives a restart
- C27: The store on disk, eviction and budgets
- C28: What the journal never does
- C29: Journal behaviour and recovery
- C30: One socket per address family; connections per context
- C31: What the callback service does with what it receives
- C32: Callback trust and bounded revalidation
- C33: Finding a cell's servers
- C34: Addresses and IPv6
- C35: Security classes and their order
- C36: Performance choices made at the start
- C37: Three clocks behind one seam
- C38: Time is 64-bit inside; wire times are converted in one place
- C39: Threads and locking
- C40: One credential context, built from what the OS attests
- C41: What each operating system attests; no PAGs
- C42: Tokens live in the daemon's memory
- C43: A token sender authenticates the daemon first
- C44: Local IPC: one framework, its encoding and compatibility rules
- C45: Two endpoints; adapters bind only by the daemon's registration
- C46: Errors

**[Part 2, OS layers and Windows](0004-architecture-2-os-layers.md)**

- L1: A layer is a translator and holds no authority
- L2: Linux uses FUSE through IronAFS's own protocol layer, floor 7.23
- L3: `/afs` never grants local privilege on a POSIX layer
- L4: Every write on Linux is attributable to a caller
- L5: Linux kernel caches start off and relax only with evidence
- L6: Commit points on Linux
- L7: Linux hosting: a small privileged mount step, an unprivileged daemon
- L8: macOS uses FSKit on macOS 27 and later, Apple silicon only
- L9: A mount whose reads and writes name no caller serves one user
- L10: Per-user FSKit mounts by default
- L11: macOS mount and cache settings start conservative
- L12: macOS writes need a commit that reports errors
- L13: The macOS gate
- L14: macOS hosting
- L15: Windows is SMB first; the Cloud Files replica is a later companion
- L16: Routes from Windows' SMB client to the loopback server
- L17: Over SMB, authority is the session's own account
- L18: SMB authentication runs in the daemon; the front end holds transport keys and session capabilities, no AFS tokens and no cache
- L19: SMB scope
- L20: The loopback endpoint is contained, or the route is off
- L21: Machine-wide Windows settings each have one owner
- L22: What the Windows spike must show
- L23: Windows hosting
- L24: The Windows name table
- L25: Windows names are total, reversible and never destructive
- L26: Windows metadata
- L27: The Cloud Files replica, if it is built
- L28: Replica isolation is per object and absolute
- L29: Names on Linux and macOS
- L30: A displayed owner never grants anything
- L31: Owner mapping
- L32: Mount points, links and special nodes
- L33: Locks, per layer
- L34: Where the filesystem appears
- L35: `fs` command paths are resolved by the caller, not the daemon
- L36: What a row means
- L37: Tier 1 targets
- L38: Tier 2 targets
- L39: Candidates
- L40: The common smoke test and the test cell
- L41: Windows route qualification
- L42: Build, mount and authentication floors
- L43: Spikes and exclusions

**[Part 3, operations, security and lifecycle](0004-architecture-3-operations.md)**

- O1: One event catalogue for every component
- O2: A published event keeps its id and meaning for good
- O3: Components report through the daemon, which stamps the source
- O4: A direct write when the daemon cannot take an event
- O5: No secret in a log, a capture or a support bundle
- O6: Default redaction and opaque references
- O7: Storm control, run-time levels and scoped captures
- O8: Native log sinks
- O9: Audit comes only from the daemon, and its loss is always visible
- O10: Audit capacity, durability and collection
- O11: Metrics and health, with no listener by default
- O12: `afs status` and `afs diagnose`
- O13: Cache inspection and flush
- O14: Support bundles
- O15: No remote debug service
- O16: A user sees only their own state
- O17: Layered configuration, and policy locks its keys
- O18: The configuration file
- O19: Policy across client versions
- O20: Nothing phones home
- O21: Retention, and what the client tells others by working
- O22: The security invariants
- O23: The threat model
- O24: Least privilege for every process
- O25: No secret in a dump
- O26: How dumps are disabled, and what stays outside the client's control
- O27: Encryption at rest is the operating system's
- O28: Constant-time comparison and zeroisation at named places
- O29: Supply chain and release signing
- O30: Vulnerability handling
- O31: FIPS 140 mode is unscheduled
- O32: Pending writes and durable state survive an upgrade
- O33: The upgrade and rollback procedure
- O34: Offline behaviour
- O35: Errors a user can act on
- O36: Tokens at login, and the command line
- O37: Packaging and installers
- O38: Identifiers: one inventory, and when they freeze
- O39: Servers supported, and how compatibility is decided
- O40: Protocol details that users notice
- O41: Where the cache and the journal may live
- O42: Backups, images and clones
- O43: Release and support policy
- O44: Generated troubleshooting reference and runbooks
- O45: A deterministic whole-system simulator
- O46: The acceptance rule
- O47: Model-based, property and fuzz tests
- O48: Interoperability and acceptance cases
- O49: Benchmarks and CI venues
- O50: The order of work
- O51: Proposed later work

**[Part 4, negotiation and formats](0004-architecture-4-negotiation.md)**

- N1: Profiles, and their order
- N2: Three controls, and the effective floor
- N3: The configured floor: keys and defaults
- N4: The token's own profile, and no anonymous fallback
- N5: Evidence and hints
- N6: Failure never lowers
- N7: One choke point
- N8: Security keys are understood from the first authenticating release
- N9: The learned pin: scope and learning
- N10: The learned pin: lifetime
- N11: The pin store
- N12: The one downgrade signal: a cell-attested legacy fileserver
- N13: What ships disabled: the opcode gate
- N14: First contact
- N15: Replicas of mixed profile
- N16: The administrator's view
- N17: Negotiation never fetches code
- N18: FIPS mode is this mechanism with fixed values
- N19: Capability observations are never persisted or shared
- N20: Fallbacks come only from a static catalogue
- N21: What the client announces
- N22: IronAFS extensions only over rxgk
- N23: Third-party optional operations
- N24: The registration gate
- N25: rxgk waits for a peer; what may be claimed
- N26: The client is an AFS-3 client; readiness costs nothing
- N27: A protocol identifier per cell
- N28: Operation grants, not rights letters
- N29: Callback kinds form a registry
- N30: Token and combine calls sit behind the profile
- N31: Evidence for any wire fact
- N32: Journal records: two layers
- N33: Journal versions, unknown records and quarantine
- N34: Journal kinds
- N35: Reserved areas in a record, and what is not reserved
- N36: The principal is a typed value
- N37: The ACL value is open
- N38: Identifier widths
- N39: A security profile is recorded as class, level and suite
- N40: No suite floor yet
- N41: Extension arms in control IPC
- N42: Per-volume capabilities
- N43: No permanent refusal for what a later cell may offer
- N44: Seams: data transform and crypto agility
- N45: Rekeying belongs to the security class
- N46: Reserved contract operations, and two baselines
- N47: Small fields that are cheap now

## Specification items

These are left to the named specifications. Each must keep the stated
invariant.

1. **S1.** The specification for the adapter endpoint (C44, C45) settles
   the order of an out-of-process revocation's events (the daemon records
   it, the front end applies the advance, a write starts, the binding
   acknowledges or is torn down) and how the tests observe each. It must
   keep C16, points 1, 3 and 6, and C31, point 1.
2. **S2.** The specification for the SMB front end (L19, point 11)
   settles which responses are framed, signed or encrypted as one unit,
   and how a unit is completed when one reply in it is fenced. It must
   keep C16, point 6: every reply in a unit is checked before its first
   byte, and no part of a protected unit is followed by an error.
3. **S3.** The specification for the macOS layer settles how an item
   keeps whether it was reached through the lookup fallback, across item
   reuse, relative operations and access upgrades. It must keep L29,
   points 4 to 6.
4. **S4.** The specification for the state directory settles how a
   cell's first generation is committed with a record that the cell was
   initialised, so first contact and the loss of both copies differ. It
   must keep N27, point 6: an initialised cell fails closed and its
   generation is never recreated automatically.
5. **S5.** The specification for the stable object key (C11) settles how
   a new state directory or a declared restore changes the keys it
   issues. It must keep N38, point 8: no key the operating system has
   seen names another object, within O42, point 6's bound.
6. **S6.** The specification for packaging (O37) settles, per package
   manager, how a site verifies the first keyring package and removes a
   compromised key from the trust store in effect. It must keep O29,
   point 6, and O20: no trust from unverified content, the advisory names
   the compromised fingerprint as well as the new one, a removed key's
   signature is rejected, and IronAFS fetches no key.
7. **S7.** The specification for the Windows route (L19, L20) settles how a
   saved path in each form the row ships (`\\afs` where the name is
   admitted, the literal `\\127.0.0.1` form on literal-address rows, and
   any site-configured name, L21, point 1) is handled after the proxy is
   removed by a policy refresh or an uninstall, when Windows' own file
   sharing listens on port 445. It must keep L20, point 8: an
   authentication attempt on each such saved path, with the route disabled
   or with a competing local listener present, reaches IronAFS's own
   listener or no listener at all; any other local or remote listener
   receiving any authentication attempt (Windows credentials included, not
   only IronAFS or AFS ones) fails the row. After an uninstall, no IronAFS
   or AFS credential is sent under the name. L41, point 4, is its gate.

## Review

The full record, with every finding and its disposition, is
[docs/reviews/2026-11-20-adr-0004-review.md](../reviews/2026-11-20-adr-0004-review.md).

- **Seats.** GLM (`glm-5.3`, max) for security and design, and GPT
  (`gpt-6.2-astra`, xhigh) for design; Gemini (`gemini-4-carbon-high`) for
  design in round 1 only.
- **Round 1** reviewed the whole ADR at `9ced6e2`. All seats failed it. The
  findings merged into 109 issues (19 blockers, 65 majors, 25 minors), none
  refuted: 86 were fixed in `8452f18`, and 23 choices were decided and
  written in `6020b53`.
- **Round 2** verified those fixes. Its 45 items were fixed in `6f39ac4`,
  including the move of the Cloud Files replica's mechanisms to a replica
  ADR required before it is built (L27).
- **Rounds 3 to 14** each verified only the previous round's findings and
  the text that changed; every finding was fixed in the next commit, and
  seven mechanisms became specification items S1 to S7.
- **Round 15**, on `8592a99..b8ae156`: GLM PASS with no findings; GPT
  PASS, every item fixed.
- **Round 16** read the whole ADR at `1ecfe1d` for decisions that
  contradict each other (GLM, one run per part): PASS, with three minor
  cross-reference findings, fixed in `d78ac85` and verified by GLM and GPT
  (both PASS, no findings).
- **Correction, 2026-11-20 (#28):** "How this ADR is organised", point 2
  now points to `docs/adr/README.md` for how an amendment is reviewed,
  instead of "the normal tier for its class". No decision changed.
- **Correction, 2026-11-20:** the four part files' status lines said
  "proposed" after the ADR was accepted; they now say "accepted". No
  decision changed.
- **Amendment, 2026-11-20 (#32, issue #31): O50, point 9, and C35, point 3.**
  - What changed: rxgk is planned for M6, after the Windows layer, and is
    tested against N25, point 3's clean-room simulator peer. It needs no
    outside peer to start. Each of the three triggers now opens a
    scheduling review that may move the work; none starts it by itself.
    O50's "Revisit if" and "Why" say the same. Work against the
    clean-room peer is conformance evidence, not interoperability. The
    server trigger is worded as N25 words it (issue #22 decided in favour
    of a server). An independent peer is used where one exists (N25,
    point 4). What waits on rxgk keeps its own gates: FIPS mode stays
    unscheduled (O31, point 1), and no negotiation RPC is designed until a
    server exists (N22, point 9).
  - Evidence: the two texts disagreed. O50, point 9, started rxgk
    implementation only on a trigger; N25, points 1 and 3, planned it for
    M6 against the clean-room peer. Both are working decisions, and no
    firm decision depends on the start rule. The owner decided on
    2026-11-20 that N25 holds. N25 is not changed.
  - Sweep: the repository was searched for "implementation starts", "one
    of these holds", "Until then rxgk", "rxgk waits", "point 9 fires",
    "O50, point 9", "O50.9", "planned for M6", "rxgk is late", "ADR 0004"
    and "rxgk".
    - O50, point 9, its "Revisit if" and its "Why" (part 3), and C35,
      point 3 (part 1): rewritten as above.
    - N25's heading and point 1 (part 4), and the overview's index entry
      for N25: "waits for a peer" and "may be advanced on the same three
      triggers" agree with the new rule, since the M6 peer is the
      clean-room simulator. Unchanged.
    - C35's "Revisit if" (part 1) and N25's "Revisit if" (part 4): name
      the triggers as reasons to revisit, not as starts. Unchanged.
    - C21's "planned for M6" (part 1), N30, point 4 (part 4), and part
      1's "Cost on multi-user hosts": agree. Unchanged.
    - Part 3's threat table ("rxgk later (O50)"), its alternative "rxgk
      before the OS layers", and its FIPS alternative: agree. Unchanged.
    - Part 4: N3's default trigger, N5, point 7, N9's "Revisit if",
      N18's "Why", the "No production rxgk token" consequence and the
      "self-written peer" alternative: about an independent peer or the
      gate of N13, not the start rule. Unchanged.
    - ADR 0001, the research note and the review records in
      `docs/reviews/`: no start rule, or a dated record. Unchanged.
    - Part 4's consequence "rxgk is late": agrees with the M6 order.
      Unchanged.
    - No skill, workflow or contributor file states the rule. The rule
      binds the same actors as before (the maintainer's session,
      contributors and their agents); no bot acts on it.
  - Review rounds, class S on the strongest row, from families that
    wrote neither the amendment nor the decision: GLM (`glm-5.3`, max)
    and GPT (`gpt-6.2-astra`, xhigh).
    - Round 1 on `3aacdf0`: GLM PASS, 2 minor; GPT FAIL, 3 major and 2
      minor (the server trigger worded unlike N25; independent peers
      required where N25, point 4, uses one only where it exists; gated
      work such as FIPS mode and the negotiation RPC implied before its
      gates). Fixed in `4ce1c33`; N25's heading was left as decided.
    - Round 2 on `3aacdf0..4ce1c33`: GLM PASS; GPT FAIL, 1 major
      (conformance and interoperability merged) and 1 minor. Fixed in
      `ea23d5d`.
    - Round 3 on `4ce1c33..ea23d5d`: GLM PASS and GPT PASS, each with
      one minor (no production check of N13's gate), fixed in the next
      commit.
- **Documentation sweep, 2026-11-21 (M1's exit).** The repository's
  public descriptions were checked against this ADR. The full list, with
  what changed and why, is
  [docs/reviews/2026-11-21-m1-close-sweep.md](../reviews/2026-11-21-m1-close-sweep.md).
  No decision changed.
  - Changed: coding standards rule 21 (no async runtime, C3);
    `docs/workflow.md` §5 (macOS 27 on Apple silicon, L8; the matrix,
    L36 to L39); one dated pointer to the answer in each of three
    research notes; the date of acceptance in the ADR index.
  - Left to its own pull request: coding standards rule 20's per-OS
    crates (C4), whose replacement is drafted and lands with the layout
    page.
  - Checked and unchanged: `README.md`, `AGENTS.md`, `CONTRIBUTING.md`,
    `AI.md`, the skills, and `docs/workflow.md` §2 and §6, which earlier
    pull requests brought in line (#23, #35, #36, #37, #39).
