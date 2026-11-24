<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The network-generation source

Status: draft (2026-11-21). Raised in issue #97.

Kind: behaviour and format (the route-netlink messages the Linux monitor
reads). Class: S: grants (C23, point 2) and per-server trust (C32,
point 1) depend on it, and its decoder reads input from outside the
process.

Sections keep their numbers once the specification is reviewed. A section
that does not apply stays, with one line saying why.

## 1. What this implements

- ADR 0004, C23, point 2 (**Working**): "A grant also binds the **network
  generation**: the client's addresses and routes toward the server. It is
  captured when the authorising request is sent and must still match when
  the grant is minted and at delivery. A change discards the reply,
  authorises again and fences outstanding grants."
- ADR 0004, C12, point 3 (**Working**): the negative cache is cleared on a
  network change.
- ADR 0004, C32, point 1 (**Working**): trust is earned only by a reply
  "since the last network change or resume".
- ADR 0004, C33, points 4 and 6 (**Working**): the flat-list confirmation
  "is rechecked when the network or VPN policy changes"; cached records
  and rankings "are dropped on a network change, and late replies from an
  earlier network state are discarded".
- ADR 0005, sheet 23 (`linux-rtnetlink`), its caller's obligations: "subscribe
  before the first snapshot and reconcile notifications received during
  it; on overflow, truncation, an interrupted dump or a monitor failure,
  invalidate the consumers at once and take a fresh verified snapshot
  before reusing state; ignore only lifetime refreshes that leave an
  address's usability unchanged."
- The owner's decision (2026-11-20) recorded in issue #97: rtnetlink, with
  separate notification and request sockets, watching links, addresses,
  routes and policy rules. Only the operating system's own address and
  route state moves the generation; an ICMP error never does.

## 2. Scope

Covered: what the generation is (5.1), the counter and how it is read
(5.2), the consumer hooks and their order (5.3), the Linux monitor: the
messages it reads (5.4 to 5.6), its states (5.7), and what it does on each
failure (section 7).

Not covered, and where it is:

- each consumer's own effect of a move: the issues that wire each one to
  this source (the grant, M2a-24; DNS records, rankings and late replies,
  M2a-26 and M2a-52; per-server trust, M2a-27; the negative cache,
  M2a-28);
- the route answer the DNS specification asks for (which interface the
  route toward an address leaves by, `docs/specs/dns-client.md`, section
  13, Q4): a request-and-reply on the request socket, specified in a later
  part of #97 (section 13, Q1). Until then a scoped cell sends nothing, as
  that specification already says;
- how a suspend is detected (the clock pair, ADR 0004 C37): this source
  only takes the event (5.7, point 6);
- macOS and Windows sources: their layers' milestones.

## 3. The design in short

1. The network generation is a counter. It moves whenever the host's
   links, addresses, routes, policy rules or route nexthops change, and
   whenever the monitor cannot be sure it saw every change.
2. A move calls every consumer's hook first and only then publishes the
   new value, so no reader sees a new value whose consumers have not yet
   dropped what the old one let them keep.
3. On Linux one thread receives on a route-netlink notify socket, joined
   before anything is read, and a second on a request socket for dumps
   (5.7, point 7). The monitor keeps a table of links and of addresses
   only to tell a lifetime refresh from a change.
4. Anything lost (an overflow, a truncated or malformed datagram, an
   interrupted dump, a failed socket, a resume) moves the generation at
   once and starts a fresh snapshot; finishing a snapshot moves it again.

## 4. Terms

- **Network generation:** the published value of the counter (5.2).
- **Move:** one step of the counter, with its cause (5.3).
- **Consumer hook:** the function one consumer registers to be told of a
  move (5.3).
- **Snapshot:** a link dump followed by an address dump on the request
  socket, building fresh tables (5.7).
- **Refresh:** a "new address" message for an address already in the
  table (the same link, family, prefix length, address and peer, 5.4,
  point 4), with the same flags and scope.
- **Loss:** any event after which the monitor cannot know it saw every
  change (section 7).

## 5. Specification

### 5.1 What the generation covers

1. The generation stands for the client's addresses and the routes toward
   any server (C23, point 2), so it moves on any change of `[OWN]`:
   - a link's existence, its usability (the `IFF_UP`, `IFF_RUNNING`,
     `IFF_LOWER_UP` and `IFF_DORMANT` flags and the operational state,
     `IFLA_OPERSTATE`), or what routing can match on it: its group
     (`IFLA_GROUP`, which a policy rule's `suppress_ifgroup` uses), its
     master (`IFLA_MASTER`, a VRF among them) and its name
     (`IFLA_IFNAME`, which a scoped DNS cell names). Other link changes
     (statistics, the MTU, the link-layer address, wireless events) leave
     addresses and routes as they were and do not move it;
   - an address's existence, or its flags or scope; a deprecated address
     carries `IFA_F_DEPRECATED`, so deprecation is a flag change;
   - any route, policy rule or nexthop object, of either family, added,
     changed or removed.
2. The monitor does not ask whether a route is toward a known server: any
   route change moves it `[OWN]`. Narrowing would need the route answer
   (section 2) and a list of servers; the cost is more moves on a host
   whose routes change often (C23's "Revisit if").
3. **Nexthop objects** `[OWN]`, proposed here: ADR 0005, sheet 23, makes
   the group set final only by an amendment of row 23 accepted by the
   owner; this point is the proposal that amendment restates (5.8,
   point 9), and stays a proposal until it is accepted. Routes can point at a nexthop object (Linux 5.3 and later), and a
   change to that object changes where those routes lead. Whether the
   kernel also reports each route using the object is a run-time setting
   (`net.ipv4.nexthop_compat_mode`, ip-sysctl in the kernel's
   documentation) that routing daemons may turn off. So the notify socket
   joins the nexthop group too (`RTNLGRP_NEXTHOP`, group 32, mask bit
   `1 << 31`, which `nl_groups` can still carry; the ABI check proves the
   number against the build's `<linux/rtnetlink.h>`, which must therefore
   be from Linux 5.3 or later, or carry the backport). On a kernel without
   the group the bit names no group; whether its bind then succeeds is
   UNVERIFIED: `netlink(7)` documents the mask, not this case
   (section 13, Q5). If it failed, the monitor would not start, which
   fails closed.
4. Neighbour entries, multicast routes and the IPv6 prefix and
   router-advertisement groups are not watched `[OWN]`: none changes an
   address or a route toward a server.
5. An ICMP error never moves the generation (C30, point 6, with ADR 0006).

### 5.2 The counter

1. The counter is a 64-bit unsigned integer. Values 1 to `2^64 - 2` are
   generations; `2^64 - 1` means **unavailable**. It starts unavailable;
   the first move publishes 1 `[OWN]`. It is also unavailable from a loss
   until the fresh snapshot after it completes (5.7, point 6): ADR 0005,
   sheet 23, "take a fresh verified snapshot before reusing state". The
   moves in that time are told to every consumer but not published.
2. Each move adds exactly one, so generations are strictly increasing and
   never reused `[OWN]`.
3. A move that would publish `2^64 - 1` is **exhaustion**: the source
   stops (point 4) instead `[OWN]`. At one move per microsecond this takes
   over half a million years; it is defined only so the counter is
   bounded.
4. **Stopped.** When the source stops (exhaustion, a fatal error, or its
   owner dropping it), the published value becomes unavailable and every
   hook is called with "stopped". **Halted:** a driver unwinding from a
   hook's panic, or finding its lock poisoned by one, stops the source
   without calling the hooks again. A read then fails, and a consumer
   holding a generation sees it no longer current. It fails closed
   `[OWN]`.
5. A reader reads the published value with acquire ordering; the
   publisher stores it with release ordering after the hooks return
   (5.3, point 2).

### 5.3 Consumer hooks

1. The consumers are a closed set, each with its own hook, all required
   when the source is made `[OWN]`, so a source without one of them
   cannot be built:

   | Hook | Consumer and its effect (owned by its issue) |
   |---|---|
   | negative cache | cleared (C12, point 3; M2a-28) |
   | grants | the grant's network generation; outstanding grants fenced (C23, point 2; M2a-24) |
   | server trust | per-server trust revalidated (C32, point 1; M2a-27) |
   | DNS records | cached records discarded (C33, point 6; M2a-52) |
   | DNS rankings | location records and rankings dropped (C33, point 6; M2a-26) |
   | DNS late replies | replies to requests sent before the move refused (C33, point 6; M2a-26) |
   | DNS flat list | the flat-list confirmation checked afresh (C33, point 4; M2a-52) |

2. **Order.** For each move, every hook is called, in the table's order,
   with the new generation and the cause, and only then is the new value
   published. A hook that reads the generation during its call reads the
   old one `[OWN]`, from the issue's failure case "an event raised after
   a consumer has already used the old state".
3. Hooks run one at a time, under the monitor's lock, on whichever
   thread hands the monitor its event: the driver's threads, the thread
   that reports a resume, or the one that drops the handle (5.7,
   point 7). They must not block on the network or on a lock the
   source's readers hold `[OWN]`. A hook on one of the driver's threads
   may drop the handle: that thread holds the lock, so the drop only
   asks the source to stop. The stop happens once the move being told has
   reached every consumer: that move is not published, and no further
   record is applied (5.7, point 7).
4. The causes are: a change (5.1), a snapshot completed (5.7, point 4),
   and each loss of section 7, named.
5. Until the first snapshot completes, no hook is told of any move
   `[OWN]`: no generation has been readable, so no consumer holds anything
   under one, and the first completion's move tells every consumer. A
   stop is the exception: it is told to every hook whenever it happens
   (5.2, point 4), the first snapshot not yet completed included.

### 5.4 Layout read

All integers are in the host's byte order (`netlink(7)`). Fields and
sizes are from the kernel's user-space headers `<linux/netlink.h>`,
`<linux/rtnetlink.h>`, `<linux/if_link.h>` and `<linux/if_addr.h>`
`[uapi: CONFIRMED]`.

1. **Message header** (`struct nlmsghdr`, 16 bytes): length (u32,
   header included), type (u16), flags (u16), sequence (u32), port id
   (u32). Messages follow one another in a datagram, each starting at the
   previous one's length rounded up to a multiple of 4 (`NLMSG_ALIGN`).
2. **Attribute** (`struct rtattr`, 4 bytes): length (u16, header
   included), type (u16), then the value; attributes follow one another,
   each at the previous length rounded up to 4 (`RTA_ALIGN`).
3. **Link** (`RTM_NEWLINK` 16, `RTM_DELLINK` 17): `struct ifinfomsg`,
   16 bytes: family (u8), pad (u8), type (u16), index (i32), flags (u32),
   change mask (u32); then attributes, of which these are read:
   `IFLA_IFNAME` (3, the name, its terminating zero included, at most 16
   bytes, `IFNAMSIZ`), `IFLA_MASTER` (10, u32), `IFLA_OPERSTATE` (16, one
   byte) and `IFLA_GROUP` (27, u32). An attribute absent is recorded as
   absent, which differs from any value: the kernel sends `IFLA_MASTER`
   only for a link that has a master.
4. **Address** (`RTM_NEWADDR` 20, `RTM_DELADDR` 21): `struct ifaddrmsg`,
   8 bytes: family (u8), prefix length (u8), flags (u8), scope (u8),
   index (u32); then attributes, of which `IFA_ADDRESS` (1), `IFA_LOCAL`
   (2) and `IFA_FLAGS` (8, u32, which replaces the one-byte flags when
   present) are read. The address is `IFA_LOCAL` when present, else
   `IFA_ADDRESS`; the peer is `IFA_ADDRESS` when present, else `IFA_LOCAL`
   (`<linux/if_addr.h>`: on a point-to-point link `IFA_ADDRESS` is the
   peer's). An address is identified by its link, family, prefix length,
   address and peer: two addresses that share their own address but not
   their peer are two entries, and a peer that changes is a change.
5. **Route** (24 to 26), **rule** (32 to 34) and **nexthop** (104 to 106)
   messages are recognised by type alone; their bodies are not read.
6. **Control messages:** `NLMSG_NOOP` (1), `NLMSG_ERROR` (2: a signed
   32-bit error, then the request's 16-byte header; error 0 acknowledges
   that request, any other value refuses it), `NLMSG_DONE` (3: a signed
   32-bit status; a negative errno says the dump failed part-way, which
   the kernel reports there and not as `NLMSG_ERROR`, from the kernel's
   netlink documentation, `userspace-api/netlink/intro`, not EXECUTED),
   `NLMSG_OVERRUN` (4). The flag `NLM_F_DUMP_INTR` (0x10) on a dump's
   message says the dump was inconsistent.
7. **Dump request** (encoded, 20 bytes): a header of length 20, type
   `RTM_GETLINK` (18) or `RTM_GETADDR` (22), flags `NLM_F_REQUEST |
   NLM_F_DUMP` (0x301), the monitor's sequence number, port id 0; then a
   `struct rtgenmsg` of family `AF_UNSPEC` (0), padded with three zero
   bytes to 4.

### 5.5 Caps

1. A datagram is at most the receive buffer, 65,536 bytes `[OWN]`. A
   larger one is truncated by the socket and is a loss (section 7); the
   site reports it, and nothing of it is decoded.
2. The decoder allocates nothing; it walks the caller's bytes.
3. The link table holds at most 1,024 links and the address table at most
   4,096 addresses `[OWN]`. While a table is over its cap, every message of
   its kind moves the generation (no refresh can be recognised), so the cap
   bounds memory without missing a change.
4. A snapshot is retried after a loss; 8 failed snapshots in a row, with
   no snapshot completed between them, stop the source (5.2, point 4)
   `[OWN]`, so a kernel that keeps refusing cannot keep the thread
   spinning. A failed snapshot is a failure of its socket or of the
   kernel's answer (section 7). A dirty snapshot (5.7, point 3) is not
   one: the host's own changes, however many, never stop the source; a
   dirty snapshot is taken again only once its barrier has come back, so
   churn costs one snapshot per round trip.

### 5.6 Decoding

The checks, in order, for each datagram. Every length is checked before
any byte it covers is read, and every sum uses checked arithmetic.

1. **Messages** (`NLMSG_OK`): at least 16 bytes remain; the header's
   length is at least 16; the length is at most the bytes remaining.
   Otherwise the datagram is malformed. The next message starts at the
   length rounded up to 4; if that passes the end, the datagram ends
   there (the last message's padding may be absent).
2. **Attributes** (`RTA_OK`), within a message's body after its fixed
   header: at least 4 bytes remain; the attribute's length is at least 4
   and at most the bytes remaining. Otherwise the message is malformed.
   The next starts at the length rounded up to 4, ending as in point 1.
3. **Fixed headers.** A link body shorter than 16 bytes, an address
   body shorter than 8, an `NLMSG_ERROR` body shorter than 20 (its error
   and the request's header) or an `NLMSG_DONE` body shorter than 4 (its
   status) is malformed. Extra bytes inside a message after its
   attributes are covered by point 2.
4. **Values.** `IFLA_OPERSTATE` with a value other than one byte,
   `IFLA_MASTER`, `IFLA_GROUP` or `IFA_FLAGS` other than four,
   `IFLA_IFNAME` empty or longer than 16, or an address other than 4
   bytes for `AF_INET` (2) or 16 for `AF_INET6` (10), is malformed. An
   address message of `AF_INET` or `AF_INET6` with no address is
   malformed. An address message of another family is skipped once its
   attributes are walked (point 2): a dump of every family (5.4, point 7)
   also returns other families' addresses, which leave routing toward a
   server as it was.
5. **Tolerance.** Unknown message types and unknown attributes are
   skipped by their length. Duplicate attributes: the last one counts.
   Message flags other than `NLM_F_DUMP_INTR` are ignored. Message types
   below 16 that are not listed in 5.4, point 6, are skipped.
6. **Malformed** input is never partly trusted: messages before the bad
   one have been applied (each a change or a refresh on its own), and the
   bad one is a loss (section 7).
7. The sender has already been checked by the site: only datagrams from
   the kernel, whole, reach the decoder (ADR 0005, sheet 23).

### 5.7 States and ordering

1. **Start.** The monitor asks for no dump until the driver says its
   notify socket has joined its groups ("subscribed"). Then the first
   snapshot starts. Until it completes, the generation is unavailable.
2. **Snapshot.** The monitor asks for a link dump, then an address dump,
   each with a new sequence number, one at a time. Dump messages with any
   other sequence number are stale and ignored. Each dump's records fill a
   fresh table; the live tables are not changed by a dump.
3. Notifications received while a snapshot runs are applied to the live
   tables, exactly as at any other time (point 5). A link or address
   notification also makes the snapshot **dirty**: it may describe a state
   the dump read before or after it, so the snapshot is never merged or
   published. It runs on to its barrier (point 4), and when that comes
   back it is taken again; that is not a failed snapshot (5.5, point 4)
   and moves nothing, since each change that made it dirty moved the
   generation itself. Route, rule and nexthop notifications leave it: the
   tables hold neither.
4. **Completion.** When the address dump's `NLMSG_DONE` arrives, the
   driver sends a **barrier** on the notify socket: `NLMSG_NOOP` with
   `NLM_F_REQUEST | NLM_F_ACK` and a sequence of its own, which the kernel
   acknowledges on that socket, after every notification queued before
   it (EXECUTED on the guest's kernel: the acknowledgement is an
   `NLMSG_ERROR` of error 0 and that sequence). The barrier outstanding
   when a datagram arrives is the one that datagram is checked against,
   whole, before any of it is applied. It is accepted only when the
   datagram decodes whole, holds no `NLMSG_OVERRUN` (otherwise the bad
   message is a loss, point 6, which abandons the snapshot), and holds
   exactly one `NLMSG_ERROR` of the barrier's sequence: error 0, echoing
   an `NLMSG_NOOP`. Any other `NLMSG_ERROR` of that sequence in the
   datagram, before or after an acknowledgement, fails the snapshot
   (refused). Every record of the datagram is applied in order; at the
   acknowledgement, with the snapshot not dirty, the fresh tables replace
   the live ones, so later records of the datagram apply to them, and
   their moves are told unpublished; after the datagram's last record the
   generation moves (cause: snapshot) and is published. So
   the publication follows everything the notify socket received before
   the dump ended, an overflow among them, on the one thread that reads
   that socket. This move is required: a change between the subscription
   and the end of the dump may already be in the fresh tables, so its
   notification, read afterwards, looks like a refresh.
5. **Notifications.**
   - route, rule or nexthop, any of them: move;
   - link added or changed: move unless the table holds the link with
     the same usability (5.1, point 1); the table is updated;
   - link removed: move; the link and its addresses leave the table;
   - address added or changed: move unless it is a refresh; the table is
     updated;
   - address removed: move; it leaves the table;
   - `NLMSG_OVERRUN`: a loss;
   - anything else: skipped.
   One datagram with several changes moves the generation once per
   change.
6. **Losses** (section 7) make the generation unavailable, then tell
   every consumer of the move at once (after the first snapshot; 5.3,
   point 5), and start a fresh snapshot,
   abandoning one in progress; its completion publishes again. After a
   loss on the notify socket (an overflow, a truncated or malformed
   datagram, `NLMSG_OVERRUN`, a failed receive) the driver makes that
   socket again, joined, before the snapshot may start: its queue may hold
   records from before the loss, which would otherwise be applied to the
   fresh tables after they were built. A resume is a loss too (changes
   seen by the network while the machine slept may not reach the kernel
   until later), but it leaves the notify socket's queue whole.
7. **The driver** `[OWN]`. One thread receives on the notify socket and
   makes it again after its losses (point 6). A second thread receives on
   the request socket: it waits for the monitor to ask for a dump, sends
   and receives without the lock, which it takes only to hand a datagram
   over, and sends the barrier. Both sockets are held under the lock. **No
   receive blocks for ever:** a stop, a loss that abandons a dump, and a
   resume each send a **wake** (`NLMSG_NOOP` with `NLM_F_REQUEST |
   NLM_F_ACK`, sequence 0, never used otherwise) to the socket of each
   thread that must look again; the kernel's acknowledgement returns its
   receive. A socket is replaced only under the lock, and a stop is
   decided under the lock before the wakes go to the sockets then in
   place, so no receive can start that a stop's wake misses; the stop's
   wakes are sent even if a hook told of the stop unwinds. A wake whose
   send fails, or whose acknowledgement the kernel drops, leaves that
   receive blocked until the next datagram on its socket, and a barrier
   whose acknowledgement is lost leaves the generation unavailable until
   the next loss: closing these needs a deadline (section 13, Q6).
   Dropping the handle stops the source and joins both threads; a hook on
   one of the threads that drops it only asks for the stop (5.3,
   point 3), which the monitor honours before publishing or applying
   anything more, and both threads check for it each time they take the
   lock after a receive; that thread then ends, without joining. This needs no call
   beyond `linux-rtnetlink`'s; a `poll` with a wake descriptor or a
   receive timeout would each need a new site of ADR 0005. A dump that does not end in order
   (links, then addresses, then the barrier) may still be running in the
   kernel on that socket, so the request socket is made again before the
   next dump. When either thread ends, or the owner drops the handle, the
   source stops (5.2, point 4).
8. **Where it lives** `[OWN]`, checked against ADR 0004, C4. The counter,
   the hooks, the decoder and the state machine are a new pure crate,
   `ironafs-netgen` (no dependencies; its consumers, pure crates among
   them, read the counter's type). The threads are in the driver
   `ironafs-net`, which C4, point 3, gives the UDP driver; the monitor is
   the other network driver, and it uses only `ironafs-os`'s
   `linux-rtnetlink` site; this is a named departure from C4, point 3's
   row. C4, point 1's table gains the row (section 13, Q4).
9. A move happens before anything that depends on the new state is read
   by a consumer (5.3, point 2). There remains a window between the
   kernel's change and the monitor reading its notification, in which a
   reader still sees the old generation; it is a residual like C23,
   point 4's, bounded by how fast the thread reads.

### 5.8 Stop, wake and barrier model

Status: designed, not yet built. This subsection is the design of the
monitor's driver under ADR 0005, sheet 23, as amended on 2026-11-22
(row 23: the bounded wait, the wake descriptor and non-blocking sends
and receives). The current driver is the one 5.7, point 7 describes.
When the driver is rebuilt to this design, this subsection replaces:
5.3, point 3's sentence on a hook that drops the handle (by point 3
below); 5.7, point 2's sequence numbers (by point 6, item 10); 5.7,
point 4's rule for accepting the barrier (by point 6, item 6); 5.7,
point 7's threads, wakes and residuals; 5.7, point 8's "no
dependencies" (by point 1); section 6's sequence row; and section 13,
Q6. The rest of 5.7 stands. Where a point applies sheet 23, it quotes
the text it applies.

1. **The parts** `[OWN]`.
   - **Two workers,** each a thread of `ironafs-net` that owns one route
     socket: the **notify worker** receives on the notify socket, makes
     it again after its losses, and sends the barrier on it; the
     **request worker** sends the dumps on the request socket, receives
     their answers, and makes that socket again. A worker sends and
     receives only on its own socket. The barrier moves from the request
     worker (5.7, point 7) to the notify worker, because its answer
     arrives on the notify socket and its deadline is the notify
     worker's to watch.
   - **Two wakes,** one per worker, each one `eventfd` of sheet 23 ("make
     a wake (one `eventfd`), signal it from any thread, and clear it").
     A worker's wake is made inside that worker's thread, at its start,
     and split in two handles over the same descriptor: the **owner
     side,** which clears and waits, is neither `Clone` nor `Send`, so
     it never leaves the thread that made it; the **signal side,** which
     only signals, is `Send` and `Sync`, and the worker hands it back to
     `spawn` before its first turn, which gives clones to the other
     worker, the handle and the resume input. So no thread can clear or
     wait on a wake that is not its own (sheet 23: "Each thread that
     waits on a route socket has its own wake; no other thread waits on
     it or clears it"). A worker whose wake cannot be made ends at once,
     and `spawn` fails, as it does for a socket now.
   - **One state lock** (a `Mutex`) over: the monitor of 5.7, the
     attempts (point 6), the two sockets now in place, each with an
     **instance number** from a `u64` counter that is never reused and
     with its own **sequence counter** (point 6, item 10), whether each
     socket must be made again, and the stop (point 3). The lock is
     taken only through one wrapper, whose guard also records, for the
     thread holding it, that it holds the lock (a thread-local mark,
     set when the guard is made and cleared when it drops).
   - **The clock** is the daemon's shared `Clock` (ADR 0004, C37),
     handed to `spawn`. Only the workers sample it. `ironafs-netgen`
     calls no seam: it is given each sample as a value (a `Stamp`, or
     the failure), so its decisions stay pure and are tested without
     threads.
   - **Two new dependency edges,** both on `ironafs-time` (pure,
     `no_std` with `alloc`): `ironafs-netgen`, for the sample and
     deadline types, which makes it its only dependency (in place of
     5.7, point 8's "no dependencies"), and `ironafs-net`, for the
     `Clock` interface. `check-deps` permits the second already; the
     first needs `check-deps` to permit exactly that edge. Both are
     recorded in section 13, Q4, with C4's amendment.

2. **What is done under the lock** `[OWN]`. Only: the monitor's calls
   (the consumers' hooks among them), registering, committing and
   ending attempts, installing a socket made outside the lock, reading
   or applying the stop, and taking a clock sample, which reads clocks
   and waits for nothing. **Never under the lock:** a send, a receive,
   a wait, a wake's signal or clear, making a socket, or joining a
   thread (sheet 23: "the caller never sends under its state lock";
   "wait, with the lock released"). A wake's signal and clear are I/O
   on a descriptor; keeping them outside the lock keeps every call that
   enters the kernel for a descriptor outside it.

3. **The stop.**
   - **Who decides it.** Each stop is decided under the lock, by
     `Monitor::stop` (every hook told once, reads fail): by the handle's
     drop on a thread that does not hold the lock (below); by a worker
     that ends, for any reason, through its exit guard; by the monitor
     itself (exhaustion, 8 failed snapshots in a row, 5.5, point 4); by
     a failed signal or clear of a wake, or a failed wait (point 7); or
     by a deferred request (the next bullet). The stop is the first
     thing each lock hold checks, so nothing is applied after it.
   - **The handle's drop is keyed to the lock, not to the thread.** A
     drop on a thread that holds the state lock (the lock's mark,
     point 1) is a hook's: a worker's, the resume input's, or a drop's
     own stop, whichever thread told it. That drop only records a
     **deferred request** and joins nothing; it cannot take the lock
     its own thread holds, and its thread cannot join workers that need
     that lock. A drop on a thread that does not hold the lock takes it,
     decides the stop, releases, signals both wakes, and joins both
     workers. No worker runs code outside the driver's own while it
     does not hold the lock (hooks run only under it, 5.3, point 3), so
     a worker never reaches the joining path.
   - **A deferred request is made only under the lock.** The request
     is the driver's, not public: the only caller is the handle's drop,
     and only on the lock-holding path above, so the request is always
     set inside the hold of the thread that owns the lock, before that
     hold's last check. The monitor applies it after the hooks of the
     move being told and before the publisher stores that move's value,
     so the move is never published. Every lock hold of a worker, the
     handle and the resume input ends by applying a request still
     pending. So no lock is released with a request pending, and no
     request can arrive between the publisher's check and its store
     from another thread. `Monitor::is_stopped` is true only once the
     stop is applied; the pending request is read separately
     (`stop_asked`). So a driver that leaves its loop on `is_stopped`
     never leaves a generation readable.
   - **How each worker is woken.** The thread that decided the stop
     releases the lock, then signals the wake of each worker that is not
     itself: the handle and the resume input signal both, a worker the
     other. The signals are sent from a drop guard armed before the
     lock is taken, so a hook that unwinds while told of the stop does
     not skip them (sheet 23: "A stop ... is decided under the caller's
     state lock and then signals that worker's wake").
   - **How a worker ends.** At every lock hold it finds the stop and
     returns, without sending or waiting again. Its exit guard finds the
     source already stopped and does nothing more. A worker that the
     signal reached ends when its current call returns. One that the
     signal did not reach ends at its next return from a wait, whose
     timeout is at most `ROUTE_WAIT_HORIZON` (point 5), subject to the
     kernel's rounding and scheduling. So the handle's drop, which joins
     both workers, returns once each worker's call in progress has
     returned, plus at most one wait's timeout and the hooks' run (5.3,
     point 3: hooks never block). The wait's own timeout bounds a wait;
     no signal bounds a send or a receive already in progress. A receive
     with `MSG_DONTWAIT` returns at once; whether a send with
     `MSG_DONTWAIT` can still wait inside the kernel is UNVERIFIED
     (sheet 23), so the drop's join is bounded only as far as section
     13, Q9, shows. Nothing in this design depends on the answer: a send
     that waits delays its worker's next look at the stop, and nothing
     else.
   - **Failed wake I/O is fatal.** A signal or a clear that fails (an
     error, or a transfer other than 8 bytes, as the site reports it) is
     reported by the thread that saw it: it takes the lock, stops the
     source (`Monitor::stop`: every consumer told, reads fail, no fresh
     snapshot), releases, and signals the wakes it still can; a worker
     then ends. The other worker sees the stop within one wait's
     timeout even if its own wake cannot be signalled (sheet 23: "A
     failed signal or clear is fatal"; "consumers are invalidated at
     once and the source stops, with no fresh snapshot").
   - **No netlink message ends a receive.** The netlink wake
     (`NLMSG_NOOP` of sequence 0, 5.7, point 7) is removed: a receive
     never blocks (`MSG_DONTWAIT`), and a wait ends on its own wake or
     its timeout (sheet 23: "The wake replaces any netlink message sent
     only to end a receive").

4. **A worker's turn,** in this order (sheet 23: "clear its wake; take
   the state lock and read what it must act on (a stop, its outstanding
   attempts and their deadlines); release the lock; wait, with the lock
   released"):
   1. Clear its own wake.
   2. Take the lock. In this order, each step ending the hold early
      where it says so:
      - If stopped, release and end.
      - Take a fresh sample. End, as expired, each of its own attempts
        whose deadline has passed or whose latched doubt has moved
        (point 6); if the sample failed, end every one of its own
        attempts that way.
      - If its socket must be made again, register nothing: release,
        and go to step 3.
      - Read whether a dump (request worker) or a barrier (notify
        worker) is wanted with no attempt of its own live for it, and,
        with a valid sample, register it (point 6, item 2) and commit
        it to dispatch (point 6, item 3). Read the deadlines of its live
        attempts. Release.
   3. If its socket must be made again: make it with the lock released,
      then take the lock, install it under a new instance number with a
      fresh sequence counter unless the source stopped meanwhile, and,
      for the notify socket, tell the monitor it is subscribed (5.7,
      points 1 and 6); release, signal the other worker if the change
      concerns it, and start the turn again. Registration comes only in
      a later hold, on the new instance, so no attempt is ever
      registered on an instance the same turn replaces. On the notify
      socket the monitor asks for nothing until it is subscribed (5.7,
      point 1); on the request socket step 2's order is the gate.
   4. Send each attempt committed in step 2, without the lock, on the
      socket instance it was registered on (point 6, item 3).
   5. Take a fresh sample, without the lock, and check it against the
      deadlines and latches copied in step 2: if the sample failed, a
      deadline has passed, or the awake doubt count is above a latch,
      take the lock, end each affected attempt still live, as expired
      (once, point 6, item 7), release, and start the turn again.
      Otherwise compute the timeout from it (point 5).
   6. Wait on its socket and its own wake, with that timeout.
   7. Act on what the wait returned:
      - "timed out", "interrupted", or the wake alone: start the turn
        again;
      - an error of the wait itself: point 7;
      - the socket ready (readable, error or hangup), with or without
        the wake: record the error and hangup flags as **pending** on
        that socket instance; take the lock; if stopped, release and
        end; end each attempt due as in step 2; release; then receive
        (step 8). So the stop and every deadline change are handled
        before the socket is read (sheet 23: "the worker handles the
        stop and any deadline change before it receives from the
        socket").
   8. Receive, taking the lock for each outcome (point 6 for a
      datagram, point 7 for a pending flag the receive does not
      explain), until the first of: "nothing queued"; `RECEIVE_BATCH`
      receives in this turn; a loss that makes the socket again; a stop;
      "interrupted". Then start the turn again. Pending flags stay
      pending across that return, an "interrupted" receive included,
      until a receive outcome on the same socket instance resolves them
      (point 7); they are dropped only with the instance.
      `RECEIVE_BATCH` is a named constant of `ironafs-net`, 16
      datagrams `[OWN]`: large enough that one turn drains an ordinary
      burst, small enough that a notify socket that never empties still
      returns to step 1 every 16 datagrams, where a newly wanted barrier
      is registered and sent and due attempts are ended. So a stream of
      notifications cannot hold off a barrier or a retry.

   **Every change another worker must see is followed by its signal.**
   A lock hold that changes what the other worker must act on (the
   dump the monitor asks for, the barrier it wants, an attempt of the
   other worker ended by this hold, a stop, a fresh subscription) is
   followed, after its release, by a signal of that worker's wake.
   **Why no signal is missed** `[OWN]`: a signaller changes the state
   under the lock before it signals, and a worker clears before it
   reads under the lock. If the worker's clear comes after a signal,
   its read comes after the change that signal follows, and sees it. If
   the clear comes before the signal, the signal stays in the counter
   and ends the worker's next wait at once (sheet 23: "So a signal made
   after the read is still in the counter, and ends the wait"). Neither
   order loses a stop: the two failure cases, a signal made before the
   clear whose change the read misses, and a read made before the
   change whose signal is cleared unseen, need a signal made before its
   change, which this order forbids.

5. **Every wait is finite** (sheet 23: "always has a finite timeout";
   "never more than a named horizon constant, `ROUTE_WAIT_HORIZON` (1
   second ...)"). The timeout passed to the wait is the time left, on
   the awake clock, from the sample of step 5 to the earliest deadline
   among the worker's own live attempts, capped at
   `ROUTE_WAIT_HORIZON`; with no attempt live, it is the horizon. A
   failed sample never reaches the wait: step 5 ends the attempts
   first, and the next turn's step 2, which registers nothing without a
   valid sample, leaves none live, so the wait takes the horizon (sheet
   23: "Meanwhile each wait takes `ROUTE_WAIT_HORIZON` as its
   timeout"). A timeout of zero is passed as zero, never as a negative
   (infinite) value; the site rounds a remainder below one millisecond
   up. How long the wait then takes is the kernel's: the horizon bounds
   the timeout passed, subject to the kernel's rounding and scheduling
   (sheet 23, Context, poll(2)). **After every return,** "interrupted"
   included, the turn starts again from step 1, so the next timeout
   comes from a fresh sample and an earlier one is never passed again
   (sheet 23: "an earlier timeout is never passed again"). An
   interrupted send is point 6, item 3. An idle worker wakes once per
   horizon.

6. **Attempts** (sheet 23: "Each attempt is registered before its
   send"; "One terminal transition per attempt"; "An answer is accepted
   only while its attempt is live and in time").
   1. **Kinds.** A link dump and an address dump, the request
      worker's; a barrier, the notify worker's. At most one dump
      attempt and one barrier attempt are live at a time.
   2. **Registration,** under the lock, with a valid sample taken in
      that hold, on a socket instance that need not be made again: an
      **id** from a `u64` counter never reused; the **socket
      instance;** the **sequence,** the next of that instance's own
      counter (item 10); a **deadline** on the awake clock, the sample's
      awake instant plus the kind's duration (point 8), as an
      `AwakeDeadline`; and the **latched doubt,** the sample's awake
      doubt count. A deadline that does not fit in 64-bit nanoseconds
      is handled as if the sample had failed: nothing is registered
      (it needs some 584 years of awake time). While samples fail
      nothing is registered, so no deadline is set until a valid
      sample returns (C37, point 5).
   3. **Dispatch.** In the same hold as its registration, the attempt
      is **committed** to one send: the worker takes its bytes, id and
      socket instance, then releases the lock and sends. A committed
      send is made even if the attempt ends in the gap (a resume or a
      loss abandons it, or another hold expires it): the send's request
      was registered on that instance before it left, so its reply is
      a reply to an ended attempt (item 9). The send's outcome is then
      reported under the lock by id: if the id is no longer live it is
      discarded and counts nothing; if it is, a failure ends it (item
      7). An interrupted send is committed again only in a new hold:
      the worker takes the lock, checks the stop, that the attempt is
      still live, and from a fresh sample that it is in time, and then
      commits it again and sends with the lock released, or ends it as
      expired. A send never leaves without a commit in a hold where its
      attempt was live, and never on an instance other than the one it
      was registered on.
   4. **An answer,** a record of the attempt's sequence received on its
      socket instance, is checked in the lock hold that handles its
      datagram, before any record of that datagram is applied: the
      attempt is live, the datagram came on the attempt's socket
      instance, and a fresh sample taken in that hold shows the deadline
      not passed and the awake doubt count not above the latch. If the
      attempt fails the time check, it ends as expired first, and the
      datagram is then handled with that attempt gone. On the notify
      socket, a datagram's notifications are applied whatever the
      barrier's fate (5.7, point 5); only the barrier's answer depends on
      it. A failed sample in that hold ends every attempt of that worker
      as expired, and the datagram is handled the same way.
   5. **A dump's answer.** Each intermediate record that passes is
      applied to the fresh tables and the attempt stays live. Its
      terminal answer is its `NLMSG_DONE` with a status of 0 or more,
      which ends it as completed (and asks for the next dump or the
      barrier, 5.7, points 2 and 4). An `NLMSG_ERROR` of its sequence, a
      negative status, and every other check of section 7's dump row end
      it as failed, with that row's cause.
   6. **The barrier's answer.** The datagram is checked whole first
      (5.7, point 4). The barrier is accepted only if the datagram holds
      **exactly one record of the barrier's sequence, and that record is
      an `NLMSG_ERROR` of error 0 echoing an `NLMSG_NOOP` header.** Any
      other record of that sequence, before or after it (a second
      acknowledgement, an error, an `NLMSG_DONE`, a link or address
      record, any type at all), ends the barrier as failed (refused).
      This replaces 5.7, point 4's "exactly one `NLMSG_ERROR` of the
      barrier's sequence", which let other record types of that sequence
      pass; section 7's barrier row follows it.
   7. **The terminal transition.** An attempt ends only through one
      function, under the lock, given the attempt's id and an outcome:
      completed, failed (a refusal or a bad answer, with its cause),
      send failed, receive failed, expired, or abandoned. It acts
      only if that id is live: it removes the attempt and tells the
      monitor; every later call with that id finds it gone and does
      nothing. So a send failure, an expiry and an answer racing for one
      attempt are decided by whichever takes the lock first, and once.
      A send failure carries the id of the attempt it sent for, never
      the sequence alone, so it cannot end a newer attempt (sheet 23: "A
      send failure is charged to the attempt whose id it carries").
      Failed, send failed ("would block", a short send and any other
      error), receive failed and expired each count once as a failed
      snapshot (5.5, point 4, and section 7). Abandoned, the end of an
      attempt whose snapshot a loss restarted (5.7, point 6), is not
      counted: the loss is.
   8. **After an attempt ends.** A dump attempt that did not complete
      marks the request socket to be made again before the next dump
      (5.7, point 7; step 2's order makes it so), so a reply to it can
      arrive only on a socket no worker reads any more. A barrier
      attempt that did not complete leaves the notify socket in place: a
      late acknowledgement of it carries a sequence no live attempt on
      that instance has, and is skipped.
   9. **A late reply to an ended attempt** does nothing: it is not
      applied, not counted, and moves nothing (sheet 23: "So a late
      acknowledgement cannot publish, even if it takes the lock before
      the expiry path does": if it takes the lock first, its own fresh
      sample finds the deadline passed).
   10. **Sequences are never reused on a socket instance.** Each socket
       instance has its own `u32` sequence counter, which starts at 1
       when the instance is installed and only grows; every attempt on
       that instance takes its next value, and no other instance's
       attempts touch it. A counter that has issued `u32::MAX` issues
       nothing more: its socket must be made again before the next
       attempt (the request socket at no cost; the notify socket as a
       loss, failed receive, section 7). So a sequence is issued at most
       once per instance, whatever the other worker or an abandoned
       snapshot takes, and a reply to an ended attempt can never match a
       live one on the same instance (sheet 23: "A retired attempt's
       sequence is never issued again on the socket that may still carry
       a reply to it"). 0 is never issued, so no attempt's sequence is a
       default value. At one attempt per millisecond an instance runs
       out after about 50 days.

7. **Errors of the wait, and error and hangup on a route socket.**
   - **A failed wait** (the site's error for any `poll` failure other
     than "interrupted", `POLLNVAL` on either entry, any bit on the wake
     but `POLLIN`, an overflowed counter's `POLLERR` included, a bit
     outside those named, or a count that disagrees with the entries)
     is fatal, as failed wake I/O is (point 3): the worker takes the
     lock, stops the source, releases, signals the other worker, and
     ends. The report cannot tell a bad wake from a bad socket, and the
     wake cannot be trusted after either `[OWN]`.
   - **Error and hangup.** The worker applies sheet 23's rule as written:
     "A wait that reports `POLLERR` or `POLLHUP` on the route socket is
     followed by a receive. `POLLERR` is explained only by an error
     return of that receive: `ENOBUFS` takes the overflow path above, and
     any other error is a failed receive. `POLLHUP` is explained by no
     receive outcome." The flags stay pending (point 4, step 8) until a
     receive outcome on that instance other than "interrupted" resolves
     them. A flag that outcome does not explain, and `POLLHUP` always,
     is a failed receive on that socket: a datagram received with it is
     not applied; on the notify socket it is a loss (failed, section 7:
     consumers told at once, the socket made again before any
     snapshot); on the request socket the socket is marked to be made
     again, and a live dump attempt, if there is one, ends as receive
     failed (counted, section 7's request-socket row). With no attempt
     live, nothing is counted, and the socket is still made again
     before the next dump.
   - This fails closed whatever the kernel does, so the code does not
     wait on the measurement; whether an unexplained flag happens at
     all is UNVERIFIED (section 13, Q8).

8. **Deadline durations,** named constants of `ironafs-netgen`
   `[OWN]`, provisional until section 13, Q7, is measured, which is
   before the attempts are coded:
   - `DUMP_DEADLINE`, 10 seconds, for each link and address dump: from
     the request's registration to its `NLMSG_DONE`.
   - `BARRIER_DEADLINE`, 10 seconds, from the barrier's registration to
     its acknowledgement, which the kernel queues after every
     notification already queued (5.7, point 4), so it also waits for
     the notify worker to read that backlog.
   - Why these values: an attempt that expires is counted, and 8 in a
     row stop the source, so a deadline too short turns a slow but
     working kernel into a stopped source; a deadline too long only
     delays the retake after a lost answer, while nothing is readable.
     So the value is set long, far above an expected round trip of
     milliseconds, and the worst case on a kernel that never answers is
     8 deadlines, 80 seconds, before the stop.
   - Each must stay above `ROUTE_WAIT_HORIZON`, so that a worker checks
     an attempt at least once before it expires.

9. **The group set and the nexthops.** Under sheet 23 the group set
   becomes final only by an amendment of row 23, accepted by the owner
   (ADR 0005, point 7.1); until then row 23 stays approved-only on
   every target, and 5.1, point 3's choice is a proposal. That choice
   is what the amendment restates. The text proposed for sheet 23's
   first obligation, in place of the sentences from "the group set is a
   constant" to "a measurement on a kernel before 5.3 settles the open
   question", not applied here:

   > The group set is a constant of eight groups, each joined by its
   > mask bit `1 << (group - 1)` in `nl_groups` (netlink(7)): links
   > (`RTNLGRP_LINK`), the addresses, routes and policy rules of IPv4
   > (`RTNLGRP_IPV4_IFADDR`, `RTNLGRP_IPV4_ROUTE`,
   > `RTNLGRP_IPV4_RULE`) and of IPv6 (`RTNLGRP_IPV6_IFADDR`,
   > `RTNLGRP_IPV6_ROUTE`, `RTNLGRP_IPV6_RULE`), and nexthop objects
   > (`RTNLGRP_NEXTHOP`, group 32, mask bit `1 << 31`), for the reason
   > the network-change monitor's specification gives (its 5.1, point
   > 3: a change to a nexthop object changes where the routes using it
   > lead, and the kernel's reporting of those routes can be turned
   > off). Point 5.6's check proves each group number against the
   > build's `<linux/rtnetlink.h>`, which must be from Linux 5.3 or
   > later or carry the nexthop group's backport. Whether a bind that
   > names the nexthop bit succeeds on a running kernel before 5.3 is
   > UNVERIFIED; if it fails, the monitor does not start, and no
   > network generation is readable on that kernel. Revisit this row if
   > a supported kernel refuses that bind.

   Accepting it needs section 13, Q5, measured on a running kernel
   before 5.3, named as the venue with its version when the measurement
   is made. If the oldest supported Linux kernel is not that kernel, it
   is measured too, separately: a bind that succeeds there says nothing
   about an unknown group bit on a kernel before 5.3, unless that
   kernel is itself before 5.3 and carries no nexthop backport.

10. **The failure cases this model excludes,** each by the point named:
    - a stop a worker can miss, by a signal made before the clear or a
      read made before the signal: point 4's order and its argument;
      even a lost signal costs at most one wait's timeout (points 3 and
      5);
    - a wait with no finite timeout, or a timeout reused after
      `EINTR`: point 5;
    - an attempt sent before it is registered, or a reply matched to an
      ended attempt: point 6, items 2, 3, 4, 7, 9 and 10;
    - a worker that waits on another worker's wake: point 1 (the owner
      side is made in its worker's thread and is neither `Clone` nor
      `Send`);
    - a lock held across a wait or a send: point 2, and point 4's steps
      3 to 8, which take the lock only between those calls;
    - a failed signal or clear that is not fatal: point 3; a failed
      wait, point 7.

11. **Tests, written before the code** (sheet 23: "The caller's design
    tests, through a seam over this site and a test clock"). The driver
    reaches the site through a seam of `ironafs-net` with two
    implementations: the site itself, and a scripted one whose wait
    returns what the test holds (ready, wake, both, timed out,
    interrupted, an error or hangup flag, a failed wait) and whose
    signal and clear can be made to fail. The clock is
    `ironafs-test-support`'s `SeededClock`, advanced, failed and
    restored by the test. A test that needs an interleaving drives the
    turn's steps one at a time from the test's own thread, so the order
    is the test's, not the scheduler's. Each test is shown failing
    against a planted copy of the defect it names.
    1. A lost wake: the other worker's signal fails; the source stops at
       once; a worker whose wake was never signalled still ends at its
       next return from a wait, whose timeout is at most the horizon,
       and the handle's drop returns.
    2. A signal between the read (step 2) and the wait: the wait returns
       at once with the wake.
    3. A signal before the clear (step 1) with its change made before
       the signal: the read of step 2 sees the change.
    4. The wake and the socket ready at once with a stop decided: the
       worker ends without receiving.
    5. The wake and the socket ready at once with an earlier deadline:
       the expiry is taken before the datagram is read.
    6. The wait, a send and a receive each interrupted repeatedly, the
       clock advanced between returns: each next timeout comes from a
       fresh sample and is never the earlier one; the interrupted send
       is not committed again once its deadline has passed.
    7. A wait timeout: the scripted wait records each timeout passed;
       none is above the horizon and none is negative.
    8. A dump that is never answered: it expires at its deadline,
       counts once, and the socket is made again; eight in a row stop
       the source.
    9. A late acknowledgement after its barrier expired, both when the
       expiry takes the lock first and when the acknowledgement does
       (the clock advanced past the deadline before its hold): it never
       publishes, and the barrier counts once.
    10. A send failure racing an expiry of the same attempt: it counts
        once; a send failure for an ended attempt does not end a newer
        one.
    11. Every send is committed first: the scripted socket refuses a
        send whose sequence was never registered on that instance, and
        one that no hold committed.
    12. A clock that fails and returns between two of the monitor's
        samples (the doubt count moved above an attempt's latch): every
        outstanding attempt expires once; nothing is registered while
        samples fail, and each wait meanwhile is the horizon.
    13. An intermediate dump record before its deadline is applied; one
        at or after it is not, and the attempt expires.
    14. An error or hangup flag followed by an empty receive, and by a
        valid datagram: the datagram is not applied; on the notify
        socket a loss and a fresh socket; on the request socket a
        failed dump, counted once, and a fresh socket; with no dump
        live, a fresh socket and nothing counted.
    15. The barrier's datagram with a valid acknowledgement and an
        `NLMSG_DONE`, and with a link record, of the barrier's sequence,
        each before and after the acknowledgement: refused, nothing
        published.
    16. A hook that drops the handle during a move: the publisher never
        stores that move's value. The test reads every value the
        publisher stores (a store log kept for tests), not only the
        reader after the call, so moving the stop check below the store
        fails it. `is_stopped` is false while the request is only
        pending inside the move, and no lock hold ends with it pending.
        The deferred request has no public caller: a drop on a thread
        that does not hold the lock takes the locked stop instead (test
        21).
    17. A hook that drops the handle: the test keeps the handle in a
        slot it owns, and the hook holds only a non-owning reference to
        that slot, so no cycle keeps the handle or its senders alive.
        Completion is a result the test reads with a bounded wait,
        independent of any hook's sender: set when both workers have
        ended, which covers a normal stop, a startup failure and a
        halt. A separate case stops the source before the first
        snapshot and checks that this result is set and the hook never
        took the handle.
    18. A wake's owner side is not `Send` (a compile-fail doc-test), and
        each worker makes its own wake in its own thread.
    19. A sequence wrap: on one instance, with the first sequences
        issued there taken by an abandoned snapshot and other sequences
        meanwhile issued on the other instance, the counter reaches
        `u32::MAX`: no sequence repeats on that instance, and the next
        attempt is registered only on a new one.
    20. A notify socket that never empties while a barrier becomes
        wanted: the barrier is registered and sent within one
        `RECEIVE_BATCH`, and a due attempt is ended within one batch.
    21. The handle dropped by a hook during a resume, on the resume
        input's thread: the drop records the request and returns; the
        resume input's hold applies it; nothing deadlocks, and nothing
        is joined.
    22. A failed dump followed by the request socket's renewal: exactly
        one failure is counted, nothing is registered on the old
        instance, and the retry is sent on the new one.
    23. An attempt abandoned between its commit and its send: the send
        is made; its outcome is discarded; its reply is ignored;
        nothing is counted.
    24. A fault first seen by step 5's sample (a failed sample, a moved
        doubt count, a passed deadline): the affected attempts end once,
        before the wait.
    25. A failed wait, on each kind the site reports: the source
        stops.
    26. `POLLHUP`, and `POLLERR`, followed by an interrupted receive:
        the flag stays pending, and the next receive outcome on that
        instance resolves it as a failed receive.
    The Linux tests of section 10, point 4, stay, and gain: the
    handle's drop on an idle host returns. The rebuild also moves the
    burst in `ironafs-os`'s kernel test
    (`a_burst_the_socket_does_not_read_is_an_overflow`), which today
    writes a file in the temporary directory named by the process id,
    to `ip -batch -` on its standard input, as `ironafs-net`'s own
    overflow test does, so no test writes a file at a path another
    user could predict.

## 6. Arithmetic

| Sum | Bound | At overflow |
|---|---|---|
| message start + 16 | ≤ datagram length (65,536) | checked; malformed |
| message length rounded up to 4 | ≤ `u32::MAX` | checked; the datagram ends |
| attribute start + 4, + attribute length | ≤ body length | checked; malformed |
| attribute length rounded up to 4 | ≤ 65,535 + 3 | checked; the body ends |
| counter + 1 | < `2^64 - 1` | exhaustion; stopped (5.2, point 3) |
| sequence number + 1 | wraps (`u32`), skipping 0 | wrapping is allowed: stale replies are told apart by equality with the current dump only; 0 is the wake's (5.7, point 7), so no dump or barrier uses it |
| failed snapshots in a row + 1 | ≤ 8 | stopped (5.5, point 4) |

## 7. Errors and failure

| Event | Effect |
|---|---|
| notify socket overflow (`ENOBUFS`) | move (overflow); fresh snapshot |
| truncated notification | move (truncated); fresh snapshot |
| malformed notification, or `NLMSG_OVERRUN` | move (malformed); fresh snapshot |
| after any of the three rows above | the notify socket is made again, joined, before the snapshot |
| a datagram from a sender other than the kernel | ignored: the site does not return it, and it moves nothing (a local process could otherwise force moves) |
| any other failed receive on the notify socket | move (failed); the notify socket is made again (joined before use), then a fresh snapshot; if it cannot be made, stopped |
| a link or address notification while a snapshot runs | the snapshot is dirty: taken again when its barrier comes back, never published; not a failed snapshot, and no move beyond the change's own |
| dump truncated, malformed, interrupted (`NLM_F_DUMP_INTR`), answered with `NLMSG_ERROR` of its sequence or `NLMSG_DONE` of a negative status, holding `NLMSG_OVERRUN` of any sequence, or holding anything of its sequence after its terminator | move (the cause); the snapshot restarts; 8 in a row: stopped. A dump datagram is checked whole before any of it is applied, so `NLMSG_DONE` never completes a snapshot ahead of bad bytes, and nothing after the link dump's terminator is applied to the address dump, whose request is not yet sent |
| the barrier answered, in one datagram, with an `NLMSG_ERROR` other than error 0 echoing `NLMSG_NOOP`, or more than once; from the rebuild of 5.8, any datagram that does not hold exactly one record of the barrier's sequence, that record the acknowledgement (5.8, point 6, item 6), whatever the order of its records | move (refused), after the datagram's records are applied; the snapshot restarts; counted as a failed snapshot |
| request socket send or receive failure | as a failed dump |
| resume | move (resumed); fresh snapshot |
| counter exhausted, the handle dropped, or a monitor thread ended | stopped: reads fail, every hook told |
| the handle dropped by a hook on one of the monitor's threads | stopped once that move is told to every consumer, before it is published or another record is applied; that thread ends |
| a hook panicked | halted: reads fail, no hook called again |
| any loss, until the fresh snapshot completes | reads fail (5.2, point 1) |

Every failure fails closed: consumers drop what they kept, and nothing
published before it is trusted after it.

## 8. Security

- The bytes are from the kernel, checked by the site (sender and
  wholeness), but they are still decoded as untrusted input (rules 1 to 3):
  a decoder flaw must not become a panic or an out-of-bounds read.
- A local process cannot move the generation: it cannot send to the
  notify socket as the kernel. A process with `CAP_NET_ADMIN` can change
  the host's state and so move it as often as it likes; that costs
  re-authorisation and refetches, nothing more.
- A process that changes routes faster than the monitor reads makes it
  overflow; each overflow is a move and a snapshot, so the result is more
  moves, never a missed one.
- A hook that panics leaves nothing readable: a loss withholds the
  generation before any hook runs, and the driver halts the source when a
  thread unwinds from a hook or finds its lock poisoned (5.2, point 4).

## 9. Interoperability

Not applicable: the monitor talks only to the local kernel.

## 10. Tests

1. Decoder: golden datagrams written from 5.4's layout (a link, an
   address of each family, a route, a rule, a nexthop, `NLMSG_DONE` with
   and without `NLM_F_DUMP_INTR` and with a negative status,
   `NLMSG_ERROR` refusing and acknowledging, an address of another
   family, a point-to-point address and its peer); a reject corpus for each
   check of 5.6; a seeded sweep over truncations and changed bytes of the
   golden datagrams that asserts no panic, that a prefix is accepted
   exactly when it ends at a message's end (with that many messages), and
   that an accepted changed input holds no more messages than its length
   allows.
2. Counter and hooks: unavailable before the first move; strictly
   increasing; hooks read the old value during the call; exhaustion and
   drop stop it.
3. Monitor: each state step of 5.7 and each row of section 7, on scripted
   datagrams; the acceptance test with a recording hook per consumer; a
   planted source that never moves, shown failing that test; a planted
   overflow and truncation, each followed by every hook and a fresh
   snapshot request.
4. Linux, in a network namespace (Arm64 guest and the hosted Ubuntu
   runners): an address, a route and a policy rule changed, each moving the
   generation; an overflow, followed by a snapshot; a nexthop seen on the
   notify socket; an address dump of every family returning addresses.
5. A fuzz target for the decoder waits on the fuzzing engine's decision
   (coding standards rule 21); until then the seeded sweep stands in
   (section 13, Q2).

## 11. Revisit if

- the M2 tests show route or link messages moving the generation often on
  an idle host (then narrow 5.1, point 2, with the route answer);
- a supported kernel reports a nexthop change neither on the nexthop group
  nor as route messages.

## 12. Sources

- `netlink(7)`, `rtnetlink(7)` (manual pages);
- the kernel's user-space headers `<linux/netlink.h>`,
  `<linux/rtnetlink.h>`, `<linux/if_link.h>`, `<linux/if_addr.h>`,
  `<linux/if.h>`, `<linux/nexthop.h>`;
- the kernel's `ip-sysctl` documentation, `nexthop_compat_mode`;
- the kernel's netlink documentation, `userspace-api/netlink/intro`
  (the status of `NLMSG_DONE`).

## 13. Open questions

- **Q1. The route answer** (`docs/specs/dns-client.md`, section 13, Q4):
  through which interface the route toward an address leaves, asked with
  an `RTM_GETROUTE` request on the request socket. Later part of #97.
- **Q2. The fuzz target**, once the fuzzing engine's binding is decided.
- **Q3. Resume detection**: who calls the resume input (the daemon's
  clock-pair check, ADR 0004 C37, point 4).
- **Q4. The crate row**: C4, point 1's table gains `ironafs-netgen`, and
  point 3's `ironafs-net` row gains the monitor. Decided: the amendment of
  C4 (Working) is written with the M2a amendments (#53), a cross-PR item;
  until then the monitor's place in `ironafs-net` is a named departure
  (5.7, point 8). The rebuild of 5.8 adds two edges to that amendment:
  `ironafs-netgen` depends on `ironafs-time`, its only dependency (in
  place of "no dependencies"), which `check-deps` must permit as that
  one edge; and `ironafs-net` depends on `ironafs-time`, which
  `check-deps` permits already (5.8, point 1).
- **Q5. The nexthop bit on a kernel before 5.3**: whether binding with an
  unknown group bit succeeds. A measurement on a running kernel before
  5.3, without the nexthop backport, settles it; its venue is named with
  the kernel's version when it is made (5.8, point 9). The oldest
  supported kernel, if it is not that kernel, is measured separately
  (5.1, point 3).
- **Q6. Wakes and the barrier without a deadline** (5.7, point 7): a wake
  whose send fails or whose acknowledgement is dropped, and a barrier
  whose acknowledgement is lost, have no retry. Retrying needs a deadline
  through the clock seam (coding standards rule 7) and a way to end a
  blocked receive when it passes (a receive timeout or a `poll` with a
  wake descriptor, each a new site of ADR 0005). Until then these stay
  residuals.
- **Q7. The deadline durations** (5.8, point 8): `DUMP_DEADLINE` and
  `BARRIER_DEADLINE` are provisional. The measurement that settles them
  is made before the attempts are coded, as sheet 23 requires ("settled
  there before code"): on each supported kernel (the Arm64 guest
  and the hosted Ubuntu runners, and the oldest supported kernel), in a
  network namespace holding the caps' worst case (1,024 links and 4,096
  addresses, 5.5, point 3), the time from registration to `NLMSG_DONE`
  of each dump, and to the barrier's acknowledgement behind a backlog
  that fills the notify socket's receive buffer, over at least 1,000
  rounds each, with and without route churn. A round whose
  `NLMSG_DONE` or acknowledgement never arrives is recorded as a failed
  round, with its kernel, never left out of the timings. A duration
  stands if the longest round trip measured is at most a tenth of it
  and no round failed; otherwise it is raised to ten times that, or a
  failed round is explained first, and the evidence is recorded here.
- **Q8. Error and hangup without a cause** (5.8, point 7): whether a
  route socket ever reports `POLLERR` or `POLLHUP` that the following
  receive does not explain. The measurement: on each supported kernel,
  in a network namespace, a soak of each overflow the tests provoke
  (5.5, point 1; section 10, point 4) and of an hour of address and
  route churn, logging every wait's `revents` with the receive that
  followed; a flag that no receive explained, or none ever, is
  recorded with its kernel. It does not block the code, which fails
  closed either way (5.8, point 7). It blocks marking row 23
  implemented: a kernel that raised such flags often would keep the
  generation unavailable (a loss each time on the notify socket) or
  stop the source (8 failed dumps on the request socket), and only the
  measurement can rule that out.
- **Q9. A send that waits inside the kernel** (5.8, point 3): whether
  `sendto` with `MSG_DONTWAIT` on a route socket can still wait inside
  the kernel, for a lock the kernel's own handler takes (ADR 0005,
  sheet 23, UNVERIFIED). The measurement: on each supported kernel, in
  a network namespace, the duration of every dump request and barrier
  send while another process keeps the kernel's routing busy (a batch
  of thousands of route additions and removals in a loop), over at
  least 1,000 sends each, recording the longest. The design holds
  either way: a send that waits only delays its worker's next look at
  the stop. What depends on it is the bound on the handle's drop (5.8,
  point 3), which is stated only as far as the measurement shows, and
  row 23's own qualification.

## 14. Review

Not yet reviewed.
