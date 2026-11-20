<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0004, part 4: Negotiation and formats

Status: proposed (2026-11-20). Part of ADR 0004 ([overview](0004-architecture.md)); raised in issue #20.

This part decides three things:

- how the client chooses a security class and optional protocol features
  with each server, and why it is never moved to a weaker one without an
  authority saying so (N1 to N25);
- what the client contains now so that a later, richer AFS profile needs no
  redesign of the client (N26 to N31);
- the shape of the formats that outlive a release: journal records, the
  principal value, identifier widths, the recorded security profile, and the
  extension rules of long-lived structures (N32 to N47).

Claim ids such as `[DES-061]` refer to the
[claim record](../reviews/2026-11-20-adr-0004-claims.md). A1 to A12 are the
named assumptions in the [overview](0004-architecture.md). Milestone names
(M2a, M2b, M3, M6) are the plan in part 3 (O50); they are a plan, not a
promise. Other parts are referenced by decision id where this part depends
on them.

## Context

### The servers the client meets

- Existing cells' clients use rxkad with Kerberos 5 tokens; OpenAFS adds
  rxgk in its 1.9 series `[DRV-089]`. The OpenAFS roadmap expects rxgk in
  2.0 `[DRV-M24]`.
- OpenAFS 1.9.0's release notes list rxgk only for server-to-server traffic
  and local keys; no client-to-fileserver rxgk is listed `[DES-048]`. The
  1.9.x installation notes say client tokens are still in progress
  `[DRV-M25]`.
- rxkad's non-DES forms, rxkad-k5 and rxkad-kdf, are described in OpenAFS
  Security Advisory 2013-003 `[DES-049]`. The community Rx specification
  does not specify security classes `[DRV-084]`.
- AFS-3 server addresses in the volume location database are IPv4
  `[DES-058]`.

### What rxgk is specified by

- The latest base draft is draft-wilkinson-afs3-rxgk-11 (2014), expired
  `[DRV-085]`. The AFS-3 standardisation group promoted it to experimental
  status `[DRV-087]` `[DES-M05]`.
- "Integrating rxgk with AFS" (draft -08) is an expired draft `[DRV-086]`.
  The recorded promotion to experimental status names the base draft only
  `[DRV-087]`.
- That draft defines a combine-tokens call. It must run only over a secured
  rxgk connection, and servers reject it at clear level `[DES-061]`.
- A zero-length token in its reply tells the client that rxgk token
  generation for that fileserver cannot work now. The client may fall back to another mechanism for
  that server, and must not downgrade in any other situation `[DES-062]`.
- The draft gives the call no procedure number: its definition ends
  `= TBD` ([draft -08, section 9](https://www.ietf.org/archive/id/draft-wilkinson-afs3-rxgk-afs-08.txt)).
- A token expiry of zero means the token never expires, recommended only for
  printed tokens `[DES-063]`.
- The base draft's connection key lifetime and byte lifetime are advisory:
  endpoints should rekey at their earliest convenience `[DES-059]`. Keys are
  derived with the GSS pseudo-random function `[DES-060]`.
- RFC 8009 defines the Kerberos encryption types
  aes128-cts-hmac-sha256-128 and aes256-cts-hmac-sha384-192 `[DES-064]`.

### Numbers

- The AFS assigned-numbers registry lists the Rx services: fileserver and
  callback service on service 1, protection on 73, volume location on 52
  `[DES-011]`.
- The registry assigns new Rx service ids first come, first served. It
  reserves a range for site-local use (64512 and above), and it does not
  manage the procedure numbers of a private service
  ([Rx services](https://registrar.central.org/rxservice.txt)). It also
  keeps capability-bit lists, such as the fileserver's
  ([fileserver capabilities](https://registrar.central.org/cap/viced.txt)).
- AFS volume errors are small positive integers that collide with system
  errno values, so a caller must tell them apart by context `[DES-055]`.

### Formats and neighbours

- A 64-bit data version appears in published extension material; that AFS
  has used one since a particular release was not found in a primary source
  `[DES-057]`.
- RFC 8881 defines ACE inheritance flags (section 6.2.1.4) and the ACL
  flags for automatic inheritance and protection (section 6.4.3.2)
  `[DES-065]`.
- RFC 9334 defines the attester, verifier and relying-party roles
  `[DES-066]`.
- On Linux FUSE, answering LSEEK or COPY_FILE_RANGE with `ENOSYS` is
  permanent for the mount `[DES-031]`. Enabling FUSE's POSIX ACL capability
  makes the kernel enforce ACLs and turns on its own permission checks
  `[DES-026]`.
- lseek(2) describes the simplest implementation of data and hole seeking:
  SEEK_HOLE returns the end of the file and SEEK_DATA the offset given, so
  the whole file is treated as data
  ([lseek(2)](https://man7.org/linux/man-pages/man2/lseek.2.html)).
- RFC 8672, a pinning scheme for TLS, says the lifetime of a pinning
  ticket on a production system should be between 7 and 31 days
  ([section 5.2](https://www.rfc-editor.org/rfc/rfc8672.txt)).

### What forces a decision now

- **Downgrade by software age.** A security setting added in a later
  release and pushed to an older client would be ignored unless the setting
  exists, and is understood, from the first release that authenticates.
- **Formats written by the first release are read by later ones.** The
  journal and the recovery store are promised across adjacent versions
  (C29, O32). Control IPC is compatible across the current and previous
  minor version (C44). The state directory carries a format version (N8,
  O32). A closed
  shape in any of them makes every later addition a format break.
- **A possible later server.** Issue #22 records an option, not a
  commitment: an IronAFS server for IronAFS clients, speaking a modern AFS
  profile. Nothing here depends on it being built. This part keeps the
  client ready for a richer profile only where doing so costs nothing
  against existing servers.

## Decision

**Negotiation and downgrade protection**

### N1. Profiles, and their order

**Firm.**

1. A **profile** is the security class and level actually used on a
   connection. Classes are ordered `rxkad` < `rxgk`. Levels are ordered
   `auth` < `crypt`.
2. rxgk at clear level and rxkad at clear level are never used with a
   token.
3. Unauthenticated access (rxnull) is not a profile. It is governed by its
   own switch (N3) and has its own path.
4. Floors, pins and ranking compare class and level only. The negotiated
   suite is recorded beside them (N39) and does not take part in the
   comparison (N40).
5. The class list and its order are data in the protocol crates, not code
   spread through the client.

Why: a floor needs an order to compare against. Class and level are two
values because the two classes protect different things at each level (the
threat model, O23); anonymous access is the absence of a credential, not
a weaker class.

### N2. Three controls, and the effective floor

**Firm.**

1. Downgrade protection has three controls, strongest first: the
   **configured floor** (N3), the **token's own profile and the chosen
   identity** (N4), and the **learned pin** (N9).
2. The **effective floor** for a cell is the higher of the configured floor
   and an unexpired pin.
3. The configured floor always applies. No exception in this part goes
   under it, including the legacy fileserver exception (N12).
4. No control is lowered by a failure, a timeout, or anything received
   without verified protection (N5, N6). A floor is lowered only by an
   audited configuration change, and a pin only by an administrator or its
   expiry.

Why: a configured floor works on first contact; the token rule needs no
stored state; a pin covers machines nobody provisioned. Each covers a case
the others miss.

### N3. The configured floor: keys and defaults

**Working.** Revisit if: client-to-fileserver rxgk ships against a tested
peer (N25), which is when a default of `min_class = "rxgk"` for cells that
offer it can be weighed; or a tier 1 cell cannot be reached at
`min_level = "crypt"`.

Per cell, with machine-wide defaults:

| Key | Values | Default |
|---|---|---|
| `min_class` | `rxkad`, `rxgk` | `rxkad` |
| `min_level` | `auth`, `crypt` | `crypt` |
| `legacy_fileservers` | `deny`, `cell-attested` | `cell-attested`; `deny` when `min_class = "rxgk"` |
| `anonymous` | `allow`, `deny` | `allow` |
| `learned` | `enforce`, `warn`, `off` | `enforce` (without effect while N13 holds) |

1. OS policy can lock these keys like any other (O17).
2. Raising or lowering the configured floor is an audited configuration
   change. Lowering it fences existing grants as any policy change does.
3. There are no per-user floors, no per-server floors, no per-service
   floors and no encryption-type list in configuration.

Why: the defaults reach existing rxkad cells unchanged `[DRV-089]` while
refusing clear-text use of a token. One floor per cell and one switch for
mixed cells is what one administrator can keep correct on a fleet.

### N4. The token's own profile, and no anonymous fallback

**Firm.**

1. While a context holds a token for a cell, expired or not, none of its
   calls to that cell go out below that token's class and level. The one
   scoped exception is N12, which uses a separate credential under that
   credential's own profile.
2. While a context has **selected an authenticated identity** for a cell,
   none of its AFS requests fall back to anonymous, whatever happens to the
   token. The caller never gets anonymous data in place of its own.
3. Installing a token records an **authenticated-intent marker** for
   (cell, context). The marker:
   - is persisted before the installation is acknowledged;
   - holds no key material and is readable only by the daemon;
   - is bound to the cell, the authenticated context and that context's
     lifetime, so it survives a daemon restart and never attaches to a
     reused identity.
4. A matching marker without a token returns `TokenRequired`.
   `TokenExpired` is returned only when trusted local expiry metadata
   establishes it under the token's profile. A server's claim that the
   token expired is reported as a server rejection. Every authentication
   failure keeps the authenticated intent.
5. The marker is removed only by the context's own explicit `unlog`, or on
   verified teardown of the context. After `unlog`, newly issued requests
   follow the cell's `anonymous` policy.
6. An authorised, audited administrator may revoke another context's
   token. The marker stays.
7. A credential change invalidates queued requests bound to the old
   credential.
8. `aklog`, `unlog` and `tokens` name the cell and work without reading
   `/afs`, so a context whose token expired can inspect and recover.

Why: without the marker, an expired or dropped token silently turns a
user's requests into anonymous ones, and a program reads different data
without an error. Expiry can then interrupt access to the cell, including
programs run from AFS; renewal at login and its failure notice (O36) are
the mitigation.

### N5. Evidence and hints

**Firm.**

1. **Evidence** is only a reply the daemon itself verified on an rxgk
   connection at `auth` or `crypt` level.
2. Everything else is a **hint**: DNS answers, rxnull replies, rxkad
   replies, Rx aborts, timeouts, capability words read without that
   protection, and anything `aklog` reports.
3. A hint may lower trust, or choose between RPC variants the catalogue
   lists as equivalent (N20). A hint never raises or satisfies a floor,
   never learns or renews a pin, and never authorises an exception.
4. Nothing received on an unauthenticated path feeds an authenticated
   path. This is the same rule the cache follows (C19).
5. `aklog` runs as the user, so what it states is a user's assertion. The
   daemon enforces the floor itself when a token is set and again at every
   connection (N7).

Why: IronAFS does not rely on any rxkad level for the integrity of a
reply's payload (the threat model, O23), and the AFS-integration draft
requires the one call that carries a downgrade signal to run over a
secured connection `[DES-061]`.

### N6. Failure never lowers

**Firm.**

1. A dropped packet, an abort, an unknown-opcode reply, a missing
   credential or a Kerberos error produces a failure or a retry inside the
   effective floor. None produces a lower profile.
2. On the rxgk path, a missing, failed or invalid combine-tokens reply
   fails closed, also before any pin exists.
3. Key exhaustion on a connection is its own outcome (N45). It leads to a
   rekey or a reconnect at the same profile, never to a weaker one.
4. The error is `SecurityFloor` (with `TokenRequired`, N4), reported to
   applications as a permission failure with the cause shown (C46,
   O35). A refusal names what refused it.
5. A cell whose rxkad-only state is established independently follows the
   baseline policy: the configured floor and the token rule.

Why: every item in point 1 can be produced by someone on the network path.
If any of them could lower the profile, that person would choose it.

### N7. One choke point

**Firm.**

1. Every connection is created through one pure function: (cell and its
   protocol identifier (N27), server, service, context, effective floor,
   evidence) to a profile or a refusal.
2. It runs again on failover, on a volume move and on a token change. The
   connection key includes the class (C30).
3. The daemon applies it when a token is set, so a user process cannot
   lower anything.
4. `aklog` asks the daemon for the effective floor first, tries rxgk when
   the cell may offer it, and falls back to rxkad only if the floor and any
   active pin allow. The fallback is an event.
5. Invariant, checked in the simulator (O45): **no packet
   leaves on a connection whose profile is below the effective floor.**

Why: one function is one place to test and to audit. A rule enforced at
several call sites is eventually missed at one.

### N8. Security keys are understood from the first authenticating release

**Firm.**

1. The floor keys, `SecurityFloor`, `TokenRequired` and the
   authenticated-intent marker exist from the first release that
   authenticates (M2b in the plan).
2. An unknown **value** of a known security key refuses that cell. It is
   never ignored. Example: `min_class = "rxgk"` on a build whose rxgk
   fileserver path is not enabled fails that cell with the reason.
3. The state directory carries a format version from the first release
   (M2a in the plan). A release refuses a newer one; upgrades follow O32
   and O33. Later stores raise it: the marker, then the pin store.
4. A floor the installed client cannot meet fails that cell with the
   reason. `min_client_version` (O19) is how a site
   states it for the machine.

Why: an older client that ignored a newer floor would be downgraded by its
own age, and a software rollback that did not know the pin store would
drop pins without saying so.

### N9. The learned pin: scope and learning

**Working.** Revisit if: the combine-tokens call gets a published
procedure number whose semantics differ from draft -08; or the first test
against an independent rxgk peer (N25) finds a cell that negotiates rxgk
but does not answer the call, which under these rules would refuse every
fileserver of that cell.

1. A pin records, for the machine, "this cell negotiates rxgk for its
   fileservers".
2. **Scope.** One pin per cell, per machine, scoped to the client profile
   and service path it was established on. There are no per-server pins
   and no per-context pins.
3. **Learning.** A pin is learned or renewed only by a **qualifying
   success**: ordinary client rxgk token negotiation succeeds, **and** the
   authenticated volume-location service returns a fresh, well-formed
   combine-tokens reply at `auth` or `crypt` level for a fileserver of that
   cell.
4. A non-empty token and an explicit empty token both qualify: either
   shows that the cell answers the negotiation. A database reply alone
   establishes no pin. Failed attempts, cached replies and rxkad traffic
   never learn or renew one.
5. Any context's qualifying success raises the pin for the machine. Only
   an administrator or expiry lowers it.
6. **Effect.** An active pin blocks a downgrade, except through N12. A
   refusal names the cell, the server, the pin's expiry and the
   administrator's command `afs security forget <cell>`.

Why: a pin is a fact about the cell, and per-context pins would give every
new login a first contact. The cell itself states, per fileserver, whether
rxgk token generation can work for that server now `[DES-062]`, so per-server pins would only remember
what is asked fresh each time. A local user cannot forge a qualifying
success for a real cell without that cell's keys.

### N10. The learned pin: lifetime

**Working.** Revisit if: a cell operator reports a rollback from rxgk that
the 30-day block made unmanageable for unmanaged machines; or published
guidance for comparable pinning schemes moves away from the 7 to 31 day
range.

1. A pin lasts 30 days from its last qualifying success. Each one renews
   it. Thirty days is IronAFS policy, informed by RFC 8672's range.
2. Elapsed time is tracked on the boot clock while running (C37)
   and from persisted timestamps across restarts. Ordinary downtime counts
   toward expiry.
3. When a wall-clock discontinuity can be detected independently and makes
   expiry uncertain, it is reported, and the pin is kept until the clock is
   trustworthy or an administrator clears it.
4. Expiry removes only the learned restriction. The configured floor and
   the token rule stand.
5. Expiry raises an event. While the expired record is kept, another event
   is raised before the first weaker use.
6. Expired records are kept at most 30 more days, within a bounded record
   count, unless replaced or removed earlier. The expired date is shown
   only to authorised administrators.
7. `afs security forget <cell>` (administrator, audited) removes a pin.
   `learned = "off"` in policy does it for a fleet after a cell-wide
   rollback.

Why: a pin with no expiry locks out every client that learned it when a
cell trials rxgk and withdraws it. We found nothing in the specifications
IronAFS builds from that lets a cell state a pin lifetime, so the client
chooses one.

### N11. The pin store

**Firm.**

1. Pins live in the state directory, never in the cache. A pin survives a
   cache wipe.
2. Each record holds the class, the level and the negotiated suite (N39).
3. A renewal is committed only after it is durably written. On a write
   failure the daemon reports a health error and keeps the active
   restriction while persistence is uncertain.
4. An **unreadable** pin file is treated as empty, with a health
   "degraded" event.
5. A **readable** record holding a suite this release cannot name refuses
   the cell until an administrator acts. It is not treated as empty.
6. A lost state directory returns the machine to first contact (N14).

Why: a record written by a newer release states a restriction this release
cannot evaluate, so refusing is the only answer that cannot be weaker than
the record. An unreadable file carries no statement at all; treating it as
a refusal would let any corruption lock out a cell.

### N12. The one downgrade signal: a cell-attested legacy fileserver

**Firm.**

With `legacy_fileservers = "cell-attested"`, a fileserver may be used over
rxkad in an rxgk cell only when all of these hold. It is the sole scoped
exception to the primary token's rxgk selection (N4) and to the learned
pin (N9).

1. **Attestation.** The cell's authenticated volume-location service
   returns, on a connection at `auth` or `crypt` level under the context's
   own rxgk credential, a fresh, successful combine-tokens reply with the
   explicit empty token for that fileserver's UUID. A failure or a missing
   reply authorises nothing.
2. **A separate credential.** The rxkad connection uses a separate, valid
   rxkad credential for the same context, cell and intended identity,
   under that credential's own profile and the configured floor. `aklog`
   may obtain this companion when policy permits and the cell issues it.
   If it is unavailable, the request is refused with that reason.
3. **Scope.** Only the attested fileserver. Never a database service of
   that rxgk cell. No extension (N22).
4. **Freshness.** A fresh attestation is required to open or re-establish
   a legacy connection. Reuse of an open one is bounded by both
   credentials' lifetimes and by a short re-attestation interval, which the
   specification for the security classes fixes before release.
5. **End.** The permission ends on a credential or policy change, or on
   verified evidence that the server supports rxgk.
6. With `deny`, such a server is not used.
7. There are no fileserver exception lists on clients.
8. The exception is disabled in production while N13 holds.

Why: the AFS-integration draft allows a fallback for one server on
exactly this signal, and forbid a downgrade in any other situation `[DES-062]`; the
signal is only trustworthy over a secured connection `[DES-061]`. This
covers mixed cells during a migration and the rollback of one fileserver.
IronAFS's rule is narrower than the draft's: it also requires a separate
credential and bounds reuse.

### N13. What ships disabled: the opcode gate

**Working.** Revisit if: the combine-tokens call gets a published
procedure number for the selected rxgk profile.

1. Until that call has a public assignment for the selected profile, a
   production build makes no combine-tokens call.
2. So learned pins (N9), the legacy fileserver exception (N12) and the
   shared cache pool (C21), which all depend on that call,
   ship **disabled**.
3. Meanwhile downgrade protection rests on the first two controls: the
   configured floor and the token's own profile.
4. Any number used for the call before an assignment is test-only.
   Fixture numbers stay out of production builds (N24).

Why: the draft leaves the call's procedure number unassigned
([draft -08, section 9](https://www.ietf.org/archive/id/draft-wilkinson-afs3-rxgk-afs-08.txt)),
and N24 forbids releasing on an unassigned shared number. OpenAFS 1.9.0
lists no client-to-fileserver rxgk `[DES-048]`, so against the first test
target the wait removes nothing a user could use today.

### N14. First contact

**Working.** Revisit if: a published, authenticated way for a cell to
state its security policy appears (for example a standardised signed
record), which would give an unprovisioned machine a floor.

1. First contact with an unconfigured cell has no downgrade guarantee.
2. `afs status` says so: "floor: default; learned: none", or "learned:
   disabled" while N13 holds.
3. Sites that need a guarantee provision the floor. While pins are
   disabled, nothing else gives an unprovisioned machine that guarantee.
4. Not built: a cell policy published in DNS, DNSSEC-validated or signed
   cell metadata.

Why: the client has no authenticated source for a cell's policy before it
has talked to the cell. Saying so is better than implying a protection
that is not there.

### N15. Replicas of mixed profile

**Working.** Revisit if: testing against a mixed cell (N25) shows that
ranking by profile first sends reads to a markedly slower site in normal
operation.

1. Sites are ranked by profile first, then by round-trip time and
   preference (C33).
2. Falling to a cell-attested legacy replica is allowed by the floor,
   raises an event the first time for each volume, and lands in a
   different cache namespace, because the namespace key includes the
   profile used (C18).

Why: a stronger site that answers is preferred to a faster, weaker one;
data fetched under a weaker profile must not be served as if it had come
under the stronger one.

### N16. The administrator's view

**Working.** Revisit if: support cases show that administrators cannot
tell from this output why a cell was refused.

1. `afs status` shows, per cell: the effective floor; where each part came
   from (file, policy, learned with the pin's expiry, or "disabled"); and
   the profile in use per server, with its suite.
2. `afs config show --effective --origin` shows the configured part.
3. `afs status` reports when a server announced capabilities this version
   does not know. That is the only upgrade hint, and it is local.

Why: a refusal an administrator cannot explain gets "fixed" by lowering
the floor.

### N17. Negotiation never fetches code

**Firm.**

1. Negotiation changes what the installed client uses. It never fetches,
   installs or runs code, and never contacts a software vendor.
2. It changes nothing in the position that IronAFS has no automatic
   update and no update checks (O20).

Why: a protocol exchange with a server that could change the client's code
would make every fileserver a software distributor.

### N18. FIPS mode is this mechanism with fixed values

**Working.** Revisit if: client-to-fileserver rxgk interoperates with an
identified peer, and the operating environments and cryptographic modules
used are covered by active validation certificates.

1. A future FIPS mode is unscheduled.
2. When built, it is N3 with fixed values: `min_class = "rxgk"`,
   `legacy_fileservers = "deny"`, `anonymous = "deny"`, with a validated
   cryptographic module in its approved configuration. rxkad, NTLM,
   unauthenticated access and every fallback are refused.
3. Its Kerberos profile is limited to RFC 8009 encryption types `[DES-064]`
   whose complete use has been verified.
4. The rest of that mode (platform policy, what is published) is in O31.

Why: the mode needs rxgk to a fileserver, and that has no peer to test
against yet (N25).

**Capabilities and extensions**

### N19. Capability observations are never persisted or shared

**Firm.**

1. An observation (a server's capability words, a remembered
   unknown-opcode reply) belongs to the connection that made it.
2. It stays in memory, and ends with the connection or on evidence of a
   server restart.
3. It is never written to disk and never shared with another connection or
   context.
4. Unknown capability bits are ignored and counted.

Why: N5, point 4. An observation made on a weaker or anonymous connection,
if shared, would steer an authenticated one. The cost is one capability
query per new connection.

### N20. Fallbacks come only from a static catalogue

**Working.** Revisit if: an optional RPC needed for a supported cell has a
fallback that cannot be described by the catalogue's fields.

1. The protocol crate holds one static catalogue. For each optional RPC it
   states: what announces it, its fallback, and whether the fallback keeps
   the same guarantees.
2. A fallback is used only where the catalogue lists one.
3. Abort codes are classified by RPC and server (C46).
4. Example: the bulk status call after a listing, and its fallback, is a
   catalogue entry whose order of use rests on A8.

Why: volume error codes collide with errno values and must be told apart
by context `[DES-055]`, so "the server refused this" has no single meaning
outside a per-RPC table. A table is also reviewable in one place.

### N21. What the client announces

**Working.** Revisit if: a tested server requires per-server variation in
the client's capability reply to interoperate.

1. The client's reply to a server's capability query on the callback
   service lists only what this build implements and has enabled, the same
   to every server.
2. An unknown callback opcode is answered with the abort the callback
   service's specification names, is counted, and changes no state.

Why: a reply that never varies cannot be used to steer the client, and it
identifies the build no more finely than its version does.

### N22. IronAFS extensions only over rxgk

**Firm.**

1. An IronAFS-defined extension is offered, accepted and used only on the
   approved rxgk profile at `auth` or `crypt` level, in both directions,
   callbacks included, with no exception.
2. Support is negotiated on that protected channel. A stronger
   confidentiality requirement of an extension applies.
3. The legacy fileserver exception (N12) grants no extension access.
4. Under rxkad the client speaks baseline AFS-3, so released OpenAFS
   servers see baseline AFS-3 only.
5. Each extension has a name and a small integer version in the catalogue
   (N20), a public specification under `docs/specs/`, and a
   `docs/provenance.md` row for its allocation.
6. A promise, lock or negotiated feature belongs to the connection and the
   server incarnation that granted it. A lock's guarantee is fixed when it
   is taken.
7. **Extended callbacks** may narrow what is refetched only when they
   arrive authenticated. Otherwise they are plain breaks (C31,
   C32).
8. **Server-coordinated locks** are a new lease kind in the lock service
   (C7, L33), never a change to an existing lock.
9. No negotiation RPC is designed until a server exists to speak it
   (issue #22).

Why: an extension negotiated where the negotiation can be forged is a
downgrade path with a new name. Negotiating on the protected channel makes
protection hold by construction.

### N23. Third-party optional operations

**Firm.**

1. Each is admitted to the static catalogue individually, with a publicly
   specified wire format and semantics and a verifiable assignment in its
   namespace.
2. Over rxkad its announcement is a hint (N5). It is used there only when
   the configured policy allows it **and** the catalogue entry shows that a
   forged announcement or reply cannot: expand authority; change state or
   a lock's guarantee; weaken cache revalidation; or expose more than the
   baseline operation does.
3. Extra work and retries are bounded.
4. An optional operation that affects trust, cache validity or lock
   guarantees requires authenticated message integrity.
5. A variant security class needs its own approval and a public profile.

Why: refusing every optional operation under rxkad would cost performance
on existing cells for no gain where a forgery gains nothing. The test is
per operation because the risk is.

### N24. The registration gate

**Firm.**

1. Nothing is released on an unassigned shared number.
2. Every shared wire number has a published assignment. An additive
   operation in a shared namespace gets a requested, published procedure
   number and capability bit.
3. An incompatible set of operations, or an IronAFS-private service, gets
   a published, registrar-assigned Rx service id and a fresh connection, so
   older clients keep the baseline service.
4. The registrar does not manage a private service's number space. Its
   procedure numbers and capability bits are IronAFS's, published in that
   service's specification.
5. Service ids in the site-local range are used only for isolated
   site-local tests.
6. When the first extension specification is drafted, the maintainer
   submits it to the registrar under the project's name, asking only for
   the ids it needs. The assignment is recorded in the specification and
   in `docs/provenance.md` once published.
7. If the registrar does not answer within 60 days, one follow-up is made
   and the request is raised on the AFS-3 standardisation list. Release
   stays gated on the assignment.

Why: the registry assigns service ids first come, first served and leaves
a private service's procedures to its owner
([Rx services](https://registrar.central.org/rxservice.txt)). A number
used before it is assigned can collide with someone else's later
assignment, and deployed clients cannot be recalled.

### N25. rxgk waits for a peer; what may be claimed

**Working.** Revisit if: an OpenAFS release offers client-to-fileserver
rxgk; or a third-party peer and wire profile are agreed; or issue #22 is
decided in favour of a server.

1. rxgk support waits for a peer to test against. It is planned for M6,
   after the Windows layer, and may be advanced on the same three
   triggers.
2. **In M2b:** pin the public base and AFS-integration drafts; record the
   exact rxgk wire profile, the provenance and namespace of every number,
   and the open specification gaps; record the test matrix by version and
   class; define the Kerberos encryption-type operations in the crypto
   crate; check the required GSS pseudo-random function `[DES-060]` and its
   exact output on each platform. Where a platform cannot supply it, the
   profile is disabled there.
3. **For M6:** the peer is a clean-room simulator whose protocol
   implementation is written independently of the client and shares no
   protocol code with it. Both sides pass the published vectors for their
   Kerberos encryption types and run positive and negative wire cases for
   the selected profile. That is conformance evidence, not
   interoperability.
4. An accessible independent peer verified for the same profile is used
   where one exists.
5. **Separately, in M2b:** a read-only database wire test at `auth` and at
   `crypt` level is attempted against an authorised, isolated OpenAFS
   1.9.x lab cell, with a test-only harness and disposable credentials
   kept out of logs and fixtures. The result is recorded as observed,
   without consulting implementation source (ADR 0001). A success shows
   only the database path.
6. **Claims.** "Interoperable" is reserved for profiles tested live
   against an independent implementation. Client-to-fileserver, pin or
   extension interoperability is claimed only for paths exercised against
   a matching independent peer. An IronAFS-only profile is called
   "supported" only after live client and server tests with real Kerberos,
   published vectors, an independent specification review, and simulator
   and negative tests.
7. Released OpenAFS stays in the baseline rxkad test matrix.

Why: rxgk rests on two expired drafts `[DRV-085]` `[DRV-086]`, one of them
experimental `[DRV-087]`; OpenAFS 1.9.0 lists rxgk only between servers
`[DES-048]` and client tokens are in progress `[DRV-M25]`. Code with no
peer cannot be shown to interoperate, and saying it does would mislead
sites that plan around it.

**Readiness for a richer profile**

### N26. The client is an AFS-3 client; readiness costs nothing

**Working.** Revisit if: issue #22 is decided, either way; or an item
below is shown to change a packet, an outcome or a resource budget against
an existing server.

1. The client stays fully compatible with existing AFS-3 servers. That is
   the product.
2. Issue #22 is an option, not a commitment. This ADR contains no server
   code and no server design.
3. Each item in N27 to N31, and each open format in N32 to N47, is kept
   only while it costs the client nothing against existing servers. Each
   milestone's shared-core change is gated on packet and outcome
   equivalence, errors, fallbacks and resource budgets included.
4. The protocol crates and the pure crypto interface depend on no client
   crate, checked in CI (C4).
5. Deferred until a profile needs them: dynamic rights catalogues, editing
   of unknown ACL forms, blanket widening of identifiers, learning the
   protocol from the network, and a second simulated profile.

Why: openness that is free now avoids a format migration later, whether
the richer peer is IronAFS's own or someone else's. Openness that costs
something now is paying for an option nobody has taken.

### N27. A protocol identifier per cell

**Firm.**

1. Each cell has a provisioned protocol identifier. It is `afs3` by
   default.
2. An unsupported value is refused before any network I/O.
3. The identifier is an input to the connection choke point (N7), part of
   the cache namespace key (C18), the scope of journal dependency bytes
   (N32), and shown in IPC status.
4. Activating another profile for a cell separates connection, credential,
   cache and journal identities, fences a same-name reconfiguration, and
   enforces a minimum client version. A re-profiled cell can never alias
   connections, credentials, cache or journal with its former self.
5. The identifier is never learned from the network.

Why: two protocol profiles under one cell name must not share a credential
or a cached byte. Making the identifier part of every key now is free;
adding it later means migrating every keyed store.

### N28. Operation grants, not rights letters

**Firm.**

1. AFS-3's caller access and mode restrictions are normalised at the
   protocol boundary into **operation grants**: one internal type, a set
   of operations. The core never interprets AFS rights letters.
2. The AFS-3 mask-to-operations evaluator is confined to one module plus
   the codecs. An `xtask` check enforces it.
3. **Unknown bits grant nothing.** A test proves it.
4. Every access answer an OS layer gives (L1) is backed, **per
   right**, by a current server-derived grant. Mode bits are metadata
   only.
5. Existing file-target semantics are kept, with the backing directory
   identified where needed.
6. A seam for a stable id-to-SID mapping is reserved for a later exact
   Windows DACL. Changing DACLs or owners stays refused until then
   (L26).

Why: a client that hard-codes one protocol's rights letters in its core
must be rewritten for any richer permission model, and a bit it does not
know must never be read as permission.

### N29. Callback kinds form a registry

**Working.** Revisit if: the specification for the callback service finds
a callback whose effect cannot be stated as grant revocation plus scope.

1. Callback kinds form a registry that maps each kind to the grants it
   revokes.
2. Revocation has an ordering and an expiry bound that do not depend on
   the data version: the client does not assume that an ACL or status
   change moves the data version.

Why: a registry lets a later profile add a kind without touching the code
that acts on one, and tying revocation to the data version would miss the
changes that leave data untouched.

### N30. Token and combine calls sit behind the profile

**Working.** Revisit if: the M2b rxgk profile record (N25) shows that a
token operation cannot be expressed behind this interface.

1. Opaque tokens, token negotiation and the combine call are reached only
   through the selected profile, never called directly by the core.
2. The lock service's choice of lease kind is one policy function (C7,
   L33).
3. Rx shares call initiation and the callback receive path, with bounded
   streaming, back-pressure, cancellation, and distinct transport and RPC
   outcomes (C30).
4. Planned for M6: opaque tokens, scoped protected callbacks, and baseline
   rxgk verification including the authenticated empty-token exception
   (N12).

Why: the class list is data (N1), so the calls that belong to a class must
be reachable only through it.

### N31. Evidence for any wire fact

**Firm.**

1. A wire fact is taken only from an allowlisted specification passage or
   an independently generated black-box fixture, recorded with its
   edition, section or fixture provenance.
2. Never from implementation history, interface-definition files,
   generated definitions, or live-cell captures holding credentials or
   private data.
3. Older published AFS documents are cited for facts only, in IronAFS's
   own words, each edition registered before use, with corroboration for
   anything that affects interoperability.

Why: ADR 0001. A wire format built on an uncheckable fact cannot be
audited for either correctness or provenance.

**Long-lived formats**

### N32. Journal records: two layers

**Firm.**

Every journal record has two layers. C28 and C29 decide what the journal
does; this is its record format.

1. A **common header**, the same for every kind and checksummed on its
   own:
   - the format version;
   - flags, split into **must-understand** and **may-ignore** bits;
   - the kind;
   - the payload length;
   - the record's **dependency scope**: the file ids it touches and, for a
     namespace record, both parent directories;
   - a data-transform id and an at-rest key id (N35).
2. The dependency scope is encoded as length-delimited bytes scoped by the
   cell's protocol identifier (N27). Under `afs3` it holds volume, vnode
   and uniquifier as 32-bit values (N38).
3. A **payload**, tagged by kind and checksummed separately, with an
   extension area.
4. Exact field order, sizes and checksum algorithm are fixed by the
   journal's specification under `docs/specs/` before the first release
   that writes a journal, under the `wire-format` rules.

Why: the journal outlives a release. With the dependency scope in a header
every release can read, a release that does not understand a record's
payload still knows what the record touches. Scoping the bytes by protocol
identifier makes a wider id in a later profile a minor version, not a
migration.

### N33. Journal versions, unknown records and quarantine

**Firm.**

1. Format versions are (major, minor). A new kind or a new must-understand
   flag is a **minor** version. Anything that changes the common header is
   a **major** version.
2. A release refuses a journal of a major version it does not know: an
   unsupported format: the release refuses to start, and the journal is
   drained with the newer release (O32, O33).
3. A journal of a higher minor version is read. Records of known kinds
   replay as usual. Unknown may-ignore flags are carried and ignored.
4. A record of an **unknown kind**, or with an **unknown must-understand
   flag**, is **quarantined**. Its framing and dependency scope are still
   readable.
5. A quarantined record holds back every later record in its dependency
   scope. They wait, shown in `afs pending` with the reason. They are never
   applied over a state their predecessor did not produce.
6. A record whose **payload** fails its checksum is quarantined the same
   way, its scope read from the common header.
7. If a **common header** fails its checksum, the dependencies cannot be
   known. Recovery stops for the whole journal, every record is kept for
   explicit recovery, and the health state says so. Nothing is replayed by
   guess.
8. That stop blocks a drain. An upgrade waits until explicit recovery
   clears it, or until the whole affected journal, with its format marker,
   is preserved verbatim in the recovery store and verified readable by the
   newer release.
9. Otherwise a drain before an upgrade is complete when nothing replayable
   remains. Quarantined records and the records they fence move to the
   recovery store, whose format later releases read.

Why: unknowns must never reorder recovery. Skipping an unreadable record
and applying its successors would store data over a state that never
existed; refusing the whole journal for one unknown record would hold
every unrelated write hostage.

### N34. Journal kinds

**Firm.** The kinds below and the rule that adds one. The reserved kinds
are named so their payloads are designed as kinds, not squeezed into an
existing one; each is built only with a profile that uses it.

1. Kinds in the first journal format: write, truncate, create, rename,
   remove, link, unlink.
2. Reserved as kinds, each with its own payload, each added as a minor
   version: setattr (mode, times, size); ACL set (the whole typed ACL, with
   the ACL version it was edited over); extended-attribute set and remove;
   a write or truncate to a named stream (stream selector, range);
   zero-range and punch; copy or clone (source file id and its data
   version, source and target ranges).
3. Operations that change nothing (a seek, a stream selector by itself)
   have no kind.

Why: a closed set of kinds makes every new operation a format break.

### N35. Reserved areas in a record, and what is not reserved

**Firm.**

1. **A precondition area**, empty under `afs3`, in the record and in the
   store path's argument structure: an expected base version and a lock
   fence token, for a profile with conditional stores and fenced locks.
   That AFS-3 has no conditional store is A9.
2. **A data-transform id** in the header, 0 for none: the id of an
   end-to-end transform (N44).
3. **An at-rest key id** in the header, 0 for none: the key of
   journal-at-rest encryption. The journal's own format marker holds the
   major and minor version and, when at-rest encryption is on, the at-rest
   suite; each record's key id names the key.
4. A record moved to the recovery store carries its originating at-rest
   suite and key id with it, so it stays decryptable after the journal it
   came from is gone.
5. The two encryptions never share framing.
6. **Not reserved:** an operation id and store-receipt fields. They arrive
   with a profile that specifies them, as a tagged payload extension
   behind a must-understand flag in a minor journal version (N33). The
   base data version is kept in the record.

Why: a field reserved before its semantics are specified is a guess that
later binds. The open record (N32, N33) makes the later addition a minor
version, so nothing is lost by waiting; the precondition area differs
because its place in the store path's arguments would be a redesign if
missing.

### N36. The principal is a typed value

**Firm.**

1. Identity is (namespace, issuer, name):
   - the **namespace**: Kerberos principal, AFS identity, anonymous
     authenticated, with room for others such as federated tokens;
   - the **issuer**: the realm, or the token issuer;
   - a variable-length **name**.
2. Equality is over all three. Two names from different issuers never
   alias.
3. With it goes a **context kind**: user session, machine, or service.
4. The value is used in context ids, cache namespace keys, journal
   records, IPC and audit.
5. **Anonymous authenticated identities** are a namespace of their own,
   distinct from rxnull, which stays the unauthenticated path (N1). Their
   shared name never makes two contexts one, and they are never eligible
   for the shared cache pool (C21).
6. A restricted token (one whose server-side rights are narrowed) carries
   that status into IPC and is never pool-eligible.
7. Token IPC reserves an optional key-handle field for sender-constrained
   (proof-of-possession) tokens, the key held by the daemon. It is empty
   now.
8. Tokens may state "never expires" `[DES-063]` and a renew-until time.

Why: an identity stored as a bare number or a bare name aliases as soon as
a second issuer appears, and the alias is a cross-user leak. The value is
in the journal and in IPC, so its shape cannot be widened later without a
format break.

### N37. The ACL value is open

**Firm.**

1. ACLs are a typed, versioned value. AFS-3's ACL is one variant. The
   client never evaluates an ACL to decide access (N28); it reads, shows,
   transports and edits it.
2. The value carries: an entry type (allow, deny, with room for audit and
   alarm); an entry flag word (inheritance flags, as in RFC 8881
   `[DES-065]`); an ordering flag; and
   ACL-level flags (automatic inheritance, protected).
3. The principal forms it represents: a numeric AFS id (signed), a named
   user, a group, a system group, a **special principal** (such as the
   object's owner or owning group), and an opaque form.
4. Answers are per object, and say when an object's ACL follows its
   directory.
5. **Unknown variants, entry types and flag bits round-trip unchanged**
   for reading and transport.
6. **An edit is refused** whenever the ACL holds an unknown variant, an
   unknown entry type, or an unknown must-understand flag, even if the raw
   values could be preserved. Unknown may-ignore bits are preserved and do
   not block an edit.

Why: an ACL the client can carry but not explain must not be edited
through it, because the client cannot show the user what the edit means.
Round-tripping keeps an older client from destroying what a newer server
stored.

### N38. Identifier widths

**Firm.**

1. **The data version is unsigned 64-bit** in every internal type, cache
   key, IPC field and journal field.
2. **Volume, vnode and uniquifier are typed 32-bit** under the `afs3`
   profile. They are distinct types, not bare integers.
3. **Legacy protection ids are typed signed 32-bit**, with checked
   conversions and tests for negative ids.
4. Principals are the typed value of N36, never a bare id.
5. The journal's dependency scope is protocol-scoped, length-delimited
   bytes (N32), so a wider file id in a later profile is a minor journal
   version.
6. No internal id is widened "to be safe". A wider id arrives with a
   profile whose published specification states its width.
7. **The stable object key** (C11; used for Windows placeholders and
   SMB file identity, L19) reserves a stream component
   (*none* means the main data) and a snapshot-view component (*none*
   means live), so a later named stream or same-volume snapshot never
   aliases its base object. Their encodings are fixed by the specification
   of the first profile that uses them.

Why: a 64-bit data version appears in published extension material
`[DES-057]`, so 64 bits is the width that cannot truncate. For the file
identifier we found no published wire specification that widens it (a
search, not a proof of absence), so any wider width chosen now would be a
guess; typed fields and a length-delimited scope make the later change
cheap instead.

### N39. A profile is recorded as class, level and suite

**Firm.**

1. A connection's profile is recorded as (class, level, suite). The suite
   is the encryption type actually negotiated, known once the security
   class completes its handshake.
2. The suite is *none* for a profile whose class negotiates no suite
   (rxkad). *None* is a valid value, not an unknown one.
3. rxnull is not a profile and has no suite value.
4. The triple appears in IPC status, in the cache namespace key (C18)
   and in the pin record (N11).
5. In IPC, the token message carries a token kind, and status carries the
   profile as class, level and suite, never as a boolean.

Why: a new encryption type needs a place in every store that names a
profile. Recording it from the first release costs a field; adding it
later is a format change to the pin store, the namespace key and IPC at
once.

### N40. No suite floor yet

**Working.** Revisit if: a supported rxgk profile can negotiate two suites
of materially different strength; or FIPS mode (N18) is scheduled.

1. The floor stays class and level (N1).
2. A suite floor is a later policy decision. N39 gives it a place.

Why: with no rxgk client path shipped there is no suite to choose between,
and a policy written before the choices exist would be untested.

### N41. Extension arms in control IPC

**Firm.**

C44 decides the IPC encoding and its compatibility rule. This is the rule
for growth inside it.

1. From the first release, these structures carry a version and a
   bounded, tagged, length-delimited extension list: attributes,
   set-attribute requests, the ACL and its entries, status, volume
   information and `statfs` results, and events.
2. Each extension is marked **critical** or not.
3. An unknown non-critical extension in a reply or event is carried and
   shown as unknown.
4. **A request with an unknown critical extension is refused whole**
   (not supported), never applied in part.
5. A sender never puts a critical extension in a reply or event to a peer
   whose handshake version predates it.
6. If one arrives anyway, the receiver treats the whole reply as failed
   ("this daemon is newer; upgrade `afs`") and the whole event as an
   unknown event. It never acts on the known part.
7. The adapter contract (C7, C44) uses the same shapes under its
   exact-version rule: it needs room to grow, not compatibility.
8. IPC passes an unknown error cause through as its code plus its text.

Why: partial application of a request the receiver half understands is
the failure to prevent; "refuse whole" is checkable. Bounded lists keep an
extension from being a memory attack.

### N42. Per-volume capabilities

**Working.** Revisit if: the first profile that offers one of these
features defines capability discovery at a different grain (per server or
per cell).

1. Beside N19's per-RPC capability words there is a capability value per
   (volume, the server serving it and that server's incarnation, profile),
   learned from that server.
2. It is an open set: holes, server-side copy and clone, atomic no-replace
   rename, extended attributes, named streams, case and normalisation
   rules, and their limits.
3. It is invalidated with the connection or incarnation (N19) and on a
   volume move or replica change.
4. A layer checks it **per operation**.
5. An OS-level flag that **promises an invariant** for the whole mount
   (for example a case-insensitive name space) is set only if it holds for
   every cell, and never for permission checks (L1, L4).
6. An OS-level flag that only **lets requests reach the daemon** may be
   set mount-wide. Each request is then checked against its volume's
   capability and limits, and refused per operation where it is absent.
7. POSIX ACL extended-attribute names are refused permanently, in the
   extended-attribute contract and in each layer's table, whatever the
   volume offers. The server's ACLs are reached only through the ACL value
   (N37).

Why: `/afs` spans every cell, so a mount-wide promise is a promise about
servers the client has not met. FUSE's POSIX ACL capability turns on the
kernel's own permission checks `[DES-026]`, which L4 never allows.

### N43. No permanent refusal for what a later cell may offer

**Working.** Revisit if: the kernel's handling of `ENOSYS` for these
requests changes so that it is no longer permanent for the mount.

1. On Linux FUSE, no operation that a volume capability (N42) could enable
   is answered `ENOSYS`.
2. Such a request is answered with a per-operation "not supported" error
   instead.

Why: `ENOSYS` for LSEEK or COPY_FILE_RANGE suppresses every later request
of that kind for the whole mount `[DES-031]`, and the mount covers every
cell.

### N44. Seams: data transform and crypto agility

**Working.** Revisit if: a client-side encryption or compression feature
is scheduled; or a supported platform's GSS library and a test KDC offer a
post-quantum key exchange.

1. **A data-transform seam** sits between wire bytes and cache bytes. It
   is empty today.
2. When a transform is active: status carries an opaque, bounded
   per-object envelope (in the status extension arm, N41) and both the
   logical and the stored size; the cache keeps the envelope in the file's
   status entry beside its data version, so cached chunks are never read
   without it; grants and chunk keys name the transform (N35's id).
3. **Crypto agility.** The crypto crate holds algorithms as registered ids
   and key lengths as data. Nothing assumes one key length or one
   key-exchange family.
4. Post-quantum readiness is conditional on the platform GSS library and
   the KDC supporting it, and on the encryption types, key lengths and
   policy IronAFS accepts. The client's part is to accept newly registered
   ones without a format change.

Why: neither feature is built. A seam costs an interface now; without it,
the cache's key and the status entry would need a format change the day
one is wanted.

### N45. Rekeying belongs to the security class

**Working.** Revisit if: the M2b rxgk profile record (N25) shows that
rekeying must be visible above the security-class interface.

1. Rekeying belongs to the Rx security-class interface, not to the crypto
   provider. A class may ask for a new key or a reconnect on a live
   connection.
2. **Key exhaustion is its own outcome**, distinct from a transport
   failure. It triggers a rekey or a reconnect at the same profile (N6).
3. A request not yet sent continues on the new key within its own
   deadline.
4. A mutation already sent whose outcome is unknown follows the lost-reply
   rule (C28). It is never resent transparently.

Why: the rxgk base draft gives a key lifetime and a byte limit as advisory
thresholds `[DES-059]`. If exhaustion looked like a transport failure, the
retry path could choose a different profile.

### N46. Reserved contract operations, and two baselines

**Working.** Revisit if: the M2a check below finds that the kernel or
common tools do not fall back when copy-range is unsupported; or a
supported cell offers one of the reserved operations.

1. Reserved in the layer contract (C7), refused with "not supported"
   until the volume's capability offers them: copy-range with a clone flag
   (an atomic clone only with the capability); allocate and punch;
   extended-attribute writes; the named-stream selector; rename flags
   (no-replace only with an atomic capability).
2. **Seek to data or hole** needs no capability. The core answers with the
   simplest policy lseek(2) allows: the whole file is treated as data.
   This claims nothing about how the server stores the file. Exact return
   values at the end of the file follow lseek(2) and are fixed in the
   layer specification.
3. **Copy-range without a capability** answers "not supported"; callers
   are expected to fall back to reading and writing. Whether the kernel
   and common tools do so is checked at M2a. If they do not, the core
   copies by reading and writing under the caller's own grants, as
   ordinary writes.
4. Reads of a hole return zeros. A hole-extent extension in read results
   is reserved in the extension arm, not built.

Why: an operation absent from the contract is a contract change when a
volume first offers it; one present and refused is a capability check.

### N47. Small fields that are cheap now

**Working.** Revisit if: a specification written for M2a or M2b shows one
of these fields cannot be populated from what an AFS-3 server returns.

1. A **status-change token** separate from the data version (N29).
2. The coherency state names the promise's **kind** (callback, lease,
   none) and its **scope** (status, data, with room for write-capable
   promises) as separate fields.
3. Invalidation events carry a change kind and their scope (object,
   directory, volume or server), and pair a rename's two names. An event a
   layer does not understand is handled conservatively at its known scope,
   and through the existing overflow path when its scope cannot be
   determined.
4. The transport is part of the connection key.
5. `TokenRevoked` is distinct from `TokenExpired`.
6. A creation time records whether the server supplied it. A layer
   synthesises one from the modification time only when the server gives
   none.

Why: each is one field or one enum arm in a structure N41 already makes
versioned. Each would otherwise be an inference a later profile proves
wrong.

## Consequences

- **A token's expiry is visible.** With N4, a user whose token expires
  gets `TokenExpired` or `TokenRequired`, not anonymous data. Programs run
  from AFS can stop, unlike a client that falls back to anonymous
  access.
- **Defaults refuse clear-text token use.** `min_level = "crypt"` (N3)
  means a cell that can only be reached at a lower level is refused until
  an administrator lowers the floor, with an audit record.
- **Pins, the legacy exception and the shared pool ship disabled** (N13).
  Until the combine call is assigned, an unprovisioned machine has no
  downgrade guarantee beyond the token rule (N14). Sites that need one
  provision the floor.
- **A cell that withdraws rxgk** keeps learned clients out for up to 30
  days after their last qualifying success, or until an administrator
  forgets the pin or policy turns learning off (N10).
- **rxgk is late.** It follows the Windows layer in the plan, and FIPS
  mode follows rxgk (N18, N25). Nothing is claimed as interoperable before
  a peer exists.
- **Extensions need a registrar's answer before a release** (N24). A slow
  answer delays the extension, not the client.
- **Costs carried now:** one capability query per new connection (N19);
  an extension list on long-lived IPC structures (N41); a common header
  and two checksums on every journal record (N32); typed identifiers and
  checked conversions throughout (N38).
- **Reserved now, built when their milestone comes:** the floor keys,
  `SecurityFloor`, `TokenRequired` and the marker (M2b); the state
  directory's version (M2a); the profile triple in IPC (N39); simulator
  servers that take a profile (classes, opcodes, capability words, combine
  answers), with N7's invariant.
- **Not built:** the pin store and the combine path before their gate; any
  extension; a negotiation RPC; DNS-published policy; per-server pins;
  per-user floors; an encryption-type list in configuration; a suite
  floor; client attestation (below).
- **Tests this part requires** (simulator unless noted; the test strategy
  is O45 to O48):
  - a forged abort, a suppressed rxgk negotiation, a dropped combine reply
    and a forged empty-token reply each end in failure, never in a lower
    profile;
  - a protected empty-token reply permits the legacy fileserver only under
    `cell-attested`, only with the companion rxkad credential, never for a
    database service, and not after the re-attestation interval;
  - `min_class = "rxgk"` defaults `legacy_fileservers` to `deny`; mixed
    replicas rank by profile;
  - an expired or revoked token, a restart and an authentication failure
    never yield anonymous data, and only `unlog` returns a context to the
    anonymous policy;
  - a database reply alone learns no pin; a combine reply with an empty or
    a non-empty token after a real negotiation does;
  - a pin survives a cache wipe, blocks rxkad until forgotten or expired,
    is not renewed by failures, cached replies or rxkad traffic, and
    survives a failed renewal write;
  - a production build makes no combine call and learns no pin;
  - unknown capability bits and callback opcodes change nothing; no
    extension is offered or accepted under rxkad or on a legacy
    connection;
  - a policy reader meeting an unknown security value refuses the cell;
  - journal: an unknown kind and a bad payload checksum quarantine and
    fence their scope; a bad header checksum stops recovery and blocks a
    drain; a higher minor version replays its known kinds;
  - ACLs: unknown variants round-trip, and an edit over one is refused;
    unknown rights bits grant nothing; negative protection ids convert
    with checks;
  - on a real OpenAFS 1.8 cell: the catalogue's fallbacks, and rxkad at
    the default floor.
- **Later work, each tracked as an issue in its milestone:** the
  specifications for the journal record format, the pin store, the
  security classes (including the re-attestation interval) and the
  callback service; the M2b rxgk profile record; the first registrar
  request, when an extension specification exists.

## Alternatives considered

- **Learned pins only, no configured floor.** Trust on first use gives an
  unprovisioned machine nothing on first contact, and nothing at all while
  the combine call is unassigned. Not taken; the floor is the primary
  control.
- **A configured floor only, no token rule.** A machine at the default
  floor would send an rxgk token holder's calls over rxkad after a forged
  failure. The token rule needs no state and closes that. Not taken.
- **Treating any authenticated reply as evidence, rxkad included.** It
  would let most of a pin's state be learned on today's cells. Not taken:
  IronAFS does not rely on rxkad for payload integrity (N5).
- **Per-server pins.** The cell answers per fileserver, fresh, over the
  protected channel; a per-server pin remembers a stale copy of that
  answer and blocks the legitimate rollback of one server. Not taken.
- **Per-context pins.** Every new login would be a first contact. Not
  taken.
- **Pins without expiry.** We found nothing in the specifications IronAFS
  builds from that lets a cell state a pin lifetime, so a cell's short trial of rxgk would lock
  clients out indefinitely. Not taken (N10).
- **Fileserver exception lists and per-service floors on clients.** An
  administrator would maintain fileserver UUIDs on every machine. The cell
  is the authority over its own servers (N12). Not taken.
- **A silent anonymous fallback when a token expires**, which keeps
  programs running. It returns different data without an error and hides
  the expiry from the user. Not taken (N4).
- **Shared, persisted capability observations**, saving one query per
  connection. They carry facts from weaker connections into stronger ones.
  Not taken (N19).
- **Shipping the combine path on a test number, or on a number IronAFS
  picks.** It would collide with a later assignment on deployed
  clients. Not taken (N24).
- **Building rxgk now against a self-written peer and calling it
  supported.** Conformance to one's own reading of two expired drafts is
  not interoperability. The simulator is kept as conformance evidence
  only (N25).
- **A negotiation RPC designed now.** No server exists to speak it. The
  rules are fixed (N22 to N24); the wire format waits for a peer.
- **Cell policy in DNS, DNSSEC or signed cell metadata.** We found no
  specification to build from, and it adds a trust path the project cannot test.
  Not taken; revisit under N14's trigger.
- **Per-user floors and encryption-type lists in configuration.** More
  policy surface than there are choices to make today. Not taken (N3,
  N40).
- **Widening every internal identifier to 64-bit or opaque now.** We
  found no published wider width for them, and every conversion to the 32-bit
  wire would need a check that can fail for no present benefit. Not taken;
  the data version, which has a published 64-bit form `[DES-057]`, is the
  exception (N38).
- **Reserving an operation id and store-receipt fields in journal records
  now.** Their semantics are unspecified, so the fields would be guessed
  and then frozen. The open record admits them later as a minor version
  (N35). Not taken.
- **A closed journal record: one fixed layout per release, converted on
  upgrade.** Every new operation would be a format break, and a rollback
  would meet records it cannot frame. Not taken (N32, N33).
- **Skipping records a release does not understand.** It applies later
  writes over a state that never existed. Not taken; they are quarantined
  and fence their scope (N33).
- **Identity as a numeric id, or as a name string.** Either aliases across
  issuers. Not taken (N36).
- **Editing an ACL that holds unknown parts, preserving the raw values.**
  The client cannot show the user what the result means. Not taken (N37).
- **Answering `ENOSYS` for operations AFS-3 does not offer.** Simple, and
  permanent for a mount that spans every cell `[DES-031]`. Not taken
  (N43).
- **Client attestation**: a server-required attestation of the client
  host, in RFC 9334's attester, verifier and relying-party roles
  `[DES-066]`. It would need a host key slot in the versioned state
  format, a request-signing seam at the choke point (N7), a verifier role,
  and a privacy decision, since either raw evidence or an appraisal result
  reveals something about the device. Not done; the four questions are
  listed in issue #22.
- **No readiness at all: design for AFS-3 only and migrate formats if a
  richer profile ever appears.** Cheaper to write. It makes the journal,
  the IPC structures, the namespace key and the pin store each a migration
  later, on machines that hold unstored user data. Not taken; readiness is
  kept only where it is free (N26).
