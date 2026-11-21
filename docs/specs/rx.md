<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Rx transport and the null security class

Status: draft (2026-11-21). Raised in issue #55. This is the first
version, written from documents only; its capture-backed revision follows
the observation lab's stage A record (section 13, Q1).

Kind: protocol. Class: S. Every reply, callback and abort the client
receives passes through this layer first, any host that can reach the
client's UDP port can send it packets, and no security class protects
its control packets ([README](README.md), "Review class per kind").

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (X1, D1 and so
on) are in section 12. The draft X1 has no section numbers; it is cited
by its headings, in quotation marks.

**Every wire fact here is UNVERIFIED.** The only document source for Rx
is the community draft X1, and README, "Sources and provenance", rules 12
and 14, make it a lead: nothing it states is CONFIRMED until an
independently generated black-box fixture shows it, and nothing is coded
from an UNVERIFIED point ([README](README.md), "Labelling claims"). So
the `ironafs-rx` crate waits on the capture-backed revision, which turns
each UNVERIFIED wire fact into OBSERVED or corrects it (section 13, Q1).

## 1. What this implements

ADR 0004, overview, constraint 4 (a constraint of the whole ADR):

> **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
> local IPC, configuration, requests from the operating system, and SMB
> messages before authentication. No panics, checked arithmetic and byte
> limits apply to all of it (coding standards rules 1 to 3).

ADR 0004, C3, point 1 (**Working**):

> Rx, XDR, the RPC codecs and the security classes are pure state
> machines: bytes, time and randomness in, actions out. They open no
> socket, read no clock and draw no random numbers.

ADR 0004, C4, point 1 (**Working**), the table rows, and point 3, the
driver row:

> | `ironafs-rx` | the Rx state machine, the security-class interface, the null class |
>
> | `ironafs-net` | the UDP driver and the `rx` thread |

ADR 0004, C4, point 4, the items that apply (**Working**):

> - no pure crate depends on a driver or on `ironafs-client`;
> […]
> - `ironafs-rx` knows nothing of AFS.

ADR 0004, C5, point 1 (**Working**), the part that applies:

> The cryptographic random source comes through the same interface. The
> production `Rng` (C3, point 3) is this source; the simulator seeds its
> own.

ADR 0004, C6, points 1 and 4 (**Firm**):

> 1. Each protocol component is written from a behaviour specification in
>    `docs/specs/` that names its sources, under ADR 0001. No
>    specification, no code.
>
> 4. A fact about the AFS protocols that this ADR could not tie to a public
>    source is an input to a specification, never something to code from.

ADR 0004, C19, point 1 (**Firm**):

> Bytes, status, lookups, routing and grants obtained without
> authentication (Rx's null class) are never used to answer a request
> made under a credential, and never enter the cache of a context that
> has one.

ADR 0004, C24, point 7 (**Firm**), the sentence that binds Rx:

> Before rxgk, a server earns callback trust (C32) only from its reply to
> a call the client made, matched by that call's random identity.

ADR 0004, C30, points 1 and 2 (**Working**):

> 1. The client uses one UDP socket per address family, on port 7001 by
>    convention `[DES-011]`, for outgoing calls and for the callback service.
> 2. One `rx` thread owns the sockets. It never takes a cache lock.

ADR 0004, C31 (**Working**), the table row and point that bind Rx:

> | More than the rate limit can process | the suspect epoch of the affected servers (of every server, if the source is unknown) advances **before** work is dropped or acknowledged; drops are counted |
>
> 2. **A break from a known server is never discarded.** Over that server's
>    rate limit, it marks the server's callbacks suspect instead.

ADR 0004, C32, point 1 (**Working**), the sentences that bind Rx:

> Callback trust is kept per server, address family and local endpoint
> generation. […] The reply is matched by the keepalive call's Rx
> identity (epoch, connection id and call number), which the client
> draws from `Rng` (C3) so that an off-path sender cannot guess it.

ADR 0004, C32, point 5 (**Working**):

> **Resume is checked when data is served,** not only on a notification
> from the operating system. Each serve compares the clocks of C37, and a
> jump marks every callback suspect before the answer is given.

ADR 0004, C39, point 3 (**Working**), the sentence that applies:

> Every queue is bounded in bytes.

ADR 0004, N5, point 2 (**Firm**), the part that applies:

> Everything else is a **hint**: DNS answers, rxnull replies, rxkad
> replies, Rx aborts, timeouts, […]

ADR 0004, N6, point 3 (**Firm**), the first sentence:

> Key exhaustion on a connection is its own outcome (N45).

ADR 0004, N7, point 5 (**Firm**), the invariant:

> **no packet leaves on a connection the predicate does not permit.**

ADR 0004, N31, points 1 and 2 (**Firm**):

> 1. A wire fact is taken only from an allowlisted specification passage or
>    an independently generated black-box fixture, recorded with its
>    edition, section or fixture provenance.
> 2. Never from implementation history, interface-definition files,
>    generated definitions, or live-cell captures holding credentials or
>    private data.

ADR 0004, N45 (**Working**), points 1 to 4:

> 1. Rekeying belongs to the Rx security-class interface, not to the crypto
>    provider. A class may ask for a new key or a reconnect on a live
>    connection.
> 2. **Key exhaustion is its own outcome**, distinct from a transport
>    failure. […]
> 3. A request not yet sent continues on the new key within its own
>    deadline.
> 4. A mutation already sent whose outcome is unknown follows the lost-reply
>    rule (C28). It is never resent transparently.

[ADR 0006](../adr/0006-rx-unprotected-packets.md), decision 1 (**Firm**),
the sentence before the points and the first sentence of each point,
which this specification keeps in full (section 5.9 carries every point
out; the ADR is the text that governs):

> The client treats every ABORT, BUSY and CHALLENGE it receives as
> unprotected, at every security class.
>
> 1. **A call-level ABORT or BUSY on an outgoing call** is considered only
>    if it matches that live call: the receiving local endpoint, the peer's
>    address and port as seen on that socket, the direction, the epoch,
>    the connection id, the channel and the channel's current call number.
> 2. **An ABORT with call number 0** is considered only if it matches a
>    connection's full connection-level tuple: […] A matching one is a
>    hint (N5, point 2) and is counted.
> 3. **A CHALLENGE** is answered only if all of these hold, and otherwise
>    no RESPONSE is sent: […]
> 4. **Anything that does not match** is dropped without a reply, in
>    bounded work and with no allocation per packet.
> 5. **A matching call-level ABORT ends that one call and nothing else.**
> 6. **It may lower trust, and only so,** and only for an ABORT on a
>    connection that meets point 9's condition; […]
> 7. **A matching BUSY** is accepted only while the outgoing call is waiting
>    to be accepted: no DATA or ACK packet of that call has been accepted
>    from the peer […]. It never fails the call.
> 8. **On an incoming callback call** (the client is the acceptor and the
>    remote side chose the identity), the lookup is exact: […]
> 9. **Precondition for points 5 and 6.** Point 5 ends a call only on a
>    connection whose wire identity is unpredictable: the client
>    initiated it, and its epoch and connection id were drawn whole from
>    the cryptographic random source (C5, point 1), never derived from a
>    clock, a counter or another identity, and checked against the live
>    and reserved identities the client holds […].

ADR 0006, decision 2 (**Working**), points 1 to 5, the first sentence of
each:

> 1. **Wire identity (beside C30).** Every connection the client initiates
>    draws its epoch and connection id (its wire identity, distinct from
>    C30, point 3's connection key) fresh from the production `Rng` of C5,
>    point 1 (the cryptographic random source).
> 2. **ICMP and socket errors (beside C30).** An ICMP error received on the
>    client's sockets, and an asynchronous error the operating system
>    reports on a UDP socket for one (on a send or a receive), is a hint
>    (N5, point 2).
> 3. **ABORT and BUSY on an incoming call (beside C31).** Decision 1, point
>    8, applied to C31's table: […]
> 4. **The keepalive's identity (with C32, point 1).** The keepalive
>    connection's epoch and connection id are drawn as point 1 says; […]
> 5. **ABORT as "an error" (changes C32, point 3).** An unprotected ABORT
>    is "an error" in C32, point 3, only as decision 1, point 6, allows: […]

ADR 0006, "Specification items for the Rx specification", items 1 to 10
(item 11 belongs to the lock service's specification). Section 9 names
the section that carries out each item.

ADR 0006 is accepted (2026-11-21) and lands with its pull request (#102).
Section 9 shows where each invariant is kept and which test fails if it is
broken.

## 2. Scope

1. **Covered:**
   - the packet header and the packet types (5.1), their caps (5.2) and
     their decoding (5.4) and encoding (5.5);
   - connections: the wire identity, the connection lookup, channels,
     call numbers and serials (5.6);
   - the call lifecycle on both sides, acknowledgements and the advertised
     window, retransmission, flow control, keepalive and timeout (5.7);
   - the security-class interface, including the rekey request and the
     "key exhausted" outcome, and the null class (5.8);
   - unprotected packets: ABORT, BUSY, CHALLENGE, and ICMP and socket
     errors, as ADR 0006 decides (5.9);
   - the server role that the callback service needs: admission, caps in
     bytes per peer and in total, the overload event and its
     confirmation, the pause and its generation, and what is sent to a
     source with no established connection (5.10);
   - the counters and events Rx hands its user (5.11);
   - the interface between the pure machine (`ironafs-rx`) and the
     driver (`ironafs-net`), as far as the ordering above needs it (5.12).
2. **Not covered, and where it is covered:**
   - opcodes, argument and result encoding, and the check that a reply
     holds exactly the bytes expected: the RPC specification (#65) and
     [xdr.md](xdr.md). Rx delivers a bounded byte stream per direction;
     it knows nothing of AFS (C4, point 4);
   - which abort codes mean what to an AFS caller: the RPC and service
     specifications (C46, point 3). Rx passes a received code through
     unchanged;
   - suspect epochs, callback trust, the serve-path comparison of the
     pause generation, and what the callback service does with a break:
     the callback service's specification (C31, C32; #66). Rx supplies the
     events, the identity it matched and the generation counter;
   - the connection key, N7's predicate, and retry and outcome rules for
     mutations: `ironafs-client`'s specifications (C28, C30, point 3, N7).
     Rx reports what was sent; it never retries a call by itself;
   - rxkad and rxgk: their own specifications, behind the interface of
     5.8;
   - jumbograms, the extended acknowledgement format on send, and the
     debugging and version interfaces: IronAFS does not send them (5.5,
     points 6 to 8). Receiving them is covered (5.4);
   - socket options, per-operating-system error reporting, and the
     address family settings of each socket: the driver's specification
     under ADR 0005 (section 13, Q6);
   - how a server lock renewal with an unknown outcome is settled: the
     lock service's specification (ADR 0006, specification item 11).

## 3. The design in short

1. Rx carries calls over UDP. A connection is identified by a direction,
   an epoch, a connection id, a service, a security class and the peer's
   endpoint; each connection has four channels, and each channel carries
   one call at a time `[X1 "Rx Connections": UNVERIFIED]`.
2. Every packet starts with a 28-byte header in network byte order
   `[X1 "Packet Formats and Protocol Constants": UNVERIFIED]`. IronAFS
   decodes it strictly, with a hard cap on the datagram size, and drops
   anything it cannot place, without a reply.
3. The client draws every connection's epoch and connection id it
   initiates from the cryptographic random source, so an off-path sender
   must guess up to 61 bits to touch a call `[ADR 0006 D2.1: Working]`.
4. Every packet that can change state must match a live connection, and
   for call-level packets a live call, on every identity field. A
   matching ABORT ends one call, and only on a connection whose identity
   is unpredictable; a call-0 ABORT is a hint; a BUSY only defers; a
   CHALLENGE draws at most a bounded RESPONSE `[ADR 0006 D1: Firm]`.
5. As acceptor (the callback service), Rx charges every byte of state to
   the sending peer and to a total, sends no more than twice what an
   unverified source sent, and at a cap hands the overload to its user
   before anything is dropped; when its fixed allowance for held excess
   is full, the `rx` thread stops reading, after raising a pause
   generation that the serve path compares (5.10).
6. The machine is pure: datagrams, time and randomness in; datagrams to
   send, deliveries, outcomes and events out (C3, point 1). The driver
   owns the sockets and the pause generation, and checks each outgoing
   datagram's permit generation immediately before it sends (5.12).
7. Rx never retries a call itself. Each failed call reports whether any
   of its request was sent, so its user can apply the lost-reply rule
   (C28, ADR 0006 D1.5).

## 4. Terms

- **Initiator, acceptor:** the side that created the connection, and the
  other side `[X1 "Rx Connections": UNVERIFIED]`. The client is the
  initiator of its outgoing calls and the acceptor of incoming callback
  calls.
- **Outgoing connection, incoming connection:** a connection the client
  initiated, and one a peer initiated to a service the client offers.
- **Wire identity:** a connection's epoch and connection id as sent in
  the header ([ADR 0006](../adr/0006-rx-unprotected-packets.md),
  "Terms"). **Connection id** always means the 30-bit field; the 2-bit
  **channel** is named separately.
- **Receiving local endpoint:** the socket a datagram arrived on, named by
  its address family and its **local endpoint generation**, a counter the
  driver raises whenever it binds that family's socket again (C32,
  point 1's key).
- **Peer endpoint:** the source address and port of a received datagram,
  after normalisation (5.4, point 3).
- **Connection tuple:** the receiving local endpoint, the peer endpoint,
  the direction, the epoch word and the connection id. The **common
  connection lookup** (5.6, point 6) finds a connection by it.
- **Call identity:** the connection tuple, the channel and the call
  number.
- **Live call:** a call whose state Rx holds and that has neither
  completed nor failed.
- **Eligible connection:** an outgoing connection whose wire identity was
  drawn and checked as 5.6, point 1, requires: ADR 0006 D1.9's condition.
  Every outgoing connection Rx opens is eligible; an incoming connection
  never is.
- **Verified peer:** for an incoming connection, a peer that has shown it
  receives at its claimed address, by answering the reachability test of
  5.10, point 7.
- **Charge:** the bytes counted against a cap for one piece of state
  (5.10, point 2).
- **Overload event:** the record of a datagram that could not be admitted
  within a cap, handed to the user, with an id the user confirms (5.10,
  point 4).
- **Pause generation:** a per-socket counter the driver raises before it
  stops reading the socket and again before it reads after a pause (5.10,
  point 6).
- **Permit generation:** a per-connection counter the user raises when it
  fences or closes a connection, and whenever the credential generation
  or a fence that N7's predicate applies to the connection changes (5.12,
  point 4).
- **Hint:** an input that may lower trust and is counted, never evidence
  (N5, point 2).

## 5. Specification

### 5.1 Layout

All multi-byte integers are unsigned and in network byte order (most
significant byte first) unless a point says otherwise
`[X1 "Packet Formats and Protocol Constants": UNVERIFIED]`. Every offset
is from the start of the UDP payload.

1. **The header** is 28 bytes, then the payload
   `[X1 "Packet Formats and Protocol Constants": UNVERIFIED]`:

   | Offset | Width | Field | Meaning |
   |---|---|---|---|
   | 0 | 4 | epoch word | bit 31 (the most significant) is the ignore-source flag; bits 0 to 30 are the 31-bit epoch `[X1 "Rx Connections": UNVERIFIED]` |
   | 4 | 4 | connection word | bits 2 to 31 are the 30-bit connection id; bits 0 and 1 are the channel `[X1 "Rx Connections": UNVERIFIED]` |
   | 8 | 4 | call number | 0 for a connection-level packet; otherwise the call on the channel `[X1 "Rx Connections", "Packet Types": UNVERIFIED]` |
   | 12 | 4 | sequence | the DATA packet's position in its direction of a call, from 1; 0 on other packets `[X1 "Rx Connections": UNVERIFIED]` |
   | 16 | 4 | serial | the sender's per-connection packet counter `[X1 "Rx Connections": UNVERIFIED]` |
   | 20 | 1 | type | point 2 |
   | 21 | 1 | flags | point 3 |
   | 22 | 1 | user status | service-specific, 0 unless a service defines it `[X1 "Rx Connections": UNVERIFIED]` |
   | 23 | 1 | security index | the connection's security class; 0 is the null class `[X1 "Rx Connections": UNVERIFIED]` |
   | 24 | 2 | security-specific | the class's own field, 0 unless the class defines it `[X1 "Rx Connections": UNVERIFIED]` |
   | 26 | 2 | service id | the service on the peer's port `[X1 "Rx Connections": UNVERIFIED]` |

2. **Packet types** `[X1 "Packet Formats and Protocol Constants",
   "Packet Types": UNVERIFIED]`, with the context each is valid in
   (call number 0 is the connection context) `[X1 "Packet Types":
   UNVERIFIED]` and IronAFS's handling:

   | Value | Type | Context | Payload | IronAFS receives | IronAFS sends |
   |---|---|---|---|---|---|
   | 1 | DATA | call | the call's bytes | 5.7 | yes |
   | 2 | ACK | call | point 5 | 5.7 | yes |
   | 3 | BUSY | call | none | 5.9, points 7 and 8 | as acceptor only (5.7, point 14) |
   | 4 | ABORT | call or connection | a 4-byte code | 5.9 | 5.7, point 15 |
   | 5 | ACKALL | call | none | 5.7, point 10 | never |
   | 6 | CHALLENGE | connection | class-specific | 5.9, point 3 | never (5.8, point 9) |
   | 7 | RESPONSE | connection | class-specific | dropped (5.9, point 4) | 5.9, point 3 |
   | 8 | DEBUG | either | not used | dropped | never |
   | 9 to 12 | parameter and unused types | connection | not used | dropped | never |
   | 13 | VERSION | either | not used | dropped | never |
   | any other | none | none | none | dropped | never |

   `[OWN]` for the last two columns, with the reasons in 5.4 and 5.5.

3. **Flags** `[X1 "Packet Formats and Protocol Constants", "Rx
   Connections": UNVERIFIED]`. Two bits have a meaning that depends on
   the type:

   | Bit | Value | On DATA | On ACK | On other types |
   |---|---|---|---|---|
   | 0 | 0x01 | client-initiated | client-initiated | client-initiated |
   | 1 | 0x02 | request-ack | request-ack | none |
   | 2 | 0x04 | last-packet | none | none |
   | 3 | 0x08 | more-packets (advisory) | extended-SACK | none |
   | 4 | 0x10 | reserved | reserved | reserved |
   | 5 | 0x20 | jumbo-packet | slow-start-ok (advisory) | none |
   | 6, 7 | 0x40, 0x80 | none | none | none |

   Client-initiated is set on every packet the initiator sends and clear
   on every packet the acceptor sends, whatever the type `[X1 "Rx
   Connections": UNVERIFIED]`. It is the packet's **direction**.

4. **ABORT payload:** one 4-byte code, read as a signed 32-bit integer
   `[X1 "Packet Types": UNVERIFIED]`. Rx does not interpret it (C46,
   point 3) `[OWN]`.

5. **ACK payload** `[X1 "Packet Formats and Protocol Constants", "Call
   Layer": UNVERIFIED]`, at payload offsets:

   | Offset | Width | Field |
   |---|---|---|
   | 0 | 2 | buffer space (unused, 0) |
   | 2 | 2 | maximum skew (unused, 0) |
   | 4 | 4 | first packet |
   | 8 | 4 | previous packet |
   | 12 | 4 | serial (of the packet that prompted this ACK, or 0) |
   | 16 | 1 | reason |
   | 17 | 1 | ack count, N |
   | 18 | N | selective acknowledgement table (SACK), one octet per packet |
   | 18 + N | 3 | reserved octets; with the extended-SACK flag, the first may be a 256th SACK octet, the second is the trailer count and the third the extra-SACK count |
   | 21 + N | 4 each | trailers, in order: maximum packet size, recommended packet size, receive window, packets per jumbogram |

   The trailers are not aligned to four bytes. Any of them may be absent:
   a short payload ends early `[X1 "Call Layer": UNVERIFIED]`. With the
   extended-SACK flag, extra SACK tables may follow the trailers
   `[X1 "Packet Formats and Protocol Constants": UNVERIFIED]`; IronAFS
   never reads them (5.4, point 12).

   Reason codes `[X1 "Packet Formats and Protocol Constants":
   UNVERIFIED]`: 1 requested, 2 duplicate, 3 out of sequence, 4 exceeds
   window, 5 no space, 6 ping, 7 ping response, 8 delay, 9 idle.

   The meaning of SACK octet i (0 ≤ i < N, and i < 256): its least
   significant bit is 1 if the packet with sequence first packet + i was
   received, 0 if not `[X1 "Call Layer": UNVERIFIED]`. With the
   extended-SACK flag, the other bits of the octet describe packets 256
   and more beyond `[X1 "Packet Formats and Protocol Constants":
   UNVERIFIED]`; IronAFS never needs them (5.7, point 6).

6. **Defaults for absent trailers** `[X1 "Call Layer": UNVERIFIED]`:
   maximum and recommended packet size 1444 bytes (claim record row
   `[ADR-006]` reads the same); receive window 16 packets (claim record
   row `[DES-056]` reads the same); packets per jumbogram 1.

7. **The null class's packets** have no class header or trailer: the
   payload is the call's bytes as they are (5.8, point 10) `[OWN]`, since
   X1 does not specify security classes `[X1 "Introduction":
   UNVERIFIED]` (claim record row `[DRV-084]`).

### 5.2 Caps

Every limit is in bytes or derived from a byte limit, and is checked
before the state it guards is created or grown (coding standards rule 3;
C39, point 3). Each is IronAFS's own `[OWN]`, with its reason; a value
here changes by a pull request of class S.

| Name | Value | Applies to | Why |
|---|---|---|---|
| `rx.packet_max` | 1444 bytes | a received datagram, header included; larger is dropped | the size IronAFS advertises in every ACK (5.5, point 4) and the size a peer assumes before any ACK (5.1, point 6) |
| `rx.payload_max` | 1416 bytes | a DATA payload sent or accepted, before a class's overhead | `rx.packet_max` less the header |
| `rx.recv_window` | 32 packets | packets of one call held for the user, per direction; so at most 32 × 1416 = 45 312 bytes of payload per call | twice X1's historical default of 16 for read throughput, while a call's buffer stays bounded; below 255 so the plain SACK covers it (5.7, point 6) |
| `rx.send_window_max` | 255 packets | packets of one call in flight | the largest span a plain SACK table describes (5.1, point 5) |
| `rx.call.reply_max`, `rx.call.request_max` | set per call by the user, at most `rx.call.bytes_ceiling` | bytes one call receives, and sends | the user knows the message it expects (C39, point 3) |
| `rx.call.bytes_ceiling` | 2³¹ − 1 bytes | the largest per-call value the user may set | keeps every per-call byte count in signed 32-bit range, whatever the peer |
| `rx.in.request_ceiling` | 65 536 bytes | the largest request a registered service may accept on an incoming call | callback requests are small; the service sets its own value at most this |
| `rx.in.reply_ceiling` | 65 536 bytes | the largest reply a registered service may send on an incoming call | callback replies are small; the service sets its own value at most this, so one incoming call never holds more than a bounded reply (5.10, point 2) |
| `rx.in.reply_deadline` | 60 s | the time an incoming call is kept after its request is complete, until every reply packet is acknowledged | a genuine initiator acknowledges within a few retransmission timeouts (`rx.rto_max` is 10 s); a peer that withholds its ACKs cannot hold reply state longer (5.7, point 14) |
| `rx.in.conn_charge` | 512 bytes | the charge for one incoming connection's record | an upper bound on the record's size; a test asserts the record fits (10.5, A22) |
| `rx.in.call_charge` | 512 bytes | the charge for one incoming call's record, besides its buffered bytes | as above |
| `rx.in.peer_bytes` | 262 144 bytes | all charges for one peer address | at least one call at both ceilings with its records (65 536 + 65 536 + 512 + 512 = 132 096 bytes), with room for several concurrent small breaks from one server; far below the total |
| `rx.in.total_bytes` | 8 388 608 bytes | all charges of the acceptor role, the held allowance included; every charge other than a held datagram's must fit `rx.in.total_bytes` less `rx.in.held_bytes` (5.10, point 2) | bounds the whole callback service's memory in Rx, and keeps the held allowance free inside it |
| `rx.in.held_bytes` | 524 288 bytes | datagrams held until their overload event is confirmed, counted inside `rx.in.total_bytes` | 5.10, point 5 |
| `rx.in.overloads_max` | 64 events | unconfirmed overload events at once | a fixed table; beyond it, one "unknown source" event (5.10, point 4) |
| `rx.in.conns_per_peer_max` | 16 connections | incoming connections from one peer address | stops one source fragmenting its charge into many small records |
| `rx.in.request_deadline` | 30 s | the time an incomplete incoming request is kept (ADR 0006, item 9) | 5.10, point 9 |
| `rx.in.conn_idle` | 60 s | an incoming connection with no live call is retired after this | frees acceptor state |
| `rx.unverified_factor` | 2 | bytes sent to an unverified peer, as a multiple of bytes received from it | 5.10, point 7 |
| `rx.drop_table_entries` | 256 sources | the per-source drop counter table (ADR 0006, item 7) | a fixed table; an aggregate counter beyond it |
| `rx.unmatched_rate` | 100 packets per second per source | unmatched packets from one source counted as ordinary drops; beyond it they are counted as `over_rate` (ADR 0006, item 7) | separates a flood from stray traffic in diagnostics without extra work per packet |
| `rx.drop_report_interval` | 60 s | how often the drop counts are handed to the user | one event per interval, never one per packet |
| `rx.icmp_table_entries` | 64 peers | the per-peer ICMP and socket-error counter table (5.9, point 9) | a fixed table; an aggregate counter beyond it |
| `rx.event_queue_bytes` | 262 144 bytes | events waiting for the user, beyond the reserved slots (5.11, "The event queue") | a fixed bound however fast a peer can make events; an event that does not fit is counted, never queued |
| `rx.icmp_hint_interval` | 1 s | the least time between two `IcmpHint` events for one peer, or for the aggregate | a hint stays prompt, while an error stream costs at most one event per peer per second |
| `rx.read_batch` | 64 datagrams | datagrams the driver reads before it runs timers and sends | unmatched traffic cannot starve live calls of the thread (5.9, point 4) |
| `rx.identity_bound` | 4096 pairs | live outgoing connections plus reserved retired identities (ADR 0006, item 8) | 5.6, point 1 |
| `rx.identity_redraws` | 8 draws | draws per new outgoing connection before it fails | a collision of a 61-bit draw is negligible; eight absorbs a broken `Rng` quickly and fails closed |
| `rx.call.deadline_max` | 1800 s | the longest call deadline the user may set | bounds R (5.6, point 2) |
| `rx.peer_linger` | 120 s | the longest a peer is assumed to keep sending after a call ends on IronAFS's side | UNVERIFIED for real peers (section 13, Q3); sets R |
| `rx.identity_margin` | 60 s | margin for network delay in R | ADR 0006, item 8 |
| `rx.idle_timeout` | 60 s by default; the user may set 10 s to 600 s per call | T: a call whose peer is not heard from for T fails (5.7, point 12) | X1 leaves T to the upper layer |
| `rx.rto_initial`, `rx.rto_max` | 1 s, 10 s | the retransmission timeout before a sample, and its ceiling | 5.7, point 11 |
| `rx.ack_delay` | 100 ms | the delay before a DELAY acknowledgement | 5.7, point 9 |
| `rx.busy_backoff` | 250 ms, doubling, at most 4 s | the wait after a BUSY | ADR 0006, item 3 |
| `rx.busy_defer_max` | the smaller of 10 s and half the time left to the call's deadline when the call started | cumulative deferral per call | ADR 0006, item 3: the call keeps at least half its time for normal retransmissions |
| `rx.response_interval` | 500 ms | the least time between two RESPONSEs on one connection | ADR 0006, item 6 |
| `rx.responses_per_handshake` | 4 | RESPONSEs before an exchange completes on the connection | ADR 0006, item 6 |
| `rx.ping_reply_interval` | 250 ms | the least time between two PING-RESPONSEs on one connection | a forged PING stream costs bounded replies |
| `rx.verify_pings_max` | 3 | reachability PINGs per unverified incoming connection | 5.10, point 7 |

Derived values:

- **R**, the identity reservation window, is `rx.call.deadline_max` +
  `rx.peer_linger` + `rx.identity_margin` = 1980 s (ADR 0006, item 8).
- The receive buffer of one call is at most `rx.recv_window` ×
  `rx.payload_max` bytes; the user's per-call caps bound what it is
  handed in total.
- The incoming connections Rx can hold are at most (`rx.in.total_bytes`
  − `rx.in.held_bytes`) ÷ `rx.in.conn_charge` = 15 360.
- The event queue holds at most `rx.event_queue_bytes` plus its reserved
  slots, each one fixed-size event that merging never grows: one per
  pending overload event (at most `rx.in.overloads_max` + 1), one per
  address family for its pause state, one for the overflow summary, and
  for each outgoing connection live or reserved, one for an `AbortHint`
  and one for an `AbortReceived` (at most 2 × `rx.identity_bound`)
  (5.11, "The event queue").

### 5.3 Versions

Not applicable: Rx has no version field that IronAFS reads or writes,
and this specification describes a peer's format, which IronAFS cannot
freeze ([README](README.md), "Formats IronAFS does not control"). What
IronAFS sends and accepts is recorded per field in 5.4 and 5.5.

### 5.4 Decoding

The checks run in this order on every received datagram. Each failure
drops the datagram, counts it under the reason named (5.11), sends
nothing, and changes no other state `[OWN]`, so that a malformed or
unplaceable packet costs only bounded counting (ADR 0006 D1.4: Firm).

1. **Size.** A datagram longer than `rx.packet_max` is dropped
   (`too_long`). The driver reads into a buffer of `rx.packet_max` + 1
   bytes, so an over-long datagram is seen as too long, never as a
   silently truncated one (5.12, point 2). A datagram shorter than 28
   bytes is dropped (`too_short`).
2. **Header fields** are read at the offsets of 5.1, point 1. No field
   is rejected for its value at this step.
3. **Peer normalisation** (ADR 0006, item 1). The peer endpoint is the
   source address and port the driver reports, with these rules `[OWN]`:
   - an IPv4 source on the IPv4 socket is used as it is;
   - an IPv6 source on the IPv6 socket is used as it is, including an
     address in a NAT64 prefix: the client reaches a fileserver through
     NAT64 at that synthesised address (C34, point 5), so replies come
     from it;
   - an IPv4-mapped IPv6 source (`::ffff:0:0/96`: 80 zero bits, 16 one
     bits, then the IPv4 address `[X2 §2.5.5.2: CONFIRMED]`) is dropped
     (`mapped_source`): the IPv6 socket is set to carry IPv6 only (5.12,
     point 1), so such a source can only be forged or misrouted;
   - source port 0 is dropped (`bad_source`).
   Matching compares normalised endpoints and the receiving local
   endpoint exactly; there is no other conversion.
4. **Type.** A type outside 1 to 13 is dropped (`unknown_type`). Types
   8 to 13 are dropped (`unused_type`) `[OWN]`. X1 says an unrecognised
   type draws a protocol-error abort, and that a peer may answer VERSION
   `[X1 "Packet Formats and Protocol Constants", "Packet Types":
   UNVERIFIED]`. IronAFS departs from both: it replies to no packet it
   cannot place, because a reply to an unverified source is a reflection
   (section 8), and remote debugging gets no data (ADR 0004, C31,
   point 4).
5. **Context.** DATA, ACK, BUSY and ACKALL with call number 0, and
   CHALLENGE and RESPONSE with a call number other than 0, are dropped
   (`bad_context`) `[X1 "Packet Types": UNVERIFIED]`. ABORT is valid in
   both.
6. **Jumbo.** A DATA packet with the jumbo-packet flag set is dropped
   (`jumbo`) `[OWN]`: IronAFS advertises one packet per jumbogram in
   every ACK (5.5, point 4), so a conforming peer sends none
   `[X1 "Jumbograms": UNVERIFIED]`, and a decoder for them would be code
   with no use.
7. **Lookup.** The common connection lookup (5.6, point 6) runs with the
   receiving local endpoint, the peer endpoint, the direction (the
   received packet's client-initiated flag), the epoch word and the
   connection id.
   - **Found:** the packet goes to its type's handler: 5.7 for DATA, ACK
     and ACKALL, except that a DATA packet with sequence 1 that starts a
     new call on an incoming connection goes to 5.10, point 3; 5.9 for
     ABORT, BUSY, CHALLENGE and RESPONSE.
   - **Not found, and the packet is DATA with sequence 1 and the
     client-initiated flag set:** it may open an incoming connection or
     call (5.10). Any other check of 5.10 that fails drops it.
   - **Not found otherwise:** dropped (`unmatched`), counted per source
     (5.9, point 4).
8. **Service id and security index** on a found connection are compared
   with the connection's own. A difference is counted
   (`service_mismatch`, `security_index_mismatch`) and does not by itself
   drop the packet (ADR 0006 D1.1: Firm; its decided question 2); the
   connection's own values are used. X1 says peers cache a connection's
   service and ignore later values `[X1 "Rx Connections": UNVERIFIED]`.
9. **Sequence on a non-DATA packet** is not checked `[OWN]`: X1 says it
   should be 0 `[X1 "Rx Connections": UNVERIFIED]`, and nothing IronAFS
   does depends on it, so dropping for it could only lose genuine
   packets.
10. **Serial** is never an acceptance condition (ADR 0006, item 2). It is
    read only to echo in an ACK and to match an ACK's serial field to a
    packet IronAFS sent (5.7, points 8 and 11; 5.10, point 7).
11. **Flags** not defined for the type, and the reserved bit 0x10, are
    ignored `[OWN]`: X1 defines no rejection for them, and their values
    change nothing IronAFS does. User status is ignored on receipt
    `[OWN]`: no IronAFS service defines it (section 13, Q8).
12. **ACK payload** (5.1, point 5), decoded only after the lookup has
    found the connection and the live call (5.7, point 6):
    - shorter than 18 bytes: dropped (`ack_short`);
    - shorter than 18 + N, with N the ack count: dropped (`ack_short`);
    - the trailers are read from offset 21 + N, in order, each only if
      all four of its bytes are present; a field cut short is absent, and
      bytes after the last field read are ignored, since X1 has a
      receiver ignore trailer fields it does not know `[X1 "Call Layer":
      UNVERIFIED]`;
    - with the extended-SACK flag: the trailer count octet is read but
      not trusted, IronAFS reads at most the four fields above, and
      extra SACK tables are never read `[OWN]`, because no packet IronAFS
      has in flight can be described by them (5.7, point 6). X1 says a
      receiver must ignore extra SACK tables that are missing or cut
      short `[X1 "Packet Formats and Protocol Constants": UNVERIFIED]`;
    - the reserved octets, buffer space and maximum skew are not checked
      `[OWN]`, for the reason of point 9.
13. **ABORT payload** of any length other than 4 is dropped
    (`abort_length`) `[OWN]`: a strict rule drops at worst a genuine
    abort, which then costs a timeout (ADR 0006, "Consequences");
    capture evidence may relax it (section 11).
14. **BUSY and ACKALL payloads** must be empty; otherwise dropped
    (`busy_length`, `ackall_length`) `[OWN]`, since X1 gives neither
    type a payload `[X1 "Packet Types": UNVERIFIED]`, for the reason of
    point 13.
15. **DATA payload** is handed to the connection's security class (5.8,
    point 5). A payload longer than `rx.payload_max` after the class is
    dropped (`data_too_long`) `[OWN]`; under the null class it cannot
    happen, since point 1 bounds the payload at `rx.packet_max` less the
    header, which is `rx.payload_max`. A DATA packet with an empty
    payload is valid only with the last-packet flag; otherwise it is
    dropped (`empty_data`) `[OWN]`: an empty packet in the middle of a
    call carries nothing and would let a peer spend a window slot for no
    byte.
16. **No allocation per dropped packet.** Every check of points 1 to
    14, and point 15 under the null class, runs on the received buffer
    without allocating; the buffer is reused (ADR 0006 D1.4: Firm).
    Another class's open (5.8, point 5) runs only on a DATA packet that
    matched a live call or opens one; whether it allocates, and how much,
    is bounded by its own specification.

### 5.5 Encoding

1. **Header.** Every packet IronAFS sends has the layout of 5.1, point 1,
   with: the ignore-source flag clear `[ADR 0006 D1.1: Firm]`; sequence 0
   on every non-DATA packet `[X1 "Rx Connections": UNVERIFIED]`; user
   status 0 `[X1 "Rx Connections": UNVERIFIED]`; the security-specific
   field as the class sets it, 0 for the null class `[X1 "Rx
   Connections": UNVERIFIED]`; the reserved flag bit 0x10 and bits 0x40
   and 0x80 clear `[OWN]`.
2. **Direction.** Client-initiated set on every packet of an outgoing
   connection, clear on every packet of an incoming one `[X1 "Rx
   Connections": UNVERIFIED]`.
3. **Serial.** 5.6, point 4.
4. **ACK.** IronAFS sends the plain format only (no extended-SACK flag)
   `[OWN]`: its receive window is at most 255 packets, so the plain table
   describes every packet it can hold. Buffer space and maximum skew are
   0 `[X1 "Call Layer": UNVERIFIED]`. Previous packet is the highest
   sequence accepted and held `[X1 "Call Layer": UNVERIFIED]`. N is the
   number of packets from first packet to the highest sequence held,
   at most `rx.recv_window`. The three reserved octets are 0. All four
   trailers are sent: maximum and recommended packet size 1444
   (`rx.packet_max`), receive window `rx.recv_window`, packets per
   jumbogram 1. The slow-start-ok flag is set, since IronAFS's sender
   follows 5.7, point 11 `[OWN]`. The request-ack flag is set only on a
   PING `[X1 "Call Layer": UNVERIFIED]`. Size: 28 + 18 + N + 3 + 16
   bytes, at most 97. **The one exception** is the reachability PING of
   5.10, point 7: N = 0, and neither the reserved octets nor the trailers
   are sent, so it is 28 + 18 = 46 bytes `[OWN]`, which a short payload
   allows (5.1, point 5) and which fits the unverified budget of any
   received datagram.
5. **ABORT.** 4 bytes of payload, the code as a signed 32-bit integer.
6. **No jumbograms**, and never the jumbo-packet or more-packets flag
   `[OWN]`: retransmitted packets may not carry more-packets
   `[X1 "Rx Connections": UNVERIFIED]`, and an advisory flag IronAFS
   does not need is one less thing a peer can misread.
7. **Never sent:** ACKALL (X1 discourages it `[X1 "Packet Types":
   UNVERIFIED]`), CHALLENGE (the client offers only the null class as
   acceptor, 5.10, point 1), DEBUG, VERSION, the parameter types and any
   unassigned type `[OWN]`.
8. **Sizes.** A sealed DATA packet is at most the smallest of
   `rx.packet_max`, the peer's last advertised maximum packet size, and
   its recommended packet size, each read as 5.7, point 7 says. Before
   any ACK from the peer, 1444 `[X1 "Call Layer": UNVERIFIED]`. The
   payload before sealing is that less 28 and the class's overhead
   (5.8, point 3).
9. **The encoder refuses** what the decoder would drop, so IronAFS never
   sends a packet it would itself reject: a call number 0 on a call
   packet, a call number other than 0 on a connection-level packet, a
   type it never sends, a DATA packet with sequence 0, a DATA packet
   with an empty payload and no last-packet flag, an ABORT payload other
   than 4 bytes, a datagram longer than `rx.packet_max`. A refusal is a
   defect in the machine, never a panic: a packet of a call fails that
   call with `ProtocolViolation` (section 7); a connection-level packet
   is not sent and is counted (`encode_refused`).

### 5.6 Connections

1. **Wire identity of an outgoing connection** (ADR 0006 D2.1: Working;
   specification item 8):
   - the epoch (31 bits) and the connection id (30 bits) are each drawn
     whole from the `Rng` the machine is given; in production that is the
     cryptographic random source (C5, point 1). No clock, counter or
     other identity is an input;
   - a value is redrawn if: the epoch is 0, or the connection id is 0
     `[OWN]`, because X1 names no reserved value `[X1 "Rx Connections":
     UNVERIFIED]` and zero is the value an uninitialised field most
     often holds, so avoiding it costs one value in 2³⁰ and protects
     against a peer that treats it as "none" (section 13, Q4); the
     connection id equals that of a live outgoing connection, whatever
     its epoch; or the pair equals a reserved pair (point 2);
   - each new connection draws its own epoch: one per connection, not one
     per process;
   - after `rx.identity_redraws` draws with no usable value, the
     connection is not opened: the open fails with `IdentityExhausted`
     and the request waits or fails at its deadline (ADR 0006 D2.1);
   - a connection is opened only if the live outgoing connections plus
     the reserved pairs, counting the new one, stay within
     `rx.identity_bound`; otherwise the open fails with
     `IdentityExhausted` and no reservation is released early (ADR 0006
     D2.1);
   - identities are never persisted or reused across a restart of the
     machine.
   Every outgoing connection opened this way is eligible (ADR 0006 D1.9).
   The ignore-source flag is always clear, so the full 31 bits of epoch
   count: 61 bits in all, if X1's reading of the epoch word is right
   `[X1 "Rx Connections": UNVERIFIED]`, and 60 under ADR 0006's caveat
   otherwise.
2. **Retirement.** An outgoing connection retires when its user closes
   it, or when it has had no live call for the time its user set when
   opening it. On retirement its epoch and connection id pair is
   reserved for R (5.2), measured on the monotonic clock (C37), and
   longer while an ABORT event of the connection waits untaken in its
   reserved slot (5.11, "The event queue"); its connection id alone is
   not reserved, so it may be drawn again at once
   under another epoch (ADR 0006 D2.1). A packet for a retired identity
   matches nothing and is dropped as `unmatched`.
3. **Channels and call numbers** `[X1 "Rx Connections": UNVERIFIED]`:
   - a connection has four channels, 0 to 3; a channel carries one call
     at a time;
   - call numbers on a channel strictly increase; the first may be any
     positive value, and each later one is usually the previous plus 1;
   - **outgoing:** the first call number on each channel is 1: not a
     fact of X1 but ADR 0006, item 4's fallback, kept until the test cell
     accepts a random first call number (section 13, Q2); each later
     call is the channel's previous number plus 1. The **keepalive
     call** of C32 is the exception (C32, point 1; ADR 0006 D2.4): the
     user marks it; it takes a channel that has carried no call on its
     connection, and its call number is drawn from `Rng` in 1 to 2³⁰.
     Later calls on that channel go on from it by 1. A keepalive start
     on a connection with no such channel is refused with `StartRefused`
     (section 7), and nothing is sent; the user opens a new connection
     for it (point 1),
     whose four channels are all unused `[OWN]`, so a random call number
     never falls at or below one already used on its channel;
   - **retirement before overflow** (ADR 0006, item 5): IronAFS never
     sends a call number above 2³¹ − 1. X1's boundary is inconsistent:
     it names 2³¹ as the last call while saying implementations treat the
     field as signed `[X1 "Rx Connections": UNVERIFIED]`. When a channel
     would need a number above 2³¹ − 1, the connection takes no new call
     and retires when its live calls end; the user opens another;
   - **incoming:** a DATA packet with sequence 1 and a call number above
     the channel's current one starts a new call there (5.10). A call
     number of 0 or above 2³¹ − 1, or below the channel's current one, is
     dropped (`stale_call`) `[OWN]`, for the signed reading above.
4. **Serials** `[X1 "Rx Connections": UNVERIFIED]`: each connection's
   serial starts at 1 for the first packet IronAFS sends on it and rises
   by 1 for every packet sent, of every type and call, retransmissions
   included; after 2³² − 1 it wraps to 1, never 0.
5. **Incoming connections** keep the identity their initiator chose. They
   are never redrawn, never checked against point 1's uniqueness, and
   are told apart from outgoing ones by direction (ADR 0006 D2.1). An
   incoming connection whose epoch and connection id equal a live
   outgoing connection's is a different connection.
6. **The common connection lookup** (ADR 0006, item 1), defined once and
   used by every handler: a connection matches a packet only if all of
   these are equal: the receiving local endpoint (family and local
   endpoint generation); the peer endpoint, normalised (5.4, point 3);
   the direction, which for a received packet is the flag the peer sets
   on that connection: clear on an outgoing connection (the peer is the
   acceptor) and set on an incoming one (5.1, point 3); the whole 32-bit
   epoch word; and the connection id. The
   channel is not part of the connection lookup; each handler checks it
   where its rule needs it. The service id and security index are not
   part of it (5.4, point 8). The lookup's cost is bounded by the
   logarithm of the capped number of connections; it uses an ordered map
   or a hash keyed from `Rng`, never a hash seeded outside the machine
   `[OWN]`, so a sender cannot craft collisions and the machine stays
   deterministic under the simulator (coding standards rule 7).
7. **The call lookup** adds the channel and the channel's current call
   number to the connection lookup.

### 5.7 Calls

**Outgoing calls (IronAFS is the initiator).**

1. **Start.** The user starts a call on an open connection with: the
   request bytes or a stream, `rx.call.request_max`, `rx.call.reply_max`,
   T (`rx.idle_timeout`), a deadline on the monotonic clock no later
   than `rx.call.deadline_max` from now, and whether it is a keepalive
   call. Rx picks a free channel; if none is free, the start waits
   inside its deadline. A keepalive call is the exception (5.6, point
   3): it takes only a channel that has carried no call on its
   connection, which is always free, and if the connection has none it
   is refused at once with `StartRefused`, never left waiting. A call
   starts by sending DATA with sequence 1 `[X1 "Call Layer":
   UNVERIFIED]`.
2. **Request.** DATA packets carry the request in order, sequence 1, 2,
   and so on; the last carries the last-packet flag, and no request
   DATA follows it `[X1 "Call Layer", "RPC Layer": UNVERIFIED]`.
3. **Waiting.** After the last request packet, the call waits for the
   reply. The first reply DATA packet acknowledges every request packet
   `[X1 "Packet Types": UNVERIFIED]`.
4. **Reply.** Reply DATA is accepted in order within `rx.recv_window`
   (point 8). A reply DATA packet that arrives before IronAFS has sent
   its last request packet is dropped (`early_reply`) `[OWN]`: X1 says
   the acceptor sends nothing until it has the whole request `[X1 "RPC
   Layer": UNVERIFIED]`, and failing the call on such a packet would let
   a matched forgery end a call outside ADR 0006's bounds.
5. **Completion.** The call completes when every reply packet up to and
   including the one with the last-packet flag is received `[X1 "RPC
   Layer": UNVERIFIED]`; IronAFS then sends an ACK covering them at once
   (point 9). Reply bytes beyond `rx.call.reply_max` fail the call with
   `ReplyTooLarge`. Whether the reply holds exactly the expected bytes
   is the RPC layer's check (section 2). The outcome reports the
   connection's security class and level, and the receiving local
   endpoint, so the user can apply C19 and C32, point 1.

**Acknowledgements.**

6. **Receiving an ACK** for a live call (it matched the call lookup):
   - a first packet lower than the highest first packet already seen on
     this call is ignored (`ack_backwards`) `[X1 "Call Layer":
     UNVERIFIED]`, compared as an offset from the call's first unacked
     sequence (section 6);
   - a first packet beyond the next sequence IronAFS would send, or a
     SACK bit of 1 for a sequence IronAFS has not sent, drops the ACK
     (`ack_invalid`) `[OWN]`: it acknowledges data that does not exist;
   - every sequence below first packet is hard-acknowledged and its
     buffer may be released `[X1 "Call Layer": UNVERIFIED]`;
   - for each offset i with 0 ≤ i < min(N, 256), SACK octet i's least
     significant bit says whether first packet + i was received
     `[X1 "Call Layer": UNVERIFIED]`; with N = 0 there is no such offset,
     and the ACK only hard-acknowledges. IronAFS never has a packet in
     flight more than 254 sequences beyond first packet, since its
     transmit window is at most `rx.send_window_max` packets from its
     first unacknowledged one, which the ACK has just set to first
     packet. So the bit for offset i is the least significant bit of
     octet i in both formats, and the other bits and extra tables are
     never needed (5.4, point 12);
   - a soft-acknowledged packet (bit 1) is kept but not retransmitted; a
     negatively acknowledged one is retransmitted as flow control allows
     `[X1 "Call Layer": UNVERIFIED]`;
   - previous packet is not used `[OWN]`: X1 calls it unreliable without
     the extended-SACK flag `[X1 "Call Layer": UNVERIFIED]`, and IronAFS
     needs it in neither format.
7. **Trailers received** set the sender's view of the peer, each kept
   until the next ACK that carries it:
   - maximum and recommended packet size: a value below 28 + the class's
     overhead + 1 is ignored; a value above `rx.packet_max` is read as
     `rx.packet_max` `[OWN]`;
   - receive window: the transmit window is at most this, and at most
     `rx.send_window_max` `[X1 "Call Layer": UNVERIFIED]`; 0 is read as 1
     `[OWN]`, so a call still progresses one packet at a time and ends at
     its deadline if the peer never opens the window;
   - packets per jumbogram: ignored `[OWN]` (5.5, point 6).
8. **Receiving DATA** for a live call, in either role. With E the next
   sequence the user has not been given and W = `rx.recv_window`:
   - sequence 0: dropped (`bad_sequence`);
   - offset sequence − E (section 6) at least W: dropped, with an ACK of
     reason exceeds window `[X1 "Call Layer": UNVERIFIED]`;
   - a sequence already held or already given to the user: dropped, with
     an ACK of reason duplicate `[X1 "Call Layer": UNVERIFIED]`;
   - a sequence above the one that carried the last-packet flag: dropped
     (`after_last`) `[OWN]`;
   - a second packet with the last-packet flag at a different sequence:
     dropped (`after_last`) `[OWN]`;
   - otherwise it is held; if it is not E, an ACK of reason out of
     sequence is sent `[X1 "Call Layer": UNVERIFIED]`;
   - held packets from E on are given to the user in order; giving them
     advances E and so first packet in the next ACK `[X1 "Call Layer"
     ("Flow Control"): UNVERIFIED]`.
   The ACKs this point sends are limited to one per received DATA packet.
9. **Sending ACKs:** at once with reason requested when a DATA or ACK
   packet asks for it, carrying that packet's serial `[X1 "Call Layer":
   UNVERIFIED]`; with reason delay after `rx.ack_delay` while received
   DATA is unacknowledged; at once, with reason requested if asked and
   delay otherwise, when the last reply packet completes an outgoing
   call. IronAFS does not send reason idle or no space `[OWN]`: it holds
   only what its window allows, so it never drops for lack of buffer.
10. **ACKALL** received for a live call acknowledges every DATA packet
    IronAFS has sent on it `[X1 "Packet Types": UNVERIFIED]`; it is
    handled as an ACK whose first packet is IronAFS's next sequence,
    with the same checks.

**Retransmission and flow control.** IronAFS's own sending behaviour,
following X1's description `[OWN]`; X1's text is a lead for it, not a
wire fact, since it governs only what IronAFS sends.

11. **Timers and windows.**
    - An RTT sample is taken from an ACK whose serial field names a
      packet IronAFS sent on this call, whose reason is not delay, and
      whose serial is not 0 `[X1 "Call Layer" ("Round-trip time
      computation"): UNVERIFIED]`. Each retransmission has its own
      serial, so a sample is never ambiguous.
    - The averages are `RTTdev ← ¾·RTTdev + ¼·|RTTavg − R|`, then
      `RTTavg ← ⅞·RTTavg + ⅛·R`, in whole microseconds with checked
      arithmetic; the first sample sets RTTavg = R and RTTdev = R/2.
    - The retransmission timeout is RTTavg + 4·RTTdev + 350 ms
      `[X1 "Call Layer" ("Packet retransmission"): UNVERIFIED]`, at most
      `rx.rto_max`; `rx.rto_initial` before any sample. Each consecutive
      timeout of the same packet doubles it, up to `rx.rto_max`.
    - A packet not positively acknowledged for the timeout is sent again,
      with a new serial and the same sequence `[X1 "Rx Connections":
      UNVERIFIED]`.
    - Packets in flight never exceed the smallest of the transmit window
      (point 7), the congestion window, and `rx.send_window_max`.
    - Congestion control is X1's: a congestion window starting at 1
      packet; on three negative ACKs in a row or a retransmission
      timeout, the threshold becomes half the effective window, at least
      2; a timeout resets the window to 1; below the threshold the window
      grows by 1 per newly acknowledged packet, at or above it by 1 per
      ACK `[X1 "Call Layer" ("Flow Control"): UNVERIFIED]`.
12. **Keepalive and timeout** `[X1 "Call Layer" ("Keepalive and
    Timeout"): UNVERIFIED]`, per live call, with T its idle timeout and
    KT = T ÷ 6:
    - if nothing has been received on the call for KT, and no PING was
      sent in the last KT, IronAFS sends an ACK of reason ping with the
      request-ack flag;
    - a received ACK of reason ping with request-ack, on a live call,
      draws one ACK of reason ping response carrying the ping's serial,
      at most one per `rx.ping_reply_interval` per connection;
    - any packet received on the call that passes 5.4 and the call lookup
      counts as hearing from the peer; a forged packet that matches the
      call identity can therefore keep a call alive until its deadline,
      never past it;
    - with nothing heard for T, the call fails with `Timeout`; at its
      deadline it fails with `DeadlineExceeded`, whatever is heard.
13. **Rx never retries a call** `[OWN]`, from N45, point 4, and ADR 0006
    D1.5. A failed call reports `sent_any`: whether any request DATA
    packet of it was handed to the driver; a packet the call-end purge
    removed before the driver performed it (5.12, point 4) was not
    handed. Whether a call may be retried
    is the user's decision. Rx's part of ADR 0006, item 8: a call that
    ended by `Timeout` or `DeadlineExceeded`, including after a call-0
    hint, is retried by the user, through N7, only on a connection opened
    after it ended, so with a fresh wire identity (5.6, point 1). The old
    connection keeps serving its other calls and new ones; neither a
    timeout nor a hint closes it (ADR 0006 D1.2).

**Incoming calls (IronAFS is the acceptor).**

14. An incoming call is opened as 5.10 says. Its request DATA is received
    by point 8, up to the service's request cap. Rx holds the request,
    charged as 5.10, point 2, says, until it is complete, and then gives
    it to the service once, whole; the window of point 8 bounds only the
    packets held beyond E. While the service has not
    finished the reply, a DATA packet with sequence 1 for a higher call
    number on the same channel draws one BUSY for that call number, sent
    within the unverified budget (5.10, point 7), and is dropped
    `[X1 "Packet Types": UNVERIFIED]`. The service submits the reply
    whole; Rx takes it only if it is at most the service's reply cap and
    its bytes can be charged (5.10, point 2), and otherwise refuses the
    submission, counts it (`reply_refused`), sends no reply DATA, and
    leaves the call to end as below `[OWN]`. A reply submitted after the
    call ended (by the deadline below or by an ABORT, 5.9, point 6) is
    refused the same way, and the service is told the call had ended.
    The reply is sent as DATA with the last-packet flag on its final
    packet. The call completes
    when the peer acknowledges every reply packet, or when the peer
    starts a higher call number on the channel, which acknowledges the
    previous call `[OWN]`. It fails at `rx.in.request_deadline` for an
    incomplete request (5.10, point 9), and at `rx.in.reply_deadline`
    after the request was complete if by then the reply is not
    submitted or not every reply packet is acknowledged (`reply_expired`)
    `[OWN]`: the service's effects stand, the reply and its
    retransmissions stop, its queued sends are dropped (5.12, point 4),
    its charge is released, and no ABORT is sent (point 15). A peer that
    withholds its ACKs so holds reply state for at most
    `rx.in.reply_deadline`.

**Aborts IronAFS sends.**

15. IronAFS sends an ABORT only on a connection it holds, for a call or
    connection it holds, and only with a code its user supplies: when the
    user cancels an outgoing call, or a service refuses an incoming call.
    Rx itself sends no ABORT for its own failures (timeout, protocol
    violation, caps) in this version `[OWN]`, because the codes for them
    are MISSING (section 13, Q5): the peer then ends the call at its own
    timeout. An ABORT is sent at most once per call, and once more per
    received packet of that call afterwards, within the unverified
    budget on an incoming connection (5.10, point 7).

### 5.8 The security-class interface and the null class

The interface every class implements, in `ironafs-rx`, used by the
machine for each connection. A class is pure: it gets its keys, time and
randomness as inputs (C3, point 1). The interface is IronAFS's own
design `[OWN]`, from N45 and ADR 0006; X1 does not specify classes
`[X1 "Introduction": UNVERIFIED]`.

1. **Index and level.** A class object reports its security index and
   its level. Rx stamps the index on every packet of the connection and
   reports both with every call outcome, so the user knows a reply came
   over the null class (C19, C20).
2. **Packets it covers.** A class seals and opens DATA payloads only.
   ACK, ABORT, BUSY and ACKALL pass through no class `[OWN]`: no class
   can protect an ABORT `[X1 "Packet Types": UNVERIFIED]` (claim record
   row `[ADR-002]`), and ADR 0006 treats ABORT, BUSY and CHALLENGE as
   unprotected at every class. A class specification that protects
   another type amends this point first.
3. **Overhead.** A class states the most bytes it adds to a payload. Rx
   sizes each DATA payload so the sealed packet fits 5.5, point 8.
4. **Seal.** For each transmission of a DATA packet, retransmissions
   included, Rx gives the class the header it will send (serial
   included) and the payload; the class returns the sealed payload and
   the security-specific field, or `KeyExhausted`. Rx keeps the payload
   before sealing and seals again for each transmission, since the class
   may bind the serial.
5. **Open.** For each received DATA packet that matched a live call (or,
   as acceptor, opens one), Rx gives the class the received header and
   payload; the class returns the payload or a rejection. A rejected
   packet is dropped (`class_reject`): no reply, no change to the call,
   and it does not count as hearing from the peer (5.7, point 12)
   `[OWN]`.
6. **Rekey request** (N45, point 1). A class may report, from seal or
   open, that it asks for a new key or a reconnect. Rx hands it to the
   user as `RekeyRequested` on that connection and changes nothing else.
   The user runs N7's predicate and then either installs a new class
   object on the connection, if the class supports rekeying in place, or
   opens a new connection. Requests not yet sent continue on the new key
   or connection within their own deadlines (N45, point 3).
7. **Key exhausted** (N45, point 2; N6, point 3). When seal returns
   `KeyExhausted`, no packet that needs sealing leaves the connection
   again. Each live call on it fails with `KeyExhausted`, which is never
   reported as a transport failure, with its `sent_any` (5.7, point 13).
   The user requeues calls with `sent_any` false through N7, and treats
   the others under the lost-reply rule (C28; N45, point 4). The
   connection takes no new call and retires when its calls end.
8. **Challenge, as initiator.** Given a CHALLENGE payload that passed
   5.9, point 3's checks, the class returns a RESPONSE payload or a
   refusal. It decodes the payload under its own specification's caps.
9. **Challenge, as acceptor.** The interface has a place for a class to
   issue a CHALLENGE and verify a RESPONSE on incoming connections. No
   class uses it in this version: the callback service accepts only the
   null class (5.10, point 1).
10. **The null class**, index 0 `[X1 "Rx Connections": UNVERIFIED]`:
    - overhead 0; seal returns the payload unchanged and a
      security-specific field of 0; open returns the payload unchanged
      and ignores the received security-specific field `[OWN]`, since it
      carries nothing the null class reads;
    - it never asks for a rekey and never reports `KeyExhausted`;
    - it refuses every CHALLENGE, so no RESPONSE is ever sent on a null
      connection; the CHALLENGE is counted (`challenge_refused`);
    - its level is "none", and every outcome on it reports so.

### 5.9 Unprotected packets: ABORT, BUSY, CHALLENGE, ICMP

This section carries out ADR 0006, decision 1 (Firm) and decision 2,
points 2 and 3 (Working). Each point names the ADR point it keeps.

1. **ABORT with a call number, on an outgoing connection** (D1.1, D1.5,
   D1.6, D1.9). It is considered only if it matches the call lookup
   (5.6, point 7): the connection tuple, the channel, and the channel's
   current call number of a live call. Then:
   - on an eligible connection, that call ends with `Aborted`, carrying
     the code and `sent_any`; no other call, and no other connection,
     changes; Rx hands the user an `AbortReceived` event with the
     connection, the call, the code and `eligible = true`, which the user
     applies under D1.5 and D1.6;
   - on any other connection (an outgoing one opened with an identity
     not drawn by 5.6, point 1, which only a test fixture can make), it
     is counted and handed as `AbortReceived` with `eligible = false`;
     the call runs on to a valid reply or its deadline (D1.9).
2. **ABORT with call number 0, on an outgoing connection** (D1.2). It is
   considered only if it matches the common connection lookup; the
   channel is not checked, because X1 states no channel for a
   connection-level packet (section 13, Q4). A matching one is counted
   and handed to the user as `AbortHint` with the connection, the code
   and `eligible`. It ends, fails, fences and retries no call; it does
   not close or re-key the connection and does not change its wire
   identity, and the connection serves the next call. A call on it that
   later times out is retried on a fresh connection (5.7, point 13).
3. **CHALLENGE** (D1.3; specification item 6). A RESPONSE is sent only
   if all of these hold, checked in this order, and otherwise the
   CHALLENGE is counted (`challenge_refused` or `unmatched`) and nothing
   is sent:
   - its call number is 0 (5.4, point 5). This is the packet type's own
     discriminator; X1 states that a CHALLENGE is a connection-only
     packet `[X1 "Packet Types": UNVERIFIED]` and states no channel, so
     no channel value is refused;
   - it matches an outgoing connection by the common connection lookup,
     and that connection is open: not retired, not fenced, not
     `KeyExhausted`. The peer may challenge at any time while it is open,
     since a restarted peer challenges again `[OWN]`;
   - fewer than `rx.responses_per_handshake` RESPONSEs have been sent on
     the connection since a DATA or ACK of a call on it was last
     accepted, and the last was at least `rx.response_interval` ago;
   - the class (5.8, point 8) returns a RESPONSE;
   - the user permits it: Rx hands `ChallengeReceived` to the user, who
     runs N7's predicate with the credential generation and fences then
     current and answers with the permit generation it approved; Rx
     builds the RESPONSE only if that is the connection's current permit
     generation, and the driver sends it only if it still is immediately
     before the send (5.12, point 4). Since the user raises the permit
     generation whenever the credential generation or a fence that
     applies to the connection changes, a RESPONSE overtaken by a fence
     or a credential change is discarded, never sent (N7, points 2 and 5;
     ADR 0006 D1.3: Firm).
   The RESPONSE goes to that connection's peer endpoint, from its
   receiving local endpoint, on its own wire identity and class (D1.3).
   A repeated CHALLENGE is answered again under the same limits, so a
   lost RESPONSE or a restarted peer recovers (specification item 6).
4. **Anything that does not match** (D1.4; specification item 7) is
   dropped without a reply, with no allocation, and counted:
   - per source address, in a table of `rx.drop_table_entries` entries
     fixed at start; once the table is full, a source not in it is
     counted in one aggregate counter;
   - the table is never cleared by traffic. At each report (5.11) whose
     `DropCounts` is queued, an entry with a non-zero count in the
     interval is zeroed and kept for its source; a report the queue
     cannot take zeroes nothing, and its counts go out with the next one
     that is queued; only an entry whose counts stayed zero for the whole
     interval is freed for a new source. So a flood that names many
     sources cannot evict a source that is still sending, and a table
     filled once is not full forever: a source that stops sending frees
     its entry after one quiet interval. A source first seen while the
     table is full is counted in the aggregate until an entry frees;
   - each table entry also holds a per-source rate: unmatched packets
     from that source beyond `rx.unmatched_rate` in a second are counted
     as `over_rate`, and the source is marked as flooding in the next
     report; the packets are dropped either way, with the same work;
   - no log line is written per packet; Rx hands the user the counts at
     most once per `rx.drop_report_interval`, as one `DropCounts` event;
   - the driver reads at most `rx.read_batch` datagrams before it runs
     the machine's timers and sends (5.12, point 3), so unmatched traffic
     cannot stop live calls' retransmissions, ACKs and timeouts. The
     work per unmatched packet is the checks of 5.4, which are bounded.
   RESPONSE received by IronAFS as initiator, CHALLENGE received as
   acceptor, and RESPONSE received as acceptor with no CHALLENGE pending
   (always, in this version) are dropped here `[X1 "Packet Types":
   UNVERIFIED]`.
5. **ABORT or BUSY on a connection whose call does not match** (a stale
   or future call number, or a channel with no live call) is point 4's.
6. **ABORT with a call number on an incoming connection** (D1.8; D2.3).
   The lookup is the call lookup, exact. A matching one is counted and
   stops only IronAFS's reply to that call and its retransmissions:
   - if the request was complete, it was already given to the service,
     whose effects stand; the ABORT never suppresses, delays or undoes
     them. The call then ends: its reply, whether queued, in flight or
     not yet submitted, is never sent or sent again, its queued sends are
     dropped (5.12, point 4), and its charge is released `[OWN]`, so the
     call holds no state without a timer;
   - if the request was incomplete, the call is kept until
     `rx.in.request_deadline` and given to the service if it completes;
   - the event `AbortReceived` is handed with `eligible = false`; it
     never raises callback trust.
   A call-0 ABORT on an incoming connection is counted and changes
   nothing else `[OWN]`, by the same reading of D1.8.
7. **BUSY on an outgoing call** (D1.7; specification item 3). It is
   considered only if it matches the call lookup, and accepted only while
   no DATA or ACK of that call has been accepted from the peer. The
   events that close that window are: an ACK of the call that passes
   5.7, point 6; a reply DATA packet of the call that passes 5.7, point
   8; an ACKALL of the call. Once closed, a BUSY is point 4's. An
   accepted BUSY delays the next retransmission of the call's first
   packet by `rx.busy_backoff`, doubling per BUSY; when the call's total
   deferral would pass `rx.busy_defer_max` (the smaller of 10 s and half
   the time from the call's start to its deadline), the BUSY is counted
   and ignored, and the call keeps its normal retransmissions. A BUSY
   never fails a call; the call ends only by its deadline or another
   outcome.
8. **BUSY on an incoming call** (D1.8; D2.3) changes nothing but a
   counter. X1 gives a BUSY from initiator to acceptor no meaning
   `[X1 "Packet Types": UNVERIFIED]`.
9. **ICMP and socket errors** (D2.2; specification item 10). An error the
   driver gets from a send or a receive on a UDP socket is handed to the
   machine as a hint, with the peer endpoint if the system reports one
   and "unattributed" otherwise. The machine counts it by family and
   kind, per peer address in a table of `rx.icmp_table_entries` entries
   fixed at start, with one aggregate entry for unattributed errors and
   for peers not in the table. It hands the user an `IcmpHint` event for
   an entry at most once per `rx.icmp_hint_interval`, carrying the counts
   since that entry's last event; an entry's counts are zeroed only when
   its own `IcmpHint` is queued and carries them, never by point 4's
   report. An `IcmpHint` the queue cannot take (5.11, "The event queue")
   zeroes nothing: its counts stay in the entry and go out with that
   entry's next `IcmpHint` that is queued. An entry
   is freed for a new peer only when it has had no error and holds no
   count for a whole `rx.drop_report_interval`; traffic never clears the
   table. So an error stream at packet rate costs counting and at most
   one event per entry per interval, and every error is carried by
   exactly one `IcmpHint` `[OWN]`. It never ends a call,
   never closes or rebinds the socket, never changes the local endpoint
   generation, and never changes the network generation, a fence or a
   trust state. The driver keeps reading after it. Path MTU signals are
   not used: packet sizes are fixed by 5.5, point 8 `[OWN]`. How each
   operating system reports these errors is section 13, Q6.
10. **Order of checks** for these types (specification item 1): 5.4,
    points 1 to 7, then the type's discriminator (point 3's call number
    for CHALLENGE; call number 0 or not for ABORT), then the connection
    lookup, then the call lookup where the type needs it, then its
    length check (5.4, points 13 and 14), then its handler. A packet that
    fails a check is counted under the first check it fails.

### 5.10 The server role: admission, caps, overload and pause

The callback service is an Rx service that accepts calls from any address
that can reach the client's port (C30, point 1; C31). This section is
what Rx does for any service it accepts calls for; it knows nothing of
callbacks (C4, point 4).

1. **Registration.** A service is registered with its service id, the
   security indices it accepts, its request cap (at most
   `rx.in.request_ceiling`) and its reply cap (at most
   `rx.in.reply_ceiling`); a registration over either ceiling is
   refused. The callback service
   accepts only index 0 before rxgk (C31) `[ADR 0004 C31: Working]`. A
   new incoming call for a service id not registered, or with an index
   the service does not accept, is dropped (`no_service`,
   `bad_security_index`) and nothing is sent `[OWN]`.
2. **Charges.** Each incoming connection is charged `rx.in.conn_charge`;
   each incoming call `rx.in.call_charge` plus the payload bytes it
   holds: its request, in order or held in the window, until the call
   ends, and its reply from submission until the reply is acknowledged
   or the call ends (5.7, point 14); each held datagram (point 5) its
   length. Every charge but a held datagram's is made to the peer
   address (its normalised address, any port) and to the total; a held
   datagram's is made to `rx.in.held_bytes` and the total only (point
   5). A charge to the total other than a held datagram's must fit
   `rx.in.total_bytes` less `rx.in.held_bytes`, so the held allowance is
   always free inside the total and a datagram that fits
   `rx.in.held_bytes` always fits the total. Each charge is made before
   the state is created or grown, with checked arithmetic, and is
   released when the state is freed. A reply whose charge does not fit
   is refused at submission (5.7, point 14); it is not an overload, since
   no received datagram waits on it. `rx.in.peer_bytes` covers one call
   at both ceilings with its records (5.2), so a service registered at
   both ceilings can complete its largest call from one peer.
3. **Admission.** A DATA packet with sequence 1 and the client-initiated
   flag that matches no connection opens an incoming connection, and one
   that matches an incoming connection but no live call on its channel
   opens a call there (5.6, point 3), only if all of these hold: the
   service is registered and accepts the index (point 1); the peer has
   fewer than `rx.in.conns_per_peer_max` incoming connections; the
   charges fit `rx.in.peer_bytes` and the total as point 2 says. A later
   DATA packet of an admitted call is admitted only if its payload's
   charge fits both and the service's request cap.
4. **At a cap: the overload event** (C31, the rate-limit row). A packet
   that point 3 refuses only because of a cap, a per-peer connection
   limit or the service's request cap is an **overload**. Rx records the
   overload with the peer address, and hands the user an `Overload`
   event with an id and that source. If `rx.in.overloads_max` events are
   already unconfirmed, the new overload joins one pending event whose
   source is "unknown"; while that event is pending, every further
   overload joins it. An overload of an incoming call that a peer had
   already been admitted for is the same event, and the call keeps the
   bytes it was admitted with.
5. **Hold until confirmed.** The datagram is not discarded when the
   overload is recorded. It is held, charged to `rx.in.held_bytes` and
   the total, under its event's id. It is not charged to its peer
   `[OWN]`: it was held because a cap refused it, often that peer's own,
   and charging it there could only refuse it again. So one source can
   fill the held allowance and cause point 6's pause; the cost is the
   fence that pause raises for every server on the socket. While paused,
   the kernel may drop any server's datagrams, but none before that
   server's suspect epoch has moved (point 6; section 8). Nothing held
   is processed, acknowledged or answered. When the user **confirms** an
   event (the callback service confirms only after the suspect epoch of
   the affected servers has advanced, under its own specification),
   every datagram held under it is dropped and counted (`overload_dropped`), and its
   charge released. So the epoch moves before any work is dropped or
   acknowledged (C31, the rate-limit row). A datagram that would not fit
   `rx.in.held_bytes` is not held: it is point 6's. One that does fit
   `rx.in.held_bytes` always fits the total too, since the held
   allowance is kept free inside the total (point 2).
6. **Pause** (the issue's fence; C30, point 1; C31, the rate-limit row).
   When a datagram cannot be held (point 5), the machine tells the
   driver to pause the socket it arrived on, and that datagram joins the
   "unknown source" event of point 4. The driver then:
   - raises that socket's pause generation by 1, itself, with no lock and
     no wait on the user, as an atomic store visible to every thread
     before its next step (5.12, point 5);
   - only then stops reading the socket;
   - and hands the machine a `Paused` notice, from which the machine
     hands the user a `Pause` event naming the socket.
   A pause is an overload from an unknown source for every server whose
   traffic arrives on that socket, because the kernel may then drop a
   packet from any of them. The pause generation is part of the suspect
   epoch of each such server, under the callback service's
   specification, so raising it advances all those epochs at once,
   before any datagram can be dropped by the kernel. No user action is
   awaited for that: the fence holds before the first unread interval,
   however late the user handles any event.
   **Resume.** When confirmations leave at least half of
   `rx.in.held_bytes` free, the machine tells the driver to resume. The
   driver raises the pause generation by 1 again, itself, before its
   first read after the pause, then reads, and the machine hands the
   user a `Resume` event.
   The `Pause` and `Resume` events are for counters and diagnostics
   only; the fence never waits on them. While paused, the machine still
   runs timers and sends.
7. **What is sent to a source with no established connection** (C39,
   point 3; coding standards rule 3; the issue's "unverified source").
   - To a source with no connection at all: nothing. Every packet that
     opens nothing is dropped (5.4, point 7).
   - To the peer of an incoming connection that is not yet verified: in
     total, at most `rx.unverified_factor` times the bytes received from
     it on that connection, counting every header. Reply DATA and PINGs
     that do not fit wait, at most until the call's
     `rx.in.reply_deadline` (5.7, point 14), and are dropped with the
     call (5.12, point 4); ACK, BUSY and ABORT packets that do not fit
     are skipped, never queued `[OWN]`, so nothing outlives the call or
     connection it answers.
   - **The reachability test** `[X1 "Introduction": UNVERIFIED]`: X1
     recommends a PING and PING-RESPONSE exchange before any response
     larger than the request. When a reply waits on the budget, Rx sends
     the reachability PING of 5.5, point 4: an ACK of reason ping with
     request-ack, N = 0 and no reserved octets or trailers (46 bytes,
     within the budget of any received datagram, which is at least 28
     bytes), at most `rx.verify_pings_max` times per connection. An ACK
     of reason ping response on that connection whose serial field is
     the serial of one of those PINGs verifies the peer, and the budget
     no longer applies; the caps of points 2 to 6 still do.
   - The residual: X1 starts serials at 1, so a sender off the path that
     forged the connection can guess the PING's serial and verify itself.
     It then gains at most the replies of the calls it is charged for,
     inside its per-peer and total caps (section 8; section 13, Q7).
8. **Incoming call lookup** (ADR 0006, item 9) is the exact call lookup
   of 5.6, point 7, with the direction "client-initiated set".
9. **Receive deadline.** An incoming request that is not complete
   `rx.in.request_deadline` after its first packet was admitted is
   dropped with its charge released, and counted (`request_expired`); no
   ABORT is sent (5.7, point 15).
10. **Retirement.** An incoming connection with no live call for
    `rx.in.conn_idle` is retired, every queued send on it is dropped, and
    its charge released. A packet for it then matches nothing.

### 5.11 Counters and events

Rx hands these to its user, which emits them through the event catalogue
(ADR 0004, O1) and shows them in diagnostics (O12). Rx writes no log
itself. Catalogue ids are assigned in the catalogue, not here `[OWN]`.
No event carries a byte of a payload, a code from a packet other than an
ABORT's, or any key material.

| Event | Carries | When |
|---|---|---|
| `DropCounts` | per-source counts by reason, and the aggregate | at a fixed interval, if any count is non-zero |
| `AbortReceived` | connection, call, first code, last code, `eligible`, `sent_any`, repeat count | a matching call-level ABORT (5.9, points 1 and 6) |
| `AbortHint` | connection, first code, last code, `eligible`, repeat count | a matching call-0 ABORT on an outgoing connection (5.9, point 2) |
| `BusyDeferred` | connection, latest call, its total deferral, repeat count | an accepted BUSY (5.9, point 7) |
| `ChallengeReceived` | connection | a CHALLENGE that passed 5.9, point 3's checks up to and including the class's RESPONSE; never on a null-class connection, whose class refuses every CHALLENGE |
| `RekeyRequested` | connection, repeat count | 5.8, point 6 |
| `IcmpHint` | family, peer address or "unattributed", counts by kind since the entry's last queued event | 5.9, point 9, at most once per `rx.icmp_hint_interval` per entry |
| `EventsDropped` | per kind, the events not queued because the queue was full, and the `DropCounts` and `IcmpHint` events deferred | 5.11, "The event queue" |
| `Overload` | event id, source or "unknown", count of overloads under it | 5.10, point 4 |
| `Pause`, `Resume` | address family, the latest local endpoint generation, the latest of the two, the number of each since the user last took the slot | 5.10, point 6 |
| `IdentityExhausted` | the reason: redraws or bound; repeat count | 5.6, point 1 |
| `Diagnostic` | connection, `service_mismatch` or `security_index_mismatch`, repeat count | 5.4, point 8, counted, at most once per connection per interval |

A repeat count is the number of events merged into the first, 0 when
none was.

Every drop reason named in 5.4, 5.5, 5.7, 5.9 and 5.10 is a counter, per
source where a source is known.

**The event queue** (C39, point 3) `[OWN]`. Events wait for the user in
one queue, bounded in bytes however fast a peer can make events. Call
outcomes (section 7) never pass through it: each is returned with its
call, so an event the queue does not hold never loses an outcome.

1. **Reserved slots**, never refused and outside
   `rx.event_queue_bytes`: one per pending overload event id (at most
   `rx.in.overloads_max` + 1, 5.10, point 4); one per address family for
   its pause state; one for `EventsDropped`; and, for each outgoing
   connection, one for its `AbortHint` and one for its `AbortReceived`.
   Those two carry the `eligible` flag on which ADR 0006 D1.6's move to
   bounded revalidation depends (5.9, points 1 and 2), so neither is
   ever lost. An outgoing connection whose ABORT event waits untaken
   keeps its identity reserved, counted against `rx.identity_bound`,
   until the user takes it (5.6, point 2), so these slots are at most
   2 × `rx.identity_bound`; a user that never takes them makes new opens
   fail with `IdentityExhausted`, never an unbounded queue.
2. **Merging.** An event is merged into one already queued of the same
   kind and key, as the table below says; a merged event keeps the place
   of the first. The key of a call-level event is its connection, never
   its call, because a peer chooses call numbers and can open and end
   calls cheaply.
3. **Capacity.** Every other queued event counts its size against
   `rx.event_queue_bytes`; a merge changes values in place and never
   adds to the size. An event that would pass the bound is not queued:
   it is counted under its kind in the `EventsDropped` slot. A
   `DropCounts` or `IcmpHint` is not lost that way: its counts stay in
   their table and go out with the next one that is queued (5.9, points
   4 and 9), and `EventsDropped` counts the deferral. An `AbortReceived`
   on an incoming connection is ordinary: it changes nothing but
   counters (5.9, point 6). Rx never waits on the user and never
   allocates past the bound for an event.
4. **Failing closed.** An event not queued never makes Rx send anything.
   A `ChallengeReceived` not queued means no RESPONSE is sent for that
   CHALLENGE (5.9, point 3); the peer challenges again.

| Kind | Key | Merge |
|---|---|---|
| `DropCounts` | none | counts added per source; a source not in the queued event goes to its aggregate |
| `AbortReceived` | connection (a reserved slot on an outgoing connection) | the first event's call, code, `eligible` and `sent_any` are kept; a repeat count rises by 1, and the last code is kept beside the first |
| `AbortHint` | connection (a reserved slot) | the first code is kept; a repeat count rises by 1, and the last code is kept beside the first |
| `BusyDeferred` | connection | the latest call and its total deferral replace the earlier; a repeat count rises by 1 |
| `ChallengeReceived` | connection | one per connection; Rx keeps only the latest CHALLENGE's RESPONSE for it, so the user's one answer covers it |
| `RekeyRequested` | connection | one per connection; a repeat count rises by 1 |
| `IcmpHint` | family and peer address, or "unattributed" | counts added by kind |
| `Overload` | event id (reserved slot) | never merged into another id's; a further overload under the same id adds to its count (5.10, point 4) |
| `Pause`, `Resume` | address family (reserved slot) | the slot keeps the latest of the two, its local endpoint generation, and the number of each since the user last took it |
| `IdentityExhausted` | the reason | a repeat count rises by 1 |
| `Diagnostic` | connection and reason | a repeat count rises by 1 |
| `EventsDropped` | none (reserved slot) | counts added by kind |

### 5.12 The machine and the driver

1. **Sockets.** The driver (`ironafs-net`) owns one UDP socket per
   address family (C30, point 1) and the `rx` thread (C30, point 2). The
   IPv6 socket carries IPv6 only; how that is set on each system is
   section 13, Q6.
2. **Receive.** The driver reads each datagram into a buffer of
   `rx.packet_max` + 1 bytes and hands the machine the bytes, the
   receiving local endpoint, the peer endpoint and the monotonic time of
   the read.
3. **Loop.** The `rx` thread repeats: read at most `rx.read_batch`
   datagrams that are ready, without waiting past the machine's next
   timer; hand each to the machine; run the machine's timers; perform
   the machine's actions. It takes no cache lock (C30, point 2).
4. **Sends and the permit generation** (N7, point 5). Every send action
   names its connection, its call where the packet belongs to one, and
   that connection's permit generation when the machine built it. The
   user raises a connection's permit generation, as an atomic value the
   driver can read, when it fences or closes the
   connection, and whenever the credential generation or a fence that
   N7's predicate applies to the connection changes (N7, points 2 and 5;
   ADR 0006 D1.3: Firm). Immediately before each send system call, the
   driver compares the action's generation with the current one and
   discards the datagram if they differ. When the user fences a
   connection, the machine also drops every queued send on it. So no
   packet leaves on a connection after the user withdrew its permission,
   including a RESPONSE (5.9, point 3) and a retransmission.
   **A call's queued sends end with it** `[OWN]`. The machine's actions
   wait in its queue until the driver performs them at the end of a loop
   (point 3), so a send built earlier in a batch could otherwise leave
   after a later datagram of the same batch ended its call. When a call
   ends, by any outcome or by an incoming ABORT (5.9, point 6), the
   machine removes every queued DATA packet and PING of that call, first
   sends and retransmissions alike, before it returns; and it never
   builds one for a call that has ended. Only the final ACK of 5.7,
   point 9, and an ABORT of 5.7, point 15, are built for a call as it
   ends or after. So no request DATA of a call leaves after the call
   ended with `Aborted` or `Timeout` (ADR 0006 D1.5: Firm), and no reply
   DATA after a matching incoming ABORT (ADR 0006 D1.8: Firm).
5. **The pause generation** of each socket is a 64-bit counter in the
   driver, readable by any thread with no lock, written only by the `rx`
   thread. A raise is a sequentially consistent store, made before the
   thread's next read decision. It starts at 0 and only rises; an odd
   value means "paused". The callback service's serve path reads it on
   every serve (its specification).
6. **Time and randomness** reach the machine only as inputs: a monotonic
   instant with each call into it, and an `Rng` for identity draws and
   keepalive call numbers (C3, point 1; coding standards rule 7). The
   machine reports its next timer as an instant.
7. **Local endpoint generation.** When the driver binds a family's socket
   again, it raises that family's local endpoint generation and tells the
   machine; every connection on the old generation stops matching
   packets (5.6, point 6), so their calls run to their deadlines.

## 6. Arithmetic

Every value read from a packet is untrusted (coding standards rule 2).

1. **Sequence offsets.** Sequences wrap after 2³² − 1 `[X1 "Rx
   Connections": UNVERIFIED]`, so every comparison of two sequences is an
   offset: `b.wrapping_sub(a)` as an unsigned 32-bit value, compared with
   a window size of at most 255. Wrapping subtraction is the format's
   definition here, not an overflow (rule 2's exception for values the
   format defines as wrapping).
2. **ACK ranges.** First packet + i for i < 256 is a wrapping addition
   for the same reason; it is compared only as an offset from the call's
   first unacknowledged sequence.
3. **Lengths.** 18 + N and 21 + N + 4k use checked addition on `usize`
   and are compared with the payload's length before any read.
4. **Serials** wrap to 1, skipping 0 (5.6, point 4); the increment is
   explicit.
5. **Call numbers** are compared as unsigned 32-bit values after the
   range check of 5.6, point 3; the increment by 1 is checked against
   2³¹ − 1.
6. **Charges and budgets** (5.10) use checked addition and subtraction on
   `u64`; an overflow is treated as over the cap.
7. **Time.** Deadlines and timers use the checked instant arithmetic of
   C37's types; a deadline that cannot be represented fails the call
   start with `DeadlineInvalid`. RTT values are whole microseconds in
   `u64` with checked arithmetic; a sample whose arithmetic overflows is
   discarded.
8. **Trailer values** are clamped as 5.7, point 7 states (clamping is
   the defined reading there).

## 7. Errors and failure

1. **Dropped packets** are not errors: they are counters (5.11). No drop
   panics, allocates per packet, or changes state other than its counter.
2. **Call outcomes**, a closed set `[OWN]`; tests assert the exact kind
   (coding standards rule 15):

   | Outcome | When | Carries |
   |---|---|---|
   | `Completed` | 5.7, point 5 | reply bytes, class and level, receiving local endpoint |
   | `Aborted` | 5.9, point 1, eligible connection | code, `sent_any` |
   | `Timeout` | 5.7, point 12 | `sent_any` |
   | `DeadlineExceeded` | the call's deadline passed | `sent_any` |
   | `KeyExhausted` | 5.8, point 7 | `sent_any` |
   | `Fenced` | the user fenced or closed the connection | `sent_any` |
   | `Cancelled` | the user cancelled the call | `sent_any` |
   | `ReplyTooLarge` | 5.7, point 5 | `sent_any` |
   | `ProtocolViolation` | the machine's own encoder refused a packet (5.5, point 9) | `sent_any` |
   | `IdentityExhausted` | 5.6, point 1 (an open, not a call) | the reason |
   | `StartRefused` | a keepalive start on a connection with no unused channel (5.6, point 3); nothing was sent | none |
   | `DeadlineInvalid` | section 6, point 7 | none |

3. **No content in errors** `[OWN]`, from coding standards rule 8: an
   outcome carries no payload byte and no field of a packet other than
   an ABORT's code. Errors from the class carry what the class's own
   specification allows.
4. **No panic** is reachable from any datagram or any sequence of
   datagrams (coding standards rule 1).
5. **Fail closed.** A packet that cannot be placed changes nothing. A
   connection that cannot get a usable identity is not opened. A
   RESPONSE that cannot be shown permitted at send time is not sent.

## 8. Security

Who can send the bytes Rx decodes: every server the client calls; any
host that can reach the client's UDP port, for the callback service; and
anyone who can forge a source address (off the path) or see the traffic
(on the path) (ADR 0006, Context; O23 as ADR 0006 supersedes it).

| Threat | What stops it |
|---|---|
| Ending calls with forged ABORTs from off the path | the full call lookup (5.9, point 1; 5.6, points 6 and 7); 61 random bits of wire identity per connection (5.6, point 1); only eligible connections end a call (5.9, point 1) |
| Ending calls from on the path | not stopped: a denial of service bounded by ADR 0006 D1.5 and D1.6, the residual O23 records; a mutation's outcome becomes unknown, never lost or repeated, because Rx never retries (5.7, point 13) |
| Killing a connection with a call-0 ABORT | a hint only (5.9, point 2) |
| Stalling a call with BUSY | only before acceptance, capped deferral inside the deadline (5.9, point 7) |
| Drawing RESPONSEs to a victim, or a RESPONSE after a fence | only to the connection's own peer, after an identity match, rate limited, and checked against the permit generation at send time (5.9, point 3; 5.12, point 4) |
| Using the client as a reflector or amplifier | no reply to anything that opens nothing (5.4, point 7; 5.9, point 4); at most twice the bytes received to an unverified peer (5.10, point 7); no DEBUG or VERSION answers (5.4, point 4) |
| Exhausting memory with new connections or calls | per-peer and total byte caps, a per-peer connection limit, all checked before state is made (5.10, points 2 and 3); the identity bound (5.6, point 1) |
| Exhausting memory with large requests or replies | the per-call request and reply caps and the receive window (5.2; 5.7, points 5 and 8); on incoming calls, the service's caps under `rx.in.request_ceiling` and `rx.in.reply_ceiling`, every request and reply byte charged to the peer and the total (5.10, points 1 and 2), and a refused reply when its charge does not fit (5.7, point 14) |
| Holding reply state by withholding ACKs | `rx.in.reply_deadline` ends the call, drops its queued sends and releases its charge (5.7, point 14; 5.12, point 4) |
| Filling the held allowance from one source | not stopped: held datagrams are charged to the held allowance and the total, not to their peer (5.10, point 5), so one source can cause a pause. The cost is the pause's fence for every server on the socket, which the callback service sees and revalidates. While paused the kernel may drop any server's datagrams, undetected by Rx (the row below), but none before that server's suspect epoch has moved, because the pause generation rises before reading stops (5.10, point 6) |
| Flooding the user with events | a queue bounded by `rx.event_queue_bytes` plus fixed reserved slots; call-level events keyed by connection, not call; events past the bound counted, never queued (5.11, "The event queue"); ICMP and socket errors at most one event per entry per `rx.icmp_hint_interval` (5.9, point 9) |
| Hiding dropped breaks from the callback service | the overload event and hold until confirmation (5.10, points 4 and 5); the pause generation, raised before reading stops (5.10, point 6) |
| Breaks lost in the kernel during a flood, without a pause | not detected by Rx in this version: the operating system's own drop counters are not read (section 13, Q6). C32 bounds the effect: trust lapses when keepalive replies stop arriving |
| Starving live calls with unmatched traffic | bounded work per packet, no allocation, bounded reads per loop (5.9, point 4; 5.12, point 3) |
| Evicting drop records with many sources | a fixed table that traffic never clears, and an aggregate counter (5.9, point 4) |
| Hash-flooding the lookups | ordered maps or hashes keyed from `Rng` (5.6, point 6) |
| A forged ICMP error ending calls or forcing a rebind | ICMP and socket errors are hints only (5.9, point 9) |
| Crashing the daemon | no panic, checked arithmetic (sections 6 and 7); fuzz targets (10.4) |
| Using unauthenticated data where a credential applies | every outcome reports its class and level (5.8, point 1); C19 is enforced by the user |
| A self-verified spoofed callback peer | the residual of 5.10, point 7: bounded by the caps; section 13, Q7 |

## 9. How the invariants are kept

| Invariant (section 1) | Kept by | Test that fails if it is broken |
|---|---|---|
| Constraint 4: no panics, checked arithmetic, byte limits | 5.2; 5.4; section 6; 7, point 4 | fuzz targets (10.4); reject corpus (10.3) |
| C3, point 1: pure, no socket, clock or randomness | 5.12, point 6; the machine is `ironafs-rx` | the pure-crate check of `cargo xtask check` (C4) |
| C4, points 1, 3 and 4: crates, and Rx knows nothing of AFS | section 2, point 2; 5.10's service registration | the dependency-graph check; review: no AFS term in `ironafs-rx` |
| C5, point 1, and ADR 0006 D2.1: identity from the production `Rng` | 5.6, point 1 | A9 (10.5) |
| C6, points 1 and 4; N31, points 1 and 2 | every point labelled; wire facts UNVERIFIED until fixtures | review of this specification; the capture-backed revision |
| C19, point 1: null-class data never feeds an authenticated path | 5.8, points 1 and 10: outcomes report class and level | A17 |
| C24, point 7, and C32, point 1: trust only from a reply matched by its random identity | 5.6, point 3 (keepalive call number from `Rng`); 5.7, point 5 (outcome carries the endpoint) | A18 |
| C30, points 1 and 2: one socket per family, one `rx` thread, no cache lock | 5.12, points 1 and 3 | review of `ironafs-net` |
| C31, rate-limit row, and point 2: epoch moves before work is dropped | 5.10, points 4 to 6 | A12, A13, A14 |
| C32, point 5: checked at serve time | 5.10, point 6, and 5.12, point 5: a generation the serve path reads | A14 |
| C39, point 3: queues bounded in bytes | 5.2; 5.10, point 2; 5.11, "The event queue"; 5.7, point 14 (`rx.in.reply_deadline`) | A11; A16; A22; A23; A26 |
| N5, point 2: aborts are hints | 5.9, points 1 and 2 | A2, A6 |
| N6, point 3; N45, points 1 to 4: key exhaustion is its own outcome; no transparent resend | 5.8, points 6 and 7; 5.7, point 13 | A19, A20 |
| N7, point 5: no packet leaves on a connection not permitted | 5.12, point 4 (the permit generation, raised on a fence, a close and a credential change); 5.9, point 3 | A8, A21 |
| ADR 0006 D1.1 and D1.5: call-level ABORT matches fully, ends one call, no transport resend | 5.9, point 1; 5.12, point 4 (a call's queued sends end with it) | A1, A2, A3, A4 |
| ADR 0006 D1.2: call-0 ABORT is a hint | 5.9, point 2 | A6 |
| ADR 0006 D1.3: CHALLENGE rules | 5.9, point 3 | A8 |
| ADR 0006 D1.4: unmatched dropped, bounded, counted | 5.4; 5.9, point 4 | A1, A10; fuzz target `ironafs-rx_machine` |
| ADR 0006 D1.6 and D1.9: trust lowered only on eligible connections | 5.9, points 1 and 2 (`eligible` flag); 5.11, "The event queue", point 1 (outgoing ABORT events in reserved slots, never lost) | A3; A26 |
| ADR 0006 D1.7: BUSY | 5.9, point 7 | A7 |
| ADR 0006 D1.8, D2.3: incoming ABORT and BUSY | 5.9, points 6 and 8; 5.12, point 4 | A15 |
| ADR 0006 D2.1: identity, reservation, bound, redraws | 5.6, points 1 and 2 | A9 |
| ADR 0006 D2.2: ICMP and socket errors are hints | 5.9, point 9 | A16 |
| ADR 0006 D2.4: keepalive identity | 5.6, point 3 | A18 |
| ADR 0006 D2.5: ABORT as "an error" | the `eligible` flag on every ABORT event (5.9, points 1 and 2); the hold is the user's | A3 (Rx's part) |

ADR 0006's specification items: item 1 is 5.4, 5.6, point 6, and 5.9,
points 3 and 10; item 2 is 5.4, point 10; item 3 is 5.9, point 7; item 4
is 5.6, point 3; item 5 is 5.6, point 3; item 6 is 5.9, point 3; item 7
is 5.9, point 4, and 5.11; item 8 is 5.6, points 1 and 2, and 5.7, point
13; item 9 is 5.10, points 8 and 9, with the reply's deadline in 5.7,
point 14; item 10 is 5.9, point 9, and section
13, Q6. Item 11 belongs to the lock service's specification.

## 10. Tests

All paths are in the `ironafs-rx` crate unless a test names another,
laid out as coding standards rules 15 and 20 set out. Behaviour tests run
the machine in the simulator (ADR 0004, C3, point 6) with a seeded `Rng`
and clock; each has a positive control, so it fails if the machine drops
everything. Values are IronAFS's own `[OWN]`; none is taken from X1.

### 10.1 Golden vectors

In `crates/ironafs-rx/tests/it/golden.rs`. Each expected byte string is
derived by hand from 5.1 and 5.5, with the derivation as a comment beside
the vector. Each is checked both ways: the machine's encoder produces
exactly the bytes, and the decoder reads exactly the fields. Bytes are
hexadecimal, grouped by four. The common header fields: epoch word
`2b1c9d07` (epoch 0x2b1c9d07, ignore-source clear); connection id
0x03ce9710 on channel 2, so the connection word is
`(0x03ce9710 << 2) | 2` = `0f3a5c42`; service id 0x1234.

| Id | Packet | Expected bytes | Proves |
|---|---|---|---|
| G1 | DATA, initiator, call 1, sequence 1, serial 1, flags client-initiated and last-packet (0x05), index 0, payload `00000007` | `2b1c9d07 0f3a5c42 00000001 00000001 00000001 01050000 00001234 00000007` | the header layout, the direction flag, the null class's unchanged payload and zero security-specific field |
| G2 | ABORT, acceptor, call 5, sequence 0, serial 9, flags 0, code 1061 | `2b1c9d07 0f3a5c42 00000005 00000000 00000009 04000000 00001234 00000425` | the type, the cleared direction, the 4-byte code |
| G3 | BUSY, acceptor, call 6, serial 10 | `2b1c9d07 0f3a5c42 00000006 00000000 0000000a 03000000 00001234` | no payload |
| G4 | ACK from IronAFS as initiator, call 1, serial 2, flags client-initiated and slow-start-ok (0x21); first packet 2, previous packet 3, serial field 7, reason requested, N = 2, SACK `01 01`, reserved 0, trailers 1444, 1444, 32, 1 | header `2b1c9d07 0f3a5c42 00000001 00000000 00000002 02210000 00001234`, then payload `00000000 00000002 00000003 00000007 01020101 000000` + `000005a4 000005a4 00000020 00000001` | the plain ACK, its unaligned trailers at offset 21 + N, and IronAFS's advertised values (5.5, point 4) |
| G5 | Decoding an extended ACK from the acceptor: flags extended-SACK (0x08), first packet 10, previous packet 309, N = 255, then 255 SACK octets, a 256th octet in the first reserved place, trailer count 4, extra-SACK count 0, trailers 1200, 1200, 64, 1 | the payload is 18 + 255 + 3 + 16 = 292 bytes; the trailers start at payload offset 276 | trailers found at 21 + N in the extended format (5.4, point 12); the 256th octet, the trailer count and the extra tables never read |
| G6 | Decoding an ACK with N = 2 and only the first two trailers | the payload is 18 + 2 + 3 + 8 = 31 bytes | absent trailers take their defaults (5.1, point 6); a cut-short field is absent |

### 10.2 Round-trip properties

In `crates/ironafs-rx/tests/it/roundtrip.rs`, with the project's own
seeded generator (ADR 0004, O47, point 2):

- P1: every header the encoder accepts decodes to the same fields.
- P2: every ACK the encoder builds decodes to the same fields, trailers
  included.
- P3: every datagram the decoder accepts as one of the types IronAFS
  sends, and that is in the form 5.5 produces, re-encodes to the same
  bytes. That form is: header fields as 5.5, point 1, sets them; for an
  ACK, the plain format with buffer space, maximum skew and the reserved
  octets 0, all four trailers present (or, for the reachability PING,
  none and no reserved octets) and nothing after them. Any other accepted
  datagram (an extended ACK, absent trailers, ignored fields 5.4 reads
  but does not use) re-encodes to that form, and decoding the result
  gives the same fields the first decode used.

### 10.3 Reject corpus

In `crates/ironafs-rx/tests/it/reject.rs`. Each input is dropped with the
named counter and no other change:

| Id | Input | Counter | Breaks |
|---|---|---|---|
| R1 | 27 bytes | `too_short` | 5.4, point 1 |
| R2 | 1445 bytes | `too_long` | 5.4, point 1 |
| R3 | type 0, type 14, type 255 | `unknown_type` | 5.4, point 4 |
| R4 | types 8 to 13 | `unused_type` | 5.4, point 4 |
| R5 | DATA, ACK, BUSY, ACKALL with call number 0; CHALLENGE and RESPONSE with call number 1 | `bad_context` | 5.4, point 5 |
| R6 | DATA with the jumbo flag | `jumbo` | 5.4, point 6 |
| R7 | an IPv4-mapped source on the IPv6 socket; source port 0 | `mapped_source`, `bad_source` | 5.4, point 3 |
| R8 | a valid ACK for a live call, cut to 17 bytes of payload; with N = 10 and 27 bytes of payload | `ack_short` | 5.4, point 12 |
| R9 | ABORT payload of 0, 3, 5 and 8 bytes on a live call | `abort_length` | 5.4, point 13 |
| R10 | BUSY and ACKALL with 1 byte of payload | `busy_length`, `ackall_length` | 5.4, point 14 |
| R11 | DATA with sequence 0 on a live call | `bad_sequence` | 5.7, point 8 |
| R12 | an ACK acknowledging a sequence never sent | `ack_invalid` | 5.7, point 6 |
| R13 | an incoming DATA with call number 2³¹, and with 0 | `stale_call`, `bad_context` | 5.6, point 3 |
| R14 | an empty DATA payload without the last-packet flag | `empty_data` | 5.4, point 15 |
| R15 | a DATA on a live call whose fixture class's open returns 1417 bytes | `data_too_long` | 5.4, point 15 |

### 10.4 Fuzz targets

In `fuzz/fuzz_targets/`, named under coding standards rule 20; where
`fuzz/` sits rests on ADR 0004's assumption A12.

1. `ironafs-rx_packet`: arbitrary bytes as a datagram to the decoder.
   Asserts: no panic or overflow; a datagram is either dropped with
   exactly one counter or yields fields that the encoder turns back into
   the same bytes, as P3 allows; the same input always gives the same
   result.
2. `ironafs-rx_machine`: a machine with live outgoing and incoming
   calls, fed arbitrary datagrams interleaved with mutated copies of the
   genuine packets of the run, and arbitrary time steps (ADR 0006,
   test 12). Asserts: no panic; memory charged stays within
   `rx.in.total_bytes` plus the outgoing calls' bounded buffers; a call
   ends by `Aborted` only on a packet that matches its full call lookup
   on an eligible connection; a mutated copy whose identity field matches
   no live call or connection leaves every outcome equal to the run
   without it, except counters; nothing is sent in reply to a packet that
   matches no connection and opens none (5.10, point 3); no DATA or PING
   of a call leaves after the call ended (5.12, point 4).

### 10.5 Acceptance cases

In `crates/ironafs-rx/tests/it/` (one module per subject) and, where
the case involves the callback service, in that service's tests. Each
case: start state, events in order, result. "Matching" means the full
lookup of the section cited. ADR 0006's tests 1 to 13 are carried by the
cases named, except test 3's hold and its exit (ADR 0006 D2.5), which
the callback service's tests carry (#66); A3 carries Rx's part of test
3. Test 14 is the test cell's (section 13, Q1).

- **A1 Guessed identities** (ADR 0006, test 1). Live outgoing calls; an
  attacker knowing peer address, client port, start time and earlier
  identities sends N ABORTs with time-derived epochs and counter
  connection ids on all channels, within one source. No call ends; all N
  are counted for that source; nothing is sent. Then an ABORT with the
  true identity ends its call.
- **A2 One field wrong** (test 2). For each field of the call lookup
  (local endpoint, peer address, peer port, direction, epoch, connection
  id, channel) and a call number one below and one above a live call's
  (at least 2), the ABORT is dropped. With all fields right, on a call
  whose user set no retry: that call fails `Aborted`, others unchanged. A
  differing service id, and separately security index, with the rest
  right: the call still ends and the diagnostic count rises. On both
  families and on a NAT64-synthesised peer address.
- **A3 Blast radius and eligibility** (test 3, Rx's part). After a
  matching ABORT on one call: other calls on the connection continue,
  `AbortReceived` carries `eligible = true`. On a fixture connection
  given a predictable identity (not drawn by 5.6, point 1): a matching
  call-level or call-0 ABORT changes only counters, the call runs on,
  and the events carry `eligible = false`.
- **A4 Abort on a store** (test 4, Rx's part). A call that has sent
  request DATA gets a matching ABORT: outcome `Aborted` with
  `sent_any = true`, and the machine sends no further request DATA of
  that call on any connection; the count of calls started equals the
  user's starts. The same with a request DATA retransmission already
  queued earlier in the same read batch as the ABORT, and again with one
  queued before the call ends by `Timeout`: the queued packet never
  reaches the socket (5.12, point 4).
- **A5 Code as bait** (test 5, Rx's part). A matching ABORT with any code
  is delivered with the code unchanged and nothing else in Rx changes.
- **A6 Connection level** (test 6). A matching call-0 ABORT: counted,
  `AbortHint` handed, the connection is not closed, fenced or re-keyed;
  a live call whose valid reply arrives after it completes at once; a
  call with no reply ends at its original deadline, not earlier; the
  next call is served on the same connection. A non-matching one is dropped and
  counted. On a channel of any value, the matching one is accepted.
- **A7 BUSY** (test 7). Before acceptance it defers the retransmission;
  after an ACK of the call it is dropped, and separately after a reply
  DATA of the call, and after an ACKALL of the call (each event that
  closes the window, 5.9, point 7); a BUSY stream stops deferring at
  `rx.busy_defer_max`, the call keeps retransmitting and never fails
  before its deadline. With a 60 s deadline the deferral stops at
  7.75 s (0.25 + 0.5 + 1 + 2 + 4; the next 4 s would pass 10 s); with a
  12 s deadline it stops at 3.75 s, since the next 4 s would pass half
  the call's time, 6 s.
- **A8 CHALLENGE** (test 8). A matching CHALLENGE on a fixture class
  draws one RESPONSE; a repeated one is answered within the rate limit;
  a peer restart recovers the handshake; a fence, and separately a
  credential change, between `ChallengeReceived` and the send, each
  raising the permit generation (5.12, point 4), means no RESPONSE
  leaves, checked at the driver seam: once with the change before the
  user's answer, once between the answer and the send; no identity
  match, or a right epoch and connection id with a wrong peer address,
  port, local endpoint or direction: no RESPONSE; call number 1: no
  RESPONSE. On the null class: no RESPONSE ever, `challenge_refused`
  counted, and no `ChallengeReceived` event.
- **A9 Identity generation** (test 9). Over many seeded connections each
  epoch and connection id is the `Rng`'s output except where redrawn,
  and the ignore-source flag is clear. With `rx.identity_redraws` at
  least six (five unusable draws, then a usable one) and a fixture `Rng`
  returning a live identity, a live connection id under another epoch, a
  0 epoch, a 0 connection id, then a reserved pair inside R: each is
  redrawn; past R the same pair is
  accepted; a retired connection id under a new epoch inside R is
  accepted.
  - Delayed packets: on the same local endpoint, a packet for a retired
    identity inside R is dropped as `unmatched`. Across a rebinding of
    the socket (5.12, point 7), a delayed packet for a connection of the
    old generation is dropped, and the test records that drop as an
    endpoint mismatch, not as evidence for R.
  - Restart: the first epoch and connection id are the fixture `Rng`'s
    next outputs, not the retired values, and delayed packets for the
    retired identities are dropped. A repeat by collision is the stated
    residual and is not tested as a drop.
  - Refusals: (a) with capacity available and a fixture `Rng` returning
    only unusable values, the open fails with `IdentityExhausted` after
    `rx.identity_redraws` draws; (b) with live connections plus
    reservations at `rx.identity_bound` and a usable draw, the open fails
    with `IdentityExhausted`; (c) with a bound of B, B − 2 reserved pairs
    and one live connection, a second connection is admitted on a usable
    draw (the last slot) and a third request waits; while it waits, both
    live connections retire inside R and are both reserved; it stays
    unadmitted through both retirements and fails at its deadline. In (b)
    and (c) the waiting request's deadline falls before the earliest
    reservation expires, so no capacity frees while it waits. In each
    refusal, live calls are unaffected and no reservation is released
    early.
  - An incoming connection with the same epoch and connection id as a
    live outgoing one reaches its own call, and the outgoing connection
    is unaffected.
- **A10 Flood** (test 11). Non-matching packets from many sources at a
  high rate while calls run: the calls keep their latency bound in
  simulated time, memory stays within the fixed table, and `DropCounts`
  is emitted at most once per interval. The latency bound: on a
  simulated link with no loss, each call completes with the same outcome
  as in the same seeded run without the flood, and no later than
  `rx.rto_initial` after its completion time in that run.
- **A11 Caps per peer and in total.** One peer opens connections and
  calls until `rx.in.peer_bytes` and `rx.in.conns_per_peer_max`; the
  next is an overload naming that peer. Many peers together reach
  `rx.in.total_bytes` less `rx.in.held_bytes` (7 864 320 bytes, 5.10,
  point 2); the next is an overload. Charges are released
  when calls complete and connections retire.
- **A12 Hold until confirmed.** At a cap, the datagram is held and an
  `Overload` event handed; until the user confirms it, the held datagram
  is neither processed, acknowledged nor answered; on confirmation it is
  dropped and counted, its charge released. With `rx.in.overloads_max`
  events pending, the next overload joins one "unknown" event. With the
  other charges at `rx.in.total_bytes` less `rx.in.held_bytes`, an
  overload datagram is still held and charged to the total.
- **A13 Pause and resume.** The held allowance fills: the machine asks
  for a pause; the driver raises the socket's pause generation, then
  stops reading; the generation is odd. A thread that never handles the
  `Pause` event still sees the raised generation before the first
  datagram the driver did not read. Confirmations free the allowance;
  the driver raises the generation again before its first read; it is
  even and two above where it started. Timers and sends run during the
  pause.
- **A14 The fence order against the serve path** (with the callback
  service's tests). An entry validated before a pause is suspect when
  served after the pause generation was raised, with no event handled
  in between; the same after the resume raise.
- **A15 Incoming ABORT and BUSY** (test 10). A complete incoming request
  then a matching ABORT, in both orders against the reply: before the
  reply is submitted no reply is sent; after, no further retransmission,
  including a reply DATA already queued earlier in the same read batch
  as the ABORT (5.12, point 4). The request was given to the service in
  both; the call then ends and its charge is released. An ABORT between
  packets of a multi-packet request: the request still completes within
  `rx.in.request_deadline` and is given to the service. A BUSY on an
  incoming call: a counter only.
- **A16 ICMP and socket errors** (test 13). Attributed and unattributed
  errors on both families alongside traffic: a call whose reply arrives
  completes; a new call is served; a call with no reply ends at its
  original deadline; the socket stays open and the local endpoint
  generation is unchanged. No action or event Rx hands out changes the
  network generation (C23, point 2), which the simulator's model of the
  user holds and which is asserted unchanged. An error stream at packet
  rate from one peer, and from more peers than `rx.icmp_table_entries`,
  yields at most one `IcmpHint` per entry per `rx.icmp_hint_interval`,
  whose counts add up to the errors sent, across at least one
  `rx.drop_report_interval` boundary (5.9, point 9).
- **A17 Class reported.** Every outcome on a null-class connection
  reports index 0 and level "none".
- **A18 Keepalive identity.** A call marked keepalive has a call number
  from `Rng` in 1 to 2³⁰, on a channel that has carried no call, on a
  connection drawn by 5.6, point 1; its outcome carries the receiving
  local endpoint; a reply with the right epoch and connection id but
  another call number does not complete it. A keepalive start on a
  connection whose four channels have all carried a call is refused at
  once with `StartRefused` and sends nothing, both when all four are
  free and when some carry live calls; on a new connection it starts on
  a channel with no previous call.
- **A19 Key exhausted.** A fixture class returns `KeyExhausted` on a
  connection with one call that has sent DATA and one that has not: both
  fail `KeyExhausted`, with `sent_any` true and false; nothing more is
  sent on the connection, and neither is a transport failure.
- **A20 Rekey request.** A fixture class asks for a rekey: the user gets
  `RekeyRequested` and nothing else changes; after the user installs a
  new class object, unsent requests go out under it.
- **A21 Permit generation.** Queued sends, retransmissions and an
  approved RESPONSE on a connection are discarded at the driver seam once
  the user raises the connection's permit generation; none reaches the
  socket.
- **A22 Charges fit.** The test asserts that the incoming connection and
  call records' sizes are within `rx.in.conn_charge` and
  `rx.in.call_charge` on each target CI builds.
- **A23 Unverified peer.** An incoming call whose reply is larger than
  twice the request: the machine sends a PING, at most
  `rx.verify_pings_max` times; no reply DATA beyond the budget leaves;
  after a PING-RESPONSE with the PING's serial, the reply is sent. Each
  PING is 46 bytes, with no trailers (5.5, point 4); one is sent after a
  single 28-byte request. With no PING-RESPONSE, the reply never leaves
  and the call fails at `rx.in.reply_deadline` (`reply_expired`), its
  charge released. Total bytes sent before verification never exceed
  twice those received. A verified peer that never acknowledges the
  reply: retransmissions stop and the call fails at
  `rx.in.reply_deadline`, its charge released.
- **A24 Retransmission and flow control.** A lossy simulated link: lost
  packets are retransmitted with new serials and the same sequence;
  packets in flight never exceed the windows; an ACK with a first
  packet that goes backwards is ignored; the timeout doubles on
  consecutive losses up to `rx.rto_max`.
- **A25 Idle timeout and keepalive.** With a silent peer, PINGs go out
  every T ÷ 6 and the call fails `Timeout` at T; with a peer that answers
  PINGs but never replies, the call fails `DeadlineExceeded` at its
  deadline.
- **A26 Reply caps and the event queue.** Registering a service with a
  reply cap above `rx.in.reply_ceiling`, or a request cap above
  `rx.in.request_ceiling`, is refused. A reply above the service's reply
  cap, and a reply whose charge does not fit `rx.in.peer_bytes` or
  the total as 5.10, point 2, says, is refused at submission
  (`reply_refused`), sends no reply DATA, and is not an `Overload`; a
  reply submitted after its call ended is refused too. With the service
  at both ceilings, one peer's call with a 65 536-byte request and a
  65 536-byte reply completes. A reply that fits is charged to the peer
  and the total and released when acknowledged.
  The event queue, with the user taking no event: (a) an admitted peer
  cycles call numbers on one incoming connection, each call opened by a
  complete one-packet request, which a fixture service accepts and
  answers with nothing, and ended by a matching ABORT, for 10 000
  calls: the queue holds one `AbortReceived` for that connection, its
  repeat count 9 999; (b) streams of matching call-0 ABORTs and accepted
  BUSYs on one outgoing connection leave one event of each kind whose
  repeat count equals the packets less one, keeping the first and last
  codes; (c) many connections and peers making events until the queue
  is full: the queue's bytes outside the reserved slots never pass
  `rx.event_queue_bytes`, the excess is counted in `EventsDropped` by
  kind, a later `Overload` is still queued in its slot, the socket's
  pause slot shows the latest of `Pause` and `Resume` with the number of
  each, a matching call-0 ABORT and a matching call-level ABORT on an
  outgoing connection are still queued as `AbortHint` and
  `AbortReceived` with `eligible`, a `DropCounts` and an `IcmpHint`
  refused for room zero no count and are counted as deferred in
  `EventsDropped`, and a `ChallengeReceived` not queued sends no
  RESPONSE. Then the user takes the queued events, which frees room:
  the next `DropCounts` and `IcmpHint` are queued, and with the earlier
  ones their counts add up to the packets and errors sent. Every call
  outcome in the run is returned with its call.
  (d) With the user never taking events and outgoing connections
  retiring with an ABORT event each: their identities stay reserved,
  and opens fail with `IdentityExhausted` once `rx.identity_bound` is
  reached; after the user takes the events, the reservations run out
  their R and opens succeed again.

### 10.6 Gaps

- The behaviour of real peers: every wire fact here is UNVERIFIED until
  the capture-backed revision (section 13, Q1).
- ADR 0006's test 14 is the test cell's, not this crate's.
- A13 and A14 need the driver and the callback service; until both
  exist they run against the simulator's driver model.

## 11. Revisit if

From the working decisions this rests on:

- ADR 0004, C3: "the M2a simulator cannot replay a failing schedule from
  its seed, or a measured workload needs more concurrent waits than a
  bounded pool of threads can hold."
- ADR 0004, C4: "a boundary below forces a dependency that point 4
  forbids, or the first implementation of a crate shows it has no content
  of its own."
- ADR 0004, C30: "the M2 tests show a NAT or firewall that maps the
  callback port differently from the calling port, or one `rx` thread
  limits measured throughput."
- ADR 0004, C31: "a fileserver offers rxgk-protected callbacks to
  clients (then a protected break is authoritative evidence under C24),
  or the test cell shows a callback procedure this table lacks."
- ADR 0004, C32: "M2 measurements against the test cell and through NAT
  show the intervals below serve stale data or cost too many
  revalidations."
- ADR 0004, C39: "M2 load tests show the fixed set starves a class of
  work, or lock-order checks find an RPC wait under a lock that cannot be
  restructured."
- ADR 0004, N45: "the M2b rxgk profile record (N25) shows that rekeying
  must be visible above the security-class interface."
- ADR 0006, decision 2, point 1: "a supported server mishandles a random
  epoch or connection id, or relies on connection ids unique beyond their
  connection's life."
- ADR 0006, decision 2, point 2: "M2 measurements show failover too slow
  because of it."
- ADR 0006, decision 2, point 3: "the test cell shows genuine
  incoming-call ABORT or BUSY traffic that this point handles wrongly."
- ADR 0006, decision 2, point 4: "a supported server rejects the
  keepalive's identity (its random epoch, connection id or call number)".
- ADR 0006, decision 2, point 5: "C32, point 3, is revisited, or the test
  of assumption A14 settles how a server reports a lost callback."
- ADR 0006, decision 1 (Firm; a superseding ADR): its three triggers,
  including test 14 showing peers' service id or security index on
  control packets differ from what this specification records.

This specification's own choices:

- a capture shows a genuine peer sending something 5.4 drops: an ABORT
  payload other than 4 bytes, a BUSY or ACKALL with a payload, a
  datagram above 1444 bytes, a jumbogram, an IPv4-mapped source, or a
  CHALLENGE or call-0 ABORT with a call number or channel this
  specification does not expect;
- the test cell accepts a random first call number (5.6, point 3; ADR
  0006, item 4);
- a peer needs epoch 0 or connection id 0 (5.6, point 1);
- a genuine callback arrives with a security index other than 0, or a
  service id the callback service does not register (5.10, point 1);
- a measured workload is limited by `rx.recv_window`, the absence of
  jumbograms, or the fixed packet size;
- a genuine server keeps sending on a call longer than `rx.peer_linger`
  after IronAFS ended it (section 13, Q3);
- the callback service's tests show the caps of 5.2 drop genuine breaks
  in normal operation;
- X1 is revised (register row E3).

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| X1 | SPECIFICATION | "Rx protocol specification draft", Nickolai Zeldovich, updated by Jeffrey Altman, 24 July 2026, register of editions row E3 ([README](README.md)), AFS Assigned Numbers Registry. Cited by heading: "Introduction", "Rx Connections", "Packet Types", "Call Layer" (with its parts "Round-trip time computation", "Packet retransmission", "Keepalive and Timeout" and "Flow Control"), "Jumbograms", "RPC Layer", "Packet Formats and Protocol Constants". Its section "Historical Implementation Notes" was not read and is not cited; it is excluded by name (README, "Sources and provenance", rule 12). The "Debugging" section's references to another implementation's files and structures, and the "References" entries that point at an implementation's source, are not used. A lead only (rule 14): every fact from it is UNVERIFIED. Also claim record rows `[DRV-084]`, `[DES-056]`, `[ADR-002]` and `[ADR-006]` ([claim record](../reviews/2026-11-20-adr-0004-claims.md)), re-read against this edition. | 3, 4, 5, 6, 10 |
| X2 | SPECIFICATION | RFC 4291, "IP Version 6 Addressing Architecture", R. Hinden and S. Deering, February 2006, register of editions row E4 ([README](README.md)), <https://www.rfc-editor.org/rfc/rfc4291>: §2.5.5.2, "IPv4-Mapped IPv6 Address" | 5.4 |
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; C3, C4, C5, C6, C19, C20, C24, C28, C30, C31, C32, C34, C37, C39, C46 ([part 1](../adr/0004-architecture-1-core.md)); O1, O12, O15, O23, O47 ([part 3](../adr/0004-architecture-3-operations.md)); N5, N6, N7, N31, N45 ([part 4](../adr/0004-architecture-4-negotiation.md)); A12, A14 | 1, 2, 5, 8, 9, 11 |
| D2 | DECISION | [ADR 0006](../adr/0006-rx-unprotected-packets.md), accepted 2026-11-21: decisions 1 and 2, "Specification items for the Rx specification" and its tests 1 to 14 | 1, 3, 5.6, 5.9, 5.10, 8, 9, 10, 11 |
| D3 | DECISION | [ADR 0001](../adr/0001-licence-and-dont-copy.md), "Don't copy" and "Test tools and test data" | 10, 12 |
| D4 | DECISION | [Coding standards](../coding-standards.md), rules 1 to 3, 7, 8, 15 and 20 | 5.2, 5.4, 6, 7, 10 |
| D5 | DECISION | [xdr.md](xdr.md): the codec the RPC layer runs over Rx's delivered bytes | 2 |
| O | OWN | Every point tagged `[OWN]`, with its reason in place | 5, 6, 7 |

No other implementation's source, test data or interface definitions
were read for this specification, and none are cited. It describes Rx in
IronAFS's own words and quotes no text of X1, which carries no licence
notice (register row E3). The field names and values in 5.1 are
functional facts. The test values of section 10 are IronAFS's own.

### 12.1 Observed behaviour

None yet. This version is written from documents only. The
capture-backed revision records the observation lab's stage A captures
here (section 13, Q1).

## 13. Open questions

- **Q1. Capture-backed revision.** Every wire fact in 5.1 and the peer
  behaviour in 5.4, 5.7 and 5.9 is UNVERIFIED until the observation lab
  (#51's stage A record, the black-box reference client on the capture
  matrix's baseline) produces fixtures, and ADR 0006's test 14 runs at
  the test cell. The second version of this specification records them
  in 12.1, relabels each fact, and adds reduced fixtures under
  `crates/ironafs-rx/tests/fixtures/` with their provenance. Blocks: the
  closing of #55.
- **Q2. Random first call number** for calls other than the keepalive
  (ADR 0006, item 4): 1 until the test cell shows peers accept a random
  start; then drawn from `Rng` in 1 to 2³⁰.
- **Q3. How long a peer keeps sending** after the call ends on IronAFS's
  side (`rx.peer_linger`, 120 s): no source states a peer's idle
  timeout. It sets R. The test cell measures it; R is raised if needed.
- **Q4. Reserved values and connection-level channels.** X1 names no
  reserved epoch or connection id, and states no channel value for a
  CHALLENGE or a call-0 ABORT. IronAFS avoids 0 for both identity fields
  and checks no channel on connection-level packets; captures settle
  both (ADR 0006's fact table rows on reserved values and on the
  CHALLENGE's call number and channel). The same captures show whether a
  genuine peer sets the ignore-source bit on packets it sends to the
  client: the common connection lookup compares the whole epoch word
  (5.6, point 6), so a peer that did would have its packets dropped as
  `unmatched`, and this point would change.
- **Q5. Rx's own abort codes.** MISSING: X1 names a protocol-error abort
  but gives no code values for it or for Rx's other failures (call dead,
  timeout). The error registry the claim record cites as `[DES-055]`
  says Rx's codes are negative, which is a lead only. Until a registered
  edition or a fixture gives the values, Rx sends no ABORT for its own
  failures (5.7, point 15). Blocks: nothing in M2a; the peer times out.
- **Q6. Operating-system behaviour on UDP sockets** (ADR 0006, item 10):
  how Linux, macOS and Windows report ICMP errors and asynchronous
  errors on a UDP socket (and whether a receive error can stop the
  loop), how the IPv6 socket is made IPv6-only, and whether the kernel's
  receive-buffer drop count can be read (which would let Rx report
  kernel drops during a flood as an overload). MISSING: each system's
  own documentation is the source, under the driver's specification and
  ADR 0005. Blocks: the driver, not the machine.
- **Q7. Strength of the reachability test.** X1 starts serials at 1, so
  an off-path sender can complete the test for a connection it forged
  (5.10, point 7). Whether a random first serial on incoming connections
  interoperates is a test-cell question; if it does, this specification
  adopts it.
- **Q8. User status.** No IronAFS service defines the user status field;
  it is ignored on receipt and sent as 0. A service specification that
  needs it amends 5.4, point 11.

## 14. Review

Reviewed by GLM and DeepSeek in three rounds on 2026-11-21, and by GLM
in a fourth check of the third round's fixes; the record is
[2026-11-21-m2a-55-rx](../reviews/2026-11-21-m2a-55-rx.md).
