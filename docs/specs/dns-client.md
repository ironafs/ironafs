<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# First-party DNS client for Linux and Unix builds

Status: draft (2026-11-21). Raised in issue #60.

Kind: protocol. Class: S. The client decodes replies from DNS servers
IronAFS does not control, and anyone who can forge a datagram on the path
can try to feed it bytes; its answers also decide where a cell's name is
sent ([README](README.md), "Review class per kind"). It also fixes
behaviour (which servers are asked, what is cached, when a name is "not a
cell"). That part is behaviour of class S under the same table, so the
specification takes the kind `protocol` and says so ([README](README.md),
"Kinds").

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (S1, M1, D1
and so on) are in section 12.

## 1. What this implements

ADR 0004, overview, constraint 4 (a constraint of the whole ADR; it
changes only with the ADR):

> **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
> local IPC, configuration, requests from the operating system, and SMB
> messages before authentication. No panics, checked arithmetic and byte
> limits apply to all of it (coding standards rules 1 to 3).

ADR 0004, C3, point 1 (**Working**):

> Rx, XDR, the RPC codecs and the security classes are pure state
> machines: bytes, time and randomness in, actions out. They open no
> socket, read no clock and draw no random numbers.

ADR 0004, C4, point 6 (**Working**):

> Where the first-party DNS client of C33 lives is settled with its
> specification; we propose a pure codec beside `ironafs-proto` and its
> socket in `ironafs-net`.

ADR 0004, C6, point 1 (**Firm**):

> Each protocol component is written from a behaviour specification in
> `docs/specs/` that names its sources, under ADR 0001. No specification,
> no code.

ADR 0004, C12, point 3 (**Working**):

> A lookup of a name that is not a cell is answered from a negative
> cache, so an indexer or a mistyped path does not cause a DNS query each
> time. This cache is machine-wide and part of cell discovery (C33), not
> a cache entry of any context: it holds only that a name is not a cell.
> It is filled only from a conclusive negative DNS answer, kept no longer
> than that answer's time to live, never filled from a timeout or an
> error, and cleared on a network change (C33, point 6).

ADR 0004, C33, points 1, 3 to 6 and 8 (**Working**; point 2 is the
macOS and Windows route, ADR 0005's, and point 7 is the location cache's):

> 1. **Order.** Configuration first, then DNS SRV, then AFSDB
>    `[DES-008]` `[DES-010]`. Configuration always wins. SRV priority comes
>    before every other ranking input, and a sole SRV target of "." means
>    the service is not offered `[DES-009]`.
>
> 3. **Linux and other Unix builds** use one first-party DNS client, the
>    same for glibc and musl builds. It queries only the host's own
>    established local resolver, for example systemd-resolved's stub
>    `[DES-002]`, the dnsmasq that NetworkManager can run as a local caching
>    name server
>    ([NetworkManager.conf](https://networkmanager.dev/docs/api/latest/NetworkManager.conf.html)),
>    or RHEL 10's dnsconfd, which Red Hat's encrypted-DNS documentation
>    describes as a local DNS cache and split-DNS daemon `[DES-007]`. The client detects the resolver at run
>    time and never requires a particular one.
> 4. **On a flat list of name servers** with no local resolver, discovery
>    runs only after an administrator confirms that list is the intended
>    DNS policy. The listed servers are then asked in order, with documented
>    timeouts. The confirmation is rechecked when the network or VPN policy
>    changes.
> 5. **Otherwise** the client uses only explicitly configured servers. For
>    a cell reachable only through a VPN, the cell's name is not sent unless
>    a managed route keeps the query inside the intended scope.
> 6. Cached records and the rankings derived from them expire together, are
>    dropped on a network change, and late replies from an earlier network
>    state are discarded.
>
> 8. Discovery can be turned off globally and per cell. Status reports
>    where each answer came from and, separately, what the resolver said
>    about validation.

ADR 0004, C34, point 3 (**Working**):

> Database servers found through configuration or DNS are tried over
> IPv6 when they answer there, with a clean fallback to IPv4 and a record
> per server of what works.

ADR 0004, C37, point 1 (**Working**), the first two clocks:

> - a **boot clock that counts suspend,** for every deadline a server
>   grants: callbacks, tokens, location lifetimes, locks;
> - an **awake clock that excludes suspend,** for round-trip and
>   retransmission timers;

ADR 0004, N5, points 2 and 3 (**Firm**):

> 2. Everything else is a **hint**: DNS answers, rxnull replies, rxkad
>    replies, Rx aborts, timeouts, capability words read without that
>    protection, and anything `aklog` reports.
> 3. A hint may lower trust, or choose between RPC variants the catalogue
>    lists as equivalent (N20). A hint never raises or satisfies a floor,
>    never learns or renews a pin, and never authorises an exception.

ADR 0004, O17, point 2 (**Working**):

> A value set by policy locks its key. No lower layer and no run-time
> override changes it.

ADR 0004, O47, point 3 (**Working**), the part that applies:

> **Fuzzing** covers every decoder: […] DNS replies; […]

ADR 0005, Decision, point 6 (**Working**):

> **DNS on Linux and other Unix builds is not in this crate** (C33,
> point 3; C4, point 6). One first-party DNS client in safe Rust, the same
> for glibc and musl builds, queries only the host's own established local
> resolver, detected at run time, through `std` UDP and TCP sockets; on a
> flat list of name servers only after an administrator confirms it as the
> intended policy (C33, point 4, and the overview's open question 1);
> otherwise explicitly configured servers (C33, point 5). Where a query
> goes stays the host resolver's decision. It needs no site.

Section 9 shows where each is kept and which test fails if it is broken.

## 2. Scope

1. **Covered:**
   - where the client lives (5.6.1);
   - how it reads the host's resolver configuration and decides whether
     the host has an established local resolver (5.6.2);
   - which servers a query may go to, the flat-list confirmation, the
     explicit-servers fallback and the scope rule for VPN-only cells
     (5.6.3), and the configuration keys that control them (5.6.4);
   - the query and reply messages it sends and accepts: SRV, AFSDB, A and
     AAAA, with CNAME, SOA and OPT records as they appear in replies (5.1
     to 5.5);
   - one query's transport: UDP, TCP, truncation, EDNS(0), timeouts,
     query identifiers, source ports and letter case (5.6.5);
   - a lookup of a cell's volume location or protection servers, from
     SRV to AFSDB to addresses, and its outcomes (5.6.6);
   - expiry, network and resolver changes, late replies, the positive
     cache, the negative cache of C12, point 3, and the failure hold-down
     (5.6.7);
   - status and events (5.6.8), and bounds on concurrent work (5.6.9).
2. **Not covered, and where it is covered:**
   - discovery on macOS and Windows: ADR 0005, Decision 3, and the
     macOS and Windows parts of `ironafs-os`;
   - ranking servers within a priority (weights, round-trip time,
     administrator preference, security profile), and the location cache
     of volume entries: the location service's specification (ADR 0004,
     C33, point 7; N15). This specification hands it records in priority
     order and the lifetime every ranking must share (5.6.7, point 2);
   - the configuration parser and how a key is spelled in a file or in
     policy: `ironafs-config` (#93). This specification states what the
     keys mean and who may set them (5.6.4);
   - what counts as a network change: the network-generation source
     (#97). This specification consumes its counter;
   - the `Clock` and `Rng` interfaces (`ironafs-time`) and the production
     random source (#68);
   - how a lookup failure appears under `/afs` (an error number in each
     layer): the layer specifications;
   - the cell name rule (lower case, labels, lengths): `ironafs-config`
     (#93), which owns the one constructor of a cell name. This
     specification only adds the rules of 5.6.6, point 1;
   - the thread that runs the socket driver, and the seam through which
     operation code and the simulator reach the client: section 13, Q1
     and Q2.
3. **Not part of this client:** DNS over TLS or HTTPS, DNSSEC
   validation, multicast DNS and LLMNR, reverse lookups, the search list,
   zone transfers, and any record type other than those in 5.1, point 6.

## 3. The design in short

1. One pure crate, `ironafs-dns`, holds the codec, the parser for the
   host's resolver configuration, the query state machine and the
   discovery caches. It has bytes, time and randomness in and actions
   out. The socket and file I/O are in `ironafs-net` (5.6.1).
2. The client reads `/etc/resolv.conf`. If every listed name server is a
   loopback address, the host has a local resolver and only it is asked.
   This is how systemd-resolved, NetworkManager's dnsmasq and dnsconfd
   present themselves `[M2 "/etc/resolv.conf": CONFIRMED]`
   `[M3 "dns": CONFIRMED]`. Anything else is a flat list, used only while
   an administrator's confirmation of exactly that list holds; otherwise
   only explicitly configured servers are used, and with none, nothing is
   sent (5.6.2, 5.6.3).
3. Names are always absolute. The search list is never applied, so a
   cell's name is never sent with another domain appended
   `[S10 §4: CONFIRMED]`.
4. Queries go over UDP with EDNS(0) at a 1,232-byte payload, with a fresh
   random query identifier and a fresh random source port for every try,
   and, to a server off the host, a random letter case in the name. A
   truncated reply, or signs of forgery, move the query to TCP
   (5.6.5).
5. Every reply is decoded through a bounded cursor with a cap on every
   count and length, names are followed through compression pointers that
   must point strictly backwards, and nothing in a reply is trusted until
   the identifier, the question and the source all match (5.4).
6. A name is "not a cell" only from a lookup of its volume location
   servers, in one of two ways: SRV says the service is not offered (a
   sole target of the root), with that record's TTL as the lifetime; or
   SRV and AFSDB both give a conclusive negative answer (a matched,
   untruncated, well-formed reply with no CNAME in it, from a server
   that recursed or is authoritative), with a lifetime from the zone's
   SOA record. Lifetimes are capped; a negative without an SOA is not
   cached. A timeout, an error or a truncated reply is never a negative,
   and a protection-server lookup never makes a name "not a cell"
   (5.6.6, 5.6.7).
7. Lifetimes run on the boot clock; per-try retransmission timers on the
   awake clock. A network change or a change in the resolver
   configuration drops every record, ranking and negative entry, and
   discards every reply still in flight (5.6.7).
8. DNS answers are hints (N5): they choose where to look, and never
   raise trust.

## 4. Terms

- **Resolver configuration:** the file `/etc/resolv.conf`, read as
  resolv.conf(5) describes it `[M1: CONFIRMED]`.
- **Resolver view:** what this client takes from the resolver
  configuration: its kind (local resolver, flat list or none) and its
  ordered list of name-server addresses (5.6.2).
- **Local resolver:** a resolver view whose every listed name server has a
  loopback address: IPv4 127.0.0.0/8 or IPv6 `::1`.
- **Flat list:** a resolver view with at least one listed name server that
  is not a loopback address.
- **Confirmed flat list:** a flat list that equals, address for address
  and in order, the list an administrator confirmed (5.6.4).
- **Explicit servers:** the servers an administrator configured for
  discovery (5.6.4), used only as C33, point 5, says.
- **Server path:** which of the three a query used: local resolver,
  confirmed flat list or explicit servers, or a scoped cell's own servers
  (5.6.3).
- **Query:** one question (a name, a type, class IN) asked until it has
  an outcome, through one or more tries.
- **Try:** one message sent to one server over one transport, and the wait
  for its reply.
- **Matched reply:** a received message whose source, identifier, opcode,
  QR bit and question all match the try that is waiting (5.4, point 2).
- **Applicable record:** a record in the answer section, of class IN, of
  the type asked, owned by the query name or by the end of the CNAME chain
  (5.4, point 6). Every other record is **irrelevant**: it is walked,
  bounds-checked and skipped.
- **Conclusive negative:** a query outcome that says the name, or the
  type at the name, does not exist (5.4, point 8).
- **Lookup:** finding the servers of one service of one cell (volume
  location or protection): SRV, then AFSDB if needed, then the targets'
  addresses (5.6.6).
- **Network generation:** the counter the network-generation source (#97)
  moves on every change of the host's addresses or routes.
- **View generation:** a counter this client moves whenever the resolver
  view changes (5.6.2, point 7).
- **Generation pair:** the network generation and the view generation
  together, captured when a try is sent.
- **Hint:** as ADR 0004, N5, point 2, defines it.

## 5. Specification

### 5.1 Layout

Every multi-byte integer is unsigned and big-endian
`[S1 §2.3.2: CONFIRMED]`. Offsets count from the first byte of the
message `[S1 §4.1.4: CONFIRMED]`.

1. **A message** is a 12-byte header, then the question, answer,
   authority and additional sections, in that order
   `[S1 §4.1: CONFIRMED]`.
2. **The header:**

   | Bytes | Field | Width | Meaning |
   |---|---|---|---|
   | 0–1 | ID | 16 bits | identifier chosen by the sender of a query and copied into the reply `[S1 §4.1.1: CONFIRMED]` |
   | 2, bit 7 | QR | 1 | 0 query, 1 response `[S1 §4.1.1: CONFIRMED]` |
   | 2, bits 6–3 | Opcode | 4 | 0 is a standard query `[S1 §4.1.1: CONFIRMED]` `[S14 §2.2: CONFIRMED]` |
   | 2, bit 2 | AA | 1 | authoritative answer `[S1 §4.1.1: CONFIRMED]` |
   | 2, bit 1 | TC | 1 | truncated `[S1 §4.1.1: CONFIRMED]` |
   | 2, bit 0 | RD | 1 | recursion desired `[S1 §4.1.1: CONFIRMED]` |
   | 3, bit 7 | RA | 1 | recursion available `[S1 §4.1.1: CONFIRMED]` |
   | 3, bit 6 | Z | 1 | reserved `[S14 §2, §2.1: CONFIRMED]` |
   | 3, bit 5 | AD | 1 | authentic data `[S14 §2: CONFIRMED]` |
   | 3, bit 4 | CD | 1 | checking disabled `[S14 §2: CONFIRMED]` |
   | 3, bits 3–0 | RCODE | 4 | response code `[S1 §4.1.1: CONFIRMED]` |
   | 4–5 | QDCOUNT | 16 | entries in the question section `[S1 §4.1.1: CONFIRMED]` |
   | 6–7 | ANCOUNT | 16 | records in the answer section `[S1 §4.1.1: CONFIRMED]` |
   | 8–9 | NSCOUNT | 16 | records in the authority section `[S1 §4.1.1: CONFIRMED]` |
   | 10–11 | ARCOUNT | 16 | records in the additional section `[S1 §4.1.1: CONFIRMED]` |

   Bit 7 is the most significant bit of its byte. The RCODE values this
   client acts on: 0 no error, 1 format error, 2 server failure, 3 name
   error (NXDOMAIN), 4 not implemented, 5 refused
   `[S1 §4.1.1: CONFIRMED]`.
3. **A question** is a name, then QTYPE (16 bits), then QCLASS (16 bits),
   with no padding `[S1 §4.1.2: CONFIRMED]`. A standard query carries
   exactly one question `[S17 §4: CONFIRMED]`.
4. **A resource record** is a name (the owner), TYPE (16), CLASS (16),
   TTL (32, unsigned), RDLENGTH (16), then RDLENGTH bytes of RDATA
   `[S1 §4.1.3: CONFIRMED]`. A TTL whose top bit is set is read as 0
   `[S3 §8: CONFIRMED]`.
5. **A name** on the wire is a sequence of labels, each a length byte of
   0 to 63 followed by that many bytes, ended by a zero byte (the root);
   or such a sequence ended by a pointer; or a pointer alone. A pointer
   is two bytes whose top two bits are both 1; the other 14 bits are an
   offset from the start of the message. A first byte whose top two bits
   are 01 or 10 is neither a label nor a pointer
   `[S1 §4.1.4: CONFIRMED]`. A label is 1 to 63 bytes and a name at most
   255 bytes in its uncompressed wire form `[S1 §2.3.4: CONFIRMED]`
   `[S3 §11: CONFIRMED]`, counting every length byte and every label
   byte, the final zero byte of the root included
   `[S1 §3.1: CONFIRMED]`. A label may hold any byte
   `[S3 §11: CONFIRMED]`.
6. **The record types this client reads:**

   | Type | Code | RDATA | Source |
   |---|---|---|---|
   | A | 1 | a 4-byte IPv4 address | `[S1 §3.4.1: CONFIRMED]` |
   | CNAME | 5 | one name | `[S1 §3.3.1: CONFIRMED]` |
   | SOA | 6 | MNAME (name), RNAME (name), SERIAL, REFRESH, RETRY, EXPIRE, MINIMUM (five 32-bit fields) | `[S1 §3.3.13: CONFIRMED]` |
   | AFSDB | 18 | subtype (16 bits), hostname (name) | `[S2 §1: CONFIRMED]` |
   | AAAA | 28 | a 16-byte IPv6 address, network order | `[S6 §2.1, §2.2: CONFIRMED]` |
   | SRV | 33 | priority, weight, port (16 bits each), target (name) | `[S5 "The format of the SRV RR": CONFIRMED]` |
   | OPT | 41 | zero or more options, each code (16), length (16), data | `[S13 §6.1.1, §6.1.2: CONFIRMED]` |

   Class IN is 1 `[S1 §3.2.4: CONFIRMED]`. An SRV target of the root
   name means the service is not offered at that name
   `[S5 "The format of the SRV RR": CONFIRMED]`. AFSDB subtype 1 names a
   host with an AFS volume location server for the cell; subtype 2 is
   DCE's `[S2 §1: CONFIRMED]`, which RFC 5864 leaves out of scope
   `[S10 §2: CONFIRMED]`.
7. **The OPT record** has the root as its name; its CLASS field carries
   the sender's UDP payload size; its TTL field carries the upper 8 bits
   of the extended RCODE, the EDNS version and the flags, of which the top
   bit is DO; a message holds at most one, in the additional section
   `[S13 §6.1.1 to §6.1.4: CONFIRMED]`. The full response code is the
   extended RCODE's 8 bits above the header's 4 `[S13 §6.1.3: CONFIRMED]`.
8. **The AFS service names** are `_afs3-vlserver._udp.<cell>` for volume
   location and `_afs3-prserver._udp.<cell>` for protection, where
   `<cell>` is the cell's name exactly, with no leading label removed
   `[S10 §4: CONFIRMED]`.
9. **TCP framing:** each message is preceded by a two-byte length that
   does not count itself `[S1 §4.2.2: CONFIRMED]`. Server port 53 for
   both UDP and TCP `[S1 §4.2: CONFIRMED]`.

### 5.2 Caps

Every cap is a named constant in `ironafs-dns`, in bytes or in a count
whose memory is bounded by a byte cap beside it. Each is checked before
the allocation or loop it guards `[OWN]`, from coding standards rules 1
to 3.

| Name | Value | Applies to | Why |
|---|---|---|---|
| `dns.udp_payload` | 1,232 bytes | the payload size advertised in OPT | the IPv6 minimum link MTU of 1,280 bytes `[S16 §5: CONFIRMED]` less 40 bytes of IPv6 header and 8 of UDP header, so a reply fits one packet on every IPv6 path; within the 1,400-byte maximum a requestor should advertise `[S18 §3.2: CONFIRMED]` `[OWN]` |
| `dns.udp_reply_max` | 4,096 bytes | a UDP reply received | some servers send more than the advertised size `[S18 §5: CONFIRMED]`; accepting up to 4,096 costs nothing and is the size RFC 6891 names as a starting point `[S13 §6.2.5: CONFIRMED]`. The receive buffer is one byte longer, so a larger datagram is seen as too large, never silently cut `[OWN]` |
| `dns.tcp_reply_max` | 16,384 bytes | a TCP reply, checked against its length prefix before any buffer is reserved | an SRV record with a compressed owner and an uncompressed 255-byte target takes 2 + 10 + 6 + 255 = 273 bytes, so `dns.targets_max` such records (4,368 bytes) fit several times over. An answer of `dns.answer_records_max` of them would need 17,472 bytes and more, and is refused as `MessageTooLarge`: a try failure, never a negative, and a cell that publishes one is configured explicitly (section 11) `[OWN]` |
| `dns.query_max` | 512 bytes | a query the encoder builds | a question of at most 255 + 4 bytes, the header and one OPT record; the encoder refuses anything longer `[OWN]` |
| `dns.name_max` | 255 bytes | a name in uncompressed wire form | `[S1 §2.3.4: CONFIRMED]` |
| `dns.label_max` | 63 bytes | one label | `[S1 §2.3.4: CONFIRMED]` |
| `dns.name_pointers_max` | 16 | compression pointers followed in one name | 5.4, point 5 already ends every walk; this bounds the work per name to a small constant `[OWN]` |
| `dns.reply_records_max` | 256 | ANCOUNT + NSCOUNT + ARCOUNT of one reply | no reply to the questions of 5.1, point 6 needs more; it bounds the walk per reply `[OWN]` |
| `dns.answer_records_max` | 64 | applicable records kept from one reply | the same number ADR 0005, Decision 3, point 1, sets for macOS and Windows (`MAX_DNS_RECORDS`), so every system takes the same answers `[D2 Decision 3.1: Working]` |
| `dns.cname_chain_max` | 8 | CNAME records followed in one answer | no AFS name needs a long alias chain; a longer one is refused, not followed `[OWN]` |
| `dns.targets_max` | 16 | distinct SRV or AFSDB targets in one lookup | bounds the address queries one lookup makes (5.6.6, point 6); a cell with more is configured explicitly `[OWN]` |
| `dns.addresses_max` | 16 | applicable A or AAAA records in one reply, so addresses per target and family | bounds the result's size; a reply with more is `Unusable`, never cut (5.4, point 8.1) `[OWN]` |
| `dns.resolv_conf_max` | 65,536 bytes | the resolver configuration read | the read stops at the cap + 1 bytes, so a larger file is seen and refused `[OWN]` |
| `dns.resolv_conf_line_max` | 1,024 bytes | one line of the resolver configuration | resolv.conf(5) names no limit `[M1: CONFIRMED]`; a longer line makes the file unusable (5.6.2, point 3) `[OWN]` |
| `dns.name_servers_max` | 3 | name servers taken from the resolver configuration | resolv.conf(5) documents up to three, currently `[M1 "nameserver": CONFIRMED]`; later lines are ignored and reported `[OWN]` |
| `dns.explicit_servers_max` | 8 | explicit servers, globally and per cell | a small, documented bound on one query's fan-out `[OWN]` |
| `dns.lookups_max` | 8 | lookups in progress at once | 5.6.9 `[OWN]` |
| `dns.waiting_max` | 64 | lookups waiting for a slot | 5.6.9 `[OWN]` |
| `dns.positive_entries_max` | 1,024 | entries in the positive cache: one "Servers" outcome per cell and service (5.6.7, point 10) | 5.6.7, point 7 `[OWN]` |
| `dns.negative_entries_max` | 4,096 | names in the negative cache | an indexer can probe many names; an evicted entry costs one query, never a wrong answer (5.6.7, point 7) `[OWN]` |
| `dns.holddown_entries_max` | 1,024 | cell and service pairs in the failure hold-down | as above `[OWN]` |

A memory bound follows: one lookup holds at most one receive buffer
(`dns.tcp_reply_max` + 2 bytes at most) and at most
`dns.answer_records_max` decoded records of at most `dns.name_max` bytes
of name each, plus fixed fields; `dns.lookups_max` lookups run at once.
The decoder never reserves memory from a count or a length read from
input before that count or length has passed its cap and been checked
against the bytes present (5.4, point 4) `[OWN]`.

### 5.3 Versions

1. This client speaks EDNS version 0 `[S13 §6.1.3: CONFIRMED]`. A reply's
   OPT may carry a higher version, which states the responder's own level
   `[S13 §6.1.3: CONFIRMED]`; it is accepted and not acted on `[OWN]`.
2. No format here is IronAFS's own, so there is no magic value and
   nothing to freeze ([README](README.md), "Formats IronAFS does not
   control"). The caches of 5.6.7 live in memory only and are never
   written to disk `[OWN]`.

### 5.4 Decoding

The decoder takes one complete, bounded reply (5.2) and the try it may
answer. Every read goes through a cursor bounded by the reply's end; no
read passes it `[OWN]`. The checks run in this order.

1. **Framing.** On TCP, a length prefix below 12 is `Truncated` and one
   above `dns.tcp_reply_max` is `MessageTooLarge`, before any buffer is
   reserved (5.6.5, point 9), and the try fails: only the server can
   write on the connection the try opened `[S9 §9.3: CONFIRMED]`. On
   UDP, a datagram shorter than 12 bytes is `Unmatched` (short) `[OWN]`:
   it cannot be matched, and since anyone who can reach the socket can
   send one, a datagram that cannot be matched never fails a try. The
   UDP size cap is checked only after the match (point 2.6).
2. **Match, before anything is trusted.** A reply is matched only if all
   of these hold; a reply that fails any of them is discarded as
   `Unmatched` with its reason, and the try keeps waiting (5.6.5,
   point 6) `[S9 §9.1: CONFIRMED]`. On UDP these checks read only the
   first `dns.udp_reply_max` bytes of the datagram, whatever its size
   `[OWN]`:
   1. it came from the address and port the try was sent to, and to the
      try's own socket (the connected UDP socket, or the TCP connection,
      ensures both) `[S9 §3, §9.1: CONFIRMED]`;
   2. ID equals the try's ID `[S1 §4.1.1: CONFIRMED]`;
   3. QR is 1 and the opcode is 0 `[S1 §4.1.1: CONFIRMED]`;
   4. QDCOUNT is 1 `[S17 §4: CONFIRMED]`;
   5. the question decodes (5.1, point 3, with its name read as point 5
      says; a question that does not decode is `Unmatched` (question)),
      its QTYPE and QCLASS equal
      the try's, and its name equals the try's name: byte for byte when
      the try used random letter case (5.5, point 4), otherwise equal
      under ASCII case folding `[S8 §3: CONFIRMED]` `[S9 §9.1: CONFIRMED]`.
      A name that differs from the try's only in the case of letters, on
      a try that used random case, is `CaseMismatch`, a kind of
      `Unmatched` that 5.6.5, point 6, treats as a sign of forgery
      `[S19 §5.5: CONFIRMED]`.
   6. **Then the size, on UDP.** A matched datagram longer than the
      try's cap is `MessageTooLarge`: the cap is `dns.udp_reply_max` on a
      try with OPT, and 512 bytes on a try without, the limit of a UDP
      message without EDNS(0) `[S1 §2.3.4, §4.2.1: CONFIRMED]`. The
      receive buffer is one byte longer than `dns.udp_reply_max`, so a
      larger datagram is seen as too large, never silently cut `[OWN]`.

   Every later check fails the try with the error named, never just the
   record: a matched reply that breaks the format is not used at all
   `[OWN]`.
3. **Truncation.** A matched UDP reply with TC set is not decoded
   further and nothing in it is used; the query goes to TCP (5.6.5,
   point 4) `[S3 §9: CONFIRMED]` `[S5 "Usage rules": CONFIRMED]`. A TCP
   reply with TC set is `TruncatedOverTcp` `[OWN]`: TCP has no smaller
   limit to escape.
4. **Counts.** ANCOUNT + NSCOUNT + ARCOUNT, summed in 32 bits, must be at
   most `dns.reply_records_max`, else `CountOverCap`. The bytes after the
   question must be at least 11 times that sum, else `CountsExceedReply`:
   every record is at least 11 bytes (a one-byte root name and ten bytes
   of fixed fields) `[S1 §4.1.3: CONFIRMED]`. Both checks come before any
   record is walked or any memory reserved `[OWN]`.
5. **Names.** A name is read from a start offset with these rules
   `[S1 §4.1.4: CONFIRMED]` `[OWN]` (the last three):
   1. a byte with top bits 00 is a label length; 0 ends the name. The
      uncompressed length counts every length byte and label byte, the
      final zero byte included (5.1, point 5). A label of length L is
      accepted only if the uncompressed length so far, plus 1 for its
      length byte, plus L, plus 1 for the zero byte still to come, is at
      most `dns.name_max`; otherwise it is `NameTooLong`, before its bytes
      are read (section 6, A4);
   2. top bits 01 or 10 are `BadLabelType` `[S1 §4.1.4: CONFIRMED]`
      `[S13 §9: CONFIRMED]` (extended labels are not used);
   3. top bits 11 begin a pointer. Its target must be **strictly less
      than the offset where the current run of labels began**: the name's
      own start for the first run, the previous pointer's target after
      that. Otherwise it is `PointerNotBackwards`. A target below 12, in
      the header where no name can be, is `PointerIntoHeader` `[OWN]`.
      So each jump lands
      strictly earlier than the last, a pointer can never reach itself or
      an earlier pointer of the same name, and every walk ends. A
      compressor only ever points to a prior occurrence of a name
      `[S1 §4.1.4: CONFIRMED]`, so a correct message always passes;
   4. more than `dns.name_pointers_max` pointers in one name is
      `PointerHopsExceeded`;
   5. the name's length in place (the bytes up to and including its zero
      byte or its first pointer) is what counts against the field that
      holds it `[S1 §4.1.4: CONFIRMED]`.

   A name is kept as its uncompressed wire bytes with their case as
   received, and compared with ASCII case folding `[S8 §3: CONFIRMED]`.
   Compression is accepted in every name this client reads, owner names
   and the names inside CNAME, SOA, SRV and AFSDB data
   `[S7 §4: CONFIRMED]`. **Tolerance:** compression in an SRV target is
   accepted although RFC 2782 says not to use it, as RFC 3597 advises
   receivers `[S7 §4: CONFIRMED]` `[S5 "The format of the SRV RR": CONFIRMED]`.
6. **Records.** Each record of every section is walked: the owner name
   (point 5), then the 10 fixed bytes, then RDLENGTH, which must not
   carry the cursor past the reply's end (`Truncated`)
   `[S1 §4.1.3: CONFIRMED]`. Then by section:
   - **Answer.** The CNAME chain is built first: start at the question
     name; while a CNAME of class IN is owned by the current name, follow
     it. Two CNAME records owned by one name are `CnameConflict`; a name
     met twice is `CnameLoop`; more than `dns.cname_chain_max` is
     `CnameChainTooLong` `[OWN]`. Then each record of class IN, of the
     type asked and owned by the chain's last name is applicable (RFC 5452
     §6's rule: only data for the name asked for is used
     `[S9 §6: CONFIRMED]`). Every other record in the answer is
     irrelevant: skipped and counted, never an error by itself `[OWN]`,
     matching ADR 0005, Decision 3, point 1 `[D2 Decision 3.1: Working]`.
   - **Authority.** An SOA record of class IN whose owner equals the last
     name of the chain or one of its ancestors is kept for the negative
     lifetime (point 8); others are skipped. If more than one is kept,
     the lowest of their lifetimes is used `[OWN]`: the shorter claim
     can never keep a negative too long. Whether any NS record is
     present is noted (point 8). Nothing else here is read `[OWN]`, from
     `[S9 §6: CONFIRMED]`.
   - **Additional.** The OPT record (point 7) is read. Every other record
     is skipped: address records a server adds for SRV or AFSDB targets
     are not used, and the targets are looked up on their own (5.6.6,
     point 5) `[OWN]`: fewer rules about which added data may be trusted;
     RFC 2782 allows either `[S5 "Usage rules": CONFIRMED]`.

   For each kept record, the fields of its type's layout (5.1, point 6)
   are read in order through a cursor bounded by the end of its RDATA,
   and the last field must end exactly where RDLENGTH ends; a field that
   would pass that end, or bytes left after the last field, are
   `RdataLength` `[OWN]`. After the last record, no byte may
   remain (`TrailingBytes`) `[OWN]`: DNS has no padding outside OPT, and a
   reply with extra bytes is not one this client understands.
7. **OPT.** More than one OPT record, an OPT outside the additional
   section, or one whose name is not the root is `BadOpt`
   `[S13 §6.1.1, §6.1.2: CONFIRMED]`. Its options are not interpreted;
   RDLENGTH bounds them as for any record. Unknown options are ignored
   `[S13 §6.1.2: CONFIRMED]`. The full response code is formed as 5.1,
   point 7, says. A response code above 5 is `UnexpectedRcode`, a server
   failure for 5.6.5 `[OWN]`.
8. **Outcome of a matched, well-formed reply.** Exactly one of:
   1. **Records.** RCODE 0 and at least one applicable record. Every
      applicable record must pass its record rule (point 9); one that
      fails makes the whole reply `Unusable` with its reason, and no
      record of it is used `[OWN]`, matching ADR 0005, Decision 3,
      point 1 `[D2 Decision 3.1: Working]`. More than
      `dns.answer_records_max` applicable records, or more than
      `dns.addresses_max` applicable A or AAAA records, is `Unusable`,
      never cut `[OWN]`. If their TTLs differ, all take the lowest: this
      client sends every query to servers it was configured to use, so it
      treats them as authoritative for that rule `[S3 §5.2: CONFIRMED]`.
      The CNAME records of the chain that led to them count in that
      lowest TTL too: each record may be kept only for its own TTL
      `[S1 §4.1.3: CONFIRMED]`, and an answer reached through an alias is
      kept no longer than the alias `[OWN]`.
   2. **Not offered.** An SRV query whose only applicable record has the
      root as its target `[S5 "Usage rules": CONFIRMED]` `[ADR 0004
      C33.1: Working]`. A root target beside other SRV records is
      `Unusable` `[OWN]`: the record means nothing unless it stands alone.
      Its lifetime is the lowest TTL of the SRV record and of the CNAME
      records of the chain that led to it, as in point 8.1 `[OWN]`.
   3. **Conclusive negative.** All of these hold
      `[S4 §2.1, §2.2: CONFIRMED]`:
      - RCODE 3 (name error), or RCODE 0 with no applicable record;
      - for RCODE 0, the reply is not a referral: an SOA record was kept
        from the authority section, or no NS record is there
        `[S4 §2.2: CONFIRMED]`;
      - the answer section holds no CNAME record. A negative that follows
        an alias can need a further query to be certain
        `[S4 §2.2.1: CONFIRMED]`; this client does not make it, and the
        outcome is `Inconclusive` `[OWN]`;
      - RA or AA is set: the server either recursed or holds the zone.
        Otherwise the outcome is `Inconclusive` `[OWN]`.

      Its lifetime is the lower of the kept SOA record's TTL and its
      MINIMUM field `[S4 §3, §5: CONFIRMED]`, capped (5.6.7, point 5).
      A MINIMUM whose top bit is set is read as 0, as a TTL is
      (5.1, point 4) `[OWN]`: the field serves as a TTL here, and the
      reading fails closed.
      With no SOA kept, the negative is still conclusive for this lookup,
      but has no lifetime and is never cached `[S4 §5: CONFIRMED]`.
   4. **Server failure.** RCODE 1, 2, 4 or 5, or `UnexpectedRcode`.
      RCODE 1 or 4 to a query that carried OPT, from a reply that has no
      OPT, means the server does not speak EDNS(0) (5.6.5, point 5)
      `[S13 §6.2.2, §7: CONFIRMED]`.
   5. **Inconclusive.** As named in point 8.3.
9. **Record rules**, for each applicable record `[OWN]` unless marked:
   - **SRV:** a target that is the root is handled by point 8.2; any other
     target must be a host name (below), with a port other than 0, which
     cannot be reached.
   - **AFSDB:** subtype 1 is used; any other subtype is irrelevant, not an
     error `[S2 §1: CONFIRMED]` `[S10 §2: CONFIRMED]`; the hostname must
     be a host name. An answer whose only AFSDB records are of other
     subtypes has no applicable record (point 8.3).
   - **A and AAAA:** exactly 4 or 16 bytes `[S1 §3.4.1: CONFIRMED]`
     `[S6 §2.2: CONFIRMED]`. These addresses are refused, so DNS can never
     point the client at the host itself, at a group, or at an address
     that needs an interface scope; the choice of list is `[OWN]`, the
     blocks are the sources':
     - IPv4: 0.0.0.0/8, which is not a valid destination
       `[S22 §2.2.2: CONFIRMED]`; 127.0.0.0/8, loopback
       `[S22 §2.2.2: CONFIRMED]`; 169.254.0.0/16, link-local
       `[S22 §2.2.2: CONFIRMED]`; 224.0.0.0/4 (224.0.0.0 to
       239.255.255.255), multicast `[S23 §3: CONFIRMED]`; and
       240.0.0.0/4, reserved and not a valid destination, which holds the
       limited broadcast address 255.255.255.255
       `[S22 §2.2.2: CONFIRMED]`.
     - IPv6: ::/96, which holds the unspecified address `::` and the
       loopback `::1` `[S21 §2.4: CONFIRMED]` and the deprecated
       IPv4-compatible addresses `[S21 §2.5.5.1: CONFIRMED]`;
       ::ffff:0:0/96, the IPv4-mapped addresses
       `[S21 §2.5.5.2: CONFIRMED]`, not a valid destination
       `[S22 §2.2.3: CONFIRMED]`; fe80::/10, link-local unicast; and
       ff00::/8, multicast `[S21 §2.4: CONFIRMED]`. An IPv4-mapped address
       is refused whatever IPv4 address it holds `[OWN]`: on a dual-stack
       socket it would reach that IPv4 address past the IPv4 list above,
       and a server's IPv4 address comes from an A record.

     Private and unique-local addresses are accepted: a cell behind a VPN
     uses them.
   - **Host name:** after ASCII lower-casing, 1 to 253 characters with no
     final dot, labels of 1 to 63 characters of ASCII letters, digits and
     hyphens, an underscore allowed only as a label's first character.
     This is the rule ADR 0005, Decision 3, point 1, applies to every
     name it sends `[D2 Decision 3.1: Working]`; a target that fails it
     could not be asked for its addresses. DNS itself allows any label
     `[S3 §11: CONFIRMED]`, and a client may restrict the values it uses
     `[S3 §11: CONFIRMED]`.

### 5.5 Encoding

1. **The canonical query** `[S1 §4.1.1 to §4.1.3: CONFIRMED]`
   `[S12 §5.7: CONFIRMED]` `[S13 §6.1.2, §6.1.3: CONFIRMED]`:
   - ID from the production `Rng` (point 3);
   - QR 0, opcode 0, AA 0, TC 0, RD 1, RA 0, Z 0, AD 1, CD 0, RCODE 0.
     RD asks the resolver to recurse. AD asks it to report whether it
     validated the answer `[S12 §5.7, §5.8: CONFIRMED]`; the bit is
     reported, never relied on (5.6.8, point 2);
   - QDCOUNT 1, ANCOUNT 0, NSCOUNT 0, ARCOUNT 1 (0 on a try without
     OPT, 5.6.5, point 5);
   - the question: the name as uncompressed labels ending in the zero
     byte, QTYPE, QCLASS 1;
   - the OPT record: name the root, TYPE 41, CLASS `dns.udp_payload`,
     TTL 0 (extended RCODE 0, version 0, DO 0), RDLENGTH 0. DO stays 0:
     this client does not validate and needs no signatures.
2. **What the encoder refuses** `[OWN]`, before writing a byte: a name
   that fails the host-name rule of 5.4, point 9, except the underscore
   labels of 5.1, point 8; a name longer than `dns.name_max` in wire
   form; a QTYPE other than SRV, AFSDB, A or AAAA; a total above
   `dns.query_max`. A cell whose service name would exceed
   `dns.name_max` is handled by 5.6.6, point 2.
3. **The query ID** is 16 bits drawn from the production `Rng`, over the
   full range 0 to 65,535, fresh for every try
   `[S9 §9.2: CONFIRMED]` `[S1 §4.1.1: CONFIRMED]`. A failure of the
   `Rng` fails the try before anything is sent `[OWN]`: an identifier
   that is not random is not sent.
4. **Random letter case.** On a UDP try to a server that is not a
   loopback address, each ASCII letter of the question name takes upper
   or lower case from one `Rng` bit `[S19 §5.1: CONFIRMED]`. Tries over
   TCP, and tries to a loopback address, send the name in lower case
   `[OWN]`: TCP resists forgery by its own means `[S9 §9.3: CONFIRMED]`,
   and a datagram on the loopback interface does not cross the network;
   whether every local resolver keeps a query's letter case in its reply
   is not documented, and a local resolver that did not would stop
   discovery on the path most hosts use. Records are matched after
   decoding with ASCII case folding, so the random case never reaches a
   cache or a status `[S19 §5.4: CONFIRMED]`.
5. **The source port** of a UDP try is drawn from the production `Rng`,
   uniformly over 1,024 to 65,535: a 16-bit draw below 1,024 is drawn
   again, and the socket is bound to the port drawn; a port in use or
   refused is drawn again. A try makes at most 16 draws in all, of either
   kind, after which it fails with `NoSourcePort` and nothing is sent
   `[S9 §9.2: CONFIRMED]` `[OWN]` (the bound and the failure). Every try has its own socket and port
   `[S9 §9.2: CONFIRMED]`. A TCP try uses the source port the operating
   system chooses `[OWN]`: the standard library's connect does not bind
   first, and TCP's sequence numbers protect it `[S9 §9.3: CONFIRMED]`.
6. **TCP framing.** The two-byte length and the query are written in one
   write `[S15 §8: CONFIRMED]`.

### 5.6 States and ordering

#### 5.6.1 Where the client lives

1. **`ironafs-dns`, a pure crate** (C4, point 1, as C4, point 6,
   allows), holds: the codec (5.1 to 5.5); the resolver configuration
   parser (5.6.2); the query state machine of 5.6.5, which takes replies,
   time and random bytes in and gives "send this", "wait until", "use
   TCP" and outcomes out, as C3, point 1, requires of protocol code; the
   lookup of 5.6.6; and the caches of 5.6.7 as pure types. It depends only
   on `ironafs-time`, follows the pure-crate rules, and is fuzzed (10.4)
   `[ADR 0004 C3.1, C4.1, C4.6: Working]`.
2. **The socket and file driver is in `ironafs-net`** (C4, point 6): it
   reads `/etc/resolv.conf`, opens UDP and TCP sockets through `std`,
   sends and receives, and hands bytes to `ironafs-dns`. It uses no
   `unsafe` and no `ironafs-os` site `[D2 Decision 6: Working]`.
3. **The composition is in `ironafs-client`**, which owns cell discovery
   and the location service (C4, point 2) and consults the negative cache
   for the dynamic root (C12, point 3) `[ADR 0004 C4.2: Working]`.
4. Why a crate of its own rather than a module of `ironafs-proto` `[OWN]`:
   DNS shares nothing with the AFS codecs (its own encoding, no XDR, no
   generated code); `ironafs-proto` keeps one subject; the fuzz targets
   take the crate's name (coding standards rule 20); and the dependency
   check (C4, point 4) can hold `ironafs-dns` to `ironafs-time` alone.
   ADR 0004's crate table (C4, point 1) and the dependency check's list
   gain the row when the crate is created (section 13, Q3).

#### 5.6.2 The host resolver view

1. **The file** is `/etc/resolv.conf` `[M1 "SYNOPSIS": CONFIRMED]`, read by
   `ironafs-net` with the standard library, which follows a symbolic
   link. The environment variables resolv.conf(5) describes
   `[M1: CONFIRMED]` are not read: the daemon's environment is not policy
   `[OWN]`.
2. **Lines** `[M1: CONFIRMED]`: a line whose first character is `;` or
   `#` is a comment; otherwise the keyword starts the line and its value
   follows after white space. Only the keyword `nameserver` is read. Every
   other keyword, `search`, `domain` and `options` among them, is
   ignored `[OWN]`: names are always absolute, because RFC 5864 asks for
   the cell's exact name `[S10 §4: CONFIRMED]`, and the timeouts
   are this specification's (5.6.5, point 7), which C33, point 4, asks to
   be documented.
3. **A `nameserver` line** holds exactly one value: an IPv4 address in
   dotted-quad form or an IPv6 address `[M1 "nameserver": CONFIRMED]`. The
   file is **unusable** if `[OWN]`: it is longer than
   `dns.resolv_conf_max`; a line is longer than
   `dns.resolv_conf_line_max`; a `nameserver` line has no value, more than
   one, a value that is not an address, or a value with an interface zone
   (`%`); or a `nameserver` line holds a byte outside printable ASCII,
   space and tab. Only the first `dns.name_servers_max` `nameserver`
   lines count, in order; later ones are ignored and counted
   `[M1 "nameserver": CONFIRMED]`. The port is always 53
   `[S1 §4.2: CONFIRMED]`.
4. **The view** `[OWN]`, from C33, points 3 to 5:
   - **none** if the file is missing, unreadable, unusable or lists no
     name server. resolv.conf(5) says a missing file or no `nameserver`
     line means the name server on the local machine
     `[M1 "DESCRIPTION", "nameserver": CONFIRMED]`, but nothing shows
     that one runs, so this is not taken as a local resolver: C33,
     point 3, asks for an *established* one;
   - **local resolver** if every listed server is a loopback address
     (section 4);
   - **flat list** otherwise, including a list that mixes loopback and
     other addresses: such a host sends queries past its local resolver
     whenever that resolver fails, so the list as a whole is not a local
     resolver.
5. **Why the loopback rule detects every established local resolver
   without requiring one.** systemd-resolved's documented stub mode lists
   its stub, on 127.0.0.53, as the only server
   `[M2 "/etc/resolv.conf": CONFIRMED]`; NetworkManager documents that
   with its dnsmasq, systemd-resolved or dnsconfd plug-ins the name server
   it writes is localhost only `[M3 "dns": CONFIRMED]`. The rule names no
   address, port or product. A host whose resolver configuration lists
   upstream servers, even while a local resolver runs (systemd-resolved's
   mode that lists all known servers `[M2 "/etc/resolv.conf": CONFIRMED]`),
   has said that programs reading the file go to those servers, and is a
   flat list here `[OWN]`.
6. **What this rule cannot know** `[OWN]`: a loopback name server is the
   host's choice, but nothing proves that the program listening there is
   the one the administrator installed (section 8, residual R1).
7. **When the view is read again** `[OWN]`, from C33, point 4: at
   start; on every network-generation change; and before every lookup,
   when the file's identity has changed since the last read (device,
   inode, size or modification time of the file the path resolves to, from
   the standard library's metadata). A path whose metadata cannot be
   read (the file is missing, unreadable, or a symbolic link whose target
   is gone) has the identity "missing"; a change from a known identity
   to "missing", or back, is a change, and the read then gives the view
   of point 4 (none, while the file cannot be read). A read that gives a
   different view (kind or list) moves the **view generation**, which
   5.6.7, point 3, treats like a network change.

#### 5.6.3 Choosing servers

For each query of a lookup, in this order
`[ADR 0004 C33.3, C33.4, C33.5: Working]`. The address queries for a
lookup's targets (5.6.6, point 5) are queries of that lookup: they take
the cell's switch, scope and path, never another's `[OWN]`.

1. **Discovery off.** If discovery is off globally, or for the cell
   (5.6.4), no query is sent and the lookup ends `DiscoveryOff`
   `[ADR 0004 C33.8: Working]`.
2. **A scoped cell** (5.6.4, point 5) is resolved only through the
   cell's own servers, each only while the network-generation source
   reports that the route toward it leaves by the cell's configured
   interface. The route is checked before every try, over either
   transport; a server whose route cannot be confirmed is skipped. With
   no server left, nothing is sent and the lookup ends `WithheldByScope`
   `[ADR 0004 C33.5: Working]`. A scoped cell never uses the local
   resolver, a flat list or the explicit servers `[OWN]`: this client
   cannot see where a local resolver forwards a name, so a stub could
   send it out of the default route while the VPN is down. The route
   answer is a requirement on the network-generation source (#97),
   MISSING until it is specified there (section 13, Q4); until then a
   scoped cell sends nothing.
3. **Local resolver.** If the view is a local resolver, its servers are
   asked, in order, and no other server ever is
   `[ADR 0004 C33.3: Working]`.
4. **Confirmed flat list.** If the view is a flat list and it equals the
   confirmed list (5.6.4, point 3), the listed servers are asked in order
   `[ADR 0004 C33.4: Working]`.
5. **Explicit servers.** Otherwise, the explicit servers are asked, in
   order; with none configured, nothing is sent and the lookup ends
   `NoServer` `[ADR 0004 C33.5: Working]`.
6. An unconfirmed flat list is never asked, not even as a fallback after
   the explicit servers fail `[ADR 0004 C33.4: Working]`.
7. The path is fixed when the query starts and recorded with its outcome
   (5.6.8). A lookup whose path changes (5.6.7, point 3) starts again on
   the new path `[OWN]`.

#### 5.6.4 Configuration keys

The meaning of each key is set here; the spelling in files and policy,
and the parser, are `ironafs-config`'s (#93). Names below are this
specification's proposal.

1. `discovery.enabled`: a boolean, default true. False turns discovery off
   for every cell `[ADR 0004 C33.8: Working]`.
2. `cells.<cell>.discovery`: a boolean, default true. False turns it off
   for one cell `[ADR 0004 C33.8: Working]`.
3. `discovery.flat_list_confirmed`: an ordered list of up to
   `dns.name_servers_max` addresses, default empty. It is the
   administrator's confirmation that exactly this flat list is the
   intended DNS policy `[ADR 0004 C33.4: Working]`. It holds only while
   the view's list equals it, address for address and in order. It is
   rechecked whenever the view is read (5.6.2, point 7), which includes
   every network change; a list that no longer equals it pauses the
   flat-list path and raises an event (5.6.8, point 3) until an
   administrator confirms the new list `[OWN]`, from C33, point 4. It
   carries no wildcard and cannot say "any list" `[OWN]`: a confirmation
   is of a policy, not a blanket permission.
4. `discovery.servers`: an ordered list of up to
   `dns.explicit_servers_max` addresses, each with an optional port
   (default 53), default empty: the explicit servers of C33, point 5.
5. `cells.<cell>.dns_scope`: `host` (default: 5.6.3, points 3 to 5) or
   `servers`; with `servers`, also `cells.<cell>.dns_servers` (up to
   `dns.explicit_servers_max` addresses with optional ports) and
   `cells.<cell>.dns_interface` (an interface name)
   `[ADR 0004 C33.5: Working]`, the scope rule for a cell reachable only
   through a VPN. Scope `servers` with no server or no interface sends
   nothing (5.6.3, point 2). This key is the only way the client learns
   that a cell is reachable only through a VPN `[OWN]`: a cell's name
   carries no such mark, so a cell without it is resolved on the host's
   path, whatever that path is (section 8, residual R4).
6. **Who can set them** `[OWN]`, from C33, points 4 and 8, and O17,
   point 2. Every key above is read only from the configuration files
   the daemon loads and from OS policy, never from a run-time override
   (O17, point 1, limits those to debug levels and captures) and never
   through the control endpoint. A file contributes these keys only if
   it, and every directory on its path, is owned by uid 0 and writable by
   no other user or group. `ironafs-config` (#93) decides what happens to
   the rest of a file that fails, under one constraint set here: a failing
   source never yields a flat-list confirmation, a server list or a scope
   that the last good configuration did not have. A value set by policy
   locks its key (O17, point 2).
7. Changing any of these keys is a policy change for this client: the
   view generation moves (5.6.7, point 3) `[OWN]`.

#### 5.6.5 One query

1. **Tries.** The query sends one try to each server of its path, in
   order, waiting for each to end before the next (C33, point 4: "asked
   in order"); that is one round. It runs at most two rounds. A server
   that ended a try with `Unreachable` is skipped in the second round
   `[OWN]`. No server address gets more than three tries per transport for
   one query `[S20 §3.1: CONFIRMED]`.
2. **The first transport is UDP**, with OPT `[S15 §5: CONFIRMED]` (either
   transport may come first) `[OWN]` (UDP's lower cost).
3. **A UDP try** binds its own socket (5.5, point 5), connects it to the
   server, so the kernel delivers only datagrams from that address and
   port, sends the query, and waits until its try deadline (point 7)
   `[S9 §9.1: CONFIRMED]` `[OWN]`.
4. **Truncation.** A matched reply with TC set ends the UDP try, and the
   same query goes at once to the same server over TCP
   `[S3 §9: CONFIRMED]`. Nothing from the truncated reply is used or
   cached (5.4, point 3).
5. **EDNS fallback.** A server failure of the kind 5.4, point 8.4, names
   for a server that does not speak EDNS(0) gives one more try to the same
   server, on the same transport, without OPT
   `[S13 §6.2.2: CONFIRMED]`. That try's replies are capped at 512 bytes
   over UDP `[S1 §4.2.1: CONFIRMED]`, and a truncated one goes to TCP as
   in point 4. Once a server has needed the fallback, every later try of
   the same query to that server, in either round, is sent without OPT,
   so no server gets more than three UDP tries for one query (point 1);
   RFC 6891 lets a requestor keep that knowledge for a short time
   `[S13 §6.2.2: CONFIRMED]`. It is not kept beyond the query `[OWN]`.
6. **Unmatched replies and forgery.** An `Unmatched` reply (5.4,
   point 2) is counted and dropped, and the try keeps waiting. If a try
   sees a `CaseMismatch`, or more than 4 other unmatched replies, the
   UDP try is abandoned and the query is sent to the same server over
   TCP `[S9 §9.3: CONFIRMED]` `[S19 §5.5: CONFIRMED]`; the threshold of 4
   is `[OWN]`. This is also how a server that does not keep the random
   case is served: through TCP, which needs no random case, so the
   protection is never switched off on a remote path `[S19 §6.3: CONFIRMED]`.
7. **Timeouts** `[OWN]`, documented as C33, point 4, requires:

   | Name | Value | Clock | Meaning |
   |---|---|---|---|
   | `dns.udp_try_timeout` | 2 s | awake | a UDP try waits this long for a matched reply. RFC 1035 recommends a minimum retransmission interval of 2 to 5 seconds `[S1 §4.2.1: CONFIRMED]`; the low end, since most queries go to a local resolver |
   | `dns.tcp_try_timeout` | 4 s | awake | connect, write and read of one TCP try together |
   | `dns.query_deadline` | 10 s | boot | one query, all tries and both transports |
   | `dns.lookup_deadline` | 20 s | boot | one lookup (5.6.6), all its queries |

   A caller may pass an earlier deadline. It bounds that caller's wait
   only: when it passes, the caller stops waiting and gets `Timeout`,
   and the lookup runs on to its own deadline for any other caller and
   for the caches (5.6.7, point 9) `[OWN]`, so one hurried caller
   neither cuts a shared lookup short nor sets a hold-down. Per-try
   timers are retransmission timers and so run on the awake clock; the
   query and lookup deadlines run on the boot clock, so a suspend during
   a lookup ends it rather than stretching it across the night
   `[ADR 0004 C37.1: Working]`. Every wait inside a try (UDP receive, TCP
   connect, write and read) is given the time left of the tighter of its
   try timer and its query deadline, never more `[OWN]`.
8. **Each try ends** with exactly one of: a matched reply (decoded as
   5.4 says); `Timeout`; `Unreachable` (the operating system reports the
   port or host unreachable on the connected socket, or the TCP connection
   is refused or reset) `[S4 §7.2: CONFIRMED]`; `NoSourcePort`; or a
   local error. Then:
   - Records, Not offered or a conclusive negative end the query with
     that outcome;
   - `Inconclusive`, a server failure, `Unusable`, a decode error, or the
     end of a try without a reply moves to the next server, then the next
     round `[S1 §4.2.1: CONFIRMED]`;
   - when every try is spent, or the query deadline passes, the query
     ends with the last failure seen, as `Failed` with its kind; it is
     never a negative `[ADR 0004 C12.3: Working]`.
9. **TCP tries** `[S15 §5, §6.2.1, §6.2.3, §7: CONFIRMED]`: one query per
   connection, closed by this client after its reply or on its timeout
   `[OWN]` (one query per connection keeps a try's state in one place).
   The length prefix is read first; a length below 12 is `Truncated`, one
   above `dns.tcp_reply_max` is `MessageTooLarge`, and only then is a
   buffer of that length reserved. A reply on a TCP connection is matched
   as 5.4, point 2, says, with the question compared under case folding
   `[S15 §7: CONFIRMED]`.
10. **Late replies.** A try's socket is closed when the try ends, so a
    reply to an earlier try is never read by a later one `[OWN]`.
11. **Randomness and time** reach the state machine only through
    `ironafs-time`'s `Rng` and `Clock` (coding standards rule 7). A failed
    clock read follows C37, point 5: the try and the query fail, and no
    new deadline is set until a valid sample returns
    `[ADR 0004 C37.5: Working]`.

#### 5.6.6 A cell lookup

A lookup is of one service of one cell: its volume location servers
or its protection servers (5.1, point 8). The points below apply to
both, with the service name changed; where they differ, the point says
so. Caches, the hold-down and coalescing are keyed by cell and service
(5.6.7) `[OWN]`.

1. **Before any query**, in this order `[OWN]`: the view is read again
   if the file's identity has changed (5.6.2, point 7), so a change seen
   there drops the caches before they are consulted; the switches of
   5.6.3, point 1; the cell name has passed the canonical cell-name rule
   (#93), and a name of one label is not looked up and the lookup ends
   `NotLookedUp`, which is never cached (it would be sent to a
   top-level domain, and RFC 5864 forbids trying shorter names, so a
   single-label cell can be found only through configuration)
   `[S10 §4: CONFIRMED]` for the last part; then the negative cache, the
   positive cache and the failure hold-down are consulted, in that order
   (5.6.7, points 6, 10 and 8). A hit in the negative cache ends a volume
   location lookup "Not a cell" and a protection-server lookup "No
   service" (point 7), without a query: a name that is not a cell has
   neither service.
2. **SRV first** `[ADR 0004 C33.1: Working]` `[S10 §5: CONFIRMED]`: a
   query for `_afs3-vlserver._udp.<cell>`, type SRV. A cell whose service
   name would be longer than `dns.name_max` cannot hold SRV records, so
   SRV is a conclusive negative without a lifetime, and no query is sent
   `[S1 §2.3.4: CONFIRMED]` `[OWN]`.
3. **Then AFSDB**, only if SRV was a conclusive negative: a query for
   `<cell>`, type AFSDB `[S10 §5: CONFIRMED]`. Each subtype 1 record
   stands for a volume location server on port 7003 and a protection
   server on port 7002, with priority 0, weight 0 and the record's TTL
   `[S10 §5: CONFIRMED]`. AFSDB is never asked after SRV failed, timed
   out, was inconclusive, or answered "not offered" `[OWN]` for the
   first three: a failure is not "no SRV records", which is what RFC 5864
   falls back on `[S10 §5: CONFIRMED]`; and after "not offered" the SRV
   rules stop `[S5 "Usage rules": CONFIRMED]`.
4. **The protection servers** (`_afs3-prserver._udp.<cell>`) are found the
   same way, as a separate lookup, when the client needs them
   `[S10 §4: CONFIRMED]`; an AFSDB record gives port 7002 for them
   `[S10 §5: CONFIRMED]`. Such a lookup never says the name is not a
   cell `[OWN]`: a cell may publish its volume location servers alone,
   and whether a name is a cell is the volume location service's
   question. Where point 7 would give "Not a cell", a protection-server
   lookup gives "No service" instead.
5. **Addresses.** For each distinct target, in priority order, an AAAA
   query and an A query, on the lookup's own path and scope (5.6.3), so
   IPv6 can be tried first and IPv4 used as the fallback
   `[ADR 0004 C34.3: Working]` `[S5 "Usage rules": CONFIRMED]`. A target
   whose address queries give no usable address is kept, marked
   unresolved; it does not fail the lookup while another target has an
   address `[OWN]`. A lookup with unresolved targets is cached only
   briefly, so they are asked again soon (5.6.7, point 5); RFC 5864
   lets a client remember unreachable targets if it retries them later
   `[S10 §4: CONFIRMED]`. A lookup in which no target has a usable
   address ends `Failed` (`NoAddress`) `[OWN]`.
6. **Bounds.** More than `dns.targets_max` distinct targets makes the
   lookup `Unusable` `[OWN]`. At most four address queries of one lookup
   are in flight at once `[OWN]`. The lookup deadline (5.6.5, point 7)
   covers everything; targets not yet resolved when it passes are
   unresolved.
7. **Outcomes** of a lookup `[OWN]`:
   - **Servers:** the record set's entries in priority order (priority,
     weight, port, target, addresses by family, unresolved or not), the
     record type that gave them (SRV or AFSDB), the server path, the AD
     bit of each reply, and one lifetime: the lowest TTL of the SRV or
     AFSDB answer and of every address answer used, capped (5.6.7,
     point 5);
   - **Not a cell** (a volume location lookup only): SRV not offered, or
     SRV and AFSDB both conclusive negatives. Its lifetime is the SRV
     "not offered" record's TTL, or the lower of the two negative
     lifetimes; if either negative has none, it has none
     `[ADR 0004 C12.3: Working]` `[S4 §5: CONFIRMED]`;
   - **No service** (a protection-server lookup only): the same
     conditions, for `_afs3-prserver._udp.<cell>`, or the cell is in the
     negative cache (point 1). It is not a negative of C12, point 3, and
     is never cached `[OWN]`: the client asks again when it next needs
     the protection servers, unless the negative cache answers first;
   - **Failed:** with the kind of the query that failed, or
     `DiscoveryOff`, `WithheldByScope`, `NoServer`, `NotLookedUp`,
     `Unusable`, `NoAddress`, `NetworkChanged`. Never cached as a
     negative.
8. **Ranking** within a priority is the location service's (section 2,
   point 2). It may use weights (RFC 2782's selection), measured
   round-trip times and administrator preference, but never ahead of
   priority `[S10 §4, §4.1: CONFIRMED]` `[ADR 0004 C33.1: Working]`.

#### 5.6.7 Expiry, changes and caches

1. **Lifetimes run on the boot clock** `[ADR 0004 C37.1: Working]`. A
   lifetime of 0 is used for the lookup in progress and not cached
   `[S1 §4.1.3: CONFIRMED]`. A boot-clock regression or a failed clock
   sample expires every cached entry at once
   `[ADR 0004 C37.4: Working]`.
2. **Records and rankings expire together.** The lookup's lifetime
   (5.6.6, point 7) is handed to the location service, which discards
   every ranking derived from the records when it passes, and asks again
   `[S10 §4, §4.1: CONFIRMED]` `[ADR 0004 C33.6: Working]`.
3. **A change drops everything.** When the generation pair moves (a
   network change, or a view change, 5.6.2, point 7, and 5.6.4, point 7),
   every positive entry, every negative entry and every failure hold-down
   is dropped, and the location service is told to drop the rankings it
   derived `[ADR 0004 C33.6, C12.3: Working]`.
4. **Late replies are discarded.** Each try records the generation pair
   when it is sent. An outcome whose pair is no longer current is
   discarded, whatever it says; the lookup starts again on the new path
   if its deadline allows, else ends `NetworkChanged`
   `[ADR 0004 C33.6: Working]`.
5. **Caps on lifetimes** `[OWN]`: a positive lifetime is at most
   86,400 seconds (`dns.positive_ttl_max`), or 30 seconds
   (`dns.unresolved_ttl_max`) when any target is unresolved (5.6.6,
   point 5); a negative one at most 3,600
   seconds (`dns.negative_ttl_max`). A client may cap any TTL
   `[S3 §8: CONFIRMED]`; RFC 2308 suggests one to three hours for
   negative answers and no more than the positive cap
   `[S4 §5: CONFIRMED]`.
6. **The negative cache** holds one entry per cell name: the name is not
   a cell, until its lifetime ends. It is filled only by a "Not a cell"
   outcome with a lifetime above 0, which only a volume location lookup
   gives (5.6.6, points 4 and 7), never by "No service", a failure, a
   timeout, a truncated reply or an inconclusive one
   `[ADR 0004 C12.3: Working]`. Filling it drops the cell's positive
   entries for both services `[OWN]`, so no older answer outlives the
   newer negative. It is machine-wide and holds no context
   or user data `[ADR 0004 C12.3: Working]`. A configuration that adds
   the cell explicitly wins over it at once
   `[ADR 0004 C33.1: Working]` (configuration is consulted first; the
   view generation also moves on any key change, 5.6.4, point 7).
7. **Bounds and eviction.** The positive cache, the negative cache and
   the failure hold-down each have their cap (5.2). When full, the entry
   that expires first is evicted `[OWN]`. Eviction only loses an answer;
   the next lookup asks again.
8. **The failure hold-down** `[OWN]`: a lookup that sent at least one
   try and then ended `Failed`, on its own deadline or by its tries,
   while its generation pair was still current, is remembered for 5
   seconds (`dns.failure_holddown`) on the boot clock, keyed by cell and
   service; a lookup of the same cell and service in that time ends at
   once with the same failure, without a query. Outcomes decided without
   a query (`DiscoveryOff`, `WithheldByScope`, `NoServer`, `NotLookedUp`,
   `Busy`), `NetworkChanged`, and a caller that stopped waiting on its
   own deadline (5.6.5, point 7) set none. An entry carries the
   generation pair of its lookup and dies when the pair moves (point 3).
   This keeps an indexer from turning a resolver outage into a storm, and
   it is not a negative: the dynamic root answers it as a failure, never
   as "no such cell" `[ADR 0004 C12.3: Working]`. RFC 9520 requires a
   resolver's failure cache to last at least 1 second and at most 5
   minutes, and gives 5 seconds as a starting point `[S20 §3.2: CONFIRMED]`.
9. **Coalescing** `[OWN]`: a lookup asked for while a lookup of the same
   cell and service is in progress waits for that one's outcome instead
   of sending its own queries, which also keeps several identical
   queries from being in flight at once `[S9 §5: CONFIRMED]`. The shared
   lookup runs under its own deadline (`dns.lookup_deadline`); each
   waiting caller's own earlier deadline ends only that caller's wait
   (5.6.5, point 7).
10. **The positive cache** `[OWN]` holds "Servers" outcomes (5.6.6,
    point 7), keyed by cell and service, each until its lifetime ends
    (points 1 and 5). It is consulted before any query (5.6.6, point 1)
    and filled when a lookup ends "Servers" with a lifetime above 0 and
    its generation pair still current. The key needs no path or scope:
    the path is fixed by the view and the configuration, and any change
    of either moves the generation pair, which empties the cache
    (point 3). It holds only what a lookup's records said, with the AD
    bits and path of 5.6.8; nothing in it raises trust (N5).

#### 5.6.8 Status and events

1. **Where an answer came from** `[ADR 0004 C33.8: Working]`: for every
   cell, status names the source (configuration, the classic files, DNS
   SRV or DNS AFSDB) and, for DNS, the server path and the server that
   answered, and the time left of the lifetime.
2. **What the resolver said about validation**, separately
   `[ADR 0004 C33.8: Working]`: the AD bit of each reply, shown as the
   resolver's own claim. It is never relied on: a hint never raises trust
   `[ADR 0004 N5.2, N5.3: Firm]`, and a responder that copies the query's
   header into its reply returns AD as this client sent it, without
   validating anything `[S14 §2: CONFIRMED]`.
3. **Events** `[OWN]`, each counted, and logged at most once per episode:
   view changed (with its kind, never the file's text); flat list not
   confirmed, or confirmation no longer matching; scope withheld a query;
   unmatched replies (by reason), case mismatches, moves to TCP for
   forgery; truncation; EDNS fallback; lookup failures by kind; decode
   errors by kind; `NoSourcePort`; cache evictions. The event catalogue
   (`ironafs-events`) names them.
4. **Errors and events carry no bytes from a reply** `[OWN]`, from coding
   standards rule 8 as `xdr.md`, section 7, point 2, applies it: a decode
   error carries its kind, the offset (from the buffer, never a value
   read from it), and this specification's constants. A target host name
   reaches status only after it has passed the host-name rule (5.4,
   point 9).

#### 5.6.9 Concurrency and bounds

1. At most `dns.lookups_max` lookups run at once, and at most
   `dns.waiting_max` wait; a lookup that finds the wait list full ends
   at once with the retryable failure `Busy` `[OWN]`, as ADR 0004, C39,
   point 2, asks of a full executor.
2. No lock is held across a send, a receive or a wait
   `[ADR 0004 C39.4: Working]`.
3. Which thread performs the I/O is section 13, Q1.

## 6. Arithmetic

Every value read from a reply or from the resolver configuration is
untrusted. All of this uses checked arithmetic and fallible conversions
(coding standards rule 2); none of it saturates.

| Id | Operation | Operands' range | Bound and result at overflow |
|---|---|---|---|
| A1 | cursor + n, against the reply's end | cursor ≤ length ≤ 16,384 + 2, n ≤ 65,535 | `checked_add` in `usize`; past the end is `Truncated` |
| A2 | ANCOUNT + NSCOUNT + ARCOUNT | each < 2¹⁶ | summed in `u32`, cannot overflow; above `dns.reply_records_max` is `CountOverCap` |
| A3 | 11 × record count, against the bytes left | count ≤ 256 after A2 | `checked_mul` in `usize`; cannot overflow after A2; short is `CountsExceedReply` |
| A4 | a name's uncompressed length + 1 + label length + 1 (the zero byte still to come) | length so far ≤ 254, label length ≤ 63 | `checked_add` in `usize`; a sum above 255 is `NameTooLong`, checked before the label is read |
| A5 | a pointer's 14-bit target, against the current run's start | both ≤ reply length | integer comparison; no arithmetic |
| A6 | the extended RCODE: (OPT's 8 bits) × 16 + header's 4 bits | ≤ 4,095 | in `u16`; cannot overflow |
| A7 | a TTL to boot-clock nanoseconds | TTL ≤ 2³¹ − 1 after 5.1, point 4, then capped to 86,400 | `checked_mul` by 10⁹ in `u64`; cannot overflow after the cap |
| A8 | now + lifetime, an expiry on the boot clock | 64-bit nanoseconds | `checked_add`; overflow expires the entry now (fails closed) |
| A9 | the lower of two TTLs, or of an SOA TTL and MINIMUM | 32-bit | `min`; no arithmetic |
| A10 | a source port draw | a 16-bit draw | values below 1,024 are drawn again, within the try's 16 draws (5.5, point 5); no arithmetic, no bias |
| A11 | the TCP length prefix to `usize`, and + 2 for framing | < 2¹⁶ | `from`; checked against `dns.tcp_reply_max` before reserving |
| A12 | the query's encoded length | ≤ `dns.query_max` | `checked_add`; above the cap the encoder refuses |
| A13 | the resolver configuration's length and line length | file read with a limit of cap + 1 | a read that returns cap + 1 bytes is unusable; no sum on untrusted values |

`[OWN]` for every row; the operations follow from 5.1 to 5.6.

## 7. Errors and failure

1. **One closed error set for the codec** `[OWN]`. Tests assert the exact
   kind (coding standards rule 15).

   | Kind | When | Effect on the try |
   |---|---|---|
   | `Unmatched` (reason: short, source, id, not a response, opcode, question count, question) | 5.4, points 1 and 2 (UDP) | dropped; the try waits on |
   | `CaseMismatch` | 5.4, point 2.5 | dropped; the query moves to TCP (5.6.5, point 6) |
   | `MessageTooLarge` | a matched UDP reply (5.4, point 2.6); a TCP length prefix (5.4, point 1; 5.6.5, point 9) | try fails |
   | `Truncated` | a TCP length prefix below 12 (5.4, point 1), or a matched reply that ends inside an item (5.4, points 5 and 6) | try fails |
   | `TruncatedOverTcp` | TC set on a TCP reply | try fails |
   | `CountOverCap`, `CountsExceedReply` | 5.4, point 4 | try fails |
   | `BadLabelType`, `NameTooLong`, `PointerNotBackwards`, `PointerIntoHeader`, `PointerHopsExceeded` | 5.4, point 5 | try fails |
   | `RdataLength`, `TrailingBytes` | 5.4, point 6 | try fails |
   | `BadOpt` | 5.4, point 7 | try fails |
   | `CnameConflict`, `CnameLoop`, `CnameChainTooLong` | 5.4, point 6 | try fails |
   | `UnexpectedRcode` | 5.4, point 7 | server failure |
   | `Unusable` (reason) | 5.4, points 8 and 9 | try fails; never a negative |

2. **Query and lookup outcomes**, beyond the reply outcomes of 5.4,
   point 8: `Timeout`, `Unreachable`, `NoSourcePort`, `RandomFailed`,
   `ClockFailed`, `Busy`, `DiscoveryOff`, `WithheldByScope`, `NoServer`,
   `NotLookedUp`, `NoAddress`, `NetworkChanged`, and the lookup outcome
   "No service" (5.6.6, point 7) `[OWN]`.
3. **No content in errors** `[OWN]`: 5.6.8, point 4.
4. **No panic is reachable** from any reply or any resolver configuration
   (coding standards rule 1). No indexing, `unwrap` or `expect` on values
   derived from them.
5. **Fail closed** `[OWN]`, from C12, point 3, and C33, points 4 and 5:
   - an error, a timeout, a truncated or inconclusive reply never
     becomes a negative and never fills the negative cache;
   - a view that cannot be read is "none", which sends only to explicit
     servers;
   - a confirmation that does not match exactly is no confirmation;
   - a scope that cannot be confirmed sends nothing, and a scoped cell
     never falls back to the host's path;
   - a datagram that cannot be matched never fails a try;
   - a failure of the random source or of the clock sends nothing;
   - a reply that is partly bad is wholly unused.

   What is left behind after any failure: the caches unchanged, except
   the failure hold-down entry (5.6.7, point 8); no socket open.

## 8. Security

Who can send or change the inputs:

- the resolver the client asks, and every server it consults: what they
  say is a hint (N5), and RFC 5864 notes that forged answers can point a
  client at a server under an attacker's control `[S10 §7: CONFIRMED]`;
- anyone who can forge or alter datagrams on the path to a server that is
  not on the host;
- a local user, who can send datagrams and open sockets, and may read the
  resolver configuration, but owns neither it nor the daemon's
  configuration.

| Threat | What stops it |
|---|---|
| An off-path forged reply | random ID per try (5.5, point 3), random source port per try (5.5, point 5), random letter case to remote servers (5.5, point 4), a connected socket so only the server's address and port reach it (5.6.5, point 3), full matching before use (5.4, point 2), and a move to TCP on signs of forgery (5.6.5, point 6) `[S9 §9.1 to §9.3: CONFIRMED]` |
| Off-path datagrams sent to end a try early (short, oversized or undecodable) | a UDP datagram is matched before its size is judged, so one that cannot be matched is dropped and counted as a sign of forgery, never a try failure (5.4, points 1 and 2) |
| A forged or malicious answer that points the client at the wrong servers | answers are hints and never raise trust (N5, points 2 and 3); configuration always wins (C33, point 1); tokens go only to their own cell's servers and Kerberos authenticates mutually (ADR 0004, part 3, threat table, "A malicious cell configuration or DNS answer"); addresses on the host itself, of a group, or needing a scope are refused, IPv4-mapped IPv6 addresses included (5.4, point 9) |
| A forged negative that hides a cell | a negative needs a matched, conclusive reply (5.4, point 8.3); only a volume location lookup can give one (5.6.6, point 4); its lifetime is capped at one hour (5.6.7, point 5) and dropped on any network or view change (5.6.7, point 3); a configured cell is never hidden (5.6.7, point 6) |
| Exhausting memory | caps before every reservation (5.2; 5.4, points 1 and 4; 5.6.5, point 9); bounded caches with eviction (5.6.7, point 7); bounded lookups (5.6.9) |
| Exhausting CPU or looping | pointers strictly backwards and a pointer cap (5.4, point 5); record and CNAME caps (5.4, points 4 and 6); every wait bounded by a deadline (5.6.5, point 7) |
| Crashing the daemon | no panics, checked arithmetic (section 6; 7, point 4); fuzz targets (10.4) |
| A query storm from an indexer or a typo | the negative cache (5.6.7, point 6), the failure hold-down (5.6.7, point 8), coalescing (5.6.7, point 9), and single-label names never sent (5.6.6, point 1) |
| A cell's name sent where the host's policy does not send it | only the local resolver when there is one (5.6.3, point 3); a flat list only while exactly confirmed (5.6.3, point 4; 5.6.4, point 3); a cell marked as reachable only through a VPN only to its own servers, while the route to each leaves by its interface, for every query of its lookups, the targets' addresses included (5.6.3, point 2); no search list (5.6.2, point 2) `[S10 §4: CONFIRMED]` |
| A local user setting the confirmation, servers or scope | keys only from root-owned files or policy, never from a run-time override or the control endpoint (5.6.4, point 6) |
| A late reply from a former network applied to the new one | the generation pair on every try (5.6.7, point 4) |
| A reply's bytes reaching a log | errors and events carry no reply bytes; host names only after their rule (5.6.8, point 4) |

**Residuals** `[OWN]`, documented:

- **R1. A program listening on the loopback resolver's address** that is
  not the host's resolver. Ports below 1,024 need privilege on Linux by
  default, so an ordinary user cannot listen on port 53 unless an
  administrator has lowered that limit. A host that has done so trusts
  every local user with its DNS.
- **R2. The same flat list on a different network.** A confirmation
  binds addresses, not networks. A host that moves between networks
  whose resolvers share an address (a home router's common address, for
  example) may send a cell's name to the second network's resolver. This
  is the question the ADR's overview leaves open (open question 1). The
  documentation advises a local resolver on hosts that move.
- **R3. A malicious resolver** can hide a cell or point at other
  servers. That is RFC 5864's stated residual for any DNS-based
  discovery without integrity protection `[S10 §7: CONFIRMED]`; this
  client does not validate DNSSEC (section 2, point 3).
- **R4. A VPN-only cell that is not marked.** The client knows a cell
  is reachable only through a VPN only from `cells.<cell>.dns_scope`
  (5.6.4, point 5). A cell without it, or a name typed under `/afs` that
  is not configured, is resolved on the host's path, which may send its
  name out of the default route. The documentation tells administrators
  of such cells to mark them, or to configure their servers.

## 9. How the invariants are kept

| Invariant (section 1) | Kept by | Test that fails if it is broken |
|---|---|---|
| Constraint 4: no panics, checked arithmetic, byte limits | 5.2; 5.4; section 6; 7, point 4 | fuzz targets `ironafs-dns_reply` and `ironafs-dns_resolv_conf` (10.4); reject corpus (10.3) |
| C3, point 1: protocol code is pure | 5.6.1, points 1 and 2 | `cargo xtask check-pure` and the dependency check, with `ironafs-dns` listed as pure (section 13, Q3) |
| C4, point 6: where the client lives | 5.6.1 | the dependency check: `ironafs-dns` depends on `ironafs-time` only |
| C6, point 1: written from a specification with named sources | this document; every point labelled; section 12 | review of this specification (section 14) |
| C12, point 3: negative cache only from a conclusive negative, no longer than its TTL, never from a timeout or error, cleared on a network change | 5.4, point 8.3; 5.6.6, points 4 and 7; 5.6.7, points 3, 5, 6 and 8 | acceptance cases T5, T6, T7, T8, T9, T17, T31 and T38 (10.5); reject R46 and R48 |
| C33, point 1: configuration, then SRV, then AFSDB; priority first; sole "." means not offered | 5.6.6, points 2, 3 and 8; 5.6.7, point 6 | T2, T3, T4 and T18 (10.5); reject R27 (10.3) |
| C33, point 3: only the local resolver; detected at run time; none required | 5.6.2; 5.6.3, point 3 | parser cases V1 to V12 (10.3); T1 and T10 |
| C33, point 4: flat list only after confirmation; in order; documented timeouts; rechecked on change | 5.6.3, points 4 and 6; 5.6.4, point 3; 5.6.5, points 1 and 7 | T11, T12, T13 and T14 |
| C33, point 5: explicit servers otherwise; VPN scope | 5.6.3, preamble and points 2 and 5; 5.6.4, point 5; 5.6.6, point 5 | T15, T16 and T16a |
| C33, point 6: records and rankings expire together, dropped on a network change, late replies discarded | 5.6.5, point 5; 5.6.7, points 2 to 4 and 10 | T17, T19, T20, T32, T36 and T39 |
| C33, point 8: switches; status of origin and of validation | 5.6.3, point 1; 5.6.4, points 1 and 2; 5.6.8 | T21 and T22 |
| C34, point 3: IPv6 first, IPv4 fallback | 5.6.6, point 5 | T3 |
| C37, point 1: boot clock for lifetimes, awake clock for retransmission | 5.6.5, point 7; 5.6.7, point 1 | T23 and T24 |
| N5, points 2 and 3: DNS answers are hints | 5.6.8, point 2; section 8 | T22; review |
| O17, point 2, with C33, points 4 and 8: keys a local user cannot set | 5.6.4, point 6 | T25 (in `ironafs-config`'s tests, #93) |
| O47, point 3: DNS replies fuzzed | 10.4 | the fuzz targets build and run on the Linux runner |
| ADR 0005, point 6: safe Rust, `std` sockets, no site | 5.6.1, point 2 | the `unsafe` check of ADR 0005, point 5 |

## 10. Tests

All paths are in the `ironafs-dns` crate unless named otherwise, laid out
as coding standards rules 15 and 20 set out. Test names use the reserved
top-level domain `test` `[S11 §6.2: CONFIRMED]`: cell `cell.test`,
targets `db1.cell.test` and `db2.cell.test`; addresses under 192.0.2,
198.51.100 and 2001:db8. Every value is IronAFS's own; none is taken from
a source's examples.

### 10.1 Golden vectors

In `crates/ironafs-dns/tests/it/golden.rs`. Each expected byte string is
derived by hand from 5.1 and 5.5, and the derivation is a comment beside
it. Bytes are in hexadecimal.

**G1. The SRV query for `cell.test`, ID 0x1234, lower case, with OPT.**

```
12 34            ID
01 20            QR 0, opcode 0, AA 0, TC 0, RD 1 | RA 0, Z 0, AD 1, CD 0, RCODE 0
00 01 00 00      QDCOUNT 1, ANCOUNT 0
00 00 00 01      NSCOUNT 0, ARCOUNT 1
0e 5f 61 66 73 33 2d 76 6c 73 65 72 76 65 72    "_afs3-vlserver" (14)
04 5f 75 64 70   "_udp"
04 63 65 6c 6c   "cell"
04 74 65 73 74   "test"
00               root
00 21 00 01      QTYPE 33 (SRV), QCLASS 1 (IN)
00 00 29 04 d0   OPT: root name, TYPE 41, CLASS 1232
00 00 00 00 00 00   TTL 0 (ext. RCODE 0, version 0, DO 0), RDLENGTH 0
```

58 bytes: 12 of header, 31 of name, 4 of type and class, 11 of OPT. The
question name starts at offset 12, and the label `cell` at offset 32. Proves 5.1, points 2 to 5 and 7,
and 5.5, point 1.

**G2.** G1 with a test `Rng` whose bits give `_AfS3-VlSeRvEr._UdP.CeLl.TeSt`
on a UDP try to 192.0.2.53: the letters' bytes become `41 66 53`, `56 6c
53 65 52 76 45 72`, `55 64 50`, `43 65 4c 6c`, `54 65 53 74`; every other
byte as G1. Proves 5.5, point 4. **G2a:** the same `Rng` on a try to
127.0.0.53 gives exactly G1's bytes.

**G3. A reply to G1 with one SRV record.** Header `12 34 81 80 00 01 00 01
00 00 00 01` (QR 1, RD 1, RA 1, RCODE 0; one answer, one additional); the
question as G1, bytes 12 to 46; the answer: `c0 0c` (pointer to the
question name), `00 21 00 01`, TTL `00 00 0e 10` (3,600), RDLENGTH
`00 15` (21), then priority `00 00`, weight `00 00`, port `1b 5b`
(7,003), target `03 64 62 31 04 63 65 6c 6c 04 74 65 73 74 00`
(`db1.cell.test`, 15 bytes); then G1's OPT. Decodes to Records: one SRV,
priority 0, weight 0, port 7,003, target `db1.cell.test`, TTL 3,600.
Proves 5.1, points 4 to 6, and 5.4, points 5 and 6.

**G4. As G3 with the target compressed** (the tolerance of 5.4,
point 5): target `03 64 62 31 c0 20`, a pointer to offset 32 (0x20),
where `04 63 65 6c 6c` begins in the question; RDLENGTH `00 0c` (12).
Decodes to the same value as G3.

**G5. A conclusive negative (name error) with an SOA.** RCODE 3, RA 1,
ANCOUNT 0, NSCOUNT 1: `cell.test` SOA (owner by pointer `c0 20`) with TTL
600 and MINIMUM 300, MNAME and RNAME uncompressed under `cell.test`.
Decodes to a conclusive negative with lifetime 300.

**G6. A NODATA with an SOA** (RCODE 0, no answer, the SOA as G5 with TTL
120 and MINIMUM 900): a conclusive negative with lifetime 120.

**G7. "Not offered":** one SRV record, priority 0, weight 0, port 0,
target the root (`00`), TTL 1,800. Decodes to Not offered, lifetime
1,800. (Port 0 is not checked for a root target: 5.4, point 9, applies
to a host target only.)

**G8. AFSDB:** a query for `cell.test` type AFSDB and a reply with two
records, subtype 1 `db1.cell.test` and subtype 2 `db2.cell.test`. Decodes
to one applicable record (subtype 1); the other is counted as irrelevant.

**G9. AAAA and A** replies for `db1.cell.test` with `2001:db8::1` and
`192.0.2.10`: 16 and 4 bytes of RDATA, decoded to those addresses.

**G10. TCP framing:** G1 framed for TCP: `00 3a` (58) followed by G1's
58 bytes, written in one write (5.5, point 6).

### 10.2 Round-trip properties

In `crates/ironafs-dns/tests/it/props.rs`, with the seeded generator and
shrinker of `ironafs-test-support` (ADR 0004, O47, point 2); each prints
its seed on failure (coding standards rule 15).

- **P1.** For every valid cell name and type the encoder accepts, the
  encoded query decodes (with the decoder's question reader) to the same
  name under case folding, type and class, and its length is at most
  `dns.query_max`.
- **P2.** For every reply the test builder makes from a generated answer
  set (with and without compression, in any record order), decoding gives
  that answer set.
- **P3.** Compressing any name of a generated reply in any valid way
  (pointers only to prior occurrences) never changes the decoded value.
- **P4.** For every generated reply, every truncation to a shorter length
  gives an error or `Unmatched`, never an outcome, and never a negative.
- **P5.** For every `Rng` sequence, every query ID and port is the value
  5.5, points 3 and 5, derive from it, and two tries never share a socket.
- **P6.** For every generated reply, flipping any one bit of the header's
  ID, of QR, of the opcode, or of the question gives `Unmatched` or
  `CaseMismatch`, never an outcome.

### 10.3 Reject corpus

Files under `crates/ironafs-dns/tests/fixtures/reject/`, one per case,
each with its expected error kind and the section it breaks; checked by
`crates/ironafs-dns/tests/it/reject.rs`, which asserts the exact kind and
offset. Each is G3 or G5 changed as stated.

| Id | Input | Expected | Breaks |
|---|---|---|---|
| R1 | a UDP datagram of 11 bytes; a TCP length prefix of 11 | `Unmatched` (short), the try waits on; `Truncated` | 5.4, point 1 |
| R2 | G3 padded to 4,097 bytes as a UDP datagram; the same with ID 0x1235 | `MessageTooLarge`; `Unmatched` (id), the try waits on | 5.4, points 2.2 and 2.6 |
| R2a | on a try without OPT, a matched UDP reply of 513 bytes | `MessageTooLarge` | 5.4, point 2.6 |
| R2b | G3 cut inside its question name | `Unmatched` (question), the try waits on | 5.4, point 2.5 |
| R3 | a TCP length prefix of 16,385 | `MessageTooLarge`, no buffer reserved | 5.6.5, point 9 |
| R4 | ID 0x1235 | `Unmatched` (id) | 5.4, point 2.2 |
| R5 | QR 0 | `Unmatched` (not a response) | 5.4, point 2.3 |
| R6 | opcode 2 | `Unmatched` (opcode) | 5.4, point 2.3 |
| R7 | QDCOUNT 0, and QDCOUNT 2 | `Unmatched` (question count) | 5.4, point 2.4 |
| R8 | QTYPE 1 in the question | `Unmatched` (question) | 5.4, point 2.5 |
| R9 | on G2's try, the question in lower case | `CaseMismatch` | 5.4, point 2.5 |
| R10 | TC set, answer cut inside the record | no error, no outcome: "use TCP"; nothing decoded | 5.4, point 3 |
| R11 | TC set on a TCP reply | `TruncatedOverTcp` | 5.4, point 3 |
| R12 | ARCOUNT 300 | `CountOverCap` | 5.4, point 4 |
| R13 | ANCOUNT 5 with 40 bytes after the question | `CountsExceedReply`, before any record is walked | 5.4, point 4 |
| R14 | the answer's owner `c0 2f`, a pointer to its own offset (47, just after G3's 35-byte question) | `PointerNotBackwards` | 5.4, point 5.3 |
| R15 | two names whose pointers point at each other | `PointerNotBackwards` | 5.4, point 5.3 |
| R16 | a pointer to an offset after the pointer | `PointerNotBackwards` | 5.4, point 5.3 |
| R17 | a name whose labels run on and end with a pointer back into the same run (a loop that "points backwards" from the pointer's own position) | `PointerNotBackwards` | 5.4, point 5.3 |
| R18 | a chain of 17 pointers, each strictly earlier | `PointerHopsExceeded` | 5.4, point 5.4 |
| R19 | a label length byte `40` and `80` | `BadLabelType` | 5.4, point 5.2 |
| R20 | a target of 4 labels of 63 bytes (4 × 64 + 1 = 257 bytes with the root) | `NameTooLong`, before the fourth label is read (192 + 1 + 63 + 1 = 257) | 5.4, point 5.1 |
| R20a | a target of labels of 63, 63, 63 and 62 bytes (256 bytes with the root) | `NameTooLong`, before the fourth label is read (192 + 1 + 62 + 1 = 256) | 5.4, point 5.1 |
| R20b | a target of labels of 63, 63, 63 and 61 letters (255 bytes with the root, 253 characters) | accepted | 5.1, point 5; 5.4, points 5.1 and 9 |
| R21 | RDLENGTH 22 on G3 (one byte past the target) | `RdataLength` | 5.4, point 6 |
| R22 | RDLENGTH running past the reply's end | `Truncated` | 5.4, point 6 |
| R23 | one byte after G3's OPT | `TrailingBytes` | 5.4, point 6 |
| R24 | two OPT records; an OPT in the answer; an OPT with owner `cell.test` | `BadOpt` | 5.4, point 7 |
| R25 | 65 SRV records | `Unusable` | 5.4, point 8.1 |
| R26 | an SRV with port 0 and a host target | `Unusable` | 5.4, point 9 |
| R27 | two SRV records, one with the root target | `Unusable` | 5.4, point 8.2 |
| R28 | a target `db_1.cell.test` | `Unusable` | 5.4, point 9 |
| R29 | A records `0.0.0.0`, `0.0.0.1`, `127.0.0.1`, `169.254.0.1`, `224.0.0.1`, `239.255.255.255`, `240.0.0.1`, `255.255.255.255`; AAAA records `::`, `::1`, `::c000:201` (IPv4-compatible), `::ffff:127.0.0.1`, `::ffff:192.0.2.1`, `fe80::1`, `ff02::1`; one case each | `Unusable` | 5.4, point 9 |
| R29a | an A record `10.0.0.1`; an AAAA `fd00::1` | Records | 5.4, point 9 |
| R30 | an A record with RDLENGTH 5 | `RdataLength` | 5.4, point 6 |
| R31 | a CNAME chain of 9 | `CnameChainTooLong` | 5.4, point 6 |
| R32 | `a` CNAME `b`, `b` CNAME `a` | `CnameLoop` | 5.4, point 6 |
| R33 | two CNAME records owned by one name | `CnameConflict` | 5.4, point 6 |
| R34 | NODATA (RCODE 0, no answer) with NS and no SOA in authority | `Inconclusive` (a referral) | 5.4, point 8.3 |
| R35 | NXDOMAIN with a CNAME in the answer | `Inconclusive` | 5.4, point 8.3 |
| R36 | NXDOMAIN with RA 0 and AA 0 | `Inconclusive` | 5.4, point 8.3 |
| R37 | NXDOMAIN with no SOA | conclusive negative, no lifetime | 5.4, point 8.3 |
| R38 | an SOA in authority owned by `other.test` | not kept; the negative has no lifetime | 5.4, point 6 |
| R39 | RCODE 2; RCODE 5; an extended RCODE making 16 | server failure; `UnexpectedRcode` | 5.4, points 7 and 8.4 |
| R40 | two SRV records with TTLs 600 and 60 | Records, both with TTL 60 | 5.4, point 8.1 |
| R41 | a TTL `80 00 00 10` | TTL 0, not cached | 5.1, point 4 |
| R42 | an AAAA answer of 17 records for `db1.cell.test` | `Unusable` | 5.4, point 8.1 |
| R43 | the answer's owner `c0 05`, a pointer into the header | `PointerIntoHeader` | 5.4, point 5.3 |
| R44 | the SRV answer reached through `_afs3-vlserver._udp.cell.test` CNAME `alias.cell.test` with TTL 30, the SRV record with TTL 3,600 | Records, TTL 30 | 5.4, point 8.1 |
| R45 | G5 with a second kept SOA, owner `test`, TTL 60 and MINIMUM 60 | conclusive negative, lifetime 60 | 5.4, points 6 and 8.3 |
| R46 | G5 with MINIMUM `80 00 01 2c` | conclusive negative, lifetime 0, not cached | 5.4, point 8.3 |
| R47 | G5 with one byte after the SOA's MINIMUM inside its RDLENGTH | `RdataLength` | 5.4, point 6 |
| R48 | G7 reached through `_afs3-vlserver._udp.cell.test` CNAME `alias.cell.test` with TTL 0 | Not offered, lifetime 0, not cached | 5.4, point 8.2 |

**Resolver configuration** cases, `crates/ironafs-dns/tests/it/resolv_conf.rs`
(V1 to V12): stub only (`nameserver 127.0.0.53`) gives local resolver;
`127.0.0.1` and `::1` give local resolver; one loopback and one other give
flat list; four `nameserver` lines keep three and count one; a comment
with `#` and with `;` in the first column; `search` and `options` lines
ignored; a zone `fe80::1%eth0` makes the file unusable; a value that is
not an address, and a line with two values, make it unusable; a line of
1,025 bytes and a file of 65,537 bytes make it unusable; no `nameserver`
line gives none; a missing file gives none; a non-ASCII byte in a
`nameserver` line makes it unusable.

### 10.4 Fuzz targets

In `fuzz/fuzz_targets/` (coding standards rule 20; where `fuzz/` sits
rests on ADR 0004's assumption A12, #45):

- **`ironafs-dns_reply.rs`:** decodes arbitrary bytes as a UDP reply and,
  framed, as a TCP reply, against a fixed try (G1's and G2's) and asserts:
  no panic, overflow or out-of-bounds access; every reservation within
  the caps of 5.2; an error's offset, where it has one, no more than the
  input's length; the same input always gives the same result; and an
  outcome that is a conclusive negative only when the input's RCODE is 0
  or 3, TC is clear and the reply matched.
- **`ironafs-dns_resolv_conf.rs`:** parses arbitrary bytes as a resolver
  configuration and asserts: no panic; at most three servers; a local
  resolver only when every server is loopback.

Seed corpora: the golden vectors and the reject corpus. Fuzzing runs on
the Linux runner (ADR 0004, O47, point 4).

### 10.5 Acceptance cases

Behaviour cases, run against `ironafs-dns`'s state machine with a
scripted network, the seeded `Rng` and the simulated clocks, in
`crates/ironafs-dns/tests/it/behaviour.rs`; T25 is in `ironafs-config`.
Each: start state, events in order, result.

- **T1.** View local resolver `127.0.0.53`; lookup `cell.test`. One SRV
  query, to `127.0.0.53` only, lower case, with OPT. Then G3; then AAAA
  and A for `db1.cell.test`. Servers, path "local resolver".
- **T2.** As T1, SRV answers G5 (NXDOMAIN, lifetime 300); AFSDB answers G6
  (NODATA, lifetime 120). Not a cell, lifetime 120; negative cache filled.
- **T3.** As T1 with two SRV targets of priorities 10 and 0. Address
  queries in priority order, AAAA before A for each; the result lists
  priority 0 first and each target's IPv6 address before its IPv4.
- **T4.** SRV answers G7. Not a cell (not offered); no AFSDB query sent.
- **T5.** SRV query times out on every try. Failed (`Timeout`); no AFSDB
  query; negative cache empty; failure hold-down set for 5 s; a second
  lookup within 5 s sends nothing and fails the same way; one after 5 s
  sends queries.
- **T6.** SRV answers R10 (TC set). The query goes to TCP at once; TCP
  answers G5. Negative from the TCP reply only; nothing from the UDP reply
  was used.
- **T7.** SRV answers R37 (NXDOMAIN, no SOA), AFSDB answers G6. Not a
  cell, no lifetime; negative cache stays empty.
- **T8.** Each of R34, R35, R36 as the SRV reply. No AFSDB query; Failed
  (`Inconclusive`); negative cache empty.
- **T9.** T2, then the network generation moves. Negative cache empty;
  the next lookup sends queries.
- **T10.** The resolver configuration changes from stub-only to a flat
  list between two lookups (new inode). The second lookup sees the new
  view, sends nothing without a confirmation, and the view generation has
  moved.
- **T11.** Flat list `192.0.2.53, 192.0.2.54`, no confirmation, no
  explicit servers. No datagram is sent; Failed (`NoServer`); event "flat
  list not confirmed".
- **T12.** Same list, confirmed as `192.0.2.53, 192.0.2.54`; both
  servers time out. Tries go to `.53`, `.54`, then `.53`, `.54` again, in
  that order, and stop: two rounds. Each try's name has random case, and
  each try has its own port and ID.
- **T13.** Confirmed `192.0.2.53, 192.0.2.54`; the view becomes
  `192.0.2.54, 192.0.2.53` (same addresses, new order). No datagram sent;
  event "confirmation no longer matches".
- **T14.** Confirmed list matches; a network change moves the generation;
  the view is read again, still matches; queries go on, and every cache
  was dropped.
- **T15.** View none; `discovery.servers = 192.0.2.60`. Queries go only to
  `192.0.2.60`.
- **T16.** View local resolver `127.0.0.53`. Cell with
  `dns_scope = servers`, server `198.51.100.53`, interface `vpn0`; the
  scripted route source says the route leaves by `eth0`. No datagram is
  sent; Failed (`WithheldByScope`). With the route on `vpn0`, the SRV
  query and then the AAAA and A queries for its target go to
  `198.51.100.53` only; `127.0.0.53` receives nothing. With
  `dns_scope = servers` and no `dns_interface`, nothing is sent.
- **T16a.** As T16 with the route on `vpn0`; the route moves to `eth0`
  while the SRV try is in flight, and the network generation moves. The
  reply is discarded, the lookup starts again and ends
  `WithheldByScope`; no datagram is sent after the move.
- **T17.** A lookup's SRV try is in flight when the network generation
  moves; the reply then arrives, matched. It is discarded; the lookup
  starts again and its outcome comes from the new state.
- **T18.** `cell.test` is in the negative cache; configuration then adds
  it. The next lookup of `cell.test` uses the configuration; no query.
- **T19.** Servers with lifetime 60 from T1. At boot-clock 59 s the
  entry is used; at 60 s it is gone, and the location service has been
  told to drop its rankings.
- **T20.** A reply to try 1 arrives after try 1 has ended and try 2 has
  begun: try 1's socket is closed, so the reply is never decoded.
- **T21.** `discovery.enabled = false`, and separately
  `cells.cell.test.discovery = false`: no datagram; Failed
  (`DiscoveryOff`).
- **T22.** G3 with AD set: status shows the path, the server, the record
  type and "AD: set by the resolver, not relied on"; nothing about the
  cell's trust or floor changes.
- **T23.** A UDP try in flight; the awake clock stands still while the
  boot clock advances 1 hour (a suspend). The lookup ends `Failed`
  (deadline) on the boot clock; the try timer had not fired.
- **T24.** Cached Servers with lifetime 600; the boot clock advances 601 s
  while the awake clock advances 1 s. The entry is expired.
- **T25.** In `ironafs-config` (#93): `discovery.flat_list_confirmed` in a
  file owned by a non-root user, or writable by group or others, or under
  such a directory, is not taken; the same key in OS policy is taken and
  locks the key.
- **T26.** Four unmatched replies (wrong ID) during one UDP try: the try
  goes on. A fifth: the query moves to TCP. A `CaseMismatch`: it moves at
  once.
- **T27.** OPT query answered with RCODE 1 and no OPT: one more try to the
  same server without OPT, ARCOUNT 0.
- **T28.** The `Rng` fails: no datagram is sent; Failed (`RandomFailed`).
  The port draws are all refused 16 times: Failed (`NoSourcePort`). An
  `Rng` whose 16 draws are all below 1,024: Failed (`NoSourcePort`).
- **T29.** 73 lookups at once of different cells, with
  `dns.lookups_max` of 8 and `dns.waiting_max` of 64: eight run, 64 wait,
  and the 73rd ends `Busy`. Two lookups of the same cell and service
  share one set of queries; a volume location and a protection-server
  lookup of one cell do not.
- **T30.** `cell` (one label): no query; Failed (`NotLookedUp`); the
  negative cache and the failure hold-down stay empty.
- **T31.** A protection-server lookup of `cell.test`: the SRV query for
  `_afs3-prserver._udp.cell.test` answers "not offered" (as G7). No
  service; the negative cache stays empty; a volume location lookup of
  `cell.test` then sends its SRV query. Again with SRV answering G5 and
  AFSDB G6: No service, the negative cache stays empty.
- **T32.** As T3; the AAAA and A queries for the priority-10 target time
  out. Servers, with that target unresolved and lifetime 30; a lookup at
  boot-clock 29 s is answered from the positive cache, one at 30 s sends
  queries. Separately, every target's address queries time out: Failed
  (`NoAddress`); the positive cache stays empty; the hold-down is set.
- **T33.** Two callers wait on one lookup, one with a deadline 50 ms away
  and one with none; the server answers after 1 s. The first gets
  `Timeout` at 50 ms; the second gets Servers; the positive cache is
  filled; no hold-down is set.
- **T34.** One server. The OPT try is answered with RCODE 1 and no OPT;
  the try without OPT times out; the second round sends one try without
  OPT, which times out. Three UDP tries in all, the last two without
  OPT; Failed (`Timeout`).
- **T35.** As T17, with the lookup deadline passing at the move: Failed
  (`NetworkChanged`); no hold-down is set, and the next lookup sends
  queries.
- **T36.** Servers cached from T1. The resolver configuration is
  rewritten (new inode) to list `127.0.0.54` only. The next lookup reads
  the view again before the caches, the view generation moves, the
  positive cache is empty, and the SRV query goes to `127.0.0.54`.
- **T37.** During one UDP try, three datagrams of 11 bytes and two of
  4,097 bytes with a wrong ID arrive from the server's address. Each is
  dropped as `Unmatched`; none fails the try; after the fifth, the query
  moves to TCP (5.6.5, point 6).
- **T38.** A protection-server Servers entry for `cell.test` is cached
  with lifetime 3,600; the volume location entry has expired. A volume
  location lookup ends "Not a cell" with lifetime 300 (as T2). The
  protection-server entry is dropped, and a protection-server lookup then
  ends "No service" from the negative cache, with no query.
- **T39.** Servers cached from T1. `/etc/resolv.conf` is deleted, with no
  network change. The next lookup finds the identity "missing", reads the
  view as none, moves the view generation and empties the caches; with no
  explicit servers it ends `NoServer` and sends nothing.

### 10.6 Gaps

- T16's route answer depends on the network-generation source (#97),
  MISSING until specified there (section 13, Q4).
- The production driver's tests (real sockets on loopback, a local test
  resolver) depend on the thread and seam decisions (section 13, Q1 and
  Q2).
- The observation per supported distribution (section 12.1).

## 11. Revisit if

From the working decisions this rests on:

- ADR 0004, C33: "the oldest supported Linux release ships systemd 256
  or later (then the resolver's own record interface is an option
  `[DES-003]`), the M2 tests show an out-of-scope query on any supported
  configuration, or the status of RHEL's local resolver changes
  `[DES-007]`."
- ADR 0004, C12: "a supported desktop's indexer or file manager still
  contacts cells from a listing of `/afs`, measured in M2a."
- ADR 0004, C3: "the M2a simulator cannot replay a failing schedule from
  its seed, or a measured workload needs more concurrent waits than a
  bounded pool of threads can hold."
- ADR 0004, C4: "a boundary below forces a dependency that point 4
  forbids, or the first implementation of a crate shows it has no content
  of its own."
- ADR 0004, C34: "assumption A11 fails, or a negotiated extension (N22)
  supplies fileserver addresses other than IPv4."
- ADR 0004, C37: "the M2 sleep, hibernation, Modern Standby and VM-pause
  tests show a clock below does not behave as its documentation says, or
  a supported target lacks a valid pair."
- ADR 0004, C39: "M2 load tests show the fixed set starves a class of
  work, or lock-order checks find an RPC wait under a lock that cannot be
  restructured."
- ADR 0004, O17: "a supported management system cannot deliver policy
  through the mechanism named here."
- ADR 0004, O47: "the fuzzing engine's platform limits change, or A12 is
  settled differently when the first fuzz target lands."
- ADR 0005, point 6: "C33 is revisited."

This specification's own choices:

- a supported distribution's established local resolver is observed
  writing a resolver configuration that is not loopback-only (5.6.2,
  point 5);
- a supported local resolver is shown to keep a query's letter case: the
  random case may then extend to loopback tries (5.5, point 4); or a
  remote resolver class is shown not to, often enough that the move to
  TCP costs too much (5.6.5, point 6);
- a real cell is found whose SRV answer, target names or address count
  break a cap or the host-name rule (5.2; 5.4, point 9);
- a way to see where a local resolver sends a name becomes usable on
  every supported release, such as systemd's record interface
  `[DES-003]`: a scope that keeps a VPN-only cell on the local resolver
  may then be specified (5.6.3, point 2);
- a supported distribution is seen writing a `nameserver` value with an
  interface zone (`%`), which makes the file unusable here (5.6.2,
  point 3);
- the observation shows a supported resolver sends a reply with trailing
  bytes or other tolerated-elsewhere oddities (5.4, point 6);
- any registered source is updated, obsoleted, or gains an erratum that
  changes a rule cited here (register rows E3 to E28);
- a non-Linux Unix build is planned: its own resolver configuration page
  is registered first (section 13, Q7).

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| S1 | SPECIFICATION | RFC 1035, "Domain names - implementation and specification", November 1987, register row E3, <https://www.rfc-editor.org/rfc/rfc1035> | 4, 5.1, 5.2, 5.4, 5.5, 5.6, 10 |
| S2 | SPECIFICATION | RFC 1183, "New DNS RR Definitions", October 1990, §1 (AFSDB), register row E4 | 5.1, 5.4 |
| S3 | SPECIFICATION | RFC 2181, "Clarifications to the DNS Specification", July 1997, register row E5 | 5.1, 5.4, 5.6.5, 5.6.7 |
| S4 | SPECIFICATION | RFC 2308, "Negative Caching of DNS Queries (DNS NCACHE)", March 1998, register row E6 | 5.4, 5.6.5, 5.6.6, 5.6.7 |
| S5 | SPECIFICATION | RFC 2782, "A DNS RR for specifying the location of services (DNS SRV)", February 2000, register row E7 | 5.1, 5.4, 5.6.6 |
| S6 | SPECIFICATION | RFC 3596, "DNS Extensions to Support IP Version 6", October 2003, register row E8 | 5.1, 5.4 |
| S7 | SPECIFICATION | RFC 3597, "Handling of Unknown DNS Resource Record (RR) Types", September 2003, §4, register row E9 | 5.4 |
| S8 | SPECIFICATION | RFC 4343, "DNS Case Insensitivity Clarification", January 2006, register row E10 | 5.4 |
| S9 | SPECIFICATION | RFC 5452, "Measures for Making DNS More Resilient against Forged Answers", January 2009, register row E11 | 5.4, 5.5, 5.6.5, 5.6.7, 8 |
| S10 | SPECIFICATION | RFC 5864, "DNS SRV Resource Records for AFS", April 2010, register row E12; claim record rows `[DES-008]` `[DES-010]` | 3, 5.1, 5.6.6, 5.6.7, 8 |
| S11 | SPECIFICATION | RFC 6761, "Special-Use Domain Names", February 2013, §6.2, register row E13 | 10 |
| S12 | SPECIFICATION | RFC 6840, "Clarifications and Implementation Notes for DNS Security (DNSSEC)", February 2013, §5.7 and §5.8, register row E14 | 5.5 |
| S13 | SPECIFICATION | RFC 6891, "Extension Mechanisms for DNS (EDNS(0))", April 2013, register row E15 | 5.1, 5.2, 5.3, 5.4, 5.5, 5.6.5 |
| S14 | SPECIFICATION | RFC 6895, "Domain Name System (DNS) IANA Considerations", April 2013, §2, register row E16 | 5.1, 5.6.8 |
| S15 | SPECIFICATION | RFC 7766, "DNS Transport over TCP - Implementation Requirements", March 2016, register row E17 | 5.5, 5.6.5 |
| S16 | SPECIFICATION | RFC 8200, "Internet Protocol, Version 6 (IPv6) Specification", July 2017, §5, register row E18 | 5.2 |
| S17 | SPECIFICATION | RFC 9619, "In the DNS, QDCOUNT Is (Usually) One", July 2024, register row E19 | 5.1, 5.4 |
| S18 | SPECIFICATION | RFC 9715, "IP Fragmentation Avoidance in DNS over UDP", January 2025, register row E20 | 5.2 |
| S19 | SPECIFICATION | "Use of Bit 0x20 in DNS Labels to Improve Transaction Identity", draft-vixie-dnsext-dns0x20-00, March 2008 (expired Internet-Draft), register row E21 | 5.4, 5.5, 5.6.5 |
| S20 | SPECIFICATION | RFC 9520, "Negative Caching of DNS Resolution Failures", December 2023, §3.1 and §3.2, register row E25 | 5.6.5, 5.6.7 |
| S21 | SPECIFICATION | RFC 4291, "IP Version 6 Addressing Architecture", February 2006, §2.4, §2.5.5.1 and §2.5.5.2, register row E26 | 5.4 |
| S22 | SPECIFICATION | RFC 6890, "Special-Purpose IP Address Registries", April 2013, §2.2.2 and §2.2.3, register row E27 | 5.4 |
| S23 | SPECIFICATION | RFC 5771, "IANA Guidelines for IPv4 Multicast Address Assignments", March 2010, §3, register row E28 | 5.4 |
| M1 | DOCUMENTATION | resolv.conf(5), Linux man-pages 6.19, page dated 2026-08-22, register row E22, <https://man7.org/linux/man-pages/man5/resolv.conf.5.html> | 3, 4, 5.2, 5.6.2 |
| M2 | DOCUMENTATION | systemd-resolved.service(8), systemd 262, register row E23, <https://www.freedesktop.org/software/systemd/man/latest/systemd-resolved.service.html>; claim record row `[DES-002]` | 3, 5.6.2 |
| M3 | DOCUMENTATION | NetworkManager.conf(5), NetworkManager 1.58.0, register row E24, <https://networkmanager.dev/docs/api/latest/NetworkManager.conf.html> | 3, 5.6.2 |
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; C3, C4, C6, C12, C33, C34, C37, C39 ([part 1](../adr/0004-architecture-1-core.md)); O17, O47 and the threat table ([part 3](../adr/0004-architecture-3-operations.md)); N5 ([part 4](../adr/0004-architecture-4-negotiation.md)); the overview's open question 1 | 1, 2, 5.6, 8, 9, 11 |
| D2 | DECISION | [ADR 0005](../adr/0005-ironafs-os.md), Decision, points 5 and 6, and Decision 3, point 1 | 1, 5.2, 5.4, 5.6.1 |
| D3 | DECISION | [ADR 0001](../adr/0001-licence-and-dont-copy.md), "Don't copy" and "Test tools and test data" | 10, 12 |
| D4 | DECISION | [Coding standards](../coding-standards.md), rules 1 to 3, 7, 8, 15 and 20, and the `wire-format` skill | 5.2, 5.6, 6, 7, 10 |
| D5 | DECISION | [XDR profile](xdr.md), section 7, point 2, for the rule that errors carry no input values | 5.6.8 |
| O | OWN | Every point tagged `[OWN]`, with its reason in place | 5, 6, 7, 8 |

No other DNS implementation's source, test data or examples were read for
this specification, and none are cited: not a C library's resolver, not a
resolver daemon's, not any library's. The resolver configuration's format
comes from its manual page only. No AFS implementation's source was read.
The specification states the protocol in IronAFS's own words and quotes
none of the sources' text (register rows E3 to E28). The test names,
addresses and values of section 10 are IronAFS's own; none is a source's
example.

### 12.1 Observed behaviour

None yet. Two observations are planned, each on a test venue with
synthetic names, through the observation work (#51), and neither blocks
this specification, because a failure of either only reduces discovery
and never sends a query where the host's policy does not:

- on each supported Linux release, with its default local resolver: that
  the resolver configuration lists only loopback addresses (5.6.2,
  point 5). Red Hat's own page for dnsconfd could not be read on
  2026-11-21 (the request was refused), so for RHEL 10 the evidence today
  is NetworkManager's documentation `[M3 "dns": CONFIRMED]`;
- for each supported local resolver: whether it keeps a query's letter
  case in its reply (section 11).

## 13. Open questions

- **Q1. The driver's thread.** ADR 0004, C39, point 1, lists a fixed set
  of threads, and none for DNS. The standard library cannot wait on
  several sockets from one thread without a site, so a lookup's waits
  need a thread of their own or a worker's. Proposal: one `dns` thread in
  `ironafs-net` running the lookups of 5.6.9 in turn, with
  `dns.lookups_max` concurrent waits served by a small fixed pool, which
  amends C39, point 1 (Working). Blocks the driver, not the codec or the
  state machine. To be settled with the seams design (#71).
- **Q2. The seam.** C3, point 3, names four injected interfaces (`Rpc`,
  `Clock`, `Rng`, `Disk`). Operation code that runs a lookup, and the
  simulator that replays it, need a fifth (a resolver) or a route through
  one of them. The seams design (#71) settles it; this specification
  defines only the state machine both sides drive.
- **Q3. The crate row.** C4, point 6, leaves the crate's place to this
  specification, which settles a new pure crate, `ironafs-dns` (5.6.1).
  C4, point 1's crate table and the dependency check's crate list gain
  the row; whether that is written as an amendment of C4, point 1
  (Working) with this specification, or with the M2a amendments (#53), is
  open.
- **Q4. The route answer for scoped cells.** 5.6.3, point 2, needs the
  network-generation source (#97) to say through which interface the
  route toward an address leaves. MISSING until its specification states
  it; until then scope `servers` sends nothing.
- **Q5. The flat list on a moving host** (residual R2). The ADR's
  overview leaves open whether a confirmation is enough (open question 1).
  This specification implements C33, point 4, as written and documents
  the residual.
- **Q6. Per-release observation** of each supported distribution's
  resolver configuration (12.1). Not a blocker: the rule fails closed.
- **Q7. Other Unix systems.** Only Linux's resolv.conf(5) is registered.
  Before a build for another Unix system (FreeBSD is a candidate in ADR
  0004), that system's manual page is registered and 5.6.2 re-checked
  against it.

## 14. Review

The review record is
[2026-11-21-m2a-60-dns-client](../reviews/2026-11-21-m2a-60-dns-client.md).
