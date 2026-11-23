<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Local IPC framework

Status: draft (2026-11-21). Raised in issue #62.

Kind: format. Class: S. The local IPC messages are bytes IronAFS defines
and sends; any local user can send them to the daemon, and the token path
and the caller check rest on this framing ([README](README.md), "Review
class per kind"). It becomes class R when the framing, handshake,
envelope and numbering rules are frozen together with the control 1.0
message set (section 13, Q1).

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (X1, P1, D1
and so on) are in section 12.

This is the first of three parts. It holds the framing, the handshake,
the envelope, numbering, versions, the definition set and its digest,
the error codes and the order of checks on a frame. What a connection
and a request are charged, and how work is scheduled, is in
[local-ipc-resources.md](local-ipc-resources.md); the Windows named
pipe is in [local-ipc-windows.md](local-ipc-windows.md).

Section and point numbers are shared by the three parts: a number names
one place, and a reference such as "5.2.1, rule 4" or "5.4, point 2"
means the same place whichever part it is written in. Where each place
is:

| Place | Part |
|---|---|
| 1 to 4; 5.1; 5.2, points 1 to 7 and 11; 5.3; 5.4, points 1 and 3 to 18; 5.5, points 1 to 6 and 8; 5.6.1; 5.6.2 (its table's rows for all systems, points 2 and 4); 5.6.3; 7; 10.1; 10.3; 10.4; 12 | [local-ipc.md](local-ipc.md) |
| 5.2, points 8 and 9; 5.2.1; 5.4, points 2, 19 and 20; 5.5, point 7; 5.6.4 | [local-ipc-resources.md](local-ipc-resources.md) |
| 5.2, point 10; 5.4, point 1's Windows rule; 5.5, point 7's Windows rows and its ownership rule; 5.6.2's Windows rows and points 1 and 3; the Windows pre-attestation residual of section 8; the Windows gate of 10.5 | [local-ipc-windows.md](local-ipc-windows.md) |
| 6, 8, 9, 10.2, 10.5, 11, 13: rows and items by subject | each part holds its own |

## 1. What this implements

ADR 0004, overview, constraint 4 (a constraint of the whole ADR):

> **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
> local IPC, configuration, requests from the operating system, and SMB
> messages before authentication. No panics, checked arithmetic and byte
> limits apply to all of it (coding standards rules 1 to 3).

ADR 0004, C44, points 1 to 6 (**Firm**):

> 1. There is one IPC framework: one framing, one handshake and one set of
>    message definitions in XDR
>    ([RFC 4506](https://www.rfc-editor.org/rfc/rfc4506)). `xtask`
>    generates the Rust and Swift codecs from those definitions. There is
>    one set of fuzz targets.
> 2. The handshake carries a version range, the role (C45) and the daemon's
>    instance id.
> 3. IPC is untrusted input in both directions (overview, constraint 4), and
>    a long-lived format under the `wire-format` skill.
> 4. **Compatibility, control endpoint.** Within a major version, only new
>    messages and optional fields are added. The daemon accepts the current
>    and the previous minor version, so an older `afs` keeps working during
>    an upgrade.
> 5. **Compatibility, adapter endpoint.** Versions must match exactly,
>    because adapters ship with the daemon.
> 6. What an IPC message says about a principal and a negotiated suite is
>    in N36, N39 and N41. The message sets themselves are in the specification for
>    IPC, written before the code.

ADR 0004, C45, points 1 to 3 (**Firm**), the sentences this framework
serves:

> 1. **The control endpoint** is open to any local user. The caller is the
>    peer the operating system reports. […]
> 2. **The adapter endpoint** is open only to adapter processes the daemon
>    itself registered. […]
> 3. A binding is made only from a **daemon-owned registration.** […] A
>    binding without one is refused. […]

ADR 0004, C43, points 1 and 2 (**Firm**):

> 1. No credential material is read or sent until the sender has shown that
>    the endpoint belongs to the daemon.
> 2. The check has two parts on every operating system: the endpoint's
>    owner is the daemon's service identity, as the operating system
>    reports it (point 4), and the handshake's instance id (C44) is bound
>    to that checked connection. A failed check aborts before anything
>    secret moves.

ADR 0004, N41, points 1 to 9 (**Firm**):

> 1. From the first release, these structures carry a version and a
>    bounded, tagged, length-delimited extension list: attributes,
>    set-attribute requests, the ACL and its entries, status, volume
>    information and `statfs` results, and events.
> 2. Each extension is marked **critical** or not.
> 3. An unknown non-critical extension in a reply or event is carried and
>    shown as unknown.
> 4. **A request with an unknown critical extension is refused whole**
>    (not supported), never applied in part.
> 5. A sender never puts a critical extension in a reply or event to a peer
>    whose handshake version predates it.
> 6. If one arrives anyway, the receiver treats the whole reply as failed
>    ("this daemon is newer; upgrade `afs`") and the whole event as an
>    unknown event. It never acts on the known part.
> 7. The adapter contract (C7, C44) uses the same shapes under its
>    exact-version rule: it needs room to grow, not compatibility.
> 8. IPC passes an unknown error cause through as its code plus its text.
> 9. A non-critical extension in a request may not change what the request
>    mutates. An unknown one is ignored, and the reply names it as ignored.

ADR 0004, C5, point 5 (**Working**), the part that applies, as accepted:

> `ironafs-os` holds the few calls that have no safe route: […] peer
> credentials (C43), […]

[ADR 0005](../adr/0005-ironafs-os.md), Consequences, amends that list
(issue #49). The amended text adds, among others: "and, on Windows, the
control endpoint's named pipe: its creation, its I/O and its caller's
token (C45, point 1)". ADR 0005's sheets 13 and 16 to 20 are those
sites. Sheets 17 to 19 refer to the control IPC design for the pipe's
instance count and caps ("its 5.2"), the prefix rule ("control IPC 5.4,
step 1") and the named zeroisation places. The control IPC design is two
specifications, this framework and the control endpoint specification,
and those references resolve to these places of the framework (the
table above says which part holds each): 5.2, point 10 (the instance
count); 5.4, point 1 (the prefix rule); and 5.5, point 7 (the
zeroisation places).

ADR 0004, C16, point 6 (**Firm**), the sentence quoted in
local-ipc-resources.md, section 1. This part's share is what a
catalogue declares: an event row's delivery class and a mandatory
row's pending bound (5.3.3, point 2; 5.6.1, point 3):

> - **Back-pressure:** the stream is bounded (C39, point 3). When it is
>   full, replies wait within their deadlines. An epoch advance is never
>   dropped: space for it is kept free.

Section 9 shows where each is kept and which test fails if it is broken.

## 2. Scope

1. **Covered:** everything a byte on either local IPC endpoint must obey
   before the meaning of an operation is known:
   - the stream transports and how a frame is cut from them (5.1.1,
     5.1.2);
   - the handshake, its version choice per endpoint, the daemon's
     instance id and the schema digest (5.1.3, 5.3);
   - the message envelope, the reply wrapper, the common error and the
     extension list (5.1.4 to 5.1.7);
   - how message numbers, extension tags and error codes are assigned
     (5.3.3);
   - every framework field bound and frame cap, as wire constants, and
     the server budgets, as policy under hard ceilings (5.2);
   - on Windows, the finite pipe pool and the one read made before the
     caller is attested (5.2, point 10; 5.4, point 1);
   - the order of checks on every frame, both directions (5.4);
   - what every byte and every unit of work is charged to, per caller
     and in all (5.2.1), and how ingress hands requests to the bounded
     executor (5.6.4);
   - the one XDR definition set, its generator, the schema digest, and
     the one fuzz suite (5.6.1, 10.4);
   - the connection states and ordering rules (5.6.2, 5.6.3).
2. **Not covered, and where it is covered:**
   - the control endpoint's address, permissions, pipe security
     descriptor, how its caller and the daemon are identified on each
     operating system, its operations, their bounds and who may call
     them, and the control 1.0 message set: the control endpoint
     specification, `docs/specs/control-endpoint.md` (issue #70);
   - the adapter endpoint's transport, registration, binding, message
     set, credit and revocation order: the adapter endpoint
     specification (ADR 0004, S1), not yet written;
   - the encoding rules of each XDR type, caps, the memory budget and the
     codec's error set: the [XDR profile](xdr.md), which every IPC
     definition obeys;
   - the notation of IronAFS's definition files: the generator's
     specification (xdr.md, section 13, Q4). This document states the
     facts each IPC definition must declare (5.6.1, point 4);
   - the operating-system calls and their safety obligations: ADR 0005,
     sheets 13 and 16 to 20;
   - how a descriptor or handle travels beside a frame (ADR 0004, L35,
     point 2): not in this version (section 13, Q2).

## 3. The design in short

1. One stream connection per client: a Unix-domain stream socket on
   Linux and macOS, a byte-mode named pipe on Windows.
2. The daemon learns who called from the operating system, never from a
   message. On Linux and macOS it knows before reading any byte. On
   Windows it reads exactly the 4-byte length prefix of the first frame
   first, because Windows attests a pipe client only after data has been
   read; nothing is decoded or answered until the caller is attested.
   Every connection is admitted against the total and the handshake
   limits before any byte is read, on every operating system.
3. A frame is a 4-byte big-endian length followed by one XDR message.
   Every frame cap is a wire constant.
4. The first frame each way is the handshake. `Hello` names the role, a
   version range and the client's schema digest. `HelloReply` names the
   chosen version, the daemon's range, its instance id and its schema
   digest. The handshake's layout never changes.
5. The control endpoint accepts the daemon's current and previous minor
   version; the adapter endpoint accepts exactly one version and one
   schema digest.
6. Every later frame starts with a 16-byte envelope: kind, message
   number and request id. A request gets exactly one reply, perhaps after
   a bounded stream of parts; events and notices are one-way.
7. Bodies that grow carry a version and a bounded extension list. An
   unknown critical extension refuses a request whole.
8. All IPC messages are defined once, in one XDR definition set. `xtask`
   generates the codecs with IronAFS's own parser; the generated code is
   checked in. The schema digest is SHA-256 over the set's canonical,
   LF-normalised bytes.
9. Every byte the daemon holds for a connection is reserved, before it
   is allocated, from a global budget and from its caller's own budget,
   and every admitted request has its terminal reply paid for before its
   handler runs.
10. Readers never run handlers. They serve callers in turns, one frame a
    turn, and hand each admitted request to a bounded executor without
    waiting; a full executor is a retryable `Busy`. Every request has a
    deadline counted from its arrival.
11. A secret leaves a client only through one operation of the checked
    connection it was built for, and a daemon sends one only on a
    connection its endpoint has admitted for it.

## 4. Terms

- **Endpoint:** the socket or pipe one role connects to. There are two,
  control and adapter (ADR 0004, C45).
- **Client:** the process that connects (`afs`, a login service, a
  session agent, an adapter). **Daemon:** `ironafsd`, which listens.
- **Caller:** the peer identity the operating system attests for a
  connection, read by the endpoint's transport (5.4, point 1). Fixed for
  the connection's life.
- **Caller key:** the value per-caller budgets are counted under. The
  endpoint specification defines it from the attested caller (for the
  control endpoint, the uid on Linux and macOS and the logon session on
  Windows, ADR 0004, C41).
- **Instance id:** 16 bytes the daemon draws from the production random
  source (ADR 0004, C5, point 1) once at each start, before any endpoint
  accepts a connection. It names one run of the daemon.
- **Frame:** a 4-byte length and the bytes it counts. **Message:** the
  bytes a frame's length counts.
- **Envelope:** the first 16 bytes of every message after the handshake.
- **Body:** the XDR value after the envelope.
- **Catalogue:** an endpoint's table of message numbers (5.3.3).
- **Checked connection:** a client connection on which the endpoint's
  daemon check has passed and a valid `HelloReply` has been read on the
  same handle (5.4, point 16).
- **Arrival:** the time, on the awake clock of ADR 0004, C37, point 1,
  at which a frame's 4-byte length is complete (5.6.4). Every deadline
  of this document runs on that clock, so a suspended machine expires
  nothing.
- **Turn:** one bounded step of an ingress thread for one connection
  (5.6.4).
- **Retryable:** said of a refusal that changed nothing and that the
  same request, sent again later with a new request id, may pass.
- **Definition set:** the XDR source files from which every IPC codec is
  generated (5.6.1).
- **Wire constant:** a bound fixed by this or an endpoint specification,
  compiled in, and changed only by a format change (5.2).
- **Policy budget:** a server limit read from validated configuration
  under a hard ceiling fixed in code (5.2).

## 5. Specification

### 5.1 Layout

All encoding follows the [XDR profile](xdr.md): big-endian, four-byte
units, zero padding, and only the profile's types `[D3 §5.1: Firm]`. No
IPC definition uses optional data, floating point or a recursive type,
which the profile leaves out (xdr.md 5.1, points 3 and 4). A value that
may be absent is a union on a `bool`, with a `void` arm for FALSE `[OWN]`:
the profile has no other form.

#### 5.1.1 Transports

1. **Linux and macOS:** a stream socket in the Unix-domain family, bound
   to a pathname `[OWN]`. Stream sockets carry the peer's credentials as
   they were at `connect` or `listen` on Linux `[P4 SO_PEERCRED:
   CONFIRMED]` and on macOS `[P5 LOCAL_PEERCRED: CONFIRMED]`; the macOS
   page offers that option only on stream sockets and lists only stream
   and datagram types for the family `[P5: CONFIRMED]`, so no packet type
   is used.
2. **Windows, the control endpoint:** a named pipe in byte mode, read in
   byte mode, rejecting remote clients, used with overlapped I/O (ADR
   0005, sheets 17 and 18) `[D2 sheet 17]`. A byte-mode pipe does not
   separate one write from another `[P1 dwPipeMode: CONFIRMED]`, so
   framing is this document's, as on Unix. ADR 0005's sheets decide this
   transport for the control endpoint only. Another endpoint on Windows
   names its transport in its own specification; nothing in
   local-ipc-windows.md applies to it until that specification says so.
3. Each endpoint specification names its own address, the permissions
   of that address, and the operating-system checks of the caller and of
   the daemon. This framework requires of every transport only: an
   ordered, reliable byte stream; a caller the operating system attests
   for the whole connection; and, for the daemon check of C43, an owner
   the client can read through the operating system `[ADR 0004 C43.2,
   C45.1: Firm]`.

#### 5.1.2 Frame

| Offset | Field | Type | Rule |
|---|---|---|---|
| 0 | `length` | unsigned 32-bit, big-endian | the byte count of `message`; a multiple of 4; never 0; at most the cap of the frame class (5.2) |
| 4 | `message` | `length` bytes | exactly one message |

1. One frame carries one message. There is no fragment bit and no
   continuation: a message larger than its frame cap cannot be sent
   `[OWN]`, which keeps the reader's buffer to one frame.
2. No checksum or MAC is computed over a frame `[OWN]`. The bytes cross a
   local kernel object from a peer the operating system attested; a
   checksum would be computed by that same peer and would detect nothing
   the stream does not already guarantee. Coding standards rule 9's
   integrity rule applies to formats that carry an integrity value, and
   this one carries none. Section 8 states what this does not cover.

#### 5.1.3 Handshake

The handshake is the first frame in each direction. Its layout is the
same for every endpoint, every role and every version, and never changes
`[OWN]`: a client and a daemon of any release must be able to read each
other's handshake to say "upgrade". A change to it is a new magic value,
which is a different protocol.

```
const IPC_MAGIC = 0x49414653;      /* the ASCII bytes "IAFS" */

struct Version {
    unsigned int major;
    unsigned int minor;
};

struct Hello {                     /* client to daemon, first frame */
    unsigned int magic;            /* IPC_MAGIC */
    unsigned int role;             /* 1 control, 2 adapter (5.3.3) */
    Version      min;              /* lowest version the client speaks */
    Version      max;              /* highest version the client speaks */
    opaque       schema_digest[32];/* the client's built digest (5.6.1) */
    opaque       client_name<32>;  /* for diagnostics only, never trusted */
};

enum HelloResult {                 /* open (5.4, point 18) */
    ACCEPTED        = 0,
    VERSION_MISMATCH = 1,
    ROLE_REFUSED    = 2,
    BUSY            = 3,
    SCHEMA_MISMATCH = 4
};

struct HelloReply {                /* daemon to client, first frame */
    unsigned int magic;            /* IPC_MAGIC */
    HelloResult  result;
    Version      chosen;           /* meaningful only when ACCEPTED */
    Version      daemon_min;       /* always filled */
    Version      daemon_max;       /* always filled */
    opaque       instance_id[16];
    opaque       schema_digest[32];/* the daemon's built digest */
};
```

1. `Hello` encodes to 60 to 92 bytes and `HelloReply` to exactly 80
   (5.2, point 3) `[OWN]`, computed from 5.1's sizes in the XDR profile.
2. `role` is a plain unsigned integer, not an enumeration, so that a
   daemon can answer `ROLE_REFUSED` to any value instead of failing to
   decode `[OWN]`.
3. `daemon_min` and `daemon_max` are filled in every reply, so a refused
   client can say which version to install `[OWN]`. `daemon_max` is the
   daemon's version on this endpoint and `daemon_min` the lowest version
   it accepts there (5.3.2, points 3 and 5); a client checks their shape
   (5.4, point 16).
4. `instance_id` is sent in every reply. It is not a secret: every
   connecting caller is told it. It is what a client binds to its
   checked connection (ADR 0004, C43, point 2) and what an agent compares
   after a reconnect (ADR 0004, C42, point 3) `[ADR 0004 C44.2: Firm]`.
   It names a run of the daemon and proves nothing about who sent it:
   the operating system's check is the only evidence that the peer is
   the daemon (5.4, point 16) `[OWN]`.
5. `client_name` is a label the client chooses (`afs`, `aklog`). The
   daemon shows it in diagnostics, escaped, and never decides anything
   on it `[OWN]`.
6. There is no extension list in the handshake `[OWN]`. Everything that
   could grow is negotiated after it, by version.

#### 5.1.4 Envelope

Every message after the handshake begins with the envelope:

```
enum Kind {                        /* closed */
    REQUEST = 1,                   /* client to daemon; one REPLY follows */
    REPLY   = 2,                   /* daemon to client */
    PART    = 3,                   /* daemon to client, before a REPLY */
    EVENT   = 4,                   /* daemon to client, one-way */
    CANCEL  = 5,                   /* client to daemon */
    NOTICE  = 6                    /* client to daemon, one-way */
};

struct Envelope {
    Kind           kind;
    unsigned int   number;         /* message number (5.3.3) */
    unsigned hyper request_id;
};
/* followed by the body the (kind, number) pair selects */
```

1. The envelope is 16 bytes, always `[OWN]`.
2. **By kind** `[OWN]`:

   | Kind | Sender | `number` | `request_id` | Body |
   |---|---|---|---|---|
   | `REQUEST` | client | a catalogue row of kind request (never 0) | not 0, and greater than that of every earlier `REQUEST` on the connection | the row's request body |
   | `REPLY` | daemon | the request's number | the request's | `ReplyBody` (5.1.5) |
   | `REPLY`, closing notice | daemon | 0 | 0 | `ErrorReplyBody` (5.1.5); the last frame the daemon sends (5.4, point 13) |
   | `PART` | daemon | the request's number, a row marked streaming | the request's | the row's part body |
   | `EVENT` | daemon | a catalogue row of kind event | 0 | the row's event body |
   | `CANCEL` | client | 0 | the request to stop | none: the message is exactly 16 bytes |
   | `NOTICE` | client | a catalogue row of kind notice | 0 | the row's notice body |

3. Only a `REQUEST` that passes 5.4, point 8 raises the connection's
   request-id mark, whatever later happens to it. A `CANCEL` never moves
   it, so a stray `CANCEL` cannot touch a later request `[OWN]`.
4. An endpoint uses the kinds its catalogue has rows for. The control
   endpoint's rows are in the control endpoint specification; the adapter
   endpoint maps its daemon-to-adapter stream onto `EVENT` and its
   acknowledgements onto `NOTICE`, in its own specification `[OWN]`, so
   that one envelope serves both (ADR 0004, C44, point 1).
5. **Numbers by kind, for a sender** `[OWN]`. The table above is the
   whole rule; no other number is ever sent:
   - `REQUEST` and `NOTICE` (client), `PART` and `EVENT` (daemon): a row
     of that kind that the chosen version has;
   - `REPLY` with `RESULT_OK`: the row of the request it answers;
   - `REPLY` with `RESULT_ERROR`: the number of the request it answers,
     copied from that request's envelope whether or not any row has it.
     This is the schema-independent path of 5.1.5, point 1, and its body
     is always an `ErrorReplyBody`;
   - the closing notice and `CANCEL`: 0, the one use of that number.

#### 5.1.5 Reply body

```
enum ResultKind { RESULT_OK = 0, RESULT_ERROR = 1 };    /* closed */

struct IgnoredExtensions {
    unsigned int tags<64>;         /* distinct, ascending */
};

/* generated once per request row X: */
union XResult switch (ResultKind kind) {
    case RESULT_OK:    XReply   ok;
    case RESULT_ERROR: IpcError err;
};
struct XReplyBody {
    IgnoredExtensions ignored;
    XResult           result;
};

/* the common error reply, for any number, known or not: */
struct ErrorReplyBody {
    IgnoredExtensions ignored;
    ResultKind        kind;        /* always RESULT_ERROR */
    IpcError          err;
};
```

1. Every row's error arm is `IpcError`, so every `XReplyBody` with
   `RESULT_ERROR` encodes to exactly the bytes of an `ErrorReplyBody`
   with the same fields `[OWN]`. The daemon builds every error reply,
   `UnknownMessage` for a number with no row included, with one common
   encoder; the client decodes every error reply with one common decoder,
   without the row's schema. The generator refuses a row whose error arm
   is anything else (5.6.1, point 3).
2. `ignored` names each distinct unknown non-critical extension tag the
   daemon found anywhere in the request, once, ascending, on
   `RESULT_OK` and `RESULT_ERROR` alike `[ADR 0004 N41.9: Firm]`. A
   message holds at most 64 extensions (5.2, point 5), so the list always
   has room for all of them. It is empty only when there were none or the
   request was refused before its extensions were walked (5.4, point 12).
   The tags are protocol metadata this format defines, copied from the
   request as numbers; they are the one part of a request, with the
   envelope's `number` and `request_id`, that a reply repeats (section 7,
   point 2).
3. A `PART`'s body is the row's part type alone.

#### 5.1.6 Error

```
struct IpcError {
    unsigned int  version;         /* 1 (5.1.7, point 2) */
    unsigned int  code;            /* open (5.3.3, point 5) */
    unsigned int  contract;        /* open; 0 = none (5.3.3, point 6) */
    string        text<512>;       /* for people; never parsed */
    ExtensionList ext;
};
```

1. `code` says what failed; `contract` carries the contract error of ADR
   0004, C46 that the handler returned, when `code` is `Contract`
   `[OWN]`.
2. `text` never holds key material, another caller's names, or paths
   outside the caller's projection; the endpoint specification says what
   each of its errors may name `[OWN]`, after ADR 0004, O16.
3. A receiver that does not know a `code` or a `contract` value shows it
   as its number plus `text` `[ADR 0004 N41.8: Firm]`.

#### 5.1.7 Extensions and body versions

```
struct Extension {
    unsigned int tag;              /* assigned once (5.3.3) */
    bool         critical;
    opaque       value<1024>;
};
typedef Extension ExtensionList<16>;
```

1. **Which types carry them.** Every structure that N41, point 1 names,
   and every body type (request, reply-ok, part, event and notice bodies,
   and `IpcError`), begins with `unsigned int version` and ends with
   `ExtensionList ext` `[ADR 0004 N41.1: Firm]`; extending it to every
   body is `[OWN]`, so that any message can gain an optional field in a
   later minor version (ADR 0004, C44, point 4). The generator refuses a
   body type without both (5.6.1, point 3). The handshake types carry
   neither (5.1.3, point 6).
2. **The `version` field.** Within a major version a type's layout never
   changes (5.3.2, point 2); growth is by new numbers and extensions. So
   `version` is the type's layout version, 1 in every major version this
   framework knows, and the only value accepted `[OWN]`. A decoder that
   reads another value refuses the message as `BadBodyVersion` (section
   7). A new major version is a new section of the endpoint
   specification, which states its own value.
3. **Order and uniqueness.** Within one list, tags are strictly
   ascending: no duplicate, no disorder `[OWN]`, so that each list has
   one encoding (xdr.md 5.5, point 1).
4. **Registration.** Each tag is registered in the definition set for
   the types it may appear in, with its critical bit and the version that
   introduced it (5.6.1, point 4). A non-critical tag registered on a
   request type is registered as **advisory**: its handler may use it to
   shape the reply, never to change which object is changed, to what
   value, or whether the change applies `[ADR 0004 N41.9: Firm]`. The
   generator refuses a non-critical request tag not registered as
   advisory; a tag that changes what is mutated is critical.
5. **Recognition.** A tag is known to a receiver on a connection only
   when it is registered for the type it arrives in and its introducing
   version is not later than the connection's chosen version `[OWN]`.
   Any other tag is unknown there and is handled by the critical bit it
   carries (5.4, points 12 and 18). A known tag whose critical bit
   differs from its registration is refused as `ExtensionFlagMismatch`
   before any handler runs.
6. **Aggregate bound.** One message holds at most 64 extensions across
   all its lists, nested lists included, and at most 16,384 bytes of
   extension values in all (5.2, point 5) `[OWN]`. Without it, a 64 KiB
   request could carry thousands of distinct unknown tags, more than any
   reply could name, and a reply type with many nested lists would have
   no useful size bound (5.6.1, point 3).
7. **Open enumerations.** A field the definition set marks open decodes
   an undeclared value to an "unknown (n)" form that keeps n, and the
   encoder writes it back unchanged; the encoder refuses the unknown form
   for a declared n, which would be a second encoding of one value
   `[D3 §5.4.9]`. Every other enumeration is closed. Which fields are open
   is stated per field; in this framework, `HelloResult`, `IpcError.code`
   and `IpcError.contract` are open `[OWN]`, because later releases add
   values a peer must be able to show.
8. **A registered value is typed** `[OWN]`. Each tag's registration
   names the XDR type of its `value`, a type of the definition set with
   its own cap table. The bytes of `value` are one XDR message of that
   type, with a cap of at most 1,024 bytes.
   - A receiver that knows the tag (point 5) decodes the value with the
     type's generated decoder, whole and with no trailing bytes, during
     the walk of 5.4, point 12, so before any handler or consumer runs.
     A value that does not decode is `Malformed`, fatal. An unknown tag's
     value is never decoded: it is carried as bytes.
   - A value type holds no `ExtensionList`, directly or in any component,
     and no secret field; the generator refuses either (5.6.1, point 3).
     So extensions do not nest, and the lists that 5.1.7, point 6 counts
     are those of the body's own structures.
   - The memory a message's known values may request when decoded is
     part of the message's memory budget: the generator adds, to each
     body type's budget, 16,384 bytes times the largest heap ratio
     (xdr.md 5.2, point 9) of the value types registered for the types
     in that body (5.2, point 7).

### 5.2 Caps

1. **Two kinds of limit** `[OWN]`, as issue #62 records the owner's
   decision of 2026-11-20:
   - **wire constants:** the frame caps and every XDR field bound a
     decoder enforces, extension bounds included. A peer may rely on
     them. They are compiled in, never read from configuration, and
     change only with the format (class R once frozen);
   - **policy budgets:** connections, concurrency, queues, streaming and
     timers. The daemon reads each from validated configuration, under a
     hard ceiling fixed in code. A client never relies on their values.
2. **Frame caps** (wire constants). The cap bounds `length`, the bytes
   after the 4-byte prefix; a buffer holds the framed size, the cap plus
   4 `[OWN]`.

   | Cap name | Frame class | Control endpoint | Adapter endpoint | On excess |
   |---|---|---|---|---|
   | `ipc.frame.hello` | `Hello` | 92 | 92 | close, no reply |
   | `ipc.frame.hello_reply` | `HelloReply` | 80 | 80 | the client closes |
   | `ipc.frame.request` | `REQUEST`, `NOTICE` | 65,536 | the adapter endpoint specification's | `FrameTooLarge`, fatal |
   | `ipc.frame.cancel` | `CANCEL` | exactly 16 | exactly 16 | `MalformedEnvelope`, fatal |
   | `ipc.frame.reply` | `REPLY`, `PART`, `EVENT` | 1,048,576 | the adapter endpoint specification's | the client closes |
   | `ipc.frame.ceiling` | any frame, any endpoint | 2,097,152 | 2,097,152 | the generator refuses a larger cap |

   Why these values `[OWN]`: the handshake caps are the handshake's exact
   maximum sizes (5.1.3, point 1), so nothing beyond the handshake can be
   read before it. A control request carries at most a token and a few
   names; 64 KiB holds them with room for extensions. A control reply
   may list every cell, server and context a status report names; 1 MiB
   is the largest a client is asked to buffer. The ceiling bounds what
   any endpoint can ask a reader to hold, and leaves room for an adapter
   frame that carries 1 MiB of file data with its header. The adapter
   endpoint specification sets its two caps, each at most the ceiling;
   until it does, the adapter endpoint is not enabled.
3. **Framework field bounds** (wire constants). In the format of xdr.md
   5.2, point 5 `[D3 §5.2.5]`. Sizes are encoded bytes; `n` is the
   variable part.

   | Cap name | Field | Kind | Declared bound | Cap | Direction | Memory budget | Secret | Why |
   |---|---|---|---|---|---|---|---|---|
   | `ipc.hello` | message | message | none | 92 | both | 92 | | 4 + 4 + 8 + 8 + 32 + 4 + 32; the only heap is `client_name`; the budget is r = 1 times the cap (point 7) |
   | `ipc.hello.client_name` | `client_name` | opaque | 32 bytes | 32 | both | | | enough for a program name; diagnostic only |
   | `ipc.hello_reply` | message | message | none | 80 | both | 0 | | fixed: 4 + 4 + 8 + 8 + 8 + 16 + 32; no heap |
   | `ipc.envelope` | message | message | none | 16 | both | 0 | | fixed |
   | `ipc.extension.value` | `Extension.value` | opaque | 1,024 bytes | 1,024 | both | | | one extension holds a small structure, never bulk data |
   | `ipc.extension_list` | `ExtensionList` | array | 16 elements | 16,576 | both | | | 16 × (4 + 4 + 4 + 1,024) |
   | `ipc.message.extensions` | all lists of one message | count | none | 64 extensions | both | | | point 5 |
   | `ipc.message.extension_bytes` | all extension values of one message | bytes | none | 16,384 | both | | | point 5 |
   | `ipc.ignored.tags` | `IgnoredExtensions.tags` | array | 64 elements | 256 | both | | | one tag per extension a message can hold |
   | `ipc.error.text` | `IpcError.text` | string | 512 bytes | 512 | both | | | one line for a person |
   | `ipc.error_reply_body` | `ErrorReplyBody` | message | none | 18,140 | both | 73,728 | | 260 + 4 + 532 + 17,344 (point 6); the budget is 4 × the cap (point 7) |

   `ExtensionList` and `IgnoredExtensions` are not messages on their own;
   they count against the message that holds them. Every other bound of
   an IPC message is in its endpoint specification's cap table, in the
   same format, each a wire constant; the generator refuses a variable
   item without one `[D3 §5.2.1]`.
4. **A body's message cap** is its frame class's cap less the 16-byte
   envelope `[OWN]`: the decoder decodes the envelope, then the body as
   one XDR message whose input is the rest of the frame (xdr.md 5.4,
   point 1).
5. **The aggregate extension bound** (5.1.7, point 6) is checked by the
   generated decoder as it goes: before it reserves an `ExtensionList`'s
   slots, the running extension count plus the list's count must be at
   most 64; before it copies an extension's value, the running value
   bytes plus the value's length must be at most 16,384. Either failure
   is `CapExceeded` naming `ipc.message.extensions` or
   `ipc.message.extension_bytes`, before any allocation for it `[OWN]`.
   The encoder refuses a message over either bound.
6. **Largest sizes the generator compares.** For each message type the
   generator computes its largest encoded size and compares it with the
   cap of the frame class it travels in (5.6.1, point 3) `[OWN]`. Every
   fixed and bounded item counts at its bound, padding included. Every
   `ExtensionList` counts as its 4-byte count word only, and the message
   then adds the **extension allowance** once: 64 extensions of 12 header
   bytes (tag, flag, value length), 16,384 value bytes, and at most 3
   padding bytes per value: 768 + 16,384 + 192 = 17,344 bytes. The
   decoder and encoder enforce the aggregate bound, so no accepted
   message passes the sum. For `ErrorReplyBody`: `ignored` 4 + 64 × 4 =
   260; the discriminant 4; `IpcError` 4 + 4 + 4 + (4 + 512) + 4 = 532;
   the allowance 17,344; in all 18,140, and with the envelope 18,156,
   framed 18,160.
7. **Memory budgets.** Each message's budget is set in its cap table and
   proved by the generator from the heap ratio of xdr.md 5.2, point 9
   `[D3 §5.2.9]`. For the framework's messages: `Hello` holds one opaque
   (heap ratio 1), so its budget is 1 × 92 = 92; `HelloReply` and the
   envelope request nothing. `ErrorReplyBody` holds arrays of
   extensions; with an `Extension` slot of at most 36 bytes and `s_min`
   12, the ratio is at most ⌈36 ÷ 12⌉ + 1 = 4, so its budget is 4 ×
   18,140 = 72,560, set at 73,728 (72 KiB) `[OWN]`. The generator's
   compile-time assertion is the check on each target; a layout that
   breaks the budget does not build there, and this table changes.
   No tag is registered for `IpcError` in this version, so its budget
   has no part for typed values (5.1.7, point 8); a body type with
   registered tags adds that part. For each row the generator also emits
   a **decode charge** for each of the row's bodies (request, reply,
   part, event or notice): the body's memory budget in bytes, and the
   largest number of heap requests one decode of it can make (xdr.md
   5.2, point 9.5), the typed values' included. The daemon uses the
   request and notice charges; a client uses the reply, part and event
   charges (5.4, point 19). 5.2.1,
   rule 2 reserves both before the body is decoded.
8. **Policy budgets:** [local-ipc-resources.md](local-ipc-resources.md), 5.2, point 8.
9. **Rules between budgets:** [local-ipc-resources.md](local-ipc-resources.md), 5.2, point 9.
10. **The Windows pipe pool:** [local-ipc-windows.md](local-ipc-windows.md), 5.2, point 10.
11. **Stream limits** (wire constants) `[OWN]`. A catalogue row of kind
    streaming request declares `parts_max`, the most `PART` frames one
    request may be answered with, and `stream_bytes_max`, the most bytes
    of those frames' messages in all. Both are wire constants of the
    row, in its cap table: the daemon never sends more, whatever its
    configured budgets (point 8), and a client enforces both on what it
    receives (5.4, point 19). The framework's ceilings, which the
    generator applies to every row:

    | Cap name | Bounds | Ceiling |
    |---|---|---|
    | `ipc.stream.parts.ceiling` | a row's `parts_max` | 65,536 |
    | `ipc.stream.bytes.ceiling` | a row's `stream_bytes_max` | 16,777,216 (16 MiB) |

    A result that can pass a row's limits is fetched by further
    requests, as its endpoint specification says; a stream is one
    bounded result, never an open-ended feed.

#### 5.2.1 Reservation before allocation

In [local-ipc-resources.md](local-ipc-resources.md), 5.2.1.

### 5.3 Versions

#### 5.3.1 Magic and the framework version

1. The magic is `0x49414653` `[OWN]`. A first frame from a client whose
   first four message bytes are not the magic is not answered: the
   connection is closed. A daemon reply with another magic is a failed
   handshake for the client.
2. The framing (5.1.2), the handshake (5.1.3), the envelope (5.1.4) and
   the numbering rules (5.3.3) have no version of their own: they are the
   same in every version of every endpoint (5.1.3) `[OWN]`. What has
   versions is each endpoint's message set.

#### 5.3.2 Endpoint versions

1. A version is `major.minor`, compared as the pair (major, minor) in
   that order. Each endpoint has its own version numbers, stated in its
   specification `[OWN]`.
2. **What a minor version may add,** on either endpoint: a new message
   number; a new extension tag; a new error code or contract value; a
   new value in a field this or an endpoint specification marks open.
   Nothing else: a minor version never changes or removes a field, a
   number, a bound or a meaning `[ADR 0004 C44.4: Firm]`, applied here
   to both endpoints because one definition set serves them.
3. **Control endpoint.** A daemon at control version `M.n` accepts
   exactly `M.n` and, when `n` ≥ 1, `M.(n − 1)` `[ADR 0004 C44.4: Firm]`.
   It chooses the highest accepted version within the client's range
   [`min`, `max`]. If there is none, it replies `VERSION_MISMATCH` and
   closes. A daemon at `M.0` accepts only `M.0`; no arithmetic is done on
   `n − 1` when `n` is 0 (section 6).
4. **Control clients.** A client at `M.k` offers `min` = `M.0` and `max` =
   `M.k` `[OWN]`: by point 2 it can speak every earlier minor of its
   major by sending only what that minor defines. After the handshake it
   sends nothing the chosen version does not define: no newer message
   number and no newer critical extension `[ADR 0004 N41.5: Firm]` (the
   rule binds the client as sender, too).
5. **Adapter endpoint.** The client's range is one version (`min` =
   `max`), equal to the daemon's adapter version, and `Hello`'s
   `schema_digest` equals the daemon's `[ADR 0004 C44.5: Firm]`. A range
   of more than one version, or another version, is `VERSION_MISMATCH`;
   the right version with another digest is `SCHEMA_MISMATCH`; either
   closes the connection. The version says what was meant; the digest
   says what was built `[OWN]`.
6. **The digest on the control endpoint** is informational `[OWN]`: the
   daemon compares nothing on it. `afs` shows both digests when they
   differ, as a diagnostic. Two builds of one version may differ in
   comments only, which change the digest (5.6.1, point 7).
7. **Newer daemon, older client.** The daemon sends only what the chosen
   version defines, and never a critical extension the chosen version
   predates `[ADR 0004 N41.5: Firm]`.
8. **A new major version** is a new section of its endpoint's
   specification. Whether a daemon serves two majors at once is that
   section's decision; this framework's rule is only the handshake's
   choice above `[OWN]`.

#### 5.3.3 Numbering

1. **Roles:** 1 control, 2 adapter. 0 is never a role. A new role is a
   new endpoint and needs an amendment of ADR 0004, C45 first `[OWN]`.
2. **Message numbers** are per endpoint: each endpoint has its own
   catalogue, and a number means nothing on the other endpoint `[OWN]`.
   - 0 is never a row; it marks `CANCEL` and the closing notice.
   - 1 to 0xFFFEFFFF are assigned by the endpoint specification.
   - 0xFFFF0000 to 0xFFFFFFFF are test numbers (point 8).
   - A catalogue row states: its number, its name, its kind (request,
     streaming request, event or notice), the version that introduced
     it, its body types, and its cap table. A request row may be marked
     a **binding row** (5.4, point 6) and may set a request deadline
     below the configured one; a streaming row states `parts_max` and
     `stream_bytes_max` (5.2, point 11); a notice row states its
     overload rule, `drop` or `fatal` (5.2.1, rule 2); an event row
     states its delivery class, droppable or mandatory, and a mandatory
     one its **pending bound**: the most events of that row one
     connection can have pending at once, over all its keys together
     and counting one being written, as a constant (5.2.1, rule 4); and
     each body
     states whether it is secret-bearing and, for a daemon-to-client
     body, who may receive it (5.5, point 6). The endpoint specification
     adds who may send it.
3. **Extension tags** form one registry over the whole definition set,
   so one tag number means one extension everywhere `[OWN]`. 0 is never a
   tag; 1 to 0xFFFEFFFF are assigned in order of registration; the test
   range is as for message numbers.
4. **Once assigned, never reused** `[ADR 0004 C44.4: Firm]` and `[OWN]`
   for the reservation rule: a number, tag, code or value keeps its
   meaning for ever. One reserved and never used, or retired, is never
   given to anything else. The definition set lists reserved and retired
   numbers, and the generator refuses their reuse (5.6.1, point 3).
5. **Error codes** (`IpcError.code`) form one registry shared by both
   endpoints, so the two share one generated error type `[OWN]`:

   | Range | Owner |
   |---|---|
   | 0 | never a code |
   | 1 to 255 | this framework (section 7) |
   | 256 to 1,023 | the control endpoint specification |
   | 1,024 to 2,047 | the adapter endpoint specification |
   | 2,048 to 0xFFFEFFFF | unassigned; given to a new owner only by amending this specification |
   | 0xFFFF0000 to 0xFFFFFFFF | test codes (point 8) |

6. **Contract values** (`IpcError.contract`) number ADR 0004, C46's
   semantic error type, once, for both endpoints `[OWN]`. 0 means none.
   The values follow C46, point 1's list in order:

   | Value | C46 error | Value | C46 error |
   |---|---|---|---|
   | 1 | not found | 12 | stale |
   | 2 | exists | 13 | busy |
   | 3 | access | 14 | offline |
   | 4 | `TokenRequired` | 15 | timed out |
   | 5 | `TokenExpired` | 16 | not yet stored |
   | 6 | `TokenRevoked` | 17 | interrupted |
   | 7 | `SecurityFloor` | 18 | not supported |
   | 8 | read-only | 19 | would block |
   | 9 | no space | 20 | unreachable |
   | 10 | quota exceeded | 21 | conflict |
   | 11 | cross-device | 22 | out of range |

   C46 says its values "include" these, so the field is open: a later
   value is added at the next free number (point 4).
7. **Hello results** are the values of 5.1.3; a new result is a new
   number, and older clients show it as "refused (n)" (5.4, point 18).
8. **Test numbers.** Message numbers, tags and codes from 0xFFFF0000 are
   used only by test definitions, which live apart from the shipped
   definition set (5.6.1, point 1) and are generated into test code only
   `[OWN]`. A shipped daemon has no row for them and answers them as it
   answers any unknown number.

### 5.4 Decoding

Checks, in this order. Each failure has the result section 7 gives.
Every frame is decoded under the XDR profile's rules (xdr.md 5.4): caps
before allocation, the bounded cursor, padding zero, closed enumerations
and booleans strict, no trailing bytes `[D3 §5.4]`. No tolerance of xdr.md
5.4, point 11 applies to any IPC message `[OWN]`: both ends are IronAFS,
so a looser input is a fault, not a peer's habit.

**Accepting a connection (daemon)**

1. **The caller first.** The daemon reads the caller from the operating
   system before it reads any byte from the peer, through the endpoint's
   transport `[ADR 0004 C45.1: Firm]`. If the operating system gives no
   caller, the connection is closed; nothing is assumed. The one
   exception is the Windows control pipe, and no other Windows
   transport unless its endpoint specification adopts
   local-ipc-windows.md by name `[OWN]`, applied as ADR 0005, sheets 18
   and 19
   decide `[D2 sheets 18, 19]`:
   - the Windows rule, in full: [local-ipc-windows.md](local-ipc-windows.md), 5.4, point 1.
2. **Admission:** [local-ipc-resources.md](local-ipc-resources.md), 5.4, point 2.
3. **The length** (on Windows, the buffered prefix): refuse 0, a value
   that is not a multiple of 4, and a value above `ipc.frame.hello`, or
   below 60, `Hello`'s smallest size. Each closes without a reply.
4. Read exactly `length` bytes under the frame deadline and the handshake
   deadline.
5. **Decode `Hello`.** A wrong magic closes without a reply (5.3.1). Any
   other decode error closes without a reply. Then, in order: `role` not
   this endpoint's: `ROLE_REFUSED`; `min` greater than `max`:
   `VERSION_MISMATCH`; the version rule of 5.3.2, points 3 or 5:
   `VERSION_MISMATCH` or `SCHEMA_MISMATCH`; the open-per-caller budget
   (point 2): `BUSY`. That last check and the taking of the caller's
   open slot are one step under the admission lock, made here for
   every `ACCEPTED` connection. A connection in `AwaitBinding` already
   holds its open slot, so opening it later needs no second check and
   concurrent bindings cannot pass the budget.
6. Reply `HelloReply`. With any result but `ACCEPTED` the daemon closes
   after the reply is written or the closing deadline passes. With
   `ACCEPTED`, at the chosen version:
   - on an endpoint whose catalogue has no binding row, the connection
     is `Open`;
   - on an endpoint whose catalogue has a **binding row** (5.3.3,
     point 2), the connection is in `AwaitBinding` (5.6.2) `[ADR 0004
     C45.2, C45.3: Firm]`. It is still a handshake connection: it keeps
     its handshake slots and stays under the handshake deadline. In this
     state one gate comes straight after point 8's envelope checks and
     before point 9: a frame passes only if it is a `REQUEST` whose
     number is a binding row of the chosen version and no binding
     request is in flight on the connection. Every other frame is
     `NotBound`, fatal, whatever point 9 would have said of it: a
     `REQUEST` of any other number, known or not, a second binding
     request, any `NOTICE` and any `CANCEL`. A frame that passes goes
     on through points 9 to 14. The daemon sends no `EVENT` and no
     `PART` in this state.
     The connection becomes `Open` only when a binding row's
     handler reports that the endpoint's registration and capability
     checks passed. That is one step under the connection's output
     lock, and a binding request's result slot is completed only under
     that same lock, by whichever path completes it (5.6.4, rule 4).
     When the handler reports success, the step checks, in this order
     and before it completes the slot, reading the clock itself whether
     or not any timer has run:
     1. the state is `AwaitBinding`. If the connection is `Closing` or
        closed, the result is suppressed (point 13);
     2. the clock is before the connection's handshake deadline. If
        not, the connection enters `Closing` as for that deadline and
        the result is suppressed;
     3. the binding request's slot is still Pending. If it is already
        Completed (by its deadline), it keeps that reply, and the
        connection closes as a failed binding;
     4. the clock is before the binding request's deadline. If not,
        with the slot Pending, the step completes the slot as
        `DeadlineExceeded`, appends that reply, and the connection
        closes as a failed binding.

     Only if all four pass does the step complete the slot with the
     handler's success, append its `REPLY`, make the state `Open` and
     free the handshake slots, in that order; only then is any other
     frame admitted or any `EVENT` queued. In every other case the
     connection never opens. The endpoint specification says
     what becomes of a capability spent on a binding that did not
     open. A
     binding that fails is answered with the endpoint's code and the
     connection is closed. An endpoint open only to registered peers
     has a binding row: the generator refuses an adapter catalogue
     without one (5.6.1, point 3). The role, the version and the digest
     in `Hello` are public values and register nothing.

**Each later frame (daemon)**

No body byte is read before point 11. Points 7 to 10 use only the 4
length bytes and the 16 envelope bytes, which the connection's record
holds (5.2.1, rule 2), and they run the same way whatever memory is
free.

7. **Length:** refuse a value below 16 or not a multiple of 4
   (`BadFrameLength`), and a value above the largest cap of the frame
   classes a client may send on this endpoint (`FrameTooLarge`). Both are
   fatal. The class's own cap is checked once the kind is known
   (point 8). The frame's arrival is recorded here (5.6.4, rule 2).
8. **Envelope.** Read and decode exactly the 16 envelope bytes. Then:
   - an undeclared kind, or one the client may not send (`REPLY`,
     `PART`, `EVENT`): `MalformedEnvelope`;
   - `length` above the kind's frame class cap: `FrameTooLarge`;
   - `REQUEST`: `number` 0 or `request_id` 0 is `MalformedEnvelope`; a
     `request_id` not greater than the connection's mark is
     `RequestIdNotIncreasing`. A `REQUEST` that passes raises the mark
     (5.1.4, point 3);
   - `CANCEL`: `number` not 0, `request_id` 0, or `length` not exactly
     16: `MalformedEnvelope`;
   - `NOTICE`: `number` 0 or `request_id` not 0: `MalformedEnvelope`.
   Every failure here is fatal (5.4, point 13), and the body is never
   read.
9. **Catalogue.**
   - a. `REQUEST`: a number with no row of kind request or streaming
     request, or a row newer than the chosen version: `UnknownMessage`
     as its reply; the connection stays. A row the endpoint reserves
     and does not serve: `NotSupported`; the connection stays. Over the
     requests-in-flight budget of the connection or of its caller key:
     `TooManyRequests`; the connection stays. (In `AwaitBinding` point
     6's gate has already run.)
   - b. `CANCEL`: if `request_id` names a request in flight on this
     connection, its handler is asked to stop; otherwise nothing is done
     and nothing is sent. A `CANCEL` is never counted against the
     in-flight budget and never refused for an unknown or late id. It
     is one turn's work like any other frame (5.6.4, rule 1).
   - c. `NOTICE`: a number with no row of kind notice, or a row newer
     than the chosen version: `UnknownMessage`, fatal, because a notice
     has no reply to carry it `[OWN]`.
10. **Authorise**, from the row and the connection's caller, by the
    endpoint's rule, before the body is read `[OWN]`, so a caller who
    may not send a message never has its body buffered or parsed. A
    refusal of a `REQUEST` is its reply, with the endpoint's code; a
    refusal of a `NOTICE` is fatal.
11. **Reserve, read and decode the body.**
    - A `REQUEST` refused at point 9a or 10 with the connection kept
      has its body discarded unread by any decoder, exactly `length` −
      16 bytes, and is then answered, as 5.2.1, rule 2 says.
    - Otherwise the daemon makes the reservation of 5.2.1, rule 2. If
      it fails, a `REQUEST` is refused as `Busy` in the same way, and a
      `NOTICE` is discarded and follows its row's overload rule.
    - Then it reads exactly `length` − 16 bytes under the frame deadline
      and decodes them with the row's generated decoder: xdr.md 5.4 in
      full; each `version` field (5.1.7, point 2); the aggregate
      extension bound as it goes (5.2, point 5); no trailing bytes. A
      failure is fatal `[OWN]`: a body that does not decode means the
      client is broken or hostile.
12. **Walk every extension list** in the body, all of them to the end,
    before anything is decided `[ADR 0004 N41.4: Firm]`:
    - tags out of order or duplicated in a list: `ExtensionOrder`, fatal;
    - a known tag (5.1.7, point 5) with the wrong critical bit:
      `ExtensionFlagMismatch`, fatal;
    - a known tag's value is decoded as its registered type (5.1.7,
      point 8); a failure is `Malformed`, fatal;
    - an unknown critical tag: the whole request is refused with
      `UnknownCriticalExtension` before any state changes; its reply
      still names every unknown non-critical tag found. A `NOTICE` with
      one is dropped whole and counted, never applied in part;
    - an unknown non-critical tag: ignored, and named in the reply's
      `ignored` (5.1.5, point 2) `[ADR 0004 N41.9: Firm]`.
13. **Fatal errors.** After the handshake, a fatal error ends the
    connection, in one step under the connection's output lock `[OWN]`:
    - the connection enters `Closing`, and from that step no frame is
      ever appended to its queue or put in its reserved reply slot,
      except the next item;
    - in the same step the daemon puts one **closing notice** in the
      reserved reply slot, to be written after every frame already
      queued: a `REPLY` with `number` 0 and `request_id` 0 whose body
      is an `ErrorReplyBody` with the code. If the slot already holds a
      frame, no notice is sent;
    - the daemon stops reading and cancels the requests in flight
      (5.6.3, rule 3). A result whose `REPLY` was not yet appended to
      the queue at that step is **suppressed**, whether it won its
      one-shot slot before the step or after: no `REPLY` is queued for
      it, whatever was already encoded is discarded, its buffers are
      released (and zeroed, if secret-bearing; 5.5, point 7), and it is
      counted. A change such a request had already committed
      stays committed (5.6.3, rule 4). So the closing notice, when
      sent, is the last frame, and frames queued before it keep their
      order;
    - the connection is closed when the queue and the slot are written,
      or the write deadline or the closing deadline passes, whichever
      is first. Its slots and reservations are held until its release;
      each request's until its worker returns (5.2.1, rule 5).
    The daemon's own stop closes each connection the same way, with
    no notice. In the handshake, a failure closes as points 3 to 6 say.
14. Only now is the request handed to the executor, without waiting
    (5.6.4, rule 3), or the notice to its consumer. No handler runs on
    the thread that read the frame.

**The client**

15. The client applies points 3, 4, 7 and 12, and point 11's decode, to
    what the daemon sends, with its own frame caps, and checks each
    envelope by kind and direction `[OWN]`:
    - `REPLY`: its `request_id` names a request this connection sent that
      has no reply yet, and its number is that request's; after it the id
      is closed. A `REPLY` with `number` 0 and `request_id` 0 is the
      closing notice: the client reports its code against every
      outstanding request and closes;
    - `PART`: as `REPLY`, the row is streaming, and its reply has not
      come (and point 19);
    - `EVENT`: `request_id` 0, a row of kind event at the chosen version,
      and only in the state the endpoint specification allows it;
    - a frame whose row is secret-bearing, on a connection that is not
      a checked connection (point 17): refused before its body is read;
    - a `PART` or `REPLY` whose request id has taken its failure
      transition (point 19): the envelope is checked as above and the
      body is discarded undecoded, whatever it holds. This is the one
      case in which a body the client reads is not decoded;
    - `REQUEST`, `CANCEL` or `NOTICE` from the daemon, or anything else:
      refused.
    Any refusal, and any decode error, fails the connection: the client
    closes it and reports every outstanding request as failed, never as
    done `[ADR 0004 C46.4: Working]`. A client holds at most one frame
    of input at a time besides what point 19 stages.
16. **The checked connection** `[ADR 0004 C43.1, C43.2: Firm]`. Before a
    client reads or sends any secret (5.5, point 6), on one connection
    handle:
    1. it connects;
    2. it runs the endpoint's daemon check through the operating system
       (for the control endpoint, the control endpoint specification's,
       after ADR 0004, C43, point 4);
    3. it sends `Hello` and reads `HelloReply` on the same handle, and
       checks the whole reply against the `Hello` it sent and the
       endpoint's rules. The reply is valid only if all of these hold
       `[OWN]`:
       - it decodes whole, its magic is right and its result is
         `ACCEPTED`;
       - the daemon's range has the shape its endpoint allows:
         both of one major version; on the control endpoint exactly the
         range of 5.3.2, point 3, that is `daemon_min` = `daemon_max`
         when `daemon_max.minor` is 0 and `daemon_min.minor` =
         `daemon_max.minor` − 1 otherwise (a single version at a minor
         above 0 is refused); on the adapter endpoint `daemon_min` =
         `daemon_max` (5.3.2, point 5);
       - `chosen` is exactly the highest version that lies in both the
         client's range and the daemon's, which the client computes
         itself; a lower common version is refused, as is a `chosen`
         outside either range;
       - on the adapter endpoint, `chosen` is the client's one version
         and the reply's `schema_digest` equals the client's own built
         digest, byte for byte;
    4. it binds the reply's `instance_id` to this connection.
    These steps make one value, the checked connection. It is made per
    connection, never cached or carried to a reconnect, and it alone
    offers the operations that read or send a secret (5.5, point 6). A
    failure at any step ends the attempt with no secret read.
    - **What authenticates what.** Step 2 is the authority: it alone
      shows that the peer is the daemon. The instance id is the run's
      name, bound to the handle that step 2 checked; a different or a
      repeated id neither passes nor fails anything by itself. A client
      that reconnects after a daemon restart runs all four steps again,
      gets a new checked connection with the new id, and proceeds; one
      whose step 2 fails gets nothing, whatever id the peer sends.
    - **Downgrade.** The checks of step 3 stop a peer from choosing a
      lower version than both sides share, or another adapter schema.
      They cannot stop a peer from claiming a lower range than the real
      daemon has; only step 2 does, by showing the peer is the daemon.
      A daemon one minor ahead of a client is the intended case (5.3.2,
      point 3), not a downgrade.
17. A client that only reads non-secret state may skip step 2, as its
    endpoint specification allows; steps 1, 3 and 4 are the same, every
    check of step 3 included `[OWN]`. Such a connection is not a checked
    connection: it cannot send a request whose own body, reply or parts
    are secret-bearing, and it refuses any secret-bearing frame it is
    sent (point 15). What it reads is only as trustworthy as the
    endpoint's address permissions make it.
18. **Tolerances** `[ADR 0004 N41.3, N41.6, N41.8: Firm]`:
    - an unknown non-critical extension in a reply, part or event: kept
      and shown as unknown;
    - an unknown critical extension in a reply: the whole reply is
      failed, "this daemon is newer; upgrade `afs`"; in a `PART`: the
      whole request's result is failed, none of its parts is shown or
      acted on, and the request ends as a request-level failure of
      point 19 (the connection stays); in an event: an unknown event;
    - an unknown error code or contract value: shown as its number plus
      `text`;
    - an unknown value in an open field (`HelloResult` included):
      "unknown (n)"; an unknown `HelloResult` is a refusal.
19. **A stream, at the client:** [local-ipc-resources.md](local-ipc-resources.md), 5.4, point 19.
20. **Every request, at the client:** [local-ipc-resources.md](local-ipc-resources.md), 5.4, point 20.

### 5.5 Encoding

1. The encoder writes the canonical form of xdr.md 5.5 `[D3 §5.5]`, and
   in each extension list ascending tags with no duplicate (5.1.7,
   point 3).
2. It refuses, with a typed error and no partial frame: a value above
   its bound or cap; a message above its frame class's cap or the
   aggregate extension bound; a `REQUEST` with `request_id` 0 or not
   above the last one sent; a `version` other than 1; an open field's
   unknown form whose number is declared (5.1.7, point 7) `[OWN]`, after
   the `wire-format` skill's "the encoder refuses what the decoder
   rejects".
3. A frame is written whole: the encoder computes the message's size,
   reserves the framed size once, writes the prefix and the message, and
   hands the buffer to the transport (xdr.md 5.5, point 3) `[D3 §5.5.3]`.
4. There are no reserved fields in the framework's types `[OWN]`. A
   reserved field in an endpoint's type is that specification's, with its
   value on encode.
5. A sender never puts a critical extension the chosen version predates
   in any message `[ADR 0004 N41.5: Firm]`, and sends only the numbers
   5.1.4, point 5 allows for the frame's kind `[OWN]`: a row of the
   chosen version for a `REQUEST`, `NOTICE`, `PART`, `EVENT` or
   `RESULT_OK` reply; the answered request's own number, row or not, for
   a `RESULT_ERROR` reply; 0 for the closing notice and `CANCEL`. The
   encoder of a closing notice or of a reply to a number without a row
   accepts only an `ErrorReplyBody`.
6. **Secret-bearing messages.** A catalogue row may mark any of its
   bodies secret-bearing (xdr.md 5.1, point 8) `[D3 §5.1.8]`. Its
   decoder follows xdr.md 5.1, point 8.
   - **Client to daemon** `[ADR 0004 C43.1: Firm]`, the mechanism
     `[OWN]`. Encoding and sending a secret-bearing `REQUEST` or
     `NOTICE` is **one operation of the checked connection** (5.4,
     point 16): it takes the message's fields, encodes them into a
     staging buffer the connection owns (`ipc-tx-stage`, point 7), and
     writes that buffer to the connection's own handle. Nothing else
     can do either half: the generated encoder of a secret-bearing body
     and its staging buffer are private to that operation, no function
     returns the encoded bytes, and no function writes caller-supplied
     bytes to a connection. So bytes encoded for one checked connection
     cannot be sent on another, checked or not; a reconnect makes a new
     checked connection (5.4, point 16) and must encode again.
   - **Reading the credential.** An endpoint's function that reads
     credential material to build a secret-bearing message takes the
     checked connection it will be sent on, and what it returns can be
     passed only to that connection's send operation (the type ties
     the two) `[ADR 0004 C43.1: Firm]`: no credential is read before
     the check, and none read for one connection is sent on another.
   - **A request with a secret-bearing reply or part** can be sent only
     through the checked connection as well, so a client never asks an
     unchecked peer for a secret, and a client refuses a secret-bearing
     frame on any other connection (5.4, point 15).
   - **Daemon to client** `[OWN]`. A row marks a reply, part or event
     body secret-bearing only if its endpoint specification states
     which connections may receive it: the caller's authority for a
     reply or part (5.4, point 10), and for an event the registration
     that admitted the connection (on the adapter endpoint, the binding
     of 5.4, point 6). The generator refuses the mark without that
     declaration (5.6.1, point 3). The daemon encodes such a body, as
     one operation of the connection that holds the request or the
     subscription, straight into a buffer that connection owns
     (`ipc-tx-daemon-stage`); the buffer is queued as it is, never
     copied, and no function returns its bytes. A reply suppressed at
     5.4, point 13 never leaves that buffer.
   - **The gate, per endpoint and operating system** `[OWN]`. Each
     endpoint specification names, for each operating system, the daemon
     check a client makes and the gate that proves it there (for the
     Windows control pipe, ADR 0005, sheet 20, in local-ipc-windows.md,
     10.5). Until that gate's result is recorded for an endpoint on an
     operating system, no secret-bearing message of that endpoint exists
     there in either direction: the client's send and receive operations
     for secret-bearing bodies, and the daemon's encode operation for
     them, are not built for that target, by one constant per endpoint
     and operating system that the generated code tests at compile time.
     The daemon therefore cannot encode or transmit such a body there,
     whatever a client does. Nor does it admit one: with the constant
     off, every request and notice row with a secret-bearing body is a
     row the endpoint does not serve on that target. A `REQUEST` for it
     is refused `NotSupported` at 5.4, point 9, its body discarded
     unread by any decoder; a `NOTICE` for it is `NotSupported`, fatal.
     So a peer with its own encoder reaches no decoder and no handler
     for a secret-bearing body before the gate has passed. One
     endpoint's gate never opens another's.
   - A terminal reply built in terminal capacity or in the reserved
     reply slot is never secret-bearing. It is an `ErrorReplyBody`,
     whose `text` holds no secret (5.1.6, point 2), or a streaming
     row's **early-end reply**. Each streaming row declares exactly one
     early-end form, used whenever its stream is cut by a budget, a
     failed reservation, a `CANCEL` or the deadline: either an
     `ErrorReplyBody` with the code that applies, or the row's
     `RESULT_OK` reply with its completeness field set and every
     variable item empty. The generator refuses a streaming row without
     that declaration, or whose early-end form is a secret-bearing body
     (5.6.1, point 3).
7. **Named zeroisation places:** [local-ipc-resources.md](local-ipc-resources.md), 5.5, point 7; the
   Windows rows and the ownership rule are in
   [local-ipc-windows.md](local-ipc-windows.md), 5.5, point 7.
8. **Secret-dependent comparisons:** none in the framework `[OWN]`. The
   instance id and the schema digest are public. An endpoint
   specification names any it has (ADR 0004, O28, point 1).

### 5.6 States and ordering

#### 5.6.1 One definition set, generated codecs

1. **The definition set** is the files of the one directory
   `crates/ironafs-ipc/schema/`, holding every message of both endpoints
   `[ADR 0004 C44.1: Firm]`. Test definitions (5.3.3, point 8) live in
   `crates/ironafs-ipc/tests/schema/` and are not part of it `[OWN]`.
   **Which files, exactly** `[OWN]`, so that every checkout on every
   platform finds the same set under the same names:
   - a file name is 3 to 64 ASCII bytes: a lower-case letter, then
     lower-case letters, digits and `_`, then `.x` (as a pattern,
     `[a-z][a-z0-9_]{0,61}\.x`). No other byte is allowed, so two names
     never differ only in case, and a name has one spelling in every
     file system's encoding;
   - the part before `.x` is none of `con`, `prn`, `aux`, `nul`, `com0`
     to `com9` and `lpt0` to `lpt9`: Windows reserves those device
     names with or without an extension (ADR 0004, part 2, Context,
     point 34), so such a file could not be checked out there;
   - the generator lists the directory without following links. Every
     entry must be a regular file with such a name. Any other entry (a
     subdirectory, a symbolic link, whatever it points to, a special
     file, or any other name, hidden files included) refuses the whole
     set; nothing is skipped silently;
   - an empty set is refused.
2. **The generator** is IronAFS's own code in `xtask` (coding standards
   rule 21): its own parser of the notation the generator's
   specification sets, and two emitters, Rust now and Swift when the
   first Swift consumer exists `[ADR 0004 C44.1: Firm]`.
3. **The generator refuses** `[OWN]`, besides everything xdr.md requires
   of a definition `[D3 §5.1, §5.2]`:
   - a directory entry point 1 does not allow;
   - a byte that is not ASCII printable, tab, LF or a CR followed by LF;
     a lone CR; a file that does not end with LF;
   - a number, tag or code used twice in its registry, or one listed as
     reserved or retired (5.3.3, point 4), or one outside its owner's
     range (5.3.3, points 2, 3 and 5);
   - a body type without `version` first and `ext` last (5.1.7, point 1);
     an extension list in a handshake type;
   - a non-critical extension registered on a request type and not
     advisory (5.1.7, point 4);
   - an extension tag registered without a value type, or whose value
     type is above 1,024 bytes, holds an `ExtensionList` or holds a
     secret field (5.1.7, point 8);
   - a streaming row without `parts_max` and `stream_bytes_max`, or with
     either above its ceiling (5.2, point 11); a notice row without an
     overload rule (5.2.1, rule 2);
   - a secret-bearing reply, part or event body whose row does not
     declare who may receive it (5.5, point 6); a secret-bearing
     handshake type or `IpcError`;
   - an adapter catalogue with no binding row, or a binding row whose
     kind is not request (5.4, point 6);
   - a streaming row without its one early-end form, or with a
     secret-bearing one (5.5, point 6); an event row without a delivery
     class, or a mandatory one without a constant pending bound
     (5.3.3, point 2);
   - a request row whose error arm is not `IpcError` (5.1.5, point 1);
   - a message whose largest size (5.2, point 6) is above its frame
     class's cap, or a frame class cap above `ipc.frame.ceiling`;
   - a variable item without a cap; a cap or memory budget the static
     proof of xdr.md 5.2, point 9 breaks. It computes, for each
     endpoint, T, R (5.2.1, rules 1 and 2) and M: the sum, over the
     endpoint's mandatory rows, of the row's pending bound × (the row's
     largest framed event + one `ipc.overhead.frame`) (5.2.1, rule 4),
     in checked arithmetic, 0 with no such row. It computes each body's
     decode charge (5.2, point 7), and emits them all.
4. **What each definition declares** `[OWN]`, in the generator's
   notation: per catalogue row, the facts of 5.3.3, point 2, a
   streaming row's one early-end form (5.5, point 6) and an event row's
   delivery class (5.2.1, rule 4) included; per
   extension tag, its number, the types it may appear in, its critical
   bit, its introducing version and, on a request type, whether it is
   advisory, and the type of its value (5.1.7, point 8); per enumeration
   field, whether it is open; per body, whether it is secret-bearing
   and, daemon to client, who may receive it; per variable item, its cap
   name and value; per message, its memory budget.
5. **Output is checked in** under `crates/ironafs-ipc/src/generated/`
   `[OWN]`. `cargo xtask ipc-gen` writes it; `cargo xtask ipc-gen
   --check` regenerates in memory and fails on any difference;
   `cargo xtask check` and CI run the check. No build script runs the
   generator, so a reviewer sees the generated code in the diff and
   `ironafs-ipc` stays a pure crate with no build script (ADR 0004, C4,
   point 1).
6. **The generator emits,** per message: the type; an encoder; a decoder
   that takes a byte slice and returns the value or a typed error and
   never panics; the caps and budgets as named constants; the catalogue
   of each endpoint as a static table; T, R and M per endpoint; each
   body's decode charge; and the schema digest as a constant `[OWN]`. The
   encoder of a secret-bearing body is emitted private to the
   connection operation of 5.5, point 6.
7. **The schema digest** `[OWN]`, as issue #62 records the owner's
   decision of 2026-11-20 ("SHA-256 over the canonical, LF-normalised
   definition bytes"):
   - normalise each file of the set by replacing every CR LF pair with
     LF (the generator has already refused every other CR, point 3);
   - order the files by file name, compared as bytes, ascending. The
     file name is the entry's name in the directory, with no path, in
     the ASCII bytes point 1 allows;
   - the canonical bytes are the ASCII text `ironafs-ipc-schema-1`, one
     LF, then for each file in order: the file name's length as an
     unsigned 32-bit big-endian integer, the file name's bytes, the
     normalised content's length as an unsigned 64-bit big-endian
     integer, and the normalised content;
   - the digest is SHA-256 over the canonical bytes, 32 bytes
     `[H1 §1, Figure 1: CONFIRMED]`, computed through `ironafs-crypto`
     (ADR 0004, C5, point 1; its first backend is issue #77).
   The lengths make the file boundaries unambiguous, the order makes the
   result independent of the directory listing, the name rule of point 1
   makes it independent of the file system's case folding and name
   encoding, and the normalisation makes it independent of a checkout's
   line endings. Any change to the
   bytes, comments included, changes the digest. One digest covers the
   whole set, so both endpoints carry the same value.
8. **The additive check.** Once a version of the set has been frozen or
   released, `cargo xtask ipc-gen --check` also compares the set with the
   last frozen or released one and fails on a removed or changed number,
   tag, code, field, bound or meaning-bearing type `[ADR 0004 C44.4:
   Firm]`. Before the first freeze there is nothing to compare, and the
   check says so rather than passing silently.
9. **Generator tests:** golden definition-to-output files; a reject
   corpus of bad definitions, one per refusal in point 3; and a property
   that every generated codec round-trips (10.2) `[OWN]`.

#### 5.6.2 Connection states (daemon)

| State | Event | Next | Rule |
|---|---|---|---|
| Accepted (Linux, macOS) | admission's first step passes; the caller is read; its second step passes (5.4, point 2) | `AwaitHello` | no byte read before the caller is known |
| Accepted (Linux, macOS) | either step fails, or no caller | Closed | nothing read, nothing sent |
| `AwaitHello` | a valid `Hello`, version chosen; the endpoint has no binding row | `Open` | `HelloReply` `ACCEPTED` queued first |
| `AwaitHello` | a valid `Hello`, version chosen; the endpoint has a binding row | `AwaitBinding` | `HelloReply` `ACCEPTED` queued first; still a handshake connection, under the handshake deadline |
| `AwaitHello` | any refusal of 5.4, points 3 to 5, or a deadline | Closing | `HelloReply` with its result where 5.4 gives one |
| `AwaitBinding` | a binding row's handler reports the endpoint's checks passed, and all four checks of 5.4, point 6's opening step pass | `Open` | its `REPLY` queued first; no `EVENT` or `PART` before this |
| `AwaitBinding` | the handler reports success and a check of the opening step fails | Closing | `DeadlineExceeded` for a passed request deadline; otherwise the result is suppressed |
| `AwaitBinding` | a failed binding, any other frame (`NotBound`), any fatal error, or the handshake deadline | Closing | the binding's error reply or the closing notice, best effort |
| `Open` | `REQUEST` | `Open` | exactly one `REPLY`, unless the connection closes first (5.4, point 13) |
| `Open` | `CANCEL` | `Open` | 5.4, point 9b |
| `Open` | `NOTICE` | `Open` | consumed; no reply |
| `Open` | any fatal error (5.4, point 13) | Closing | the closing notice, best effort, last |
| `Open`, `AwaitBinding` | the peer closes | Closed | requests in flight are cancelled (5.6.3, rule 3); what each had committed stays whole (5.6.3, rule 4) |
| `Open`, `AwaitBinding` | the daemon stops | Closing | no new frame read; no notice |
| Closing | queue and slot written, or the write deadline, or the closing deadline | Closed | nothing appended after entering Closing; later results suppressed (5.4, point 13) |
| Closed | every transport operation has reached its terminal completion (Windows); at once (Linux, macOS) | released | the allowance and the connection's places in the counts are released once (5.2.1, rule 5) |

The rows for the Windows states (`PreAttestation`, `Draining`) are in
[local-ipc-windows.md](local-ipc-windows.md), 5.6.2.

1. `PreAttestation`: [local-ipc-windows.md](local-ipc-windows.md), 5.6.2, point 1.
2. **Release** is the one event that frees a connection's allowance, its
   place in the connection count, and any handshake slot it still holds
   (one it did not free by becoming `Open`). On Linux and macOS it
   follows the close at once. On Windows it waits for every operation's
   terminal completion, however long that takes `[D2 sheet 18]`.
3. `Draining`: [local-ipc-windows.md](local-ipc-windows.md), 5.6.2, point 3.
4. Requests may still be running when a connection is released; their
   own charges follow 5.2.1, rule 5.

#### 5.6.3 Ordering rules

1. **Replies may arrive in any order;** the request id pairs them
   `[OWN]`.
2. **Parts of one request** arrive in order, all before its reply. A
   streaming row's terminal reply says whether the stream was complete,
   cut by the parts budget, or cancelled; the endpoint specification
   gives each reply's field for this `[OWN]`.
3. **Cancel, deadline, close or result.** A request's handler result, a
   `CANCEL`, the request deadline (5.6.4, rule 2) and the connection's
   close meet at the request's one-shot result slot (ADR 0004, C39,
   point 3); the first to arrive wins `[OWN]`, decided in one step
   under the slot's lock with the clock read in that step, and a
   completed slot never changes (5.6.4, rule 4). A result already produced
   is reported unchanged, even if its `REPLY` is not yet queued, and a
   later `CANCEL` or deadline does nothing. A `CANCEL` or a deadline
   that arrives first makes the handler stop at its next cancellation
   point, and the reply is `Cancelled` or `DeadlineExceeded`, with
   nothing applied, or a stream's terminal reply reporting the cut. A
   mutating handler that claimed the slot first (5.6.4, rule 4) is not
   stopped: its own result, the ordinary success reply included, is the
   reply, except on a row whose outcome can be unknown, where the
   deadline still answers (5.6.4, rule 4's table). A close
   that arrives first stops the handler the same way and no reply is
   sent (5.4, point 13).
4. **A change is whole or absent.** A mutating request is either applied
   whole and acknowledged, or not applied `[OWN]`. An endpoint
   specification may name a request whose outcome can be unknown to the
   client, and must say what the client then does. A client that loses
   the connection before a reply asks the daemon's state rather than
   assume either result.
5. **The caller never changes** during a connection. A process that
   changes identity reconnects `[ADR 0004 C45.1: Firm]`. On Linux and
   macOS the credentials are those at `connect` `[P4 SO_PEERCRED:
   CONFIRMED]` `[P5 LOCAL_PEERCRED: CONFIRMED]`, so a connection handed to
   another process still carries its opener's identity (section 8).
6. **After a reconnect** a client makes a new checked connection (5.4,
   point 16); one that watches for a daemon restart compares the new
   `instance_id` with the old (ADR 0004, C42, point 3) `[OWN]`.
7. **Events** on one connection arrive in the order the daemon queued
   them. Whether a dropped event is reported, and how, is the endpoint
   specification's `[OWN]`.

#### 5.6.4 Ingress, turns and the executor

In [local-ipc-resources.md](local-ipc-resources.md), 5.6.4.

## 6. Arithmetic

| Computation | Bound | At overflow or excess |
|---|---|---|
| `length` against a frame cap | caps ≤ 2,097,152 | refused before allocation |
| framed size, `length` + 4 | `length` already ≤ the cap | `checked_add` all the same |
| body input, `length` − 16 | after `length` ≥ 16 is checked (5.4, point 7) | `checked_sub` all the same; a failure is `BadFrameLength` |
| every XDR length, count and offset | xdr.md section 6, A1 to A12 | as xdr.md says |
| running extension count + a list's count; running value bytes + a value's length | ≤ 64; ≤ 16,384 | `checked_add`; above the bound is `CapExceeded` |
| request id against the mark | u64 compare, no arithmetic | n/a |
| version choice: the previous minor | computed only when `minor` ≥ 1 | n/a: at `M.0` only `M.0` is accepted |
| client: the highest common version; the daemon range's shape | compares of (major, minor) pairs | `daemon_max.minor` − 1 is computed only when `daemon_max.minor` ≥ 1 |
| digest record lengths: file name (u32), content (u64) | a name is at most 64 bytes (5.6.1, point 1); content above 2⁶⁴ − 1 bytes cannot exist in the set | the generator refuses the set (`try_from`) |
| generator sizes: largest message size, T, R, M (per row, a pending bound × (a framed size + `ipc.overhead.frame`), summed), decode charges, budgets | caps < 2³² | 64-bit `checked_add` and `checked_mul`; overflow refuses the definition |

Rows for the accounts, queues, deadlines and client staging are in
[local-ipc-resources.md](local-ipc-resources.md), section 6.

`[OWN]` for every row. No decoder indexes a slice; every read goes
through xdr.md's bounded cursor.

## 7. Errors and failure

1. **The framework's codes** (5.3.3, point 5) `[OWN]`:

   | Code | Name | When | Connection | State left |
   |---|---|---|---|---|
   | 1 | `FrameTooLarge` | `length` above its class's cap | fatal | none changed |
   | 2 | `BadFrameLength` | `length` 0, not a multiple of 4, or below the envelope | fatal | none changed |
   | 3 | `Truncated` | the stream ends inside a frame, or a body ends inside an item | fatal | none changed |
   | 4 | `Malformed` | a body fails an XDR profile check (xdr.md section 7: the kind travels in `text`) | fatal | none changed |
   | 5 | `MalformedEnvelope` | a rule of 5.4, point 8 | fatal | none changed |
   | 6 | `RequestIdNotIncreasing` | 5.4, point 8 | fatal | none changed |
   | 7 | `BadBodyVersion` | a `version` other than 1 | fatal | none changed |
   | 8 | `ExtensionOrder` | tags out of order or duplicated | fatal | none changed |
   | 9 | `ExtensionFlagMismatch` | a known tag with the wrong critical bit | fatal | none changed |
   | 10 | `UnknownMessage` | a number with no row, or newer than the chosen version | open for a `REQUEST` (its body discarded, 5.2.1, rule 2); fatal for a `NOTICE` | none changed |
   | 11 | `NotSupported` | a reserved row the endpoint does not serve, or a secret-bearing row before its gate has passed (5.5, point 6) | open for a `REQUEST`; fatal for a `NOTICE` | none changed |
   | 12 | `UnknownCriticalExtension` | N41, point 4 | open | none: checked before the handler |
   | 13 | `TooManyRequests` | the requests-in-flight budget | open | none changed |
   | 14 | `Busy` | a reservation failed (5.2.1), or the executor's queue was full (5.6.4, rule 3). Retryable | open | none changed |
   | 15 | `Cancelled` | 5.6.3, rule 3 | open | nothing applied |
   | 16 | `ResultTooLarge` | a stream cut by the parts or part-bytes budget, where the row reports it as an error | open | reads only |
   | 17 | `Contract` | the handler returned a C46 error, in `contract` | open | per that error |
   | 18 | `Internal` | a fault in the daemon | open | logged with a correlation id; `text` holds only that id |
   | 19 | `DeadlineExceeded` | the request's deadline passed before its result (5.6.3, rule 3) | open | nothing applied, unless the endpoint specification names the request as one whose outcome can be unknown (5.6.3, rule 4) |
   | 20 | `NotBound` | before the endpoint's binding, any frame but one binding request at a time (5.4, point 6) | fatal | none changed |

   Handshake refusals are `HelloResult` values, not codes. A closed
   connection with no reply (no caller, a wrong magic, a refused length,
   a pre-attestation failure) has no code on the wire; the daemon counts
   it.
2. **No input in diagnostics.** An error's code, contract value and
   `text`, and everything the daemon logs or counts about a refusal,
   carry only the specification's constants; never a value read from
   the input, and from a secret-bearing message no offset or length
   (xdr.md section 7, point 2) `[D3 §7.2]`. Three items of **protocol
   metadata** are not diagnostics and are repeated from the request by
   the wire format itself `[OWN]`: the envelope's `number` and
   `request_id`, which pair a reply with its request (5.1.4), and the
   `ignored` tag numbers, which ADR 0004, N41, point 9 requires (5.1.5,
   point 2). Each is an integer the envelope or an extension header
   carried, never a field's content, and `ignored` holds only tags the
   receiver does not know, so it never repeats a value of a registered
   field. No other input reaches any reply.
3. **Fail closed.** A request that cannot be decoded, authorised or
   understood whole changes nothing. A client that cannot decode a reply
   reports failure and acts on no part of it `[ADR 0004 N41.4, N41.6:
   Firm]`.
4. **No panic** is reachable from any frame, on either side (coding
   standards rule 1).

## 8. Security

Who can send the bytes: on the control endpoint, any local user; on the
adapter endpoint, only a peer the daemon registered, after the endpoint's
checks; to a client, whatever process answers at the endpoint's address,
which is the daemon only if the daemon check passes.

| Who | Could try | Stopped by |
|---|---|---|
| Any local user | malformed frames to crash the daemon or exhaust its memory | frame caps before allocation (5.2, point 2); xdr.md's caps, cursor and budgets; the aggregate extension bound (5.2, point 5); reservation before allocation (5.2.1); no panic (7, point 4); the fuzz targets (10.4) |
| Any local user | claim another identity in a message | no message carries the caller; it comes from the operating system (5.4, point 1) |
| A process that takes the endpoint while the daemon is down | receive secrets | the operating system's daemon check, on every connection, before any credential is read (5.4, point 16); sending bound to the checked handle (5.5, point 6); the endpoint's address permissions (endpoint specification) |
| A process that replays the daemon's instance id | pass as the daemon | the id authenticates nothing; only the operating system's check does (5.4, point 16) |
| A peer that is not the daemon, or a faulty daemon | make a client use a lower version or another schema | the client recomputes the highest common version and checks the range's shape and, for adapters, the digest (5.4, point 16); a lower claimed range is stopped only by the daemon check |
| Code in a client that holds two connections | send bytes encoded for the checked one on another | encode and send are one operation of the checked connection; no encoded secret leaves it (5.5, point 6) |
| An attested but unregistered peer on the adapter endpoint | receive events or send requests | `AwaitBinding`: nothing but the binding exchange before the endpoint's registration and capability checks pass (5.4, point 6) |
| A malicious or replaced daemon | feed a client bad replies, or an endless stream | the client decodes with the same caps, matches every reply to an outstanding request, and fails the connection on any mismatch (5.4, point 15); streams are staged within the row's part and byte limits and a deadline, and shown only when whole (5.4, point 19); every request has a client deadline (5.4, point 20) |
| Anyone reading freed memory | recover secret bytes from IPC buffers | the named zeroisation places, both sides and both directions, on every path (5.5, point 7) |
| A newer peer | have half of a request applied | unknown critical extensions refuse it whole; non-critical ones may not change what is mutated (5.1.7, point 4; 5.4, point 12) |
| A privileged helper handed a connection opened by another user | act as the opener | documented: the caller is fixed at `connect` (5.6.3, rule 5); the endpoint specification says whether it accepts inherited connections |

Rows for connection, memory and scheduling limits are in
[local-ipc-resources.md](local-ipc-resources.md), section 8; the Windows rows and the Windows
pre-attestation residual are in [local-ipc-windows.md](local-ipc-windows.md), section 8.

Not protected, by design: confidentiality against the kernel, root or
the daemon's own account, and anything a process running as the same
caller can do, which is the same caller (ADR 0004, O23 and C41).

## 9. How the invariants are kept

| Invariant | Kept by | Test that fails if broken |
|---|---|---|
| Constraint 4: no panics, checked arithmetic, byte limits | 5.2; 5.4; section 6; 7, point 4 | fuzz targets (10.4); reject corpus (10.3) |
| C44.1: one framing, one handshake, one definition set, generated codecs, one fuzz suite | 5.1; 5.6.1; 10.4 | `ipc_gen_check`; `handshake_golden` |
| C44.2: the handshake carries a version range, the role and the instance id | 5.1.3 | `handshake_golden`; `hello_rejects` |
| C44.3: untrusted in both directions; a long-lived format | 5.4, points 1 to 15 for both sides; 5.6.1, point 8 | reject corpus (daemon and client cases); `ipc_session_client`; `schema_additive_check` |
| C44.4: only additions within a major; current and previous minor accepted | 5.3.2, points 2 to 4; 5.4, point 16 (the client's own check of the choice); 5.6.1, point 8 | `compat_previous_minor`; `version_choice`; `hello_reply_checks`; `schema_additive_check` |
| C44.5: adapters match exactly | 5.3.2, point 5; 5.4, point 16 (both sides check version and digest) | `adapter_exact_version`; `hello_reply_checks` |
| C44.6: message sets specified before code | the generator reads only the definition set; endpoint specifications hold the sets | review; `ipc_gen_check` |
| C45.2, C45.3: the adapter endpoint only for registered adapters; a binding only from the daemon's registration | `AwaitBinding`: only the binding exchange before the endpoint's registration and capability checks pass, and a generator that refuses an adapter catalogue without a binding row (5.4, point 6; 5.6.1, point 3); the checks themselves are the adapter endpoint specification's. Also the role check and the exact version and digest (5.4, point 5; 5.3.2, point 5) | `binding_before_open`; `generator_rejects` (adapter catalogue without a binding row); `hello_rejects` (role 1 on the adapter endpoint, role 2 on the control endpoint); `adapter_exact_version` |
| C45.1: the caller is the peer the OS reports | 5.4, points 1 and 2; 5.6.3, rule 5 | `caller_before_bytes`; `admission_order`; `preauth_reads_four_bytes` (Windows gate) |
| C43.1, C43.2: no secret read or sent before the daemon is checked; instance id bound to the checked connection | 5.4, points 15 to 17; 5.5, point 6 (send and credential read are operations of the checked connection) | `unchecked_cannot_encode_secret` and `secret_send_bound_to_handle` (compile-fail); `checked_connection_per_handle`; `unchecked_refuses_secret_frame`; the Windows gate's sheet 20 cases |
| N41.1: versioned, bounded, tagged extension lists | 5.1.7; 5.2, points 3 and 5 | `generator_rejects` (missing `version` or `ext`); `ext_aggregate_bound` |
| N41.2 to N41.6: critical bit; unknown handling both ways; a failed stream shown in no part | 5.1.7; 5.4, points 12, 18 and 19 | `critical_ext_refused_whole`; `client_ext_tolerance`; `ext_recognition`; `ext_value_typed`; `client_stream_budget` |
| N41.7: the adapter contract uses the same shapes | 5.1.7 applies to every body of both endpoints; one generator (5.6.1, point 3) | `generator_rejects` (an adapter body without `version` or `ext`) |
| N41.8: unknown error cause as code plus text | 5.1.6, point 3; 5.4, point 18 | `unknown_error_passthrough` |
| N41.9: non-critical request extensions never change what is mutated; unknown ones named | 5.1.7, point 4; 5.1.5, point 2; 7, point 2 | `generator_rejects` (non-advisory request tag); `noncritical_ext_named`; each endpoint's mutation test |
| C5.5 as amended: peer credentials and the Windows pipe only through `ironafs-os` | 5.1.1; 5.4, point 1; ADR 0005's sheets | the `ironafs-os` inventory check of ADR 0005; `ironafs-ipc` stays pure (`check-pure`) |

Rows for C16.6, C39.2, C39.3 and O28 are in [local-ipc-resources.md](local-ipc-resources.md),
section 9.

## 10. Tests

Paths are under `crates/ironafs-ipc/tests/` unless stated. None exists
yet; each is written with the code (issue #95). The Windows cases run at
the Windows gate; until then they are listed as gaps (end of 10.5).

### 10.1 Golden vectors

Derived by hand from this specification, with IronAFS's own values
`[OWN]`. Bytes in `tests/vectors/`, each file the whole frame. `a5×32`
is 32 bytes of 0xA5, a stand-in digest; `11×16` is 16 bytes of 0x11.

| File | Frame, hexadecimal | Proves |
|---|---|---|
| `hello_control_1_0.bin` | `00000040` `49414653` `00000001` `00000001 00000000` `00000001 00000000` `a5×32` `00000003` `61667300` | `Hello` layout: role control, range 1.0 to 1.0, `client_name` "afs" with one byte of padding; length 64 |
| `hello_reply_accepted.bin` | `00000050` `49414653` `00000000` `00000001 00000000` `00000001 00000000` `00000001 00000001` `11×16` `a5×32` | `HelloReply` layout: a 1.1 daemon choosing 1.0 for a 1.0 client; length 80 |
| `hello_reply_mismatch.bin` | as above with result `00000001` and `chosen` `00000000 00000000` | a refusal still fills the daemon's range and instance id |
| `cancel.bin` | `00000010` `00000005` `00000000` `00000000 00000007` | a `CANCEL` of request 7: exactly 16 bytes, number 0 |
| `closing_notice.bin` | `0000002c` `00000002` `00000000` `00000000 00000000` `00000000` `00000001` `00000001` `00000005` `00000000` `00000000` `00000000` | the closing notice: `REPLY`, number 0, id 0; empty `ignored`; `RESULT_ERROR`; `IpcError` version 1, code 5 (`MalformedEnvelope`), contract 0, empty `text`, empty `ext`; length 44 |
| `error_unknown_message.bin` | `0000002c` `00000002` `00000063` `00000000 00000009` `00000000` `00000001` `00000001` `0000000a` `00000000` `00000000` `00000000` | the common error reply for request 9 of number 99, which has no row: `UnknownMessage` (10), decoded without any row's schema (5.1.5, point 1) |

Endpoint golden vectors (control 1.0's bodies) are in the endpoint
specifications.

**The schema digest's canonical bytes** (5.6.1, point 7), for a set of
two files `[OWN]`: `a_first.x`, whose content is `/* a */`, CR LF,
`const A = 1;`, CR LF; and `b_second.x`, whose content is
`const B = 2;`, LF. In `tests/vectors/schema_digest/`, with the
canonical bytes in `canonical.bin` (98 bytes):

| Part | Bytes, hexadecimal |
|---|---|
| the label and LF | `69726f6e6166732d6970632d736368656d612d31` `0a` |
| first name: length 9, `a_first.x` | `00000009` `615f66697273742e78` |
| first content: length 21, CR LF pairs now LF | `0000000000000015` `2f2a2061202a2f0a636f6e73742041203d20313b0a` |
| second name: length 10, `b_second.x` | `0000000a` `625f7365636f6e642e78` |
| second content: length 13 | `000000000000000d` `636f6e73742042203d20323b0a` |
| SHA-256 of the 98 bytes | `16eb51dcacee49c585ed5adc922d691527d051b6ad91128d45c17ce8c58d046c` |

The bytes were laid out by hand from 5.6.1, point 7 and the digest
computed with a general-purpose SHA-256 tool, outside the Cargo build.
These two files are a test fixture, not definitions: the vector tests
the canonical form, not the parser.

### 10.2 Round-trip properties

Over values from IronAFS's own seeded generator (ADR 0004, O47, point 2):

1. `decode(encode(v)) == v` for every generated type.
2. `encode(decode(b)) == b` for every accepted `b`; no tolerance exists
   (5.4), so every accepted input is canonical.
3. A frame split at every byte boundary across reads decodes the same.
4. An open field's undeclared value decodes to its unknown form and
   re-encodes to the same bytes.
5. For every request row, an `XReplyBody` with `RESULT_ERROR` encodes to
   the same bytes as the matching `ErrorReplyBody`.
6. The schema digest of a copy of the set with CR LF line endings equals
   that of the LF copy; a one-byte change, a renamed file, or a reordered
   listing of the same files gives the result the rules of 5.6.1, point 7
   give (different, different, the same).
7. The accounts property: [local-ipc-resources.md](local-ipc-resources.md), 10.2, item 7.

### 10.3 Reject corpus

Each input, its result and the section it breaks:

- daemon, handshake: length 0, 2, 56 (below 60), 96 (above 92); a wrong
  magic; `role` 0, 2 on the control endpoint, 7; `min` above `max`; a
  range with no accepted version; on the adapter endpoint, a range of two
  versions and a wrong digest; a 33-byte `client_name`; non-zero padding
  in `client_name`; trailing bytes; a second `Hello`; a `REQUEST` before
  `Hello` (5.4, points 3 to 5); on an endpoint with a binding row, a
  non-binding `REQUEST`, a `NOTICE` and a `CANCEL` before the binding,
  each `NotBound` (5.4, point 6);
- daemon, after the handshake: `length` 4, 8, 12 (below an envelope),
  18 (not a multiple of 4), 65,540 (above the request cap); kind 0, 2, 3,
  4, 7; a `REQUEST` with number 0, with id 0, with an id equal to the
  last; a `CANCEL` with number 1, with id 0, with a body; a `NOTICE` with
  an id; a body `version` 0 and 2; tags 5 then 3, and 4 twice, in one
  list; 17 extensions in one list; 65 extensions over nested lists;
  16,385 value bytes over several lists; a 1,025-byte value; a known tag
  with its critical bit flipped; a known tag whose value is not a whole
  message of its registered type (short, trailing bytes, non-zero
  padding); an array count of 0xFFFFFFFF; trailing bytes after a body;
  a `REQUEST` of an unknown number, of an unauthorised row and of a row
  over the in-flight budget, each with a 60,000-byte body, each
  followed by a valid request that must be answered (5.4, points 7 to
  12);
- client: a `REPLY` for an id never sent, a second `REPLY` for one id, a
  `PART` for a non-streaming row, a `REPLY` whose number differs from
  its request's, an `EVENT` the endpoint does not allow in this state, a
  `REQUEST` from the daemon, a reply above the aggregate bound, a
  `HelloReply` whose `chosen` lies outside the client's range, or
  inside both ranges and below their highest common version, or whose
  daemon range is reversed, spans two majors, spans three minors, or
  is the single version 1.1 (chosen 1.1); on
  the adapter endpoint a `HelloReply` with a two-version daemon range or
  another digest; a secret-bearing frame on an unchecked connection; a
  stream of `parts_max` + 1 parts, and one of `stream_bytes_max` + 4
  bytes (5.4, points 15, 16 and 19);
- encoder: a listed value in an open field's unknown form; a request id
  0; a message one byte over its cap (5.5, point 2); a `REQUEST` with
  number 0 or a number the chosen version lacks; a `RESULT_OK` reply to
  a number without a row; a closing notice with a body that is not an
  `ErrorReplyBody` (5.5, point 5). The closing notice (number 0) and an
  error reply echoing an unknown number are accepted, as 10.1's
  vectors show;
- generator: one bad definition per refusal of 5.6.1, point 3; and one
  bad directory per rule of 5.6.1, point 1: `con.x` and `com1.x`, a name
  with an upper-case letter, a name with a non-ASCII byte, a 65-byte
  name, a subdirectory, a symbolic link to a valid file, a hidden file,
  an empty directory.

### 10.4 Fuzz targets

One suite for both endpoints (ADR 0004, C44, point 1), in `fuzz/`, named
as coding standards rule 20 sets out. Every target runs under a counting
allocator and asserts no panic and no single allocation above the frame
cap in force:

- `ironafs-ipc_frame`: bytes into the frame reader and envelope decoder;
  the cursor never passes the frame's end.
- `ironafs-ipc_decode_all`: bytes into every generated decoder, chosen
  by the first byte; every accepted input re-encodes to the same bytes.
- `ironafs-ipc_session_daemon`: a sequence of frames into the daemon's
  connection state machine with a stub endpoint and a writer that may
  stall; at most one reply per request; no handler call before
  authorisation or for a request with an unknown critical extension; no
  body byte read before authorisation; after every refusal that keeps
  the connection, the next frame read at the right offset; the live
  bytes each account counts never above its budget, and each allocation
  inside a reservation; ordinary occupancy above the output budget only
  by funded terminal replies, and mandatory occupancy never above M;
  nothing queued after a closing notice
  (5.2.1, rules 2 and 4; 5.4, point 13).
- `ironafs-ipc_session_client`: the same from the daemon's side into the
  client; no outstanding request reported done after a failed
  connection; no part shown before its stream's terminal reply; staged
  bytes never above the row's limits.
- `xtask_ipc_schema`: text into the definition parser; anything accepted
  regenerates identically.

### 10.5 Acceptance cases

- `handshake_golden`, `hello_rejects`: 10.1 and 10.3's handshake cases.
- `version_choice`: daemons at 1.0, 1.1 and 2.0 against client ranges
  1.0–1.0, 1.0–1.1, 1.0–1.2, 1.1–1.1 and 0.0–0.9: the chosen version or
  `VERSION_MISMATCH` as 5.3.2, point 3 gives; a 1.2 daemon refuses a
  1.0-only client.
- `compat_previous_minor`: a client built from the previous minor's
  definition set completes a request against the current daemon. Until
  the first minor bump, a synthetic 1.1 test build serves a 1.0 client.
- `adapter_exact_version`: an adapter `Hello` with the daemon's version
  and digest is accepted; one with another version, a two-version range,
  or another digest is refused with the right result and closed.
- `hello_reply_checks`: a client against a stand-in daemon that answers
  `ACCEPTED` with each bad reply of 10.3; every one fails the handshake
  and no request is sent. A 1.1 client against a stand-in that claims
  the range 1.0 to 1.1 and chooses 1.0 refuses; against a real 1.0
  daemon it proceeds at 1.0.
- `binding_before_open`: on a test endpoint with a binding row, an
  attested peer with a valid `Hello` receives no `EVENT` and has no
  request served before its binding succeeds; a client that holds a
  checked connection and has not bound is sent no secret-bearing reply
  or event; an unknown number, a `CANCEL`, a `NOTICE` and a second
  binding request before the binding are each `NotBound`; two
  connections of one caller binding at once cannot pass the caller's
  open budget; a binding that finishes after the handshake deadline
  fired is suppressed and never opens; with the stand-in clock past
  the handshake deadline and its timer held back, a successful binding
  still does not open; a binding request whose own deadline completed
  its slot first is answered `DeadlineExceeded`, the later success
  opens nothing, and the connection closes; with the clock past the
  binding request's deadline, that timer held back and the handshake
  still live, a successful binding is answered `DeadlineExceeded` and
  the connection closes without opening; a failed binding closes;
  the handshake deadline covers the wait; the connection counts as a
  handshake slot until it is `Open`.
- `caller_before_bytes` (Linux, macOS): with a stand-in transport that
  records the order of calls, the caller is read before any read on the
  connection, and a connection with no caller is closed with zero bytes
  read.
- `checked_connection_per_handle`: the checked connection cannot be made
  from a `HelloReply` read on another handle, nor reused after a
  reconnect. A legitimate restart: the stand-in daemon restarts with a
  new instance id and passes the operating system's check; the client
  makes a new checked connection, sees the changed id, and sends its
  secret on the new connection. A planted endpoint: the stand-in fails
  the operating system's check and replays the earlier, genuine
  instance id; no credential is read and nothing secret is sent. The
  instance id decides neither case.
- `unchecked_cannot_encode_secret`, `secret_send_bound_to_handle`:
  compile-fail tests. A secret-bearing message cannot be encoded or sent
  without a checked connection; its encoder and staging buffer cannot be
  named outside the connection's send operation; a credential read for
  one checked connection cannot be passed to another's send; nothing
  read or encoded before a reconnect can be sent after it.
- `secret_gate_off`: on a test endpoint built with its gate constant
  off, a stand-in peer with its own encoder sends a secret-bearing
  `REQUEST` and `NOTICE`: the request is answered `NotSupported` with
  its body discarded and no decoder or handler run, the notice is
  fatal; and a stand-in daemon that sends a secret-bearing reply and
  event to a client built with the gate off is refused before the body
  is read. Neither side has the encode or send operation in its build.
- `unchecked_refuses_secret_frame`: an unchecked client fails the
  connection on a secret-bearing reply, part or event before reading
  its body, and cannot send a request whose reply is secret-bearing.
- `ext_value_typed`: a known tag's value is decoded as its type before
  the handler runs; a bad value is fatal and the recording endpoint
  shows no handler call.
- `ext_aggregate_bound`, `ext_recognition`, `critical_ext_refused_whole`,
  `noncritical_ext_named`, `client_ext_tolerance`,
  `unknown_error_passthrough`: N41's cases, with a recording endpoint
  showing no state change on refusal, and a request with 64 extensions
  (one unknown critical and 63 unknown non-critical) refused and naming
  all 63.
- `cancel_cases`: a `CANCEL` for a running read, for a committed change,
  for a stream, for an unknown id, for an answered id, and for id 100
  when the mark is 5 (a later `REQUEST` with id 6 is accepted).
- `closing_notice`: each fatal error after the handshake yields the
  closing notice when the slot is free and none when it is in use; the
  client reports the code against every outstanding request. With
  handlers that finish at every point around the fatal error, the
  notice is the last frame written, no reply follows it, suppressed
  replies are counted, and a stalled reader is closed at the closing
  deadline.
- `ipc_gen_check`, `schema_digest_fixture`: `cargo xtask ipc-gen
  --check`; the digest of the checked-in set equals a value recorded by
  an independent procedure outside the Cargo build, over the canonical
  bytes of 5.6.1, point 7; `schema_digest_files`: 10.1's two-file
  vector, byte for byte, and each bad directory of 10.3.
- `schema_additive_check`: from the first freeze.
- The acceptance cases for admission, accounts, turns, the executor,
  deadlines, output, client streams, configuration and zeroisation are
  in [local-ipc-resources.md](local-ipc-resources.md), 10.5; the Windows gate is in
  [local-ipc-windows.md](local-ipc-windows.md), 10.5.

## 11. Revisit if

- ADR 0004, C5 (Working): "a mainstream crate offers rxkad's cipher, a
  FIPS mode is scheduled (O31), or an operating system ships a safe
  interface that removes a site from the `ironafs-os` list." A removed
  peer-credential or pipe site changes 5.1.1 and 5.4, point 1.
- This specification's own:
  - a control result needs more than 65,536 parts or 16 MiB in one
    stream (5.2, point 11);
  - a measured control reply comes near the 1 MiB cap, or an adapter
    frame needs more than the 2 MiB ceiling (5.2, point 2);
  - a second language consumer whose needs the XDR profile cannot
    express;
  - a platform where a client cannot read the endpoint owner's
    credentials, which would need another daemon check and so an
    amendment of ADR 0004, C43, point 4.

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| X1 | SPECIFICATION | RFC 4506, register row E1, through the XDR profile (D3) | 5.1 |
| H1 | SPECIFICATION | FIPS 180-4, Secure Hash Standard, August 2015, register row E53: section 1, Figure 1 (SHA-256's message digest is 256 bits) | 5.6.1 |
| P1 | DOCUMENTATION | Microsoft Learn, `CreateNamedPipeW`, register row E48: parameters `dwPipeMode`, `nMaxInstances`; Remarks | 5.1.1, 5.2 |
| P2 | DOCUMENTATION | Microsoft Learn, `ImpersonateNamedPipeClient`, register row E49: Remarks | 5.4 |
| P3 | DOCUMENTATION | Microsoft Learn, System Error Codes 1300 to 1699, register row E50: error 1368 | 5.4 |
| P4 | DOCUMENTATION | Linux man-pages 6.19, unix(7), register row E51: socket option `SO_PEERCRED` | 5.1.1, 5.6.3 |
| P5 | DOCUMENTATION | unix(4), macOS 26.7, register row E52: `LOCAL_PEERCRED` and the supported socket types | 5.1.1, 5.6.3 |
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; C4, C5, C9, C16, C37, C39, C41 to C46 ([part 1](../adr/0004-architecture-1-core.md)); L35 ([part 2](../adr/0004-architecture-2-os-layers.md)); O16, O23, O28, O47 ([part 3](../adr/0004-architecture-3-operations.md)); N41 ([part 4](../adr/0004-architecture-4-negotiation.md)); S1 | all |
| D2 | DECISION | [ADR 0005](../adr/0005-ironafs-os.md), accepted 2026-11-21: sheets 13 and 16 to 20; Consequences (the amendment of C5, point 5) | 1, 5.1.1, 5.2, 5.4, 5.5, 5.6.2, 8, 10.5 |
| D3 | DECISION | [The XDR profile](xdr.md) | 5.1, 5.2, 5.4, 5.5, 5.6.1, 6, 7 |
| D4 | DECISION | [Coding standards](../coding-standards.md), rules 1 to 3, 8, 9, 15, 20 and 21; the `wire-format` skill | 5, 6, 7, 10 |
| D5 | DECISION | Issue #62: the owner's decisions of 2026-11-20 recorded there (the split, the two kinds of limit, the digest, the Windows pool and prefix rule) | 5.2, 5.6.1 |
| O | OWN | Every point tagged `[OWN]`, with its reason in place | 5, 6, 7 |

No other implementation's source, test data or interface definitions
were read for this specification, and none are cited. The outside
documents are cited for facts only, in IronAFS's own words, and none is
quoted (register rows E48 to E53, "Licence status"). They are platform
facts and a hash's output length, not AFS wire facts. The vectors of
10.1 are IronAFS's own.

### 12.1 Observed behaviour

None yet. To be recorded at each layer's gate: the credentials a client
reads for a listening socket (Linux, macOS), and the Windows gate cases
of 10.5.

## 13. Open questions

- **Q1. Freezing.** The framing, handshake, envelope and numbering rules
  are frozen together with the control 1.0 message set and its bounds,
  as one class R change, before the first release that ships `afs`
  (issue #62, the owner's decision of 2026-11-20). Until then this
  document is `draft`, then `reviewed`.
- **Q2. A descriptor or handle beside a frame** (ADR 0004, L35, point 2).
  No frame in this version carries one. Before the first control
  operation that passes one, this specification gains a section that
  settles: how a descriptor is paired with its frame on Linux and macOS,
  at most one per request, closed after the operation and on every
  refusal (Linux through ADR 0005, sheet 14; macOS needs its own site,
  added to ADR 0005's list first); and on Windows how a handle reaches
  the daemon's process and with which rights. Blocks: the `fs`
  operations. No public issue yet.
- **Q3. The adapter endpoint's frame caps** (5.2, point 2). Set by the
  adapter endpoint specification (ADR 0004, S1), at most the ceiling.
  Blocks: enabling the adapter endpoint.
- Q4 and Q5 (Windows) are in [local-ipc-windows.md](local-ipc-windows.md), section 13.
- **Q6. The adapter endpoint's mandatory events** (ADR 0004, C16,
  point 6). This framework keeps the space and the order (5.2.1,
  rules 1 and 4) and requires each event row to declare its class and
  a constant pending bound. The rest belongs to the adapter endpoint
  specification (ADR 0004, S1), not yet written, which must:
  - make the epoch advance a mandatory row;
  - show how coalescing and overflow keep the pending advances within
    the row's bound in every state, so that an append is never refused
    in normal operation;
  - say what a refused append (a fault) does to the binding, and when,
    without depending on the connection's queue draining;
  - keep C16, point 6's separate moments: the revocation recorded, the
    advance applied by the front end, and the advance acknowledged or
    the binding torn down. A written event is none of these.

  Two review findings stand open against it until then, quoted from the
  review, without their labels:

  > The mandatory class and prepaid space are present, but the
  > generic output and arithmetic rules still reject any `EVENT`
  > above the ordinary queue budget. A pending limit per key does
  > not bound the number of live keys. The generator output list
  > omits M, and there is no explicit requirement that the adapter's
  > epoch advance use the mandatory class. The saturation test also
  > puts the event behind existing frames and then expects it to be
  > written first.

  > New mandatory-event teardown contradiction. Exceeding the declared
  > bound supposedly closes the connection at once and tears down the
  > binding, but the cited fatal path enters `Closing` and drains queued
  > output. The new paragraph treats writing the event as the end of its
  > fencing handoff, while C16.6 distinguishes front-end application
  > from acknowledgement or teardown. It also leaves ordinary coalescing
  > and overflow indistinguishable from exceeding the bound due to a
  > fault.

  Of the first, the occupancy, the aggregate bound, M and the test are
  settled here; the requirement on the epoch advance is the adapter
  endpoint specification's. Of the second, the framework's teardown and
  fencing sentences are withdrawn; all of it is that specification's.
  Blocks: enabling the adapter endpoint.

### 13.1 Blockers and open items from review

The review ended without the second seat passing ([the review
record](../reviews/2026-11-21-m2a-62-local-ipc.md)). The items below
were still open at its last round. A **blocker** (B) is a major:
implementation (issue #95) must not start on the sections it names until
it is closed by a reviewed change to this specification. An **open
item** (M) is a minor, recorded and not gating. B1, B3, B4 and B6 to
B8 are in local-ipc-resources.md, 13.1; where they name a place in
this part, that place is gated too.

- **B2. The binding step when both deadlines have passed, and a binding
  handler that must claim.** Parts: local-ipc.md, 5.4, point 6 and
  5.6.2; local-ipc-resources.md, 5.6.4, rule 4.
  Quoted from the review, without its label:

  > Defect: The ordered checks correctly handle an expired request
  > with a live handshake and a Pending slot. The state table still
  > says to answer DeadlineExceeded for a passed request deadline
  > even when the handshake deadline has also passed and the ordered
  > check suppresses the result. The opening step also requires
  > Pending, while a binding handler that changes state must claim
  > under the general mutation rule.
  >
  > Consequence: The prose and table give different wire outcomes when
  > both deadlines have passed. A binding handler that follows the
  > commit-claim rule can report success from Claimed, a state the
  > opening step cannot accept and has no failure branch for.
  >
  > Fix asked: Have the table use the ordered opening step's exact reply
  > behavior. Specify whether binding checks remain read-only until the
  > serialized opening step or how a claimed binding is arbitrated.
  > Cover both expired deadlines and the chosen binding commit path in
  > acceptance cases.

  Closes with: the state table giving exactly the opening step's reply
  in every case, and a decision on whether binding checks stay read-only
  until the opening step or how a Claimed binding slot is arbitrated,
  with acceptance cases for both.
  Gated until then: 5.4, point 6, the `AwaitBinding` rows of 5.6.2, and
  so the adapter endpoint.

- **B5. The secret gate in the ordered frame checks, and one predicate
  for the Windows prefix rule.** Parts: local-ipc.md, 5.4, points 9 and
  15 and 5.5, point 6; local-ipc-resources.md, 5.2.1, rule 1 and 5.4,
  point 2. Quoted from the review, without its label:

  > Defect: The gate now prohibits daemon admission of
  > secret-bearing input, and the caller-first rule scopes prefix
  > capture to the control pipe and explicit adopters. The actual
  > catalogue step omits the gate-off branch for notices, and the
  > client envelope step checks whether the connection is checked
  > without checking whether its gate is enabled. Resource admission
  > includes adopters in its first step but omits them from the
  > capture timing in its second, while capture scratch is still
  > specified for Windows generally.
  >
  > Consequence: Following the ordered frame checks can reach a body
  > read that the gate is meant to prevent. A Windows endpoint can also
  > get capture accounting that disagrees with its transport selection.
  >
  > Fix asked: Put the gate predicates in the daemon catalogue and
  > client envelope steps. Test gate-off receive on an otherwise checked
  > connection, including PART. Use the same
  > control-pipe-or-explicit-adopter predicate for all prefix admission,
  > capture and accounting rules.

  Closes with: the gate's predicate written into the daemon's catalogue
  step (notices included) and the client's envelope step, a test of
  gate-off receive on a checked connection, `PART` included, and the
  same control-pipe-or-explicit-adopter predicate in every prefix
  admission, capture and accounting rule.
  Gated until then: every secret-bearing row on every endpoint, and the
  capture scratch of 5.2.1, rule 1.

## 14. Review

Reviewed in five rounds on 2026-11-21 by two non-author families, and
not converged: one seat passed and one did not. The rounds, every
finding and the decision to stop are in [the review
record](../reviews/2026-11-21-m2a-62-local-ipc.md). Open here: blockers
B2 and B5 (13.1), and Q6; B1, B3 and B8 also name places in this
part. This specification stays `draft`.
