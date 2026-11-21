<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0006: Rx: what an unprotected packet may change

Status: proposed (2026-11-20). Raised in issue #52. Class S. Numbered
0006 because 0005 is reserved for the `ironafs-os` ADR, in its own pull
request. It is accepted before the first task of the `ironafs-rx` crate.
Partly supersedes the parts of ADR 0004 named under "What it supersedes"
(C30, C31, C32 and O23). Interim, pending the owner's decisions (Q3, Q4:
open questions 3 and 4).

## Context

An off-path attacker can send UDP with a forged source address but cannot
see the client's traffic. If it can guess enough of a call's Rx identity,
an unprotected ABORT ends the call: a fetch fails, or a store's outcome
becomes unknown. Repeated, that is a denial of service against any user.

ADR 0004 covers part of this and leaves a gap:

- No security class protects an abort (part 1, Context, point 3). C19
  keeps aborts out of authenticated state, C24 limits weak evidence, N5
  classes an Rx abort as a hint and N6 says an abort never lowers the
  protocol profile.
- C32, point 1, draws the keepalive call's identity from `Rng`. No
  decision says how any other connection's identity is chosen (C30,
  point 3, defines the connection key only), or which ABORT, BUSY or
  CHALLENGE packets a connection accepts.

**Terms.** A connection's **wire identity** is its epoch and connection
id as sent in the Rx header. Here **connection id** always means the
30-bit field, not the 2-bit channel that shares its word; the channel is
named separately wherever it is matched. The wire identity is not C30,
point 3's connection key (the
selector that chooses which connection a request uses); one key may own
several connections over time, each with its own wire identity. A
**mutation** is what N7, point 2, and C28 mean by one: a store or other
journalled operation (N34). A **server-state call** is any other call
whose requested effect is to change server state with no journal record,
such as setting or renewing a server lock (L33). The call's requested
effect decides, not protocol bookkeeping it incurs: a fetch that also
establishes a callback promise is a read, not a server-state call.

**Scope.** This ADR decides ABORT, BUSY and CHALLENGE packets, and ICMP
and socket errors. Acceptance of DATA, ACK and RESPONSE packets,
including a reply that completes a store, is not decided here: it
follows C19, C24, C28, N5 and N7, and is set by the Rx and security-class
specifications, each of which keeps decision 1.

Protocol facts. "Rx draft" below is the "Rx protocol specification draft"
(Zeldovich, updated by Altman), copy dated 24 July 2026, AFS Assigned
Numbers Registry, <https://registrar.central.org/rx-spec.txt>. It has no
section numbers and no licence notice; it is cited by heading, for facts
only. "rxgk -11" is draft-wilkinson-afs3-rxgk-11 (expired individual
submission, not a standard). These facts were read at source by a review
seat on 2026-11-20 and are re-checked at source by the Rx specification
task before it cites them.

| Fact | Label and source |
|---|---|
| The initiator chooses a 31-bit epoch; the draft recommends random epochs and permits a fresh one per connection | CONFIRMED, Rx draft, "Rx Connections" |
| The initiator chooses a 30-bit connection id, sharing its word with a 2-bit channel; the draft recommends connection ids unique at the initiator, independent of epoch, service, security class and remote endpoint | CONFIRMED, Rx draft, "Rx Connections" |
| Call numbers strictly increase per channel and may start at any positive value; call number 0 marks connection-level packets | CONFIRMED, Rx draft, "Rx Connections" and "Call Layer" |
| The draft's channel retirement boundary is inconsistent | CONFIRMED (internal to the draft), "Rx Connections" |
| A sender's serial advances for every packet on the connection, retransmissions, ABORT and BUSY included; it normally starts at 1 | CONFIRMED, Rx draft, "Call Layer" |
| ABORT carries a 32-bit error code; an ABORT with call number 0 signals connection termination; BUSY has no payload; the acceptor sends CHALLENGE | CONFIRMED, Rx draft, "Packet Types" |
| No Rx security class can protect an ABORT | CONFIRMED, Rx draft, "Packet Types"; rxgk -11, section 12.1 |
| An rxgk CHALLENGE carries a 20-byte nonce; the RESPONSE carries a start time, a token and an encrypted authenticator | CONFIRMED, rxgk -11, sections 8.4 to 8.6 |
| Released servers accept a random epoch, a random connection id and a first call number above 1 | UNVERIFIED: the M2 test cell |
| Whether any epoch or connection id value (0, for example) is reserved | UNVERIFIED: the Rx specification task |
| The serial, service id, security index, source address and port that real peers put on a genuine ABORT, BUSY or CHALLENGE | UNVERIFIED: the M2 test cell |
| Whether a BUSY sent by an initiator to an acceptor has any meaning | UNVERIFIED: the Rx specification task |
| How predictable the epoch, connection id and call numbers are that real servers choose for callback calls | UNVERIFIED: the M2 test cell |
| What a server does with a forged ABORT aimed at its side of a call | UNVERIFIED: the M2 test cell |
| Whether rxkad fills or checks its security-specific header field on an ABORT | UNVERIFIED; given no security credit |
| The epoch word's top bit is an "ignore source" flag, leaving 31 bits of epoch | UNVERIFIED here (the earlier seat's reading of "Rx Connections"); the Rx specification task. Point 1 does not rest on it: the client matches the peer's address whatever the bit says |
| Which call number and channel a CHALLENGE, and an ABORT with call number 0, carry | UNVERIFIED: the Rx specification task (specification item 1) |

A random epoch and connection id per connection give up to 61 bits an
off-path sender must guess (60 if the ignore-source bit, which the
client keeps clear, proves to lie inside the 31-bit epoch rather than
above it: fact table). The endpoint is known and the channel has 2
bits, so they must match but add no secret worth counting. The call
number adds bits only where it is drawn from `Rng` (the keepalive's,
C32, point 1; other calls' only if specification item 4 adopts it); the
bound stated here counts only the epoch and connection id, even where the
call number is random. This is a calculation from
the field sizes, not a cited fact. The bound is per
connection, not per call: every call on the connection shares it, and an
attacker who has learned the wire identity (one on the path) is not
bounded by it at all. The serial adds nothing secret: it counts packets.

This needs a decision now because the acceptance rule is a security
invariant of the Rx crate, and the crate is written next.

## Decision

### 1. An unprotected control packet must match before it acts, and then acts narrowly

**Firm.**

The client treats every ABORT, BUSY and CHALLENGE it receives as
unprotected, at every security class.

1. **A call-level ABORT or BUSY on an outgoing call** is considered only
   if it matches that live call: the receiving local endpoint, the peer's
   address and port as seen on that socket, the direction, the epoch,
   the connection id, the channel and the channel's current call number.
   The client sets the ignore-source bit clear on its own connections and
   always matches the peer's address itself. The service id and the
   security index are matched only as far as the Rx specification, from
   test-cell evidence (test 14), shows genuine peers send them on control
   packets; a mismatch in either is counted as a diagnostic and does not
   by itself drop the packet (owner, 2026-11-20). They add nothing to the
   guessing bound, which rests on the random epoch and connection id
   (Context); the endpoint, direction and channel must match too but are
   not secret, and the call number must match but the bound credits none
   of its bits, even where it is drawn from `Rng`.
2. **An ABORT with call number 0** is considered only if it matches a
   connection's full connection-level tuple: the same fields as point 1
   without the call number, with the channel matched as the Rx
   specification says such a packet carries it (specification item 1). A
   matching one is a hint (N5, point 2)
   and is counted. On a connection that meets point 9's condition, its
   only further effect is point 6's, applied to the context that owns the
   connection; on any other connection it is counted only. It never ends, fails,
   fences or retries a call; never closes, re-keys or renegotiates the
   connection; and never changes its wire identity. The connection's live
   calls settle by their own replies, deadlines and retry rules; a call
   that times out after such a hint and had sent any part of a mutation
   or a server-state call follows point 5's rule for it.
3. **A CHALLENGE** is answered only if all of these hold, and otherwise
   no RESPONSE is sent:
   - its own discriminator holds: its call number and channel are those
     the Rx specification says a CHALLENGE carries (specification item 1),
     checked before the lookup; any other value is point 4. This replaces
     point 2's channel rule, which is the call-0 ABORT's own
     discriminator. The values are a packet-type check, not peer
     behaviour in a field that carries no secret; if test 14 shows
     genuine CHALLENGEs with other values, that is a trigger below;
   - it matches an outgoing connection by the common connection lookup
     (receiving local endpoint, peer address and port, direction,
     epoch and connection id, with the service id and security index as
     point 1 says; specification item 1 defines it once for points 2 and
     3), not by the
     wire identity alone, and that connection is in a state the Rx
     specification lets the peer challenge (specification item 6);
   - the RESPONSE goes only to that connection's peer address and port,
     from its own local endpoint, on its own wire identity;
   - when the RESPONSE is sent, not only when the CHALLENGE arrives, N7's
     predicate permits that connection with the credential generation and
     fences then current (N7, points 2 and 5); a RESPONSE overtaken by a
     credential change or a fence is discarded, never sent;
   - the RESPONSE uses that connection's own security class and level
     (N6, N7).
4. **Anything that does not match** is dropped without a reply, in
   bounded work and with no allocation per packet. It is counted per
   source in a table of fixed size, with one aggregate counter once the
   table is full. No per-packet log line is written. The work spent on
   unmatched packets is bounded so that they cannot starve live calls
   (specification item 7).
5. **A matching call-level ABORT ends that one call and nothing else.**
   - A call that sent no part of a mutation or server-state call fails,
     or retries under its normal budget inside the effective floor (N6,
     point 1).
   - **A call that sent any part of a mutation** has an ambiguous outcome:
     an unprotected ABORT cannot show whether the server applied it. The
     same holds however such a call ends without a valid reply: a matching
     ABORT, a timeout after a call-0 hint (point 2), or a plain timeout.
     No transport retry, on this connection or on a fresh one
     (specification item 8), sends the mutation again. The transport reports the outcome as
     unknown to the operation that made the call. For a journalled
     operation of every N34 kind (N34, point 1's write, truncate,
     create, rename, remove, link and unlink, and any kind added under
     N34, point 2),
     the lost-reply rule (C28, point 4) is the one rule, as N7, point 2,
     and N45, point 4, apply it to every sent mutation:
     - the record is quarantined as a conflict: neither the transport nor
       automatic journal replay resends it, and it waits for explicit
       recovery, whose retry C28, point 5, allows and C29, point 4,
       specifies (C28, points 3 and 4);
     - its outcome is read back (C28, point 4; C16, point 6). The
       read-back is reported to the user; it cannot show which operation
       produced the server's state, so it never releases the quarantine.
       A read-back that cannot be made yet (the server unreachable, for
       example) leaves the record quarantined, as above;
     - a kind is not built until its specification defines how its
       outcome is read back. Settling a kind without a read-back would
       change Firm C28, point 4, and C16, point 6, and needs a superseding
       ADR, not a specification.
     N7, point 2, applies the same rule to a fenced mutation.
   - **A server-state call** (Terms) whose part was sent and that ends
     without a valid reply has an unknown outcome too. The transport
     never resends it and reports the outcome as unknown to the service
     that made it, which settles it under ADR 0004 and its own
     specification. For a server lock set or renewal, the packet alone is
     not a failed renewal under L33, point 7, and marks no lock lost, the
     sponsor's or another holder's: an unknown renewal outcome neither
     extends nor disproves coverage confirmed before the call, so that
     coverage stays as it was. Loss or expiry is marked only when
     established on its own: by L33, point 7's events, or when L33, point
     12's continuity check can no longer establish the coverage; L33,
     point 7, then applies to every lock that relied on it (L33, point
     10). C24, point 6, separately bars the packet from ending another
     context's coverage, which is not on its closed list.
     How the unknown outcome is then settled is the lock service's
     specification (specification item 11). Any other server-state call
     names its rule in its specification before it is built.
   - Whatever its code says (C46, point 3), it never:
     - marks a server down for other calls or other contexts;
     - discards, expires or marks unusable a token or credential; a
       token-expired or revoked code is reported as a server rejection
       (N4, point 4), never as local expiry;
     - settles, drops or replays a journal record;
     - changes a negotiation profile, floor or pin (N5, point 3; N6,
       point 1);
     - raises callback trust (C24, point 7), counts as evidence that the
       server reset the client's callback state, advances a suspect epoch,
       evicts or retires anything (C24, point 3), or records absence
       (C19, point 2).
6. **It may lower trust, and only so,** and only for an ABORT on a
   connection that meets point 9's condition; on any other connection a
   matching ABORT, call-level or call 0, changes only counters:
   - it counts against that server's health in the context that owns the
     connection, and in no other; that count never steers another
     context's choice of server;
   - the one shared effect: callback trust for that server, address family
     and local endpoint generation (C32, point 1's key) moves to bounded
     revalidation (C32, point 2), the one transition C32, point 3, allows
     on an error. It suspends callback promises and so stays inside C24,
     point 6's closed list.
7. **A matching BUSY** is accepted only while the outgoing call is waiting
   to be accepted: no DATA or ACK packet of that call has been accepted
   from the peer (specification item 3 lists the events that close the
   window). It delays the retry with a bounded backoff. Cumulative BUSY
   deferral per call is capped so that the call keeps its normal
   retransmissions inside its existing deadline; past the cap, a BUSY is
   counted and ignored. It never fails the call.
8. **On an incoming callback call** (the client is the acceptor and the
   remote side chose the identity), the lookup is exact: a live incoming
   call found by the fields of point 1. A miss is point 4. A matching
   ABORT is counted and stops only the client's reply and its
   retransmissions:
   - once the request is complete, its C31 effects apply on arrival (C31,
     point 1) and stand; an ABORT never suppresses, delays or undoes them,
     and a break from a known server is never discarded (C31, point 2);
   - an incomplete request is not cut short: it is kept until its own
     receive deadline (specification item 9) and applied if it completes;
   - a BUSY on an incoming call changes nothing but a counter.
   Neither raises callback trust.
9. **Precondition for points 5 and 6.** Point 5 ends a call only on a connection
   whose wire identity is unpredictable: the client initiated it, and its
   epoch and connection id were drawn whole from the cryptographic random
   source (C5, point 1), never derived from a clock, a counter or another
   identity, and checked against the live and reserved identities the
   client holds (decision 2, point 1, says how). The condition is met by
   how the identity was drawn and checked, not by proof that it never
   occurred before: a repeat by random collision past R or across a
   restart (decision 2, point 1's stated residual) does not remove it. The
   same condition gates point 6's effects,
   for call-level and call-0 ABORTs alike. On any other connection a
   matching ABORT is a counter-only hint: it has none of point 6's
   effects, is not an error under C32, point 3 (as decision 2, point 5,
   reads it), and the call completes on
   a valid reply or, with none, settles at its deadline. Incoming
   callback calls are point 8's.

Why: a forged packet that matches nothing must cost only bounded
counting. A forged ABORT that matches costs at most one call (for a
mutation, an unknown outcome settled by the operation's own rule, such as
a record quarantined for explicit recovery under C28, point 4, never a
lost or repeated write), plus the transition C32, point 3, allows on an
error, plus counters; and it can end a call or lower trust only by
matching an unpredictable wire identity (point 9). The other matching
packets have their own bounds, whatever the identity: a BUSY only defers
a call within its deadline (point 7); a CHALLENGE only draws a bounded
RESPONSE to the connection's own peer (point 3, specification item 6);
an ABORT on an incoming callback call only stops the client's reply
(point 8). For ABORT, BUSY and ICMP the rule
is local to the client. It still changes which genuine exchanges
complete, and the identity rule changes the values the client sends, so
genuine peer behaviour is checked at the test cell (test 14) and a
mismatch is a trigger below. It is Firm because it is a security
invariant (README, "Firm and working decisions"). It is a decision of
its own, not a point inside C30 or C32, so that revisiting their sockets
or intervals does not reopen it.

### 2. Wire identity, ICMP, incoming ABORT and the keepalive (beside C30, C31 and C32)

**Working.** Each point names its own "Revisit if". Each preserves every
firm decision above and in ADR 0004. C-, L-, N- and O-numbered decisions
are ADR 0004's; "point N" alone is a point of this decision.

Interim, pending the owner's decision (Q4): these points are this ADR's
own decision (option (b), partial supersession), not amendments written
into C30, C31 and C32.

1. **Wire identity (beside C30).** Every connection the client initiates
   draws its epoch and connection id (its wire identity, distinct from
   C30, point 3's connection key) fresh from the production `Rng` of C5,
   point 1 (the cryptographic random source). One fresh epoch per
   connection, not one per process. A value the Rx specification reserves
   is redrawn. A connection id is unique among the client's live
   initiated connections whatever their epoch, as long as its connection
   lives. When the connection retires, only its epoch and connection id
   pair stays reserved, for a fixed, finite reservation window **R**; a
   draw that collides with a live connection id or a reserved pair is
   redrawn. So a retired connection id may be drawn again at once under
   another epoch, and the same pair may repeat after R. That is a stated
   departure from the Rx draft's recommendation of connection ids unique
   at the initiator. Incoming callback connections keep the identity
   their initiator chose: they are never redrawn, never checked against
   this point's uniqueness, and are told apart from initiated ones by
   direction (decision 1, point 8). Specification item 8 sets R at no
   less than the longest time a peer may still send on the retired
   connection (its longest call deadline plus retransmission) plus a
   stated margin for network delay. The number of reserved identities is
   bounded. A new connection is admitted only while the live initiated
   connections plus the reserved pairs, counting the new one, stay within
   that bound, so that every live connection can be reserved when it
   retires. When the bound would be exceeded, or a bounded number of
   redraws finds no usable value, no new connection is opened (the
   request waits or fails at its deadline) and no reservation is released
   early (specification item 8). Identities are never persisted or reused
   across a daemon restart. A packet delayed past R, or arriving after a
   restart, can match only if a new identity repeats the old one by random
   collision (about 2^-61 per pair, or 2^-60 under the Context's caveat),
   a stated residual. No clock or counter is an input. Revisit if: a
   supported server mishandles a random epoch or connection id, or relies
   on connection ids unique beyond their connection's life.
2. **ICMP and socket errors (beside C30).** An ICMP error received on the
   client's sockets, and an asynchronous error the operating system
   reports on a UDP socket for one (on a send or a receive), is a hint
   (N5, point 2). It never closes or rebinds the family's shared socket,
   never changes the network generation of C23, point 2 (taken only from
   the operating system's own address and route state), never fences
   grants, never advances a suspect epoch and never changes negotiation.
   A call completes on a valid reply or, with none, settles at its
   deadline. Path MTU signals, if used, change only packet sizes
   (specification item 10). Revisit if: M2 measurements show failover too
   slow because of it.
3. **ABORT and BUSY on an incoming call (beside C31).** Decision 1, point
   8, applied to C31's table: on an incoming callback call, a matching
   ABORT stops only the client's reply and its retransmissions; a complete
   request's effects stand, as C31, point 1, applies them; an incomplete
   one runs to its receive deadline and is applied if it completes; a BUSY
   changes only a counter; and neither raises callback trust. Revisit if:
   the test cell shows genuine incoming-call ABORT or BUSY traffic that
   this point handles wrongly.
4. **The keepalive's identity (with C32, point 1).** The keepalive
   connection's epoch and connection id are drawn as point 1 says; that is
   the random identity C32, point 1, and C24, point 7 (firm), require. Its
   call number stays drawn from `Rng`, as C32, point 1, requires;
   specification item 4's fallback to 1 does not apply to it. The
   keepalive's epoch and connection id stay random under every amendment
   of point 1, because C24, point 7, requires a random identity; an
   amendment that let other connections use a predictable identity would
   not apply to it. Trust is still renewed only by a reply that completes
   that keepalive call, on the current path and network generation.
   Revisit if: a supported server rejects the keepalive's identity (its
   random epoch, connection id or call number); that is decided by its own
   change, with that evidence.
5. **ABORT as "an error" (changes C32, point 3).** An unprotected ABORT
   is "an error" in C32, point 3, only as decision 1, point 6, allows: a
   matching ABORT, call-level or call 0, on a connection that meets
   decision 1, point 9's condition moves the server to bounded
   revalidation; a matching ABORT on any other connection, and an ABORT
   that matches nothing, is not such an error. Every other error a call
   shows is unchanged. Because assumption A14 lets the loss be learned
   "through ... an error", and an ABORT on an ineligible connection can no
   longer show it, a server is held in bounded revalidation (C32, point
   2), never in C32, point 3's uncapped branch, while any connection the
   client uses to it fails decision 1, point 9's condition, whether or not
   any packet arrives on it. When that hold ends, these callback promises
   from that server stay suspended (C24, point 2) until each entry is
   revalidated by a call sent after the hold ends: every promise held at
   any time during the hold, and every promise granted by a reply to a
   call sent before the hold ended, on any connection and whenever that
   reply arrives (a fetch or validation outstanding when the hold starts,
   or begun during it). The server's loss signal may have come as an
   ABORT the client only counted, before such a reply arrived; neither
   the end of the hold, a keepalive reply (which shows reachability, C32,
   point 1), nor that delayed reply restores those promises. Under point
   1, every initiated connection meets the condition, so the hold applies
   only if point 1 is later weakened. Revisit if: C32, point 3, is revisited, or the test of
   assumption A14 settles how a server reports a lost callback.

Decision 1 does not depend on these points for its acceptance rule. If
point 1 is amended because servers reject random values, the acceptance
rule stands, decision 1, point 9, turns a matching ABORT on an affected
connection into a hint, and the guessing bound falls; that is a trigger
below.

### How this ADR relates to ADR 0004

This ADR adds decision 1 (Firm) beside C19, and decision 2 (Working)
beside C30, C31 and C32. It also states, for O23 (Firm), the residual of
a network attacker at every security class. Decision 2 and the O23
statement are partial supersessions of ADR 0004, listed under "What it
supersedes"; decision 1 is a new decision beside C19 and supersedes
nothing in ADR 0004. ADR 0004's
decision text is not edited: its status lines, and an annotation at each
superseded decision, point readers here.

Interim, pending the owner's decision (Q4): option (b), partial
supersession. Interim, pending the owner's decision (Q3): option (a), as
revised in question 3.

The route is supersession because the README's amendment route needs a
fired trigger or contradicting evidence, and neither is present. No named
trigger of C30, C31 or C32 has fired. Point by point:

- Decision 2, points 1 to 3, are additions alongside C30 and C31: a gap,
  not a contradiction. C30 says nothing on how a connection's identity is
  drawn or what an ICMP or socket error does; C31's table says nothing on
  an ABORT or BUSY on an incoming call. C30's and C31's own points stand.
  The Context shows the gap is exploitable: a predictable identity lets an
  off-path sender end calls with few guesses.
- Decision 2, point 4, restates for the keepalive what C32, point 1,
  already requires and cites point 1 for how the epoch and connection id
  are drawn; C32, point 1's meaning is unchanged.
- Decision 2, point 5, is the one existing rule whose meaning changes:
  C32, point 3's "or an error" conflicts with decision 1, point 9 (an
  ABORT on an ineligible connection is not such an error). The conflict
  is with a new firm decision, not with new field evidence. C32's own
  premise is about the keepalive reply that raises trust; a forged ABORT
  that lowers trust does not forge that reply, and C24, point 2, already
  lets weak input lower trust, so the Context does not contradict that
  premise.

### What it supersedes

In ADR 0004, part 1 (interim, pending the owner's decision (Q4)):

- **C30:** decision 2, points 1 and 2, add the wire identity rule and the
  ICMP and socket-error rule. C30's points 1 to 4 stand.
- **C31:** decision 2, point 3, adds the rule for an ABORT or BUSY on an
  incoming call. C31's table and points 1 to 4 stand.
- **C32:** decision 2, point 4, ties the keepalive's epoch and connection
  id to point 1, with C32, point 1's meaning unchanged; decision 2, point
  5, changes what C32, point 3's "or an error" covers, and adds the hold
  and its exit.

In ADR 0004, part 3 (interim, pending the owner's decision (Q3)):

- **O23:** the threat model gains a network attacker at every security
  class, not only against rxkad, for unprotected Rx control packets.
  Defences: decision 1; decision 2, point 1. What remains: no Rx security
  class protects an ABORT (Context, fact table); an attacker on the path
  sees the wire identity and can end calls with a matching forged ABORT,
  a denial of service bounded by decision 1, points 5 and 6; an off-path
  one must first guess a random wire identity (decision 2, point 1). BUSY
  and CHALLENGE are treated as unprotected by the client's own policy
  (decision 1), not on a cited fact: a forged BUSY only defers a call
  within its deadline, and a forged CHALLENGE only draws a bounded
  RESPONSE to the connection's own peer. This ADR requires the residual
  to be stated in the administrator's guide, as O23, rule 3, requires for
  each residual in its table, and in the threat model document when that
  is written. O23's table
  is not edited; an annotation there points here.

Everything else in ADR 0004 stands.

### Specification items for the Rx specification

Each must keep decision 1.

1. The exact match of points 1 to 3 and 8, the order of the checks, and
   the decoding of each control packet with its size limits (short
   packets, trailing bytes). The common connection lookup of points 2
   and 3, defined once. Each packet type's own discriminator, checked
   before that lookup: the call number and channel a call-0 ABORT
   carries, and those a CHALLENGE carries; a negative fixture only where
   the specification forbids some value. The local endpoint and peer tuple as seen on
   each family's socket, normalised once: IPv4, IPv6, IPv4-mapped forms
   and NAT64-synthesised addresses (C34, point 5).
2. No serial window: the serial is not an acceptance condition. It adds no
   secret, and a window would drop genuine reordered packets.
3. BUSY: the events that close point 7's window, the backoff bounds, and
   the cap on cumulative deferral, all inside the call's deadline.
4. The first call number of calls the client initiates, other than the
   keepalive's (incoming callback calls keep the number their initiator
   chose, point 8):
   drawn from `Rng` in a range well below retirement (for example 1 to
   2^30) only if the test cell accepts it; otherwise 1. The keepalive
   call's number stays random (C32, point 1).
5. Channel retirement before signed overflow, given the draft's
   inconsistent boundary.
6. RESPONSE: the connection states a CHALLENGE may be answered in, the
   send-time recheck of point 3, and retransmission: a repeated CHALLENGE
   is answered again, within a per-connection rate limit and a bound on
   RESPONSEs per handshake, so that a lost RESPONSE or a restarted peer
   recovers.
7. The drop counters: the fixed-size per-source table and its aggregate
   counter, the per-source rate limit, the work budget for unmatched
   packets, and their events in the catalogue (O1), shown in diagnostics
   (O12).
8. Wire identity retirement: the value of the reservation window R of
   decision 2, point 1 (finite and fixed, no less than the retired connection's
   longest call deadline plus retransmission, plus a stated margin for
   network delay); the bound on reserved identities, with admission that
   counts live initiated connections against it; the redraw budget (a
   stated number of draws); decision 2, point 1's fail-closed outcome when
   either is reached; and when a call that sent no part of a mutation or
   server-state call and timed
   out, including after a matching call-0 ABORT, retries on a fresh
   connection with a fresh wire identity, through N7. A call that sent any
   part of a mutation or server-state call is never retried this way
   (decision 1, point 5).
9. Incoming calls: the lookup of point 8, and the receive deadline of an
   incomplete request.
10. ICMP and socket errors: how each operating system reports them on a
    UDP socket, how each is attributed to a peer, and how path MTU
    signals are used, if at all; none ends a call or changes trust.
11. For the lock service's specification, not the Rx one: how a server
    lock set or renewal with an unknown outcome is settled under L33, and
    whether a new renewal call may be made before the lease ends (L33,
    point 12). Both keep decision 1, point 5: the packet alone marks no
    lock lost. Tested with one renewal covering two holders in different
    contexts: after a matching ABORT on the sponsor's renewal, the other
    holder's read and write continue while the coverage confirmed before
    it lasts, and fail (L33, point 7's list) once continuity can no longer
    be established.

## Consequences

- Forged unprotected control packets from an off-path network attacker,
  at every security class, are covered by design once this ADR, with its
  decision 2, lands, and in practice once the Rx crate passes the
  tests below and test 14 shows genuine peer packets match.
- **Residual** (recorded for O23 under "What it supersedes"; interim,
  pending the owner's decision (Q3)): an attacker on
  the path sees the wire identity and can still end calls. That stays a denial of service, bounded by points 5
  and 6, as for any UDP protocol without packet authentication. On a
  store, it leaves a quarantined record for the user's explicit recovery
  (C28, point 4), never a lost or repeated write; on a server lock set
  or renewal, an unknown outcome the lock service settles under
  specification item 11, never a lock lost on the packet alone.
- A genuine connection-level ABORT, a genuine ICMP unreachable, and a
  genuine ABORT on a connection without an unpredictable identity
  (point 9), take until a deadline to surface. That is slower failover,
  not lost data.
- A genuine ABORT on a store quarantines the record instead of failing it
  cleanly. The read-back is reported but does not settle it: the record
  stays quarantined for the user's explicit recovery (C28, point 4).
- A genuine ABORT sent from a server address other than the connection's
  peer is dropped and costs a timeout. Whether servers do this is
  UNVERIFIED.
- The same forged packet aimed at the server may end the server's side
  of a call (UNVERIFIED). The client then sees a timeout or a real abort,
  and settles as above.
- The bound does not carry over to incoming callback calls, whose identity
  the remote side chooses. Point 8 limits their effect instead: a forged
  ABORT there costs at most the client's reply on the client's side. How
  the server treats a break it got no reply to is assumption A14's
  question (UNVERIFIED). If the server drops other promises without a
  signal the client recognises, the client has no finite bound for
  them while trust lasts: that is the A14 dependency C32, point 3,
  already names, and its fallback applies if the specification gate
  refutes A14 (data under trust served only within a revalidation cap).
- After a daemon restart, or once the reservation window R has passed, a
  new wire identity can repeat an old one only by random collision;
  delayed packets for the old one could then match.

Tests (simulator with deterministic `Rng` and clock, unless stated). Each
has a positive control, so that it fails if the client drops everything:

1. **Guessed identities.** An attacker who knows the peer address, the
   client port, the start time and every earlier connection's identity
   sends N aborts with time-derived epochs and counter connection ids on
   all channels, from one source and within the per-source rate limit,
   with the table not full. No call ends; all N are dropped and the
   source's count rises by N; nothing is sent back. (Counting above the
   rate limit is specification item 7's, tested there.)
   In the same run, an abort with the true identity ends its call.
2. **One field wrong.** For each field of point 1's mandatory tuple (the
   receiving local endpoint, the peer's address and port, the direction,
   the epoch, the connection id and the channel), and the call number
   stale or ahead by one, each with a value the simulator's table
   confirms matches no live call (a live call number of at least 2, so
   that "stale by one" is never 0; a channel with no live call), the
   abort is dropped. A value that selects another live call acts on that
   call only (test 12); call number 0 is point 2's (test 6). All fields
   right, on a read with a retry budget of zero: that call fails and no
   other call is affected. A retry case, if tested, uses a read whose
   fixture retry policy chooses a retry: it retries inside the floor.
   Separately, with the mandatory tuple and call number
   right, a service id that differs (and, separately, a security index
   that differs): the abort is not dropped for that alone; it ends its
   call as a matching abort does, and the diagnostic count rises by one
   (point 1; decided question 2). On both families and on a NAT64 path.
3. **Blast radius.** After a matching abort on a read: other calls on the
   connection, other contexts' health and server choice, the token, the
   profile and pins are unchanged; callback trust changes only by C32,
   point 3's transition, for that server, family and local endpoint
   generation; no suspect epoch advances. On a connection given a
   predictable identity by a fixture (failing point 9's condition), a
   matching call-level or call-0 abort changes only counters: the call
   runs on, and health and callback trust are unchanged. The hold and its
   exit (decision 2, point 5): a trusted server gains a fixture connection
   that fails point 9's condition; the server is in bounded revalidation
   while that connection lives, with and without packets arriving on it;
   a matching abort standing for the server's loss signal arrives on it
   and is only counted; that last ineligible connection retires; a
   keepalive reply then arrives; with the reader's credential, grant and
   the entry's nominal callback lifetime valid throughout, a read of an
   entry cached before the hold is revalidated by a call sent after the
   hold ended before delivery, not served under trust. Then a reply
   in flight, with the reader's credential and grant valid throughout, so
   that only callback continuity is tested: a fetch on an eligible
   connection is outstanding when the hold starts; the server grants its
   promise; the matching abort standing for the loss signal arrives on
   the ineligible connection and is only counted; that connection
   retires; a keepalive reply arrives; only then does the fetch's reply
   arrive. A read of that entry is revalidated by a call sent after the
   hold ended before delivery, not served under trust. The same with the
   fetch begun during the hold. Then a validation that starts and
   succeeds during the hold: with the credential, the grant and the
   promise's nominal callback lifetime valid throughout and the keepalive
   fresh, a validation call is sent during the hold and its successful
   reply arrives during it; then the matching abort standing for the loss
   signal arrives on the ineligible connection and is only counted; that
   connection retires; a keepalive reply arrives. A read of that entry
   is delivered only after revalidation by a call sent after the hold
   ended; the in-hold validation does not count.
4. **Abort on a store.** The simulated server applies a store, then a
   matching abort arrives before its reply: the record is quarantined as
   a conflict and read back, no second store is sent, and after the
   read-back the record is still quarantined, waiting for explicit
   recovery. The same with the abort before the server applies it. The
   simulated server applies a store, then a matching call-0 abort
   arrives and no reply follows: at the timeout the record is
   quarantined; then the read-back completes; then all pending retry and
   connection work is run; then the client is asserted to have issued
   exactly one store call across all its connections (counted as logical
   calls the client issued, not as packets or as what the server
   received, so retransmissions within that call count once and a second
   call whose packets were lost still counts), and the record is still
   quarantined. A server lock renewal ended by a matching abort: the
   transport sends no second copy of that call and reports an unknown
   outcome to the lock service; no lock is marked lost by the packet, and
   a second holder in another context keeps its coverage until
   continuity can no longer be established (specification item 11).
5. **Abort code as bait.** A matching abort with a token-expired or
   bad-ticket style code. On rxkad: the live credential is kept, the
   cause is reported as a server rejection, and any retry stays inside
   the floor. On a permitted rxnull connection (N7): the anonymous
   context is unchanged, any credential in another context is intact,
   and the cause is reported as a server rejection.
6. **Connection level.** A matching call-0 abort is counted as a hint;
   the connection is not closed, fenced or re-keyed and serves the next
   call. In both orders: a call whose valid reply arrives after the hint
   completes normally at once; a call that gets no reply ends at its
   original deadline, not earlier. A non-matching one is
   dropped and counted. On a connection failing point 9's condition, a
   matching one changes only counters (callback trust unchanged). Where
   the Rx specification forbids some channel value on a call-0 abort, one
   with a valid connection tuple and that channel is dropped.
7. **BUSY.** Before acceptance it delays the retry; after a DATA or ACK
   of the call it is dropped; a BUSY stream stops deferring at the cap,
   the call keeps its retransmissions, and it never fails before its
   deadline.
8. **CHALLENGE.** A matching CHALLENGE draws one RESPONSE and the
   handshake completes. A lost RESPONSE: the repeated CHALLENGE is
   answered, within the rate limit. A peer restart: the handshake
   recovers. A credential change or a raised restriction between receipt
   and send: no RESPONSE leaves. A CHALLENGE with no identity match: no
   RESPONSE. A CHALLENGE with the right epoch and connection id but a
   wrong peer address or port, a wrong local endpoint, or the wrong
   direction: no RESPONSE, and the RESPONSE to a genuine one goes only to
   the connection's peer. A CHALLENGE with a valid connection tuple but a
   call number the Rx specification does not allow for a CHALLENGE (and
   a channel, where it forbids some value): no RESPONSE.
9. **Identity generation.** Over many seeded initiated connections,
   every epoch and connection id is the `Rng` output except where
   redrawn, and the ignore-source bit is always clear. With the redraw
   budget set to at least five draws: a fixture `Rng` that returns a live
   identity, then a live connection id under another epoch, then a
   reserved value, then a retired pair still inside R, gets each
   redrawn; past R, the same retired pair is accepted; a retired
   connection id under a new epoch inside R is accepted. Separately:
   on the same local endpoint, a delayed packet for a retired identity
   inside R is dropped; across socket rebinding, a delayed packet is
   dropped and the case is recorded as an endpoint mismatch, not as
   evidence for R. Across a restart: the client's first epoch and
   connection id are the fixture `Rng`'s next outputs, not the retired
   values (no persisted identity is used), and delayed packets for the
   retired ones are dropped; a repeat by collision is the stated residual
   and is not tested as a drop. Separately: (a) with capacity available
   and a fixture `Rng` that returns only unusable values, after the
   redraw budget no connection opens; (b) with live connections plus
   reservations at the bound and a usable draw, no connection opens; (c)
   with a bound of B, B - 2 reserved pairs and one live connection, a
   second connection is admitted on a usable draw (the last slot), and a
   third request waits; while it still waits, both live connections
   retire inside R, before any reservation expires, and are both
   reserved; it stays unadmitted through both retirements and fails at
   its deadline, before any reservation expires. In (b) and (c),
   the refused request's deadline falls before the earliest reservation
   expires, so no capacity frees while it waits. In each refusal, the
   waiting request fails at its deadline, live calls are unaffected, and
   no reservation is released early. An incoming
   callback call whose epoch and connection id equal a live initiated
   connection's reaches its own call, and the initiated connection is
   unaffected.
10. **Callback side.** A complete break, then a matching abort, in both
    orders against the reply: an abort before the reply is submitted
    means no reply is sent; an abort after it is submitted means no
    further retransmission of it. In both the break's suspension is
    recorded and stays. An
    abort between the packets of a multi-packet break: the request still
    completes and is applied. A BUSY on an incoming call: a counter only.
    Callback trust and suspect epochs are asserted unchanged except as
    C31's table sets them.
11. **Flood.** Non-matching packets at a high rate, from many sources:
    live calls keep their latency bound, memory stays within the fixed
    table, and logging is bounded.
12. **Fuzz.** Arbitrary ABORT, BUSY and CHALLENGE headers of any length
    against live calls: no panic, and a call ends only on a match of
    point 1's mandatory tuple and call number on a connection that meets
    point 9's condition.
    Interleaved mutated copies of real packets, with three expectations:
    a copy with an identity field changed that the simulator's table
    confirms matches no live call or connection leaves the outcome equal
    to the run without it, except for counters; a mutated copy that
    happens to match another live call or connection acts as decision 1
    says for that one; a copy with only other fields changed (code,
    serial, nonce) acts as decision 1 says for its type.
13. **ICMP and socket errors.** Attributed and unattributed ICMP errors
    and asynchronous socket errors, on both families, alongside ordinary
    traffic: a live call whose reply arrives while the errors arrive
    completes normally, and a new call started then is served; a call
    with no reply ends at its original deadline, not earlier; the family's
    socket stays open, and the network generation is unchanged.
14. **Test cell (M2).** A real server accepts random epochs and connection
    ids. Independently of item 4's result for other calls, a keepalive
    whose call number is drawn above 1 is accepted, or the rejection is
    recorded and fires decision 2, point 4's trigger. A random first call number for
    other calls is tried for item 4. Genuine
    ABORT (call-level and connection-level), BUSY and CHALLENGE exchanges
    complete or fail as decision 1 expects. The test-cell plan names how
    each is elicited; a type the cell cannot be made to send stays
    UNVERIFIED in the fact table, and its row says so. Recorded per peer and security
    class: the serial, service id, security index, source address and
    port of genuine control packets; restart recovery; the server's
    callback call identities; a NAT64 path.

## Alternatives considered

- **Put the rule inside C30 as a working point.** Not taken: it is a
  security invariant, and a revisit of C30's sockets must not reopen it.
- **Change the README so an amendment may add a firm decision.** Not
  taken: it would weaken the rule that firm decisions change only by a
  later ADR.
- **A serial window.** Not taken: the serial is a packet counter with no
  secret, and a window drops genuine reordered packets.
- **Rely on rxkad's header check on aborts.** Not taken: whether it is
  filled or checked is UNVERIFIED.
- **One epoch per process with counter connection ids.** Not taken: one
  observed connection predicts the others.
- **Let a call-0 abort end every call on the connection.** Not taken: an
  unmatched or loosely matched one would end every call with few guesses.
- **Retry a store after a matching abort.** Not taken: the abort cannot
  show the store was not applied, and C28, point 4, forbids a blind
  replay.
- **Put the identity generation itself in decision 1.** Not taken here:
  the owner placed it in a working point (decided question 1; now decision
  2, point 1, interim, pending the owner's decision (Q4)); point 9 makes
  decision 1's effect depend on the property instead (option A of the
  owner's decision, 2026-11-20; see "Decided questions").

## Revisit if

Decision 1 changes only by a superseding ADR. Reopen it if:

- a security class that protects control packets is offered to clients
  (a protected ABORT could then be evidence under C24);
- the test cell shows genuine ABORT, BUSY or CHALLENGE packets that the
  match of points 1 to 3 rejects in normal operation (for example, a
  source address or port other than the connection's peer), or test 14
  shows peers' service id or security index on control packets differ
  from what the Rx specification records;
- a supported server rejects random epochs or connection ids, so the
  guessing bound no longer holds.

Decision 2's points carry their own triggers, above.

## Decided questions

1. **Where unpredictability lives (finding GPT-2): option A (owner,
   2026-11-20).** The generation rule stays in a working point, as the
   owner's decision that raised this ADR placed it: C30, point 5, when
   decided; decision 2, point 1, of this
   ADR under the interim route of question 4. That move changes only
   where the working point is written, not option A; the owner confirms it
   with question 4. Decision 1, point 9, makes ending a call, and every
   effect of point 6 (shared callback trust included), conditional on an
   unpredictable wire identity; on any other connection a matching ABORT
   is a counter-only hint. A later amendment of the generation rule cannot
   weaken the protection, only slow genuine failover. Not taken: moving
   the generation rule into decision 1 (it changes that split, and a
   server that rejects random values would need a superseding ADR), and a
   stated floor of unpredictable bits instead of "drawn whole".
2. **Service id and security index in the Firm match (finding GLM-F1):
   option B (owner, 2026-11-20).** They are matched only as the Rx
   specification, from test-cell evidence (test 14), shows genuine peers
   send them; a mismatch is counted as a diagnostic and does not by
   itself drop the packet (decision 1, point 1). The security bound rests
   on the random epoch and connection id (the endpoint and channel must
   match but are not secret, and the call number must match but the bound
   credits none of its bits; wording corrected after review rounds 5
   and 7, the decision unchanged); a Firm
   rule does not depend on unverified peer behaviour in fields that carry
   no secret. Not taken: keeping them in the Firm match (a mismatch in
   normal operation would need a superseding ADR), and dropping them from
   the match entirely.

## Open questions

3. **Q3. Where the on-path residual is recorded (finding DS-5, review
   round 5).** O23 is
   Firm; its only network-attacker row is "A network attacker against
   rxkad", and its third column does not state this residual, which
   holds at every class. Options: (a) this ADR partly supersedes O23: it
   states the residual for a network attacker at every security class
   ("What it supersedes"), naming who is covered and which part is a
   cited fact (ABORT) and which is client policy (BUSY, CHALLENGE); O23's
   table is not rewritten, and ADR 0004 gains a status line and an
   annotation at O23 pointing here; (b) O23 is left as it is, with no
   reference, and this ADR's Consequences, the administrator's guide and
   the threat model document carry the residual, with this ADR's "Revisit
   if" naming the gap. Under either option this ADR puts the residual in
   the administrator's guide, as O23, rule 3, does for each residual in
   its table; the choice is whether O23 also carries a supersession
   reference. Recommend (a): a reader of the threat model
   finds the residual from O23, and a superseding ADR is the route README
   requires for a Firm change; this ADR is already class S on the
   strongest row, so it adds no review. Interim, pending the owner's
   decision (Q3): option (a), applied as written here.
4. **Q4. Whether the C30, C31 and C32 changes may go by amendment
   (findings DS-6 and A-8, review round 5; A-v3, review round 6).** README
   allows an amendment when "a working decision's trigger fires, or new
   evidence contradicts it"; no named trigger has fired, and the
   point-by-point evidence ("How this ADR relates to ADR 0004") is a gap
   for C30 and C31, a citation for C32, point 1, and a conflict with this
   ADR's own decision for C32, point 3, not contradicting field evidence.
   Options: (a) amend in place, available only if a fired trigger or
   contradicting evidence is identified first, which this draft does not
   have; (b) this ADR partly supersedes C30, C31 and C32: the points are
   its decision 2 (Working), additions beside C30 and C31 and one change
   in meaning to C32, point 3; ADR 0004 gains status lines and an
   annotation at each superseded decision, and the sweep is listed in
   this ADR's Review; (c) change the README to allow purely additive
   amendments (not recommended: it widens the route for every ADR and is
   a workflow change of its own). Recommend (b): it needs no reading of
   "contradicts" that the README text does not support, keeps the review
   the same (this ADR is class S on the strongest row), and pairs with
   3(a) as one supersession. It also moves the generation rule from C30,
   point 5, where decided question 1 placed it, to decision 2, point 1,
   still Working and outside decision 1. Interim, pending the owner's
   decision (Q4): option (b), applied as written here.

## Review

The full record, with every finding and its disposition, is
[docs/reviews/2026-11-21-adr-0006-review.md](../reviews/2026-11-21-adr-0006-review.md).

This ADR was written by Claude models, and its fix rounds by Claude Code
(`claude-opus-5-5`). Before drafting, two model families checked the
proposal; their changes are folded in (exact match without a serial
window, the call-0 match, the callback, CHALLENGE and ICMP points).

- **Seats.** Class S on the strongest row, from families that wrote
  neither this ADR nor the decisions of ADR 0004 it touches. Rounds 1 to
  4: GPT (`gpt-6.2-sol`, xhigh) for design and GLM (`glm-5.3`, max; high
  in round 4) for security. Rounds 5 to 10: GPT (`gpt-6.2-astra`, xhigh,
  in rounds 5 to 8; `gpt-6.2-sol`, xhigh, in rounds 9 and 10) for design,
  and DeepSeek (`deepseek-flash`, max; high in round 8) as the third
  family in rounds 5 to 8, whose last round passed with minor findings,
  fixed and checked by GPT in round 9.
- **Round 1** reviewed the whole draft. GPT FAIL (1 blocker, 8 major, 1
  minor); GLM FAIL (2 major, 6 minor). All fixed, except that two choices
  went to the owner as questions 1 and 2.
- **Rounds 2 and 3** verified those fixes. GPT FAIL in round 2 (4
  earlier findings open, 3 new major), fixed; GPT PASS in round 3. GLM
  found nothing new in either.
- **Round 4.** The owner decided questions 1 (option A) and 2 (option B)
  on 2026-11-20, and they were applied. GPT, checking the batch of
  drafts that carried owner decisions, found one major in test 2's
  expected results (service id and security index), fixed. GLM (high)
  found the decisions applied correctly.
- **Round 5**, a fresh strongest-row read of the whole draft: GPT FAIL (1
  blocker, 9 major, 2 minor); DeepSeek FAIL (1 blocker, 5 major,
  8 minor). All fixed, except two route choices, which became open
  questions 3 and 4 (below).
- **Rounds 6 to 9** each verified the previous round's fixes and the text
  that changed. Round 6: GPT FAIL (6 major, 8 minor), DeepSeek FAIL (1 new
  major, 5 new minor; earlier findings partly fixed), and one major from
  the author's own sweep. Round 7: GPT FAIL (9 major, 4 minor), DeepSeek
  FAIL (2 major, 6 moderate or minor). From round 7 the interim answers
  to questions 3 and 4 are applied and marked. Round 8: GPT FAIL (2
  major, 6 minor), DeepSeek PASS with 6 minor. Round 9: GPT FAIL (1
  major, 4 minor). Every finding was fixed in the next round, or
  refuted in part with the evidence recorded.
- **Round 10**: GPT (`gpt-6.2-sol`, xhigh) PASS, with 3 minor findings
  (two in the draft of the ADR 0004 status lines and annotations, one
  about private file names in this section), fixed and recorded; rounds
  stopped.
- **Publication edits** after round 10, changing no decision: this Review
  section is a summary of the rounds instead of per-round tables; the
  status line names the issue and why the number is 0006; the threat model document is named without
  an issue number, since it has none yet; the open questions are labelled
  Q3 and Q4 and cite their findings by round; decided question 2 cites
  rounds by their numbers here; and the sweep below was run again on
  this pull request's tree.

**Partial supersession of ADR 0004, YYYY-MM-DD (issue #52): C30, C31,
C32 and O23** (interim, pending the owner's decisions (Q3, Q4)).
YYYY-MM-DD is the date the owner accepts this ADR.

- What it supersedes: as "What it supersedes" states. ADR 0004's
  decision text is not edited. On acceptance, its overview status line
  gains "Partly superseded by 0006 (C30, C31, C32 and O23)"; part 1's and
  part 3's status lines name their decisions; C30, C31, C32 and O23 each
  gain a one-line annotation pointing here, and O34, point 1, a one-line
  pointer to decision 2, point 5. The ADR index in `docs/adr/README.md`
  lists 0006 as proposed while this ADR is a draft, and, when the owner
  accepts it, gives 0006 as "accepted YYYY-MM-DD" and 0004's status as
  "accepted 2026-11-20; amended 2026-11-20 (#32); partly superseded by
  0006 (YYYY-MM-DD)". Each supersession status line carries that date.
  None of these ADR 0004 changes is made while this ADR is proposed.
- Why superseded, not amended: no named trigger fired and the changes
  fill gaps or follow from decision 1, which the README's amendment route
  does not cover ("How this ADR relates to ADR 0004"; question 4). O23 is
  Firm, so only a superseding ADR can add to it (question 3).
- Sweep, run again on this pull request's tree: the repository searched
  for "C30", "C31", "C32", "O23", "wire identity", "connection id",
  "epoch", "keepalive call", "or an error", "callback continuity",
  "network attacker", "abort" and "ADR 0004".
  - C30, C31, C32 (part 1) and O23 (part 3): annotated on acceptance,
    as above.
  - Assumption A14 (overview): its "through a reset of its callback state
    or an error" now reads with decision 2, point 5; A14 is unchanged, and
    the uncapped branch it supports is not used while a connection fails
    decision 1, point 9's condition, nor, until revalidated by a call
    sent after that hold, for promises held during it or granted by a
    reply to a call sent before it ended.
  - C24, point 6 (part 1): decision 1, point 5, marks no lock lost on the
    packet alone, so nothing leaves its closed list. Agrees; unchanged.
  - C24, point 7 (part 1): "matched by that call's random identity"
    agrees with decision 2, point 4. Unchanged.
  - L33, points 7, 10 and 12 (part 2): decision 1, point 5, says an
    ABORT alone is not a failed renewal; continuity still decides loss.
    Agrees; unchanged.
  - C34, point 6, and the overview's NAT64 text (C32, point 1, on the
    client's own flow): agree. Unchanged.
  - N7, point 2, and N45, point 4 (a mutation already sent follows the
    lost-reply rule, C28), C16, point 6 (a fenced or unacknowledged
    mutation's outcome is read back and it is never repeated, C28, point
    4), and C28, points 3 and 4: agree with decision 1, point 5, which
    requires a read-back for every N34 kind. Unchanged.
  - C28, point 5, and C29, point 4 (part 1; a quarantined record is
    retried only on the durable owner's explicit instruction, after the
    difference is seen): decision 1, point 5, bars only transport resends
    and automatic journal replay, and leaves the record quarantined for
    that explicit recovery, which it cites; C16, point 6's "never
    repeated" is read as C28, point 4's "never replayed blindly".
    Unchanged, cited.
  - Part 1, Context, point 3, and C19 (no security class protects an
    abort): agree with "What it supersedes", O23. Unchanged.
  - Part 3, O34, point 1 (restates C32's two branches): annotated on
    acceptance with one line pointing to decision 2, point 5, for the hold and its exit,
    which its standalone text omits. Its meaning is unchanged.
  - Part 3, O34, point 6, and O45's read invariant (C32 branches and C32,
    points 1 and 2): agree. Unchanged.
  - Part 4, N5, N6 and the "can only lower trust" text (C31, C32): agree.
    Unchanged.
  - The overview's decision index (C30, C31 and C32 titles): titles
    unchanged.
  - `docs/reviews/2026-11-20-adr-0004-review.md`, ADR 0004's Review
    entry for the documentation sweep at M1's exit and its record
    `docs/reviews/2026-11-21-m1-close-sweep.md`: dated records, none
    stating C30, C31, C32 or O23. Unchanged.
  - `docs/reviews/2026-11-20-adr-0004-claims.md`, row ADR-002 (no Rx
    security class protects an ABORT): agrees with the fact table.
    A dated record. Unchanged.
  - Other "abort" and "epoch" hits (FUSE connection aborts in the
    research note on kernel code and in the review records, and time
    epochs in the code): a different meaning. Unchanged.
  - Actors, per changed rule. For each, the actor is whoever writes that
    part, the maintainer's session, contributors and their agents alike;
    bots and other automation have no role, since none generates or
    checks these rules, and no skill, workflow, contributor file or bot
    configuration states them.
    - Decision 1, points 1 to 4 and 6 to 9, and decision 2, points 1 to
      4: the authors of the Rx specification and the Rx crate.
    - Decision 1, point 5: the Rx crate (no resend, unknown outcome
      reported), the journal (read-back and quarantine) and the lock
      service (specification item 11).
    - Decision 2, point 5 (C32, point 3): the callback service and the
      cache's revalidation.
    - O23: the authors of the administrator's guide and of the threat
      model document, who state the residual.
