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
3. **Nexthop objects** `[OWN]`, decided here: ADR 0005, sheet 23, leaves
   the nexthops to "the network-change monitor's design"; this is that
   decision, and the sheet's text is left as accepted. Routes can point at a nexthop object (Linux 5.3 and later), and a
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
   asks it to stop, and it stops the source once the hook returns (5.7,
   point 7).
4. The causes are: a change (5.1), a snapshot completed (5.7, point 4),
   and each loss of section 7, named.
5. Until the first snapshot completes, no hook is told of anything
   `[OWN]`: no generation has been readable, so no consumer holds anything
   under one, and the first completion's move tells every consumer.

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
   `NLMSG_ERROR` of error 0 and that sequence). Only an `NLMSG_ERROR` of
   error 0 that echoes an `NLMSG_NOOP` is that acknowledgement; any other
   `NLMSG_ERROR` of the barrier's sequence fails the snapshot (refused).
   The acknowledgement counts only in a datagram whose every message
   decodes and holds no `NLMSG_OVERRUN`; otherwise the bad message is a
   loss (point 6), which abandons the snapshot. When the notify side reads
   that acknowledgement, with the snapshot not dirty, the fresh tables
   replace the live ones and the generation moves (cause: snapshot). So
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
   one of the threads that drops it only records the stop, and that
   thread, once the hook returns, stops the source and ends, without
   joining. This needs no call
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
| the barrier answered with an `NLMSG_ERROR` other than error 0 echoing `NLMSG_NOOP` | move (refused); the snapshot restarts; counted as a failed snapshot |
| request socket send or receive failure | as a failed dump |
| resume | move (resumed); fresh snapshot |
| counter exhausted, the handle dropped, or a monitor thread ended | stopped: reads fail, every hook told |
| the handle dropped by a hook on one of the monitor's threads | that thread stops the source once the hook returns, and ends |
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
  (5.7, point 8).
- **Q5. The nexthop bit on a kernel before 5.3**: whether binding with an
  unknown group bit succeeds. A measurement on such a kernel settles it
  (5.1, point 3).
- **Q6. Wakes and the barrier without a deadline** (5.7, point 7): a wake
  whose send fails or whose acknowledgement is dropped, and a barrier
  whose acknowledgement is lost, have no retry. Retrying needs a deadline
  through the clock seam (coding standards rule 7) and a way to end a
  blocked receive when it passes (a receive timeout or a `poll` with a
  wake descriptor, each a new site of ADR 0005). Until then these stay
  residuals.

## 14. Review

Not yet reviewed.
