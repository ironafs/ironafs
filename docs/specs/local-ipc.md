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

ADR 0004, C45, points 1 and 2 (**Firm**), the first sentences, which
this framework serves:

> 1. **The control endpoint** is open to any local user. The caller is the
>    peer the operating system reports. […]
> 2. **The adapter endpoint** is open only to adapter processes the daemon
>    itself registered. […]

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
token (C45, point 1)". ADR 0005's sheets 13 and 16 to 20 are those sites.
Sheets 17 to 19 refer to the control IPC design for the pipe's instance
count and caps ("its 5.2"), the prefix rule ("control IPC 5.4, step 1")
and the named zeroisation places. The control IPC design is two
specifications, this framework and the control endpoint specification,
and those references resolve to this document: 5.2, point 10 (the instance count);
5.4, point 1 (the prefix rule); and 5.5, point 7 (the zeroisation
places).

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
9. Every byte the daemon holds for a connection is reserved from a global
   budget before it is allocated, and every admitted request has its
   terminal reply paid for before its handler runs.

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
2. **Windows:** a named pipe in byte mode, read in byte mode, rejecting
   remote clients, used with overlapped I/O (ADR 0005, sheets 17 and 18)
   `[D2 sheet 17]`. A byte-mode pipe does not separate one write from
   another `[P1 dwPipeMode: CONFIRMED]`, so framing is this document's,
   as on Unix.
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
   client can say which version to install `[OWN]`.
4. `instance_id` is sent in every reply. It is not a secret: every
   connecting caller is told it. It is what a client binds to its
   checked connection (ADR 0004, C43, point 2) and what an agent compares
   after a reconnect (ADR 0004, C42, point 3) `[ADR 0004 C44.2: Firm]`.
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
8. **Policy budgets.** Each has a default and a hard ceiling. A
   configured value of 0 or above its ceiling, or one that breaks a rule
   in point 9, refuses the whole configuration: at start the daemon does
   not start; on a reload the last good configuration stays `[OWN]`.

   | Budget | Default | Ceiling | On excess |
   |---|---|---|---|
   | Connections in handshake (Windows pre-attestation and `AwaitHello` together), all callers | 16 | 64 | close without reading (5.6.2) |
   | Connections in `AwaitHello` per caller key (Linux and macOS, where the caller is known at accept) | 4 | 16 | close without reading |
   | Open connections per caller key | 16 | 64 | `HelloReply` `BUSY`, then close |
   | Open connections in all | 128 | 1,024; on Windows also point 10 | close without reading |
   | Listening pipe instances (Windows) | 2 | 8 | n/a: the daemon never creates more |
   | Handshake deadline: from accept until `HelloReply` is queued | 5 s | 30 s | close; no reply |
   | Frame deadline: from a frame's first byte until its last | 10 s | 60 s | close |
   | Write deadline: no byte of queued output written for this long | 30 s | 300 s | close; requests in flight are cancelled (5.6.3) |
   | Requests in flight per connection | 16 | 256 | `TooManyRequests` for the extra request; the connection stays |
   | Parts per streaming request | 4,096 | 65,536 | the stream ends; its terminal reply reports the early end |
   | Output queued per connection (encoded frames not yet written) | 2 MiB | 16 MiB | back-pressure (5.2.1, rule 4) |
   | In-flight bytes, all connections (5.2.1) | 64 MiB | 1 GiB | `Busy` without running the request |

   An endpoint specification may set a lower ceiling for its endpoint, or
   add budgets of its own (the adapter endpoint's credit), never a higher
   ceiling `[OWN]`.
9. **Rules between budgets**, checked when the configuration is
   validated `[OWN]`:
   - connections in handshake ≤ open connections in all;
   - output queued per connection ≥ the endpoint's largest framed reply
     frame plus T (5.2.1, rule 1), so one reply always fits;
   - in-flight bytes ≥ (connections in handshake + open connections in
     all) × the admission allowance (5.2.1, rule 1);
   - on Windows, point 10.
10. **The Windows pipe pool.**
    - Windows accepts an instance count from 1 to 255, and 255 means no
      limit but the system's resources `[P1 nMaxInstances: CONFIRMED]`;
      every instance of one pipe must pass the same count, or creation
      fails `[P1 Remarks: CONFIRMED]`. A count of 1,024 cannot be passed.
    - `ipc.pipe.instances` = **254** for each IronAFS pipe, unless its
      endpoint specification sets a lower constant. It is a constant in
      the `ironafs-os` wrapper, the same on every instance (ADR 0005,
      sheet 17), never configuration, and 255 is never passed, because
      it would leave the bound to the system `[D2 sheet 17]`. 254 is the
      largest finite count: a multi-session host runs one session agent
      per logon session beside short-lived `afs` commands, and the
      per-caller budgets, not a small pool, stop one user from taking
      every instance `[OWN]`.
    - Every instance counts against the pool from its creation until its
      handle is closed: listening, in pre-attestation, in `AwaitHello`,
      open, and disconnecting with an operation not yet at its terminal
      completion (ADR 0005, sheet 18). Configuration is refused unless
      listening instances + open connections in all ≤ 254 `[OWN]`; since
      connections in handshake ≤ open connections in all (point 9),
      unauthenticated handshakes fit inside it too. When every instance
      is in use, the daemon creates none, and a new client's connect
      finds the pipe busy.

#### 5.2.1 Reservation before allocation

The in-flight budget of 5.2, point 8 counts every byte the daemon holds
for connections. Each byte is reserved before it is allocated, never
charged afterwards `[OWN]`, from coding standards rule 3 and ADR 0004,
C39, point 3.

1. **Admission.** Each accepted connection reserves an allowance: one
   handshake buffer of 96 bytes (the larger handshake frame, framed), a
   discard buffer of 4 KiB for refused frames, and one **reserved reply
   slot** of T bytes. T is the largest framed terminal reply the daemon
   may have to send without its handler having built it: the larger of
   the framed `ErrorReplyBody` (18,160 bytes, 5.2, point 6) and the
   largest early-end reply of the endpoint's streaming rows, computed by
   the generator (5.6.1, point 3). If the budget cannot cover the
   allowance, the connection is closed without reading and without a
   reply; on Windows it never leaves pre-attestation admission.
2. **Before a frame's body is read** (state `Open` only) the daemon
   reserves `length` bytes, already checked against the cap, and T bytes
   of **terminal capacity**, as one reservation. If the frame is a
   `CANCEL` or a `NOTICE`, or is refused at 5.4, point 8, the terminal
   capacity is released at once; for an admitted `REQUEST` it is that
   request's. If the reservation fails, nothing is allocated for the
   frame, and:
   - a `length` below 16 is refused as `Truncated` (fatal) unread;
   - otherwise exactly the 16 envelope bytes are read into the handshake
     buffer, which is free once the connection is open, and 5.4, point 8
     runs in full. A `REQUEST` that passes it raises the mark, has the
     rest of its frame read and discarded through the discard buffer,
     each chunk zeroed, and is answered `Busy` from the reserved reply
     slot. A `NOTICE` is discarded the same way and counted. A valid
     `CANCEL` goes to 5.4, point 9b.
3. **Handlers.** A handler reserves its largest working set before it
   allocates it, and a mutating handler before its first change. The
   working set of a non-streaming request includes its largest encoded
   reply, so a reply, once built, never needs another reservation. A
   failed reservation ends the request as `Busy`, nothing applied, from
   its terminal capacity. An endpoint specification may name a request
   whose reply after a committed effect differs (an outcome that cannot
   be confirmed in time); its reply is still paid from the handler's
   working set.
4. **Output.**
   - An admitted request holds its terminal capacity until its one
     `REPLY` is queued. Any terminal reply that is not the handler's own
     built reply (an error, `Busy`, `Cancelled`, a stream's early end) is
     encoded into it. When the handler's own reply is queued instead, the
     capacity is released.
   - A `PART` and an `EVENT` reserve their bytes before they are
     encoded. A `PART` whose reservation fails ends the stream; the
     terminal reply reports the early end. An `EVENT` whose reservation
     fails is dropped and counted under the endpoint's rule.
   - The output cap is a back-pressure threshold, never a refusal of a
     funded terminal reply. While a connection's queue is at or above
     its cap, the daemon reads no frame from it, builds no `PART` for it
     and queues no `EVENT` to it; a terminal `REPLY` whose bytes are
     already reserved is still appended, after the frames already in the
     queue. The queue can pass its cap only by the funded terminal
     replies of requests already in flight, each within its own
     reservation, so the global budget still holds.
   - The reserved reply slot is outside the output cap and is used only
     for point 2's `Busy` or for the closing notice (5.4, point 13). While
     it holds an unwritten frame, nothing more is read from the
     connection.
5. **Release.** Every reservation is released exactly once, when its
   bytes are freed, with the checked counters of section 6. A buffer the
   operating system still owns (an overlapped read or write on Windows)
   is freed, and its reservation released, only at that operation's
   terminal completion (ADR 0005, sheet 18). A request that runs on after
   its connection closes keeps its working set until it ends.

So the ceiling is never passed, and every admitted request's terminal
reply is paid for before its handler runs.

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
     it, its body types, and its cap table. The endpoint specification
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
   exception is Windows `[OWN]`, applied as ADR 0005, sheets 18 and 19
   decide `[D2 sheets 18, 19]`:
   - Windows lets a pipe server impersonate its client only after data
     has been read from the pipe, and then takes the security context of
     the last message read `[P2 Remarks: CONFIRMED]`
     `[P3 1368: CONFIRMED]`. So on Windows the daemon reads exactly the 4
     bytes of `Hello`'s length prefix before it knows the caller, and
     nothing more.
   - The connection is in **pre-attestation** (5.6.2). It holds one of
     the handshake budget's slots (5.2, point 8) and runs under the
     handshake deadline. Its 4-byte buffer is the only byte cap in this
     state, and the connection's admission allowance (5.2.1, rule 1) is
     already reserved.
   - The read of the prefix and the capture of the caller are one
     transition of the pipe driver, on one instance and one connection
     generation (ADR 0005, sheet 18). A read that completes with fewer
     than 4 bytes is continued, inside the same transition, for exactly
     the missing bytes. A read of 0 bytes, an end of stream, a broken
     pipe or any other failure ends the transition with a failure. When
     the handshake deadline passes with a read outstanding, the driver
     cancels it and fails the transition only after the read's terminal
     completion.
   - The 4 bytes are not decoded, compared or answered in this state.
     When all 4 are read, the caller is captured at once (impersonate,
     open the thread token, revert; ADR 0005, sheet 19). A result is
     bound to the connection only while its generation is the
     instance's current one and the connection is still in
     pre-attestation; any other result is discarded and its buffer
     zeroed (5.5, point 7).
   - Only after the caller is captured are point 2's limits applied and
     the prefix checked as point 3 says.
2. **Admission.** The handshake budget, the per-caller budgets and the
   total of 5.2, point 8 are applied in that order, with the allowance of
   5.2.1, rule 1. Over the handshake budget, the `AwaitHello` per-caller
   budget or the total: the connection is closed without reading.
   Over the open-per-caller budget: the daemon still reads `Hello`
   (points 3 to 5) and replies `BUSY`, then closes, so the client can
   tell "busy" from a fault `[OWN]`.
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
   (point 2): `BUSY`.
6. Reply `HelloReply`. With `ACCEPTED` the connection is `Open` at the
   chosen version; with any other result the daemon closes after the
   reply is written or the write deadline passes.

**Each later frame (daemon)**

7. **Length:** refuse a value below 16 or not a multiple of 4
   (`BadFrameLength`), and a value above the largest cap of the frame
   classes a client may send on this endpoint (`FrameTooLarge`). Both are
   fatal. The class's own cap is
   checked once the kind is known (point 8).
8. **Envelope.** Decode the 16 bytes. Then:
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
   Every failure here is fatal (5.4, point 13).
9. **Catalogue.**
   - a. `REQUEST`: a number with no row of kind request or streaming
     request, or a row newer than the chosen version: `UnknownMessage`
     as its reply; the connection stays. A row the endpoint reserves
     and does not serve: `NotSupported`; the connection stays. Over the
     requests-in-flight budget: `TooManyRequests`; the connection stays.
   - b. `CANCEL`: if `request_id` names a request in flight on this
     connection, its handler is asked to stop; otherwise nothing is done
     and nothing is sent. A `CANCEL` is never counted against the
     in-flight budget and never refused for an unknown or late id.
   - c. `NOTICE`: a number with no row of kind notice, or a row newer
     than the chosen version: `UnknownMessage`, fatal, because a notice
     has no reply to carry it `[OWN]`.
10. **Authorise**, from the row and the connection's caller, by the
    endpoint's rule, before the body is decoded `[OWN]`, so a caller who
    may not send a message never has its body parsed past the envelope.
    A refusal of a `REQUEST` is its reply, with the endpoint's code; a
    refusal of a `NOTICE` is fatal.
11. **Decode the body** with the row's generated decoder: xdr.md 5.4 in
    full; each `version` field (5.1.7, point 2); the aggregate extension
    bound as it goes (5.2, point 5); no trailing bytes. A failure is fatal
    `[OWN]`: a body that does not decode means the client is broken or
    hostile, and its later frames cannot be trusted to be aligned.
12. **Walk every extension list** in the body, all of them to the end,
    before anything is decided `[ADR 0004 N41.4: Firm]`:
    - tags out of order or duplicated in a list: `ExtensionOrder`, fatal;
    - a known tag (5.1.7, point 5) with the wrong critical bit:
      `ExtensionFlagMismatch`, fatal;
    - an unknown critical tag: the whole request is refused with
      `UnknownCriticalExtension` before any state changes; its reply
      still names every unknown non-critical tag found. A `NOTICE` with
      one is dropped whole and counted, never applied in part;
    - an unknown non-critical tag: ignored, and named in the reply's
      `ignored` (5.1.5, point 2) `[ADR 0004 N41.9: Firm]`.
13. **Fatal errors.** After the handshake, a fatal error ends the
    connection. The daemon first queues, best effort from the reserved
    reply slot, one **closing notice**: a `REPLY` with `number` 0 and
    `request_id` 0 whose body is an `ErrorReplyBody` with the code. If the
    slot is in use, nothing is sent. Then it stops reading, cancels the
    requests in flight (5.6.3) and closes once the queue is written or
    the write deadline passes `[OWN]`. In the handshake, a failure closes
    as points 3 to 6 say.
14. Only now does the handler run (or the notice's consumer).

**The client**

15. The client applies points 3, 4, 7, 11 and 12 to what the daemon sends,
    with its own frame caps, and checks each envelope by kind and
    direction `[OWN]`:
    - `REPLY`: its `request_id` names a request this connection sent that
      has no reply yet, and its number is that request's; after it the id
      is closed. A `REPLY` with `number` 0 and `request_id` 0 is the
      closing notice: the client reports its code against every
      outstanding request and closes;
    - `PART`: as `REPLY`, the row is streaming, and its reply has not
      come;
    - `EVENT`: `request_id` 0, a row of kind event at the chosen version,
      and only in the state the endpoint specification allows it;
    - `REQUEST`, `CANCEL` or `NOTICE` from the daemon, or anything else:
      refused.
    Any refusal, and any decode error, fails the connection: the client
    closes it and reports every outstanding request as failed, never as
    done `[ADR 0004 C46.4: Working]`.
16. **The checked connection** `[ADR 0004 C43.1, C43.2: Firm]`. Before a
    client reads or sends any secret (5.5, point 6), on one connection
    handle:
    1. it connects;
    2. it runs the endpoint's daemon check through the operating system
       (for the control endpoint, the control endpoint specification's,
       after ADR 0004, C43, point 4);
    3. it sends `Hello` and reads `HelloReply` on the same handle. The
       reply is valid only if it decodes whole, its magic is right, its
       result is `ACCEPTED`, and `chosen` is within both the client's
       range and [`daemon_min`, `daemon_max`];
    4. it binds the reply's `instance_id` to this connection.
    These steps make one value, the checked connection. It is made per
    connection, never cached or carried to a reconnect, and is the only
    way to construct the encoder of a secret-bearing message (5.5,
    point 6). A failure at any step ends the attempt with no secret read.
17. A client that only reads non-secret state may skip step 2, as its
    endpoint specification allows; the order of steps 1, 3 and 4 is the
    same `[OWN]`.
18. **Tolerances** `[ADR 0004 N41.3, N41.6, N41.8: Firm]`:
    - an unknown non-critical extension in a reply, part or event: kept
      and shown as unknown;
    - an unknown critical extension in a reply: the whole reply is
      failed, "this daemon is newer; upgrade `afs`"; in a `PART`: the
      whole request's result is failed, none of its parts is shown or
      acted on, and the client still reads the remaining parts and the
      reply (the connection stays); in an event: an unknown event;
    - an unknown error code or contract value: shown as its number plus
      `text`;
    - an unknown value in an open field (`HelloResult` included):
      "unknown (n)"; an unknown `HelloResult` is a refusal.

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
   in any message, and never a message number the chosen version lacks
   `[ADR 0004 N41.5: Firm]`.
6. **Secret-bearing messages.** A catalogue row may mark its body
   secret-bearing (xdr.md 5.1, point 8) `[D3 §5.1.8]`. Its encoder takes
   the checked connection (5.4, point 16) as an argument, so it cannot be
   called on any other connection: the compiler enforces C43, point 1 on
   the client `[OWN]`. Its decoder follows xdr.md 5.1, point 8.
7. **Named zeroisation places** `[ADR 0004 O28.2, O28.3: Firm]`. Secret
   bytes pass through transport buffers before and after the typed
   value. Each place has a label, used in code at the place:

   | Label | Buffer | Zeroed |
   |---|---|---|
   | `ipc-rx-frame` | the daemon's receive buffer for one frame, on an endpoint whose catalogue has a secret-bearing client message. It cannot know a frame is secret before the envelope, so every such buffer is treated as secret | before reuse or release, on every path: decoded, refused at any step, truncated, timed out, cancelled, connection closed, daemon stopping |
   | `ipc-rx-pending` (Windows) | every read buffer of the pipe driver, the 4-byte prefix buffer included | before reuse or free, on every path (completed, cancelled, failed, discarded for a stale generation or a closed connection), and never before the operation's terminal completion (ADR 0005, sheet 18) |
   | `ipc-rx-discard` | the daemon's discard buffer (5.2.1, rule 2) | each chunk before the next is read, and at release |
   | `ipc-tx-daemon` (Windows) | a daemon write buffer of the pipe driver that holds a secret-bearing reply or event | as `ipc-rx-pending`, after terminal completion |
   | `ipc-tx-stage` | the client's encoder output for a secret-bearing message | after the last byte is written or the write fails, and on cancel or close |
   | `ipc-tx-pending` (Windows) | the client's overlapped write buffer, if it uses one | after terminal completion, on every path |

   Each buffer is sized once from the frame cap before it is filled and
   never grown by reallocation while it may hold secret bytes `[OWN]`,
   so no copy is left behind (xdr.md 5.1, point 8). The token-specific
   places (what `aklog` reads to build a token) are the control endpoint
   specification's.
8. **Secret-dependent comparisons:** none in the framework `[OWN]`. The
   instance id and the schema digest are public. An endpoint
   specification names any it has (ADR 0004, O28, point 1).

### 5.6 States and ordering

#### 5.6.1 One definition set, generated codecs

1. **The definition set** is every file `crates/ironafs-ipc/schema/*.x`,
   one directory with no subdirectories, holding every message of both
   endpoints `[ADR 0004 C44.1: Firm]`. Test definitions (5.3.3, point 8)
   live in `crates/ironafs-ipc/tests/schema/` and are not part of it
   `[OWN]`.
2. **The generator** is IronAFS's own code in `xtask` (coding standards
   rule 21): its own parser of the notation the generator's
   specification sets, and two emitters, Rust now and Swift when the
   first Swift consumer exists `[ADR 0004 C44.1: Firm]`.
3. **The generator refuses** `[OWN]`, besides everything xdr.md requires
   of a definition `[D3 §5.1, §5.2]`:
   - a byte that is not ASCII printable, tab, LF or a CR followed by LF;
     a lone CR; a file that does not end with LF;
   - a number, tag or code used twice in its registry, or one listed as
     reserved or retired (5.3.3, point 4), or one outside its owner's
     range (5.3.3, points 2, 3 and 5);
   - a body type without `version` first and `ext` last (5.1.7, point 1);
     an extension list in a handshake type;
   - a non-critical extension registered on a request type and not
     advisory (5.1.7, point 4);
   - a request row whose error arm is not `IpcError` (5.1.5, point 1);
   - a message whose largest size (5.2, point 6) is above its frame
     class's cap, or a frame class cap above `ipc.frame.ceiling`;
   - a variable item without a cap; a cap or memory budget the static
     proof of xdr.md 5.2, point 9 breaks.
   It computes T for each endpoint (5.2.1, rule 1) and emits it.
4. **What each definition declares** `[OWN]`, in the generator's
   notation: per catalogue row, the facts of 5.3.3, point 2; per
   extension tag, its number, the types it may appear in, its critical
   bit, its introducing version and, on a request type, whether it is
   advisory; per enumeration field, whether it is open; per body, whether
   it is secret-bearing; per variable item, its cap name and value; per
   message, its memory budget.
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
   of each endpoint as a static table; T per endpoint; and the schema
   digest as a constant `[OWN]`.
7. **The schema digest** `[OWN]`, as issue #62 records the owner's
   decision of 2026-11-20 ("SHA-256 over the canonical, LF-normalised
   definition bytes"):
   - normalise each file of the set by replacing every CR LF pair with
     LF (the generator has already refused every other CR, point 3);
   - order the files by file name, compared as bytes, ascending;
   - the canonical bytes are the ASCII text `ironafs-ipc-schema-1`, one
     LF, then for each file in order: the file name's length as an
     unsigned 32-bit big-endian integer, the file name's bytes, the
     normalised content's length as an unsigned 64-bit big-endian
     integer, and the normalised content;
   - the digest is SHA-256 over the canonical bytes, 32 bytes
     `[H1 §1, Figure 1: CONFIRMED]`, computed through `ironafs-crypto`
     (ADR 0004, C5, point 1; its first backend is issue #77).
   The lengths make the file boundaries unambiguous, the order makes the
   result independent of the directory listing, and the normalisation
   makes it independent of a checkout's line endings. Any change to the
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
| Accepted (Linux, macOS) | caller read; admission passes (5.4, point 2) | `AwaitHello` | no byte read before the caller is known |
| Accepted (Linux, macOS) | no caller, or admission fails | Closed | nothing read, nothing sent |
| Accepted (Windows) | a handshake slot and the allowance are free | `PreAttestation` | otherwise closed without reading. The state records the instance's connection generation |
| `PreAttestation` | the transition returns, for this generation, 4 prefix bytes and a captured caller; admission passes | `AwaitHello` | nothing decoded or answered in this state; the prefix is checked on leaving it (5.4, point 3) |
| `PreAttestation` | the transition fails, admission fails, or the handshake deadline passes | Closed | no reply; a failed revert ends the daemon process (ADR 0005, sheet 19) |
| `PreAttestation` | a transition result tagged with another generation | `PreAttestation` | discarded, buffer zeroed, bound to nothing |
| Closed (from `PreAttestation`) | a transition result of this generation | Closed | discarded, buffer zeroed, bound to nothing |
| `AwaitHello` | a valid `Hello`, version chosen | `Open` | `HelloReply` `ACCEPTED` queued first |
| `AwaitHello` | any refusal of 5.4, points 3 to 5, or a deadline | Closed | `HelloReply` with its result where 5.4 gives one |
| `Open` | `REQUEST` | `Open` | exactly one `REPLY`, always |
| `Open` | `CANCEL` | `Open` | 5.4, point 9b |
| `Open` | `NOTICE` | `Open` | consumed; no reply |
| `Open` | any fatal error (5.4, point 13) | Closing | the closing notice, best effort |
| `Open` | the peer closes | Closed | requests in flight are cancelled (5.6.3, rule 4) |
| `Open` | the daemon stops | Closing | no new frame read; queued frames written within the write deadline |
| Closing | queue written, or the write deadline | Closed | every reservation released once (5.2.1, rule 5) |

`PreAttestation` is left only through its own transition's result or a
close. On a close the driver still waits for the transition's terminal
completion before the instance is disconnected or reused (ADR 0005,
sheet 18).

#### 5.6.3 Ordering rules

1. **Replies may arrive in any order;** the request id pairs them
   `[OWN]`.
2. **Parts of one request** arrive in order, all before its reply. A
   streaming row's terminal reply says whether the stream was complete,
   cut by the parts budget, or cancelled; the endpoint specification
   gives each reply's field for this `[OWN]`.
3. **Cancel or result.** A request's handler result and a `CANCEL` meet
   at the request's one-shot result slot (ADR 0004, C39, point 3); the
   first to arrive wins `[OWN]`. A result already produced is reported
   unchanged, even if its `REPLY` is not yet queued, and the `CANCEL` does
   nothing. A `CANCEL` that arrives first makes the handler stop at its
   next cancellation point, and the reply is one of: `Cancelled`, with
   nothing applied; the ordinary success reply of a change already
   committed; or a stream's terminal reply reporting cancellation.
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

## 6. Arithmetic

| Computation | Bound | At overflow or excess |
|---|---|---|
| `length` against a frame cap | caps ≤ 2,097,152 | refused before allocation |
| framed size, `length` + 4 | `length` already ≤ the cap | `checked_add` all the same |
| body input, `length` − 16 | after `length` ≥ 16 is checked | `checked_sub`; failure is `Truncated` |
| every XDR length, count and offset | xdr.md section 6, A1 to A12 | as xdr.md says |
| running extension count + a list's count; running value bytes + a value's length | ≤ 64; ≤ 16,384 | `checked_add`; above the bound is `CapExceeded` |
| request id against the mark | u64 compare, no arithmetic | n/a |
| version choice: the previous minor | computed only when `minor` ≥ 1 | n/a: at `M.0` only `M.0` is accepted |
| in-flight, output-queue and terminal-capacity counters | each ≤ its budget | `checked_add` before a reservation; subtraction only of what was added; a failure is `Busy` or back-pressure, never a wrap |
| per-caller and total connection counters; requests in flight; parts sent | ≤ their budgets | compared before the increment; never wraps |
| digest record lengths: file name (u32), content (u64) | a name above 2³² − 1 bytes or content above 2⁶⁴ − 1 bytes cannot exist in the set | the generator refuses the set (`try_from`) |
| generator sizes: largest message size, T, budgets | caps < 2³² | 64-bit `checked_add` and `checked_mul`; overflow refuses the definition |

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
   | 10 | `UnknownMessage` | a number with no row, or newer than the chosen version | open for a `REQUEST`; fatal for a `NOTICE` | none changed |
   | 11 | `NotSupported` | a reserved row the endpoint does not serve | open | none changed |
   | 12 | `UnknownCriticalExtension` | N41, point 4 | open | none: checked before the handler |
   | 13 | `TooManyRequests` | the requests-in-flight budget | open | none changed |
   | 14 | `Busy` | a reservation failed (5.2.1) | open | none changed |
   | 15 | `Cancelled` | 5.6.3, rule 3 | open | nothing applied |
   | 16 | `ResultTooLarge` | a stream cut by the parts budget, where the row reports it as an error | open | reads only |
   | 17 | `Contract` | the handler returned a C46 error, in `contract` | open | per that error |
   | 18 | `Internal` | a fault in the daemon | open | logged with a correlation id; `text` holds only that id |

   Handshake refusals are `HelloResult` values, not codes. A closed
   connection with no reply (no caller, a wrong magic, a refused length,
   a pre-attestation failure) has no code on the wire; the daemon counts
   it.
2. **No input in errors.** An error carries its code, the specification's
   constants and its `text`; never a value read from the input, and from
   a secret-bearing message no offset or length (xdr.md section 7,
   point 2) `[D3 §7.2]`.
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
| Any local user | take every connection or slot | per-caller budgets before the total (5.2, point 8; 5.4, point 2); handshake and frame deadlines |
| Any local user on Windows | hold the pre-attestation slots, which cannot be counted per caller | the handshake budget and deadline bound the hold; the residual is a denial of service to new connections for at most one handshake deadline per slot, repeatable. It is accepted and stated: Windows gives no caller before the first read (5.4, point 1) |
| Any local user on Windows | have bytes acted on before the caller is known | only the 4-byte prefix is read, never decoded or answered, before the capture (5.4, point 1) |
| A local user on Windows | swap clients between the prefix read and the capture | one driver-owned transition per instance and generation; stale results discarded (5.4, point 1; 5.6.2; ADR 0005, sheet 18) |
| Any local user on Windows | exhaust the pipe's instances | a finite count of 254, never the unlimited value; per-caller budgets inside it (5.2, point 10) |
| A process that takes the endpoint while the daemon is down | receive secrets | no secret before a checked connection (5.4, point 16; 5.5, point 6); the endpoint's address permissions (endpoint specification) |
| A replayed or stale daemon identity | bind a secret to another run | the instance id is bound to the checked connection, per connection (5.4, point 16) |
| A malicious or replaced daemon | feed a client bad replies | the client decodes with the same caps, matches every reply to an outstanding request, and fails the connection on any mismatch (5.4, point 15) |
| A slow or stalled reader | hold the daemon's memory | output budget and write deadline; one frame of input at a time; the global budget (5.2.1) |
| Any local user | exhaust the budget so requests go unanswered | terminal capacity reserved before the handler; the reserved reply slot; the low-memory path reads at most the envelope (5.2.1) |
| Anyone reading freed memory | recover secret bytes from IPC buffers | the named zeroisation places on every path (5.5, point 7) |
| A newer peer | have half of a request applied | unknown critical extensions refuse it whole; non-critical ones may not change what is mutated (5.1.7, point 4; 5.4, point 12) |
| A privileged helper handed a connection opened by another user | act as the opener | documented: the caller is fixed at `connect` (5.6.3, rule 5); the endpoint specification says whether it accepts inherited connections |

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
| C44.4: only additions within a major; current and previous minor accepted | 5.3.2, points 2 to 4; 5.6.1, point 8 | `compat_previous_minor`; `version_choice`; `schema_additive_check` |
| C44.5: adapters match exactly | 5.3.2, point 5 | `adapter_exact_version` |
| C44.6: message sets specified before code | the generator reads only the definition set; endpoint specifications hold the sets | review; `ipc_gen_check` |
| C45.2: the adapter endpoint only for registered adapters | the adapter endpoint specification's registration; here, the role check and the exact version and digest (5.4, point 5; 5.3.2, point 5) | `hello_rejects` (role 1 on the adapter endpoint, role 2 on the control endpoint); `adapter_exact_version` |
| C45.1: the caller is the peer the OS reports | 5.4, point 1; 5.6.3, rule 5 | `caller_before_bytes`; `preauth_reads_four_bytes` (Windows gate) |
| C43.1, C43.2: no secret before the daemon is checked; instance id bound to the checked connection | 5.4, point 16; 5.5, point 6 | `unchecked_cannot_encode_secret` (compile-fail); `checked_connection_per_handle` |
| N41.1: versioned, bounded, tagged extension lists | 5.1.7; 5.2, points 3 and 5 | `generator_rejects` (missing `version` or `ext`); `ext_aggregate_bound` |
| N41.2 to N41.6: critical bit; unknown handling both ways | 5.1.7; 5.4, points 12 and 18 | `critical_ext_refused_whole`; `client_ext_tolerance`; `ext_recognition` |
| N41.7: the adapter contract uses the same shapes | 5.1.7 applies to every body of both endpoints; one generator (5.6.1, point 3) | `generator_rejects` (an adapter body without `version` or `ext`) |
| N41.8: unknown error cause as code plus text | 5.1.6, point 3; 5.4, point 18 | `unknown_error_passthrough` |
| N41.9: non-critical request extensions never change what is mutated; unknown ones named | 5.1.7, point 4; 5.1.5, point 2 | `generator_rejects` (non-advisory request tag); `noncritical_ext_named`; each endpoint's mutation test |
| C5.5 as amended: peer credentials and the Windows pipe only through `ironafs-os` | 5.1.1; 5.4, point 1; ADR 0005's sheets | the `ironafs-os` inventory check of ADR 0005; `ironafs-ipc` stays pure (`check-pure`) |
| C39.2, C39.3: bounded executor, one-shot result slots, queues bounded in bytes | 5.2, point 8; 5.2.1; 5.6.3, rule 3 | `stalled_reader`; `global_budget_exhausted`; `cancel_cases` |
| O28.2, O28.3: zeroisation at named places | 5.5, point 7 | `secret_buffers_wiped` |

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

### 10.3 Reject corpus

Each input, its result and the section it breaks:

- daemon, handshake: length 0, 2, 56 (below 60), 96 (above 92); a wrong
  magic; `role` 0, 2 on the control endpoint, 7; `min` above `max`; a
  range with no accepted version; on the adapter endpoint, a range of two
  versions and a wrong digest; a 33-byte `client_name`; non-zero padding
  in `client_name`; trailing bytes; a second `Hello`; a `REQUEST` before
  `Hello` (5.4, points 3 to 5);
- daemon, after the handshake: `length` 4, 8, 12 (below an envelope),
  18 (not a multiple of 4), 65,540 (above the request cap); kind 0, 2, 3,
  4, 7; a `REQUEST` with number 0, with id 0, with an id equal to the
  last; a `CANCEL` with number 1, with id 0, with a body; a `NOTICE` with
  an id; a body `version` 0 and 2; tags 5 then 3, and 4 twice, in one
  list; 17 extensions in one list; 65 extensions over nested lists;
  16,385 value bytes over several lists; a 1,025-byte value; a known tag
  with its critical bit flipped; an array count of 0xFFFFFFFF; trailing
  bytes after a body (5.4, points 7 to 12);
- client: a `REPLY` for an id never sent, a second `REPLY` for one id, a
  `PART` for a non-streaming row, a `REPLY` whose number differs from
  its request's, an `EVENT` the endpoint does not allow in this state, a
  `REQUEST` from the daemon, a reply above the aggregate bound, a
  `HelloReply` whose `chosen` lies outside the client's range (5.4,
  points 15 and 16);
- encoder: a listed value in an open field's unknown form; a request id
  0; a message one byte over its cap (5.5, point 2);
- generator: one bad definition per refusal of 5.6.1, point 3.

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
  authorisation or for a request with an unknown critical extension; the
  live bytes the budget counts never above the in-flight ceiling, and
  each allocation inside a reservation; the output queue above its cap
  only by funded terminal replies (5.2.1, rule 4).
- `ironafs-ipc_session_client`: the same from the daemon's side into the
  client; no outstanding request reported done after a failed
  connection.
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
- `caller_before_bytes` (Linux, macOS): with a stand-in transport that
  records the order of calls, the caller is read before any read on the
  connection, and a connection with no caller is closed with zero bytes
  read.
- `checked_connection_per_handle`: the checked connection cannot be made
  from a `HelloReply` read on another handle, nor reused after a
  reconnect; a reconnect to a planted endpoint that passes the operating
  system's check but sends another instance id gets nothing secret.
- `unchecked_cannot_encode_secret`: a compile-fail test; a secret-bearing
  encoder called without a checked connection does not build.
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
  client reports the code against every outstanding request.
- `stalled_reader`, `global_budget_exhausted`, `slow_peer`,
  `caps_per_caller`: the budgets of 5.2, point 8 and the reservation
  rules of 5.2.1, with allocator and counter checks.
- `config_bounds`: each policy budget at 0, at its ceiling, and one
  above; each rule of 5.2, point 9 broken; refused whole at start and on
  reload.
- `secret_buffers_wiped`: each label of 5.5, point 7 on each path it
  names, with a test hook that inspects the buffer at release.
- `ipc_gen_check`, `schema_digest_fixture`: `cargo xtask ipc-gen
  --check`; the digest of the checked-in set equals a value recorded by
  an independent procedure outside the Cargo build, over the canonical
  bytes of 5.6.1, point 7.
- `schema_additive_check`: from the first freeze.
- **Windows gate** (with the Windows layer, on each supported floor;
  gaps until then):
  - `preauth_reads_four_bytes`: impersonation before any read fails with
    1368; after the 4-byte prefix it succeeds, and nothing beyond the
    prefix has been read;
  - short prefix completions: 1 then 3 bytes (attested after all 4);
    2 bytes then a close, 2 bytes then a stall past the deadline, a
    0-byte completion (each a failed transition, closed with no reply,
    the outstanding read's buffer released only after its terminal
    completion);
  - a disconnect, and a reconnect by another client, between the prefix
    read and the capture: the first identity is never bound to the second
    client;
  - `pipe_pool`: 254 instances in use; a configuration whose listeners
    plus total connections pass 254 is refused; the 255th connect finds
    the pipe busy.

## 11. Revisit if

- ADR 0004, C5 (Working): "a mainstream crate offers rxkad's cipher, a
  FIPS mode is scheduled (O31), or an operating system ships a safe
  interface that removes a site from the `ironafs-os` list." A removed
  peer-credential or pipe site changes 5.1.1 and 5.4, point 1.
- ADR 0004, C39 (Working): "M2 load tests show the fixed set starves a
  class of work, or lock-order checks find an RPC wait under a lock that
  cannot be restructured."
- This specification's own:
  - the Windows gate shows that reading only the 4-byte prefix is not
    enough to impersonate on a supported release, or that some release
    lets a pipe server identify its client before any read (5.4,
    point 1);
  - a measured control reply comes near the 1 MiB cap, or an adapter
    frame needs more than the 2 MiB ceiling (5.2, point 2);
  - a multi-session Windows host needs more than 254 connections
    (5.2, point 10);
  - a second language consumer whose needs the XDR profile cannot
    express;
  - a platform where a client cannot read the endpoint owner's
    credentials, which would need another daemon check and so an
    amendment of ADR 0004, C43, point 4.

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| X1 | SPECIFICATION | RFC 4506, register row E1, through the XDR profile (D3) | 5.1 |
| H1 | SPECIFICATION | FIPS 180-4, Secure Hash Standard, August 2015, register row E8: section 1, Figure 1 (SHA-256's message digest is 256 bits) | 5.6.1 |
| P1 | DOCUMENTATION | Microsoft Learn, `CreateNamedPipeW`, register row E3: parameters `dwPipeMode`, `nMaxInstances`; Remarks | 5.1.1, 5.2 |
| P2 | DOCUMENTATION | Microsoft Learn, `ImpersonateNamedPipeClient`, register row E4: Remarks | 5.4 |
| P3 | DOCUMENTATION | Microsoft Learn, System Error Codes 1300 to 1699, register row E5: error 1368 | 5.4 |
| P4 | DOCUMENTATION | Linux man-pages 6.19, unix(7), register row E6: socket option `SO_PEERCRED` | 5.1.1, 5.6.3 |
| P5 | DOCUMENTATION | unix(4), macOS 26.7, register row E7: `LOCAL_PEERCRED` and the supported socket types | 5.1.1, 5.6.3 |
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; C4, C5, C39, C41 to C46 ([part 1](../adr/0004-architecture-1-core.md)); L35 ([part 2](../adr/0004-architecture-2-os-layers.md)); O16, O23, O28, O47 ([part 3](../adr/0004-architecture-3-operations.md)); N41 ([part 4](../adr/0004-architecture-4-negotiation.md)); S1 | all |
| D2 | DECISION | [ADR 0005](../adr/0005-ironafs-os.md), accepted 2026-11-21: sheets 13 and 16 to 20; Consequences (the amendment of C5, point 5) | 1, 5.1.1, 5.2, 5.4, 5.5, 5.6.2 |
| D3 | DECISION | [The XDR profile](xdr.md) | 5.1, 5.2, 5.4, 5.5, 5.6.1, 6, 7 |
| D4 | DECISION | [Coding standards](../coding-standards.md), rules 1 to 3, 8, 9, 15, 20 and 21; the `wire-format` skill | 5, 6, 7, 10 |
| D5 | DECISION | Issue #62: the owner's decisions of 2026-11-20 recorded there (the split, the two kinds of limit, the digest, the Windows pool and prefix rule) | 5.2, 5.6.1 |
| O | OWN | Every point tagged `[OWN]`, with its reason in place | 5, 6, 7 |

No other implementation's source, test data or interface definitions
were read for this specification, and none are cited. The outside
documents are cited for facts only, in IronAFS's own words, and none is
quoted (register rows E3 to E8, "Licence status"). They are platform
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

## 14. Review

Not yet reviewed. The record will be in `docs/reviews/`.
