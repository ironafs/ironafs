<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0004: The architecture

Status: proposed (2026-11-20). Raised in issue #20.

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
     new evidence and is reviewed at the normal tier for its class
     ([docs/workflow.md](../workflow.md)). That route opens once
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
what would settle it. The parts cite them as A1 to A13. An assumption that
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
  - Settled by: site requests, or a survey of the AFS user community.
    Without either, the row lapses at its cutoff.
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
  back-end port while IronAFS is stopped `[WIN-189]`. A persistent
  port reservation cannot: Windows gives such reservations no security
  between components `[WIN-187]`.
  - Depends on it: the fail-closed admission criterion for route 3
    (part 2).
  - Settled by: a test in M2 in which an ordinary local user tries to bind
    the port while IronAFS is stopped.
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
  - Depends on it: the design of the Cloud Files replica and when it is
    scheduled (part 2).
  - Settled by: public measurement on the named Windows builds, before the
    replica is built. Until then this ADR cites only Microsoft's
    documentation for the replica.
  - Two documented facts belong beside it: hard links on placeholders are
    refused by default `[WIN-M04]`, and Microsoft's two pages disagree on
    whether the partial hydration policy is supported `[WIN-M44]`.
- **A7. Identity and lifecycle in a Cloud Files replica.** We assume a
  replica identifies callers per Windows user and per root `[WIN-059]`.
  Microsoft documents a session id in callbacks and no user or token
  `[DRV-019]`. We also assume that logoff needs handling apart from a service
  stop `[WIN-128]`, and that keeping the cache within a size is the
  provider's job, with no automatic dehydration `[WIN-085]`.
  - Depends on it: the replica's one-root-per-user model and its cache-size
    policy (part 2).
  - Settled by: a spike that logs callback session ids across two users, a
    logoff and a run under disk pressure.
- **A8. The order of the bulk status calls.** We assume an AFS-3 fileserver
  accepts InlineBulkStatus after a directory listing, and that BulkStatus
  is the fallback when a server refuses it. Both RPCs exist; their
  order of use is our assumption `[DES-076]`.
  - Depends on it: listing performance (C36).
  - Settled by: a packet capture of a listing against the OpenAFS test
    cell, in M2.
- **A9. No conditional store in AFS-3.** We assume AFS-3 offers no store
  that is conditional on the server's current version of a file. So a
  client cannot atomically refuse to overwrite a version it has not seen,
  and a replica's publish sequence keeps a short window `[WIN-098]`
  `[WIN-100]`.
  - Depends on it: the rule that deferred stores are never automatic
    (C28), the replica's publish protocol (part 2), and the case for a
    conditional store in a later server profile (issue #22).
  - Settled by: a published AFS-3 file-service specification, or a
    decision, recorded in an ADR, on whether the 1990s AFS reference
    documents count as published specifications under ADR 0001.
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
  - Depends on it: the IPv6 statement in C34. If a callback's translated
    source cannot be matched to its server, that server earns no callback
    trust and stays in bounded revalidation (C34, point 6; C32).
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
- **A13. rxkad and the integrity of a reply.** We assume no rxkad level
  protects the integrity of a reply's payload. The claim record holds no
  public source that states, level by level, what rxkad protects. The
  rxgk base draft describes rxkad as limited to very weak cryptography,
  approximately single-DES `[ADR-034]`.
  - Depends on it: IronAFS's stated position on rxkad (O23, point 4);
    what counts as evidence in negotiation (N1, N5); and the rule that
    no rxkad fetch enters the shared pool (C18, C21).
  - Settled by: the specification route for rxkad (O50, point 3), which
    confirms or corrects it.

## Open questions for review

On these points the parts do not yet give one answer, or give one whose
cost the review should weigh. Each entry names the decisions involved and
the options. None is answered here.

1. **Renaming over a file that is open on the same machine** (C10,
   point 5). C10 allows the rename only where one atomic replacement can
   also keep the replaced file for its open handles, and refuses it
   otherwise. We know of no AFS-3 operation that does both, so today it
   is always refused, and a program that saves by renaming over a file
   another local process holds open fails. Options:
   - keep the refusal;
   - do the one atomic rename, let handles open on the replaced file fail
     with `Stale`, and keep that file's unstored bytes as recovery data
     (C29);
   - refuse only when the replaced file has unstored bytes.
2. **rxgk in a production build while the combine-tokens call has no
   assigned number** (N4, N6, N8, N12, N13). N13 gives such a build no
   rxgk fileserver path. If it still held an rxgk token for a cell's
   database services, N4, point 1, would refuse that cell's fileservers,
   because the exception of N12 is disabled too. Options:
   - no rxgk token is installed in a production build while the gate is
     closed, so rxgk stays test-only;
   - rxgk for database services only, with a stated exception to N4 that
     lets fileservers use a companion rxkad credential inside the
     configured floor;
   - no release offers rxgk until the call is assigned.
3. **May an administrator export another user's recovery data?** (C28,
   point 7; C29, points 4 and 5; O16; O33, point 5; O42, point 4). C29
   gives a record's bytes to its owner's account only. The upgrade and
   imaging procedures use `afs pending export` where no owner may be
   present. Options:
   - the owner's account only: an administrator can list and discard;
   - an administrator may export any record, as an audited action;
   - an administrator may move records, unread, to where their owner can
     recover them.
4. **Uploads from the Windows replica, if it is built** (C28, point 3;
   L27, point 3; A9). A9 speaks of a publish sequence with a short
   window, which reads as an automatic store. C28 allows no automatic
   store of a write that was not committed while its application waited.
   Options:
   - an upload is a deferred store: it waits for the user's instruction;
   - an upload counts as an immediate store, made when the provider sees
     the change, and the last writer wins;
   - a check-then-store publish sequence, as a named exception to C28.
5. **DNS discovery on a flat list of name servers** (C33, point 4). Is
   an administrator's confirmation, with the listed servers asked in
   order under documented timeouts, enough to keep IronAFS's own DNS
   client from going around the host's choice of server? Options:
   - as C33 states;
   - no discovery on a flat list: configured servers only.
6. **Where the time and randomness seam lives** (C3, point 3; C4,
   point 7; C5; C37; N8; O45). No crate in C4 holds the `Clock` and `Rng`
   interfaces and the time types, the simulator, or the state
   directory's format marker. C37 puts the production clocks in
   `ironafs-os`, and C5 puts the cryptographic random source behind
   `ironafs-crypto`. Options:
   - a small pure crate, for example `ironafs-time`, with the production
     implementations in `ironafs-os`;
   - inside `ironafs-vfs`;
   - inside `ironafs-client`, with the daemon supplying the system
     implementations.
   The simulator and the format marker need a home under any of these.

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
     rests on such a claim only through a named assumption (A1 to A13),
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

Filled in when all four parts are drafted.

## Review

Reviews are recorded here, part by part.
