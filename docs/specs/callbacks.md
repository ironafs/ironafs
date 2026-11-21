<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The callback service: procedures and the allowlist

Status: draft (2026-11-21). Raised in issue #66. This is the first
version, written from documents only. Two revisions follow: one that
settles the allowlist entries marked "pending" against the test cell
(#47), and one that adds the request and reply layouts observed in the
observation lab's stage B record (#51) (section 13, Q1 and Q2).

Kind: protocol, with behaviour parts (5.6 to 5.8: the callback state,
the order of the service's steps, and assumption A14). Class: S. Any
host that can reach the client's UDP port can call this service without
authentication, its effects decide which cached data every user is
served, and it decides what the client sends back to an unverified
source ([README](README.md), "Review class per kind": a protocol
specification, and a behaviour specification touching callbacks and the
cache).

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (K1, X1, D1
and so on) are in section 12.

**This is the documents-only version.** No admissible document gives the
argument and result layouts of the callback procedures (section 12,
"What was not used"). So every layout point in 5.1 is MISSING or
UNVERIFIED, and no codec is written from it until the capture-backed
revision (section 13, Q1). What this version fixes is everything that
does not depend on the byte layout: which procedures are answered, which
are refused and with what, what each does to the cache's trust, the caps,
the hostile-input rule for every field, the order of the service's steps
against Rx's overload and pause, and the tests.

## 1. What this implements

ADR 0004, overview, constraint 4 (a constraint of the whole ADR):

> **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
> local IPC, configuration, requests from the operating system, and SMB
> messages before authentication. No panics, checked arithmetic and byte
> limits apply to all of it (coding standards rules 1 to 3).

ADR 0004, assumption A14 (an assumption, settled by this specification
and a test-cell test; section 5.8 states what the sources say):

> **A14. Callback continuity.** We assume an AFS-3 fileserver that cannot
> deliver a break stops treating this client as holding that callback,
> and that the client learns of the loss no later than its next call to
> that server, through a reset of its callback state or an error. No
> public source in the claim record states this.

ADR 0004, C6, points 1 and 4 (**Firm**):

> 1. Each protocol component is written from a behaviour specification in
>    `docs/specs/` that names its sources, under ADR 0001. No
>    specification, no code.
>
> 4. A fact about the AFS protocols that this ADR could not tie to a public
>    source is an input to a specification, never something to code from.

ADR 0004, C16, points 1 and 2 (**Firm**):

> 1. **Record first, notify after.** Break counters, staleness and
>    credential generations change in the core before any event is queued.
>    Every later operation sees the change, whether or not a layer has
>    applied it.
> 2. **A reply that fails its fence is never served.** A fetch records the
>    break counters of its object, volume and server when it is sent. If any
>    changed while it was in flight, the reply is discarded and the fetch
>    retried a bounded number of times, then fails with `Stale`. It is never
>    handed to the request that started it.

ADR 0004, C24, points 2, 3, 6 and 7 (**Firm**):

> 2. Anything weaker, including a reply protected only by a user's key and
>    everything the callback service receives before rxgk (C31), may
>    **suspend** callback promises, revoke grants and mark entries suspect.
>    A suspect entry is not delivered until its reader has revalidated it
>    under the reader's own credentials.
> 3. Weak evidence never evicts another context's data or the pool's,
>    never retires their handles (save point 6's purge-failure case) or
>    journal records, and never establishes
>    that two contexts see the same volume incarnation.
>
> 6. **What weak evidence may do across contexts** is this closed list:
>    suspend callback promises, revoke grants, mark entries suspect, and
>    purge operating-system caches (C16, point 4; C17). […]
> 7. **Callback trust is raised only by a reply to the client's own call.**
>    Before rxgk, a server earns callback trust (C32) only from its reply to
>    a call the client made, matched by that call's random identity.
>    Nothing the callback service receives, probe or break, raises it; such
>    input may only lower trust.

ADR 0004, C26, points 2, 3 and 5 (**Working**):

> 2. Callback expiry is counted from when the request was sent, on the boot
>    clock (C37).
> 3. Callbacks never persist across a restart.
>
> 5. The client's host UUID persists.

ADR 0004, C30, point 1 (**Working**):

> 1. The client uses one UDP socket per address family, on port 7001 by
>    convention `[DES-011]`, for outgoing calls and for the callback service.

ADR 0004, C31 (**Working**), the whole decision; section 5.6, point 9,
restates its table against this specification:

> Before rxgk, nothing the callback service receives is authenticated. One
> table governs all of it:
>
> | Received | Effect |
> |---|---|
> | A break naming files, from a known server's address | those files' promises are suspended and their entries marked suspect in every context (C24, point 2); each reader revalidates on its next use. Nothing is evicted |
> | A break naming files, from an address matching no known server | the same, for the named files only |
> | A break or reset naming no files | every promise from the servers that source could be moves to a new suspect epoch; affected entries are suspect until their reader revalidates them |
> | More than the rate limit can process | the suspect epoch of the affected servers (of every server, if the source is unknown) advances **before** work is dropped or acknowledged; drops are counted |
> | A probe | answered per the allowlist (point 4); never raises callback trust (C24, point 7) and is never authority over any data |
>
> 1. A break changes the core's counters before the reply goes back, and
>    never waits on a layer (C16).
> 2. **A break from a known server is never discarded.** Over that server's
>    rate limit, it marks the server's callbacks suspect instead.
> 3. A break's source is matched to a server through the addresses the
>    volume location database lists `[DES-058]` for servers the client holds
>    callbacks from.
> 4. The service answers only an allowlist of callback procedures, drawn
>    from published protocol material and tests against the test cell.
>    Remote debugging queries get no data (O15).

ADR 0004, C32, points 1 to 5 (**Working**):

> 1. Callback trust is kept per server, address family and local endpoint
>    generation. It is earned only by a reply from that server to the
>    client's own keepalive call (point 4), received on the client's
>    callback socket (C30), no older than **E**, and since the last network
>    change or resume. […] Nothing the callback service
>    receives raises trust (C24, point 7).
> 2. Without fresh evidence the server is in **bounded revalidation**: its
>    data is served only if its status was validated within the last **T**
>    seconds (we propose 30 to 60). Otherwise it is validated before
>    delivery. If that validation cannot complete, the request fails at its
>    deadline; stale data is not served.
> 3. With trust, cached data is served while its callback is valid and the
>    credential and grants are valid, with no further cap since last
>    contact. A break, or a known loss of callback continuity, ends that.
>    A loss is known when a call to that server shows it: a reset of the
>    client's callback state, or an error. This branch rests on assumption
>    A14 (callback continuity). If the specification gate refutes A14, the
>    uncapped branch does not ship: data under trust is served only within
>    a revalidation cap, whose length is then a project decision, and this
>    decision is reopened.
>
> 5. **Resume is checked when data is served,** not only on a notification
>    from the operating system. Each serve compares the clocks of C37, and a
>    jump marks every callback suspect before the answer is given.

ADR 0004, C34, points 2 and 6 (**Working**):

> 2. The callback service listens on both families.
>
> 6. A callback that arrives from a translated address the client cannot
>    match to a server (C31, point 3) is handled as one from an unknown
>    address: a break still suspends the files it names. Trust comes from
>    replies on the client's own flow (C32, point 1), so an unattributable
>    translated source affects only whether breaks are received, not
>    whether a server is trusted. […]

ADR 0004, C39, point 3 (**Working**), the sentence that applies:

> Every queue is bounded in bytes.

ADR 0004, C46, point 3 (**Working**):

> 3. **An AFS abort code is classified by the RPC and the server that
>    returned it, never by its number alone.**

ADR 0004, O15 (**Working**), points 1 to 5:

> 1. The client offers no remote debug service and no setting to enable one.
> 2. The callback service on the client's port `[DES-011]` answers an
>    allowlist of calls: callback delivery, state management, probes, and the
>    identity, capability and address discovery that supported fileservers
>    require. The allowlist is written from published protocol material and
>    clean-room interoperability tests (ADR 0001).
> 3. Legacy Rx diagnostic and version requests are dropped without a reply.
> 4. A valid, parsed but unsupported callback call gets the documented
>    unsupported-operation response when protocol state and the reply budget
>    permit. Malformed, incomplete or rate-limited traffic is discarded
>    silently.
> 5. Parsing, allocation, response size and rate-limit state are bounded.
>    Required callback traffic stays serviceable under diagnostic load.
>    Refusals and drops are counted in aggregate.

ADR 0004, N5, points 2 and 3 (**Firm**):

> 2. Everything else is a **hint**: DNS answers, rxnull replies, rxkad
>    replies, Rx aborts, timeouts, capability words read without that
>    protection, and anything `aklog` reports.
> 3. A hint may lower trust, or choose between RPC variants the catalogue
>    lists as equivalent (N20). A hint never raises or satisfies a floor,
>    never learns or renews a pin, and never authorises an exception.

ADR 0004, N21 (**Working**):

> 1. The client's reply to a server's baseline capability query on the
>    callback service lists only the baseline AFS-3 capabilities and the
>    approved third-party ones (N23) this build implements and has enabled,
>    the same to every server. IronAFS extensions are never listed there;
>    they are advertised only on a protected rxgk channel (N22, point 2).
> 2. An unknown callback opcode is answered with the abort the callback
>    service's specification names, is counted, and changes no state.

ADR 0004, N22, points 1 and 7 (**Firm**):

> 1. An IronAFS-defined extension is offered, accepted and used only on the
>    approved rxgk profile at `auth` or `crypt` level, in both directions,
>    callbacks included, with no exception.
>
> 7. **Extended callbacks** may narrow what is refetched only when they
>    arrive on the protected channel of point 1. One that arrives without it
>    is not accepted as an extension (point 1): its extension part is never
>    used, and it is acted on only as a plain break of the objects it names,
>    which can only lower trust (N5, point 3; C31, C32).

ADR 0004, N29 (**Working**):

> 1. Callback kinds form a registry that maps each kind to the grants it
>    revokes.
> 2. Revocation has an ordering and an expiry bound that do not depend on
>    the data version: the client does not assume that an ACL or status
>    change moves the data version.

ADR 0004, N31, points 1 and 2 (**Firm**):

> 1. A wire fact is taken only from an allowlisted specification passage or
>    an independently generated black-box fixture, recorded with its
>    edition, section or fixture provenance.
> 2. Never from implementation history, interface-definition files,
>    generated definitions, or live-cell captures holding credentials or
>    private data.

[ADR 0006](../adr/0006-rx-unprotected-packets.md), decision 1 (**Firm**),
points 6, 8 and 9, the sentences that bind this service:

> 6. **It may lower trust, and only so,** and only for an ABORT on a
>    connection that meets point 9's condition; […]
>    - the one shared effect: callback trust for that server, address family
>      and local endpoint generation (C32, point 1's key) moves to bounded
>      revalidation (C32, point 2), the one transition C32, point 3, allows
>      on an error. […]
> 8. **On an incoming callback call** (the client is the acceptor and the
>    remote side chose the identity), the lookup is exact: […] A matching
>    ABORT is counted and stops only the client's reply and its
>    retransmissions:
>    - once the request is complete, its C31 effects apply on arrival (C31,
>      point 1) and stand; an ABORT never suppresses, delays or undoes them,
>      and a break from a known server is never discarded (C31, point 2);
>    - an incomplete request is not cut short: […]
>    - a BUSY on an incoming call changes nothing but a counter.
>    Neither raises callback trust.
> 9. **Precondition for points 5 and 6.** […] On any other connection a
>    matching ABORT is a counter-only hint: it has none of point 6's
>    effects, […]

ADR 0006, decision 1, point 5, the item of its last list that binds this
service (**Firm**): a matching call-level ABORT never "advances a suspect
epoch".

ADR 0006, decision 2, points 3 and 5 (**Working**), the first sentence of
each and the hold of point 5:

> 3. **ABORT and BUSY on an incoming call (beside C31).** Decision 1, point
>    8, applied to C31's table: […]
> 5. **ABORT as "an error" (changes C32, point 3).** An unprotected ABORT
>    is "an error" in C32, point 3, only as decision 1, point 6, allows: […]
>    a server is held in bounded revalidation (C32,
>    point 2), never in C32, point 3's uncapped branch, while any connection the
>    client uses to it fails decision 1, point 9's condition, whether or not
>    any packet arrives on it. When that hold ends, these callback promises
>    from that server stay suspended (C24, point 2) until each entry is
>    revalidated by a call sent after the hold ends: every promise held at
>    any time during the hold, and every promise granted by a reply to a
>    call sent before the hold ended, on any connection and whenever that
>    reply arrives […].

From [rx.md](rx.md) (a specification this one rests on), the parts it
leaves to the callback service: the confirmation of each `Overload` event
only after the suspect epoch of the affected servers has advanced (rx.md,
5.10, point 5); the pause generation as part of the suspect epoch of
every server on its socket, compared on every serve (rx.md, 5.10, point
6, and 5.12, point 5); and ADR 0006's test 3's hold and exit, which the
callback service's tests carry (rx.md, 10.5). Rx's review record names
two items left here as well: the policy that confirms overloads and so
ends a pause, and repeated revalidation under a sustained flood from one
source. Sections 5.6 and 5.7 carry all of them.

Section 9 shows where each invariant is kept and which test fails if it
is broken.

## 2. Scope

1. **Covered:**
   - the service's registration with Rx on each family's socket, and what
     Rx drops before the service sees a call (5.1.1);
   - the allowlist: every procedure the service answers, the role and
     source of each, and the status ("listed" or "pending") of each
     (5.1.2);
   - the unsupported-operation reply, and what is discarded without a
     reply (5.7);
   - the fields of each allowlisted call that IronAFS reads, their caps
     and the hostile-input rule for each (5.1.3, 5.2, 5.4), and the
     replies IronAFS builds (5.5);
   - the callback state the client keeps per server, address family and
     local endpoint generation, and every input that lowers it (5.6);
   - C31's table restated against this specification (5.6, point 9);
   - the order of the service's steps: effects before replies, Rx's
     overload events, the advance tick and the pause generation (5.6,
     5.7);
   - ADR 0006's callback points: an ABORT or BUSY on an incoming call, an
     ABORT as "an error", and the hold and its exit (5.6);
   - what the sources say on assumption A14 (5.8);
   - the counters and events the service emits (5.9).
2. **Not covered, and where it is covered:**
   - Rx itself, including admission, the caps in bytes per peer and in
     total, the overload event, the hold until confirmation, the pause
     generation, the unverified-source budget and what Rx drops itself:
     [rx.md](rx.md). This specification uses Rx's interface and never
     restates its rules;
   - the keepalive call, how its reply is matched, how callback trust is
     earned and server time: the keepalive specification (#81). This
     specification defines the callback state record (5.6, point 1) and
     every input that lowers it; #81 defines the one input that raises it;
   - the file-service calls that grant callback promises, and the file id
     and promise records as they arrive in replies: the VL and FS read
     specification (#65), which hands the promise words to this service
     unread (its 5.1.3, point 3). This specification gives them their
     meaning (5.1.3, point 3);
   - the node table, the break counters, entry validation and the serve
     path that compares this specification's tuple: the cache (C16, C26;
     #91). This specification states what the cache must compare and
     when (5.6, points 3 to 5);
   - the invalidation stream to the layers (C17): the contract
     specification (#67). A break records its counters in the core and
     never waits on a layer (C31, point 1);
   - Rx's own diagnostic and version packets: dropped by Rx ([rx.md](rx.md),
     5.4, point 4), as O15, point 3, requires;
   - the host UUID's storage and creation (C26, point 5; O42, point 5):
     the state directory's specification, not yet written (section 13,
     Q9). This service only reads it;
   - rxgk-protected callbacks and extended callbacks over a protected
     channel: M6, under N22 (section 11);
   - the wire time-field table (C38): #82, which takes this
     specification's one time field (5.1.4).

## 3. The design in short

1. The service is Rx service 1 on both of the client's UDP sockets, for
   the null security class only (C30, C31, C34). Rx drops every call for
   another service id or class, and every Rx debug or version packet,
   without a reply.
2. A closed allowlist names the procedures the service answers: callback
   delivery (the extended form only as a plain break), three reset
   variants, a liveness probe and two identity queries. One more, a UUID
   probe, is pending the test cell and refused until then. Every other
   procedure number, the diagnostic ones included, is refused with one
   abort code, within a refusal budget, and changes nothing.
3. Breaks only lower trust. A break naming files suspends those files'
   promises in every context; a reset, a break naming no file, a
   malformed break from a known server, and break work over the rate
   limit move the server to a new suspect epoch. Probes never do. The effect is recorded in the core before the reply leaves, and
   stands whatever happens to the reply.
4. The client keeps, per server, address family and local endpoint
   generation, a suspect epoch (from one client-wide counter that never
   repeats), a trust state and a hold. A cached entry records its fence
   tuple when its validating call is sent: the object's and volume's
   break counters, the server's suspect epoch, the socket's pause
   generation and a resume generation. The serve path compares the C37
   clocks, then every value of the tuple, on every serve, so a break
   during a fetch, an Rx pause, or a sleep fences the entry before
   anything stale is served.
5. Work the service cannot do now (Rx's overload events, breaks over a
   server's rate limit) waits for one **advance tick**: at most once per
   second, the service advances every affected suspect epoch, and only
   then confirms the overloads and acknowledges the calls. So no work is
   dropped or acknowledged before the epoch moves, and a sustained flood
   moves any one epoch at most once a second.
6. Identity and probe calls are answered only to addresses the client is
   talking to; they carry the host UUID, no addresses and no capability
   bit. Nothing the service receives raises trust.
7. Every state the service keeps is a fixed table, a field of a record
   the cache already holds, or one record per live incoming call, which
   Rx's own caps bound.

## 4. Terms

- **The service:** the callback service, an Rx service the client offers
  on each family's socket (C30, point 1; C34, point 2).
- **Known server:** a fileserver the client holds a callback state record
  for (5.6, point 1). A record exists from the first call the client sends
  to that server on that family until the record retires (5.6, point 10).
- **A server's addresses:** the addresses the volume location database
  lists for it (C31, point 3), and every address the client itself sends
  that server's calls to on that family (the NAT64-synthesised address of
  C34, point 5, for example) `[OWN]`, because a callback comes from the
  address the client's own calls reach.
- **The servers a source could be:** the known servers one of whose
  addresses equals the source address of the call, on the receiving
  family. A source that is none of them **matches no known server**.
- **Unknown source:** a source Rx could not name: an `Overload` event
  whose source is "unknown", and a pause ([rx.md](rx.md), 5.10, points 4
  and 6). An address that matches no known server is not an unknown
  source; it is a source that matches no known server.
- **Server key:** a server, an address family and the local endpoint
  generation of that family's socket: C32, point 1's key. Written K.
- **Callback state record:** the per-K record of 5.6, point 1.
- **Suspect epoch:** the per-K counter of 5.6, point 1. It is the
  server's break counter of C16, point 2.
- **Pause generation:** the per-socket counter Rx's driver raises before
  it stops reading and again before it reads after a pause
  ([rx.md](rx.md), 5.12, point 5).
- **Resume generation:** a client-wide counter the serve path raises when
  its comparison of the C37 clocks detects a suspend or a jump (5.6,
  point 4).
- **Fence tuple:** for an object of server key K, the values (the
  object's break counter, its volume's break counter, the suspect epoch
  of K, the pause generation of K's family's socket, the resume
  generation). The local endpoint generation is part of K itself.
- **Recorded tuple:** the fence tuple a validating or fetching call
  recorded for its object when it was sent, kept with the entry it
  validates.
- **Promise:** the callback promise a fileserver grants with a reply
  (#65's 5.1.3, point 3), keyed by context and file.
- **Break-class call:** a call to CallBack, ExtendedCallBack or one of
  the three reset procedures (5.1.2). **Identity-class call:** a call to Probe, ProbeUuid,
  WhoAreYou or TellMeAboutYourself. **Refused call:** any other call.
- **Advance tick:** the step of 5.6, point 7, that advances pending
  suspect epochs and then confirms and acknowledges the work waiting on
  them.
- **Work unit:** the cost the rate limits count: one per call, plus one
  per file id the call names (5.2).
- **Hint:** an input that may lower trust and is counted, never evidence
  (N5, point 2).

## 5. Specification

### 5.1 Layout

#### 5.1.1 Framing and registration

1. **A request** is the procedure number as a 32-bit integer, then the
   procedure's arguments, XDR-encoded, carried as an incoming Rx call's
   request bytes `[X1 "RPC Layer": UNVERIFIED]`. **A reply** is the
   procedure's results, XDR-encoded, as the call's reply bytes
   `[X1 "RPC Layer": UNVERIFIED]`. A refusal is an Rx ABORT with a 32-bit
   code ([rx.md](rx.md), 5.7, point 15). The procedure numbers used here
   are all below 2³¹, so the signedness of the number on the wire does
   not change them.
2. **Service and port.** The cache manager's callback service is Rx
   service 1, on UDP port 7001 by convention `[K1 rxservice.txt:
   UNVERIFIED]` `[K2 §1: UNVERIFIED]` `[DES-011]`. The client registers
   service 1 on each family's socket ([rx.md](rx.md), 5.10, point 1)
   `[ADR 0004 C30.1, C34.2: Working]`, with:
   - security index 0 only `[ADR 0004 C31: Working]`; Rx drops a call for
     any other index without a reply (`bad_security_index`);
   - request cap `cb.request_max` and reply cap `cb.reply_max` (5.2).
3. **What never reaches the service.** Rx drops, without a reply and
   counted under its own reasons ([rx.md](rx.md), 5.4 and 5.10, point 1):
   - a call for any service id the client does not register. Among them
     are the registry's Rx statistics service (409) and an extension
     callback service of another product (2501) `[K1 rxservice.txt:
     UNVERIFIED]`; IronAFS registers neither `[OWN]`, since both describe
     the client's state or extend the protocol outside N22 and N23;
   - Rx debug and version packets (O15, point 3);
   - every packet that matches no connection and opens none.
4. **The procedure number** is the first 4 bytes of the request. A
   request shorter than 4 bytes has no procedure number: it is discarded
   (5.7, point 3).

#### 5.1.2 The allowlist

The allowlist is closed: a procedure number not in this table is refused
(5.7, point 1). Numbers and names are from the registry's procedure table
for the cache manager's callback interface `[K1 rx/RXAFSCB.txt]`; no
passage of the registry is allowlisted for wire facts yet (section 13,
Q3), so every number is UNVERIFIED until a capture shows it. None is
taken from an interface-definition file or a generated definition (ADR
0004, N31, point 2). The "Registry source" column is the registry's own
note of where each number came from.

**Status.** "Listed": a registered document states what a fileserver uses
the procedure for, and IronAFS answers it. "Pending": only the registry
names it, and whether a supported fileserver requires an answer is
settled at the test cell (C31, point 4; section 13, Q2). Until its row is
settled in the revision that waits on #47, a pending procedure is
refused with the unsupported-operation abort, exactly as a number off the
list (5.7, point 1), and no handler for it is written (C6, point 4).

| IronAFS name | Registry name | Number | Registry source | Role (O15, point 2) | What the service does | Status | Label |
|---|---|---|---|---|---|---|---|
| `cb.callback` | CallBack | 204 | IBM AFS 3.0 | callback delivery: the server invalidates the client's cached copies of the files it names `[K3 §3.1: UNVERIFIED]` | a break naming files (5.6, point 9, rows B1 to B4) | listed | `[K1 rx/RXAFSCB.txt: UNVERIFIED]` |
| `cb.init_state` | InitCallBackState | 205 | IBM AFS 3.0 | state management: the server resets the callback state it holds for the client `[K2 §6.1: UNVERIFIED]` | a reset (5.6, point 9, rows R1 and R2) | listed | `[K1 rx/RXAFSCB.txt: UNVERIFIED]` |
| `cb.init_state2` | InitCallBackState2 | 211 | IBM AFS 3.5, marked deprecated by the registry | as `cb.init_state` `[K2 §6.1: UNVERIFIED]` | as `cb.init_state` | listed | `[K1 rx/RXAFSCB.txt: UNVERIFIED]` |
| `cb.init_state3` | InitCallBackState3 | 213 | IBM AFS 3.5 | as `cb.init_state` `[K2 §6.1: UNVERIFIED]` | as `cb.init_state` | listed | `[K1 rx/RXAFSCB.txt: UNVERIFIED]` |
| `cb.probe` | Probe | 206 | IBM AFS 3.0 | probe: by its name, a check that the client answers; K2 names WhoAreYou as its successor for a client with several addresses `[K2 §5: UNVERIFIED]` | answered with an empty result, to a known server only (5.5, point 2) | listed | `[K1 rx/RXAFSCB.txt: UNVERIFIED]` |
| `cb.who_are_you` | WhoAreYou | 212 | IBM AFS 3.5 | identity and address discovery: a probe that also reports the client's interface addresses, for a client with several addresses `[K2 §5: UNVERIFIED]` | answered with the host UUID and an empty address list (5.5, point 3) | listed | `[K1 rx/RXAFSCB.txt: UNVERIFIED]` |
| `cb.tell_me` | TellMeAboutYourself | 65538 | (names an implementation's change history; not used) | identity, address and capability discovery: the same addresses as WhoAreYou, and a capability vector `[K2 §5: UNVERIFIED]` `[K1 cap/client.txt: UNVERIFIED]` | answered with the host UUID, an empty address list and no capability bit (5.5, points 3 and 4) | listed | `[K2 §5: UNVERIFIED]` |
| `cb.ext_callback` | ExtendedCallBack | 65543 | a registration request (`matt (#124161)`) | callback delivery with extension data: a break of the files it names, with details of the change `[K3 §4.2.1: UNVERIFIED]` | a plain break only (N22, point 7): its extension part is never used. Until the layout is captured (Q1), its arguments are not read and it is acted on as a break naming no file IronAFS can read (5.6, point 9, rows X1 and X2) | listed | `[K1 rx/RXAFSCB.txt: UNVERIFIED]` |
| `cb.probe_uuid` | ProbeUuid | 214 | IBM AFS 3.5 | a probe naming a UUID; its meaning is MISSING (section 13, Q2) | refused with −455 until the test cell settles it (5.5, point 5) | pending | `[K1 rx/RXAFSCB.txt: UNVERIFIED]` |

`[OWN]` The registry's names are used only to identify the procedure;
the IronAFS names in the first column are the ones code and tests use.

Notes on the table:

1. **TellMeAboutYourself's number** is taken from the capabilities draft,
   which gives it as 65538 `[K2 §5: UNVERIFIED]`. The registry's row for
   65538 agrees, but its source column names an implementation's change
   history, so that row is not used as a source (README, "Sources and
   provenance", rule 14; the same treatment as the VL and FS read
   specification's registry rows, #65).
2. **ExtendedCallBack** is listed although IronAFS never advertises
   extended callbacks (5.5, point 4), because N22, point 7 (Firm), acts
   on one that arrives without rxgk "only as a plain break of the objects
   it names", and C31, point 2, never discards a break from a known
   server. Its extension part is never used. Until its layout is captured
   (Q1), no part of its arguments is read, so it is acted on by the
   widest plain break its source allows: an advance of every key of the
   servers the source could be (row X1), which covers every object it
   could name. Once the layout exists, the revision reads only its list
   of file ids and acts as row B1.
3. **InitCallBackState2** is kept although the registry marks it
   deprecated `[OWN]`: a reset that is refused is a reset the client never
   learns of, while answering one costs a suspect epoch at most (5.6,
   point 9, row R1). Its arguments are never read (5.1.3, point 4).
4. **Not allowlisted, and refused (5.7, point 1):**
   - every number the registry assigns to a diagnostic query of the cache
     manager: 207 to 210 and 215 to 218, and 65536, 65537 and 65539 to
     65542 `[K1 rx/RXAFSCB.txt: UNVERIFIED]`. They describe the client's
     locks, cache entries, statistics, configuration and cells; O15,
     points 1 and 6, give remote tools no such data. The classification
     is IronAFS's own, from the registry's names `[OWN]`; it changes
     nothing, since every non-allowlisted number is refused alike;
   - 65544, an asynchronous byte-range lock, an extension
     `[K1 rx/RXAFSCB.txt: UNVERIFIED]` that is not a break. IronAFS never
     advertises it (5.5, point 4; N21, point 1), so a conforming server
     never sends it; one that arrives is an unknown opcode (N21, point 2);
   - 65540. The extended-callback draft gives its procedure that number
     `[K3 §4.2.1: UNVERIFIED]`, while the registry assigns 65540 to a
     diagnostic query and the extended callback to 65543 `[K1
     rx/RXAFSCB.txt: UNVERIFIED]`. IronAFS follows the registry `[OWN]`
     (README, "Sources and provenance", rule 7): a number has one meaning
     on the wire, the registry records the assignment, and IronAFS never
     advertises extended callbacks, so a server has no reason to send the
     draft's number to it. Capture evidence of a server sending a break
     at 65540 is a trigger (section 11);
   - the numbers below 204 and from 219 to 65535, which the registry
     reserves, and every unassigned number.

#### 5.1.3 Fields IronAFS reads

The order, width and signedness of every field below are MISSING unless
a point says otherwise: no admissible document gives them (section 12,
"What was not used"). The capture-backed revision fills them in from raw
bytes, varying one input at a time (section 13, Q1).

1. **CallBack's arguments** name the files invalidated and carry one
   callback promise record for each `[K3 §3.1: UNVERIFIED]`. That the
   files are named by a list of file ids, beside a list of the promise
   records, is IronAFS's reading, MISSING until a capture shows it (Q1);
   so is whether the two lists must have equal lengths.
   - **File id:** as the VL and FS read specification decodes it (#65,
     its 5.1.3, point 1): a volume id, a vnode number and a uniquifier,
     each in its own type `[ADR 0004 N38.2: Firm]`. How a break names a
     whole volume rather than one file is MISSING (Q1); 5.4, point 6,
     says how IronAFS reads a break on a volume it holds a whole-volume
     promise for.
   - **Promise records in a break:** decoded for their bounds only; no
     value is used `[OWN]`. A break can only suspend (C24, point 2; C31's
     table), so nothing a break carries can grant, extend or shorten a
     promise, and reading a value would be a use with no effect allowed.
2. **The promise record in a fileserver's reply** (handed here unread by
   #65): three words, the promise's expiry and two further values
   `[K3 §3.1: UNVERIFIED]`. IronAFS reads the expiry (5.1.4) and records
   the other two words unread. Which values of the other two words mean
   that no promise was granted, or a promise of a different scope, is
   MISSING (Q1): until the capture-backed revision settles it, no code
   treats any reply as granting a promise (C6, point 4).
3. **A read-only volume's promise** covers the whole volume, not one
   file, and any update to the replica breaks it for the whole volume
   `[K3 §4.5: UNVERIFIED]`. So a promise granted for a file in a
   read-only or backup volume is held as a promise on the volume (5.4,
   point 6) `[OWN]`, until a capture shows a server granting per-file
   promises there (Q1).
4. **The reset procedures' arguments** are never read `[OWN]`. A reset's
   effect depends only on its procedure number and the call's source
   (C31's table, row 3; C31, point 3): an identity a reset might carry
   is unauthenticated and can be no evidence (C24, point 7; N5, point 2).
   They are bounded by the message cap (5.2) and otherwise not decoded,
   so a reset whose layout IronAFS has not yet seen still acts.
5. **Probe, WhoAreYou and TellMeAboutYourself** carry no arguments IronAFS
   reads `[OWN]`; whether they carry any is MISSING (Q1). Any argument
   bytes are bounded by the message cap and not interpreted.
6. **ProbeUuid's argument** is a UUID, by the procedure's name only
   `[K1 rx/RXAFSCB.txt: UNVERIFIED]`; its encoding is MISSING (Q1, Q2).

#### 5.1.4 Time fields

The service reads one time field. #82 owns the wire time-field table and
takes this row (C38, point 2).

| Field | Direction | Width | Signedness | Epoch | Boundary and refusal |
|---|---|---|---|---|---|
| a promise's expiry, in a fileserver's reply (5.1.3, point 2) | in | MISSING (Q1) | MISSING (Q1) | MISSING: whether it is a duration or a time on the server's clock (Q1) | `[OWN]`: a value that converts to a lifetime of 0 or less grants no promise; a lifetime above `cb.promise.lifetime_max` is cut to it; the lifetime is counted from when the request was sent, on the boot clock `[ADR 0004 C26.2: Working]`. A value the conversion cannot represent grants no promise |

The promise records inside a break (5.1.3, point 1) carry the same
field; it is not read.

### 5.2 Caps

Each cap is IronAFS's own `[OWN]`, with its reason, in the columns of
[xdr.md](xdr.md), 5.2, point 5. "Declared bound" is MISSING for every
field: no admissible document states one. The capture-backed revision
records each declared bound found by observation and lowers a cap that
exceeds it (xdr.md, 5.2, point 4).

| Cap name | Field | Kind | Declared bound | Cap | Direction | Memory budget | Secret | Why |
|---|---|---|---|---|---|---|---|---|
| `cb.callback.request` | message | message | MISSING | 65 536 bytes | in | 65 536 bytes | | the most Rx delivers for this service (`cb.request_max`); a larger break is an overload, which moves the server's suspect epoch instead (5.6, point 9, row O1) |
| `cb.callback.request.fids` | file ids | array | MISSING | 32 760 bytes | in | — | | half of the message less the number and two count words (65 524 bytes), rounded down to a multiple of 4 |
| `cb.callback.request.promises` | promise records | array | MISSING | 32 760 bytes | in | — | | as the file ids; decoded for bounds only (5.1.3, point 1) |
| `cb.ext_callback.request` | message | message | MISSING | 65 536 bytes | in | 0 bytes | | as `cb.callback.request`; not decoded until its layout is captured (5.1.2, note 2) |
| `cb.init_state.request` | message (each of the three resets) | message | MISSING | 4 096 bytes | in | 0 bytes | | the arguments are not decoded (5.1.3, point 4); 4 KiB is far above a number, an identity and an address list |
| `cb.identity.request` | message (each identity-class call) | message | MISSING | 256 bytes | in | 64 bytes | | a number and at most one UUID |
| `cb.identity.reply` | message (each identity-class reply) | message | MISSING | 1 024 bytes | out | — | | a UUID, an empty address list and one capability word (5.5) |

Other limits `[OWN]`:

| Name | Value | Applies to | Why |
|---|---|---|---|
| `cb.request_max` | 65 536 bytes | the request cap the service registers with Rx | `rx.in.request_ceiling` ([rx.md](rx.md), 5.2): the largest break a server sends is MISSING (Q1), and a break above the cap costs the whole server's suspect epoch, so the cap is as high as Rx allows; Rx's per-peer and total caps bound the memory |
| `cb.reply_max` | 1 024 bytes | the reply cap the service registers with Rx | the largest reply is `cb.identity.reply`; a small reply cap leaves Rx's per-peer budget to requests |
| `cb.rate.key_units` | 4 096 work units per second | the rate at which one server key's break-class calls are processed at once; identity-class calls are not charged (5.6, point 8) | a work unit is one file id applied in one cell: one indexed lookup of that file id in the node table, and one counter change per context that holds it, so its work is bounded by the cache's count of contexts (#91); the genuine rate of a server's breaks is MISSING (Q4) |
| `cb.rate.key_burst` | 8 192 work units | the bucket size for one server key | holds one break at `cb.callback.request`'s largest (2 731 units if a file id is 12 bytes) with room for more |
| `cb.rate.total_units` | 32 768 work units per second, burst 65 536 | all break-class calls from known servers together | bounds the callback thread's time per second, so breaks from one server cannot starve the others or the service's other work |
| `cb.unmatched.table_entries` | 256 sources | the per-source table for sources that match no known server | a fixed table, as Rx's drop table ([rx.md](rx.md), 5.9, point 4); one aggregate entry beyond it |
| `cb.unmatched.units` | 1 024 work units per second per source, burst 4 096; 4 096 per second in total, burst 8 192 | break-class calls from sources that match no known server | such a break only suspends the files it names (5.6, point 9, row B2), so a low rate costs little; beyond it, the call is discarded (row B4) |
| `cb.refuse.table_entries` | 256 sources | the per-source table for refusals | a fixed table; one aggregate entry beyond it |
| `cb.refuse.rate` | 10 aborts per second per source, burst 20; 100 per second in total, burst 200 | the unsupported-operation aborts the service sends (5.7, point 1) | a refusal is 32 bytes, no larger than the smallest request that can carry a procedure number, so it is no amplifier; the rate makes the client a negligible reflector even then |
| `cb.fair_batch` | 16 calls | break-class calls the callback thread takes before it takes one waiting call of the other queue (5.6, point 14) | a break flood cannot starve identity replies, while breaks keep almost all of the thread |
| `cb.advance_interval` | 1 s | the least time between two advance ticks (5.6, point 7) | bounds how often a sustained flood can move any one suspect epoch, and so how often it can fence a validation in flight (5.6, point 7); a genuine reset or over-limit call waits at most this long for its reply |
| `cb.promise.lifetime_max` | 7 200 s | the longest promise lifetime the client relies on | a hostile server's expiry can be any value; cutting a promise only lowers trust (C24); a genuine promise longer than two hours costs one revalidation |
| `cb.report_interval` | 60 s | how often the service hands its counts to the event catalogue | one summary event per interval, never one per call (5.9) |
| `cb.key_idle` | 1 980 s | how long a callback state record outlives its last promise and its server's last live connection (5.6, point 10) | `rx`'s identity reservation window R ([rx.md](rx.md), 5.2), so a key never retires while a reply to one of its calls can still arrive |

Derived bounds:

- **Records per call.** The service keeps at most one fixed-size record
  per live incoming call Rx holds for it (5.6, point 7): at most
  (`rx.in.total_bytes` − `rx.in.held_bytes`) ÷ `rx.in.call_charge` =
  15 360 ([rx.md](rx.md), 5.2). Its queues hold only those records, so
  they are bounded by Rx's bytes (C39, point 3).
- **Records per server key.** One callback state record per K, inside the
  cache's server table, whose bound is the cache's (#91).
- **Fixed tables.** `cb.unmatched.table_entries` and
  `cb.refuse.table_entries`, each fixed at start, plus their aggregate
  entries.

### 5.3 Versions

Not applicable as a format version: these are peer procedures IronAFS
does not control ([README](README.md), "Formats IronAFS does not
control"). A variant of a procedure (the three resets) is its own row in
5.1.2.

### 5.4 Decoding

A request reaches the service only whole, within `cb.request_max`, after
Rx's own checks ([rx.md](rx.md), 5.7, point 14, and 5.10). The service
then runs these checks in order; the first failure decides the outcome,
and nothing from the request is used beyond what the outcome names
`[OWN]`.

1. **Procedure number.** Fewer than 4 bytes: discarded (5.7, point 3).
   Otherwise the first 4 bytes are the number (5.1.1, point 4).
2. **Allowlist.** A number not in 5.1.2's table is refused (5.7, point
   1). Nothing after the number is read.
3. **Source.** The source address is matched to the servers it could be
   (section 4), on the receiving family, by exact address comparison
   after Rx's normalisation ([rx.md](rx.md), 5.4, point 3) `[ADR 0004
   C31.3: Working]`. The source port is not compared `[OWN]`: a port
   carries no secret, a NAT may rewrite it, and C31, point 3, matches by
   address.
4. **Message cap.** A request longer than its message's cap (5.2) is
   malformed. A break-class one goes to row M1 or M2 (5.6, point 9); an
   identity-class one is discarded and changes nothing but a counter
   (row I4).
5. **XDR decode**, with [xdr.md](xdr.md)'s strict rules and the caps of
   5.2, for CallBack only; the other allowlisted calls are not decoded
   past the number (5.1.3, points 4 and 5). No tolerance is declared for
   any field (xdr.md, 5.4, point 11): none has a source that shows a
   supported peer needs one. A CallBack that fails is malformed (rows M1
   and M2). Trailing bytes are a failure (xdr.md, 5.4, point 10).
6. **File ids in a break.** Each named file id costs one work unit per
   cell it is looked up in (5.2). Each is looked up in the
   cache's node table of every context, in the cell of each server the
   source could be; for a source that matches no known server, in every
   cell `[OWN]`, since such a break can only suspend (C31's table, row
   2) and a file id names no cell. A file id whose volume the client
   holds a whole-volume promise for (5.1.3, point 3) breaks that volume
   `[OWN]`. A file id whose vnode number is 0 breaks its volume `[OWN]`:
   it can name no file the client holds, and reading it as the widest
   scope can only lower trust; whether servers send such ids is MISSING
   (Q1). A file id that matches no node and no volume changes nothing
   and is counted (`break_unknown_fid`); nothing is created for it.
7. **No allocation for a refused or discarded call** beyond the record
   Rx's call already has `[OWN]`: points 1 to 4 read the request in
   place.

### 5.5 Encoding

Every reply is built by the service and sent by Rx, within Rx's caps and
its unverified-source budget: at most twice the bytes received from a
peer that has not answered Rx's reachability test, and a reply that does
not fit waits for that test or ends with the call ([rx.md](rx.md), 5.10,
point 7). The service never sends a datagram itself.

1. **Break-class replies.** An empty result: the call's results hold no
   bytes. Whether the procedures return any result is MISSING (Q1); an
   empty result is IronAFS's reading, UNVERIFIED until a capture shows
   it, and nothing is coded from it before then (C6, point 4).
2. **Probe:** an empty result, as point 1.
3. **WhoAreYou and TellMeAboutYourself** report the client's interface
   addresses `[K2 §5: UNVERIFIED]`. IronAFS reports its host UUID (C26,
   point 5) and an empty address list `[OWN]`: an unauthenticated query
   that describes the client's network is exposure with no local owner
   (O15, "Why"), and a client with one address per family is known to a
   server by the address its own calls come from. Whether supported
   servers accept an empty list is pending the test cell (Q5). The
   UUID's encoding is the one the VL and FS read specification decodes
   for a server's identity (#65, its 5.1.3, point 6), UNVERIFIED there;
   the address record's layout is MISSING (Q1).
4. **TellMeAboutYourself's capability vector:** a variable-length vector
   of 32-bit words whose first word is a bit mask, bit 0 its least
   significant bit `[K2 §3, §3.1: UNVERIFIED]` `[K1 cap/client.txt:
   UNVERIFIED]`. IronAFS sets no bit `[ADR 0004 N21.1: Working]`: the
   registry's client bits are error translation (0), extended callbacks
   (1) and byte-range locks (5) `[K1 cap/client.txt: UNVERIFIED]`, and
   this build implements none of them; extended callbacks and byte-range
   locks are extensions IronAFS takes only under N22 and N23. A zero word
   means the same as an absent one `[K2 §3: UNVERIFIED]`, so IronAFS
   sends one word of 0 `[OWN]`, the shortest vector whose first word
   exists; whether servers accept an empty vector as well does not
   matter. The vector is the same to every server (N21, point 1).
5. **ProbeUuid (pending):** what a client returns for its own UUID and for
   another is MISSING (Q2). Until the test cell settles it, every
   ProbeUuid call is refused with the unsupported-operation abort (5.7,
   point 1) and changes nothing `[OWN]`: a refusal reads nothing past the
   number, so no MISSING layout is decoded. The revision that settles it
   replaces this point.
6. **The unsupported-operation abort:** an Rx ABORT whose 4-byte code is
   −455 (5.7, point 1).
7. **Identity replies are built once** `[OWN]`: from the host UUID and
   the constant vector, when the service starts, and rebuilt only when the
   host UUID changes (O42, point 5). Answering one costs no allocation
   and no work that grows with the input.

### 5.6 States and ordering

1. **The callback state record**, one per server key K `[ADR 0004 C32.1:
   Working]`, kept by the cache beside its server entry:

   | Field | Type | Written by | Meaning |
   |---|---|---|---|
   | `suspect_epoch` | `u64`, taken from one client-wide counter that only rises; a new record starts at that counter's next value, and every advance takes the next value again | this specification (points 6 to 9) | the server's break counter of C16, point 2. No value is ever used twice, by any record, so an entry recorded under a retired record can never match a later one `[OWN]` |
   | `recorded_since_advance` | `bool` | the cache, when a call records the current tuple (point 3); cleared by an advance | whether any call to the server has recorded the current suspect epoch |
   | `advance_pending` | `bool` | this specification (point 7) | an advance waits for the next tick |
   | `trust` | none, or trusted until an instant on the boot clock | raised only by the keepalive specification (#81, under C32, point 1); lowered here (point 11) | C32, points 1 and 3 |
   | `hold` | count of live outgoing connections to the server, on any family, that fail ADR 0006 decision 1, point 9's condition | the connection layer, from each connection's `eligible` flag ([rx.md](rx.md), 5.6, point 1) | ADR 0006 D2.5 (point 12) |
   | `bucket` | the rate state of `cb.rate.key_units` | this specification (point 8) | C31, point 2 |
   | counters | fixed set (5.9) | this specification | diagnostics |

   The record is created by the first call the client sends to the server
   on that family, and retires as point 10 says. Callbacks never persist
   across a restart `[ADR 0004 C26.3: Working]`: no record survives one.
2. **The fence tuple** of an object of K is (the object's break counter,
   its volume's break counter, `suspect_epoch` of K, the pause generation
   of K's family's socket, the resume generation) `[ADR 0004 C16.2:
   Firm]`. The object's and volume's break counters are the cache's
   (#91), raised by rows B1 and B2; the suspect epoch is this record's;
   the pause generation is the driver's ([rx.md](rx.md), 5.12, point 5);
   the resume generation is point 4's. The local endpoint generation is
   part of K: a rebinding makes a new K, whose record starts with no
   trust and no promise ([rx.md](rx.md), 5.12, point 7) `[OWN]`.
3. **What a call records.** Every call the client sends to a server that
   can validate an entry or grant a promise (a status or data fetch, a
   bulk status, a keepalive) records, when it is sent, the fence tuple of
   each object it names (a keepalive, which names none, records K's
   three shared values), and sets K's `recorded_since_advance` `[ADR 0004
   C16.2: Firm]`. Its reply is installed for an object only if that
   object's fence tuple is unchanged, every value of it, when the reply
   is installed; otherwise it fails its fence and C16, point 2, applies
   (discarded, retried a bounded number of times, then `Stale`, and
   never handed to the request that started it). So a break that names
   the object, or its volume, while the fetch is in flight fails the
   fetch, as a server-wide advance or a pause does. The entry it installs
   keeps the recorded tuple, and installing it never clears a break: the
   break's raised counter is already in the recorded tuple of any later
   fetch, and an earlier one fails.
4. **What the serve path compares** `[ADR 0004 C32.5: Working]`
   ([rx.md](rx.md), 5.12, point 5). On every serve of an entry from K,
   before the answer is given, the cache first compares the C37 clocks,
   as C32, point 5, requires: when the comparison shows a suspend or a
   jump (the rule that detects one is C37's and the keepalive
   specification's, #81), it raises the resume generation before it goes
   on. It then reads the object's current fence tuple (the pause
   generation read with no lock from the driver) and compares every value
   with the entry's recorded tuple. A difference makes the entry
   suspect: it is not delivered until it is revalidated by a call that
   recorded the current tuple `[ADR 0004 C24.2: Firm]`. The comparison is
   made on every serve, never only on an event, so it holds however late
   the service handles any event.
5. **When an entry may be served,** with its recorded tuple equal to the
   current one:
   - K trusted and `hold` = 0: while its promise is valid, and the
     credential and grants are valid (C32, point 3's branch, which rests
     on A14; 5.8);
   - otherwise (bounded revalidation, C32, point 2): only if it was
     validated within the last T seconds, by a call that recorded the
     current tuple; else it is validated before delivery.
   An entry with any other recorded tuple is suspect (point 4) in both
   branches.
6. **An advance** of K, made in the core before any event is queued
   (C16, point 1):
   - if `recorded_since_advance` is set, `suspect_epoch` takes the next
     value of the client-wide counter (point 1), with checked arithmetic
     (section 6), and `recorded_since_advance` is cleared;
   - if it is clear, nothing changes `[OWN]`: no call has recorded the
     current epoch, so no entry was validated at it and no call in flight
     holds it; every entry of K is already suspect against it, and the
     advance would add nothing. The skip is exact, not a rate limit.
   An advance never evicts, never destroys bytes and never changes any
   other context's data (C24, points 3 and 6): it only makes entries
   suspect.
7. **The advance tick** (the policy that confirms overloads and so ends
   a pause). Work the service cannot do at once waits for the tick:
   - **Taking an `Overload` event** ([rx.md](rx.md), 5.10, point 4): the
     service takes it from Rx's event queue and sets `advance_pending` on
     every key of every server its source could be; for an unknown
     source, on every key of both families, since the event names no
     family ([rx.md](rx.md), 5.11) `[ADR 0004 C31: Working, the rate-limit
     row]`. For a source that matches no known server, on none (point 9,
     row O2). It does not confirm the event yet.
   - **A call that waits** (an over-limit call, point 8; a reset or an
     empty break, rows R1 and B0; an extended callback, row X1; a
     malformed break from a known server, row M1): the keys of the
     servers its source could be get `advance_pending`, and the call
     waits, holding only its record.
   - **The tick** runs at most once per `cb.advance_interval`; it runs at
     once when work arrives and no tick ran in the last interval, and
     otherwise at the end of the interval. In order:
     1. every key with `advance_pending` is advanced (point 6), and the
        flag is cleared;
     2. only then is every `Overload` event taken since the last tick
        confirmed to Rx ([rx.md](rx.md), 5.10, point 5), so Rx may drop
        what it held;
     3. only then is every waiting call answered (5.5), or, for a
        malformed one, discarded (5.7, point 3).
   So the suspect epoch of every affected server moves before any of its
   work is dropped or acknowledged, however late the service took the
   event `[ADR 0004 C31: Working, the rate-limit row]`. A call that ends
   while it waits (by an ABORT, ADR 0006 D1.8, or by Rx's reply deadline)
   loses only its reply: its key's advance stays pending and runs at the
   tick (point 9, row A1).
8. **The rate limit** `[ADR 0004 C31.2: Working]`. Each break-class call
   from a known server costs its work units against its key's bucket and
   the total bucket (5.2). A call whose units are available is processed
   at once (point 9). A call whose units are not is an over-limit call:
   it is not processed per file, and the keys of the servers its source
   could be wait for the tick (point 7; row B3). Identity-class calls
   are not charged and never wait `[OWN]`: each is answered from a
   prebuilt reply in O(1) (5.5, point 7), so none is ever more than the
   service can process, and no identity call is dropped or delayed for
   load. So no probe ever moves an epoch (C31's probe row): a flood of
   probes reaches only Rx's own caps, whose overloads are rows O1 to O3,
   as for any other packet.
   A break from a known server is so never discarded: it is either
   applied per file or covered by a server-wide advance before its reply.
   A call from a source that matches no known server costs its units
   against that source's entry in the unmatched table (an aggregate entry
   once the table is full) and against the unmatched total; one whose
   units are not available is discarded (point 9, row B4).
9. **C31's table, restated.** Each row says what is received, what it
   does, and what is sent. "Record first" means the change is made in
   the core before the reply is submitted and before any event is queued,
   and never waits on a layer `[ADR 0004 C16.1: Firm; C31.1: Working]`.
   No row raises trust, evicts anything, or retires a handle or a journal
   record `[ADR 0004 C24.3, C24.7: Firm]`.

   | Row | Received | Effect | Reply |
   |---|---|---|---|
   | B1 | CallBack from a known server, within the rate limit | record first: for each named file, the object's break counter rises in every context that holds it, its promise is suspended and its entry is suspect (C24, point 2); a volume read as 5.4, point 6, says: that volume's break counter rises the same way | empty result (5.5, point 1) |
   | B0 | CallBack from a known server naming no file, within the rate limit | as R1: every key of each server the source could be waits for the tick and advances there, before the reply `[ADR 0004 C31, row 3: Working]` | empty result, after the advance |
   | B2 | CallBack from a source that matches no known server, within its rate | as B1, for the named files only, in every cell (5.4, point 6) `[ADR 0004 C31, row 2; C34.6: Working]` | empty result |
   | B3 | CallBack from a known server, over the rate limit | waits for the tick: every key of the servers the source could be advances, then the reply (point 7) | empty result, after the advance |
   | B4 | CallBack from a source that matches no known server, over its rate | discarded (5.7, point 3), counted | none |
   | B5 | CallBack naming no file, from a source that matches no known server | counted; nothing else changes, as R2 | empty result |
   | X1 | ExtendedCallBack from a known server | a plain break (N22, point 7): until its layout is captured, as B0 (the widest plain break its source allows, 5.1.2, note 2); its extension part is never used. Over the rate limit, as B3 | empty result, after the advance (5.5, point 1) |
   | X2 | ExtendedCallBack from a source that matches no known server | counted; nothing else changes until its layout is captured, since no file id can be read (C34, point 6's residual, section 8); then as B2 | empty result |
   | R1 | InitCallBackState, InitCallBackState2 or InitCallBackState3 from a known server | every key of each server the source could be, on both families, waits for the tick and advances there (point 6), before the reply `[ADR 0004 C31, row 3: Working]`. A reset goes to the tick even within the rate limit `[OWN]`, so forged resets move an epoch no more often than a flood does (point 7); the tick runs at once when none ran in the last interval, so a genuine reset waits for nothing | empty result, after the advance |
   | R2 | a reset from a source that matches no known server | counted; nothing else changes, since no known server is a server that source could be (C31, row 3; C34, point 6's residual, section 8) | empty result |
   | M1 | a break-class call from a known server that is malformed (5.4, points 4 and 5) | as B3: its key waits for the tick and advances `[ADR 0004 C31.2: Working]` | none: discarded after the tick (5.7, point 3) |
   | M2 | a malformed break-class call from a source that matches no known server | counted | none: discarded |
   | O1 | an `Overload` event naming a known server's address | the keys of the servers that source could be wait for the tick; the event is confirmed after they advance (point 7) | Rx's, as it holds and then drops |
   | O2 | an `Overload` event naming an address that matches no known server | counted; confirmed at the next tick with no advance | as O1 |
   | O3 | an `Overload` event whose source is unknown | every key of both families waits for the tick; confirmed after they advance | as O1 |
   | P1 | a pause, then a resume, of one family's socket | none by the service: the driver's raise of the pause generation changes the fence tuple of every key of that family before the first unread datagram (point 4); the `Pause` and `Resume` events are counted and shown in diagnostics only | none |
   | I1 | an identity-class call from a known server, within its message cap | none: counted; never raises trust, and is never authority over any data `[ADR 0004 C24.7: Firm; C31, row 5: Working]` | 5.5, points 2 to 5 |
   | I4 | an identity-class call from a known server longer than `cb.identity.request` | none: counted; no advance (5.4, point 4) | none: discarded (5.7, point 3) |
   | I2 | an identity-class call from a source that matches no known server | none: counted. The client answers identity calls only from servers it has called `[OWN]`: a server that calls them first has already been called by the client (a record exists from the first call sent, point 1), and anyone else learns nothing (O15, "Why") | none: discarded (5.7, point 3) |
   | U1 | a call to a procedure not in the allowlist | none (N21, point 2): counted by number class (5.9) | the unsupported-operation abort, within `cb.refuse.rate` (5.7, point 1) |
   | A1 | a matching ABORT on an incoming call ([rx.md](rx.md), 5.9, point 6) | none here: the effects the call's row applied stand, and an advance its row left pending still runs at the tick `[ADR 0006 D1.8: Firm; D2.3: Working]` | Rx stops the reply and its retransmissions |
   | A2 | a BUSY on an incoming call | none: Rx counts it ([rx.md](rx.md), 5.9, point 8) | none |
   | E1 | an incomplete incoming request that Rx drops at its receive deadline | none here: Rx counts it (`request_expired`); section 13, Q6 | none |

10. **Retirement of a record.** A record retires when, for
    `cb.key_idle`, the client has held no promise from its server on
    that key, has had no live outgoing connection to it, and has had no
    call to it in flight `[OWN]`. Its keys' `advance_pending` work runs
    first. A retired key's addresses no longer make a source match.
11. **What lowers trust** (`trust` becomes none), and nothing else does:
    - a break (rows B1 and B3) ends trust for no key: it suspends the
      files it names, and C32, point 3's "a break ... ends that" is that
      suspension, not a server-wide change `[OWN]`;
    - a reset (rows R1 and B0), and an advance from rows B3, X1, M1, O1 and O3, do
      not change `trust` either: every entry of the key is already
      suspect against the new tuple, which is stronger;
    - a call outcome that shows a loss of callback continuity `[ADR 0004
      C32.3: Working]`: in this version, every call to the server on K
      that ends without a valid reply (`Timeout`, `DeadlineExceeded`,
      `Aborted`, `KeyExhausted`; [rx.md](rx.md), section 7), and every
      abort code the server returns, **except** an ABORT that ADR 0006
      excludes. That is: a matching ABORT, call-level or call 0, on a
      connection that fails decision 1, point 9's condition changes only
      counters `[ADR 0006 D1.6, D1.9: Firm; D2.5: Working]`. Which codes
      a server uses to report a lost callback is A14's question; until
      the test cell answers it, every failure counts (section 13, Q7);
    - an ABORT never advances a suspect epoch `[ADR 0006 D1.5: Firm]`:
      it moves trust to none only.
    Trust is raised only as #81 specifies under C32, point 1, by a
    keepalive call whose recorded tuple is still K's current fence tuple
    when its reply arrives (point 3) `[OWN]`: a keepalive sent before the
    latest pause-generation raise of K's family, or before K's latest
    advance, raises nothing, because a break from that server may have
    been dropped in between.
12. **The hold and its exit** `[ADR 0006 D2.5: Working]`.
    - While `hold` > 0 on any key of the server, every key of the server
      is served as bounded revalidation (point 5), whatever `trust`
      says, with or without packets on the ineligible connection.
    - When `hold` falls to 0, every key of the server is advanced at once
      (point 6; not at the tick), before the event that ended the hold
      is reported. Every entry validated before or during the hold then
      has a recorded tuple older than the current one, so it is suspect
      until revalidated by a call sent after the hold ended (point 4). A
      reply to a call sent before the hold ended, on any connection,
      arrives with an older recorded tuple and fails its fence (point 3),
      so its promise is never installed; its retry is a call sent after
      the hold ended. Neither a keepalive reply nor the end of the hold
      restores an entry.
    - For the exit's advance, `recorded_since_advance` is treated as set
      `[OWN]`, so the advance is never skipped: the hold's promise rests
      on that advance being unconditional, and one extra advance costs
      nothing.
    Under [rx.md](rx.md), 5.6, point 1, every outgoing connection is
    eligible, so `hold` is 0 in production; a fixture connection makes it
    positive in tests (10.5, C13).
13. **Order of the service's steps** for one call, from Rx's delivery:
    5.4's checks; then the row's effect, recorded first; then, for rows
    R1, B0, B3, X1, M1, O1 and O3, the wait for the tick and the advance;
    then the reply, submitted to Rx. Rx may end the call at any point; the steps already done stand
    (row A1).
14. **Queues.** The service takes Rx's deliveries into two FIFO queues of
    fixed-size call records: break-class calls, and all others
    `[OWN]`. The callback thread takes from the break-class queue first,
    except that after every `cb.fair_batch` break-class calls it takes
    one call from the other queue if one waits, so neither queue starves
    the other. An identity-class or refused call costs O(1) (5.5, point
    7; 5.7), so the other queue never holds back a break for longer than
    that O(1) work once per `cb.fair_batch` breaks, and a break flood
    cannot hold back a genuine server's probe for longer than
    `cb.fair_batch` breaks' work at a time; both queues are bounded by Rx's live incoming
    calls (5.2, "Derived bounds"). Rx's own events (`Overload`, `Pause`,
    `Resume`, `AbortReceived`) are taken before either queue.

### 5.7 What is refused, and what is discarded

1. **The unsupported-operation abort.** A call whose procedure number is
   not in the allowlist (5.1.2) is refused with an Rx ABORT carrying the
   code −455 `[OWN]` `[ADR 0004 N21.2, O15.4: Working]`. The registry
   lists −455 as the rxgen error "unknown opcode" `[K1 errors.html,
   "rxgen error codes": UNVERIFIED]`, and the capabilities draft says a
   caller learns that a peer lacks a procedure from that error
   `[K2 §1.2: UNVERIFIED]`: two leads that agree, neither allowlisted
   (Q3). Each refusal costs one token from the source's entry in the
   refusal table and from the refusal total (`cb.refuse.rate`). Without
   a token, the call is discarded instead (point 3) and counted
   (`refuse_suppressed`). Rx sends the ABORT only within its
   unverified-source budget ([rx.md](rx.md), 5.10, point 7): a request
   that carries a procedure number is at least 32 bytes (28 of header and
   4 of number), and the ABORT is 32, so it always fits. A refusal reads
   nothing past the number and changes no state but counters (N21,
   point 2).
2. **What Rx drops** before the service: 5.1.1, point 3. Those packets
   and calls get no reply at all (O15, point 3).
3. **Discarded without a reply.** A call is discarded (it ends with
   nothing sent and its record freed) when it is malformed (no procedure
   number; rows M1 and M2), an identity-class call from a source that is
   not a known server (row I2), a break from a source
   that matches no known server over its rate (row B4), or a refusal
   over `cb.refuse.rate` `[ADR 0004 O15.4: Working]`. Rx has no action
   that ends an incoming call at once with nothing sent (section 13, Q8):
   until it has, the service submits no reply and Rx ends the call at
   `rx.in.reply_deadline`, its charge released then ([rx.md](rx.md), 5.7,
   point 14).
4. **Required traffic under diagnostic load** `[ADR 0004 O15.5:
   Working]`. Refused and discarded calls never take work units from the
   break buckets, never enter the break-class queue, and never use a
   break's reply: Rx's budget is per connection, so a refusal sent to one
   peer takes nothing from another's. What a diagnostic flood from one
   source can take is that source's own Rx budget and one source's
   refusal tokens; a flood from many sources can reach Rx's total, and
   then the overload path (5.6, point 9, rows O1 to O3) moves the
   affected servers' epochs before anything of theirs is dropped.

### 5.8 Assumption A14: what the sources say

ADR 0004 asks this specification to state, labelled, what the sources say
on A14 (C32, point 3; A14, "Settled by").

1. **No registered source states A14**, and none refutes it.
   - The extended-callback draft says a promise holds until its expiry or
     a break, and that on a break the client must treat what it cached of
     the file as unknown `[K3 §3.1: UNVERIFIED]`. It says nothing of what
     a server does with a break it cannot deliver.
   - The capabilities draft says a fileserver can reset the callback state
     it shares with a client through the three reset procedures
     `[K2 §6.1: UNVERIFIED]`. That is a way for a server to tell the
     client of a loss; it does not say a server uses it after a failed
     delivery, or when.
   - The Rx draft says nothing of callbacks (X1 has no callback section;
     its "RPC Layer" heading describes calls in general).
   So A14 stays an assumption: UNVERIFIED. The specification gate does
   not refute it, so C32, point 3's uncapped branch is not reopened by
   this specification. It rests on the test-cell test that A14's
   "Settled by" names (every retransmission of a break dropped while
   keepalives succeed), which is planned with the test cell (#47) and is
   this specification's test-cell case TC2 (10.5).
2. **What bounds the uncapped branch meanwhile** `[OWN]`: a promise is
   relied on only until its expiry, cut to `cb.promise.lifetime_max`
   (5.1.4); a reset or any failed call to the server ends trust (5.6,
   point 11); and the serve path's fence tuple catches every drop Rx or
   the kernel makes while the client is overloaded (5.6, points 4 and 9).
   None of these is the revalidation cap C32, point 3, names for a
   refuted A14; that cap is a project decision if TC2 refutes A14.
3. **If TC2 refutes A14**, C32, point 3's uncapped branch does not ship
   (C32, point 3): 5.6, point 5's first branch is removed, and C32 is
   reopened by an ADR change, not by this specification.

### 5.9 Counters and events

The service emits through the event catalogue (ADR 0004, O1) and shows
its state in diagnostics (O12). It writes no log line per call.
Catalogue ids are assigned in the catalogue, not here `[OWN]`. No event
carries a byte of a request other than its procedure number, a file id
of a break, or the host UUID.

- **Counters**, per server key for known servers, per table entry for
  other sources, and in aggregate: breaks applied (B1, B2) and the file
  ids they named; volumes broken; `break_unknown_fid`; resets (R1, R2);
  over-limit calls (B3); discarded breaks (B4); empty breaks (B0, B5);
  extended callbacks (X1, X2); malformed calls (M1,
  M2); overloads taken and confirmed, by row (O1 to O3); advances made
  and advances skipped (5.6, point 6); ticks; identity calls answered
  and discarded (I1, I2, I4); refusals sent and `refuse_suppressed`, by
  number class (diagnostic, extension, reserved, unassigned); trust
  lowered, by cause (5.6, point 11); hold entered and ended.
- **Events:** one `CallbackSummary` per `cb.report_interval` with the
  counts since the last, if any is non-zero; one `CallbackHold` when a
  server's hold starts and when it ends; one `CallbackAdvance` per key
  per interval at most, naming the rows that caused its advances. Each
  is a fixed-size record; the count of keys bounds them (C39, point 3).
- **Status** (`afs status`, C32, point 6): per known server and family,
  `trust`, `hold`, the time of the last advance and its cause.

## 6. Arithmetic

Every value read from a request is untrusted (coding standards rule 2).

1. **Work units.** 1 plus the count of file ids, in checked `u64`
   addition; the count is already bounded by the array's cap (xdr.md,
   5.2, point 6), so the sum fits.
2. **Buckets.** Each bucket holds a `u64` count of units and refills by
   the elapsed monotonic time times its rate, in checked arithmetic,
   capped at its burst; an overflow is treated as a full bucket.
3. **`suspect_epoch`** takes the client-wide counter's next value, which
   rises by 1 in checked arithmetic. At `u64::MAX` it cannot rise; the key is then served as if every entry were suspect for
   every serve, and the record is retired at the next opportunity
   `[OWN]`. Even at the most the ticks allow for every key together, this
   takes far longer than any daemon runs, so it is a guard, not a case.
4. **Promise lifetime.** The expiry is converted by #82's table into a
   64-bit duration (C38, point 4), then cut to
   `cb.promise.lifetime_max` and added to the send instant on the boot
   clock with C37's checked instant arithmetic; a sum that cannot be
   represented grants no promise (5.1.4).
5. **Tick timing** uses the monotonic clock's checked instant arithmetic
   (C37); a tick whose next time cannot be represented runs at once.

## 7. Errors and failure

1. **No outcome is an error to the peer** except the refusal (5.7, point
   1). Malformed, unmatched or over-rate input is counted and discarded,
   never answered with a code that describes why (O15, point 4).
2. **No panic** is reachable from any request, any sequence of requests,
   or any order of Rx's events (coding standards rule 1). A field that
   fails a check ends that call's processing at that check (5.4).
3. **Fail towards suspicion.** Every failure the service cannot attribute
   to a call that changes nothing (U1, I2, B4, M2, O2) changes only
   counters. Every failure that could hide a break from a known server
   (M1, B3, O1, O3, and the end of a hold) advances a suspect epoch, or
   leaves it pending for the tick, before the work is dropped or
   acknowledged.
4. **State left behind.** A call's record is freed when its call ends;
   its key's pending advance is not, until the tick runs it (5.6, point
   7).
5. **A tick that cannot run** (the service is stopped): nothing is
   confirmed and nothing is answered, so Rx keeps holding, then pauses,
   and the pause generation fences every server (5.6, point 4). Failing
   to run the tick only ever delays work; it never drops a break unseen.

## 8. Security

Who can call the service: any host that can send UDP to the client's
port, with any source address it can forge (off the path) or with the
real address of a fileserver it controls; and anyone on the path, who
sees the calls (ADR 0004, O23; ADR 0006, "Context").

| Threat | What stops it |
|---|---|
| Making stale data look fresh with a forged probe, reply or identity call | nothing the service receives raises trust (5.6, point 9, row I1, and point 11; C24, point 7) |
| Destroying cached data or pending writes with forged breaks | a break only suspends, revokes grants and marks entries suspect; nothing is evicted, no handle or journal record is retired (5.6, point 9; C24, points 3 and 6) |
| Hiding a genuine break by flooding | a known server's break is applied or covered by a server-wide advance before its reply (5.6, points 7 and 8); an overload moves the epoch before Rx drops (row O1); a pause moves every key of the socket before the kernel can drop (row P1; 5.6, point 4) |
| Forcing repeated revalidation by a sustained flood that names one known server | not stopped; bounded: that server's epoch moves at most once per `cb.advance_interval`, and only if a call recorded the current epoch (5.6, points 6 and 7), so a validation fails its fence only when a tick falls inside its round trip; C16, point 2's retries are bounded (10.5, C8). Other servers are unaffected |
| Keeping one file's fetches failing their fence with forged breaks that name it, from its server's address | not stopped; bounded only by the server key's bucket (`cb.rate.key_units`): within it, each break raises the file's break counter at once, as C16, point 2, and C31, point 1, require, so a fetch of that file in flight fails its fence and C16, point 2's bounded retries can end in `Stale`. The attacker must know the server's address, the client's port and the file id. A forged break can cost refetches, and here a failed read of that file; it never serves stale data and never destroys anything (section 13, Q10) |
| The same, from the attacker's own address | no epoch moves: its address matches no known server (row O2); its breaks name only files (row B2) at its own rate |
| Forcing global revalidation through Rx's unknown-source events and pauses | bounded: confirmations, and so resumes, happen only at ticks, so a family's pause generation rises at most twice per `cb.advance_interval` whatever the flood's bandwidth (5.6, point 7) |
| Using the client as a reflector or amplifier | every reply goes through Rx's unverified-source budget (5.5); a refusal is 32 bytes and rate-limited per source and in total (5.7, point 1); identity calls from sources that match no known server get nothing (row I2) |
| Forcing revalidation with forged probes | not possible: identity-class calls are not charged and never wait, so no probe moves an epoch (5.6, point 8); a probe flood reaches only Rx's caps (rows O1 to O3) |
| Serving data cached before a sleep or a clock jump | the serve path compares the C37 clocks first and raises the resume generation, which is in every fence tuple (5.6, points 2 and 4; C32, point 5) |
| Reusing an old entry after its record retired and a new one was made | suspect epochs come from one client-wide counter that never repeats a value (5.6, point 1) |
| Learning the client's state remotely | diagnostic procedures are refused like any unknown number (5.1.2, note 4); identity replies carry the host UUID, no address and no capability bit, and only to known servers (5.5, points 3 and 4) |
| Steering the client by its capability reply | the reply is constant and the same to every server (5.5, point 4; N21, point 1) |
| Exhausting memory | Rx's per-peer and total caps bound every request; the service keeps one fixed record per live call, fixed tables and fields of records the cache already holds (5.2) |
| Starving breaks with diagnostic calls | separate buckets, queues and budgets; refusals and identity answers cost O(1) (5.6, point 14; 5.7, point 4) |
| Forged extended callbacks or extension calls | an extended callback is only a plain break, its extension part never used (row X1; N22, point 7), and from a known server's address costs at most a server-wide advance per tick, the same as a forged reset; other extension calls are refused before any argument is read; nothing advertised invites them (5.1.2, notes 2 and 4; N22) |
| An ABORT on the client's reply undoing a break | effects are recorded before the reply and stand (row A1; ADR 0006 D1.8) |
| A forged ABORT lowering trust through an outgoing call | only on an eligible connection, and only to bounded revalidation, never an epoch advance (5.6, point 11; ADR 0006 D1.5, D1.6) |
| A reset or break from a NAT64 translator's unmatched address | not stopped: C34, point 6's stated residual. A break still suspends the files it names; a reset from such an address moves nothing (row R2). Trust still lapses without keepalive replies (C32, point 1) |
| A server that drops the client's callbacks without telling it | A14's residual (5.8); bounded meanwhile by the promise's cut expiry and 5.6, point 11 |
| Crashing the daemon | no panic, checked arithmetic (sections 6 and 7); fuzz targets (10.4) |

## 9. How the invariants are kept

| Invariant (section 1) | Kept by | Test that fails if it is broken |
|---|---|---|
| Constraint 4: no panics, checked arithmetic, byte limits | 5.2; 5.4; section 6; 7, point 2 | fuzz targets (10.4); reject corpus (10.3) |
| C6, points 1 and 4; N31, points 1 and 2 | every point labelled; layouts MISSING; nothing coded from an UNVERIFIED wire fact | review of this specification; the capture-backed revision (Q1) |
| C16, point 1: record first | 5.6, points 9 and 13 | C2 |
| C16, point 2: fence on the object's, volume's and server's break counters | 5.6, points 2 and 3 | C2, C7, C13, C23 |
| C24, points 2, 3 and 6: weak evidence only suspends | 5.6, point 9 | C2, C3, C4 |
| C24, point 7, and C31, row 5: nothing received raises trust; a probe is never authority over any data | 5.6, point 8 and point 9 (rows I1, I2, I4) and point 11 | C11 |
| C26, points 2 and 3: expiry from send; no persistence | 5.1.4; 5.6, point 1 | C16 |
| C30, point 1, and C34, point 2: both families, port 7001 | 5.1.1, point 2 | C1 |
| C31, rows 1 to 3 | 5.6, point 9 (B0 to B2, B5, R1, R2, X1, X2) | C2, C3, C4, C21 |
| C31, rate-limit row, and point 2 | 5.6, points 7 and 8 (R1, B0, B3, X1, M1, O1, O3) | C4, C5, C6, C15, C21 |
| C31, point 1: counters before reply, never waiting on a layer | 5.6, point 13 | C2 |
| C31, point 3: source matched by address | 5.4, point 3 | C3 |
| C31, point 4, and O15, point 2: closed allowlist | 5.1.2; 5.7, point 1 | C1, C19 |
| C32, point 5 (the clocks compared on every serve), and Rx's pause fence | 5.6, points 2 and 4 (the resume and pause generations) | C7, C22 |
| C32, point 3, and ADR 0006 D2.5: trust lowered only as allowed | 5.6, point 11 | C14 |
| ADR 0006 D2.5: the hold and its exit | 5.6, point 12 | C13 |
| ADR 0006 D1.8 and D2.3: ABORT and BUSY on an incoming call | 5.6, point 9 (A1, A2) | C12 |
| C34, point 6: unattributable sources | 5.6, point 9 (B2, R2, O2) | C3, C9 |
| C39, point 3: queues bounded in bytes | 5.2; 5.6, point 14 | C18, C24 |
| C46, point 3: codes classified by procedure and server | 5.6, point 11 (loss signals per server); 5.7, point 1 (the service's one code) | C14 |
| O15, points 1, 3, 4 and 5 | 5.1.1, point 3; 5.7 | C1, C10, C19, C20 |
| N5, points 2 and 3: hints only lower | 5.6, points 9 and 11 | C11, C14 |
| N21, points 1 and 2 | 5.5, point 4; 5.7, point 1 | C11, C19 |
| N22, points 1 and 7: no extension without rxgk; an unprotected extended callback is a plain break | 5.1.2, notes 2 and 4; 5.5, point 4; row X1 | C19 |
| N29: kinds map to grants, independent of data version | 5.6, point 9 and the registry below | C2, C17 |
| A14: stated, labelled | 5.8 | TC2 (test cell) |

**N29's registry, for the kinds this version knows** `[ADR 0004 N29:
Working]`. Each kind revokes every grant minted under a promise in its
scope (C23, point 3), in every context, whatever the data version:

| Kind | Scope | From |
|---|---|---|
| file break | the named objects | rows B1, B2 |
| volume break | every object of the volume | 5.4, point 6 |
| server advance | every object whose entry recorded the server's key | rows R1, B0, B3, X1, M1, O1, O3; the hold's exit |
| fence change | every object of every server on one family | row P1 |

## 10. Tests

The callback service is in `ironafs-client` and its request types and
codecs in `ironafs-proto` (ADR 0004, C4, points 1 and 2); the callback
state record and the serve-path comparison are in `ironafs-cache`. Test
paths follow coding standards rules 15 and 20. Behaviour tests run in the
simulator (ADR 0004, C3, point 6) with a seeded `Rng` and clock, against
Rx's machine; each has a positive control, so it fails if the service
does nothing. Values are IronAFS's own `[OWN]`.

### 10.1 Golden vectors

In `crates/ironafs-proto/tests/it/callback_golden.rs`, derived by hand
from [xdr.md](xdr.md), 5.1 (a 32-bit integer is 4 bytes, most significant
first), with the derivation beside each vector. The numbers are
UNVERIFIED (5.1.2), so these vectors fix only IronAFS's encoding of the
numbers it reads and the code it sends:

| Id | Value | Bytes | Proves |
|---|---|---|---|
| G1 | the unsupported-operation code, −455 | `fffffe39` (2³² − 455) | the refusal's ABORT payload (5.5, point 6) |
| G2 | the allowlist's numbers 204, 205, 206, 211, 212, 213, 214 | `000000cc`, `000000cd`, `000000ce`, `000000d3`, `000000d4`, `000000d5`, `000000d6` | the dispatcher reads the number as written |
| G3 | TellMeAboutYourself, 65538 | `00010002` | a number above 2¹⁶ |

Vectors for CallBack's arguments and for every reply wait on the
capture-backed revision (Q1).

### 10.2 Round-trip properties

In `crates/ironafs-proto/tests/it/callback_roundtrip.rs`, once the
layouts exist (Q1): every CallBack argument set the encoder accepts
decodes to the same file ids, and every identity reply the encoder builds
decodes to the same UUID and vector. Until then: P1, every 4-byte number
round-trips through the dispatcher's reader.

### 10.3 Reject corpus

In `crates/ironafs-client/tests/it/callback_reject.rs`. Each input is
handled by the row named, with the named counter and no other change:

| Id | Input | Outcome | Breaks |
|---|---|---|---|
| J1 | a request of 0, 1, 2 and 3 bytes | discarded, malformed | 5.4, point 1 |
| J2 | numbers 0, 203, 207, 210, 214, 215, 218, 219, 65535, 65536, 65539 to 65542, 65544, 2³¹ − 1 | refused with −455 | 5.7, point 1; 5.5, point 5 |
| J3 | CallBack with a file-id count that claims more bytes than remain | malformed: M1 from a known server, M2 otherwise | 5.4, point 5 |
| J4 | CallBack of 65 537 bytes | Rx's overload, not the service's (row O1); with a service test double, malformed | 5.2 |
| J5 | CallBack with trailing bytes | malformed | 5.4, point 5 |
| J6 | a reset of 4 097 bytes | malformed: M1 from a known server | 5.2 |
| J7 | WhoAreYou of 257 bytes from a known server | discarded, no advance (row I4) | 5.2; 5.4, point 4 |

### 10.4 Fuzz targets

In `fuzz/fuzz_targets/`, named under coding standards rule 20:

1. `ironafs-proto_callback_request`: arbitrary bytes as a request to the
   dispatcher's decoder. Asserts: no panic or overflow; the outcome is
   exactly one of the rows of 5.6, point 9, for the source given; a
   refused number reads nothing past byte 4; the same input gives the
   same outcome.
2. `ironafs-client_callback_service`: the service over Rx's machine, a
   model cache with known servers on both families, arbitrary requests
   from known and unmatched sources, arbitrary `Overload`,
   `Pause` and `Resume` events, ABORTs on incoming calls and arbitrary
   time steps. Asserts: no panic; no `Overload` event is confirmed and no
   waiting call (rows R1, B0, B3, X1, M1) is answered before its keys
   advanced; no row raises
   trust; a refused call changes no state but counters; the service's
   records never exceed one per live incoming call plus its fixed
   tables; no reply is submitted for a discarded call.

### 10.5 Acceptance cases

In `crates/ironafs-client/tests/it/callback/` (one module per subject).
Each: start state, events in order, result. "Known server" is a server
with a callback state record on the family named.

- **C1 The allowlist is closed.** Every number of 5.1.2's table and every
  registry number of note 4 (5.1.2), on both families: allowlisted ones
  reach their rows; ProbeUuid (214), while pending, and every number off
  the list are refused with −455, at most
  `cb.refuse.rate` per source and in total, the excess discarded and
  counted; no state other than counters changes. A call for service id
  409 or 2501, or with security index 2, reaches no row and draws no
  packet.
- **C2 Break naming files** (row B1). Two contexts hold a file under one
  promise, and a third file is being fetched: a CallBack naming both
  files from the server's address; the object counters rise in both
  contexts before the reply is submitted (the test observes the counters
  at the Rx seam when the reply is handed over); both entries are suspect
  and are revalidated by each reader under its own credential; the
  fetch's reply fails its fence on the object's break counter alone (the
  server's epoch and the pause generation unchanged), is not handed to
  its request, and a retry sent after the break is installed; the same
  with a break that names the fetched file's volume; nothing is evicted; a layer that never
  acknowledges its invalidation does not delay the reply.
- **C3 Unmatched source** (rows B2, R2, O2). A CallBack naming a file
  from an address matching no known server: the file is suspect in every
  cell that holds that file id; a reset and an `Overload` naming that
  address move no epoch.
- **C4 Resets** (row R1). Each of the three resets from a known server:
  every key of the server, on both families, advances once before the
  reply, at once when no tick ran in the last interval; its arguments,
  garbage or empty, are not read; another server's keys do not move. A
  stream of resets from one address within the rate limit moves the
  epoch at most once per `cb.advance_interval`.
- **C5 Rate limit** (row B3). A known server sends breaks above
  `cb.rate.key_units`: the first within the bucket are applied per file;
  the excess wait; at the tick the key advances, then each waiting call
  gets its reply, in that order; no break is discarded; another server's
  breaks within its bucket are applied at once meanwhile.
- **C6 Overload order** (rows O1, O3). Rx's held allowance and caps are
  driven to an overload naming a known server, then to one with an
  unknown source: the service takes each event, advances the keys at the
  tick, and only then confirms; Rx drops nothing held before the
  confirmation. With the service's handling delayed by an arbitrary
  amount, still nothing is dropped before the advance.
- **C7 The pause fence** (row P1; ADR 0006 is not involved). Entries of
  two servers on the IPv4 socket and one on IPv6 are validated; Rx
  pauses the IPv4 socket; with no event handled, a serve of each IPv4
  entry finds it suspect, and the IPv6 entry is not; the same after the
  resume raise, for an entry validated while paused; a validation in
  flight across the raise fails its fence; a keepalive sent before the
  raise does not raise trust when its reply arrives after it.
- **C8 Sustained flood from one spoofed source.** For 60 simulated
  seconds, over-limit breaks and overloads name one known server's
  address at a high rate while a reader reads that server's files with a
  round trip of 50 ms: the server's epoch advances at most 61 times; with
  C16, point 2's retry bound at 3, every read completes; a second
  server's reads are never fenced.
- **C9 Flood from the attacker's own address.** The same flood from an
  address that matches no known server: no epoch moves; known servers'
  breaks keep their latency, within one `cb.advance_interval` of the run
  without the flood.
- **C10 Diagnostic load** `[ADR 0004 O15.5]`. 10 000 diagnostic and
  unassigned calls per second from 1 000 sources, with a genuine
  CallBack from a known server every second: every break is applied
  within the tick's bound and answered; refusals sent stay within
  `cb.refuse.rate`; no break waits behind more than the O(1) work of the
  calls ahead of it.
- **C11 Identity** (rows I1, I2). Probe, WhoAreYou and
  TellMeAboutYourself from a known server: answered
  with the prebuilt replies, the vector one word of 0, the address list
  empty, the same bytes to every server; trust, epochs and entries
  unchanged. The same calls from any other source: no packet. 100 000
  identity calls per second from a known server's address for 10
  simulated seconds: every one within its cap is answered, no epoch of
  that server moves, and its entries stay servable.
- **C12 ABORT and BUSY on an incoming call** (ADR 0006, test 10, the
  service's part). A complete CallBack, then a matching ABORT, before and
  after the reply is submitted: the break's counters stand in both. An
  over-limit break aborted while it waits: the key still advances at the
  tick. A BUSY on an incoming call: counters only.
- **C13 The hold and its exit** (ADR 0006, test 3, the hold and its
  exit; with Rx's A3 for Rx's part). A trusted server gains a fixture
  connection that fails D1.9's condition: while it lives, with and
  without packets on it, the server is served as bounded revalidation. A
  matching abort standing for the server's loss signal arrives on it and
  is only counted; that connection retires; a keepalive reply then
  arrives; with the reader's credential, grant and the entry's nominal
  callback lifetime valid throughout, a read of an entry cached before
  the hold is revalidated by a call sent after the hold ended before
  delivery. Then, with the credential and grant valid throughout: a
  fetch on an eligible connection is outstanding when the hold starts;
  the server grants its promise; the matching abort arrives on the
  ineligible connection and is only counted; that connection retires; a
  keepalive reply arrives; only then does the fetch's reply arrive: it
  fails its fence, and a read of that entry is revalidated by a call
  sent after the hold ended. The same with the fetch begun during the
  hold. Then a validation sent and answered during the hold, with the
  keepalive fresh: after the hold ends and a keepalive reply arrives, a
  read of that entry is delivered only after a validation sent after the
  hold ended.
- **C14 What lowers trust** (5.6, point 11). On a trusted server: a
  matching ABORT on an eligible connection moves trust to none and moves
  no epoch; the same ABORT on a fixture ineligible connection changes
  only counters; a `Timeout` and an `Aborted` with a "no such file" code
  each move trust to none; a break and a reset leave `trust` as it was
  while making entries suspect.
- **C15 Malformed break** (rows M1, M2). A truncated CallBack from a
  known server: the key advances at the tick, then the call is discarded
  with nothing sent; the same from an unmatched source: counted only.
- **C16 Promise time.** Promises of lifetime 0, negative (if the field is
  signed, Q1), 3 600 s, 7 201 s and the field's maximum: none, none,
  3 600 s, 7 200 s and 7 200 s, counted from the send instant on the boot
  clock; across a simulated restart, no promise survives.
- **C17 Volume scope** (5.4, point 6). A break naming a file of a
  read-only volume the client holds a whole-volume promise for, and one
  naming vnode 0 of a read/write volume: every entry of that volume is
  suspect, in every context; a break naming an unknown file id creates
  nothing.
- **C18 Bounds.** Requests from more sources than
  `cb.unmatched.table_entries` and `cb.refuse.table_entries`: the tables
  stay fixed and the aggregate entries count the rest; the service's
  records never exceed Rx's live incoming calls.
- **C19 Extension numbers** (N21, point 2; N22). An ExtendedCallBack
  (65543) from a known server, with arguments of any content: every key
  of the server advances at the tick before the empty reply, trust is
  unchanged, and nothing of its extension part is used; from an
  unmatched source: counted only. The draft's number 65540 and the
  byte-range lock call at 65544, from a known server, with valid-looking
  arguments: refused with −455; no entry, epoch or trust changes.
- **C21 Empty break** (rows B0, B5). A CallBack naming no file from a
  known server, within the rate limit: every key of the server advances
  before the reply, so an entry cached before it is revalidated before
  its next serve under trust; the same from an unmatched source: counted
  only.
- **C22 Suspend and clock jump** (C32, point 5). Entries validated, then
  a simulated suspend (the boot clock moves while the awake clock does
  not) and, separately, a wall-clock jump, with no socket pause and no
  rebinding: the first serve after each raises the resume generation and
  finds every entry suspect, before the answer is given; a validation in
  flight across the suspend fails its fence.
- **C23 Retirement and recreation** (5.6, points 1 and 10). A server's
  record retires after `cb.key_idle` while the cache still holds entries
  recorded under it; a new call creates a new record and its reply
  grants a whole-volume promise: every older entry of that server is
  suspect at its next serve, since no suspect epoch value repeats.
- **C24 Fairness** (5.6, point 14). A within-rate break flood from one
  known server with probes from a second: each probe is answered after
  at most `cb.fair_batch` breaks' work.
- **C20 Rx's own drops.** Rx debug and version packets, and a call for an
  unregistered service id, from a known server's address: no packet in
  reply, no row reached (rx.md's tests carry the counters).

**Test-cell cases**, in the revision that waits on #47 (section 13, Q2),
in the test cell's harness, each recorded under the venue rules of
[README](README.md), "Sources and provenance", rule 4:

- **TC1 What servers call.** Which allowlisted procedures a released
  fileserver calls on the client's first contact, after the client
  restarts with the same host UUID, after a server restart, and on a
  volume release; whether any calls a procedure not in the allowlist and
  what it does after −455.
- **TC2 A14.** Every retransmission of a break dropped while keepalives
  succeed (A14's "Settled by"): what the server does with the client's
  promise, and how and when the client learns of the loss.
- **TC3 Identity replies.** Whether servers accept an empty address list
  (Q5), and the capability word of 0.
- **TC4 ProbeUuid** (Q2).
- **TC5 Source of callbacks.** The source address and port of each
  server's callbacks, on IPv4 and through NAT64.

### 10.6 Gaps

- Every layout: MISSING until the capture-backed revision (Q1).
- The pending row (ProbeUuid) and the test-cell cases: the revision that
  waits on #47 (Q2).
- C7 and C8 need Rx's driver model and the cache's serve path; until both
  exist they run against the simulator's models.

## 11. Revisit if

From the working decisions this rests on:

- ADR 0004, C26: "the specification gate of C25 shows a restore
  discriminator."
- ADR 0004, C30: "the M2 tests show a NAT or firewall that maps the
  callback port differently from the calling port, or one `rx` thread
  limits measured throughput."
- ADR 0004, C31: "a fileserver offers rxgk-protected callbacks to
  clients (then a protected break is authoritative evidence under C24),
  or the test cell shows a callback procedure this table lacks."
- ADR 0004, C32: "M2 measurements against the test cell and through NAT
  show the intervals below serve stale data or cost too many
  revalidations."
- ADR 0004, C34: "assumption A11 fails, or a negotiated extension (N22)
  supplies fileserver addresses other than IPv4."
- ADR 0004, C39: "M2 load tests show the fixed set starves a class of
  work, or lock-order checks find an RPC wait under a lock that cannot be
  restructured."
- ADR 0004, C46: "a layer's error mapping cannot express a contract
  error without losing what the user needs to act on it."
- ADR 0004, O15: "a supported fileserver is shown to need an answer to a
  diagnostic request before it will deliver callbacks."
- ADR 0004, N21: "a tested server requires per-server variation in the
  client's capability reply to interoperate."
- ADR 0004, N29: "the specification for the callback service finds a
  callback whose effect cannot be stated as grant revocation plus scope."
  (This specification finds none: section 9's registry.)
- ADR 0006, decision 2, point 3: "the test cell shows genuine
  incoming-call ABORT or BUSY traffic that this point handles wrongly."
- ADR 0006, decision 2, point 5: "C32, point 3, is revisited, or the test
  of assumption A14 settles how a server reports a lost callback."
- Assumption A14: TC2 refutes it (5.8, point 3).

This specification's own choices:

- TC1 shows a supported server calling a procedure outside the allowlist
  and depending on an answer, or using a reset variant this table lacks;
- TC3 shows a server that needs an address list or a capability bit;
- a capture shows breaks larger than `cb.request_max`, a whole-volume
  break encoded other than 5.4, point 6, reads it, or per-file promises
  on read-only volumes;
- a measured workload shows genuine breaks above `cb.rate.key_units`, or
  C8's bound fails with real round-trip times;
- a capture shows a server sending an extended callback at 65540, or a
  break the client did not advertise for (5.1.2, note 4);
- TC2 shows which abort codes report a lost callback (5.6, point 11's
  "every failure");
- Rx gains an action that discards an incoming call at once (5.7, point
  3; Q8);
- the registry, the capabilities draft or the extended-callback draft is
  revised (register rows E11 to E13).

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| K1 | DOCUMENTATION | AFS Assigned Numbers Registry, register of editions row E11 ([README](README.md)): the procedure table `rx/RXAFSCB.txt`, the client capability list `cap/client.txt`, the Rx service list `rxservice.txt` and the error tables `errors.html` ("Rx RPC error codes", "rxgen error codes"), retrieved 2026-11-21. Rows whose source column names an implementation's change history are not used. A lead until a passage is allowlisted (section 13, Q3) | 5.1, 5.5, 5.7 |
| K2 | SPECIFICATION | T. Keiser et al., "AFS-3 Protocol Capabilities Query Mechanism", draft-keiser-afs3-capabilities-00, register of editions row E12: §1 to §10 prose. Its appendices A, B and C are excluded by name. A lead only | 5.1, 5.5, 5.7, 5.8 |
| K3 | SPECIFICATION | M. Benjamin, "AFS Callback Extensions", draft-benjamin-extendedcallbackinfo-02, register of editions row E13: §1, §2, §3 (3.1 and 3.2), §4.1, §4.2.1, §4.5's prose before §4.5.1, and §5 to §7. Excluded by name: §4.2.2 to §4.4, §4.5.1, §4.5.2 and Appendix A (the draft's type definitions and constants). A lead only | 5.1, 5.8 |
| X1 | SPECIFICATION | "Rx protocol specification draft", N. Zeldovich, updated by J. Altman, 24 July 2026, register of editions row E3. Cited by heading: "RPC Layer"; also read: "Introduction". Read only as the mechanically excerpted copy; its section "Historical Implementation Notes" was not read and is not cited (README, "Sources and provenance", rule 12). A lead only | 5.1.1, 5.8 |
| C | DECISION | [Claim record](../reviews/2026-11-20-adr-0004-claims.md) rows `[DES-011]` and `[DES-058]`, re-read against their sources' rows on 2026-11-21 (the service list, and the VL database's addresses) | 5.1, 5.4 |
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; A11, A14; C4, C6, C16, C17, C23, C24, C26, C30, C31, C32, C34, C37, C38, C39, C46 ([part 1](../adr/0004-architecture-1-core.md)); O1, O12, O15, O23, O42 ([part 3](../adr/0004-architecture-3-operations.md)); N5, N21, N22, N23, N29, N31, N38 ([part 4](../adr/0004-architecture-4-negotiation.md)) | 1, 2, 5, 8, 9, 11 |
| D2 | DECISION | [ADR 0006](../adr/0006-rx-unprotected-packets.md): decision 1, points 5, 6, 8 and 9; decision 2, points 3 and 5; tests 3 and 10 | 1, 5.6, 8, 9, 10 |
| D3 | DECISION | [rx.md](rx.md): 5.2, 5.4, 5.6, 5.7, 5.9 to 5.12 and section 7; [xdr.md](xdr.md): 5.1, 5.2 and 5.4 | 2, 5, 6, 10 |
| D4 | DECISION | the VL and FS read specification (#65): its 5.1.3, points 1, 3 and 6 (the file id, the promise words handed here, the UUID's encoding) | 5.1.3, 5.5 |
| D5 | DECISION | [ADR 0001](../adr/0001-licence-and-dont-copy.md), "Don't copy"; [coding standards](../coding-standards.md), rules 1 to 3, 15 and 20 | 6, 7, 10, 12 |
| O | OWN | every point tagged `[OWN]`, with its reason in place | 5, 6, 7 |

**What was not used.** No source code, test data, interface definition
or generated definition of any AFS or Rx implementation was read for
this specification, and none is cited. No copy of the 1991 AFS-3
programmer's references was used: K2 cites them as donated developer
documentation `[K2 §1]`, but no copy whose provenance is independent of
every implementation's repository has been found, so none is registered
(README, "Sources and provenance", rule 10). K3's §3.1 points, in
passing, at another implementation's source as the basis of one remark,
and §3.2 at an analysis of another implementation's code; neither remark
is used, and nothing here rests on them. The registry's tables and the
drafts are cited for facts only, in IronAFS's own words; none is quoted,
and no figure, type definition, constant name or example from any of
them is reproduced. The procedure names in 5.1.2's second column are the
registry's names for the procedures, used only to identify them; every
other name here is IronAFS's own.

### 12.1 Observed behaviour

None yet. The capture-backed revision records the observation lab's
stage B captures here (#51), and the test-cell revision records TC1 to
TC5 (#47): a test venue with synthetic cell names, users, keys and data;
raw bytes first, a protocol analyser only as a later cross-check; each
observation with its venue, server release, date, scenario and the
evidence label EXECUTED.

## 13. Open questions

- **Q1. The capture-backed revision** (#51, stage B). Every layout in
  5.1.3 and 5.5 is MISSING: CallBack's two lists and whether they have
  equal lengths; how a break names a whole volume, and whether a vnode
  of 0 occurs; the promise record's three words, the expiry's width,
  signedness and epoch (a duration, or a time on the server's clock),
  and which values of the other two words mean "no promise" or another
  scope; whether read-only volumes get per-file promises; the arguments,
  if any, of every allowlisted call; the results of each, and whether
  the break-class results are empty; the address record of WhoAreYou and
  TellMeAboutYourself; the UUID's encoding there. Recommendation: one
  campaign with the VL and FS read specification's (#65, Q1), varying
  one input at a time: a break of one file, of many, on a read-only
  volume during a release, after a server restart. Blocks: every codec
  of this specification.
- **Q2. The pending entries and the test cell** (#47). TC1 to TC5. Until
  then ProbeUuid's row is pending, and every "listed" row rests on leads.
  Recommendation: run TC1 first, since it settles whether the allowlist
  lacks anything; the issue closes only when both revisions are accepted.
- **Q3. Allowlisting the registry and the drafts.** The registry (E11) is
  a published list of assigned numbers kept apart from any
  implementation's repository, and K2 and K3 are published
  Internet-Drafts; none is allowlisted, so every number and every code
  here is UNVERIFIED until a capture shows it. Recommendation: the same
  as the VL and FS read specification's Q2 (#65): allowlist the "Code"
  column of `rx/RXAFSCB.txt` for rows whose source column names a
  release or a registration request, and the code values of
  `errors.html`, for numbers only; keep the drafts as leads. It is the
  owner's decision on clean-room policy (ADR 0001).
- **Q4. The rate-limit values.** `cb.rate.key_units`,
  `cb.rate.total_units` and `cb.unmatched.units` are IronAFS's guesses;
  the genuine rate of a server's breaks during a large release or a mass
  store is MISSING. Recommendation: measure it at the test cell (TC1's
  release case) and in the M2 load tests; lower nothing below the
  measured peak.
- **Q5. The empty address list.** WhoAreYou and TellMeAboutYourself
  report no address (5.5, point 3). If a supported server needs one,
  the choice is between the receiving socket's local address and the
  host's configured addresses; both disclose more. Recommendation: keep
  the empty list unless TC3 fails, then send only the address the query
  arrived on.
- **Q6. An incomplete break that expires.** Rx drops an incomplete
  request at its receive deadline without an event (row E1;
  [rx.md](rx.md), 5.10, point 9). If it was a genuine break from a known
  server, the client never suspects that server for it; under A14 the
  server tells the client at its next call. Recommendation: an rx.md
  amendment (class S) that hands the service a notice, with the source,
  for each expired incomplete request on a service that asks for it, so
  the service advances the key afterwards. It cannot be "before", since
  the bytes never arrived; it narrows A14's dependence.
- **Q7. Which failures end trust.** 5.6, point 11, counts every failed
  call outcome and every abort code (except ADR 0006's ineligible
  ABORT) as a loss of callback continuity. That is the conservative
  reading of C32, point 3's "or an error"; it costs trust after an
  ordinary "no such file" until the next keepalive reply. TC2 shows
  which codes servers use for a lost callback. Recommendation: keep the
  conservative reading for M2a and narrow it only from TC2's evidence,
  by an amendment of this specification.
- **Q8. Discarding an incoming call in Rx.** [rx.md](rx.md) lets a service
  refuse a call with an ABORT (5.7, point 15) but names no action that
  ends an incoming call at once with nothing sent. Until it has one, a
  discarded call (5.7, point 3) holds its Rx charge until
  `rx.in.reply_deadline`. Recommendation: an rx.md amendment (class S)
  adding a `Discard` action: the call ends at once, its queued sends are
  dropped and its charge released, and nothing is sent.
- **Q9. The host UUID.** C26, point 5, and O42, point 5, say it persists
  and when it is replaced; no specification yet says how it is created
  and stored. This service only reads it. Blocks: the identity replies'
  code, not this specification.

- **Q10. Forged breaks on one file.** Within a server's bucket, forged
  breaks naming one file raise its break counter at packet rate, so its
  fetches can fail their fence until C16, point 2's retries end in
  `Stale` (section 8). Coalescing repeated breaks on one object to the
  tick would leave a break the client has received unapplied for up to
  `cb.advance_interval`, which C31, point 1, does not allow.
  Recommendation: keep this specification's rule, and let the cache's
  specification (#91) decide whether a read whose fetch keeps failing
  its fence on the object's counter alone may be answered from a fetch
  sent after the read began, which would need C16, point 2 (Firm) to
  allow it; until then the residual stands as stated.

## 14. Review

No review yet. The review record will be linked here.
