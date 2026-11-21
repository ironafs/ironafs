<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Local IPC framework: resources and scheduling

Status: draft (2026-11-21). Raised in issue #62.

Kind: behaviour. Class: S. Any local user can connect to the daemon;
what one connection or one caller can make the daemon hold or do is a
security rule ([README](README.md), "Review class per kind").

This is the second of three parts of the local IPC framework. The first,
[local-ipc.md](local-ipc.md), holds the wire format and the order of
checks, and the table that says which part holds each place. Section
and point numbers are shared by the three parts: a number names one
place, whichever part cites it. Labels follow [README](README.md),
"Labelling claims". Source ids are those of local-ipc.md, section 12.

## 1. What this implements

ADR 0004, C39, points 2 and 3 (**Working**), the parts that apply:

> 2. **Ingress is separate from execution.** […] They hand work to a
>    bounded executor without blocking. A request that finds the executor
>    full gets a retryable error and is counted.
> 3. Workers wait on one-shot reply slots with a deadline counted from
>    arrival and a cancel token. Every queue is bounded in bytes.

ADR 0004, O28, points 2 and 3 (**Firm**), the parts that apply:

> 2. Key material is zeroed when its use ends. This covers token keys, IPC
>    secrets, […]
> 3. The places are named in code and listed in the specification for each
>    component, so a review can check the list is complete.

ADR 0004, C16, point 6 (**Firm**), the sentence that binds the adapter
endpoint's stream, which this framework carries as `EVENT` frames:

> - **Back-pressure:** the stream is bounded (C39, point 3). When it is
>   full, replies wait within their deadlines. An epoch advance is never
>   dropped: space for it is kept free.


Section 9 shows where each is kept and which test fails if it is broken.

## 2. Scope

1. **Covered:** admission of a connection; the byte accounts, for all
   connections and per caller; the policy budgets and the rules between
   them; ingress turns, the executor, deadlines and the result slot;
   output queues; the client's stream and request limits; the named
   zeroisation places.
2. **Not covered, and where it is covered:** the wire format and the
   order of checks on a frame: local-ipc.md. The Windows named pipe, its
   instance pool and the pre-attestation state:
   [local-ipc-windows.md](local-ipc-windows.md).

## 3. The design in short

local-ipc.md, section 3 gives the whole design in short; this part's
share is its points 9 and 10.

## 4. Terms

Those of local-ipc.md, section 4.

## 5. Specification

### 5.1 Layout

Not in this part: local-ipc.md, 5.1.

### 5.2 Caps

Points 1 to 7 and 11 are in local-ipc.md; point 10 is in
local-ipc-windows.md.

8. **Policy budgets.** Each has a default and a hard ceiling. A
   configured value of 0 or above its ceiling, or one that breaks a rule
   in point 9, refuses the whole configuration: at start the daemon does
   not start; on a reload the last good configuration stays `[OWN]`.

   | Budget | Default | Ceiling | On excess |
   |---|---|---|---|
   | Connections in all: every connection from its accept until its release (5.6.2), in any state | 128 | 1,024; on Windows also point 10 | close without reading |
   | Connections in handshake, all callers: `PreAttestation`, `AwaitHello` and `AwaitBinding`, and `Closing` or `Draining` entered from one of them without ever being `Open` | 16 | 64 | close without reading |
   | Connections in handshake per caller key: `AwaitHello` and `AwaitBinding` on every operating system (on Windows from the capture), and `Closing` or `Draining` entered from one of them without ever being `Open` | 4 | 16 | close without reading further |
   | Open connections per caller key | 16 | 64 | `HelloReply` `BUSY`, then close |
   | Listening pipe instances (Windows) | 2 | 8 | n/a: the daemon never creates more |
   | Handshake deadline: from accept until the connection is `Open` or its refusal is queued | 5 s | 30 s | close; no reply |
   | Frame deadline: from a frame's first byte until its last, read or discarded | 10 s | 60 s | close |
   | Write deadline: no byte of queued output written for this long | 30 s | 300 s | close; requests in flight are cancelled (5.6.3) |
   | Closing deadline: from entering `Closing` until the connection is closed | 5 s | 30 s | close, whatever is still queued |
   | Request deadline: from a `REQUEST`'s arrival until its result (5.6.4) | 60 s | 3,600 s | `DeadlineExceeded` (5.6.3, rule 3) |
   | Requests in flight per connection | 16 | 256 | `TooManyRequests` for the extra request; the connection stays |
   | Requests in flight per caller key, those still running after their connection closed included | 32 | 512 | `TooManyRequests`; the connection stays |
   | Executor queue: admitted requests waiting for a worker, all callers (5.6.4) | 256 | 4,096 | `Busy`, retryable, counted; the connection stays |
   | Parts per streaming request | 4,096 | the row's `parts_max` (point 11) | the stream ends; its terminal reply reports the early end |
   | Part bytes per streaming request | 4 MiB | the row's `stream_bytes_max` (point 11) | the stream ends; its terminal reply reports the early end |
   | Output queued per connection (encoded frames not yet written, each with its entry) | 2 MiB | 16 MiB | back-pressure (5.2.1, rule 4) |
   | In-flight bytes per caller key (5.2.1) | 16 MiB | 256 MiB, and half of the next row (point 9) | as a failed reservation (5.2.1) |
   | In-flight bytes, all connections (5.2.1) | 64 MiB | 1 GiB | as a failed reservation (5.2.1) |
   | Live heap requests, all connections (5.2.1, rule 6) | 1,048,576 | 16,777,216 | as a failed reservation (5.2.1) |

   An endpoint specification may set a lower ceiling for its endpoint,
   set a lower request deadline for a row, or add budgets of its own
   (the adapter endpoint's credit), never a higher ceiling `[OWN]`.
9. **Rules between budgets**, checked when the configuration is
   validated `[OWN]`. A is the admission allowance and R the endpoint's
   largest single-request reservation (5.2.1, rules 1 and 2), both
   computed from the configured values and the generator's constants:
   - connections in handshake ≤ connections in all; connections in
     handshake per caller key ≤ connections in handshake;
   - output queued per connection ≥ the endpoint's largest framed reply
     frame plus `ipc.overhead.frame`, so one reply or part always fits
     an empty queue;
   - in-flight bytes, all connections ≥ connections in all × (A +
     `ipc.overhead.caller`), plus on Windows P × `ipc.overhead.operation`;
   - in-flight bytes per caller key ≥ (open connections per caller key +
     connections in handshake per caller key) × A + R, so a caller at
     its connection limits can still run a request; and ≤ half of
     in-flight bytes, all connections. The promise is about a caller's
     own account: no one caller's account can hold more than half the
     global budget. What is left for the others is the other half less
     the charges made in the global account only (caller records, pipe
     instances' connect blocks, allowances of connections whose caller
     is not yet known), which the first rule of this point bounds.
     Among several callers the rest is first come, first served: no
     share is promised to each;
   - requests in flight per caller key ≤ executor queue ÷ 4, so one
     caller can fill at most a quarter of the queue;
   - each streaming budget ≤ its row constants (point 11);
   - closing deadline ≤ write deadline;
   - on Windows, point 10.

#### 5.2.1 Reservation before allocation

The in-flight budgets of 5.2, point 8 count every byte the daemon holds
for connections, in two accounts: one for all connections and one for
each caller key. A reservation is made in both accounts or in neither,
before the bytes are allocated and never afterwards `[OWN]`, from coding
standards rule 3 and ADR 0004, C39, point 3. A reservation that either
account cannot cover has failed. A connection whose caller is not yet
known (Windows, `PreAttestation`) is charged in the global account only,
and for its allowance only; the allowance is charged to its caller's
account at the capture (5.4, point 2).

1. **Admission.** Each accepted connection reserves an allowance, A:
   - the **record charge**: `ipc.overhead.connection` (4,096 bytes),
     plus the configured requests in flight per connection ×
     `ipc.overhead.request` (512 bytes), plus on Windows 2 ×
     `ipc.overhead.operation` (256 bytes). It pays for the connection's
     record, its 16-byte envelope buffer, its 96-byte handshake buffer
     (the larger handshake frame, framed), its request table, the fixed
     part of its output queue and, on Windows, the blocks of the one
     read and one write an instance may have outstanding (5.2,
     point 10);
   - on Windows, 1,024 bytes of **capture scratch**: the caller capture
     of ADR 0005, sheet 19 allocates token-information buffers of at
     most that size, and holds one at a time `[D2 sheet 19]`. It is
     charged before the capture runs, in the global account, like the
     rest of a `PreAttestation` connection's allowance;
   - 92 bytes, `Hello`'s memory budget (5.2, point 3), for the one heap
     request its decode makes (`client_name`);
   - a discard buffer of 4 KiB for refused frames;
   - M bytes for the endpoint's mandatory events (rule 4), 0 on an
     endpoint that has none;
   - one **reserved reply slot** of T bytes. T is the whole charge of
     the largest terminal reply the daemon may have to send without its
     handler having built it: `ipc.overhead.frame` for its queue entry
     plus the larger of the framed `ErrorReplyBody` (18,160 bytes, 5.2,
     point 6) and the largest framed early-end reply of the endpoint's
     streaming rows (5.5, point 6), computed by the generator (5.6.1,
     point 3).

   The three `ipc.overhead.*` values, and `ipc.overhead.frame` (64
   bytes, rule 4), are ceilings fixed here. The code asserts at compile
   time, on each target, that the types each one pays for are no larger;
   a target where one is larger does not build, and this point changes.
   The allowance makes a fixed number of heap requests,
   `ipc.overhead.connection_requests` = 16 at most, counted under
   rule 6. Everything in the allowance is allocated once, at admission, and
   nothing in it grows. Storage that exists before any connection is
   charged on its own, in the global account only: each Windows pipe
   instance reserves one `ipc.overhead.operation` when it is created,
   for its connect wait's block, and releases it when its handle is
   closed; an instance that cannot reserve it is not created. If either
   account cannot cover the allowance, the connection is closed without
   reading and without a reply; on Windows it never enters
   `PreAttestation`.
2. **A frame after the handshake** (state `Open` or `AwaitBinding`). The
   order of checks is 5.4, points 7 to 11; what each step holds:
   - The 4 length bytes and the 16 envelope bytes are read into the
     record's envelope buffer. Nothing is reserved for them, and no body
     byte is read until the envelope, the catalogue and the caller's
     authority have been checked (5.4, points 8 to 10).
   - **A refusal that keeps the connection** (`UnknownMessage`,
     `NotSupported`, `TooManyRequests`, the endpoint's authorisation
     code, or `Busy` below) reserves nothing. The daemon reads exactly
     the frame's remaining `length` − 16 bytes into the discard buffer,
     a chunk at a time, zeroing each chunk before the next is read,
     under the frame's own deadline, counted from its first byte; a
     deadline that passes first closes the connection. Only then is the
     refusal queued, as an `ErrorReplyBody` in the reserved reply slot.
     While that slot holds an unwritten frame the daemon reads nothing
     more from the connection but a `CANCEL` (5.6.4, rule 1), so a
     client is refused at most as fast as it reads its refusals. The
     next frame therefore starts at the right byte on every path.
   - **An admitted `REQUEST`** makes one reservation: its body's
     `length` − 16 bytes; T bytes of **terminal capacity**; and the
     row's decode charge, bytes and heap requests (5.2, point 7). If it
     fails, the request is refused as `Busy`, as above. The largest such
     reservation an endpoint can make is R (5.2, point 9), which the
     generator emits.
   - **An admitted `NOTICE`** reserves its body bytes and its row's
     decode charge. If that fails, the notice's body is discarded as
     above and the row's declared overload rule applies (5.6.1,
     point 4): `drop`, counted, or `fatal`. A notice has no reply to
     carry `Busy`.
   - A `CANCEL` has no body and reserves nothing.
   - **After the body is decoded and its extension lists are walked**
     (5.4, point 12, which decodes the typed values) the decode charge
     is reduced to what the decoders' ledgers recorded, bytes and heap
     requests (xdr.md 5.2, point 9.2), and the rest is released; never
     earlier. What remains is held until
     the decoded value is dropped. A body's buffer is held as long as
     any value borrows from it (xdr.md 5.1, point 8).
   - These are the only results that depend on the memory then free: a
     `REQUEST` that passed the checks before reservation answered
     `Busy`; a `NOTICE` that passed them dropped or fatal as its row
     declares; and rules 3 and 4's `Busy`, stream end and
     dropped event. What never depends on memory is exactly the checks
     made before the first reservation: a bad length, a bad envelope,
     an unknown number and an unauthorised caller get the same result
     on every path. A body is validated only if its reservation
     succeeded. One discarded for want of memory is never decoded, so a
     known, authorised frame with a malformed body is then answered
     `Busy`, or dropped or fatal as its notice row declares, and its
     malformation goes unseen. That is accepted `[OWN]`: validating a
     body needs the memory that is missing, the bytes are discarded
     whole, and nothing is applied either way.
3. **Handlers.** A handler reserves its largest working set before it
   allocates it, and a mutating handler before its first change. The
   working set of a non-streaming request includes its largest encoded
   reply and that frame's queue entry, so a reply, once built, never
   needs another reservation. A failed reservation ends the request as
   `Busy`, nothing applied, from its terminal capacity. An endpoint
   specification may name a request whose reply after a committed effect
   differs (an outcome that cannot be confirmed in time); its reply is
   still paid from the handler's working set.
4. **Output.**
   - Every queued frame is charged its framed size plus
     `ipc.overhead.frame` for its queue entry, against the connection's
     output budget and in both accounts.
   - An admitted request holds its terminal capacity until its one
     `REPLY` is queued or suppressed (5.4, point 13). Any terminal reply
     that is not the handler's own built reply (an error, `Busy`,
     `Cancelled`, `DeadlineExceeded`, a stream's early end) is encoded
     into it, and the capacity then is that queued frame's charge, entry
     included, until the frame is written or discarded: nothing is
     reserved anew and nothing is released in between. This holds on
     every path that releases a request's other reservations early (a
     full executor, 5.6.4, rule 3). When the handler's own reply is
     queued instead, the capacity is released.
   - **One lock.** Every append to a connection's queue (the check of
     headroom, the charge and the append itself) is one step under the
     connection's output lock, the lock that 5.4, point 13's closing
     step takes. Two producers never both count the same headroom, and
     an append either precedes the closing step or does not happen.
   - **A `PART` or an `EVENT` is admitted whole or not at all.** It is
     built only if the queue's occupancy plus the frame's whole charge
     is at most the output budget, and its bytes are reserved before it
     is encoded. A `PART` that does not fit waits: its handler blocks at
     a cancellation point until the queue has room, the request is
     cancelled, its deadline passes or the connection closes. A `PART`
     whose reservation fails ends the stream; the terminal reply reports
     the early end. An `EVENT` whose row is **droppable** and that does
     not fit, or whose reservation fails, is dropped and counted under
     the endpoint's rule. So no nonterminal frame takes the queue past
     its budget.
   - **Mandatory events have their own space** `[ADR 0004 C16.6: Firm]`
     ("An epoch advance is never dropped: space for it is kept free");
     the mechanism is `[OWN]`. An event row is droppable or mandatory
     (5.3.3, point 2). For an endpoint with mandatory rows the generator
     emits M, the whole charge, queue entries included, of the most
     mandatory events one connection can have pending at once (5.6.1,
     point 6). Every connection of that endpoint reserves M in its
     allowance (rule 1) at admission, so the space is kept free from
     then on.
     - A connection has two occupancies. **Ordinary occupancy** is the
       charge of its queued replies, parts and droppable events, held
       against the output budget. **Mandatory occupancy** is the charge
       of its pending mandatory events, held against M and against
       nothing else. A mandatory event is pending from the step that
       appends it until its last byte is written or the connection is
       closed, the time it is being written included.
     - A mandatory event is never tested against the output budget,
       against either account or against a paused reader: the rules
       above for a `PART` and a droppable `EVENT` do not apply to it. It
       is appended, under the output lock, behind the frames already
       queued, and is written in that order. Its space returns to M
       when it is written.
     - The append succeeds whenever the row's declared bound holds. An
       append past the bound is refused to its caller in the daemon,
       with nothing appended. The framework drops no mandatory event
       and decides nothing about the binding: which events are
       mandatory, how they are coalesced so the bound always holds,
       and what a refused append does to the binding are the endpoint
       specification's (for the adapter endpoint ADR 0004, S1; section
       13, Q6 of local-ipc.md lists what it must settle).
   - While a connection's queue is at or above its budget, the daemon
     reads no frame from it but a `CANCEL` (5.6.4, rule 1). A terminal
     `REPLY` whose bytes are already reserved is still appended, after
     the frames already in the queue. The queue can pass its budget only
     by the funded terminal replies of requests already in flight, at
     most the requests-in-flight budget of them, each within its own
     reservation, so both accounts still hold.
   - The reserved reply slot is outside the output budget and is used
     only for rule 2's refusals or for the closing notice (5.4,
     point 13). While it holds an unwritten frame, nothing more is read
     from the connection but a `CANCEL` (5.6.4, rule 1).
5. **Release.** Every reservation is released exactly once, in both
   accounts, when its bytes are freed, with the checked counters of
   section 6. A buffer the operating system still owns (an overlapped
   read or write on Windows) is freed, and its reservation released,
   only at that operation's terminal completion (ADR 0005, sheet 18). A
   request keeps its request slots (per connection while the connection
   lives, per caller key always), its executor place and every byte it
   reserved until its worker has returned, whether or not its connection
   is still open and whether or not a reply was sent (5.6.4, rule 5). A
   connection keeps its allowance and its place in the connection counts
   until its release (5.6.2).
6. **What is measured.** Every guarantee of this section is in
   requested bytes, as the XDR profile's ledger counts them (xdr.md 5.2,
   point 9), not the capacity an allocator returns `[D3 §5.2.9]`. An
   allocator's overhead per request and any surplus capacity are
   outside the accounts, and this specification states no byte bound
   for them. What it bounds is their number: every reservation states
   how many heap requests it covers, and the sum is the
   live-heap-requests budget of 5.2, point 8, reserved and released
   with the bytes. A deployment that needs a byte bound multiplies that
   count by its own allocator's worst overhead and surplus per request.
   Storage the operating system holds for a connection (socket buffers,
   the pipe buffers of 5.2, point 10) is outside both and is bounded by
   the connection count.
7. **Caller accounts.** The account record of a caller key exists
   exactly while the key has a live charge: a connection not yet
   released, a reservation, or a request whose worker has not returned.
   The record has a charge of its own, `ipc.overhead.caller` (256
   bytes, a ceiling asserted like rule 1's), reserved in the global
   account when the record is created and released when it is removed,
   so it is paid for however long detached work keeps it; a record that
   cannot be paid for fails admission's second step (5.4, point 2).
   It is removed with its last charge, so the records are at most the
   connections in all plus the requests still running after their
   connection closed, and those are bounded by the executor (5.6.4).
   No account, counter or mark outlives its last charge; the framework
   keeps no history per caller `[OWN]`.

So, in requested bytes, neither ceiling is passed, no caller can reserve
more than its own budget, and every admitted request's terminal reply is
paid for before its handler runs.

### 5.3 Versions

Not in this part: local-ipc.md, 5.3.

### 5.4 Decoding

The order of checks is local-ipc.md, 5.4. This part holds its points 2,
19 and 20.

**Accepting a connection (daemon)**

2. **Admission,** in two steps, each one atomic step under the
   daemon's admission lock `[OWN]`:
   - **At accept, before any read** (on Windows before the prefix read
     is started; on Linux and macOS before the caller is read): the
     connection count, then the handshake budget for all callers, then
     the allowance of 5.2.1, rule 1 in the global account. A connection
     that fails any of them is closed without a read and without a
     reply, and is never counted. One that passes holds its place in
     the connection count and its allowance from this step until its
     release, and its handshake slot until it is `Open` or released,
     whichever is first (5.2, point 10, Handoffs, on every operating
     system).
   - **When the caller is known** (Linux and macOS: at once, still
     before any read; Windows: at the capture, before any byte after the
     prefix is read): the handshake budget per caller key, then the
     allowance in the caller's account. A connection that fails either
     is closed without reading further and without a reply. So an
     attested caller holds at most its own handshake budget of
     handshake slots, on every operating system.
   - The open-per-caller budget is applied at `Hello` (point 5). Over
     it, the daemon still reads `Hello` (points 3 to 5) and replies
     `BUSY`, then closes, so the client can tell "busy" from a fault
     `[OWN]`. Such a connection is one of its caller's handshake slots
     while it lasts, so the per-caller handshake budget bounds them.

**The client**

19. **A stream, at the client** `[OWN]`. A client shows or acts on no
    part of a stream until the stream's terminal `REPLY` has been
    decoded and every check of points 12 and 18 has passed on every
    part and on the reply `[ADR 0004 N41.6: Firm]`. Until then it
    **stages** the parts, and it bounds what it stages:
    - at most the row's `parts_max` parts and `stream_bytes_max` bytes
      of part messages (5.2, point 11), counted with checked arithmetic
      as each `PART`'s length is read, before its body is read. The
      caller of the client library may set lower limits for a request,
      never higher;
    - an absolute **stream deadline** from the moment the `REQUEST` is
      written: the client's request deadline, at most 3,600 s;
    - one **staging budget** per client connection, shared by all its
      live requests: set by the caller of the client library, at most 64
      MiB. What is retained for a staged part is its body, the `length`
      − 16 bytes after the envelope, and one entry; the envelope is
      checked and not kept, and no decoded value is kept. Before a
      part's body is read the client reserves from that budget exactly
      that: the body's bytes plus `ipc.overhead.frame`. With a stream's
      first part the client also reserves, once, the row's largest
      decode charge for a part or its reply (5.2, point 7), and holds
      it until the stream has been shown or has failed. Under it each
      part is decoded once to make its checks, the value dropped, and
      decoded again, one part at a time, when the stream is shown; so
      neither the checks nor the showing can run out of memory part
      way. Staging is zeroed on release if the row is secret-bearing.

    Two kinds of failure are kept apart. A frame that breaks point 15
    (framing, envelope, a body that does not decode, `ExtensionOrder`,
    `ExtensionFlagMismatch`) **fails the connection**, as point 15
    says. A **request-level failure** leaves the connection open and
    fails only the request: a part that would pass a limit, a failed
    staging reservation, an unknown critical extension in a part
    (point 18), or the request deadline of point 20, which for a stream
    is its stream deadline. This is the **one failure transition** of a
    request, streaming or not, taken at most once per request id: on
    the first request-level failure the client releases what it staged,
    marks the request failed, queues one `CANCEL` (the only one that id
    ever gets), and starts the grace period `ipc.client.cancel_grace`
    (10 s) at that moment, whether or not the `CANCEL` can be written.
    From then on it reads that request's remaining frames by envelope:
    each envelope is checked as point 15 says, each body is discarded
    undecoded through the discard buffer, and the terminal `REPLY`'s
    envelope closes the request id. If the grace period ends, or a
    further `parts_max` parts are discarded, before that `REPLY`, the
    client fails the connection. A daemon within this specification
    never triggers either: it enforces the same row limits.
20. **Every request, at the client,** has a request deadline the caller
    of the client library sets, at most 3,600 s `[OWN]`. When it passes
    without a `REPLY` the client reports the request as failed with an
    unknown outcome (5.6.3, rule 4) and takes point 19's one failure
    transition for that request id, nothing else: the same single
    `CANCEL`, the same grace period, and from then on that id's frames
    are read by envelope only, bodies discarded, until its `REPLY`'s
    envelope closes it.

### 5.5 Encoding

Points 1 to 6 and 8 are in local-ipc.md. The Windows rows of point 7's
table, and its ownership rule, are in local-ipc-windows.md.

7. **Named zeroisation places** `[ADR 0004 O28.2, O28.3: Firm]`. Secret
   bytes pass through transport buffers before and after the typed
   value, in both directions. Each place has a label, used in code at
   the place. A side that cannot know a frame is secret-bearing before
   it has read the envelope treats every buffer of that kind as secret,
   on an endpoint whose catalogue has a secret-bearing message in that
   direction `[OWN]`.

   | Label | Side | Buffer | Zeroed |
   |---|---|---|---|
   | `ipc-rx-frame` | daemon | the receive buffer for one frame's body, on an endpoint with a secret-bearing client message | before reuse or release, on every path: decoded, refused at any step, truncated, timed out, cancelled, connection closed, daemon stopping; and not before every value that borrows from it is dropped |
   | `ipc-rx-discard` | daemon | the discard buffer (5.2.1, rule 2) | each chunk before the next is read, and at release |
   | `ipc-tx-daemon-stage` | daemon | the encoder output for a secret-bearing reply, part or event, while it is built and while it is queued | after its last byte is written; and on every path that ends it unwritten or partly written: an encode error, a suppressed reply (5.4, point 13), a dropped event, a failed or timed-out write, the closing deadline, connection closed, daemon stopping |
   | `ipc-tx-stage` | client | the checked connection's staging buffer for a secret-bearing message | after the last byte is written or the write fails, on an encode error, and on cancel or close |
   | `ipc-rx-client` | client | the receive buffer for one frame, on an endpoint with a secret-bearing daemon message | before reuse or release, on every path: decoded, refused, truncated, timed out, connection failed or closed; and not before every value that borrows from it is dropped |
   | `ipc-rx-client-stage` | client | the staging storage of a stream whose row is secret-bearing (5.4, point 19) | at release, on every path: shown, failed, over a limit, cancelled, deadline, connection failed |
   | `ipc-rx-client-discard` | client | the client's discard buffer (5.4, point 19) | each chunk before the next is read, and at release |

   Each buffer is allocated once, before it is filled, at exactly the
   size reserved for it (a frame buffer: that frame's own body or framed
   size, never above its frame cap; a discard buffer: 4 KiB; a prefix
   buffer: 4 bytes), and never grown by reallocation while it may hold
   secret bytes `[OWN]`, so no copy is left behind (xdr.md 5.1,
   point 8). No secret-bearing
   frame is copied from one buffer to another on either side: the
   encoder's output is the buffer that is queued and written, and the
   receive buffer is the one the decoded value borrows from. Each
   buffer above is inside a reservation of 5.2.1 on the daemon and
   inside the client's own budget on the client. The token-specific
   places (what `aklog` reads to build a token) are the control endpoint
   specification's.

### 5.6 States and ordering

5.6.1 to 5.6.3 are in local-ipc.md.

#### 5.6.4 Ingress, turns and the executor

ADR 0004, C39, points 2 and 3 `[ADR 0004 C39.2, C39.3: Working]`,
applied to the IPC endpoints; the mechanisms are `[OWN]`.

1. **Ingress threads only read.** The threads that serve the endpoints
   read frames, run the checks of 5.4, points 7 to 13, handle `CANCEL`,
   deadlines, closes and cleanup themselves, and write queued output.
   They never run a handler and never wait: not for a peer (every read
   and write is non-blocking or overlapped, and a partial frame's state
   is kept in the connection's record), not for the executor (rule 3),
   and not for memory (a failed reservation is a result, 5.2.1).
   - **Turns.** An ingress thread serves the **runnable** connections in
     turns. A connection is runnable when it has input to read, queued
     output (the reserved reply slot included) that can be written, a
     deadline that has passed, or a completion to deliver (a result, a
     transport completion). A connection with only output to write is
     served like any other; no reply waits for more input. One turn
     serves one connection and does at most one frame's work: the bytes
     of at most one frame that can be read without waiting, that frame's
     checks, its decode and walk, its handoff or refusal, or one discard
     chunk. Then the thread takes the next turn. A frame cap bounds a
     turn's work.
   - **Order of turns.** Turns go round-robin over the caller keys that
     have a runnable connection, one turn per key, and within a key
     round-robin over that key's runnable connections. A caller with many
     connections, or one that sends an unbroken run of small, refused or
     cancelled frames, gets one turn per round like every other caller;
     every frame it sends, whatever its fate, costs it a turn.
   - A connection in Windows `PreAttestation` has no caller key and
     takes no turn: its one read belongs to the pipe driver and nothing
     is decoded for it.
   - A turn writes at most one frame's bytes of the connection's
     output, besides its one frame of input work.
   - **`CANCEL` under back-pressure.** While a connection's reads are
     paused (5.2.1, rule 4: its queue at or above its budget, or its
     reserved reply slot full) the daemon still reads the next frame's
     length and envelope into the record, which needs no reservation. If
     that frame is a `CANCEL`, it is handled at once; any other frame
     waits at its envelope, body unread, until reading resumes. At
     most one envelope is read ahead. So a `CANCEL` that is the next
     frame is always handled. One that follows another frame is not: it
     waits behind that frame until output drains, and until then the
     stream is ended only by its own budgets, its request deadline or
     the write deadline. This limit is accepted `[OWN]`: reading past a
     frame would need its body buffered, which is what the pause
     refuses. A frame whose body the daemon had begun to read when the
     pause began is read to its end, since its bytes are reserved, and
     is handled when reading resumes.
2. **Arrival and the request deadline.** The ingress thread records a
   frame's arrival when its 4-byte length is complete (5.4, point 7). An
   admitted request's deadline is its arrival plus the request deadline
   of 5.2, point 8, or its row's lower one; it is an absolute time on
   the awake clock, fixed at admission and never extended. Time spent
   reading the frame, waiting in the executor's queue and running all
   count. The frame and write deadlines of 5.2, point 8 govern the
   transport only.
3. **Handoff without waiting.** After 5.4, point 12's checks the ingress
   thread first compares the request's deadline with the clock: a
   request already past it (its body arrived slowly) is answered
   `DeadlineExceeded` from its terminal capacity and is not offered.
   Otherwise it offers the request to the executor with one call that
   returns at once. A worker compares the deadline again when it takes a
   request, before any of the handler runs. The executor is bounded: its
   workers are a fixed set (ADR 0004, C39, point 1) and its queue holds
   at most the executor-queue budget of 5.2, point 8.
   - If the queue is full, the request is answered `Busy` from its
     terminal capacity (which stays charged as that frame's, 5.2.1,
     rule 4), its other reservations are released, and the refusal is
     counted. `Busy` is retryable (section 4): nothing was applied.
   - The queue is kept per caller key and served round-robin over the
     keys, one request per key per round. A caller's requests in flight,
     queued or running, are at most its per-caller budget, which is at
     most a quarter of the queue (5.2, point 9). So a caller cannot
     delay another caller's queued request by more than one request of
     its own per round, and cannot fill the queue alone.
   - **Workers.** With W workers, W at least 2, the executor starts a
     request only if its caller key then has fewer than ⌈W ÷ 2⌉
     running; the caller's further requests stay queued. So one caller
     never holds more than half the workers, rounded up, and at least
     one is left for the others. With one worker only the round-robin
     order is promised. Several callers together can still hold every
     worker until their deadlines (section 8).
   - A queued request whose deadline passes, whose `CANCEL` arrives or
     whose connection closes before a worker takes it is removed
     without running; nothing was applied, and it is answered
     `DeadlineExceeded` or `Cancelled`, or on a close not at all.
   - **A `NOTICE`** is never offered to the executor. Its consumer runs
     on the ingress thread inside the frame's turn, never waits, and
     does work bounded by the frame's size. A consumer that needs more
     than that puts the decoded notice on a queue its endpoint
     specification bounds in entries and bytes, with a call that returns
     at once; when that queue is full, the row's overload rule applies
     (5.2.1, rule 2). The notice's body and decode charge stay reserved,
     in both accounts, until the consumer has returned and any queued
     copy has been consumed.
4. **The one-shot result slot.** Each admitted request has one slot
   (ADR 0004, C39, point 3), completed exactly once, by whichever comes
   first: the handler's result, a `CANCEL`, the request deadline, or
   the connection's close (5.6.3, rule 3).
   - **The commit claim.** The slot is Pending, Claimed or Completed,
     and every transition is one step under the slot's lock (for a
     binding request, its connection's output lock, 5.4, point 6). A mutating
     handler makes one claim, immediately before its first change. The
     claim succeeds only if, at that step, the slot is Pending, the
     clock is before the request's absolute deadline, and the
     connection is neither `Closing` nor closed. The clock and the
     state are read in the step itself, so a deadline or close whose
     timer or notification has not yet run still fails the claim. A
     failed claim completes the slot as the cause says (`Cancelled`,
     `DeadlineExceeded`, or no reply on a close) and the handler
     changes nothing.

     | From | By | To | Who publishes, and what |
     |---|---|---|---|
     | Pending | the handler's result (a read, or a refusal) | Completed | the handler: its result |
     | Pending | `CANCEL`, the deadline, or a close | Completed | that path: `Cancelled`, `DeadlineExceeded`, or nothing on a close. Nothing was applied |
     | Pending | the handler's claim, passing the checks above | Claimed | nothing yet |
     | Claimed | the handler's result | Completed | the handler: its result, the ordinary success reply included |
     | Claimed | `CANCEL` | Claimed | nothing; the `CANCEL` is ignored |
     | Claimed | the deadline, on an ordinary row | Claimed | nothing; the handler's result follows |
     | Claimed | the deadline, on a row its endpoint specification names as one whose outcome can be unknown (5.6.3, rule 4) | Completed | the deadline path: `DeadlineExceeded` marked outcome unknown. The handler's later result is discarded and counted |
     | Claimed | a close | Claimed, then Completed by the handler | nothing is sent: the reply is suppressed (5.4, point 13) |

   - **After a claim** the handler finishes the change it claimed for
     and then completes the slot; it starts no other work. On an
     ordinary row that work waits on no peer and no server, which the
     endpoint specification must show for each mutating row, so the
     result follows the claim without a wait the deadline would have
     to bound. A row whose change waits on a server is one the endpoint
     specification names as outcome-unknown.
   - So `Cancelled`, and `DeadlineExceeded` not marked outcome unknown,
     always mean nothing was applied. The worker is given the
   deadline and a cancel token and checks them at its cancellation
   points; every wait a handler makes is bounded by the deadline.
5. **A request is charged until its worker returns.** Completing the
   slot sends or suppresses the reply; it does not free the request.
   Its per-caller request slot, its executor place and every byte it
   reserved stay charged until the worker has actually returned (or,
   for a request removed from the queue, at once), also after its
   reply was sent, after a `CANCEL` or deadline, and after its
   connection closed and was released. Its per-connection slot is held
   likewise while the connection lives. So work that outlives its
   reply or its connection is bounded by the same budgets as live
   work: at most the executor's workers plus its queue, and per caller
   at most that caller's budget, which stops a caller from starting new
   work by abandoning old.
6. **No work past the worker's return.** When a handler returns, its
   request's work has ended. An operation whose effect continues in the
   daemon afterwards (a store that is "not yet stored", ADR 0004, C9)
   continues under that subsystem's own bounded queues and budgets, as
   its endpoint specification names, never under the request.

## 6. Arithmetic

| Computation | Bound | At overflow or excess |
|---|---|---|
| in-flight counters (global and per caller key), heap-request, output-queue and terminal-capacity counters | each ≤ its budget | `checked_add` before a reservation, in both accounts or neither; subtraction only of what was added; a failure is a failed reservation or back-pressure, never a wrap |
| ordinary occupancy + a frame's whole charge (framed size + `ipc.overhead.frame`) | ≤ the output budget for a `PART` or a droppable `EVENT` | `checked_add`; above the budget the frame is not built |
| mandatory occupancy + a mandatory event's whole charge | ≤ M | `checked_add`; above M the append is refused to its caller (5.2.1, rule 4) |
| the allowance A and the largest request reservation R, from configured values | budgets ≤ their ceilings | 64-bit `checked_add` and `checked_mul` at validation; overflow refuses the configuration |
| arrival + the request deadline | deadline ≤ 3,600 s | the checked addition of `ironafs-time`'s deadline type; overflow refuses the request as `Internal` |
| per-caller and total connection counters; requests in flight per connection and per caller key; executor queue entries; parts and part bytes sent | ≤ their budgets | compared before the increment; never wraps |
| client: parts staged + 1; part bytes staged + a part's `length`; parts discarded + 1 | ≤ `parts_max`; ≤ `stream_bytes_max`; ≤ `parts_max` | `checked_add` before the part's body is read; above the bound fails the request (5.4, point 19) |

`[OWN]` for every row. The other rows are in local-ipc.md, section 6.

## 7. Errors and failure

The codes are local-ipc.md, section 7.

## 8. Security

The rows of local-ipc.md, section 8's table that rest on this part:

| Who | Could try | Stopped by |
|---|---|---|
| Any local user | take every connection or slot | the per-caller handshake and open-connection budgets on every operating system (5.2, point 8; 5.4, point 2); handshake and frame deadlines |
| Any local user | take the shared memory, the readers or the executor while inside its connection limits | the per-caller byte account and request budget (5.2.1; 5.2, point 8); one turn per caller per round, whatever the frames are (5.6.4, rule 1); a per-caller share of the executor's queue, served round-robin, and at most half the workers, rounded up (5.6.4, rule 3); a caller's account at most half the global byte budget (5.2, point 9); requests charged until their worker returns (5.6.4, rule 5). Residual: many distinct callers together can fill the executor; each then gets a retryable `Busy` |
| Any local user | keep work running by abandoning it | the request deadline from arrival; cancellation on close; slots and bytes held until the worker returns (5.6.4, rules 2, 4 and 5) |
| A slow or stalled reader | hold the daemon's memory | the output budget, checked against each whole frame; the write and closing deadlines; one frame of input at a time; both accounts (5.2.1) |
| Any local user | exhaust the budget so requests go unanswered, or desynchronise the stream with refused frames | terminal capacity reserved before the handler; the reserved reply slot; every length, envelope, catalogue and authority check before the first reservation, and a refused body discarded to its exact length, unvalidated (5.2.1, rule 2); the per-caller account |

## 9. How the invariants are kept

| Invariant | Kept by | Test that fails if broken |
|---|---|---|
| C39.2: ingress separate from execution; handoff without blocking; a full executor is a counted, retryable error | 5.6.4, rules 1 and 3; 5.4, point 14 | `ingress_never_blocks`; `executor_full_retryable`; `fair_turns` |
| C16.6: an epoch advance is never dropped; space for it is kept free | the space and the order only: M reserved in every connection's allowance, a mandatory occupancy apart from the output budget, appended in order (5.2.1, rules 1 and 4). That the epoch advance is a mandatory row, its coalescing and the fence are the adapter endpoint specification's (local-ipc.md, section 13, Q6) | `mandatory_event_reserved`; the rest with the adapter endpoint specification |
| C39.3: one-shot result slots with a deadline from arrival and a cancel token; every queue bounded in bytes | 5.6.4, rules 2, 4 and 5; 5.6.3, rule 3; 5.2, point 8; 5.2.1 | `request_deadline`; `charged_until_worker_returns`; `cancel_cases`; `stalled_reader`; `output_admission`; `global_budget_exhausted`; `caps_per_caller` |
| O28.2, O28.3: zeroisation at named places | 5.5, point 7 | `secret_buffers_wiped` |

## 10. Tests

Paths and conventions are those of local-ipc.md, section 10. 10.1, 10.3
and 10.4 are in local-ipc.md.

### 10.2 Round-trip properties

Item 7 of local-ipc.md, 10.2's list:

7. For every reservation sequence the seeded generator makes (admit,
   refuse, decode, reply, cancel, close, in any order), each account
   returns to zero when the last charge is released, the two accounts
   never disagree, and no caller's account passes its budget.

### 10.5 Acceptance cases

- `admission_order`: with a stand-in transport that records every call,
  a connection over the connection count, over the handshake budget, or
  without room for its allowance is closed with zero reads, on each
  operating system's transport (on the Windows stand-in, with no prefix
  read started); one over its caller's handshake budget is closed with
  no read after the caller is known.
- `refusal_consumes_body`: 10.3's refused requests with bodies, on the
  ordinary path and with the in-flight budget exhausted: the same code
  each time (or `Busy`, only for a request that passed the checks
  before reservation), the body
  discarded in zeroed chunks, the following request answered; a
  `NOTICE` with a bad envelope, an unknown number or no authority is
  fatal on both paths; a known, authorised one under exhaustion follows
  its row's overload rule whether its body is well formed or not.
  Paired cases, each for a known, authorised `REQUEST` and `NOTICE`
  with a malformed body: with the reservation made, fatal; with the
  reservation failed, `Busy` or the row's overload rule. A well-formed
  notice with the reservation made is consumed.
- `output_admission`: a `PART` and a droppable `EVENT` that would take
  the queue one byte past its budget are not built (the part waits, the
  event is dropped and counted); ordinary occupancy passes the output budget
  only by funded terminal replies, and mandatory events are counted
  apart from it. Two producers racing
  for the last headroom: one fits, the other waits or is dropped. A
  terminal reply from terminal capacity needs no reservation with both
  accounts at their budgets, its queue entry included.
- `mandatory_event_reserved`: on a test endpoint with a mandatory event
  row, with the output queue at its budget, both accounts exhausted and
  the reader stalled, the mandatory event is appended behind the frames
  already there and, when the reader resumes, is written after them and
  before anything appended later; once written, its space in M is free
  again and a second mandatory event is appended under the same
  saturation; with reads paused and a stream filling the queue, a
  `CANCEL` that is the next frame is still handled, one sent behind
  another request waits until output drains, and a frame half read when
  the pause began is read to its end and handled afterwards (5.6.4, rule
  1); an append past the row's declared bound is refused to its caller
  and nothing queued is lost.
- `stalled_reader`, `global_budget_exhausted`, `slow_peer`,
  `caps_per_caller`: the budgets of 5.2, point 8 and the reservation
  rules of 5.2.1, with allocator and counter checks: one caller at its
  byte budget gets `Busy` while a second caller's requests still run;
  every allocation, the decoder's and `Hello`'s `client_name` included,
  is inside a reservation: of both accounts, except the three charges
  that 5.2.1 makes in the global account only (a pipe instance's
  connect block, a connection's allowance before its caller is known,
  a caller's account record), which are checked against the global
  account alone; the decode charge is
  reduced to the ledgers' total only after the extension walk; heap
  requests stay within their budget; a caller's account record is gone
  after its last charge, and stays paid for while a detached worker
  keeps it after its connection's allowance was released; listener
  blocks and the capture scratch are inside a reservation before they
  are allocated; with one caller's account at its byte budget, a
  second caller can still reserve the other half of the global budget
  less the global-only charges then live.
- `fair_turns`, `ingress_never_blocks`: one caller with its full number
  of connections sending small, unknown and cancelled frames without
  pause, and a second caller with one connection: the second's request
  is read within one round and its latency stays within a stated bound;
  with every worker held by a stand-in handler, ingress still serves
  `CANCEL`, closes and deadlines; a reply queued for a client that
  sends nothing more is written without waiting for input; one caller
  never runs on more than half the workers, rounded up.
- `executor_full_retryable`: with the queue full, a request is answered
  `Busy` at once, counted, nothing applied, and the same request
  succeeds when sent again; one caller cannot take more than its share
  of the queue.
- `request_deadline`: a handler held past the deadline: the reply is
  `DeadlineExceeded`, counted from arrival (a frame sent slowly has less
  time left); a request whose body took longer than the deadline to
  arrive is answered `DeadlineExceeded` and never offered; a queued
  request past its deadline never runs. A mutating stand-in handler
  on an ordinary row, raced with `CANCEL` and with the deadline at
  every point: the reply is `Cancelled` or `DeadlineExceeded` exactly
  when nothing was applied, and the success reply exactly when the
  change was. With the deadline's timer held back by the stand-in
  clock, a claim made after the absolute deadline fails and nothing is
  applied; so does a claim on a connection already `Closing`. On a row
  named outcome-unknown, a deadline after the claim gives
  `DeadlineExceeded` marked outcome unknown, and the late result is
  discarded. A notice
  consumer's queue at its bound applies the row's overload rule and
  the ingress thread does not wait.
- `charged_until_worker_returns`: a caller that sends its budget of
  requests to a held handler and disconnects cannot start more from a
  new connection until workers return; its bytes stay in its account
  until then.
- `client_stream_budget`: a stand-in daemon sends one part too many, one
  byte too many, a part with an unknown critical extension followed by
  more parts, and a stream that never ends: the client shows no part in
  any case, sends one `CANCEL`, stages nothing further, and fails the
  connection at the grace limit, also when the `CANCEL` cannot be
  written; a part that does not decode fails the connection at once;
  after a request-level failure the terminal reply's envelope closes the
  id and the connection serves another request; two streams together
  never stage more than the connection's staging budget; a well-formed
  stream is shown only after its terminal reply.
- `config_bounds`: each policy budget at 0, at its ceiling, and one
  above; each rule of 5.2, point 9 broken; refused whole at start and on
  reload.
- `secret_buffers_wiped`: each label of 5.5, point 7, on the daemon and
  on the client, on each path it names, with a test hook that inspects
  the buffer at release; a test catalogue with a secret-bearing request,
  reply, part and event covers both directions. One case, with a
  stand-in transport: a secret-bearing write that has written part of
  its frame times out, the connection closes, and the write's terminal
  completion arrives afterwards; the buffer is neither zeroed nor freed
  before that completion and is zeroed at it.

## 11. Revisit if

- ADR 0004, C39 (Working): "M2 load tests show the fixed set starves a
  class of work, or lock-order checks find an RPC wait under a lock that
  cannot be restructured."
- This specification's own:
  - load tests show round-robin turns or the per-caller quarter of the
    executor's queue starving a legitimate caller (5.6.4);

## 12. Sources

The source table is local-ipc.md, section 12; this part uses its ids D1
to D5 and O. No other implementation's source, test data or interface
definitions were read for this specification, and none are cited.

### 12.1 Observed behaviour

None yet.

## 13. Open questions

None of its own. Those of the framework are in local-ipc.md, section
13; its Q6 is the open part of ADR 0004, C16, point 6.

## 14. Review

Not yet reviewed. The record will be in `docs/reviews/`.
