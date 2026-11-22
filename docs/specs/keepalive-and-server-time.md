<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Fileserver keepalive, callback trust and server time

Status: draft (2026-11-21). Raised in issue #81. This is the first
version, written from documents only. A second version adds the facts
the observation lab records (#51) and the test cell's cases (#47)
(section 13, Q1). Code that relies on a fact this version marks MISSING
or UNVERIFIED, the keepalive's codec among it, waits for that version.

Kind: protocol, with behaviour parts (5.6: when the client sends the
keepalive, what its reply may raise, and what a raise covers; 5.7: what
a server's time may change). Class: S. The keepalive reply is the only
input that raises callback trust, and so decides whether cached data is
served without revalidation; a server's time is untrusted input from a
peer IronAFS does not control ([README](README.md), "Review class per
kind": a protocol specification, and a behaviour specification touching
callbacks and the cache).

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (K1, K2, N1
and so on) are in section 12.

**This is the documents-only version.** No admissible document gives the
keepalive procedure's argument or result layout, or says that a released
fileserver answers it without authentication (section 12, "What was not
used"). So the request and reply layouts in 5.1 are MISSING or
UNVERIFIED, server time is "not measured" (5.7), and no codec is written
from this version (C6, point 4). What this version fixes is everything
that does not depend on the bytes: which call is the keepalive, on which
connection and with which identity, when it is sent, the exact
conditions under which its reply raises callback trust and what the
raised trust covers, the bound on everything a server's time may affect,
the caps, the time fields, and the tests.

## 1. What this implements

ADR 0004, overview, constraint 4 (a constraint of the whole ADR):

> **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
> local IPC, configuration, requests from the operating system, and SMB
> messages before authentication. No panics, checked arithmetic and byte
> limits apply to all of it (coding standards rules 1 to 3).

ADR 0004, C6, points 1 and 4 (**Firm**):

> 1. Each protocol component is written from a behaviour specification in
>    `docs/specs/` that names its sources, under ADR 0001. No
>    specification, no code.
>
> 4. A fact about the AFS protocols that this ADR could not tie to a public
>    source is an input to a specification, never something to code from.

ADR 0004, C23, point 2 (**Working**):

> 2. A grant also binds the **network generation**: the client's addresses
>    and routes toward the server. It is captured when the authorising
>    request is sent and must still match when the grant is minted and at
>    delivery. A change discards the reply, authorises again and fences
>    outstanding grants.

ADR 0004, C24, point 7 (**Firm**):

> 7. **Callback trust is raised only by a reply to the client's own call.**
>    Before rxgk, a server earns callback trust (C32) only from its reply to
>    a call the client made, matched by that call's random identity.
>    Nothing the callback service receives, probe or break, raises it; such
>    input may only lower trust.

ADR 0004, C26, point 2 (**Working**):

> 2. Callback expiry is counted from when the request was sent, on the boot
>    clock (C37).

ADR 0004, C30, points 1 and 3 (**Working**):

> 1. The client uses one UDP socket per address family, on port 7001 by
>    convention `[DES-011]`, for outgoing calls and for the callback service.
>
> 3. **The connection key**, defined here once and cited by N7 and N47:
>    the cell, its protocol identifier and configuration generation (N27);
>    the server, the service and the transport; the security class and
>    level; and the credential context with its generation. […]

ADR 0004, C32, points 1, 2, 4, 5 and 6 (**Working**), partly superseded
by ADR 0006 as quoted below:

> 1. Callback trust is kept per server, address family and local endpoint
>    generation. It is earned only by a reply from that server to the
>    client's own keepalive call (point 4), received on the client's
>    callback socket (C30), no older than **E**, and since the last network
>    change or resume. The reply is matched by the keepalive call's Rx
>    identity (epoch, connection id and call number), which the client
>    draws from `Rng` (C3) so that an off-path sender cannot guess it. We
>    propose E = 2 × the keepalive interval. Nothing the callback service
>    receives raises trust (C24, point 7).
> 2. Without fresh evidence the server is in **bounded revalidation**: its
>    data is served only if its status was validated within the last **T**
>    seconds (we propose 30 to 60). Otherwise it is validated before
>    delivery. If that validation cannot complete, the request fails at its
>    deadline; stale data is not served.
>
> 4. The client sends keepalive calls (we propose every 60 to 100 seconds)
>    to servers that hold its callbacks, to keep a NAT mapping open and to
>    earn point 1's evidence. Trust lapses when no reply has arrived within
>    E.
> 5. **Resume is checked when data is served,** not only on a notification
>    from the operating system. Each serve compares the clocks of C37, and a
>    jump marks every callback suspect before the answer is given.
> 6. The state is shown in `afs status` and in the logs.

ADR 0004, C37, points 1, 4, 5 and 6 (**Working**):

> 1. The `Clock` interface gives three clocks:
>    - a **boot clock that counts suspend,** for every deadline a server
>      grants: callbacks, tokens, location lifetimes, locks;
>    - an **awake clock that excludes suspend,** for round-trip and
>      retransmission timers;
>    - **wall time,** only for AFS timestamps and for token expiry as the
>      protocol states it.
>
> 4. **Suspend detection.** […] A detected suspend, a regression of a
>    monotonic clock, or a jump of wall time past the threshold marks
>    callbacks suspect (C32). Wall time can only lower trust. […]
> 5. Startup is refused without a valid pair. If a later read fails,
>    outstanding deadlines expire, callbacks become suspect, one event is
>    logged per episode, and no new deadline is set until a valid sample
>    returns.
> 6. The client never sets the system clock.

ADR 0004, C38, points 1 to 4 (**Working**):

> 1. Inside IronAFS, wall time is one signed 64-bit type (seconds and
>    nanoseconds since 1970). Boot and awake time are 64-bit nanoseconds. No
>    32-bit time exists outside the wire codecs.
> 2. Every wire time field is converted in one module of `ironafs-proto`.
>    For each field it states the width, signedness, epoch and granularity,
>    from the specification.
> 3. Conversion never sign-extends by accident and never wraps. A value the
>    wire field cannot carry is refused with the "out of range" error
>    (C46, point 1); […]
> 4. Arithmetic on server-granted times is done in 64 bits after
>    conversion.

ADR 0004, N5, points 2 and 3 (**Firm**):

> 2. Everything else is a **hint**: DNS answers, rxnull replies, rxkad
>    replies, Rx aborts, timeouts, capability words read without that
>    protection, and anything `aklog` reports.
> 3. A hint may lower trust, or choose between RPC variants the catalogue
>    lists as equivalent (N20). A hint never raises or satisfies a floor,
>    never learns or renews a pin, and never authorises an exception.

The keepalive's reply is an rxnull reply, so it is a hint under N5. It
raises callback trust only because C24, point 7 (Firm), names that one
case; callback trust is not a floor or a pin in N5's sense (N3, N9), and
nothing in this specification lets the reply do anything else that N5,
point 3, forbids (section 8).

ADR 0004, N31, points 1 and 2 (**Firm**):

> 1. A wire fact is taken only from an allowlisted specification passage or
>    an independently generated black-box fixture, recorded with its
>    edition, section or fixture provenance.
> 2. Never from implementation history, interface-definition files,
>    generated definitions, or live-cell captures holding credentials or
>    private data.

ADR 0004, O12, point 2, and O40, point 2 (**Working**), as they stand:

> 2. `afs diagnose` checks: clock skew against servers and the KDC; […]
>
> 2. Server times are converted at receipt. Clock skew is detected and warned
>    about.

ADR 0004, O50, point 1 (**Working**), the bullet that applies:

> - XDR; Rx with the null security class; volume location and file-service
>   reads; callbacks with their trust and NAT rules.

**The amendment this specification depends on.** The owner decided on
2026-11-20 that an absolute skew above 60 seconds is a warning, and that
the fileserver check is kept for M2a only on evidence that qualifies
under N31; otherwise it is "not measured" for M2a. Issue #53 amends O12,
point 2, O40, point 2, and O50, point 1, to that rule. Its point 3, as
proposed and not yet accepted:

> **Fileserver skew only on qualified evidence** (O12, point 2; O40,
> point 2; O50, point 1): above 60 seconds absolute is a warning; a
> fileserver's time is used only from a call qualified under N31,
> otherwise "not measured"; KDC skew moves to M2b's authentication
> diagnostics.

Section 5.7 rests on that amendment. Until #53 is accepted, 5.7's "not
measured" departs from O40, point 2's present text ("Clock skew is
detected and warned about") and from O12, point 2's skew check against
servers; so this specification does not leave draft before #53 is
accepted, and if #53 is accepted with other words, 5.7 is revised to
them (section 11).

[ADR 0006](../adr/0006-rx-unprotected-packets.md), decision 2, point 4
(**Working**), the whole point:

> 4. **The keepalive's identity (with C32, point 1).** The keepalive
>    connection's epoch and connection id are drawn as point 1 says; that is
>    the random identity C32, point 1, and C24, point 7 (firm), require. Its
>    call number stays drawn from `Rng`, as C32, point 1, requires;
>    specification item 4's fallback to 1 does not apply to it. The
>    keepalive's epoch and connection id stay random under every amendment
>    of point 1, because C24, point 7, requires a random identity; an
>    amendment that let other connections use a predictable identity would
>    not apply to it. Trust is still renewed only by a reply that completes
>    that keepalive call, on the current path and network generation.
>    Revisit if: a supported server rejects the keepalive's identity (its
>    random epoch, connection id or call number); that is decided by its own
>    change, with that evidence.

ADR 0006, decision 2, point 1 (**Working**), the sentences that bind the
keepalive's connection:

> 1. **Wire identity (beside C30).** Every connection the client initiates
>    draws its epoch and connection id (its wire identity, distinct from
>    C30, point 3's connection key) fresh from the production `Rng` of C5,
>    point 1 (the cryptographic random source). One fresh epoch per
>    connection, not one per process. […] When the bound would be exceeded,
>    or a bounded number of redraws finds no usable value, no new
>    connection is opened (the request waits or fails at its deadline) and
>    no reservation is released early (specification item 8). […]

ADR 0006, decision 1, point 6 (**Firm**), the shared effect:

> - the one shared effect: callback trust for that server, address family
>   and local endpoint generation (C32, point 1's key) moves to bounded
>   revalidation (C32, point 2), the one transition C32, point 3, allows
>   on an error. […]

ADR 0006, decision 1, point 5 (**Firm**), the item that binds this
specification: a matching ABORT never "raises callback trust (C24, point
7), counts as evidence that the server reset the client's callback
state, advances a suspect epoch".

From [callbacks.md](callbacks.md) (a specification this one rests on):
the callback state record, of which this specification writes only
`trust` (callbacks.md, 5.6, point 1); the fence tuple and the fence
check (5.6, point 2); what a call records (5.6, point 3); every input
that lowers trust (5.6, point 11), and its last paragraph, which leaves
the one input that raises it to this specification. From [rx.md](rx.md):
the keepalive call's channel and random call number (5.6, point 3), the
start of a keepalive call (5.7, point 1), the outcomes (section 7) and
the reservation window R (5.2). From the seams specification (#71, its
5.4, point 1): the reply evidence a keepalive's reply earns trust
through.

Section 9 shows where each invariant is kept and which test fails if it
is broken.

## 2. Scope

1. **Covered:**
   - the keepalive call: the procedure, its service and port, its
     connection, its identity, its caps, and its request and reply as far
     as documents give them (5.1, 5.2, 5.4, 5.5);
   - how callback state is established on the client: when a server key
     becomes active, the first keepalive, and what a server's first
     contact may do (5.6, points 2 and 3);
   - when keepalives are sent, at what interval, and when early (5.6,
     points 4 and 5);
   - the exact conditions under which a keepalive's reply raises callback
     trust, and what a raised trust covers (5.6, points 6 and 7);
   - what a failed keepalive does (5.6, point 8);
   - server time: the evidence gate, what is measured once it passes, the
     warning, and the bound on everything a server's time may change
     (5.7);
   - every time field this specification qualifies, in its own section
     (5.8);
   - the counters, events, `afs status` and `afs diagnose` output of the
     keepalive (5.9).
2. **Not covered, and where it is covered:**
   - every input that lowers trust, the fence tuple, the serve path's
     comparison, the hold and its exit, and the callback service itself:
     [callbacks.md](callbacks.md). This specification raises `trust` and
     defines what a raise covers; it lowers nothing itself (a failed
     keepalive lowers trust through callbacks.md, 5.6, point 11);
   - Rx, the call lifecycle, the wire identity and its reservation, the
     keepalive channel rule and the outcomes: [rx.md](rx.md);
   - the `Rpc` seam, `Reply`, `ReplyEvidence` and the clocks' stamps:
     the seams specification (#71);
   - the conversion of every wire time field: the wire time-field table
     (#82, C38), which takes this specification's rows (5.8);
   - the promise expiry in a fileserver's reply, and whether it is a
     duration or a time on the server's clock: [callbacks.md](callbacks.md),
     5.1.4, and its question Q1. This specification bounds only what a
     server's time may do to it (5.7, point 6);
   - the file-service read procedures that grant promises: the VL and FS
     read specification (#65);
   - the KDC's clock skew: M2b's authentication diagnostics (the
     amendment of #53, point 3);
   - giving promises up when the client stops: not done in M2a `[OWN]`.
     The registry names a procedure for it (`GiveUpCallBacks`, 147)
     `[K1 rx/RXAFS.txt: UNVERIFIED]`; callbacks never persist across a
     restart (C26, point 3), so a promise the client no longer holds
     costs the server at most a break the restarted client counts as
     `break_unknown_fid` (callbacks.md, 5.4, point 6), and no decision
     needs the call.

## 3. The design in short

1. The keepalive is the file service's time procedure, called with the
   null security class on a connection of its own per server key, each
   call on a channel that has carried no call, with a call number drawn
   from `Rng`. The connection's epoch and connection id are random too,
   so an off-path sender must guess the whole identity to forge a reply.
2. A server key is **active** while the client holds a promise from it,
   or has a call in flight that may grant one. An active key gets a
   keepalive at once when it becomes active, then about every 60
   seconds, and early after a network change, a resume from sleep or
   another clock episode, an advance of its suspect epoch, a pause of its
   socket, or a lowering of its trust, never more often than once per 10
   seconds.
3. A keepalive's reply raises trust only if it completes the keepalive
   call, decodes exactly, arrives on the key's own socket generation with
   the key's fence values, network generation and clock episode
   unchanged since the call was sent, and arrives within E (120 seconds)
   of the send. Trust then lasts until E after the reply was received.
4. A raise covers only promises recorded since the key's trust was last
   lowered. An entry validated before a lowering stays in bounded
   revalidation until it is revalidated, however many keepalives succeed
   after it, so a keepalive never revives a promise across a known loss
   of continuity.
5. A failed keepalive lowers trust (callbacks.md, 5.6, point 11) and
   brings the next one forward, with a doubling gap.
6. A server's time is used for diagnostics only. In this version it is
   not measured. Once a revision qualifies the time call under N31, the
   skew is measured with its round-trip uncertainty and warned about
   above 60 seconds. A server's time never sets the system clock, never
   feeds a deadline, never raises or lowers trust, and can only shorten
   a promise's lifetime, never lengthen it.
7. Every piece of state is a fixed-size field of the callback state
   record, and the keepalive's wire identities have a share of Rx's
   identity bound; beyond it, keepalives are not sent and the key stays
   in bounded revalidation.

## 4. Terms

- **Server key, K:** a server, an address family and the local endpoint
  generation of that family's socket: C32, point 1's key, as
  [callbacks.md](callbacks.md), section 4, defines it.
- **Callback state record:** callbacks.md, 5.6, point 1. This
  specification adds the keepalive fields of 5.6, point 1, to it.
- **Keepalive:** one call of the keepalive procedure (5.1.2) to K, made
  by this specification's schedule, marked as a keepalive to Rx
  ([rx.md](rx.md), 5.7, point 1).
- **Keepalive connection:** the outgoing connection K's keepalives use
  (5.6, point 4).
- **Active key:** a K that holds at least one promise in any context, or
  has a call in flight that may grant one (callbacks.md, 5.6, point 3's
  calls other than a keepalive).
- **Lowering:** every event of callbacks.md, 5.6, point 11, that sets
  K's `trust` to none, whatever its value before (so a failed call while
  K is already untrusted is a lowering too), and the end of a raised
  trust that was not renewed (5.6, point 7).
- **Lowering count:** a per-K counter that rises by one at every lowering
  (5.6, point 1).
- **Network generation:** C23, point 2's generation, the client's
  addresses and routes toward the server, taken from the operating
  system's own state (ADR 0006, decision 2, point 2).
- **Episode:** a detected suspend, a regression of a monotonic clock, a
  failed clock read, or a jump of wall time, as the seams specification
  (#71, its 5.3, point 2) counts them in a `Stamp`.
- **Send stamp, receive stamp:** the `first_sent` of a keepalive's
  `Reply` and the `received` of its `ReplyEvidence` (#71, its 5.4, point
  1): `Clock` samples taken when the request was first handed to the
  transport and when the datagram that completed the reply was read.
- **E:** `ka.evidence_max` (5.2), C32, point 1's evidence age.
- **Server time:** the time a fileserver states in a reply as its own
  current time. Not a file's timestamp, and not a promise's expiry.
- **Qualified:** a wire fact that N31, point 1, allows code to rest on:
  an allowlisted passage of a registered edition, or an independently
  generated black-box fixture.
- **Hint:** an input that may lower trust and is counted, never evidence
  (N5, point 2).

## 5. Specification

### 5.1 Layout

#### 5.1.1 Framing

1. **A request** is the procedure number as a 32-bit integer, then the
   procedure's arguments, XDR-encoded, as the Rx call's request bytes;
   **a reply** is the procedure's results, XDR-encoded, as the call's
   reply bytes; **a failure** is an Rx ABORT with a 32-bit code. These
   are the framing of the VL and FS read specification (#65, its 5.1.1,
   points 1 to 3), all UNVERIFIED there, and this specification adds
   nothing to them.
2. **Service and port.** The keepalive goes to the file service, Rx
   service 1 at UDP port 7000 `[K1 rxservice.txt: UNVERIFIED]`
   `[DES-011]`, at the address the client's own file-service calls to
   that server use on K's family (#65, its 5.1.1, point 5) `[OWN]`: that
   is the path a NAT mapping must stay open on for the server's calls
   back (C30, "Why"). A NAT may keep a UDP mapping alive only on the
   inside host's outbound packets `[N1 §4.3: CONFIRMED]`, and may drop
   packets from any remote address, or address and port, the inside host
   has not sent to `[N1 §5: CONFIRMED]`. A published reference page advises
   firewall UDP timeouts of at least twenty minutes for callbacks to
   work well `[L1 afsd(8), "CAUTIONS": UNVERIFIED]`; IronAFS assumes no
   such configuration, and keeps the mapping open itself (5.6, point 5).

#### 5.1.2 The keepalive procedure

| IronAFS name | Registry name | Service | Number | Registry source | Purpose | Label |
|---|---|---|---|---|---|---|
| `fs.get_time` | GetTime | file | 153 | IBM AFS 3.0 | the keepalive (5.6); its reply's server time (5.7) | `[K1 rx/RXAFS.txt: UNVERIFIED]` |

`[OWN]` The registry's name identifies the procedure; `fs.get_time` is
the name code and tests use. No passage of the registry is allowlisted
yet (section 13, Q2), so the number is UNVERIFIED until a capture shows
it. It is not taken from an interface-definition file or a generated
definition (N31, point 2).

**Why this procedure** `[OWN]`, with its leads:
- The capabilities draft says clients used this procedure for periodic
  server probing and for refreshing NAT table entries `[K2 §4.1:
  UNVERIFIED]`. A published reference page says a client may set its
  clock from a fileserver's every five minutes, and that a client probes
  the file servers it uses every few minutes `[L1 afsd(8),
  "DESCRIPTION": UNVERIFIED]`. Both are leads (README, "Sources and
  provenance", rules 11 and 14): they suggest the call exists, is cheap,
  and states the server's time.
- The registry lists it from the first release, IBM AFS 3.0 `[K1
  rx/RXAFS.txt: UNVERIFIED]`, so every target server should offer it,
  and its name says it names no file, so it can grant no promise and
  touch no file's state.
- The capabilities draft recommends that clients call the file
  service's capability query where they formerly called this procedure,
  to keep their view of a server's capabilities current `[K2 §4.1:
  UNVERIFIED]`. IronAFS does not follow that recommendation in M2a
  (README, "Sources and provenance", rule 7): M2a reads no capability
  word (the VL and FS read specification, #65, its section 2, point 3,
  leaves the capability query out), capability words read without
  integrity protection are hints (N5, point 2; K2 §10), and the
  capability reply is a variable-length vector, a larger decoder for no
  decision M2a makes. Section 13, Q4 keeps the choice open.
- A data-free status call (#65's status procedure on a file the client
  holds) would also be a call to the server, but it names a file, may
  grant or refresh a promise, and belongs to one credential context; the
  keepalive belongs to the key, not to a context (5.6, point 4).

Whether the procedure has arguments, and what its results are, is
MISSING (5.1.3, 5.1.4). Whether a released fileserver answers it over
the null security class, without authentication, is MISSING (Q1).

#### 5.1.3 Request

1. **Arguments: MISSING.** By its name the procedure asks for nothing
   but the server's time, so IronAFS's reading is that it has no
   arguments and the request is the 4-byte procedure number alone. That
   reading is UNVERIFIED, and the encoder of 5.5 is not written from it
   until a capture shows it (Q1).

#### 5.1.4 Reply

1. **Results: MISSING.** By its name the reply states the server's
   current time. Its fields, their order, widths, signedness, units,
   epoch and granularity are MISSING, and so is whether the reply carries
   anything else (Q1). This specification's reading is only that the
   reply is fixed-size and holds one server time (5.8); no field is
   coded until the capture-backed version gives the layout from raw
   bytes, varying the server's clock one known value at a time
   ([README](README.md), "Sources and provenance", rule 4).
2. **What the reply is used for:**
   - its completion, matched by Rx's call lookup, is the evidence that
     raises trust (5.6, point 6); its content plays no part in that
     beyond decoding exactly (5.4);
   - its server time is read only when 5.7, point 1's gate has passed,
     and then only for 5.7's diagnostics.

### 5.2 Caps

Each cap is IronAFS's own `[OWN]`, with its reason, in the columns of
[xdr.md](xdr.md), 5.2, point 5. "Declared bound" is MISSING: no
admissible document states one. The capture-backed version records the
declared bound found by observation and lowers a cap that exceeds it
(xdr.md, 5.2, point 4).

| Cap name | Field | Kind | Declared bound | Cap | Direction | Memory budget | Secret | Why |
|---|---|---|---|---|---|---|---|---|
| `ka.request` | message | message | MISSING | 4 bytes | out | 4 bytes | | the procedure number alone (5.1.3); the capture-backed version raises it only if the procedure has arguments |
| `ka.reply` | message | message | MISSING | 64 bytes | in | 64 bytes | | a fixed-size reply holding one time (5.1.4); 64 bytes is sixteen 32-bit words, far above any time encoding, and is also the call's `rx.call.reply_max`, so Rx buffers no more |

Other limits `[OWN]`:

| Name | Value | Applies to | Why |
|---|---|---|---|
| `ka.interval` | 60 s | the longest gap between two keepalive sends to an active key in steady state (5.6, point 4) | the low end of C32, point 4's proposed 60 to 100 seconds: a NAT's UDP mapping timer may be as short as two minutes `[N1 §4.3: CONFIRMED]`, and outbound packets refresh it `[N1 §4.3: CONFIRMED]`, so one keepalive per minute keeps the mapping with one interval to spare |
| `ka.jitter` | 6 s | the gap is drawn uniformly from (`ka.interval` − `ka.jitter`) to `ka.interval`, from `Rng` | keepalives to many servers do not fall on one instant; the gap never exceeds `ka.interval`, so E stays two intervals |
| `ka.evidence_max` (E) | 120 s | the longest age of a reply that raises trust, and how long a raise lasts (5.6, points 6 and 7) | C32, point 1's proposal, E = 2 × the keepalive interval; a mapping timer may be as short as two minutes `[N1 §4.3: CONFIRMED]` |
| `ka.deadline` | 30 s | the keepalive call's deadline, on the boot clock | half an interval: a keepalive ends before the next is due, so at most one is in flight per key; on the boot clock, so a suspend ends it (a keepalive across a suspend can raise nothing, 5.6, point 6) |
| `ka.idle_timeout` | 10 s | the call's T ([rx.md](rx.md), 5.7, point 12) | `rx.idle_timeout`'s least value: a server silent for 10 seconds on a 4-byte call has not answered it |
| `ka.min_gap` | 10 s | the least time between two keepalive sends to one key, for any cause | an early cause (5.6, point 5), repeated by a flood or a flapping network, costs at most one keepalive per key per 10 seconds |
| `ka.conn_idle` | 120 s | the idle time the keepalive connection is opened with ([rx.md](rx.md), 5.6, point 2: an outgoing connection retires after the idle time its user set) | twice `ka.interval`: the longest wait between a keepalive's outcome and the next send is `ka.interval` (5.6, point 5), so a connection in use never retires idle; one that does is a local outcome (5.6, point 8) |
| `ka.calls_per_conn` | 4 calls | keepalives on one keepalive connection before it is closed | [rx.md](rx.md), 5.6, point 3: a keepalive takes only a channel that has carried no call, and a connection has four |
| `ka.identity_share` | 2 048 pairs | live keepalive connections plus their reserved retired identities, all keys together | half of `rx.identity_bound` ([rx.md](rx.md), 5.2): keepalives for many servers can never take the identities that data calls need |
| `ka.skew.warn` | 60 s | the absolute skew above which a warning is given (5.7, point 4) | the owner's decision of 2026-11-20 (the amendment of #53, point 3) |
| `ka.skew.rtt_max` | 2 s | the longest round trip, on the awake clock, of a keepalive whose server time is sampled | a sample's uncertainty is its round trip; two seconds keeps it far below `ka.skew.warn` |
| `ka.skew.sample_max` | 300 s | the oldest skew sample that may shorten a promise's lifetime (5.7, point 6) | five keepalive intervals: a sample older than that may describe a clock since corrected, and an old sample can only shorten, so it is dropped rather than trusted |
| `ka.skew.event_min` | 3 600 s | the least time between two skew events for one key | a server that flaps across the threshold, or lies, costs at most one event per key per hour |
| `ka.report_interval` | 60 s | how often the keepalive's counts are handed to the event catalogue | one summary event per interval, never one per call (5.9) |

Derived bounds:

- **Per key.** One fixed-size set of keepalive fields in the callback
  state record (5.6, point 1); the number of records is the cache's
  server table bound (#91).
- **In flight.** At most one keepalive per key at a time (5.6, point 4).
- **Wire identities.** A keepalive connection is opened only to send a
  keepalive, and keepalives to one key are at least `ka.min_gap` apart;
  a retired connection's pair stays reserved for R = 1 980 seconds
  ([rx.md](rx.md), 5.2). So one key holds at most ⌈1 980 ÷ 10⌉ + 1 = 199
  pairs if every keepalive needed a new connection (each one `Fenced`,
  say); at most ⌈1 980 ÷ 40⌉ + 1 = 51 under a sustained stream of early
  causes, when each connection carries its `ka.calls_per_conn` calls;
  and at most ⌈1 980 ÷ 216⌉ + 1 = 11 in steady state, when a new
  connection follows every four gaps of at least 54 seconds. All keys
  together hold at most `ka.identity_share` (5.6, point 4), counted in
  one fixed table of `ka.identity_share` slots (5.6, point 1).
- **Rate.** At most one keepalive per key per `ka.min_gap`, each at most
  `ka.request` plus Rx's header and retransmissions within
  `ka.deadline`.

### 5.3 Versions

Not applicable as a format version: the keepalive procedure is a peer's
procedure IronAFS does not control ([README](README.md), "Formats
IronAFS does not control").

### 5.4 Decoding

The reply reaches this specification only as a `Completed` outcome
([rx.md](rx.md), section 7), within `ka.reply`, after Rx's call lookup
has matched it to the keepalive's call identity ([rx.md](rx.md), 5.6,
point 7). Then, in order; the first failure decides `[OWN]`:

1. **Exact decode** with [xdr.md](xdr.md)'s strict rules, against the
   reply layout of the capture-backed version (5.1.4). Trailing bytes are
   a failure (xdr.md, 5.4, point 10). No tolerance is declared for any
   field (xdr.md, 5.4, point 11). Until that version exists there is no
   decoder, and no reply decodes: a keepalive raises nothing in code
   built from this version (C6, point 4).
2. **A reply that fails to decode** is a call that ended without a valid
   reply: it lowers trust (callbacks.md, 5.6, point 11) and raises
   nothing. It is counted (`reply_malformed`).
3. **The server time** is handed, as raw wire words, to the wire
   time-field converter (#82, C38, point 2), and only when 5.7, point
   1's gate has passed. A value the converter refuses is no sample
   (5.7, point 3); it does not make the reply invalid, since the time
   plays no part in trust (5.4, point 1) `[OWN]`.

### 5.5 Encoding

1. **The request** is the 4-byte procedure number 153, as [xdr.md](xdr.md)
   encodes an unsigned 32-bit integer, and nothing else (5.1.3). The
   number and the empty argument list are UNVERIFIED and MISSING; the
   encoder is written in the capture-backed version (Q1).
2. **The call** is started on the keepalive connection, marked as a
   keepalive ([rx.md](rx.md), 5.7, point 1), with `rx.call.request_max`
   = `ka.request`, `rx.call.reply_max` = `ka.reply`, T =
   `ka.idle_timeout`, and a deadline `ka.deadline` after its scheduling
   on the boot clock. Rx gives it a channel with no previous call and a
   call number from `Rng` in 1 to 2³⁰ ([rx.md](rx.md), 5.6, point 3)
   `[ADR 0006 D2.4: Working]`.

### 5.6 States and ordering

1. **The keepalive fields** of the callback state record, one set per
   K, kept with callbacks.md, 5.6, point 1's fields `[OWN]`:

   | Field | Type | Written by | Meaning |
   |---|---|---|---|
   | `trust` | none, or trusted until an instant on the boot clock, with the network generation and the episode count in force at the raise (callbacks.md's field) | raised only here (point 6); lowered by callbacks.md, 5.6, point 11, and by point 7's end | C32, points 1 and 3 |
   | `lowerings` | `u64`, rises by 1 at every lowering, checked (callbacks.md's field, 5.6, point 1) | callbacks.md, 5.6, point 11, which includes point 7's end | the lowering count; recorded by every call that records a fence tuple (callbacks.md, 5.6, point 3), so a raise can tell which promises it covers (point 7) |
   | `ka_due` | none, or a boot-clock deadline | points 4 and 5 | when the next keepalive may be sent |
   | `ka_in_flight` | none, or the call's record: the K values it recorded (callbacks.md, 5.6, point 3), the network generation, and `lowerings`, all as at the send | point 4 | at most one keepalive in flight per K |
   | `ka_from` | none, or the stamp the next due time counts from (point 5) | point 5 | the reply's send stamp after a `Completed` outcome that has one; otherwise the arrival of the housekeeping step that handled the outcome, or that made an attempt which sent nothing |
   | `ka_conn` | none, or the keepalive connection and the count of its channels used (0 to 4) | point 4 | the connection's identity is Rx's; this field never holds it |
   | `ka_failures` | `u8`, saturating at 255 | points 6 and 8 | consecutive failed keepalives, for the gap of point 5 |
   | `ka_last` | none, or the receive stamp and round trip of the last keepalive that raised trust | point 6 | `afs status` (5.9) |
   | `skew` | not measured; or no sample yet; or the last sample (5.7, point 3) with its receive stamp, and whether it is above `ka.skew.warn` | 5.7 | diagnostics only |

   Besides the per-K fields, one client-wide table counts keepalive
   wire identities `[OWN]`: `ka.identity_share` fixed slots, each free,
   live (a keepalive connection that is open), or reserved until a
   boot-clock instant (the instant the connection was closed plus R,
   [rx.md](rx.md), 5.6, point 2). A slot whose instant has passed is
   free. Opening a keepalive connection takes a free slot; closing it
   marks the slot reserved; an open that fails, for any cause
   (`IdentityExhausted` in [rx.md](rx.md)'s terms, or any error #71's
   `Rpc::open` returns), frees the slot at once, since nothing was sent
   on it. Rx keeps its own reservations (rx.md, 5.6,
   point 2); this table only keeps keepalives within their share, and
   errs on the side of holding a slot longer.
   Every field is fixed-size. No field is persisted: callbacks never
   persist across a restart `[ADR 0004 C26.3: Working]`, and neither
   does trust.
2. **Establishment on the client** `[OWN]`, from C32, points 1 and 4,
   and callbacks.md, 5.6, point 1:
   - a callback state record is created by the first call the client
     sends to the server on that family (callbacks.md, 5.6, point 1),
     with `trust` none, `lowerings` 0, and no keepalive state;
   - K becomes active (section 4) when the client sends it the first
     call that may grant a promise, or when a reply installs one;
   - when K becomes active and no keepalive is in flight, a keepalive is
     due at once (point 5), so trust can be earned while the first
     promise is in use;
   - until a keepalive raises trust, every entry of K is served under
     bounded revalidation (callbacks.md, 5.6, point 5; C32, point 2).
3. **What a server's first contact may do.** A fileserver keeps callback
   state for a client, which it may reset with any of three procedures
   `[K2 §6.1: UNVERIFIED]`, and it may probe the client periodically to
   check that it is alive `[L1 afsd(8), "DESCRIPTION": UNVERIFIED]`.
   Which calls a released server makes when it first meets the client,
   and in what order, is MISSING (callbacks.md, 10.5, TC1). Whatever it
   calls, the client acts on it only through callbacks.md's table: an
   identity call raises nothing (row I1), and a reset advances K's suspect
   epoch (row R1), so a keepalive sent before the reset raises nothing
   (point 6, condition 4) and the next one earns trust. No call the
   server makes to the client is needed before trust can be earned
   `[OWN]`: C24, point 7, makes the client's own call the only evidence.
4. **The keepalive connection and the call** `[OWN]`:
   - each K has at most one keepalive connection: an outgoing
     connection to the file service (5.1.1, point 2), with the null
     security class and no credential context. Its connection key
     (C30, point 3) names the server, the service, the transport, the
     null class, and in place of a credential context the value "none",
     which no other connection uses; it is never shared with a
     credential context's connections (C18), since trust is per K and
     not per context (C32, point 1) (section 13, Q6);
   - its wire identity is drawn as [rx.md](rx.md), 5.6, point 1, draws
     every outgoing connection's `[ADR 0006 D2.1, D2.4: Working]`;
   - each keepalive takes a channel of `ka_conn` that has carried no
     call. When all `ka.calls_per_conn` channels have been used and the
     last call has ended, the connection is closed and retires
     ([rx.md](rx.md), 5.6, point 2); the next keepalive opens a new one.
     A keepalive start that Rx refuses (`StartRefused`) opens a new
     connection and is started there; nothing was sent (rx.md, section
     7);
   - the keepalive connection is opened with `ka.conn_idle` as its idle
     time (5.2);
   - a keepalive connection is opened only if the identity table (point
     1) has a free slot. Without one, or when
     Rx fails the open with `IdentityExhausted` (rx.md, 5.6, point 1),
     no keepalive is sent: K's trust lapses at its end (point 7) and K
     stays in bounded revalidation, which is safe and costs only
     revalidation `[ADR 0006 D2.1: Working]`. The skip is counted
     (`identity_skip`), the housekeeping step's arrival becomes
     `ka_from`, and the keepalive is due again `ka.min_gap` after it;
   - the keepalive connection is closed, and never reused, when K's
     record retires (callbacks.md, 5.6, point 10), when K stops being
     active and its last keepalive has ended, or when the address the
     client's calls to the server use changes (5.1.1, point 2);
   - at most one keepalive per K is in flight. A cause that would make
     one due while one is in flight (point 5) makes the next one due
     when the current one ends, still no sooner than `ka.min_gap` after
     `ka_from` (point 5);
   - **what the call records when it is sent:** the K values
     callbacks.md, 5.6, point 3, records for a keepalive (K's suspect
     epoch, the pause generation of K's family's socket, the resume
     generation), the network generation (C23, point 2), and K's
     `lowerings`, read in one K step (point 10). It sets K's
     `recorded_since_advance` (callbacks.md, 5.6, point 3).
5. **When a keepalive is due** `[ADR 0004 C32.4: Working; OWN for the
   values]`:
   - **steady state:** while K is active, a keepalive is due a gap after
     `ka_from` (point 1), the gap drawn from `Rng` as `ka.jitter` says
     (5.2);
   - **early causes:** K becoming active (point 2); a change of the
     network generation; an episode (section 4); an advance of K's
     suspect epoch (callbacks.md, 5.6, point 6); a raise of the pause
     generation of K's family's socket (callbacks.md, 5.6, point 9, row
     P1); and a lowering of K's trust by any cause (callbacks.md, 5.6,
     point 11, a failed keepalive included): every change that can fail
     condition 4, 5 or 6 of point 6. Each makes a keepalive due at once,
     but no sooner than `ka.min_gap` after `ka_from`;
   - **after failures:** while `ka_failures` is n ≥ 1, the gap after the
     previous send is the smaller of `ka.interval` and `ka.min_gap` ×
     2ⁿ⁻¹, counted from `ka_from`, and an early cause cannot shorten it
     below that `[OWN]`: a
     server that does not answer, or an attacker who makes every
     keepalive fail, costs a doubling gap up to one keepalive per
     interval. A published client probes servers it has marked down
     every 180 seconds by default `[L1 fs_checkservers(1),
     "DESCRIPTION": UNVERIFIED]`; IronAFS keeps to `ka.interval`,
     because trust, not only reachability, waits on the next reply;
   - **inactive:** a K that is not active sends no keepalive. Its trust
     lapses at its end (point 7), and its record retires as callbacks.md,
     5.6, point 10, says.
   **What a due time counts from** `[OWN]`. Every due time is a
   boot-clock deadline counted from `ka_from` and built by the core's
   sampling function (#71, its 5.3, point 2). After a `Completed`
   outcome, `ka_from` is the reply's send stamp, `first_sent` (#71, its
   5.4, point 1), a source that function accepts. After any other
   outcome, after a `Completed` one whose `first_sent` is none (its
   hand-off sample failed: condition 7 fails, and the reply is dropped
   as stale, point 6), and after any attempt that sends nothing (no
   identity slot, a failed deadline build), no send stamp exists (#71's
   `RpcError` carries none), and `ka_from` is the arrival the
   housekeeping step that handles the outcome or the attempt records, as any ingress records its request's arrival; that
   step runs after the call ended, so after any packet of it was sent,
   and the gap from it is never shorter than the gap from the true send.
   Before K's first keepalive, `ka_from` is none and a keepalive is due
   at once. That housekeeping steps record an arrival is section 13,
   Q5's amendment of #71; until it lands, the failure path has no
   accepted `from`, and the code for it waits with the rest of this
   version's code. The housekeeping thread sends keepalives (C39, point
   1).
6. **The one input that raises trust** `[ADR 0004 C24.7: Firm; C32.1:
   Working; ADR 0006 D2.4: Working]`. A keepalive's outcome raises K's
   trust if, and only if, every one of these holds when it is handled:
   1. the outcome is `Completed` ([rx.md](rx.md), section 7), on K's
      keepalive connection, for the call this specification started
      marked as a keepalive. Rx has matched it by the full call
      identity: the receiving local endpoint, the peer endpoint, the
      direction, the epoch, the connection id, the channel and the call
      number ([rx.md](rx.md), 5.6, points 6 and 7). No second match is
      made here;
   2. the reply decodes exactly (5.4, point 1);
   3. the reply evidence names K's own family and local endpoint
      generation (#71, its 5.4, point 1, `family` and
      `local_generation`): a reply on a socket bound again since K was
      created belongs to another K, and raises nothing for this one
      ([rx.md](rx.md), 5.12, point 7; callbacks.md, 5.6, point 2);
   4. the fence check (callbacks.md, 5.6, point 2) finds K's three
      shared fence values, the suspect epoch, the pause generation of
      K's family's socket and the resume generation, unchanged since the
      keepalive recorded them (a keepalive names no object or volume, so
      no per-object, per-volume or per-context value of the fence tuple
      is part of its check) (callbacks.md, 5.6, points 3 and 11): no
      advance of K, no pause of the socket and no detected suspend or
      clock jump came between its send and now;
   5. the network generation is unchanged since the send (C23, point 2;
      C32, point 1's "since the last network change");
   6. K's `lowerings` is unchanged since the send `[OWN]`: a lowering
      after the send is a possible loss of continuity that the reply,
      sent before it was known, cannot speak for;
   7. the reply evidence meets the seams specification's conditions
      (#71, its 5.4, point 1): both the send and the receive stamps
      exist; the boot clock grew no more than the awake clock between
      them, beyond C37, point 4's threshold; both carry the same episode
      count; the receipt is within E of the send on the boot clock; and
      a fresh sample, taken when the raise is made, shows no more than E
      since the receipt and the same episode count as the receipt.
   Conditions 3 to 7, the end check of point 7 and the write of `trust`
   are one K step (point 10), so no lowering can fall between the check
   and the raise. Then `trust` becomes "trusted until the receive
   stamp's boot instant plus E", recorded with the current network
   generation and the receive stamp's episode count, or keeps a later
   instant it already holds, with the generation and count taken now
   `[OWN]`: a raise never moves the instant earlier. `ka_failures` becomes 0, and
   `ka_last` records the receive stamp and the round trip.
   If any condition fails, nothing is raised, the failing condition is
   counted (5.9), and:
   - a failure of condition 1 or 2 is a failed keepalive (point 8);
   - a failure of conditions 3 to 7 is not a failure of the server: the
     reply is dropped as stale, `ka_failures` is unchanged, and a
     keepalive is due as point 5 says for the cause that changed (a
     network change, an episode, an advance, a pause or a lowering)
     `[OWN]`.
   Nothing the callback service receives (C24, point 7), no other call's
   reply, no ABORT, no ICMP error and no server time raises trust
   (section 8).
7. **What a raised trust covers, and its end** `[OWN]`, under C32,
   point 3, and ADR 0006 D1.6:
   - **Coverage.** A raise covers only the entries of K whose validating
     call was sent at K's current `lowerings`: every call that records a
     fence tuple (callbacks.md, 5.6, point 3) also records K's
     `lowerings`, and an entry is served under trust (callbacks.md, 5.6,
     point 5, first branch) only if its recorded `lowerings` equals K's
     current one. Every other entry of K is served under bounded
     revalidation (C32, point 2) until a call sent after the latest
     lowering revalidates it.
   - **Why.** callbacks.md, 5.6, point 11, lowers trust on a reset or a
     failed call, the signals by which assumption A14 lets the client
     learn of a lost callback (C32, point 3). A reset also advances the
     suspect epoch, but a failed call, and an ABORT above all, may not
     (ADR 0006 D1.5: an ABORT "never ... advances a suspect epoch"). If
     a later keepalive's raise covered entries validated before such a
     lowering, a promise the server had already dropped would be served
     under trust again with no further cap, which C32, point 3, forbids
     ("a known loss of callback continuity ... ends that"). Coverage by
     the lowering count keeps those entries in bounded revalidation, the
     one transition ADR 0006 D1.6 allows on an error, without advancing
     any suspect epoch, and so stays inside ADR 0006 D1.5.
   - **End.** A raised trust ends when the boot clock passes its instant
     (C32, point 4: "trust lapses when no reply has arrived within E"),
     when the network generation changes (C32, point 1: trust holds only
     "since the last network change"), and at an episode (the seams
     specification, #71, its 5.4, point 1). A check sees the second and
     third by comparing the current network generation and episode count
     with those recorded with `trust` at the raise (point 1). Each end is
     a lowering: `trust` becomes none and
     `lowerings` rises `[OWN]`, so a lapse, which may hide a dropped
     break, needs every entry revalidated as any other lowering does.
     The end is applied at the first check that sees it, and every serve
     of an entry of K, every keepalive send to K and every handling of a
     keepalive's outcome checks it first, so no keepalive records a count
     that a lapse already passed, and nothing is served under a trust
     that has ended.
   - **Every lowering** callbacks.md, 5.6, point 11, makes also raises
     `lowerings`, by one, before any event is queued (C16, point 1).
   callbacks.md carries this condition: it records `lowerings` with
   every call's fence tuple (its 5.6, point 3), compares it in the first
   branch of its 5.6, point 5, and raises it at every lowering and at the
   end above (its 5.6, point 11).
8. **A failed keepalive, and a local outcome** `[ADR 0004 C32.4:
   Working; OWN for the split]`. The outcomes of a keepalive fall in two
   sets, by whether the server could have been asked:
   - **A failed keepalive:** a call that was handed to the transport and
     ended without a valid reply: `Completed` with a reply that fails to
     decode (5.4, point 2); `Aborted`; `Timeout`; `DeadlineExceeded` and
     `KeyExhausted` with `sent_any` set; `ReplyTooLarge` ([rx.md](rx.md),
     section 7). In the seams specification's terms (#71, its 5.4, point
     2): `Aborted`, and `NoReply` with the causes `Deadline`, `PeerLost`,
     `ReplyTooLarge` and `Security`.
   - **A local outcome:** the server was not asked, or the client itself
     ended the call: `StartRefused`, `IdentityExhausted`,
     `DeadlineInvalid` and `ProtocolViolation`; `DeadlineExceeded` and
     `KeyExhausted` with `sent_any` clear; `Fenced` and `Cancelled`, which
     only the client causes; and, in the seams specification's terms,
     every `NotSent` cause, and `NoReply` with the causes `Cancelled`,
     `Closed`, `NetworkChanged` and `ClockFailed`. A network change and a
     clock failure each end trust by themselves (point 7, "End"); the
     others say nothing about the server.
   For a failed keepalive:
   - K's trust is lowered as callbacks.md, 5.6, point 11, says for a call
     that ends without a valid reply, with its one exception, an ABORT
     on a connection that fails ADR 0006 D1.9's condition, which changes
     only counters (ADR 0006 D1.6, D1.9). Every keepalive connection is
     eligible ([rx.md](rx.md), 5.6, point 1), so in production every
     failed keepalive lowers trust;
   - `ka_failures` rises by one, saturating;
   - the next keepalive is due as point 5's failure rule says;
   - the outcome's abort code, if any, is counted by value class and
     never interpreted further `[OWN]`: an abort code is classified by
     the procedure and the server that returned it (C46, point 3), and
     this specification names no code with another meaning. A server
     that answers the keepalive with "unknown procedure" (−455 in the
     registry's rxgen table, `[K1 errors.html: UNVERIFIED]`) never gives
     the client trust, and is shown so in `afs status` (5.9).
   For a local outcome: trust is not lowered by it and `ka_failures` is
   unchanged; it is counted by kind (5.9); the connection is closed if
   Rx retired or fenced it, so the next keepalive opens a new one
   (point 4); and the next keepalive is due `ka.min_gap` after `ka_from`
   `[OWN]`. A local outcome that repeats (no identity slot, for example)
   keeps K untrusted at its end (point 7), which is safe.
   Neither set ever advances a suspect epoch, evicts, or changes another
   context's data (C24, points 3 and 6; ADR 0006 D1.5).
9. **What a keepalive does for assumption A14** `[OWN]`. A14 says the
   client learns of a lost callback "no later than its next call to that
   server". The keepalive is such a call. While A14 holds, a promise the
   server dropped is served under trust until the earlier of the next
   keepalive's outcome, which shows the loss (a failure, or a reply that
   shows a reset of the client's callback state, each a lowering,
   callbacks.md, 5.6, point 11; a reset call from the server, rows R1
   and B0, advances K's suspect epoch instead, which fences every entry
   and lowers no trust), and the end of the trust the last
   raise set, E after its receipt (point 7). When keepalives go out on
   schedule, the first is at most `ka.interval` plus `ka.deadline` after
   the loss; when one is not sent (no identity slot, a failed clock
   sample or deadline build, section 6, point 1), the second applies,
   at most E after the last receipt. So the bound is at most the larger
   of `ka.interval` + `ka.deadline` and E: 120 seconds with this
   version's values. If A14 fails, this bound does not hold;
   callbacks.md, 5.8, point 3, says what then changes.
10. **The K step, and order** `[OWN]`. Every read or change of K's
    `trust` and `lowerings` is one **K step**: it runs under K's record
    lock, the server level of C39, point 4's lock order, held for that
    step only, with no RPC wait, disk write or kernel notification
    under it (C39, point 4). The K steps are: every lowering of
    callbacks.md, 5.6, point 11, including point 7's ends; every raise;
    the read of `lowerings` a call records when it is sent (point 4;
    callbacks.md, 5.6, point 3); and the read of `trust`, `hold` and
    `lowerings` a serve makes (callbacks.md, 5.6, point 5). A K step is
    the same kind of single step as the callback service's tick
    (callbacks.md, 5.6, point 14). The fence check's clock comparison
    and the fresh sample of condition 7 are reads, not waits, and run
    inside the raise's K step. So for one keepalive the order is: sent
    (point 4, its values read in a K step); its outcome; then one K step
    that applies any end that is due (point 7), checks conditions 3 to
    7, and raises or does not. A lowering is a K step of its own, so it
    is wholly before the raise's step, where condition 6 sees it, or
    wholly after it, where it lowers the raised trust: no interleaving
    lets a raise survive a lowering recorded before it. Conditions 1 and
    2 are properties of the outcome, known before the step.


### 5.7 Server time

1. **The evidence gate** (the owner's decision of 2026-11-20; the
   amendment of #53, point 3). A fileserver's time is read only if all
   of these are qualified (N31, point 1): the keepalive procedure's
   number; the reply's time fields, with their widths, signedness, units,
   epoch and granularity; and that the procedure is answered over the
   null security class, without authentication, by the target servers
   of the capture matrix (#51). In this version none is: the number is
   UNVERIFIED (5.1.2), the fields are MISSING (5.1.4), and the
   availability is MISSING (Q1). So, for M2a, **server time is not
   measured**: no server time is decoded, `skew` is "not measured" for
   every K, and `afs diagnose` and `afs status` say "not measured" with
   the reason (5.9). The capture-backed version opens the gate only if
   the lab's fixtures qualify all three; otherwise "not measured" stays
   for M2a and the check moves to M2b (section 13, Q7).
2. **Leads, not facts.** A published reference page says a client may
   set its clock from a fileserver's, every five minutes, and recommends
   NTP instead `[L1 afsd(8), "DESCRIPTION", "-nosettime": UNVERIFIED]`;
   the capabilities draft names the keepalive procedure among periodic
   probes `[K2 §4.1: UNVERIFIED]`. They say a server's time is
   obtainable; neither gives its encoding. IronAFS never sets the system
   clock from it, or from anything `[ADR 0004 C37.6: Working]`.
3. **A sample, once the gate has passed** `[OWN]`. From one keepalive
   that raised trust (5.6, point 6):
   - S is the server time, converted by #82's converter into wall time
     (C38, point 1), and g its granularity from #82's row: the server's
     clock read some instant in [S, S + g);
   - w1 and w2 are the wall times of the send and receive stamps (#71,
     its 5.3, point 1), the client's own wall clock;
   - no sample is taken unless both stamps exist, they carry the same
     episode count (so no wall jump came between them, C37, point 4),
     w2 ≥ w1, and the round trip on the awake clock is at most
     `ka.skew.rtt_max`;
   - the skew is the signed distance between the intervals [S, S + g)
     and [w1, w2]: 0 if they overlap; S − w2 if the server is ahead;
     −(w1 − (S + g)) if it is behind. It is a lower bound on the true
     offset, never an estimate that could be too large, so a warning is
     never caused by the round trip;
   - the sample, its sign and its uncertainty (w2 − w1 + g) are kept in
     `skew`, replacing the last.
4. **The warning** (the owner's decision; the amendment of #53, point
   3). A sample whose absolute skew is above `ka.skew.warn` puts K's
   `skew` in warning; a later sample at or below it takes it out. Each
   change emits one `ServerClockSkew` event, at most one per
   `ka.skew.event_min` per K (5.9). `afs diagnose` reports each K in
   warning, with the sample's age and uncertainty. A warning changes
   nothing but diagnostics.
5. **What a server's time never does** `[OWN]`, under C37, points 1 and
   6, and C24, point 7:
   - it never sets or slews the system clock (C37, point 6);
   - it never sets or lengthens a deadline: every deadline is on the
     boot or awake clock (C37, point 1), counted from a stamp the client
     took (#71, its 5.3, point 2), so no call deadline, no keepalive due
     time, no E, no T and no retransmission timer is computed from it,
     and its one use on an expiry is point 6's, which can only shorten a
     promise's lifetime;
   - it never raises or lowers trust, advances a suspect epoch, makes an
     entry suspect, or changes a fence value: a server's clock says
     nothing about callback continuity, and a lowering on it would give
     a lever to anyone who can change a reply's time (section 8);
   - it is never compared with a file's timestamps for cache validity,
     and is never volume incarnation evidence (#78);
   - it is never handed to another component: a specification that
     wants it cites this section and keeps point 6's bound.
6. **The bound on callback lifetimes** `[OWN]`. A promise's lifetime is
   callbacks.md's (5.1.4: counted from the request's send on the boot
   clock, C26, point 2, and cut to `cb.promise.lifetime_max`). Whether
   the promise's expiry is a duration or a time on the server's clock is
   MISSING (callbacks.md, Q1). Whatever the capture shows, a server's
   time may only shorten a lifetime, never lengthen it:
   - if the expiry is a duration, no server time enters the lifetime;
   - if it is a time on the server's clock, the lifetime is computed
     against the client's wall time at the granting call's send (w1 of
     that call, an AFS timestamp in C37, point 1's sense), then cut to
     `cb.promise.lifetime_max`; then, if a sample of point 3 no older
     than `ka.skew.sample_max` shows the server's clock ahead of the
     client's by D, the lifetime is shortened by D; a server shown
     behind never lengthens it;
   - a lifetime of 0 or less grants no promise (callbacks.md, 5.1.4).
   So a server that lies about its time, or an attacker on the path who
   changes the time in a keepalive's reply, can shorten promises and
   cost revalidation, which the server could do anyway through the
   expiry, and can never make the client rely on a promise longer than
   the cap or longer than the expiry read against the client's own
   clock. This point binds #82's conversion of the expiry and
   callbacks.md's lifetime rule (section 13, Q8).
7. **The KDC's clock** is not this specification's: its skew is M2b's
   (the amendment of #53, point 3).

### 5.8 Time fields

Every time field this specification qualifies, inbound or outbound. The
wire time-field table (#82) owns the table and takes these rows (C38,
point 2); a later version of this specification that adds or changes a
field edits that table in the same pull request.

| Field | Direction | Width | Signedness | Epoch | Granularity | Boundary and refusal |
|---|---|---|---|---|---|---|
| the server's current time, in the keepalive's reply (5.1.4) | in | MISSING (Q1): whether one field or a seconds part and a sub-second part | MISSING (Q1) | MISSING (Q1) | MISSING (Q1) | `[OWN]`: converted only once 5.7, point 1's gate has passed. A value the converter cannot represent as wall time (C38, point 3), and a sub-second part of one second or more, is refused as a sample: no sample is taken, `skew` keeps its last state, and the refusal is counted (`time_refused`). It never makes the reply invalid (5.4, point 3) and is never clamped into range |

Outbound: none. The keepalive's request carries no time (5.1.3).

Constrained here, owned elsewhere: a promise's expiry in a fileserver's
reply is callbacks.md's row (5.1.4); 5.7, point 6, bounds how a server's
time may enter its conversion.

### 5.9 Counters, events, status and diagnose

The keepalive emits through the event catalogue (ADR 0004, O1) and shows
its state in diagnostics (O12). It writes no log line per keepalive.
Catalogue ids are assigned in the catalogue, not here `[OWN]`. No event
carries a byte of a reply other than a counted abort code's value class
and, once measured, the skew.

- **Counters,** per K and in aggregate: keepalives sent; trust raised;
  raises refused, by the condition of 5.6, point 6, that failed;
  keepalives failed, by outcome; local outcomes, by kind (5.6, point
  8); abort codes, by value class; replies
  malformed; `identity_skip`; connections opened and closed; trust
  lapsed at its end; lowerings by cause (those of callbacks.md, 5.6,
  point 11, and the lapse); skew samples taken and refused
  (`time_refused`), and samples skipped for round trip or episode.
- **Events:** one `KeepaliveSummary` per `ka.report_interval` with the
  counts since the last, if any is non-zero; one `KeepaliveLost` when an
  active K's trust lapses or is lowered after it was raised, and one
  `KeepaliveRestored` when it is raised again, at most one of each per K
  per `ka.report_interval`; one `ServerClockSkew` per 5.7, point 4. Each
  is a fixed-size record; the count of keys bounds them (C39, point 3).
- **`afs status`** (C32, point 6; O12, point 1), per known server and
  family: trust and its remaining time, the age of the last keepalive
  reply that raised it and its round trip, the revalidation mode
  (callbacks.md, 5.9), consecutive failures, and the skew: "not
  measured" in this version, else the last sample with its age, sign and
  uncertainty. In the user projection only as O12, point 1, allows.
- **`afs diagnose`** (O12, point 2, as #53 amends it): per K, whether
  keepalives earn trust, and the server's clock skew, "not measured" in
  this version, with the reason "server time not qualified". A keepalive
  that succeeds shows that the client's own calls and their replies get
  through; it does not show that the server's calls to the client do
  (callbacks.md, 10.5, TC5), and the output says so.

## 6. Arithmetic

Every value read from a reply is untrusted (coding standards rule 2).

1. **Due times and deadlines** are built by the seams specification's
   constructors from a stamp and a duration, with their checked
   arithmetic (#71, its 5.3, point 1, and its section 6); an overflow is
   a failed build, and the keepalive is then not sent: the housekeeping
   step's arrival becomes `ka_from`, and the keepalive is due again
   `ka.min_gap` after it `[OWN]`.
2. **The gap** of 5.6, point 5: `ka.min_gap` × 2ⁿ⁻¹ is computed as a
   checked shift on `u64` nanoseconds, with n at most 255 by
   `ka_failures`'s saturation; a shift past 63 bits, or an overflow, is
   taken as `ka.interval`, and the result is the smaller of the two.
3. **The jitter** is a `u64` from `Rng` reduced to `ka.jitter`'s range
   in nanoseconds by rejection sampling, so no value is more likely than
   another `[OWN]`; the gap is `ka.interval` minus it, which cannot
   underflow because `ka.jitter` < `ka.interval`.
4. **Trust's end** is the receive stamp's boot instant plus E, by the
   seams specification's checked instant arithmetic; an overflow raises
   nothing (the condition fails as condition 7 of 5.6, point 6).
5. **`lowerings`** rises by one with checked arithmetic; at `u64::MAX`
   the record is retired at the next opportunity, and meanwhile K is
   served as if it were never trusted `[OWN]`, as callbacks.md, section
   6, point 3, treats an exhausted epoch. It is a guard, not a case.
6. **`ka_failures`** saturates at 255.
7. **The skew** of 5.7, point 3: S, S + g, w1 and w2 are wall times
   (C38, point 1); their differences are computed in `i128` nanoseconds,
   so no difference of two valid wall times overflows, and the result is
   saturated to `i64` nanoseconds when stored. S + g, and any lifetime
   shortened by D (5.7, point 6), use checked arithmetic; an overflow
   takes no sample, or grants no promise.
8. **The identity share** counts keepalive connections and reserved
   pairs as `u32`s bounded by `ka.identity_share`; a count that would
   pass it opens nothing (5.6, point 4).

## 7. Errors and failure

1. **Nothing is an error to the peer.** The keepalive is the client's own
   call; a reply that fails 5.4 is counted, lowers trust as a failed
   call, and gets no answer.
2. **No panic** is reachable from any reply, any outcome, or any order
   of outcomes, lowerings, network changes and episodes (coding
   standards rule 1).
3. **Fail towards suspicion.** Every failure, of the keepalive, of the
   identity budget, of a deadline build or of a clock sample, leaves K
   untrusted or lowers it, and so leaves K in bounded revalidation (C32,
   point 2). No failure raises trust, and no failure drops or delays a
   lowering.
4. **State left behind.** A failed keepalive leaves its connection (with
   its remaining channels) for the next keepalive, unless the outcome was
   `Fenced`, in which case the connection is closed `[OWN]`. Nothing else
   is left but counters and the due time.
5. **No server answers the keepalive** (it lacks the procedure, needs
   authentication, or is down): the client never trusts it, and serves
   its data under bounded revalidation for as long as that lasts. This
   is the documented outcome, not a fault (C32, point 2), and `afs
   status` shows it (5.9).

## 8. Security

Who can affect the keepalive: the server itself; anyone on the path
between client and server, who sees the keepalive's identity; and any
host that can send UDP to the client's port with a forged source, who
cannot see it (ADR 0004, O23; ADR 0006, "Context").

| Threat | What stops it |
|---|---|
| An off-path sender forges a keepalive reply to raise trust, making stale data look fresh | the reply must complete the keepalive call: a random epoch and connection id (up to 61 bits) and a random call number (30 bits) on a channel never used before, matched exactly by Rx (5.6, points 4 and 6; rx.md, 5.6; ADR 0006 D2.1, D2.4) |
| A keepalive's identity an off-path sender can guess | every keepalive connection is drawn fresh from the cryptographic `Rng`, one epoch per connection, with no clock or counter as input, and its call number from `Rng` (5.5, point 2; 5.6, point 4); the keepalive never uses a fallback call number of 1 (ADR 0006 D2.4) |
| A forged break, probe or identity call raising trust | nothing the callback service receives raises trust (5.6, point 6; C24, point 7) |
| An old or delayed keepalive reply raising trust after a break, a pause, a suspend or a network change | conditions 4, 5 and 7 of 5.6, point 6: the fence values, the network generation and the episode count must be those of the send, and the reply no older than E |
| A keepalive reviving a promise the server dropped, after an error showed the loss | a raise covers only entries validated since K's last lowering (5.6, point 7) |
| An on-path attacker forging keepalive replies while dropping breaks | not stopped: the stated residual of C32 ("Why") and A13. Bounded by the promise's cut expiry (callbacks.md, 5.1.4) and by any reset or failed call that reaches the client |
| An on-path attacker ending keepalives with forged ABORTs | a denial of trust only: each lowers trust to bounded revalidation (ADR 0006 D1.6), never advances an epoch or evicts (D1.5); the client's keepalives back off (5.6, point 5) |
| A server or an on-path attacker lying about the server's time | the time sets or lengthens no deadline, feeds no trust and no clock, and its one use on an expiry can only shorten a promise (5.7, points 5 and 6); in this version it is not even read (5.7, point 1) |
| A server lying about its time to make the client set its clock | the client never sets the system clock (C37, point 6) |
| Warning floods from a flapping or lying server clock | one skew event per K per `ka.skew.event_min` (5.7, point 4) |
| Exhausting wire identities by making many servers active | keepalives use at most `ka.identity_share`; beyond it no keepalive is sent and those servers stay in bounded revalidation; data calls keep the other half of `rx.identity_bound` (5.6, point 4) |
| Making the client send a stream of keepalives (a flapping network, forged ABORTs, many lowerings) | at most one per K per `ka.min_gap`, with a doubling gap after failures (5.6, point 5) |
| A large or malformed reply | `ka.reply` is the Rx call's reply cap; the decoder is strict, with no trailing bytes (5.2, 5.4) |
| Using the keepalive to learn about the client | the keepalive goes only to servers the client already holds promises from, carries no argument (5.1.3) and no credential (5.6, point 4) |
| Crashing the daemon | no panic, checked arithmetic (sections 6 and 7); the fuzz target (10.4) |

## 9. How the invariants are kept

| Invariant (section 1) | Kept by | Test that fails if it is broken |
|---|---|---|
| Constraint 4: no panics, checked arithmetic, byte limits | 5.2; 5.4; section 6; 7, point 2 | the fuzz target (10.4); the reject corpus (10.3); KA20 |
| C6, points 1 and 4; N31, points 1 and 2 | every wire fact labelled; layouts MISSING; nothing coded from an UNVERIFIED fact; server time not measured | review of this specification; the capture-backed version (Q1); KA16 |
| C23, point 2: the network generation | 5.6, point 6, condition 5; point 5's early cause | KA6 |
| C24, point 7: trust raised only by a reply to the client's own call, matched by its random identity | 5.6, point 6 | KA2, KA3, KA4 |
| C26, point 2: expiry counted from the send on the boot clock | 5.7, point 6 (no server time lengthens it) | KA18 |
| C30, points 1 and 3: the one socket; the connection key | 5.1.1, point 2; 5.6, point 4 | KA1 |
| C32, point 1: per K, on the callback socket, no older than E, since the last network change or resume, matched by the random identity | 5.6, points 4, 6, 7 and 10 | KA2 to KA8, KA21 |
| C32, point 2: bounded revalidation without fresh evidence | 5.6, points 2, 7 and 8 | KA9, KA22 |
| C32, point 4: keepalives to servers holding callbacks; trust lapses within E | 5.6, points 2, 5 and 7 | KA1, KA10, KA11 |
| C32, point 5: resume checked when data is served | 5.6, point 6, conditions 4 and 7 (the fence check compares the clocks first) | KA5 |
| C32, point 6: shown in `afs status` and the logs | 5.9 | KA15 |
| C37, points 1, 4, 5 and 6 | 5.6, point 5 (boot-clock due times); 5.6, point 6, condition 7; 5.7, point 5 | KA5, KA17 |
| C38, points 1 to 4 | 5.8; 5.4, point 3; section 6, point 7 | the converter's tests (#82); KA19 |
| N5, points 2 and 3: an rxnull reply is a hint, which raises no floor or pin | 5.6, point 6 (trust only, under C24, point 7); 5.7, point 5 | KA16 |
| O12, point 2, and O40, point 2, as #53 amends them | 5.7, points 1 and 4; 5.9 | KA16, KA19 |
| ADR 0006 D2.1 and D2.4: the keepalive's identity | 5.5, point 2; 5.6, point 4 | KA3, KA12 |
| ADR 0006 D1.5 and D1.6: an ABORT only lowers, never advances | 5.6, points 7 and 8 | KA9 |

## 10. Tests

Paths follow coding standards rules 15 and 20. The keepalive's schedule
and trust rule are tested in the core's crate against the simulator's
fileserver model (#96); its codec, once the capture-backed version
exists, in `ironafs-proto`.

### 10.1 Golden vectors

- **The request** (5.5, point 1): `00 00 00 99`, the number 153 as four
  big-endian bytes, derived by hand from this specification. Held until
  the capture-backed version confirms the number and that no argument
  follows (Q1); until then it is not a golden vector, and nothing is
  coded from it.
- **The reply:** none until the capture-backed version gives its layout
  (5.1.4). Its vectors then come from independently generated black-box
  fixtures, reduced, with the server's clock set to known values: a time
  before 2038, one after the signed 32-bit boundary and one after the
  unsigned boundary where the server can be set to them (C38, point 5).

### 10.2 Round-trip properties

None until the capture-backed version: the client encodes only the
request and decodes only the reply, so no value goes both ways.

### 10.3 Reject corpus

Once the layout exists, each with the reason and the section it breaks:
a reply one byte longer than the layout (trailing bytes, 5.4, point 1);
a reply one byte shorter; a reply of `ka.reply` + 1 bytes (Rx's
`ReplyTooLarge`, 5.6, point 8); a sub-second part equal to its unit
(5.8); a server time the converter cannot represent (5.8). The first
three are failed keepalives; the last two are valid replies with no
sample.

### 10.4 Fuzz target

`fuzz/fuzz_targets/fs_get_time_reply.rs`, once the layout exists: any
bytes up to `ka.reply` either decode or fail with an error; decoding
never panics and never allocates beyond the cap; every decoded time
either converts or is refused, never wraps.

### 10.5 Acceptance cases

In the simulator, each as a start state, the events in order, and the
result that must be seen. "Trusted" means K's `trust` is set and its
recorded entries are served without revalidation; "bounded" means
bounded revalidation.

- **KA1 Establishment.** No record for the server. A status call grants
  a promise. A keepalive goes out at once on a new keepalive connection
  with the null class, to the file service's port at the address the
  status call used; nothing is sent to a server the client holds no
  promise from and calls nothing that may grant one. Until its reply,
  the entry is served bounded.
- **KA2 The raise.** KA1's keepalive is answered. K becomes trusted
  until the receive instant plus E; the entry is served without
  revalidation while its promise is valid.
- **KA3 Identity.** A reply with the keepalive's epoch and connection id
  but another call number, one with another connection id, and one from
  another source address: none completes the call, none raises trust.
  Each keepalive connection draws a fresh epoch and connection id; each
  keepalive's call number is from `Rng` on an unused channel; the fifth
  keepalive opens a new connection.
- **KA4 Nothing else raises.** With K untrusted: a probe, an identity
  call, a break, a reset, a reply to a status call, and an ABORT arrive.
  K stays untrusted.
- **KA5 Suspend and clock jumps.** A keepalive is sent; the simulated
  host suspends for 5 seconds; its reply is read after the resume. No
  raise. The same with a wall-time jump of 2 seconds between the send
  and the receipt, and with a failed clock read. Then the next keepalive,
  sent after the episode, raises trust.
- **KA6 Network change.** A keepalive is sent; the network generation
  changes; the reply arrives. No raise; a keepalive is due at
  `ka.min_gap` after the first; its reply raises trust. Then, with K
  trusted, the network generation changes again: trust ends at once and
  `lowerings` rises; an entry validated before the change is served
  bounded until revalidated.
- **KA7 Fence moved in flight.** A keepalive is sent; a reset from the
  server (callbacks.md, row R1) advances K; the reply arrives. No raise.
  The same with a pause and resume of the socket in between.
- **KA8 Evidence age.** In production a `Completed` keepalive is
  received within `ka.deadline` of its send, which is less than E, so
  condition 7's "receipt within E of the send" cannot fail through Rx;
  it guards a later rise of `ka.deadline`. It is tested at the seam: a
  scripted `Rpc` returns a `Reply` whose receive stamp is E + 1 second
  after its send stamp: no raise, counted under condition 7. Through
  Rx: a keepalive whose reply the simulated network holds past
  `ka.deadline` ends `DeadlineExceeded` with `sent_any` set, a failed
  keepalive. And a reply received 10 seconds after its send but handled
  115 seconds after receipt raises trust that ends 5 seconds later.
- **KA9 Coverage across a lowering.** K trusted; entry A validated.
  A status call to the server ends with an ABORT on an eligible
  connection: trust none, `lowerings` rises, no suspect epoch moves.
  The next keepalive raises trust. A is served bounded (validated within
  T, else revalidated); after A is revalidated, A is served under trust.
  An entry B validated after the ABORT is served under trust at once.
- **KA10 Lapse.** K trusted; the simulated server stops answering.
  Keepalives fail; trust is lowered at the first failure, the gaps
  double from 10 seconds to 60; no stale read is served beyond T.
- **KA11 Inactive.** K's last promise expires and no call is in flight.
  No keepalive is sent after the one in flight ends; trust lapses at its
  end; the record retires after `cb.key_idle`.
- **KA12 Identity share.** With `ka.identity_share` set small in the
  test, more servers become active than it allows. The extra servers
  send no keepalive, stay bounded, and `identity_skip` counts; data
  calls still open connections.
- **KA13 One in flight.** A network change and a lowering arrive while a
  keepalive is in flight. No second keepalive starts; the next starts
  when the first ends, no sooner than `ka.min_gap` after its send.
- **KA14 Flood of early causes.** A forged-ABORT stream on every
  keepalive, and a network generation that flaps every second. At most
  one keepalive per K per `ka.min_gap`, the gap doubling; the identity
  pairs held by K stay within 5.2's bound.
- **KA15 Status.** `afs status` shows trust, its remaining time, the
  last reply's age and round trip, failures, and skew "not measured".
- **KA16 Server time not measured.** The simulated server's keepalive
  replies carry a time 3 hours ahead of the client. No skew is shown
  but "not measured"; no event; no deadline, trust, lifetime or clock
  changes.
- **KA17 No clock setting.** In any run, the simulated client's wall
  clock is never set or slewed by the client.
- **KA18 Lifetimes only shorten** (the capture-backed version, once the
  expiry's form is known and the gate has passed). The simulated server
  states a time 10 minutes behind the client and grants an absolute
  expiry: the lifetime is not lengthened. The same server 10 minutes
  ahead: the lifetime is shortened by 10 minutes. A lifetime that falls
  to 0 grants no promise.
- **KA19 Skew warning** (once the gate has passed). Samples of 59, 61
  and 59 seconds ahead, with a round trip of 1 second: the warning
  starts at the second and ends at the third, one event each, then no
  further event within `ka.skew.event_min` though the server flaps; a
  sample with a round trip of 3 seconds is skipped; a time inside the
  send-to-receive window gives 0.
- **KA21 Raise against a racing lowering.** Under the simulator's
  interleaving search, a keepalive's reply is handled while a status
  call to the same server ends with `Timeout` on another thread, in
  every order the scheduler can produce. In no run does K end trusted
  with `lowerings` raised by the `Timeout` and `trust` set by that
  keepalive; an entry validated after the `Timeout` is never served
  under a trust that keepalive set.
- **KA22 Local outcomes.** A keepalive start refused (`StartRefused`),
  an open with no identity slot, a `NotSent(Admission)` and a
  connection retired by Rx before the next keepalive: trust is not
  lowered by any of them, `ka_failures` is unchanged, the housekeeping
  step's arrival becomes `ka_from`, and the next keepalive is due
  `ka.min_gap` later; a failed open frees its identity slot, so 3 000
  failed opens in a row leave the table's free slots as they were; a
  `Completed` reply whose send stamp is missing raises nothing, is not a
  failure, and the next keepalive is due `ka.min_gap` after the step
  that handled it; a `Timeout` after the request was
  sent is a failed keepalive and lowers trust.
- **KA20 Hostile replies.** Every reply of the reject corpus, in turn:
  no panic, the outcomes of 10.3.

**Test-cell and lab cases**, in the capture-backed version (#51) and the
test cell (#47), recorded under [README](README.md), "Sources and
provenance", rule 4:

- **TK1 The procedure.** Whether each server of the capture matrix
  answers procedure 153 over the null class without a token, with no
  arguments; the reply's bytes with the server's clock set to known
  values, one at a time.
- **TK2 Identity.** Whether each server accepts a keepalive on a
  connection with a random epoch and connection id and a random first
  call number in 1 to 2³⁰ (ADR 0006 D2.4's trigger).
- **TK3 Side effects.** Whether the keepalive changes any callback state
  on the server: the server's callback calls to the client before and
  after a keepalive from a client it has not met.
- **TK4 NAT.** Through a NAT with a two-minute mapping timer, breaks
  still reach the client with keepalives every `ka.interval`, and stop
  reaching it when keepalives stop (C32's trigger).

### 10.6 Gaps

- The codec and its golden vectors, the reject corpus and the fuzz
  target: the capture-backed version (Q1).
- KA18 and KA19: the capture-backed version, and only if the gate
  passes (Q7).
- TK1 to TK4: the lab and the test cell (#51, #47).

## 11. Revisit if

From the working decisions this rests on:

- ADR 0004, C23: "the M2 tests show the network-generation rule
  discards replies on ordinary roaming often enough to matter, or the
  ACL specification shows a revocation source this list lacks."
- ADR 0004, C26: "the specification gate of C25 shows a restore
  discriminator."
- ADR 0004, C30: "the M2 tests show a NAT or firewall that maps the
  callback port differently from the calling port, or one `rx` thread
  limits measured throughput."
- ADR 0004, C32: "M2 measurements against the test cell and through NAT
  show the intervals below serve stale data or cost too many
  revalidations."
- ADR 0004, C37: "the M2 sleep, hibernation, Modern Standby and VM-pause
  tests show a clock below does not behave as its documentation says,
  or a supported target lacks a valid pair."
- ADR 0004, C38: "the wire time table (C6) shows a field whose published
  definition this rule cannot represent."
- ADR 0004, O12: "support cases show a recurring failure neither command
  names."
- ADR 0004, O40: "the specification work for the file service (C6) shows
  a server behaviour that contradicts a point here."
- ADR 0004, O50: "a phase's gate fails, a specification route proves
  unavailable, or an rxgk trigger in point 9 opens a scheduling review."
- ADR 0006, decision 2, point 1: "a supported server mishandles a random
  epoch or connection id, or relies on connection ids unique beyond
  their connection's life."
- ADR 0006, decision 2, point 4: "a supported server rejects the
  keepalive's identity (its random epoch, connection id or call number);
  that is decided by its own change, with that evidence."
- Assumption A14: the callback specification's test TC2 refutes it
  (callbacks.md, 5.8, point 3); then 5.6, point 9's bound does not hold.
- Issue #53 is accepted with words other than those quoted in section
  1, or not accepted.

This specification's own choices:

- TK1 shows a target server that does not answer the keepalive
  procedure without authentication, or that it takes arguments, or that
  its reply carries more than a time (5.1);
- TK2 shows a server that rejects the keepalive's random identity;
- TK3 shows that the keepalive changes callback state on the server;
- TK4, or the M2 NAT tests, show a mapping timer below two minutes on a
  supported path, or breaks lost between keepalives (`ka.interval`,
  E);
- the callback specification's TC5 shows a server's callbacks coming
  from an address or port other than the one the keepalive goes to
  (5.1.1, point 2; Q9);
- the cost of 5.6, point 7's coverage rule is measured as too many
  revalidations after ordinary errors (callbacks.md, Q7 narrows which
  failures lower trust, and so how often it applies);
- M2a reads a capability word, or the capability query is needed for
  another reason (Q4);
- the registry, the capabilities draft, RFC 4787 or the reference
  pages are revised (register rows E7, E9, E10 and E56).

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| K1 | DOCUMENTATION | AFS Assigned Numbers Registry: the file service procedure table `rx/RXAFS.txt`, register of editions row E7 ([README](README.md)); the Rx service list `rxservice.txt` and the error tables `errors.html`, row E7. Rows whose source column names an implementation's change history are not used. A lead until a passage is allowlisted (section 13, Q2) | 2, 5.1, 5.5, 5.6 |
| K2 | SPECIFICATION | T. Keiser et al., "AFS-3 Protocol Capabilities Query Mechanism", draft-keiser-afs3-capabilities-00, register of editions row E9: §4, §4.1, §5, §5.1, §6, §6.1 and §10 prose. Its appendices are excluded by name. A lead only | 5.1.2, 5.6, 5.7 |
| N1 | SPECIFICATION | RFC 4787, "Network Address Translation (NAT) Behavioral Requirements for Unicast UDP", BCP 127, January 2007, register of editions row E56: §4.3 (REQ-5, REQ-6) and §5, both allowlisted there for facts about NATs, so those facts are CONFIRMED | 5.1.1, 5.2, 10.5 |
| L1 | DOCUMENTATION | OpenAFS reference pages `afsd(8)` ("DESCRIPTION", "CAUTIONS" and the `-nosettime` and `-settime` options) and `fs_checkservers(1)` ("DESCRIPTION", "OPTIONS"), register of editions row E10. A lead only | 5.1.2, 5.6, 5.7 |
| C | DECISION | [Claim record](../reviews/2026-11-20-adr-0004-claims.md) row `[DES-011]` (ports and services), re-read on 2026-11-21. The record has no row on keepalives or a server's time; its clock rows (`[DES-012]` to `[DES-017]`, `[DES-019]`) are C37's and C38's | 5.1 |
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; A13, A14; C6, C16, C18, C23, C24, C26, C30, C32, C37, C38, C39, C46 ([part 1](../adr/0004-architecture-1-core.md)); O1, O12, O23, O40, O50 ([part 3](../adr/0004-architecture-3-operations.md)); N3, N5, N9, N31 ([part 4](../adr/0004-architecture-4-negotiation.md)) | 1, 2, 5, 8, 9, 11 |
| D2 | DECISION | [ADR 0006](../adr/0006-rx-unprotected-packets.md): decision 1, points 5, 6 and 9; decision 2, points 1, 2 and 4 | 1, 5.5, 5.6, 8, 9 |
| D3 | DECISION | the amendment of ADR 0004 in issue #53, point 3 (proposed, not yet accepted), and the owner's decision of 2026-11-20 it carries | 1, 5.2, 5.7 |
| D4 | DECISION | [callbacks.md](callbacks.md): 5.1.4, 5.4, 5.6 (points 1 to 5, 10 and 11), 5.8, 5.9, section 6, 10.5 and section 13; [rx.md](rx.md): 5.2, 5.6, 5.7, 5.12 and section 7; [xdr.md](xdr.md): 5.2 and 5.4 | 2, 5, 6, 7 |
| D5 | DECISION | the VL and FS read specification (#65): its 5.1.1 (framing, services) and section 2, point 3 (procedures not used; its sentence on the time procedure is superseded by this specification, section 13, Q10) | 5.1 |
| D6 | DECISION | the seams specification (#71): its 5.3, points 1 and 2 (`Stamp`, `Deadline`, episodes, the sampling function) and 5.4, point 1 (`Reply`, `ReplyEvidence`, the trust conditions) | 4, 5.6, 5.7, 6 |
| D7 | DECISION | [ADR 0001](../adr/0001-licence-and-dont-copy.md), "Don't copy"; [coding standards](../coding-standards.md), rules 1 to 3, 15 and 20 | 6, 7, 10, 12 |
| O | OWN | every point tagged `[OWN]`, with its reason in place | 5, 6, 7 |

**What was not used.** No source code, test data, interface definition
or generated definition of any AFS or Rx implementation was read for
this specification, and none is cited. The community Rx specification
draft was not opened for it: what it needs of Rx is taken from
[rx.md](rx.md), and its section "Historical Implementation Notes" was
not read (README, "Sources and provenance", rule 12). No copy of the
1991 AFS-3 programmer's references was used: no copy whose provenance is
independent of every implementation's repository has been found, so
none is registered (rule 10). The capabilities draft's appendices, which
hold sample interface definitions, were not opened; the procedure
definition fragment in its §4 prose is not used for any wire fact. The
registry rows on server statistics whose source column names an
implementation's change history are not used. The reference pages are
cited for facts only, in IronAFS's own words; no command example or
output is reproduced. The procedure name in 5.1.2's second column is the
registry's, used only to identify the procedure; every other name here
is IronAFS's own.

### 12.1 Observed behaviour

None yet. The capture-backed version records the lab's fixtures here
(#51), and the test-cell cases TK1 to TK4 (#47): a test venue with
synthetic cell names, users, keys and data; raw bytes first, a protocol
analyser only as a later cross-check; each observation with its venue,
server release, date, scenario and the evidence label EXECUTED.

## 13. Open questions

- **Q1. The capture-backed version** (#51; #47). Whether the target
  servers answer procedure 153 over the null class without a token
  (TK1); its arguments, if any; its reply's fields, widths, signedness,
  units, epoch and granularity, from raw bytes with the server's clock
  set to known values; whether they accept the keepalive's random
  identity (TK2); whether it has side effects (TK3). Recommendation: one
  campaign with the callback specification's Q1 and the VL and FS read
  specification's Q1. Blocks: the keepalive's codec and every code path
  that raises trust; the gate of 5.7, point 1.
- **Q2. Allowlisting the registry.** The registry (E7) is a
  published list of assigned numbers kept apart from any
  implementation's repository; no passage is allowlisted, so 153 and the
  port are UNVERIFIED until a capture shows them. Recommendation: the
  same as the callback specification's Q3 and the VL and FS read
  specification's Q2: allowlist the "Code" column of `rx/RXAFS.txt` for
  rows whose source column names a release or a registration request,
  for numbers only. It is the owner's decision on clean-room policy (ADR
  0001).
- **Q3. The coverage condition in the serve path.** Settled in this
  version: callbacks.md records K's `lowerings` with every call's fence
  tuple, compares it in the trusted branch of its 5.6, point 5, raises it
  at every lowering and at a lapse (its 5.6, point 11), and tests it
  (its C26). Kept here so that the question's number stays stable.
- **Q4. The keepalive procedure.** The capabilities draft recommends the
  capability query where clients formerly called the time procedure
  (K2 §4.1). This version keeps the time procedure (5.1.2).
  Recommendation: keep it for M2a; revisit when IronAFS reads capability
  words on a protected channel (N22), or if TK1 shows a target server
  that answers the capability query without a token and not the time
  procedure.
- **Q5. The seams for the keepalive.** Two gaps in the seams
  specification (#71): its `Rpc::call` has no way to mark a call as the
  keepalive, which [rx.md](rx.md), 5.7, point 1, requires; and its
  sampling function takes `from` only from a request's arrival or a
  reply's `first_sent`, while the keepalive's due times and call
  deadline start from housekeeping, which has neither for its first
  keepalive. Recommendation: amend #71 with a separate
  `Rpc::keepalive` call (same arguments and `Reply`, so only the
  keepalive owner can ask for a random call number on an unused
  channel), and with housekeeping's tick recorded as an arrival, so a
  due time counts from a recorded stamp like any other deadline. A
  third gap: #71's `Rpc::open` takes no idle time, while the keepalive
  connection is opened with `ka.conn_idle` (5.2); add it to the same
  amendment (an idle-time argument to `open`, or to `Rpc::keepalive`'s
  connection). A fourth: `Rpc::open`'s error has no cause for rx.md's
  `IdentityExhausted`; add one (a `NotSent` cause), so an identity skip
  is counted as such. Every failed open frees its slot whatever its
  cause (5.6, point 1), so the slot table does not wait on it. Until then a connection Rx retires early is a local
  outcome (5.6, point 8), which costs a new connection and never
  lowers trust.
- **Q6. The keepalive's connection key.** C30, point 3, names a
  credential context in every connection key; the keepalive belongs to
  none (5.6, point 4). Recommendation: the client's connection
  specification names the value "none" for the keepalive only, so no
  credential context's connection can carry a keepalive and no keepalive
  connection carries a context's call.
- **Q7. Server time in M2a.** In this version server time is "not
  measured" (5.7, point 1). If the capture-backed version qualifies the
  procedure, its fields and its unauthenticated availability, the skew
  check of 5.7, points 3 and 4, is turned on for M2a in that version.
  Otherwise the check moves to M2b, as the conditional item M2b-34 of
  the plan (#41), which is opened as its own issue then.
  Recommendation: decide at the end of the lab's first campaign, on its
  fixtures only.
- **Q8. The form of a promise's expiry.** Whether it is a duration or a
  time on the server's clock is the callback specification's Q1. 5.7,
  point 6, bounds either case; #82's row for it, and callbacks.md, 5.1.4,
  quote that bound when the form is known. Recommendation: settle it in
  the same campaign as Q1.
- **Q9. A server with several addresses.** The keepalive goes to the
  address the client's calls use. A NAT that filters by the remote
  address `[N1 §5: CONFIRMED]` drops a server's calls from any other address of that
  server, and from another port. Recommendation: keep one address until
  the callback specification's TC5 shows where callbacks come from; if a
  server's callbacks come from another address, keep that address's
  mapping open too.

- **Q10. The VL and FS read specification's note on the time
  procedure.** #65, section 2, point 3, lists the file-service time
  procedure (153) among the procedures not used in M2a, and leaves "the
  keepalive and the server probe" to C32 and the callback
  specification. This specification makes that procedure M2a's
  keepalive (5.1.2). Recommendation: amend #65's point in its own pull
  request so that it says the procedure is used, by this specification
  only, as the keepalive, and that #65 decodes nothing of it. Blocks:
  neither specification's content, only their agreement once both are
  on `main`.

## 14. Review

Reviewed by GLM and DeepSeek in three rounds on 2026-11-21; the record
is [2026-11-21-m2a-81-keepalive](../reviews/2026-11-21-m2a-81-keepalive.md).
