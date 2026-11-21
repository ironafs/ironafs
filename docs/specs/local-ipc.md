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

ADR 0004, C45, points 1 to 3 (**Firm**), the sentences this framework
serves:

> 1. **The control endpoint** is open to any local user. The caller is the
>    peer the operating system reports. […]
> 2. **The adapter endpoint** is open only to adapter processes the daemon
>    itself registered. […]
> 3. A binding is made only from a **daemon-owned registration.** […] A
>    binding without one is refused. […]

ADR 0004, C16, point 6 (**Firm**), the sentence that binds the adapter
endpoint's stream, which this framework carries as `EVENT` frames:

> - **Back-pressure:** the stream is bounded (C39, point 3). When it is
>   full, replies wait within their deadlines. An epoch advance is never
>   dropped: space for it is kept free.

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
and those references resolve to this document: 5.2, point 10 (the
instance count); 5.4, point 1 (the prefix rule); and 5.5, point 7 (the
zeroisation places).

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
   the row's **decode charge**: its request body's memory budget in
   bytes, and the largest number of heap requests one decode of it can
   make (xdr.md 5.2, point 9.5), the typed values' included. 5.2.1,
   rule 2 reserves both before the body is decoded.
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
     in-flight bytes, all connections, so that no one caller can leave
     the others with less than half. Among several callers the rest is
     first come, first served: no share is promised to each;
   - requests in flight per caller key ≤ executor queue ÷ 4, so one
     caller can fill at most a quarter of the queue;
   - each streaming budget ≤ its row constants (point 11);
   - closing deadline ≤ write deadline;
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
      every instance `[OWN]`. Below, P is the constant the wrapper
      actually passes for the endpoint, 254 or the endpoint's lower one.
    - **One count of instances** `[OWN]`. Every instance is in exactly
      one of these states, and counts against P from its creation until
      its handle is closed:

      | Instance state | Counts as a listener | Counts as a connection (point 8) | Operations that may be outstanding |
      |---|---|---|---|
      | Listening | yes | no | one connect wait |
      | Refused: its connect completed and admission's first step failed | yes | no | none; it is disconnected at once |
      | Idle: released, or refused and disconnected, with no connect wait yet | yes | no | none |
      | `PreAttestation` | no | yes, and a handshake slot | one read, of at most 4 bytes |
      | `AwaitHello`, `AwaitBinding` | no | yes, a handshake slot and a per-caller one | one read, one write |
      | `Open`; `Closing` after `Open` | no | yes; no handshake slot | one read, one write |
      | `Closing` never `Open` | no | yes; the handshake slots it held | one read, one write |
      | `Draining` | no | yes; and the handshake slots it held when it entered | those started before, each awaiting its terminal completion |

      **Handoffs** `[OWN]`. When a listener's connect wait completes,
      one step under the admission lock either makes the instance a
      connection (it leaves the listener count and enters the
      connection count) or leaves it a listener, Refused. The daemon
      keeps the configured number of listeners: when the listener count
      is below it, it gives an Idle instance a connect wait, or creates
      an instance if none is Idle. A connection's **handshake slots**,
      global and per caller, are freed at the first of: the step that
      makes it `Open`, or its release. So an `Open` connection never
      holds one, and a connection refused or timed out in the handshake
      holds its slots through `Closing` and `Draining`.

      An instance is **released**, and only then leaves every count but
      P's, when every operation started on it has reached its terminal
      completion and it has been disconnected (ADR 0005, sheet 18). A
      released instance is Idle until it is given a new connect wait or
      its handle is closed.
    - **Configuration is refused unless** listening instances +
      connections in all ≤ P `[OWN]`. Connections in handshake are
      inside connections in all (point 9), and `Draining` instances stay
      in it until release, so every instance the daemon can hold is in
      the sum, and a listener that became a connection can always be
      replaced. When the connection count is at its budget the daemon
      still listens; a client that connects then is disconnected without
      a read (5.4, point 2).
    - **Pipe buffers.** `ipc.pipe.buffer_in` = `ipc.pipe.buffer_out` =
      4,096 bytes, constants in the wrapper, the same on every instance
      (ADR 0005, sheet 17) `[OWN]`: the daemon's own buffers hold
      frames, so the pipe needs no more than one page each way. The
      system holds this storage, outside the in-flight budgets; its
      nominal bound is P × 8,192 bytes (2,080,768 at 254). How Windows
      rounds or reserves the two sizes is UNVERIFIED until the Windows
      gate records it; nothing here is coded from it.
    - **Outstanding operations** are at most the last column's: one
      connect wait on a listener, one read and one write on a connected
      instance, so at most 2 × P in all `[D2 sheet 18]`. Each operation's
      block is inside the admission allowance (5.2.1, rule 1); its
      buffer is one this connection has already reserved (the prefix,
      envelope, frame or discard buffer for a read; a queued frame for a
      write), at most one frame. So outstanding operations are bounded
      in count and in bytes by the budgets of point 8. A block whose
      completion never arrives stays counted, with its instance in
      `Draining` (5.6.2).

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
     more from the connection, so a client is refused at most as fast
     as it reads its refusals. The next frame therefore starts at the
     right byte on every path.
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
     valid `REQUEST` answered `Busy`; a valid `NOTICE` dropped or fatal
     as its row declares; and rules 3 and 4's `Busy`, stream end and
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
   - **A mandatory `EVENT` is never dropped** `[ADR 0004 C16.6: Firm]`
     ("An epoch advance is never dropped: space for it is kept free");
     the mechanism is `[OWN]`. Every event row declares its delivery
     class, droppable or mandatory. A mandatory row also declares a
     coalescing key and how many events per key may be pending at once,
     so that the most a connection can owe is a constant, M bytes, which
     the generator computes for the endpoint from those bounds, queue
     entries included. Every connection of that endpoint reserves M in
     its allowance (rule 1), in both accounts, at admission: the space
     is kept free from then on, outside the output budget, and nothing
     else may use it. A mandatory event is appended, under the output
     lock, after the frames already queued, so it keeps its place in
     the one ordered stream; neither the output budget, nor either
     account, nor a paused reader can refuse it. If the endpoint ever
     asks for more than its declared bound (a fault), the connection is
     closed at once (5.4, point 13), never the event dropped: for the
     adapter endpoint that tears the binding down, which C16, point 6
     counts as completing the revocation. Until the event is written or
     the connection is closed, the fencing of that binding is the
     daemon's, as C16, point 6 and the adapter endpoint specification
     (ADR 0004, S1) say; this framework guarantees only the space and
     the order.
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
     one its coalescing key and bound (5.2.1, rule 4); and each body
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
   exception is Windows `[OWN]`, applied as ADR 0005, sheets 18 and 19
   decide `[D2 sheets 18, 19]`:
   - Windows lets a pipe server impersonate its client only after data
     has been read from the pipe, and then takes the security context of
     the last message read `[P2 Remarks: CONFIRMED]`
     `[P3 1368: CONFIRMED]`. So on Windows the daemon reads exactly the 4
     bytes of `Hello`'s length prefix before it knows the caller, and
     nothing more. The documentation establishes that a read must come
     first; it does not say how many bytes that read must return. Four
     bytes is ADR 0005, sheet 18's rule, the whole prefix and nothing
     else `[D2 sheet 18]`. That a 4-byte read is enough on every
     supported release, and whether a shorter one would be, is
     UNVERIFIED until the Windows gate runs it (10.5).
   - The connection is in **pre-attestation** (5.6.2). Before the read
     is started it has passed point 2's first step: it holds a place in
     the connection count and one of the handshake budget's slots (5.2,
     point 8), and its admission allowance (5.2.1, rule 1) is reserved.
     It runs under the handshake deadline. Its 4-byte buffer is the only
     byte cap in this state.
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
   - Only after the caller is captured is point 2's second step applied
     and the prefix checked as point 3 says.
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
     lock: if the connection is still `AwaitBinding`, the handler's
     `REPLY` is appended, the state becomes `Open` and the handshake
     slots are freed, in that order, and only then is any other frame
     admitted or any `EVENT` queued. If the handshake deadline or a
     fatal error entered `Closing` first, it has won: the result is
     suppressed (point 13), the connection never opens, and the
     endpoint specification says what becomes of a capability spent on
     it. A
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
      MiB. A part is staged as its frame, not as a decoded value. Before
      a part's body is read the client reserves from that budget the
      body's bytes plus `ipc.overhead.frame` for its entry. Each part is
      decoded once to make its checks, under a decode charge reserved
      from the same budget for the length of that decode, and the value
      is dropped; it is decoded again only when shown. Staging is zeroed
      on release if the row is secret-bearing. Two kinds of failure are
      kept apart. A frame that breaks point 15 (framing, envelope, a
      body that does not decode, `ExtensionOrder`,
      `ExtensionFlagMismatch`) **fails the connection**, as point 15
      says. A **request-level failure** leaves the connection open and
      fails only the request: a part that would pass a limit, a failed
      staging reservation, an unknown critical extension in a part
      (point 18), or the stream deadline. On the first one, once, the
      client releases what it staged, marks the request failed, queues
      one `CANCEL`, and starts the grace period
      `ipc.client.cancel_grace` (10 s) at that moment, whether or not
      the `CANCEL` can be written. From then on it reads that request's
      remaining frames by envelope: each envelope is checked as point 15
      says, each body is discarded undecoded through the discard buffer,
      and the terminal `REPLY`'s envelope closes the request id. If the
      grace period ends, or a further `parts_max` parts are discarded,
      before that `REPLY`, the client fails the connection. A daemon
      within this specification never triggers either: it enforces the
      same row limits.
20. **Every request, at the client,** has a request deadline the caller
    of the client library sets, at most 3,600 s `[OWN]`. When it passes
    without a `REPLY` the client reports the request as failed with an
    unknown outcome (5.6.3, rule 4), queues one `CANCEL`, and fails the
    connection if the `REPLY` has not come within
    `ipc.client.cancel_grace` of the deadline, whether or not the
    `CANCEL` could be written.

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
   | `ipc-rx-pending` (Windows) | daemon | every read buffer of the pipe driver, the 4-byte prefix buffer included | before reuse or free, on every path (completed, cancelled, failed, discarded for a stale generation or a closed connection), and never before the operation's terminal completion (ADR 0005, sheet 18) |
   | `ipc-rx-discard` | daemon | the discard buffer (5.2.1, rule 2) | each chunk before the next is read, and at release |
   | `ipc-tx-daemon-stage` | daemon | the encoder output for a secret-bearing reply, part or event, while it is built and while it is queued | after its last byte is written; and on every path that ends it unwritten or partly written: an encode error, a suppressed reply (5.4, point 13), a dropped event, a failed or timed-out write, the closing deadline, connection closed, daemon stopping |
   | `ipc-tx-daemon` (Windows) | daemon | the same storage as `ipc-tx-daemon-stage`, from the moment a write is started on it until that write's terminal completion; the pipe driver owns it then | only at terminal completion, on every path. A timeout, close or stop in the row above requests the write's cancellation and waits; it never zeroes or frees a buffer the operating system still holds |
   | `ipc-tx-stage` | client | the checked connection's staging buffer for a secret-bearing message | after the last byte is written or the write fails, on an encode error, and on cancel or close |
   | `ipc-tx-pending` (Windows) | client | the client's overlapped write buffer, if it uses one | after terminal completion, on every path |
   | `ipc-rx-client` | client | the receive buffer for one frame, on an endpoint with a secret-bearing daemon message | before reuse or release, on every path: decoded, refused, truncated, timed out, connection failed or closed; and not before every value that borrows from it is dropped |
   | `ipc-rx-client-stage` | client | the staging storage of a stream whose row is secret-bearing (5.4, point 19) | at release, on every path: shown, failed, over a limit, cancelled, deadline, connection failed |
   | `ipc-rx-client-discard` | client | the client's discard buffer (5.4, point 19) | each chunk before the next is read, and at release |
   | `ipc-rx-client-pending` (Windows) | client | the client's overlapped read buffer, if it uses one | after terminal completion, on every path |

   **One owner at a time** `[OWN]`, after ADR 0005, sheet 18. A buffer
   belongs to its connection until an overlapped read or write is
   started on it, then to the transport until that operation's terminal
   completion, then to the connection again; its reservation goes with
   it and is never doubled. While the transport owns a buffer, every
   trigger in the table means: request cancellation, wait for the
   terminal completion, then zero with `ironafs-os`'s wipe (ADR 0005,
   sheet 24) and release. This holds for the client's Windows rows as
   for the daemon's.

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
     class, or a mandatory one without its coalescing key and bound
     (5.2.1, rule 4);
   - a request row whose error arm is not `IpcError` (5.1.5, point 1);
   - a message whose largest size (5.2, point 6) is above its frame
     class's cap, or a frame class cap above `ipc.frame.ceiling`;
   - a variable item without a cap; a cap or memory budget the static
     proof of xdr.md 5.2, point 9 breaks.
   It computes T and R for each endpoint (5.2.1, rules 1 and 2) and each
   row's decode charge (5.2, point 7), and emits them.
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
   of each endpoint as a static table; T and R per endpoint; each row's
   decode charge; and the schema digest as a constant `[OWN]`. The
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
| Accepted (Windows) | admission's first step passes (5.4, point 2) | `PreAttestation` | otherwise disconnected without a read. The state records the instance's connection generation |
| `PreAttestation` | the transition returns, for this generation, 4 prefix bytes and a captured caller; admission's second step passes | `AwaitHello` | nothing decoded or answered in this state; the prefix is checked on leaving it (5.4, point 3) |
| `PreAttestation` | the transition fails, or admission's second step fails | Closed | no reply; nothing more read; a failed revert ends the daemon process (ADR 0005, sheet 19) |
| `PreAttestation` | the handshake deadline passes with the read outstanding | `Draining` | the driver requests the read's cancellation; no reply |
| `PreAttestation` | a transition result tagged with another generation | `PreAttestation` | discarded, buffer zeroed, bound to nothing |
| `Draining`, Closed (from `PreAttestation`) | a transition result of this generation | unchanged | discarded, buffer zeroed, bound to nothing |
| `AwaitHello` | a valid `Hello`, version chosen; the endpoint has no binding row | `Open` | `HelloReply` `ACCEPTED` queued first |
| `AwaitHello` | a valid `Hello`, version chosen; the endpoint has a binding row | `AwaitBinding` | `HelloReply` `ACCEPTED` queued first; still a handshake connection, under the handshake deadline |
| `AwaitHello` | any refusal of 5.4, points 3 to 5, or a deadline | Closing | `HelloReply` with its result where 5.4 gives one |
| `AwaitBinding` | a binding row's handler reports the endpoint's checks passed | `Open` | its `REPLY` queued first; no `EVENT` or `PART` before this |
| `AwaitBinding` | a failed binding, any other frame (`NotBound`), any fatal error, or the handshake deadline | Closing | the binding's error reply or the closing notice, best effort |
| `Open` | `REQUEST` | `Open` | exactly one `REPLY`, unless the connection closes first (5.4, point 13) |
| `Open` | `CANCEL` | `Open` | 5.4, point 9b |
| `Open` | `NOTICE` | `Open` | consumed; no reply |
| `Open` | any fatal error (5.4, point 13) | Closing | the closing notice, best effort, last |
| `Open`, `AwaitBinding` | the peer closes | Closed | requests in flight are cancelled (5.6.3, rule 3); what each had committed stays whole (5.6.3, rule 4) |
| `Open`, `AwaitBinding` | the daemon stops | Closing | no new frame read; no notice |
| Closing | queue and slot written, or the write deadline, or the closing deadline | Closed | nothing appended after entering Closing; later results suppressed (5.4, point 13) |
| Closed | every transport operation has reached its terminal completion (Windows); at once (Linux, macOS) | released | the allowance and the connection's places in the counts are released once (5.2.1, rule 5) |
| Closed (Windows) | an operation is still outstanding | `Draining` | the instance is not disconnected, reused or closed (ADR 0005, sheet 18) |
| `Draining` (Windows) | the last terminal completion | released | buffers zeroed, then the instance is disconnected and may listen again |

1. `PreAttestation` is left only through its own transition's result or
   a close. On a close the driver still waits for the transition's
   terminal completion before the instance is disconnected or reused
   (ADR 0005, sheet 18).
2. **Release** is the one event that frees a connection's allowance, its
   place in the connection count, and any handshake slot it still holds
   (one it did not free by becoming `Open`). On Linux and macOS it
   follows the close at once. On Windows it waits for every operation's
   terminal completion, however long that takes `[D2 sheet 18]`.
3. **`Draining` has no deadline that frees anything** `[OWN]`, after
   ADR 0005, sheet 18: a block whose completion never arrives stays
   counted. Cancellation is requested once, when the state is entered.
   The cited documentation gives no bound on how long a cancelled pipe
   read or write takes to complete: UNVERIFIED, measured at the Windows
   gate (10.5). An instance still in `Draining` after one closing
   deadline is counted as **stuck** and reported in an event and in the
   daemon's status; it stays counted in the pool, in the connection
   count and in the handshake slots it held. The daemon does not close
   its handle, start anything on it, or restart itself because of it:
   a restart would drop every user's tokens (ADR 0004, C42) to recover
   one slot. Stuck instances are freed by their completion or by the
   end of the process. If they fill a budget, new connections are
   refused at admission and existing ones are unaffected; section 8
   states this as part of the Windows residual.
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
   point 3); the first to arrive wins `[OWN]`. A result already produced
   is reported unchanged, even if its `REPLY` is not yet queued, and a
   later `CANCEL` or deadline does nothing. A `CANCEL` or a deadline
   that arrives first makes the handler stop at its next cancellation
   point, and the reply is `Cancelled` or `DeadlineExceeded`, with
   nothing applied, or a stream's terminal reply reporting the cut. A
   mutating handler that claimed the slot first (5.6.4, rule 4) is not
   stopped: its own result, the ordinary success reply included, is the
   reply. A close
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
     waits at its envelope, body unread, until reading resumes. So a
     stream that fills the queue can always be cancelled, and at most
     one envelope is read ahead.
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
   - **The commit claim.** The slot is pending, claimed or completed. A
     mutating handler makes one atomic claim on the slot, pending to
     claimed, immediately before its first change. If a `CANCEL`, the
     deadline or a close completed the slot first, the claim fails and
     the handler changes nothing; so `Cancelled` and `DeadlineExceeded`
     written by those paths always mean nothing was applied. Once the
     claim succeeds, a `CANCEL` does nothing and only the handler
     completes the slot, with its own result, the ordinary success
     reply included.
   - After a claim the deadline completes the slot only for a row its
     endpoint specification names as one whose outcome can be unknown
     (5.6.3, rule 4), with `DeadlineExceeded` saying so; for every
     other row the work after the claim waits on no peer and no server,
     which that specification must show.
   - A close after a claim suppresses the reply and leaves the change
     to complete (5.4, point 13). The worker is given the
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
| `length` against a frame cap | caps ≤ 2,097,152 | refused before allocation |
| framed size, `length` + 4 | `length` already ≤ the cap | `checked_add` all the same |
| body input, `length` − 16 | after `length` ≥ 16 is checked (5.4, point 7) | `checked_sub` all the same; a failure is `BadFrameLength` |
| every XDR length, count and offset | xdr.md section 6, A1 to A12 | as xdr.md says |
| running extension count + a list's count; running value bytes + a value's length | ≤ 64; ≤ 16,384 | `checked_add`; above the bound is `CapExceeded` |
| request id against the mark | u64 compare, no arithmetic | n/a |
| version choice: the previous minor | computed only when `minor` ≥ 1 | n/a: at `M.0` only `M.0` is accepted |
| in-flight counters (global and per caller key), heap-request, output-queue and terminal-capacity counters | each ≤ its budget | `checked_add` before a reservation, in both accounts or neither; subtraction only of what was added; a failure is a failed reservation or back-pressure, never a wrap |
| queue occupancy + a frame's whole charge (framed size + `ipc.overhead.frame`) | ≤ the output budget for a `PART` or `EVENT` | `checked_add`; above the budget the frame is not built |
| the allowance A and the largest request reservation R, from configured values | budgets ≤ their ceilings | 64-bit `checked_add` and `checked_mul` at validation; overflow refuses the configuration |
| arrival + the request deadline | deadline ≤ 3,600 s | the checked addition of `ironafs-time`'s deadline type; overflow refuses the request as `Internal` |
| per-caller and total connection counters; requests in flight per connection and per caller key; executor queue entries; parts and part bytes sent | ≤ their budgets | compared before the increment; never wraps |
| client: parts staged + 1; part bytes staged + a part's `length`; parts discarded + 1 | ≤ `parts_max`; ≤ `stream_bytes_max`; ≤ `parts_max` | `checked_add` before the part's body is read; above the bound fails the request (5.4, point 19) |
| client: the highest common version; the daemon range's shape | compares of (major, minor) pairs | `daemon_max.minor` − 1 is computed only when `daemon_max.minor` ≥ 1 |
| digest record lengths: file name (u32), content (u64) | a name is at most 64 bytes (5.6.1, point 1); content above 2⁶⁴ − 1 bytes cannot exist in the set | the generator refuses the set (`try_from`) |
| generator sizes: largest message size, T, R, decode charges, budgets | caps < 2³² | 64-bit `checked_add` and `checked_mul`; overflow refuses the definition |

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
   | 11 | `NotSupported` | a reserved row the endpoint does not serve | open | none changed |
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
| Any local user | take every connection or slot | the per-caller handshake and open-connection budgets on every operating system (5.2, point 8; 5.4, point 2); handshake and frame deadlines |
| Any local user | take the shared memory, the readers or the executor while inside its connection limits | the per-caller byte account and request budget (5.2.1; 5.2, point 8); one turn per caller per round, whatever the frames are (5.6.4, rule 1); a per-caller share of the executor's queue, served round-robin, and at most half the workers, rounded up (5.6.4, rule 3); at most half the global byte budget (5.2, point 9); requests charged until their worker returns (5.6.4, rule 5). Residual: many distinct callers together can fill the executor; each then gets a retryable `Busy` |
| Any local user | keep work running by abandoning it | the request deadline from arrival; cancellation on close; slots and bytes held until the worker returns (5.6.4, rules 2, 4 and 5) |
| Any local user on Windows | hold the pre-attestation slots, which cannot be counted per caller | bounded, not prevented: the **Windows pre-attestation residual**, stated below the table |
| Any local user on Windows | have bytes acted on before the caller is known | only the 4-byte prefix is read, never decoded or answered, before the capture (5.4, point 1) |
| A local user on Windows | swap clients between the prefix read and the capture | one driver-owned transition per instance and generation; stale results discarded (5.4, point 1; 5.6.2; ADR 0005, sheet 18) |
| Any local user on Windows | exhaust the pipe's instances | a finite count, never the unlimited value; one count of every instance in every state, `Draining` included; the connection and handshake limits applied before the prefix read; per-caller budgets inside them (5.2, point 10; 5.4, point 2) |
| A process that takes the endpoint while the daemon is down | receive secrets | the operating system's daemon check, on every connection, before any credential is read (5.4, point 16); sending bound to the checked handle (5.5, point 6); the endpoint's address permissions (endpoint specification) |
| A process that replays the daemon's instance id | pass as the daemon | the id authenticates nothing; only the operating system's check does (5.4, point 16) |
| A peer that is not the daemon, or a faulty daemon | make a client use a lower version or another schema | the client recomputes the highest common version and checks the range's shape and, for adapters, the digest (5.4, point 16); a lower claimed range is stopped only by the daemon check |
| Code in a client that holds two connections | send bytes encoded for the checked one on another | encode and send are one operation of the checked connection; no encoded secret leaves it (5.5, point 6) |
| An attested but unregistered peer on the adapter endpoint | receive events or send requests | `AwaitBinding`: nothing but the binding exchange before the endpoint's registration and capability checks pass (5.4, point 6) |
| A malicious or replaced daemon | feed a client bad replies, or an endless stream | the client decodes with the same caps, matches every reply to an outstanding request, and fails the connection on any mismatch (5.4, point 15); streams are staged within the row's part and byte limits and a deadline, and shown only when whole (5.4, point 19); every request has a client deadline (5.4, point 20) |
| A slow or stalled reader | hold the daemon's memory | the output budget, checked against each whole frame; the write and closing deadlines; one frame of input at a time; both accounts (5.2.1) |
| Any local user | exhaust the budget so requests go unanswered, or desynchronise the stream with refused frames | terminal capacity reserved before the handler; the reserved reply slot; every length, envelope, catalogue and authority check before the first reservation, and a refused body discarded to its exact length, unvalidated (5.2.1, rule 2); the per-caller account |
| Anyone reading freed memory | recover secret bytes from IPC buffers | the named zeroisation places, both sides and both directions, on every path (5.5, point 7) |
| A newer peer | have half of a request applied | unknown critical extensions refuse it whole; non-critical ones may not change what is mutated (5.1.7, point 4; 5.4, point 12) |
| A privileged helper handed a connection opened by another user | act as the opener | documented: the caller is fixed at `connect` (5.6.3, rule 5); the endpoint specification says whether it accepts inherited connections |

**The Windows pre-attestation residual** `[OWN]`. Windows gives a pipe
server no caller before the first read (5.4, point 1), so a connection
in `PreAttestation` cannot be charged to anyone. Any local user can hold
those connections. What bounds it, each stated on its own:

1. **How many at once:** at most the handshake budget for all callers
   (16 by default, 64 at most), and never more than the connection
   count or the pool, because both limits are applied before the prefix
   read is started (5.4, point 2). Open connections are not affected.
2. **When the daemon gives up on one:** at the handshake deadline (5 s
   by default) it requests the read's cancellation. That is a bound on
   the request, not on the slot.
3. **How long the slot stays taken:** until the cancelled read's
   terminal completion, plus the disconnect (5.6.2, `Draining`). No
   cited document bounds that time: UNVERIFIED, measured at the Windows
   gate (10.5). It is expected to be short; this specification does not
   rely on that, and a completion that never arrives leaves the slot
   counted (5.6.2, point 3).
4. **How often:** without limit. A user can take each slot again as soon
   as it is released, so while the attack lasts new connections can be
   refused indefinitely. The effect is confined to new connections on
   that endpoint.
5. **After the capture** nothing of this remains: from the moment the
   caller is known the connection is one of that caller's handshake
   slots, on Windows as elsewhere (5.4, point 2).

The residual is the smallest this design allows, since one read must
come before the capture. **It contradicts ADR 0004, O23 as that
decision stands:** O23's row for a malicious local IPC client says of
what remains "None known", and O23, rule 3 puts every residual in the
administrator's guide. O23 is Firm and this specification does not
change it. Until the owner decides section 13, Q4, the two documents
disagree, and this one is the accurate statement for Windows. This
specification recommends Q4's option (a): an amendment of O23's row
that states the scope (new connections on one endpoint), the
repeatability, and a lifetime that depends on a completion whose
timing is unverified, carried into the administrator's guide. Q4
blocks marking the Windows parts `reviewed`.

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
| C39.2: ingress separate from execution; handoff without blocking; a full executor is a counted, retryable error | 5.6.4, rules 1 and 3; 5.4, point 14 | `ingress_never_blocks`; `executor_full_retryable`; `fair_turns` |
| C16.6: an epoch advance is never dropped; space for it is kept free | mandatory event rows: M reserved in every connection's allowance, appended in order, never refused (5.2.1, rules 1 and 4); the fence itself is the adapter endpoint specification's | `mandatory_event_reserved` |
| C39.3: one-shot result slots with a deadline from arrival and a cancel token; every queue bounded in bytes | 5.6.4, rules 2, 4 and 5; 5.6.3, rule 3; 5.2, point 8; 5.2.1 | `request_deadline`; `charged_until_worker_returns`; `cancel_cases`; `stalled_reader`; `output_admission`; `global_budget_exhausted`; `caps_per_caller` |
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
7. For every reservation sequence the seeded generator makes (admit,
   refuse, decode, reply, cancel, close, in any order), each account
   returns to zero when the last charge is released, the two accounts
   never disagree, and no caller's account passes its budget.

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
  inside a reservation; the output queue above its budget only by
  funded terminal replies and mandatory events; nothing queued after a
  closing notice
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
  fired is suppressed and never opens; a failed binding closes;
  the handshake deadline covers the wait; the connection counts as a
  handshake slot until it is `Open`.
- `admission_order`: with a stand-in transport that records every call,
  a connection over the connection count, over the handshake budget, or
  without room for its allowance is closed with zero reads, on each
  operating system's transport (on the Windows stand-in, with no prefix
  read started); one over its caller's handshake budget is closed with
  no read after the caller is known.
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
- `refusal_consumes_body`: 10.3's refused requests with bodies, on the
  ordinary path and with the in-flight budget exhausted: the same code
  each time (or `Busy` only for a request that was valid), the body
  discarded in zeroed chunks, the following request answered; a
  `NOTICE` with a bad envelope, an unknown number or no authority is
  fatal on both paths; a known, authorised one under exhaustion follows
  its row's overload rule whether its body is well formed or not, and
  the same body is fatal when memory is free.
- `output_admission`: a `PART` and a droppable `EVENT` that would take
  the queue one byte past its budget are not built (the part waits, the
  event is dropped and counted); the queue passes its budget only by
  funded terminal replies and mandatory events. Two producers racing
  for the last headroom: one fits, the other waits or is dropped. A
  terminal reply from terminal capacity needs no reservation with both
  accounts at their budgets, its queue entry included.
- `mandatory_event_reserved`: on a test endpoint with a mandatory event
  row, with the output queue at its budget, both accounts exhausted and
  the reader stalled, the mandatory event is queued in order behind the
  frames already there and is written first when the reader resumes;
  with reads paused and a stream filling the queue, a `CANCEL` is still
  handled (5.6.4, rule 1); an event past the row's declared bound
  closes the connection and is never dropped.
- `stalled_reader`, `global_budget_exhausted`, `slow_peer`,
  `caps_per_caller`: the budgets of 5.2, point 8 and the reservation
  rules of 5.2.1, with allocator and counter checks: one caller at its
  byte budget gets `Busy` while a second caller's requests still run;
  every allocation, the decoder's and `Hello`'s `client_name` included,
  is inside a reservation of both accounts; the decode charge is
  reduced to the ledgers' total only after the extension walk; heap
  requests stay within their budget; a caller's account record is gone
  after its last charge, and stays paid for while a detached worker
  keeps it after its connection's allowance was released; listener
  blocks and the capture scratch are inside a reservation before they
  are allocated; one caller at its byte budget leaves a second caller
  at least half the global budget.
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
  raced with `CANCEL` and with the deadline at every point: the reply
  is `Cancelled` or `DeadlineExceeded` exactly when nothing was
  applied, and the success reply exactly when the change was. A notice
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
- `ipc_gen_check`, `schema_digest_fixture`: `cargo xtask ipc-gen
  --check`; the digest of the checked-in set equals a value recorded by
  an independent procedure outside the Cargo build, over the canonical
  bytes of 5.6.1, point 7; `schema_digest_files`: 10.1's two-file
  vector, byte for byte, and each bad directory of 10.3.
- `schema_additive_check`: from the first freeze.
- **Windows gate** (with the Windows layer, on each supported floor;
  gaps until then):
  - `preauth_reads_four_bytes`: impersonation before any read fails with
    1368; after the 4-byte prefix it succeeds, and nothing beyond the
    prefix has been read. The run records, per release, whether a
    shorter read would also have sufficed (recorded, not relied on);
  - `preauth_per_caller`: one user opens connections that each send
    only the 4-byte prefix: after the capture that user holds at most
    its per-caller handshake budget, the excess is closed with no
    further read, and a second user still connects;
  - `cancel_completion_time`: the time from the cancellation request to
    the terminal completion of a cancelled prefix read, a cancelled
    frame read and a cancelled write, measured over many runs and
    recorded in 12.1; with a stand-in whose completion never arrives,
    the instance stays in `Draining`, is reported stuck after the
    closing deadline, stays in every count, and is never disconnected,
    reused or closed;
  - short prefix completions: 1 then 3 bytes (attested after all 4);
    2 bytes then a close, 2 bytes then a stall past the deadline, a
    0-byte completion (each a failed transition, closed with no reply,
    the outstanding read's buffer released only after its terminal
    completion);
  - a disconnect, and a reconnect by another client, between the prefix
    read and the capture: the first identity is never bound to the second
    client;
  - `pipe_pool`: P instances in use, in every state of 5.2, point 10's
    table, `Draining` included; with the pool full, a connect that has
    just completed and fails admission is disconnected with no read and
    its instance listens again; a handshake that was refused keeps its
    handshake slot through `Closing` and through a delayed terminal
    completion, and an `Open` connection holds none; a configuration
    whose listeners plus connections in all pass P is refused; with the
    connection count at its budget a new client is disconnected with no
    read; with every instance in use a connect finds the pipe busy; the
    buffer sizes the live pipe reports are recorded;
  - **the daemon check, ADR 0005, sheet 20** (a shipping gate for every
    secret-bearing message on Windows): the real daemon checked by an
    ordinary, unprivileged user; a pipe of the same name held by another
    user's process; the server exiting and being replaced between
    connect and check; and the residual interval before the process is
    opened, with the server's pid observed, independently of the check,
    to be held by another process: the check must answer "not the
    service" from a step that tells the connected pipe's server from
    the replacement, before any credential is read. Sheet 20 sets what
    counts as a pass. Until it passes, no secret-bearing message moves
    on the Windows control pipe in either direction: no client sends
    one and none accepts a secret-bearing reply, part or event. Sheets
    16 to 20 cover the control endpoint's transport only. An endpoint
    with another transport (the adapter endpoint on Windows) names, in
    its own specification, its transport and its daemon check, with
    their own `ironafs-os` sites and gate, before any secret-bearing
    message of that endpoint is enabled; this gate does not stand in
    for that.

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
  - the Windows gate measures a cancelled pipe operation's completion
    time as long or unbounded in a case a local user can cause (5.6.2,
    point 3), which would make the residual of section 8 larger than
    stated;
  - load tests show round-robin turns or the per-caller quarter of the
    executor's queue starving a legitimate caller (5.6.4);
  - a control result needs more than 65,536 parts or 16 MiB in one
    stream (5.2, point 11);
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
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; C4, C5, C9, C16, C37, C39, C41 to C46 ([part 1](../adr/0004-architecture-1-core.md)); L35 ([part 2](../adr/0004-architecture-2-os-layers.md)); O16, O23, O28, O47 ([part 3](../adr/0004-architecture-3-operations.md)); N41 ([part 4](../adr/0004-architecture-4-negotiation.md)); S1 | all |
| D2 | DECISION | [ADR 0005](../adr/0005-ironafs-os.md), accepted 2026-11-21: sheets 13 and 16 to 20; Consequences (the amendment of C5, point 5) | 1, 5.1.1, 5.2, 5.4, 5.5, 5.6.2, 8, 10.5 |
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
- **Q4. ADR 0004, O23's row for a malicious local IPC client** says of
  what remains: "None known". Section 8's Windows pre-attestation
  residual is something that remains, and O23, rule 3 requires each
  residual to be in the administrator's guide. O23 is Firm, so this
  specification cannot change it. The owner decides between: (a) an
  amendment of O23 that states the residual in that row, which this
  specification recommends; (b) leaving O23 as it is and accepting the
  residual here only, which leaves the two documents disagreeing; (c) a
  further operating-system site in ADR 0005 that gives the daemon a
  hint of the connecting process before any read, used for counting
  only, which would shrink the residual at the cost of a new unsafe
  site and a value that proves nothing. Blocks: marking this
  specification `reviewed` on its Windows parts, and the Windows gate.
  No public issue yet.
- **Q5. How long a cancelled pipe operation takes to complete** (5.6.2,
  point 3) is unverified. The Windows gate measures it
  (`cancel_completion_time`). Blocks: stating a time bound for the
  residual of section 8. Part of the Windows gate (M5-28, planned).

## 14. Review

Not yet reviewed. The record will be in `docs/reviews/`.
