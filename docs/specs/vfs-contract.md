<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The contract between the core and the layers

Status: draft (2026-11-21). Raised in issue #67.

Kind: behaviour. Class: S, because the contract carries every caller's
authority into the core, fences every reply a layer writes, and carries the
invalidation stream that keeps the operating systems' caches coherent
([README](README.md), "Review class per kind": tokens, authorisation,
callbacks, the cache, concurrency).

Sections keep their numbers once the specification is reviewed. A section
that does not apply stays, with one line saying why.

## 1. What this implements

ADR 0004 is the decision set. This page carries it out; it decides nothing
an ADR decides, and restates no decision as its own.

| Decision | Points | Firm or working | Section here |
|---|---|---|---|
| C7 | 1 to 6 | Working | 5.1, 5.4 to 5.7 |
| C8 | 1 to 3 | Firm | 5.3, 5.11 |
| C9 | 1 to 5 | Working | 5.6, point 2; 5.8, points 3 and 4; 5.12, point 6 |
| C10 | 6 | Working | 5.6, point 9; 5.12, point 2 |
| C11 | 2, 4, 10 | Working | 5.2; 5.6, point 1 |
| C13 | 1 to 5 | Firm | 5.3; 5.5 |
| C14 | 1, 3 | Working | 5.3, point 3; 5.8, point 9 |
| C15 | 6 | Firm | 5.10, point 5 |
| C16 | 1 to 6 | Firm | 5.9 |
| C17 | 1 to 6 | Working | 5.10 |
| C37 | 1, 4, 5 | Working | 5.8 |
| C39 | 1 to 4 | Working | 5.8 |
| C44, C45 | C44.5; C45.2 to 4 | Firm | 2; 5.3 |
| C46 | 1 to 4 | Working | 5.12 |
| L1 | 1 to 7 | Firm | 5.2; 8 |
| L2 | 4 | Working | 5.12 |
| L6 | 1 to 5 | Working | 5.12, point 6 |
| L29 | 1, 2 | Firm | 5.2, point 2 |
| N41 | 1 to 4, 7, 9, 10 | Firm | 5.2, point 5 |
| N43 | 1, 2 | Working | 5.12 |
| N46 | 1, 2, 4 | Working | 5.5, points 1 and 5; 5.6, point 8 |
| N47 | 3, 5 | Firm | 5.10; 5.12 |
| O45 | 3 | Working | 5.8 |
| ADR 0004 S1 | | (specification item) | 2 |

The invariants this page must keep, word for word:

- **C7, point 4** (Working): "Binding operations carry no caller and change
  no data: forget, release, release directory, reclaim, cancel, volume
  sync, pin and unpin. They can only release or flush what the binding
  already holds."
- **C8, point 2** (Firm): "A layer cannot construct a caller capability or
  a handle capability, and cannot use one minted for another binding."
- **C9, point 4** (Working): "`release` never fails."
- **C9, point 5** (Working): "A commit reports success only when the
  server has accepted the store (point 1)."
- **C13, point 2** (Firm): "Where the front end can identify the open,
  every handle operation of C7, point 3, checks that the handle is live
  and takes the intersection of the rights at open with the current
  effective rights."
- **C16, point 2** (Firm): "A reply that fails its fence is never served."
- **C16, point 3** (Firm): "The last check is at **submission**: the
  binding checks the reply's context, generation, grant and node state in
  the same step that writes it to the kernel or to the SMB connection."
- **C16, point 5** (Firm): "**While a purge is pending** in a binding, the
  node serves nothing in that binding that needs the purged state."
- **C17, point 2** (Working): "Events are **acknowledged per binding,** in
  this order: new replies that need the purged state are gated (C16, point
  5); every reply the binding issued earlier that could fill the cache
  again is drained or fenced out (C16, points 3 and 6); the operating
  system's purge is applied; then the event is acknowledged."
- **C39, point 2** (Working): "A request that finds the executor full gets
  a retryable error and is counted."
- **C39, point 4** (Working): "**No lock is held across an RPC wait, a disk
  write or a kernel notification.**"
- **C46, point 4** (Working): "**A failure never looks like success:**
  never an empty or short listing, zero bytes or a made-up attribute."
- **L1, point 2** (Firm): a layer never "turns an error into success, an
  empty or short listing, zero bytes or a made-up attribute".
- **L1, point 7** (Firm): a layer never "waits without a deadline".
- **N41, point 4** (Firm): "**A request with an unknown critical extension
  is refused whole** (not supported), never applied in part."
- **N46, point 1** (Working): reserved operations are "refused with 'not
  supported' until the volume's capability offers them".
- **O45, point 3** (Working): "every OS request completes within its
  deadline".

## 2. Scope

1. **Covered:** the operations of C7, points 2 to 4, with their request and
   completion payloads and the caps on every value a layer supplies; the
   capabilities and how the core checks them; the request table, its
   completion owner, cancellation and deadlines, including the clock of a
   request's deadline (the seams specification's Q3); the fence check at
   submission for a binding in the daemon's process; the invalidation
   stream's acknowledgement rules; the error mapping per operating system;
   the rule that only the composition names `Mint`, and its check; and the
   values ADR 0004 leaves open (the extension caps, the name cap, the fence
   retry count, the request deadline).
2. **Not covered, and where it is covered:**
   - the adapter endpoint (ADR 0004, S1): its transport, registration,
     message set, credit, and the order of an out-of-process revocation's
     events. This page states what any binding must do; S1 states how a
     binding in its own process does it over IPC. The IPC framing and the
     numbering of contract errors on the wire are the local IPC
     specification's (issue #62);
   - the scheduling model, `Clock::wait`, and the simulator's mandatory
     deadline completions: the [seams specification](seams-and-scheduling.md)
     (5.3, 5.6, 5.7);
   - each layer's own protocol: the FUSE codec (issue #92), the macOS layer
     (with S3, the lookup fallback of L29, points 3 to 6), and the SMB front
     end (with S2, its framed units);
   - the macOS and Windows name codecs (L24, L25, L29, points 3 to 6): with
     their layers. Only the Linux codec is in `ironafs-vfs` today;
   - the attribute, ACL (N37), lock (L33) and status-change token (N47,
     point 1) values in full: this page fixes their caps and their place
     in the contract; their fields come with the file service's
     specification (issue #65) and the layer specifications;
   - the node table, aliases and pins as the core keeps them: issue #91;
   - commit's store path and journal (C28, C29): M3.

## 3. The design in short

1. **Three kinds of operation, one type each.** A caller operation takes a
   `&CallerCap`, a handle operation a `&HandleCap`, and a binding
   operation no capability to act with: only `release` and
   `release_directory` take a `HandleCap`, by value, to end it. The types
   make it a compile error to call a caller operation with a handle (C7,
   C8).
2. **Only the core mints.** The fields of a capability are private to
   `ironafs-vfs`. `Mint` makes them; only the composition names `Mint`, and
   a check over the workspace's sources holds that. The core checks every
   capability it is handed against its own table, so a capability from
   another binding or another mint is refused at run time (C8, point 2).
3. **A request table per binding.** Each request from the operating system
   gets one entry: its id, its arrival sample, its deadline, its cancel
   token and its state. The entry moves from open to claimed once, in one
   compare-and-set. The worker's submission, the deadline timer, a cancel
   and shutdown each try it; the one that wins writes the completion, and
   only it. The claim reads the clock: past the deadline, the completion is
   the deadline's error whoever wins.
4. **The completion owner** is the housekeeping loop. It waits with
   `Clock::wait` on the earliest open deadline and claims that request,
   whoever is still working on it.
5. **Deadlines are on the boot clock,** counted from arrival. A suspend
   that crosses a deadline completes the request at resume (pending the
   amendments of section 13, Q1).
6. **The fence check ends in the claim.** For a binding in the daemon's
   process, submission registers a sequence number, takes the object locks
   of every node the reply depends on in one order, compares the reply's
   ticket with the core's counters and gates, claims the request's entry,
   releases the locks, then writes to the kernel. A break recorded after
   the claim finds the reply delivered; the notifier drains every
   registered submission before it purges.
7. **Invalidations are coalesced per node and acknowledged by revision**
   after the operating system's purge. A purge that fails is reported, never
   acknowledged.
8. **One error type, mapped once per system,** by an exhaustive match; no
   mapping arm reaches success.

## 4. Terms

- **Core:** `ironafs-client` and the pure crates (ADR 0004, C4); here, the
  side of the contract that decides.
- **Layer:** the code that translates one operating system's requests into
  contract operations (L1). It is untrusted input to the core.
- **Binding:** one attachment of a layer to the core (a FUSE mount, an
  FSKit volume, an SMB share), named by a `BindingId` (C45, point 3).
- **In-process binding:** a binding whose layer runs in `ironafsd` (the
  FUSE layer). **Out-of-process binding:** one whose layer runs in its own
  process (the FSKit extension, the SMB front end; C16, point 6).
- **Contract request:** a request from the operating system, through a
  layer, that the core must complete once (the seams specification, 4).
- **Request table:** a binding's bounded table of open contract requests
  (5.8).
- **Claim:** the one compare-and-set that moves a request's entry from
  open to claimed; its winner, and only its winner, writes the completion
  (5.8, point 4).
- **Completion owner:** the loop that completes a request at its deadline
  independently of the worker serving it (5.8, point 6).
- **Ticket:** what a reply carries to submission: the recorded fence
  tuple of each object it carries state of (the callback specification,
  issue #66, section 4 and 5.6, points 2 and 3: the object's and the
  volume's break counters, the server's suspect epoch, the pause
  generation, the resume generation and the context's local volume
  epoch), the credential and rights generations, and the grant (C16,
  point 3).
- **Fence table:** the cache's table that holds each context's break
  counters and local volume epoch copies and the resume generation, behind
  one lock (the callback specification, 5.6, point 2).
- **Submission:** the step that checks a reply's ticket and claims its
  request's entry; a reply whose submission succeeded counts as delivered
  (C16, point 3).
- **Revision:** a binding's event counter: one sequence per binding,
  raised by every merge into any of its slots and by every overflow (C17,
  point 6).
- **Slot:** a node's pending invalidation in one binding: each pending
  effect with the revision at which it was last merged, and the latest
  revision delivered for the node (5.10, point 1).
- **Composition:** the crate that builds the daemon from the core and the
  drivers: `ironafsd`.
- **Cap:** the most a value may be, checked when the value is received.

## 5. Specification

### 5.1 Layout

This page specifies a Rust interface inside one process and the behaviour
behind it, not a byte layout. For an out-of-process binding the same values
are carried in the adapter endpoint's XDR messages (S1), under the caps of
5.2. The types live in `ironafs-vfs` (C7, point 1); the operations are
methods of one trait the core implements, `Core`, which each layer calls,
and one trait each binding implements, `Binding`, which the core calls to
submit replies and push events [ADR 0004 C7.1: Working].

1. **Request envelope.** Every contract request carries, beside its
   operation's payload: the `BindingId`; the `Ingress` record (its
   `RequestId`, its arrival `Stamp`, its cancel `WatchRef`); and an
   `Extensions` list [ADR 0004 N41.7: Firm]. No operation carries a second
   list in its payload. Binding operations carry the envelope and no
   capability to act with; `release` and `release_directory` take the
   `HandleCap` they end, by value (5.6, point 1).
2. **Completion envelope.** Every completion is `Result<Reply, VfsError>`
   for one `RequestId`, plus, for an out-of-process binding, the fence
   epoch of C16, point 6. No completion is both; no `Reply` carries an
   error code [ADR 0004 C46.4: Working].
3. **Ids.** `BindingId`, `ContextId`, `CredentialGeneration`, `HandleId`,
   `NodeId`, `RequestId` and `Revision` are 64-bit unsigned. Capability
   nonces are 128-bit, drawn by the core from its `Rng` [OWN: 128 bits make
   a guess no better than chance for the life of any binding; the value is
   checked, not trusted].

### 5.2 Caps

Every value a layer supplies is untrusted input (L1; overview, constraint
4). Each cap is checked when the value is built from the layer's bytes,
before any allocation sized by it, and a value over its cap is never handed
to operation code. "Refused" means the request completes with the error
named, and nothing else happens.

| Value | Cap | Over the cap | Label |
|---|---|---|---|
| A name (one component) | 1 to 255 bytes; no `/`, no NUL, not `.` or `..` | the layer's own name error (5.12, point 4) | [OWN] proposal, point 2 |
| An extension list | 16 extensions, 4,096 payload bytes in all, each tag once | `NotSupported` for the request | [OWN] proposal, point 5 |
| Read length | 1,048,576 bytes | `OutOfRange` | [OWN] point 3 |
| Write payload | 1,048,576 bytes | `OutOfRange` | [OWN] point 3 |
| Read-directory budget | 1,048,576 bytes of encoded entries | `OutOfRange` | [OWN] point 3 |
| Offset, and offset plus length | at most `i64::MAX`, by checked addition | `OutOfRange` | [OWN] point 4; [ADR 0004 C38.3: Working] |
| Lock range | start and end at most `i64::MAX`, by checked addition; length 0 means "to the end" | `OutOfRange` | [OWN] point 4 |
| Symbolic-link target (symlink) | 1 to 4,095 bytes, no NUL | `OutOfRange` | [OWN] point 6 |
| Extended-attribute name | 1 to 255 bytes, no NUL | `OutOfRange` | [OWN] point 6 |
| Extended-attribute read budget | 65,536 bytes | `OutOfRange` | [OWN] point 6 |
| Extended-attribute name list (reply) | 65,536 bytes, counted as each name's bytes plus one | `Protocol` | [OWN] 5.4, point 5 |
| An ACL value (set) | 65,536 encoded bytes; entries as N37's specification caps them | `OutOfRange` | [OWN] point 6 |
| A `control` request | the control endpoint's request cap (local IPC specification, 65,536 bytes) | `OutOfRange` | [OWN] point 6 |
| Time values (set attributes) | seconds as `i64`, nanoseconds below 1,000,000,000 | `OutOfRange` | [ADR 0004 C38.1, C38.3: Working] |
| Mode (set attributes, create) | at most `0o7777` | `OutOfRange` | [OWN] point 7 |
| Size (set attributes: truncate or extend) | at most `i64::MAX` | `OutOfRange` | [OWN] point 4; [ADR 0004 C38.3: Working] |
| Copy-range | both ranges as "offset plus length"; length at most 1,048,576 per call | `OutOfRange` | [OWN] points 3 and 4 |
| Allocate, punch | as "offset plus length" | `OutOfRange` | [OWN] point 4 |
| Extended-attribute write value | 65,536 bytes | `OutOfRange` | [OWN] point 6 |
| Named-stream selector (`open`) | as a name | the layer's own name error | [OWN] point 2 |
| `access` mask, `subscribe` filter, `open` flags, share access | only the bits this page defines | `OutOfRange` | [OWN] point 8 |
| A `control` reply | 1,048,576 bytes (the local IPC specification's reply cap) | `Protocol` | [OWN] point 6 |
| Subscriptions | one per directory handle; 1,024 per binding | a second on one handle: `Conflict`; past 1,024: `Busy` | [OWN] point 9 |
| Byte-range locks held | 4,096 per credential context | `Busy` | [OWN] point 9 |
| `forget` count | the references the binding holds | saturates; a layer fault, counted | [OWN] 5.6, point 4 |
| Open requests per binding | the request table's size, 1,024 entries | `Busy`, counted | [ADR 0004 C39.2: Working]; [OWN] size, 5.8 |
| Open handles per binding | the context's budget (C27, point 4) | `Busy` | [ADR 0004 C11.10: Working] |
| Pinned nodes | the pinning context's budget | `Busy` | [ADR 0004 C11.10: Working] |
| Acknowledgements and failure reports | only for a delivery the binding holds (5.10, point 6) | changes nothing; counted unless a repeat | [OWN] |

1. **Ids from a layer.** A `RequestId` is unique among the binding's open
   entries (5.8, point 1, says when an id is free again). A second request
   with the id of an open entry is dropped at ingress: it never enters the
   table, no completion is written under that id for it, the open entry is
   untouched, and a layer fault is counted [OWN: a completion under the
   id would answer the open request too]. For an out-of-process binding,
   S1 says whether the fault tears the binding down. A
   `NodeId` the binding does not hold, and a capability the core did not
   record for this binding, are refused as 5.3 says. No id from a layer is
   an index: each is looked up, and a miss is an error.
2. **The name cap: 255 bytes** (proposal). It is `MAX_NAME_BYTES` in
   `ironafs-vfs`. Reasons: macOS's `NAME_MAX` is 255 bytes `[E63: CONFIRMED]`
   (`sys/syslimits.h`); FSKit lets a volume declare its longest component and
   refuse a longer one with `ENAMETOOLONG` rather than truncate it `[E63:
   CONFIRMED]` (`FSVolume.h`, `maximumNameLength`, `truncatesLongNames`), so
   the macOS layer declares 255 and never truncates; Linux's `NAME_MAX` is
   255 bytes [UNVERIFIED: no registered source yet, section 13, Q6]. A
   longer name could reach a POSIX application only by truncation, which
   L29, point 1, forbids. A name the server returns that is not a valid
   `Name` fails the operation that met it (5.12, point 5). The Windows name
   table (L24, L25) maps names of up to 255 UTF-16 code units; whether every
   such name fits 255 bytes after L25's mapping is that table's question
   (section 13, Q6).
3. **Data sizes: 1 MiB.** One cap for a read, a write and a directory
   budget, so a worker's reply buffer is bounded per request; it leaves room
   for the adapter frame the local IPC specification sizes for 1 MiB of file
   data. A layer whose kernel asks for more splits the request; the FUSE
   layer negotiates a maximum no larger than this [OWN].
4. **Offsets** are `u64` on every system the layers serve, and a file's
   size and every offset the core accepts fit `i64`, so no value changes
   sign on its way to the wire codecs [OWN; ADR 0004 C38.3: Working].
5. **Extension caps: 16 extensions, 4,096 bytes** (proposal). These are
   `MAX_EXTENSIONS` and `MAX_EXTENSION_BYTES` in `ironafs-vfs`. Reasons:
   N41 asks for a bounded list so that it "cannot be a memory attack"; the
   only arms reserved now are the read results' hole extents (N46, point 4)
   and the data-transform envelope in status (N44, point 2), and a list per
   request of 16 arms in 4 KiB holds both with room; the byte cap is also
   the most one extension may carry, so a single arm cannot take a whole
   request's budget beyond it. The adapter contract keeps exact versions
   (C44, point 5), so a cap raised later is raised on both sides at once.
   A list over either cap is refused whole as not supported, as an unknown
   critical extension is [ADR 0004 N41.4: Firm].
6. **Other byte caps.** 4,095 bytes for a link target leaves the
   terminating NUL of a 4,096-byte path buffer; the macOS layer's shorter
   `PATH_MAX` (1,024 bytes `[E63: CONFIRMED]`) is that layer's limit to
   present, not the contract's. 255 bytes for an extended-attribute name and
   65,536 for a value follow the name cap and a common system limit
   [UNVERIFIED for Linux's own limit; section 13, Q6]. The ACL and
   `control` caps defer to the specifications that own those values; the
   contract's figure is a ceiling they may lower and never raise without
   changing this page. A `control` reply over its cap is a fault of the
   core's own reply, reported as `Protocol` (5.12).
7. **Mode.** Only the twelve permission and set-id bits cross the
   contract; a value with any other bit is refused. What the core stores of
   a set-id bit is the file service's question; what a layer shows never
   includes one (L3, point 2) [OWN].
8. **Bit sets.** A mask, a filter, open flags or share access with a bit
   this page does not define is refused whole; no unknown bit is ignored or
   passed on [OWN: an ignored bit is a request applied in part, which N41,
   point 4's rule forbids for extensions].
9. **Counts the core holds for a layer.** A directory handle carries at
   most one subscription; a second `subscribe` on it is refused with
   `Conflict`, and `unsubscribe` or the handle's release ends it. A binding
   holds at most 1,024 subscriptions, and a credential context at most
   4,096 byte-range locks (each with its owner token); past either, the
   request is refused with `Busy` [OWN: both are core memory a layer could
   otherwise grow without limit; the figures are generous for one desktop
   and are revisited with C27's budgets].

### 5.3 Capabilities

1. **Minting.** A caller capability is minted for one binding, one
   credential context and one credential generation, from the operating
   system's statement of the caller (`Attestation`): the uid in a FUSE
   request header `[E61: CONFIRMED]` (`fuse_in_header`), the uid in FSKit's
   caller context, or an authenticated SMB session [ADR 0004 C8.1: Firm].
   A handle capability is minted for one binding and one handle the core
   opened [ADR 0004 C13.1: Firm]. Each carries a fresh nonce. An
   attestation the core does not accept mints nothing, and the request is
   refused with `Access`: a uid other than a per-user FSKit binding's
   owner (C45, point 3), or an SMB session id the core did not
   authenticate for this binding or that has ended. A uid on a shared
   binding is always accepted: the core finds or creates its context, with
   or without tokens (C40, point 2), so no attested uid lacks one. An
   unknown or ended SMB session id is also counted as a layer fault; a uid
   the owner check refuses is not, since the operating system may send it
   [OWN: `Access`, because the caller, not the handle, is what failed].
2. **The core's record.** For every capability it mints, the core records
   `(binding, nonce)` with what the capability stands for: for a caller,
   the context and generation; for a handle, the handle's whole authority
   of C13, point 1 (context, generation, node, open mode, binding, rights at
   open, the volume's incarnation). The capability itself carries no
   authority the core trusts.
3. **The check.** On every caller or handle operation, before any other
   work, the core looks up the capability's nonce in its record for the
   request's binding. The operation is refused unless: the record exists;
   its binding is the request's binding and the capability's; and, for a
   caller, the context's generation is still the one recorded; for a
   handle, the handle is live (C13, point 2) and its generation current or
   suspended (C14). A capability with no record is refused with `Stale`; a
   caller capability of a retired generation with `Stale`; a capability
   recorded for another binding with `Stale`, and the event is counted as a
   layer fault [ADR 0004 C8.2: Firm; OWN: `Stale` tells the layer to
   attest the caller again, and names no other binding].
4. **Accessors are display.** `CallerCap::binding()` and the other
   accessors exist for logging and for the layer's own routing; the core
   never takes a decision from a value a capability reports about itself,
   only from its record [OWN].
5. **Handle capabilities have one holder.** `HandleCap` is not `Clone`;
   `release` and `release directory` take it by value. An out-of-process
   binding carries `(binding, handle, nonce)` on the wire; the front end's
   table plays the single holder there (S1) [OWN].
6. **The SMB and FSKit owner check.** On a per-user FSKit binding the core
   refuses any operation whose attested uid is not the binding's owner, uid
   0 included, before minting [ADR 0004 C45.3: Firm]. A caller capability
   for an SMB session is minted only for that session's context (L17).
7. **No default, no literal, no rebinding.** The compile-fail tests of
   10.5, point 1, show that a layer cannot write a capability as a literal,
   change its binding, copy a handle capability, pass a handle where a
   caller is expected, or take a default [ADR 0004 C8.2: Firm].

### 5.4 Caller operations

Each takes `&CallerCap` and the request envelope [ADR 0004 C7.2: Working].
Every name is a `Name` (5.2); every node is a `NodeId` the binding holds.
"Entry" means `(NodeId, Attributes)`; `Attributes` is the contract's status
value (5.7, point 1).

| Operation | Request payload | Completion payload |
|---|---|---|
| `lookup` | parent node, name | entry |
| `get_attributes` | node | `Attributes` |
| `access` | node, a mask of read, write, execute | nothing (success) or `Access` |
| `rights` | node | the caller's effective AFS rights on the node, as bits |
| `open` | parent node, name, disposition (open, create, create new, open or create, overwrite, overwrite or create), access mode (read, write, both, attributes only), share access (read, write, delete), flags (truncate, append, write-through, directory, non-directory), mode for a create, a named-stream selector (absent, or a name: reserved, 5.6, point 8) | `HandleCap`, entry, whether it created the file |
| `open_directory` | node | `HandleCap` |
| `create` | parent node, name, mode, flags as `open` | `HandleCap`, entry |
| `make_directory` | parent node, name, mode | entry |
| `unlink` | parent node, name | nothing |
| `remove_directory` | parent node, name | nothing |
| `rename` | source parent, source name, target parent, target name, `RenameFlags` (5.6, point 8) | nothing |
| `link` | node, new parent, new name | entry |
| `symlink` | parent node, name, target bytes | entry |
| `read_link` | node | target bytes |
| `set_attributes` | node, `SetAttributes` (5.7, point 2) | `Attributes` after the change |
| `statistics` | node | `FsStatistics` (5.7, point 3) |
| `lock` | node, `LockRequest` (5.7, point 4) | nothing; a conflict is `WouldBlock` or `Conflict` (5.7, point 4), naming no other holder |
| `get_acl` | node | `Acl` |
| `set_acl` | node, `Acl` | nothing |
| `extended_attribute` | node, name, read budget | the whole value; or, with a budget of 0, only its length |
| `list_extended_attributes` | node | every name |
| `set_extended_attribute` (reserved) | node, name, value, disposition (create, replace, either) | nothing |
| `remove_extended_attribute` (reserved) | node, name | nothing |
| `control` | the node the layer resolved (or none, for a command on no node), an `fs` command's encoded request | its encoded reply |

1. **`open` is one call.** Disposition and share access travel with the
   open, so an SMB create is one atomic call [ADR 0004 C7.2: Working].
   Share access is checked, reserved and enforced in the core's lock
   service; a layer only forwards it [ADR 0004 C7.6: Working].
2. **The parent is a node, never a path.** No caller operation takes a
   path; a layer resolves components one `lookup` at a time, so every
   component is authorised in the core (L5, point 1).
3. **`control`** carries the `fs` family of commands that act on a node the
   layer resolved. A descriptor from the operating system is accepted only
   after C7, point 5's checks [ADR 0004 C7.5: Working].
4. **POSIX ACL attribute names are refused** in `extended_attribute` and
   `list_extended_attributes` never shows them [ADR 0004 N42.7: Firm].
5. **Extended attributes are never truncated.** A value longer than a
   non-zero budget fails with `TooSmall` and returns nothing; a budget of
   0 asks only for the value's length; an absent attribute is
   `NoAttribute` [ADR 0004 C46.4: Working; OWN: the getxattr(2) shape the
   POSIX layers present]. `list_extended_attributes` takes no budget and
   returns every name: each system encodes a list differently, so each
   layer computes its own list's length, its size-only answer and its
   too-small error from the names [OWN]. A list whose names, counted as
   each name's bytes plus one, pass 65,536 bytes fails with `Protocol`
   (5.2).

### 5.5 Handle operations

Each takes `&HandleCap` and acts with the handle's authority, checked as
5.3, point 3, then C13, points 2 and 3, say [ADR 0004 C7.3: Working;
C13.2, C13.3: Firm].

| Operation | Request payload | Completion payload |
|---|---|---|
| `read` | offset, length | exactly `length` bytes, or fewer only at end of file, and the read result's `Extensions` (N41, point 10) |
| `write` | offset, bytes | the count written, always the whole payload |
| `read_directory` | a cookie (`u64`, 0 at the start), budget in bytes | entries (name, node, kind, the next cookie), and whether the listing ended |
| `commit` | `CommitReason`, durability (data, or data and attributes), boundary (all dirty data up to now) | nothing |
| `lock_handle` | `LockRequest` | as `lock` |
| `subscribe` | a change-notification filter (point 7), and whether the subtree is watched (a directory handle only) | nothing; events then arrive on the stream (5.10) |
| `unsubscribe` | none | nothing; idempotent when the handle has no subscription |
| `get_information` | none | `Attributes` |
| `set_information` | `SetAttributes` | `Attributes` |
| `set_disposition` | delete on close, or not | nothing |
| `rename_handle` | target parent, target name, `RenameFlags` | nothing |
| `seek` | offset, whence (data or hole) | the resulting offset |
| `copy_range` (reserved) | source offset, a second `&HandleCap` and its offset, length, the clone flag | the count copied, always the whole length |
| `allocate` (reserved) | offset, length, whether the size may grow | nothing |
| `punch` (reserved) | offset, length | nothing |

1. **A short read is the end of the file, and only that.** A read that
   cannot return every byte up to the end of file in its range fails; it
   never returns fewer bytes for another reason [ADR 0004 C46.4: Working].
   A hole reads as zeros [ADR 0004 N46.4: Working].
2. **`write` writes the whole payload or fails.** No partial count is
   reported [ADR 0004 C46.4: Working; OWN: a partial count lets an
   application believe a prefix is stored when nothing is].
3. **`read_directory`** returns whole entries until the budget is reached;
   an entry that does not fit ends the reply. A listing that fails after
   entries were returned in earlier replies fails its next call; a cookie
   the core did not issue for this handle, or one from before the
   directory changed, fails with `Stale` [OWN; ADR 0004 C46.4: Working].
4. **Rename and delete through a handle** also need the delete right on
   the source directory; a rename needs fresh authority on the target
   directory [ADR 0004 C7.3: Working].
5. **`seek`** treats the whole file as data: seeking for data returns the
   offset if it is below the size; seeking for a hole returns the size. An
   offset at or past the size fails with `PastEnd`, which the POSIX layers
   present as `ENXIO`, lseek(2)'s answer; the exact values at the end of
   the file are the Linux layer's (issue #92) [ADR 0004 N46.2: Working].
   It is a handle operation because it acts on an open file (C7, point 3's
   rule); C7, point 3, does not name it, and 13, Q7, asks for that line.
   C13, point 2 checks every C7, point 3 handle operation, including
   `subscribe` [ADR 0004 C13.2: Firm]. Every handle operation on this page
   uses that check, including `seek` and the reserved ones [OWN reading;
   13, Q7].
6. **`commit`** follows 5.12, point 6.
7. **The change-notification filter** is a set of the effect kinds of
   5.10: entries (names added, removed or renamed), attributes, data, and
   rights. These four bits are the only ones defined (5.2, point 8). A
   layer whose system offers finer filters maps each of its own onto these
   kinds, and an event reaches the subscription when its effects meet the
   filter; the SMB front end's mapping of its completion filter is S2's
   [OWN: the core can only report what its invalidations know].
8. **`unsubscribe`** is an ordinary handle operation, with the checks and
   the completion of any other (5.3, point 3; 5.8). Removing a
   subscription the handle does not have succeeds and changes nothing
   [OWN].

### 5.6 Binding operations, and the rules every operation keeps

1. **Binding operations** take the `BindingId` and what they name, and no
   capability to act with: never a `CallerCap`, never a `&HandleCap`.
   `release` and `release_directory` take the `HandleCap` they end, by
   value, so the layer holds no copy afterwards [ADR 0004 C7.4: Working]:

   | Operation | Request payload | Completion payload |
   |---|---|---|
   | `forget` | node, count | none: it has no reply |
   | `release` | `HandleCap` by value | nothing; it never fails |
   | `release_directory` | `HandleCap` by value | nothing; it never fails |
   | `reclaim` | node | nothing |
   | `cancel` | `RequestId` | none: it has no reply |
   | `volume_sync` | node (any node of the volume), or the whole binding | nothing, or the commit errors of 5.12, point 6 |
   | `pin`, `unpin` | node | nothing; `pin` may fail with `Busy` (C11, point 10) |
   | `acknowledge` | `Ack` (5.10) | none |
   | `purge_failed` | `PurgeFailed` (5.10, point 5): a node and the revision delivered, or a node the binding holds and the overflow's revision | none |

   `acknowledge` and `purge_failed` answer the invalidation stream; they
   release state the binding holds, as C7, point 4, requires, and are
   listed here so that every call a layer makes is in one of the three
   kinds [OWN].
2. **Release never fails, and holds no wait.** `release` and
   `release_directory` end the handle in the core's table at once, and a
   commit they need (L6, point 5) continues on its own under the handle's
   retained authority; its result goes to the side channel [ADR 0004 C9.4:
   Working; L6.5: Working]. Releasing a handle the core recorded for this
   binding that is no longer live (revoked, its generation retired, its
   node stale) is ordinary: it ends the record and is never a layer fault,
   since a FUSE kernel sends a release for every open, revoked or not.
   Only a `HandleCap` with no record for this binding (another binding's,
   another mint's, or one already released) is counted as a layer fault.
   Either way the release answers success [OWN: release's contract is "the
   layer holds nothing now", which holds].
3. **Binding operations change no data.** `volume_sync` flushes, through
   the commit path, only what the binding's handles already hold dirty,
   and is the one binding operation that waits: it has a request-table
   entry and a deadline as a commit has (5.8, point 3);
   `reclaim` and `forget` drop the binding's references; `pin` and `unpin`
   move the binding's pins. None names a caller, so none can act for one
   [ADR 0004 C7.4: Working].
4. **`forget` counts are bounded.** A count larger than the references the
   binding holds drops them all and is counted as a layer fault; it never
   wraps [OWN].
5. **Unknown critical extensions.** A request whose `Extensions` holds a
   critical tag the core does not know is refused whole with
   `NotSupported` before any effect; the reply to a request with unknown
   non-critical tags names them as ignored [ADR 0004 N41.4, N41.9: Firm].
6. **One completion.** Every request except `forget`, `cancel`,
   `acknowledge` and `purge_failed` gets exactly one completion (5.8). The
   four without a reply get none, and a layer never waits for one [OWN;
   for FUSE, a FUSE_INTERRUPT "requires no response" `[E61: CONFIRMED]`].
7. **No made-up values.** A completion's every field comes from the core's
   state; a field the core cannot fill fails the operation rather than
   being filled with a default [ADR 0004 C46.4: Working]. The creation time
   records whether the server supplied it (N47, point 6). A value from the
   server that the contract cannot carry (a name that is not a `Name`, a
   link target over its cap, a time outside C38's range) fails the
   operation with `Protocol` (5.12, point 5).
8. **Reserved operations are present and refused.** Copy-range (with or
   without the clone flag), allocate, punch, extended-attribute writes and
   removals, the named-stream selector in `open`, and rename flags
   (`RenameFlags::no_replace`, and exchange) are part of the contract.
   Until the volume's capability offers one, the core answers it with
   `NotSupported`, per operation, never by refusing the kind for the whole
   binding [ADR 0004 N46.1: Working; N43.1, N43.2: Working]. Each value a
   reserved operation carries is checked against 5.2's caps first, so a
   refusal never follows an unchecked value. Copy-range, allocate and punch
   are handle operations (two handles for a copy); extended-attribute
   writes and removals are caller operations (C7's rule) [OWN].
9. **A rename with flags, presented.** The contract value of a refused
   rename flag is `NotSupported` (N46, point 1). C10, point 6, says the
   application sees the operating system's "invalid argument" for "do not
   replace". Both hold: the POSIX layers present `NotSupported` from a
   rename with flags as `EINVAL`, a per-operation row of 5.12, point 2,
   and every other `NotSupported` as that table says [ADR 0004 N46.1,
   C10.6: Working; OWN: the reconciliation]. `Reserved::RenameFlags`'s
   refusal in `ironafs-vfs` is `NotSupported`, which this keeps.

### 5.7 Values

1. **`Attributes`:** node, kind (file, directory, symbolic link, mount
   point not yet resolved, special), size, link count, mode (permission
   bits only, 5.2, point 7), the displayed owner and group (display only,
   L30), access, modification and change times (C38's 64-bit wall time),
   creation time with whether the server supplied it (N47, point 6), the
   data version, the status-change token (N47, point 1; "none" under
   `afs3`), the coherency promise's kind and scope (N47, point 2), and
   `Extensions` [ADR 0004 N41.1: Firm]. Its full field list is fixed with
   the file service's specification (issue #65).
2. **`SetAttributes`:** each of mode, size, access time, modification time
   is either absent, a value, or (for times) "now"; the size is at most
   `i64::MAX` (5.2). An owner or group change
   is refused with `NotSupported` until L31's mapping is specified [OWN: a
   displayed owner grants nothing (L30, point 1), so no local id can be
   taken as an AFS owner].
3. **`FsStatistics`:** block size, total, free and available blocks,
   total and free files, the name cap. Where the server gives no figure,
   the operation fails; it never reports zero or a guess [ADR 0004 C46.4:
   Working].
4. **`LockRequest`:** kind (shared, exclusive, unlock), range (5.2),
   whether to wait, and the owner token the layer gives (an opaque 64-bit
   value naming the lock owner on its system). The rules are L33's; a
   waiting lock waits within its request's deadline. A lock that conflicts
   and was asked not to wait is `WouldBlock`; one that waited to its
   deadline is `TimedOut`; a share-access conflict at open is `Conflict`
   [ADR 0004 L33: Working; OWN: the split]. A conflict never names the
   other holder, so it tells no user who else on the machine holds the
   file (C10, "Why").
5. **`Name`** is defined in 5.2 and in `ironafs-vfs`'s `Name`.
6. **`CommitReason`:** flush, fsync, close, interval, write-through, sync
   [ADR 0004 C9.1: Working].

### 5.8 The request table, deadlines and the completion owner

1. **One table per binding.** The core keeps, per binding, a table of
   contract requests, bounded at 1,024 entries [OWN: twice a generous
   executor queue, so the table is never what refuses a request the
   executor would take; a figure measured at M2a load tests replaces it].
   An entry holds: the `RequestId`; the arrival `Stamp`; the `Deadline`
   (point 2); the cancel token; whether a mutation RPC has been sent for it
   (point 7); whether a store is in progress for it (point 4); and its
   state: **open**, then **claimed** (with the claimant: worker, deadline,
   cancel or shutdown, and the completion it chose), then gone. The id map
   that 5.2, point 1, checks holds open entries only: a claimed entry's id
   is free from the moment of its claim, before its completion is written,
   so a kernel that reuses an id as soon as it reads the reply never meets
   it [OWN]. Claimed entries stay counted in the binding's **writing set**
   until their completion is written, for teardown (point 9). The table is
   owned by the core's ingress side and lives in `ironafs-client`.
2. **The deadline.** The ingress records the arrival sample (`Ingress`).
   The core, never the layer, builds the deadline from that sample: the
   arrival's boot-clock reading plus the request deadline (point 3), on the
   **boot clock** (the clock that counts suspend) [ADR 0004 C39.3: Working;
   C37.1: Working, amended as 13, Q1 proposes]. A layer never hands the core
   a deadline or an arrival: for the FUSE binding, the reader that samples
   is the daemon's own thread; for an out-of-process binding, the daemon
   samples when the request arrives on the adapter endpoint, and the front
   end keeps its own deadline for the request its operating system sees
   (point 6) [OWN]. An ingress whose clock sample failed records no arrival
   and completes the request at once with `TimedOut`, never entering it in
   the table: no deadline is set without a valid sample [ADR 0004 C37.5:
   Working].
3. **The request deadline.** One value for every caller and handle
   operation and for `volume_sync`: 60 seconds by default, configurable
   from 1 to 3,600 seconds [OWN proposal: long enough for a fetch over a
   slow link with Rx's own retries, short enough that no process waits as
   an NFS hard mount would; section 13, Q2]. A commit's absolute deadline is
   the same value unless the configuration gives commit its own (C9, point
   3). Every other binding operation (`forget`, `release`,
   `release_directory`, `reclaim`, `cancel`, `pin`, `unpin`, `acknowledge`,
   `purge_failed`) waits on nothing: it completes, or ends with no reply,
   without a deadline wait and without a table entry.
4. **The claim.** An entry moves from open to claimed once, by an atomic
   compare-and-set on its state, which never blocks. Whoever attempts it
   (the worker at submission, 5.9; the completion owner, point 6; a
   `cancel`'s worker, point 7; shutdown, point 9) first takes one fresh
   `Clock::sample` and evaluates the deadline, doubt included (the seams
   specification, 5.3, point 4), then attempts the compare-and-set with
   the completion it will write:
   - if the deadline has passed, the completion is the **deadline's
     error**, whoever attempts: `NotYetStored` when a store is in progress
     for the request (a `commit` or a `volume_sync` whose store is still
     making progress, C9, point 3), otherwise `TimedOut`. A worker's
     success is never the completion of a claim made after the deadline;
   - otherwise, the attempt's own completion (the worker's reply, or
     `Interrupted`, or shutdown's error).

   **The winner of the compare-and-set, and only the winner, writes the
   completion** it claimed with (point 5); every other attempt finds the
   entry claimed and writes nothing. So exactly one completion is written
   for each request, by exactly one party, and a deadline that has passed
   is honoured by whichever party reaches the entry first [the seams
   specification, 5.7, point 4; OWN: the single-writer rule].
5. **Writing.** The winner writes the completion to the binding (for FUSE,
   the one reply whose `unique` is the request's `[E61: CONFIRMED]`), then
   removes the entry from the writing set. A write that fails (the kernel
   refused it, or the device closed) still removes it: the request is
   over, and the failure is logged [OWN].
6. **The completion owner is the housekeeping loop** (C39, point 1, gives
   it expiry). It waits with `Clock::wait` on the earliest open deadline of
   every in-process binding's table, through a deadline index kept beside
   the tables, and on that index's signal when an earlier deadline is
   added. When a deadline is reached it attempts the claim for that entry
   (point 4). It never blocks on a device, a `Disk` call or an `Rpc` wait
   [the seams specification, 5.7, point 4; ADR 0004 C39.4: Working]. Its
   completions are bounded by the tables' sizes. An out-of-process
   binding's front end is the completion owner for the request its
   operating system sees, with its own table and timer under the same
   rules; the daemon's table completes the IPC request on its side, and a
   late daemon reply finds the front end's entry claimed and is discarded
   (S1) [OWN].
7. **Cancel.** `cancel(request)` fires the entry's cancel token, and
   nothing else. The worker's next wait ends `Cancelled`, and it attempts
   the claim with `Interrupted` only while no mutation RPC has been sent
   for the request. From the moment a mutation is sent, the request's
   cancel no longer ends it: the worker waits for the outcome within the
   deadline, and a claim at the deadline completes with the deadline's
   error (point 4), the mutation's outcome read back and never repeated
   [ADR 0004 C28.4: Firm; OWN: `Interrupted` tells the application nothing
   happened, which is true only before a mutation leaves]. So **every wait
   after a mutation is sent** (its `Rpc::call`, a `Disk` call, a read-back
   RPC) takes only the shutdown cancel, never the request's. A `cancel`
   naming no open entry is ignored: the request was already answered, or
   never existed [OWN; FUSE documents that an interrupt may arrive for a
   request already answered `[E62: CONFIRMED]`]. On FUSE, the interrupt's
   own reply, if the layer sends one, is the FUSE codec's question: the
   kernel's documentation describes an `EAGAIN` reply to an interrupt
   `[E62: CONFIRMED]`, while fuse(4) says the interrupt "requires no
   response" `[E61: CONFIRMED]`; the codec's specification settles which it
   follows (issue #92). FSKit's volume operations offer no per-operation
   cancel in the macOS 27.2 headers inspected (only `FSTask` has a
   cancellation handler) `[E63: CONFIRMED]`, so an FSKit request ends at
   its completion or its deadline. An SMB cancel names its target by
   `MessageId` on the same connection `[E65: CONFIRMED]`; how the front end
   completes it is S2's.
8. **Executor full.** A request that finds the executor full is completed
   by the ingress at once with `Busy`, is counted, and never enters the
   table [ADR 0004 C39.2: Working]. A request that finds the table full is
   treated the same way.
9. **Shutdown and binding teardown** attempt the claim on every open
   entry with `Interrupted` (point 4 still turns a passed deadline into the
   deadline's error), so each open request is completed once, never with a
   success. Then they wait until the binding's writing set is empty, so a
   completion another party claimed is written before the binding closes;
   that wait is bounded by the kernel's acceptance of a reply, and a write
   that fails empties its place (point 5) [the seams specification, 5.7,
   point 4; OWN: the writing set]. Teardown also revokes the binding's
   handles [ADR 0004 C14.3: Working].
10. **Suspend.** A suspend that crosses an open request's boot-clock
    deadline completes the request at the first instant the host runs
    after it, with the deadline's error [the seams specification, 5.6,
    point 7]. This is a departure from O45, point 3, as written; 13, Q1,
    proposes its amendment.

### 5.9 The fence check at submission

This is how a binding in the daemon's process makes C16, point 3's check
and the write one serialised step without holding a core lock across the
write [ADR 0004 C16.3: Firm; C39.4: Working]. A binding in its own process
makes it with the fence epoch of C16, point 6, under S1 (point 7).

1. **The lock set.** A reply depends on every node whose state it carries
   or whose cache entry it lets the kernel fill. Its **lock set** is the
   object locks of those nodes, one lock per file object (aliases of one
   file share it), taken in ascending order of the object's key (cell,
   volume, vnode, uniquifier) and released together. This order sits at
   the object level of C39, point 4's order (server, volume, object, chunk)
   and is the only order in which two object locks are ever held together
   [ADR 0004 C39.4: Working; OWN: the order within the level].

   | Reply of | Lock set |
   |---|---|
   | `lookup`, `create`, `make_directory`, `symlink`, `open` (with a name) | the parent and the entry's node |
   | `link` | the node and the new parent |
   | `rename`, `rename_handle` | both parents, the moved node, and the replaced node if any |
   | `unlink`, `remove_directory` | the parent and the removed node |
   | `read_directory` | the directory |
   | `copy_range` | both files |
   | every other operation | the node it names, or its handle's node |

2. **The submission step.** `Binding::submit(request, Fenced<Reply>)`
   does, in order, with no yield from step 1 to step 5:
   1. **register:** take the binding's next **submission sequence number**
      by one atomic compare-and-swap that rises by 1 and refuses to pass
      `u64::MAX` (6, point 4). The advance itself puts the number in
      flight: every number below the counter is in flight until step 7
      marks it done, with no separate step that could be missed;
   2. take the lock set (point 1);
   3. **check:** run the fence check of the callback specification, 5.6,
      point 2, on each recorded tuple of the ticket, under the fence
      table's lock (point 3): it compares the clocks first (C32, point 5),
      then every value of the tuple (for a reply that is never cached,
      every value but the local volume epoch, its point 3), looked up
      with the request's current context and credential generation, so a
      credential change finds no record and fails it; then compare the
      ticket's credential and rights generations with the current ones;
      check that the handle or caller
      is still live, that no node of the lock set has a gate in this
      binding for the state the reply needs (point 5), and that the
      binding has no overflow pending;
   4. **claim:** attempt the claim of 5.8, point 4, with the reply if the
      check passed, or attempt nothing if it failed;
   5. release the lock set;
   6. if the claim was won with the reply, write the reply to the kernel,
      with no core lock held; if it was won with the deadline's error,
      write that error instead and discard the reply;
   7. mark the sequence number done, whatever happened above: a failed
      check, a lost claim, or a write that failed.

   If the check fails, the reply is discarded and `submit` returns
   `Fenced`; if the compare-and-set is lost, the reply is discarded,
   nothing is written and `submit` returns `Late`; if the claim is won with
   the deadline's error, `submit` writes that error and returns `Late`. A
   discarded reply is never written [ADR 0004 C16.2: Firm].
3. **Recording a change, and why the step holds.** Whoever records a
   break, a revocation, a staleness, a generation change or an overflow
   does so in the core before any event is queued (C16, point 1), and in
   the same step, under the locks it records under, it sets the gate of
   each affected binding's slot (or the binding's overflow) **before** it
   queues the event. The sequence counter's compare-and-swap and load are
   sequentially consistent atomic operations. Every write of a counter,
   generation, gate or overflow flag that step 3 reads is either made
   under a lock that step 3 holds while it reads the value, or is a
   sequentially consistent atomic store that step 3 reads with a
   sequentially consistent load [OWN]. The fence table's values (the
   break counters, the local volume epoch copies, the resume generation)
   and the suspect epoch are of the first kind, written and read under the
   fence table's lock; the pause generation is of the second, stored by
   Rx's driver with no lock and loaded by the fence check (the callback
   specification, 5.6, points 1 and 2; the Rx specification, 5.12, point 5).
   **The fence table's lock is a leaf** in C39, point 4's order: step 3
   takes it after the lock set, holds it for the fence check and no
   longer, and takes no lock while it holds it [OWN, within ADR 0004
   C39.4: Working; the callback specification, 5.6, point 2]. The check
   needs the table held exclusively, so no submission can check without
   it. Then, for any submission and any recorded change:
   - if the submission's registration comes before the notifier's read of
     the sequence counter for that change's event (point 4), the drain
     waits for it;
   - otherwise its registration comes after that read, which comes after
     the change was recorded, so its check (step 3) reads the change and
     fails: for a value written under a lock, step 3 takes that lock after
     the recorder released it (had it taken it first, its registration
     would come before the change, and so before the notifier's read); for
     an atomic value, its load follows the store in the single order of
     sequentially consistent operations.

   A change recorded under a node's object lock is also ordered by that
   lock against every submission whose lock set holds the node. A change
   recorded after step 5 finds the reply submitted, which counts as
   delivered (C16, point 3). Steps 2 to 5 hold the lock set, and step 3
   the fence table's lock within it, never across a wait or a write.
4. **The drain.** When the notifier takes an event for a binding, it reads
   the binding's sequence counter, then waits with `Clock::wait`, bounded
   by the event's deadline (5.10, point 8), until every sequence number
   below that reading is marked done. Only then does it apply the purge
   [ADR 0004 C17.2: Working]. **A drain that reaches the event's deadline
   with any of those numbers still in flight applies no purge and
   acknowledges nothing:** the event is treated as `purge_failed` for its
   node (or, for an overflow, for every node the binding holds), and every
   later purge on that binding also waits for those numbers [OWN: purging
   while a reply may still be written would let that reply refill the
   cache; failing closed is C15, point 6's path]. The writes waited on
   hold no lock, so a drain is bounded by the kernel's acceptance of a
   reply.
5. **Gated nodes.** From the moment a change is recorded until the binding
   acknowledges its revision, the node's gate in that binding fails step 3
   for a reply that needs the purged state, and new operations needing it
   wait for the acknowledgement within their deadline [ADR 0004 C16.5:
   Firm].
6. **Retries.** A reply that fails its fence at the install or at
   submission is discarded, and the read is retried at most 3 times in all,
   then fails with `Stale` [ADR 0004 C16.2: Firm; OWN: three attempts
   outlast a single break and a racing revalidation, and bound the work a
   break storm can cause; the seams specification's `fence_retries`]. A
   fenced mutation reply is never retried: its outcome is read back (C28,
   point 4).
7. **Out of process, the fence epoch carries the fence tuple** [ADR 0004
   C16.6: Firm]. A binding in its own process cannot run step 3's fence
   check, so this page owns telling it that a value of the check changed
   (the callback specification, 5.6, point 2, "What is not in the tuple").
   The daemon advances an out-of-process binding's fence epoch, and pushes
   it on the binding's ordered stream, after it records (C16, point 1)
   any change that could fail a reply of that binding at step 3:
   - every advance of the suspect epoch of a server key the binding holds
     data from;
   - every raise of a pause generation, for every binding holding data
     from a server on that family's socket, and every raise of the resume
     generation, for every binding;
   - every raise of an object's or a volume's break counter in a context
     the binding serves;
   - every change of a context's local volume epoch for a volume the
     binding holds data from;
   - every eviction from the fence table of an object or volume record of
     a context the binding serves, and, for every binding, the table's
     exhaustion: an evicted or exhausted value reads as no value, which
     matches nothing, so the fence check fails every tuple recorded under
     it (the callback specification, 5.6, point 2, "Values never
     repeat"), and a later break finds no record to raise;
   - every change of a credential or rights generation, and every
     revocation, as C16, point 6, already says.

   This is a duty, not a mechanism: which task observes each change and
   pushes the advance, how the daemon knows which bindings hold data from
   a server key or a record, and how a change made with no event behind
   it (a raise, an eviction, exhaustion) reaches the push are open
   (section 13, Q9).

### 5.10 The invalidation stream and its acknowledgements

The values are `ironafs-vfs`'s `Event`, `Invalidation`, `Overflow`,
`Effects`, `Entries`, `Rights`, `Scope` and `Ack`, and the `PurgeFailed`
report of point 5 [ADR 0004 C17: Working; N47.3: Firm].

1. **Coalesced per node, one revision per binding.** Each binding has one
   revision counter (`u64`, from 1). Every merge into one of its slots, and
   every overflow, takes the next revision. A node's slot holds each
   pending effect (data, attributes, entries, rights) with the revision at
   which it was last merged, and the latest revision delivered for the
   node; the union widens by `Effects::merge` and covers both
   (`Effects::covers`) [ADR 0004 C17.1, C17.6: Working]. Two different
   entry names, or a rename paired with any other names, widen to all
   entries; two contexts' rights widen to all contexts [ADR 0004 N47.3:
   Firm]. A delivery carries the slot's pending effects and the revision of
   its latest merge. **A slot has at most one delivery unanswered:** merges
   that arrive while one is in flight wait in the slot, and are delivered
   after the binding acknowledges or reports it; so the latest delivered
   revision of a node is the only one a valid report can name [OWN]. An
   overflow likewise is delivered once and stays **open** until
   `Ack::Overflow` for its revision.
2. **Order.** The core gates the node when it records the change (5.9,
   point 3). The binding then, for each event: drains or fences earlier
   replies (5.9, point 4); applies the operating system's purge; then
   acknowledges [ADR 0004 C17.2: Working].
3. **What an acknowledgement clears.** `Ack::Node { node, revision }` says
   the purge of the delivery carrying `revision` was applied. The core
   clears from the slot each effect whose merge revision is at or below
   `revision`; an effect merged later, even of the same kind, stays
   pending under its own revision, and the node stays gated until it too
   is acknowledged [ADR 0004 C17.6: Working]. `Ack::Overflow { revision }`
   clears the overflow of that revision; a slot merged after it stays.
4. **Only after the purge.** A binding acknowledges only after the
   operating system accepted the purge. For FUSE, a notification the kernel
   answers with "not cached" is a purge applied [UNVERIFIED: the kernel's
   answer for an uncached entry; settled with the FUSE codec, issue #92].
5. **A purge that fails** is reported, never acknowledged, with
   `purge_failed`, carrying a `PurgeFailed`: either `Node { node,
   revision }`, for a delivery of that node's slot, or `OverflowNode {
   node, revision }`, for one node the binding holds that failed under the
   overflow of `revision`. Under an overflow, the binding reports each
   node that failed and then acknowledges the overflow, which clears it for
   every node not reported. For each valid report the core applies C17,
   point 5, and C15, point 6, to that node, which on Linux is: the node
   answers `Stale` in every binding, its handles are revoked, and new opens
   and mappings of it are refused until a purge of it succeeds [ADR 0004
   C15.6: Firm; C17.5, C17.6: Working]. The core offers the failing binding
   the purge again, as a new delivery widened to every effect, and the
   node's stale state ends when that binding acknowledges it or is torn
   down [OWN].
6. **Reports are untrusted, and one rule covers both kinds.** A report is
   **valid** only if it answers an open delivery to this binding:
   - `Ack::Node` and `PurgeFailed::Node`: the revision equals the node's
     unanswered delivery (point 1); the first answer closes it;
   - `PurgeFailed::OverflowNode { node, revision }`: the revision is the
     open overflow's, the binding held the node when the overflow was
     delivered, and no report for that node under that overflow came
     before; the overflow stays open;
   - `Ack::Overflow`: the revision is the open overflow's; it closes it.

   A valid report acts as points 3 and 5 say. Every other report changes
   nothing, and falls in exactly one of two classes:
   - a **fault**, counted, when its revision is above the latest revision
     the core delivered to this binding for that node (or above the latest
     overflow delivered), when the binding never had a delivery for that
     node or does not hold it, when no overflow was ever delivered, or when
     an `OverflowNode` names a node the binding did not hold at the
     delivery of an overflow that is still open; for an out-of-process
     binding, S1 says whether faults tear the binding down;
   - otherwise a **repeat**: ignored, never a fault, whatever the slot's
     state now. That includes a second answer to a delivery, a second
     `OverflowNode` for one node under one overflow, any report naming an
     overflow already acknowledged, and a revision at or below the node's
     latest delivered one that the core delivered to another node.

   So a late `OverflowNode` for a node the binding holds now but did not
   hold at the overflow's delivery is a repeat once that overflow is
   acknowledged, and a fault while it is open. An honest layer cannot send
   the second: while an overflow is open every reply is fenced (5.9,
   point 2, step 3), so the binding can come to hold no new node [OWN].

   An acknowledgement never clears effects above its revision, and a
   failure report never revokes anything for a delivery it does not name
   [OWN; ADR 0004 C24.6: Firm: only demonstrated purge failure revokes].
   The core keeps the latest delivered revision of each node the binding
   holds, so the repeat rule is checkable; that memory is bounded by the
   node table.
7. **Rights events name a context.** The core sends a rights event to every
   binding holding the node, and a layer purges for the named context, or
   for all where it cannot tell contexts apart (FUSE's kernel caches are
   shared, L5) [ADR 0004 C17.3: Working].
8. **The event's deadline.** A binding that has neither acknowledged nor
   reported a failure within the request deadline (5.8, point 3) of the
   event's delivery is treated as having reported `purge_failed` for it
   [OWN: C16, point 5, gates the node until then; without a bound, a
   stalled layer would hold a node gated for ever and the core could not
   say the revocation completed].
9. **Overflow.** When a binding's pending slots reach their byte bound
   (C39, point 3; proposed: 1 MiB of slot state per binding), the binding
   gets one `Overflow`: every node it holds is gated and purged, or
   reported under point 5 [ADR 0004 C17.6: Working]. An event a layer does
   not understand is handled conservatively at its scope, or as an
   overflow where its scope cannot be determined [ADR 0004 N47.3: Firm].
10. **Who writes kernel notifications.** For an in-process binding, the
    notifier thread, holding no core lock (C17, point 4; L5, point 4).

### 5.11 Only the composition names `Mint`

1. **The rule.** The type `ironafs_vfs::Mint` is named only by
   `ironafs-vfs` (which defines it), `ironafsd` (the composition, which
   creates the one mint and gives it to the core), `ironafs-client` (the
   core, which holds it), and the test-only crates `ironafs-sim` and
   `ironafs-test-support` (the scripted core, issue #98); and, in `xtask`,
   only the module of the check in point 3 and its planted canary fixture,
   which name it to find it. No layer crate (`ironafs-fuse`, `ironafs-smb`,
   `ironafs-cloudfiles`) and no other crate or module names it. None of the
   allowed crates exposes a function or a macro that returns a `Mint`, or a
   capability built outside the core's record. `Mint` implements neither
   `Default`, `Clone` nor `Copy`, and `Mint::new` is its one constructor
   [OWN; it serves ADR 0004 C8.2: Firm].
2. **Why it is a rule and not only a type.** A crate that builds its own
   `Mint` can make capability values, but the core refuses every one it did
   not record (5.3, point 3), so the rule is a second barrier behind the
   run-time check, not the check itself. It keeps a layer's code from even
   looking like it mints.
3. **How it is checked.** Two checks, each proved to fire:
   - **A token ban.** `cargo xtask check` reads every Rust source file of
     every workspace crate outside point 1's list, as the compiler
     tokenises it (the source reader `check-deps` already uses), and fails
     on any identifier token `Mint`, in code, whatever path, alias,
     `extern crate ... as`, glob import or module re-export it reaches it
     through. Banning the token, not a path, is what closes the alias
     routes: every route that reaches the type must spell its name.
   - **A type ban.** The root `clippy.toml` lists `ironafs_vfs::Mint` under
     `disallowed-types`, so clippy, which resolves types, refuses any use
     of it in any crate; the allowed crates of point 1 each name it at their
     few sites under `#[expect(clippy::disallowed_types, reason = ...)]`.
     The token check also fails on any `allow` or `expect` of
     `clippy::disallowed_types` in a crate outside point 1's list.

   Both run against a planted canary crate that reaches `Mint` each of
   those ways (a direct path, `use ... as`, a crate alias, `extern crate
   ... as`, a glob import, a re-export from a module, an `expect` of the
   lint) and must fail on every one, as `check-bans` proves its bans fire.
   **Limits, which review covers:** stable Rust cannot assemble an
   identifier from macro fragments without a procedural macro, and no layer
   crate may depend on one (C4, point 4), so the token ban sees every name;
   a `macro_rules!` macro exported by an allowed crate that expands to
   `Mint` would hide it, and point 1 forbids such a macro. The checks
   belong with `check-deps`'s layer rules (C4, point 4), which already know
   which crates are layers [OWN; section 13, Q4, asks where they land].
   Until they exist, review holds the rule, as the crate's documentation
   says.

### 5.12 Errors, per operating system

1. **One error type,** `VfsError`, with C46, point 1's values and the
   seven this page adds (point 9); the token
   errors and `SecurityFloor` reach applications as a permission error and
   `OutOfRange` as an invalid-argument error, each with its cause in the
   side channel [ADR 0004 C46.1: Working]. Each layer maps it with an
   exhaustive `match` with no wildcard arm, so a new value does not compile
   until every layer maps it [ADR 0004 C46.2, C46.4: Working].
2. **The mapping.** Names are symbolic; their numbers differ by system and
   are each layer's to take from its system's headers. Linux names are from
   errno(3) `[E60: CONFIRMED]`; macOS names from the SDK's `sys/errno.h`
   `[E63: CONFIRMED]`, and the macOS layer reports each as an `NSError` with
   that POSIX code through FSKit's `fs_errorForPOSIXError` `[E63:
   CONFIRMED]`; NTSTATUS names and values from MS-ERREF 2.3.1 `[E64:
   CONFIRMED]`.

   | `VfsError` | Linux | macOS | Windows (proposed; S2 settles) |
   |---|---|---|---|
   | `NotFound` | `ENOENT` | `ENOENT` | `STATUS_OBJECT_NAME_NOT_FOUND` |
   | `Exists` | `EEXIST` | `EEXIST` | `STATUS_OBJECT_NAME_COLLISION` |
   | `Access` | `EACCES` | `EACCES` | `STATUS_ACCESS_DENIED` |
   | `TokenRequired` | `EACCES` | `EACCES` | `STATUS_ACCESS_DENIED` |
   | `TokenExpired` | `EACCES` | `EACCES` | `STATUS_ACCESS_DENIED` |
   | `TokenRevoked` | `EACCES` | `EACCES` | `STATUS_ACCESS_DENIED` |
   | `SecurityFloor` | `EACCES` | `EACCES` | `STATUS_ACCESS_DENIED` |
   | `ReadOnly` | `EROFS` | `EROFS` | `STATUS_MEDIA_WRITE_PROTECTED` |
   | `NoSpace` | `ENOSPC` | `ENOSPC` | `STATUS_DISK_FULL` |
   | `QuotaExceeded` | `EDQUOT` | `EDQUOT` | `STATUS_QUOTA_EXCEEDED` |
   | `CrossDevice` | `EXDEV` | `EXDEV` | `STATUS_NOT_SAME_DEVICE` |
   | `Stale` | `ESTALE` | `ESTALE` | `STATUS_FILE_INVALID` |
   | `Busy` | `EAGAIN` | `EAGAIN` | `STATUS_INSUFF_SERVER_RESOURCES` |
   | `Offline` | `ENETDOWN` | `ENETDOWN` | `STATUS_NETWORK_UNREACHABLE` |
   | `TimedOut` | `ETIMEDOUT` | `ETIMEDOUT` | `STATUS_IO_TIMEOUT` |
   | `NotYetStored` | `EIO` | `EIO` | `STATUS_UNEXPECTED_IO_ERROR` |
   | `Interrupted` | `EINTR` | `EINTR` | `STATUS_CANCELLED` |
   | `NotSupported` | `EOPNOTSUPP` | `ENOTSUP` | `STATUS_NOT_SUPPORTED` |
   | `WouldBlock` | `EAGAIN` | `EAGAIN` | `STATUS_LOCK_NOT_GRANTED` |
   | `Unreachable` | `EHOSTUNREACH` | `EHOSTUNREACH` | `STATUS_HOST_UNREACHABLE` |
   | `Conflict` | `EBUSY` | `EBUSY` | `STATUS_SHARING_VIOLATION` |
   | `OutOfRange` | `EINVAL` | `EINVAL` | `STATUS_INVALID_PARAMETER` |
   | `NotADirectory` | `ENOTDIR` | `ENOTDIR` | `STATUS_NOT_A_DIRECTORY` |
   | `IsADirectory` | `EISDIR` | `EISDIR` | `STATUS_FILE_IS_A_DIRECTORY` |
   | `NotEmpty` | `ENOTEMPTY` | `ENOTEMPTY` | `STATUS_DIRECTORY_NOT_EMPTY` |
   | `NoAttribute` | `ENODATA` | `ENOATTR` | `STATUS_NONEXISTENT_EA_ENTRY` |
   | `TooSmall` | `ERANGE` | `ERANGE` | `STATUS_BUFFER_TOO_SMALL` |
   | `PastEnd` | `ENXIO` | `ENXIO` | `STATUS_END_OF_FILE` |
   | `Protocol` | `EIO` | `EIO` | `STATUS_INVALID_NETWORK_RESPONSE` |

   **Per-operation rows.** A layer maps by value, and these operations
   override one value: on Linux and macOS, `NotSupported` from a rename
   with flags is `EINVAL` (5.6, point 9) [ADR 0004 C10.6: Working]. Any
   other per-operation row is the layer specification's, and is listed
   there; no per-operation row may map to success.

   The permission and invalid-argument rows are ADR 0004 C46, point 1's
   [Working]. Every other row is [OWN], chosen as the system's own error
   for that condition by its documented meaning, with these reasons:
   - `Busy` and `WouldBlock` share `EAGAIN`, which errno(3) documents as
     "Resource temporarily unavailable" and as the same value as
     `EWOULDBLOCK` on Linux `[E60: CONFIRMED]`, as on macOS `[E63:
     CONFIRMED]`; both are retryable, so sharing a code loses nothing an
     application acts on, and the side channel tells them apart;
   - `NotYetStored` is `EIO`, the error an application already treats as
     "the data you wrote is not safely saved" at `fsync` and `close`; a
     timeout code would read as "try again later" for data that is in fact
     only in the journal (C9, point 5);
   - a lock that cannot be granted without waiting is `WouldBlock`, so a
     non-waiting POSIX lock fails with `EAGAIN`, its usual answer;
     `Conflict` is for share access and for a change that collides, and
     `EBUSY`, "Device or resource busy" `[E60: CONFIRMED]`, is the nearest
     POSIX meaning;
   - `Stale` is `ESTALE`, "Stale file handle" `[E60: CONFIRMED]`, the
     condition it names;
   - the seven added values take the errors errno(3) documents for the
     same conditions: "Not a directory", "Is a directory", "Directory not
     empty", `ENODATA` as "The named attribute does not exist", "Result too
     large", "No such device or address" (lseek(2)'s answer for data or a
     hole past the end), and "Input/output error" `[E60: CONFIRMED]`; macOS
     names the missing attribute `ENOATTR`, "Attribute not found" `[E63:
     CONFIRMED]`; the NTSTATUS rows are error-severity values of MS-ERREF
     2.3.1 `[E64: CONFIRMED]`. `Protocol` and `NotYetStored` share `EIO`:
     both tell an application the operation did not do what it asked, and
     the side channel tells them apart;
   - on Linux `ENOTSUP` and `EOPNOTSUPP` are one value `[E60: CONFIRMED]`;
     on macOS they differ and `ENOTSUP` is "Operation not supported" `[E63:
     CONFIRMED]`. Never `ENOSYS`: L2, point 4, and N43 forbid it for any
     operation a volume could later offer, and the contract answers every
     operation itself.
3. **Windows: errors only of error severity.** An NTSTATUS's top two bits
   are its severity, and 3 means error `[E64: CONFIRMED]` (2.3). Every
   NTSTATUS in the table has error severity (values `0xC…`): MS-ERREF lists
   `STATUS_DEVICE_BUSY` as `0x80000011`, a warning, so it is not used for
   `Busy`, and `STATUS_NOTIFY_ENUM_DIR` (`0x0000010C`) is used only as the
   change notification's "rescan" (C17, point 5), never as an error `[E64:
   CONFIRMED]`. A test checks every NTSTATUS the SMB mapping can return has
   error severity [OWN: a warning or success status could let a client
   proceed as if the request worked, which C46, point 4, forbids]. The
   Windows column is a proposal; the SMB front end's specification (S2)
   fixes it against MS-SMB2's server rules, and may differ by operation
   (for example `STATUS_OBJECT_PATH_NOT_FOUND` for a missing intermediate
   component).
4. **Errors before the contract.** A layer refuses bytes that are not a
   `Name` before calling the core, with its system's own error:
   `ENAMETOOLONG` for `NameError::TooLong`, `EINVAL` for every other
   `NameError` on Linux and macOS; `STATUS_OBJECT_NAME_INVALID` on Windows
   (proposed; L24 and L25 settle it) [OWN]. A request whose extension list
   breaks its caps is refused as `NotSupported` (5.2, point 5).
5. **A server value the contract cannot carry** fails the operation with
   `Protocol`, cause in the side channel: a directory entry whose name is
   not a valid `Name`, a link target over its cap, a time outside C38's
   range, a reply over its cap (point 7). The listing or reply is never
   shortened to leave it out [ADR 0004 C46.4: Working; OWN: `Protocol`,
   because the server, not the caller, sent the value]. `OutOfRange` stays
   for a value the caller supplied that the protocol cannot carry (C38,
   point 3).
6. **`commit` and its errors.** `commit` (and `volume_sync`, for the
   commits it makes) returns success only when the server accepted the
   store; a claim at its deadline while the store is still making progress
   completes it with `NotYetStored`, whoever makes the claim (5.8, point
   4), never `TimedOut` and never success; otherwise the store's own error
   [ADR 0004 C9.3, C9.5: Working]. The Linux layer maps FLUSH and FSYNC to
   it as L6 says, and RELEASE never fails [ADR 0004 L6: Working; C9.4:
   Working].
7. **A reply over its cap.** An `Rpc` call that ends
   `NoReply(ReplyTooLarge)` (the seams specification, 5.4, point 2) fails
   its request at once with `Protocol`, is not retried, and does not mark
   the server unreachable; its own event, naming the server and the RPC,
   goes to the side channel [OWN: the peer answered with more than any
   reply the contract can carry; neither "timed out" nor "unreachable" is
   true, a retry meets the same cap, and "invalid argument" would blame
   the caller].
8. **An abort code** is classified by the RPC and the server that returned
   it, never by its number alone, before it becomes a `VfsError` [ADR 0004
   C46.3: Working]. That classification is the protocol specifications'.
9. **The seven added values.** C46, point 1, lists the values the error
   type "include[s]"; the layers need seven more to present their systems'
   errors without inventing them: `NotADirectory`, `IsADirectory`,
   `NotEmpty`, `NoAttribute`, `TooSmall`, `PastEnd` and `Protocol`. Each
   is a distinct condition the core decides and a layer cannot derive from
   another value, and each fails, never succeeds [ADR 0004 C46.1, C46.4:
   Working; OWN: the seven]. Section 13, Q5, records that a reading of
   C46, point 1, as a closed list would make this a Working amendment.
   Adding them changes the crate's enum (so every layer's match), and the
   local IPC specification's numbering of contract values (issue #62).

## 6. Arithmetic

1. `offset + length` for read, write, allocate, punch and lock ranges, and
   for both the source and the destination range of copy-range: checked
   addition in `u64`, then compared with `i64::MAX`; an overflow or an
   excess is `OutOfRange` (5.2). A `SetAttributes` size is compared with
   `i64::MAX` the same way.
2. The deadline: the arrival's boot reading plus the request deadline in
   nanoseconds, by checked addition in the `ironafs-time` deadline type; an
   overflow is `TimedOut` at ingress, never a wrapped deadline [OWN].
3. A binding's revision counter: `u64`, raised by one per merge or
   overflow with checked addition. A binding whose counter would overflow is
   torn down [OWN: unreachable in practice, and never a wrap that would let
   an old acknowledgement clear a new effect].
4. Submission sequence numbers: `u64`, raised by one per registration by
   a compare-and-swap that refuses to pass `u64::MAX` (5.9, point 2,
   step 1), never wrapping; a registration it refuses tears the binding
   down [OWN].
5. `forget` counts: subtraction saturating at zero, with the excess counted
   (5.6, point 4).
6. Extension list totals: checked addition against `MAX_EXTENSION_BYTES`
   (`Extensions::push`).

## 7. Errors and failure

1. Every refusal in 5.2 and 5.3 completes the request with the error named
   and leaves the core's state as it was; nothing is applied in part
   [ADR 0004 N41.4: Firm].
2. A capability check that fails never reveals what the capability named
   in another binding (5.3, point 3).
3. A layer fault (5.2, 5.3, 5.6, 5.10) is counted **per binding**, one
   counter for each binding whatever the fault's kind, and logged once per
   episode; it is never counted against another binding or against a
   credential context. Whether repeated faults tear a binding down is S1's
   for an out-of-process binding and the FUSE layer's for FUSE [OWN].
4. A failed clock sample at ingress completes the request with `TimedOut`
   at once (5.8, point 2); a failed sample at the claim treats the
   deadline as passed [the seams specification, 5.3, point 4].
5. Every failure path fails closed: an uncertain fence, an unacknowledged
   purge, a drain that did not finish, an unreadable deadline and an
   unknown critical extension all end in a refusal or a purge failure,
   never in a served reply or an applied purge.

## 8. Security

| Who | Could try | Stopped by |
|---|---|---|
| A buggy or compromised layer | to act for a caller it does not serve | 5.3: no minting outside the core, the record check per binding (C8, point 2; C45, point 4) |
| The same | to use a capability of another binding | 5.3, point 3 |
| The same | to exhaust the daemon's memory with large values or many requests | 5.2's caps, the bounded table (5.8, point 1), the bounded slots (5.10, point 9) |
| The same | to clear an invalidation it never applied | 5.10, points 3 and 6: an acknowledgement clears only up to its revision; a forged one only hides the purge in its own operating system, which it already controls |
| The same | to revoke other users' handles, in other bindings, with forged failure reports | 5.10, point 6: a report naming no delivery the core made to this binding changes nothing and is counted |
| The same | to hold locks or subscriptions without limit | 5.2, point 9 |
| The same | to hold nodes gated for ever | 5.10, point 8 |
| A local user | to read data after a revocation through a reply already in flight | 5.9: the lock set and the fence at submission; 5.9, point 3: registration before the check; 5.9, point 4: the drain before the purge, failing closed |
| A slow or hostile server | to hang a process | 5.8: every request completes by its deadline, by the completion owner, whatever the worker is doing |
| The same | to send an oversized reply | 5.12, point 7 |
| A layer's own error mapping | to turn a failure into success | 5.12, points 1 to 3: exhaustive matches, per-operation rows that never reach success, error severity only |

What this page does not stop: a compromised out-of-process front end can
act for the users it currently serves on its own bindings (ADR 0004 C8,
"Why"; C45, point 4).

## 9. How the invariants are kept

| Invariant | Kept by | Test that fails if broken |
|---|---|---|
| C7, point 4 | 5.6, points 1 to 3 | `ironafs-vfs`: compile-fail `binding_op_takes_no_caller`; `ironafs-client`: `binding::volume_sync_flushes_only_held` |
| C8, point 2 | 5.3; 5.11 | `ironafs-vfs`: the compile-fail doctests of `capability.rs`; `ironafs-client`: `capability::other_binding_refused`, `capability::foreign_mint_refused`; `xtask`: `mint_check_fires_on_canary` |
| C9, point 4 | 5.6, point 2 | `ironafs-client`: `binding::release_never_fails` (including a stale cap) |
| C9, point 5 | 5.8, point 4; 5.12, point 6 | `ironafs-client`: `commit::not_yet_stored_at_deadline`, `commit::deadline_claim_by_worker_is_not_yet_stored` (M3) |
| C13, point 2 | 5.3, point 3; 5.5 | `ironafs-client`: `handle::lost_right_refused_kept_right_works` |
| C16, point 2 | 5.9, points 2 and 6 | `ironafs-client`: `read::break_before_submission_is_fenced` (the seams specification's) |
| C16, point 3 | 5.9, points 1 to 3 | `ironafs-client`: `submit::break_after_claim_is_delivered`, `submit::parent_break_fences_lookup`, `submit::generation_change_between_register_and_check` |
| C16, point 5 | 5.9, points 3 and 5 | `ironafs-client`: `binding::gate_set_when_recorded` |
| C17, point 2 | 5.9, point 4; 5.10, points 2 and 4 | `ironafs-client`: `binding::drain_before_purge`, `binding::drain_timeout_is_purge_failure`, `binding::ack_only_after_purge` |
| C39, point 2 | 5.8, point 8 | `ironafs-client`: `ingress::executor_full_is_busy_and_counted` |
| C39, point 4 | 5.8, point 6; 5.9, points 1 and 2 | `ironafs-client`: `submit::no_lock_across_kernel_write`, `submit::lock_set_in_key_order` (lock-order check in the simulator) |
| C46, point 4; L1, point 2 | 5.5, points 1 to 3; 5.12, points 1, 3, 5 | `ironafs-vfs`: compile-fail `mapping_without_wildcard`; `ironafs-fuse`: `errno::every_value_maps_to_an_error`; `ironafs-smb`: `ntstatus::all_error_severity`; `ironafs-client`: `readdir::bad_server_name_fails_listing` |
| L1, point 7; O45, point 3 | 5.8 | the simulator's invariant "every OS request completes within its deadline" (the seams specification, 10.5); `ironafs-client`: `request::deadline_beats_stuck_worker`, `request::late_worker_writes_deadline_error`, `request::one_writer_per_request` |
| N41, point 4 | 5.6, point 5 | `ironafs-vfs`: `extension::first_unknown_critical`; `ironafs-client`: `request::unknown_critical_refused_whole` |
| N46, point 1 | 5.6, points 8 and 9 | `ironafs-vfs`: `reserved::every_reserved_refused`; `ironafs-client`: `reserved::refused_per_operation`; `ironafs-fuse`: `errno::rename_flags_is_einval` |

## 10. Tests

### 10.1 Golden vectors

None: this page defines no byte layout. The adapter endpoint's encoding is
S1's, with its own vectors.

### 10.2 Round-trip properties

1. The Linux name codec: for every `Name`, `linux::decode(linux::encode(n))
   == n`; for all bytes, `decode` either refuses or returns a name whose
   bytes are the input (`ironafs-vfs`, `tests/it/name.rs`, and the fuzz
   target of 10.4).
2. `Effects::merge`: after `a.merge(b)`, `a` covers the old `a` and `b`
   (`ironafs-vfs`, `invalidation` tests, property form).

### 10.3 Reject corpus

For `Name::new`: the empty string; `.`; `..`; any byte string with `/` or
NUL; 256 bytes (5.2). For `Extensions`: a 17th extension; 4,097 bytes in
one extension; 4,097 bytes over two; a repeated tag (5.2, point 5). Each
lives in the crate's unit tests. For the core's checks (`ironafs-client`
and the scripted core): a size of `i64::MAX + 1`; an offset and length
whose sum overflows, for each range of a copy-range; an `access` mask or
open flags with an undefined bit; a 1,025th subscription on a binding; a
second subscription on one handle; a 4,097th lock in one context (5.2).

### 10.4 Fuzz target

`fuzz/fuzz_targets/vfs_name_linux.rs`: arbitrary bytes into
`linux::decode`; asserts no panic, and that an accepted name's bytes equal
the input and pass every rule of 5.2, point 2. It runs under the fuzzing
job of issue #45 for the duration that job states.

### 10.5 Acceptance cases

1. **No layer mints.** Compile-fail tests: a literal capability, a
   changed binding, a cloned `HandleCap`, a `HandleCap` passed as a
   `CallerCap`, a `Default` capability, a `Default` or cloned `Mint`, and a
   binding operation other than `release` and `release_directory` called
   with a capability argument. Each must fail to compile with the error
   code named (rule 15).
2. **Another binding's capability.** Start: two bindings, a caller
   capability minted on the first. Events: a lookup on the second with it.
   Result: `Stale`; the fault is counted; nothing is looked up (issue #98's
   scripted core, then `ironafs-client`).
3. **Deadline beats a stuck worker.** Start: a read whose worker is held in
   a `Disk` call. Events: virtual time reaches the deadline. Result: the
   completion owner completes it with `TimedOut` at the deadline's instant;
   the worker's later submission returns `Late` and writes nothing.
4. **Late success discarded, deadline still written.** Start: a read
   whose worker finishes after the deadline, before the completion owner's
   timer runs. Result: the worker wins the claim, which evaluates the
   deadline as passed; the worker writes `TimedOut` and discards its
   reply; the completion owner's later attempt finds the entry claimed and
   writes nothing. Exactly one completion is written. The same case with a
   commit whose store is making progress writes `NotYetStored`.
5. **Cancel before and after a mutation.** Start: a create. Events: a
   cancel before the RPC is sent; separately, a cancel after it is sent.
   Result: the first completes `Interrupted` and sends nothing; the second
   waits for the outcome and completes with it, or `TimedOut` at the
   deadline with the outcome read back.
6. **Suspend across a deadline.** Start: an open request on the boot clock.
   Events: a suspend whose jump crosses the deadline. Result: `TimedOut`
   at the resume instant, before any other entry runs (the seams
   specification, 5.6, point 7).
7. **Fence and claim are one step.** Start: a read reply ready to submit.
   Events, each order across a seed sweep with an interleaving witness (the
   seams specification, 5.6, point 6): a callback break recorded, and the
   submission's claim. Result: break first, the reply is fenced and
   retried; claim first, the reply is delivered and the notifier's drain
   waits for its write before the purge. Repeated for a `lookup` with the
   break on the parent, and for a credential generation change recorded
   between the submission's registration and its check: in each order the
   reply is fenced or drained, never written after the purge.
8. **A second event during a purge.** Start: a rights event and an entry
   event on one node, merged. Events: the binding begins the purge; a data
   event arrives; the binding acknowledges the first revision. Result: the
   data effect stays pending under its revision and the node stays gated
   until it is acknowledged (C17, point 6's test).
9. **Forged and repeated reports.** Events: an acknowledgement above the
   latest delivered revision; one for a node the binding does not hold; a
   `purge_failed` for a revision never delivered; then a valid
   acknowledgement sent twice. Result: the first three change nothing and
   count three faults, and no handle in any binding is revoked; the repeat
   is ignored and counts nothing.
10. **A purge that fails.** Events: `purge_failed` for a delivered revision
    of a node two contexts hold. Result: both get `Stale` on that node only;
    their data and records are kept (C15, point 6's test). Under an
    overflow: one node reported failed, then the overflow acknowledged; only
    that node is stale.
11. **Reply too large.** Start: a fetch whose reply exceeds its cap.
    Result: `Protocol` at once, no retry, the server not marked
    unreachable, one event.
12. **Executor full.** Result: `Busy`, counted, no table entry.
13. **Reserved operations.** Each of N46, point 1's operations answers
    `NotSupported` per request, and the next request of the same kind
    reaches the core again (no `ENOSYS`, N43).
14. **A drain that cannot finish.** Start: a submission registered and
    held before its write. Events: an event for that node reaches its
    deadline. Result: no purge is applied, nothing is acknowledged, and the
    node is handled as `purge_failed`.
15. **Teardown waits for claimed writes.** Start: a request whose worker
    has claimed and not yet written. Events: teardown. Result: the
    binding closes only after that write; every other open request gets
    `Interrupted` once.
16. **An id reused at once.** Start: a request claimed and being written.
    Events: the kernel sends a new request with the same id before the
    write returns. Result: the new request enters the table (the id was
    freed at the claim) and is answered once.

**Gaps:** every `ironafs-client` test above waits for that crate's
operation code (issues #91, #98 and the M2a operation briefs); the `xtask`
Mint check waits on section 13, Q4; the Windows and macOS mapping tests wait
on their layers.

## 11. Revisit if

Copied from the working decisions this page rests on:

- **C7:** "building a layer shows an operating-system request that fits
  none of the three kinds, or needs a caller the kind does not carry."
- **C9:** "a layer's gate shows a commit point that the reasons below
  cannot express, or that the deadline rule makes an application lose data
  it was told was saved."
- **C14:** "the M2b tests show that renewing a token interrupts I/O in
  progress, or the ten-minute bound in point 5 proves too long or too
  costly against the test cell."
- **C17:** "measurements in M2a show coalescing per node loses an event a
  layer needs, or a layer's operating system offers a purge that cannot
  fail."
- **C37:** "the M2 sleep, hibernation, Modern Standby and VM-pause tests
  show a clock below does not behave as its documentation says, or a
  supported target lacks a valid pair."
- **C39:** "M2 load tests show the fixed set starves a class of work, or
  lock-order checks find an RPC wait under a lock that cannot be
  restructured."
- **C46:** "a layer's error mapping cannot express a contract error without
  losing what the user needs to act on it."
- **N43:** "the kernel's handling of `ENOSYS` for these requests changes so
  that it is no longer permanent for the mount."
- **N46:** "the M2a check below finds that the kernel or common tools do not
  fall back when copy-range is unsupported; or a supported cell offers one
  of the reserved operations."
- **O45:** "the simulator cannot reproduce a failure seen against a real
  cell, or keeping its server models in step with observed behaviour costs
  more than the tests return."
- **L2, L6, L29 (point 3):** as their own triggers state.

This page's own choices reopen if: the M2a load tests show the request
table or the 1 MiB data cap refuses work the executor could do; a layer
shows a name over 255 bytes that its users need; the fence retry count is
reached in a measured break storm without a real staleness; or a binding's
acknowledgement deadline (5.10, point 8) fires on a healthy layer.

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| ADR 0004 | DECISION | [ADR 0004](../adr/0004-architecture.md), parts 1 to 4, accepted 2026-11-20: the decisions of section 1, and S1, S2 | throughout |
| Seams | DECISION | [The seams specification](seams-and-scheduling.md), draft (2026-11-21): 4, 5.3, 5.4 point 2, 5.6 points 6 and 7, 5.7 point 4, 5.8, section 13 Q3 | 5.8, 5.9, 5.12 point 7 |
| Callbacks | DECISION | The callback service specification, draft, issue #66 (pull request #119): section 4's terms "Fence tuple", "Fence table" and "Ticket"; 5.6, points 1 to 3 and row P1 of point 9 | 4, 5.9 points 2, 3 and 7, 13 Q9 |
| Rx | DECISION | The Rx specification, issue #55: 5.12, point 5 (the pause generation) | 5.9 point 3 |
| Incarnation | DECISION | The volume incarnation specification, draft, issue #78 (pull request #125): 5.6, points 1 and 3 (the local volume epoch and its copy in the fence table) | 5.9 point 7, 13 Q9 |
| IPC | DECISION | The local IPC specification, draft, issue #62: its request cap and its numbering of contract errors | 2, 5.2 |
| E60 | DOCUMENTATION | errno(3), Linux man-pages 6.19, page dated 2026-05-28; register row E60 | 5.12 |
| E61 | DOCUMENTATION | fuse(4), Linux man-pages 6.19, page dated 2026-02-08; register row E61: DESCRIPTION, the request and reply headers, FUSE_INTERRUPT | 5.3, 5.6, 5.8 |
| E62 | DOCUMENTATION | The Linux kernel documentation, "FUSE", page for 7.4.0-rc3; register row E62: "Interrupting filesystem operations" | 5.8 |
| E63 | DOCUMENTATION | macOS 27.2 SDK headers: `sys/errno.h`, `sys/syslimits.h`, FSKit's `FSKitFunctions.h`, `FSVolume.h`, `FSTask.h`; register row E63 | 5.2, 5.8, 5.12 |
| E64 | SPECIFICATION | [MS-ERREF] 2.3, "NTSTATUS" (page updated 2024-10-30), and 2.3.1, "NTSTATUS Values" (page updated 2024-11-20); register row E64 | 5.12 |
| E65 | SPECIFICATION | [MS-SMB2] 2.2.30, "SMB2 CANCEL Request", page updated 2026-03-06; register row E65 | 5.8 |
| OWN | OWN | IronAFS's own design: the caps, the table's size, the request deadline, the claim, the submission step, the drain, the fence retry count, the acknowledgement rules, the Mint check, the error table's OWN rows | as labelled |

No other implementation's source was read for this page. No AFS or Rx
document is cited: the contract faces the operating systems, not the
servers.

### 12.1 Observed behaviour

None yet. The FUSE kernel's answer to a notification for an uncached
entry (5.10, point 4) is to be observed with the FUSE codec (issue #92).

## 13. Open questions

- **Q1. The clock of a request's deadline** (the seams specification's
  Q3). **Answer: the boot clock,** counted from arrival. A request issued
  before a night's sleep then ends at resume, rather than holding an
  application for the rest of its deadline in awake time; every
  server-granted deadline the request depends on (callbacks, tokens) is on
  the boot clock already and has expired too (C37, point 1); and an `Rpc`
  call on a boot deadline still retransmits at awake cadence (the seams
  specification, 5.4, point 3). It needs two Working amendments, by their
  own issue: C37, point 1, adds "the deadline of every request from the
  operating system (C39, point 3)" to the boot clock's list; O45, point 3,
  becomes "every OS request completes within its deadline, or, where a
  host suspend crosses the deadline, at the first instant the host runs
  after it" (the request then completes with the deadline's error of
  5.8, point 4). **Settled for this page:** both amendments are to be
  folded into the ADR 0004 Working amendments of issue #53. Blocks:
  freezing 5.8, points 2 and 10, until #53 lands; nothing else.
- **Q2. The request deadline's value.** 60 seconds by default, 1 to 3,600
  configurable (5.8, point 3). **Recommendation:** keep it as proposed and
  measure at the M2a acceptance runs against the test cell; the
  configuration key is the configuration specification's (issue #93).
- **Q3. The crate and the scripted core follow this page.** `ironafs-vfs`
  has no `PurgeFailed` value (5.10, point 5), no `VfsError` values of 5.12,
  point 9, and documents `Revision` as a slot's counter rather than a
  binding's (5.10, point 1). The scripted core (issue #98) refuses another
  binding's capability with `Access` where 5.3, point 3, says `Stale`; its
  `purge_failed` takes no revision and checks only that the node is held
  (5.10, points 5 and 6); its acknowledgement accepts any revision issued
  rather than delivered, and treats every unknown one as an error rather
  than splitting repeats from faults (5.10, point 6); and it has no
  overflow failure report. **Recommendation:** bring both in line in
  issue #67's and issue #98's pull requests, before this page is
  reviewed again.
- **Q4. Where the Mint check lands.** **Recommendation:** in `check-deps`
  (issue #46's checks), with its canary, filed as a follow-up of #67;
  review holds the rule until then.
- **Q5. The seven added error values** (5.12, point 9). C46, point 1,
  introduces its list with "include", so this page reads it as open and
  adds `NotADirectory`, `IsADirectory`, `NotEmpty`, `NoAttribute`,
  `TooSmall`, `PastEnd` and `Protocol`. If the review reads the list as
  closed, adding them is a Working amendment of C46, point 1.
  **Recommendation:** name them in issue #53's amendments with Q1's, so
  the ADR's list and the crate's agree, and have the local IPC
  specification (issue #62) number them.
- **Q6. Platform limits to register.** Linux's `NAME_MAX` (255) and its
  extended-attribute value limit, and whether every Windows name of 255
  UTF-16 units fits 255 bytes after L25's mapping, are UNVERIFIED here.
  **Recommendation:** register the Linux limits with the FUSE codec (issue
  #92); settle the Windows question with L24's table, where a longer cap
  for Windows names would be a change of this page.
- **Q7. Handle operations to add to C7, point 3.**
  N46, point 2, requires the core to answer `seek`, and it acts on an open
  file. C13, point 2 (Firm), checks every handle operation of C7, point 3,
  including subscription. This page applies that check to every handle
  operation (5.5, point 5).
  **Settled for this page:** adding "seek to data or hole" and the
  reserved handle operations to C7, point 3 (Working) is to be folded
  into issue #53's amendments with Q1's.
- **Q8. Mapping rows chosen here.** Every non-ADR row of 5.12, point 2's
  table is [OWN]. **Recommendation:** the FUSE codec's review (issue #92)
  re-checks the Linux column against each operation's documented errors;
  S2 fixes the Windows column.
- **Q9. Who pushes a fence-epoch advance out of process** (5.9, point 7;
  C16, point 6). The callback specification records suspect-epoch
  advances and object and volume breaks in the core, and counts its
  `Pause` and `Resume` events for diagnostics only (its 5.6, point 9,
  row P1); the incarnation specification writes local volume epoch
  changes; Rx's driver raises the pause generation with no lock; the
  resume generation is raised by the clock comparison that every fence
  check and every recording makes first (its 5.6, point 2), so a resume
  with neither behind it raises nothing; and the fence table evicts
  records, or becomes exhausted, inside a recording or a write of an
  epoch copy, with no event of its own. Open: which task turns
  each kind of change into an advance and pushes it (the callback
  service's task, the notifier, or the housekeeping loop running the
  clock comparison after a resume); how the daemon finds the bindings
  holding data from a server key, or whether it advances every
  out-of-process binding; and how advances are coalesced so that a
  break storm stays within the stream's bounds (5.10, point 9). The
  push must follow the record (C16, point 1) and must never let a reply
  produced before the change be written after the binding applied the
  advance. **Recommendation:** settle it with S1 (the adapter endpoint),
  before the first out-of-process binding is built. Blocks: every
  out-of-process binding; nothing in the daemon's process.

## 14. Review

The review record is [2026-11-21-m2a-67-contract](../reviews/2026-11-21-m2a-67-contract.md).
