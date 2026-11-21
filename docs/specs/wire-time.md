<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Wire time fields

Status: draft (2026-11-21). Raised in issue #82. This is the first
version, written from the participating specifications and documents
only. A capture-backed revision fills in the widths, signedness, epochs
and granularities that the observation lab's records settle (#51), and
the issue closes only when that revision is accepted (section 13, Q1).

Kind: protocol. Class: S. Every time a fileserver states passes through
this table before IronAFS uses it, one of those times (a callback
promise's expiry) sets how long cached data is served without
revalidation, and a hostile or broken peer controls every one of them
([README](README.md), "Review class per kind").

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (D1, S1 and
so on) are in section 12. Row ids (WT1, WR1 and so on) and the ids of
the disagreements in 5.5 (G1 and so on) are this specification's own
and stay stable once reviewed.

**Nothing here is coded yet.** No admissible document gives the width,
signedness, epoch or granularity of any AFS time field that M2a reads
(section 12, "What was not used"). Every such cell is MISSING, and the
converter of 5.3 has no entry for a row until its cells are filled
(ADR 0004, C6, point 4). What this version fixes is everything that does
not depend on those bytes: which fields exist, who owns each, which way
each travels, what IronAFS does with each, the conversion rules every
entry will follow, the refusal for each, and the tests.

## 1. What this implements

ADR 0004, overview, constraint 4 (a constraint of the whole ADR):

> **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
> local IPC, configuration, requests from the operating system, and SMB
> messages before authentication. No panics, checked arithmetic and byte
> limits apply to all of it (coding standards rules 1 to 3).

ADR 0004, C6, points 3 and 4 (**Firm**), the gate this table is:

> 3. **Specification gates.** These are written and reviewed before the
>    code that depends on them:
>    - before `ironafs-proto`: the table of wire time fields (C38), inbound
>      and outbound;
>
> 4. A fact about the AFS protocols that this ADR could not tie to a public
>    source is an input to a specification, never something to code from.

ADR 0004, C26, point 2 (**Working**):

> 2. Callback expiry is counted from when the request was sent, on the boot
>    clock (C37).

ADR 0004, C37, points 1 and 6 (**Working**):

> 1. The `Clock` interface gives three clocks:
>    - a **boot clock that counts suspend,** for every deadline a server
>      grants: callbacks, tokens, location lifetimes, locks;
>    - an **awake clock that excludes suspend,** for round-trip and
>      retransmission timers;
>    - **wall time,** only for AFS timestamps and for token expiry as the
>      protocol states it.
>
> 6. The client never sets the system clock.

ADR 0004, C38, points 1 to 5 (**Working**), the decision this table
carries out:

> 1. Inside IronAFS, wall time is one signed 64-bit type (seconds and
>    nanoseconds since 1970). Boot and awake time are 64-bit nanoseconds. No
>    32-bit time exists outside the wire codecs.
> 2. Every wire time field is converted in one module of `ironafs-proto`.
>    For each field it states the width, signedness, epoch and granularity,
>    from the specification.
> 3. Conversion never sign-extends by accident and never wraps. A value the
>    wire field cannot carry is refused with the "out of range" error
>    (C46, point 1); for example, a
>    file time the server cannot store is refused at store, not truncated.
> 4. Arithmetic on server-granted times is done in 64 bits after
>    conversion.
> 5. Tests cover the 32-bit boundaries of 2038 and 2106 `[DES-019]`, times
>    before 1970 and zero, and simulator runs with the clock set in 2039 and
>    in 2107.

ADR 0004, C46, points 1 and 4 (**Working**), the sentences that bind a
refusal:

> 1. […] out of range (C38, point 3). […] Out of range reaches
>    applications as an invalid-argument error (`EINVAL`, an `NSError`
>    with POSIX `EINVAL`, or `STATUS_INVALID_PARAMETER`), with the cause in
>    the side channel.
>
> 4. **A failure never looks like success:** never an empty or short
>    listing, zero bytes or a made-up attribute.

ADR 0004, O40, point 2 (**Working**):

> 2. Server times are converted at receipt. Clock skew is detected and warned
>    about.

The second sentence of O40, point 2, is amended for M2a by issue #53
(proposed, not yet accepted): a fileserver's time is used only from a
call qualified under N31, otherwise "not measured". The keepalive
specification (S2) rests on that amendment, and so does row WT5 here.

ADR 0004, N31, points 1 and 2 (**Firm**):

> 1. A wire fact is taken only from an allowlisted specification passage or
>    an independently generated black-box fixture, recorded with its
>    edition, section or fixture provenance.
> 2. Never from implementation history, interface-definition files,
>    generated definitions, or live-cell captures holding credentials or
>    private data.

**The re-check the issue asks for** (2026-11-21). The public claim
record holds one row on wire time: `[DES-019]`, the 32-bit boundaries.
It was recomputed: 2³¹ − 1 seconds after 1970 is
2038-01-19T03:14:07Z and 2³² − 1 is 2106-02-07T06:28:15Z, as the row
states. The record has no row giving the width, signedness or epoch of
any AFS time field; its rows `[DES-051]`, `[DES-059]` and `[DES-063]`
concern locks and rxgk, which M2a does not use (5.2's reserved rows).
C38 was re-read on `main` and quoted above. No contradiction between
the record, C38 and the participating specifications was found that
stops this task; the disagreements between the specifications
themselves are in 5.5.

Section 9 shows where each invariant is kept and which test fails if it
is broken.

## 2. Scope

1. **Covered:**
   - every time-valued field of the M2a wire procedures, inbound and
     outbound, in Rx, the volume location and file services, the
     callback service and the keepalive (5.1);
   - fields that look like times and are not converted, with the reason
     (5.2);
   - rows reserved for later specifications: rxkad, rxgk, stores and
     locks (5.2);
   - the conversion rules every entry of the converter follows, and the
     refusal for a value it cannot represent (5.3);
   - the disagreements between the participating specifications, and
     between them and ADR 0004, each with a recommended resolution (5.5).
2. **Not covered, and where it is covered:**
   - the layout of each message that carries a time: the specification
     that owns the procedure. This table names a field and its
     conversion; the owner names its place in the message;
   - what a callback promise means and how long it is relied on: the
     callback specification (S1), 5.1.4 and section 6, point 5;
   - when the keepalive is sent, and what a server's time may change: the
     keepalive specification (S2), 5.6 to 5.8;
   - the clocks, the `Stamp` and the `Deadline` types: the seams
     specification (S7) and `ironafs-time`;
   - how each operating system layer shows a converted time to an
     application, and its own range: the contract specification (#67) and
     each layer's specification.
3. **The rule for later specifications** (from the issue): a
   specification that adds or changes a wire time field edits this
   table in the same pull request. A row's owner is the specification
   named in its "Owner" cell; this table owns the columns.

## 3. The design in short

1. M2a's replies carry five wire time fields (WT1 to WT5): a file's two
   modification times in the file service's status record, a callback
   promise's expiry in a reply and again in a break, and a fileserver's
   own time in the keepalive's reply. M2a converts three of them, the two
   modification times and the reply's expiry; the break's expiry is never
   read, and the server's time is defined but not read in M2a. It sends
   none.
2. Only one of them feeds a deadline: the promise's expiry, which becomes
   a duration, cut to the callback specification's cap and counted on
   the boot clock from the request's send. The modification times are
   attributes. The server's time is diagnostics only, and in M2a it is
   not even read.
3. Every conversion happens once, in one module of `ironafs-proto`,
   after the XDR decode, from the raw words, by the row's width,
   signedness, epoch and granularity. It never sign-extends by accident,
   never wraps and never guesses an era.
4. A row whose width, signedness, epoch or granularity is MISSING has no
   converter entry. Its field is decoded as raw words and goes nowhere.
5. A value the converter refuses fails only what needs it: the
   attribute, the promise (none is granted) or the skew sample (none is
   taken). No refusal is ever replaced by a made-up time.
6. Several fields look like times and are not: the Rx epoch, the Rx
   acknowledgement's skew field and the time fields inside a UUID. None
   is converted, and none feeds a clock or a deadline.

## 4. Terms

- **Wire time field:** a field of a message on the wire whose value is a
  point in time or a span of time, as its owner specification describes
  it.
- **Row:** one wire time field in 5.1, with a stable id (WT1 and so on).
- **Owner:** the specification that decodes or encodes the message the
  field is in, and names the field's role.
- **Converter:** the one module of `ironafs-proto` that turns a row's
  raw words into IronAFS's internal time types (C38, point 2), named
  `wire_time` here `[OWN]`.
- **Timestamp:** a row whose value is a point in time. It converts to
  `WallTime` (signed 64-bit seconds and nanoseconds since
  1970-01-01T00:00:00Z, C38, point 1).
- **Span:** a row whose value is a length of time. It converts to a
  signed 64-bit count of nanoseconds, so that a negative wire value
  stays negative and its owner can refuse it.
- **Agreed range:** for a 32-bit seconds field whose signedness is not
  settled, the values 0 to 2³¹ − 1, which mean the same under the signed
  and the unsigned reading (5.3, point 7).
- **Raw words:** the XDR integers or hypers exactly as decoded; the XDR
  codec gives them no meaning ([xdr.md](xdr.md), 5.1, point 7).

## 5. Specification

### 5.1 Layout: the time fields M2a reads or sends

Two tables describe each row: what it is (5.1.1) and what IronAFS does
with it (5.1.2). "MISSING" means no admissible source states it; the
capture-backed revision fills it from raw bytes, varying the server's
clock or the file's time one known value at a time ([README](README.md),
"Sources and provenance", rule 4; section 13, Q1).

#### 5.1.1 Identity and layout

| Row | Field | Owner | Procedures that carry it | Direction | Width | Signedness | Epoch and unit | Granularity | Label |
|---|---|---|---|---|---|---|---|---|---|
| WT1 | a file's client modification time (`client_mtime`), in the status record | S3, 5.1.3, point 2 | `fs.fetch_status` (132), `fs.bulk_status` (155), `fs.inline_bulk_status` (65536), `fs.fetch_data` (130), `fs.fetch_data64` (65537): each returns a status record (S3, 5.1.4, points 4 to 6) | in | MISSING | MISSING | MISSING | MISSING | that the record carries it: UNVERIFIED (S3); layout MISSING (S3 Q1) |
| WT2 | a file's server modification time (`server_mtime`), in the status record | S3, 5.1.3, point 2 | as WT1 | in | MISSING | MISSING | MISSING | MISSING | as WT1 |
| WT3 | a callback promise's expiry, in a fileserver's reply | S1, 5.1.4 (S3 decodes the promise record unread and hands it over, its 5.1.3, point 3) | the procedures of WT1: `fs.fetch_status` and both fetches return one promise, the bulk calls one per file (S3, 5.1.4, points 4 to 6) | in | MISSING | MISSING | MISSING: whether a span or a timestamp on the server's clock (S1 Q1) | MISSING | the record's three words: `[K3 §3.1: UNVERIFIED]` in S1; layout MISSING (S1 Q1) |
| WT4 | the same expiry, in each promise record of a break | S1, 5.1.3, point 1, and 5.1.4's last paragraph | `cb.callback` (204), an incoming request; `cb.ext_callback` (65543) is not read at all until its layout is captured (S1, 5.1.2) | in | as WT3 | as WT3 | as WT3 | as WT3 | decoded for its bounds only; never converted (5.1.2) |
| WT5 | the server's current time, in the keepalive's reply | S2, 5.1.4 and 5.8 | `fs.get_time` (153) | in | MISSING: one field, or a seconds part and a sub-second part (S2 Q1) | MISSING | MISSING | MISSING | the reply's layout: MISSING (S2 Q1); the procedure number: UNVERIFIED (S2, 5.1.2) |

**Outbound: none.** No request IronAFS sends in M2a carries a time:
the VL lookups name a volume or a server, the file-service calls name
files, offsets and lengths (S3, 5.1.4), the keepalive's request is its
procedure number alone (S2, 5.1.3), and the callback service's replies
carry no time (S1, 5.5; the host UUID of points 3 and 4 there is in
5.2). The first outbound row is a store's modification time (5.2,
WR3).

**Rx carries no time field IronAFS reads.** The 28-byte header and the
packet payloads of the Rx specification (S4, 5.1) hold no time.
Round-trip samples are taken from local stamps of a packet's send and of
the acknowledgement that names its serial (S4, 5.7, point 11); no
timestamp crosses the wire. The two Rx fields that look like times are
in 5.2.

#### 5.1.2 What IronAFS does with each row

| Row | Converted to | Use | Fed to a deadline? | Range and wrap | Refusal | Decoder, then converter |
|---|---|---|---|---|---|---|
| WT1 | `WallTime` (a timestamp, C37, point 1: an AFS timestamp) | an attribute the layers show as a file's modification time (S3, 5.1.3, point 2) | no. Never compared for cache validity (the data version decides, S3, 5.4, point 5), never volume incarnation evidence (S6, 5.1.1: a file's times are attributes only), never handed to a clock `[OWN]` | point 6 below | the attribute fails: S3, 5.4, point 8 ("fails the attribute that needs it ... not the decode"); the operation that needs it gets out of range (C46, point 1); no attribute is made up (C46, point 4) | the file service's status record decoder (S3), then `wire_time` |
| WT2 | `WallTime`, as WT1 | as WT1: which of WT1 and WT2 a layer shows as the modification time is the contract specification's (#67) | no, as WT1 | as WT1 | as WT1 | as WT1 |
| WT3 | a span, if the capture shows a span; if it shows a timestamp, `WallTime`, turned into a span as point 5 below says | the promise's lifetime: cut to `cb.promise.lifetime_max` and counted from the request's send on the boot clock (S1, 5.1.4 and section 6, point 5) `[ADR 0004 C26.2: Working]` | **yes, the only row that is.** The deadline is on the boot clock, counted from the boot instant of the request's send stamp. Wall time enters only if the expiry is a timestamp, and only to turn it into a span: the expiry minus the wall time of the same send stamp (5.3, point 5), which C37, point 1, allows for an AFS timestamp. No deadline is held or compared on wall time `[ADR 0004 C37.1: Working]` | point 6 below; a lifetime of 0 or less grants no promise (S1, 5.1.4) | no promise is granted (S1, 5.1.4: "A value the conversion cannot represent grants no promise"), and so for every failure of section 6, point 7; the reply's other values stand | S3's reply decoder calls `wire_time` for WT3 with the call's `Reply` send stamp (S7) at receipt (5.3, point 9), and hands S1 the converted span or the refusal, with the promise record's other words unread |
| WT4 | not converted | none: a break only suspends, so nothing it carries can grant, extend or shorten a promise (S1, 5.1.3, point 1) | no | not read | none: the words are bounded by the message cap and not read | the callback service's request decoder (S1); never `wire_time` |
| WT5 | `WallTime` | diagnostics only: the skew sample and its warning (S2, 5.7, points 3 and 4). In M2a it is not read: server time is "not measured" (S2, 5.7, point 1) | no: it never feeds a deadline, never raises or lowers trust, never sets the clock, and can only shorten a promise (S2, 5.7, points 5 and 6) `[ADR 0004 C37.6: Working]` | point 6 below | no sample is taken; `skew` keeps its last state; counted as `time_refused`; the reply stays valid (S2, 5.8) | the keepalive reply decoder (S2, 5.4), then `wire_time`, only once S2's gate has passed |

### 5.2 Fields that look like times, and reserved rows

#### 5.2.1 Not converted

These fields hold, or are named for, a time in some source, and IronAFS
never reads them as one `[OWN]`. Listing them is how a reviewer checks
that the table is complete by identity: a field that is neither in 5.1
nor here has been missed.

| Field | Owner | Direction | Why it is not a time to IronAFS |
|---|---|---|---|
| the Rx epoch, 31 bits of the header's first word | S4, 5.1, point 1, and 5.6, points 1 and 5 | out and in | outbound, it is drawn whole from `Rng` with no clock as input (S4, 5.6, point 1; ADR 0006, decision 2, point 1); inbound, it is the peer's choice and is used only to match connections (S4, 5.6, points 5 and 6). No specification reads a peer's epoch as a start time or as evidence of a restart, and none may: it is an identity |
| the Rx acknowledgement's maximum skew, 2 bytes | S4, 5.1, point 5 | out and in | sent as 0 and never checked on receipt (S4, 5.4, point 12, and 5.5, point 4); its unit is not stated by any source used |
| the UUID's time-low word and its two 16-bit time fields, in a server identity | S3, 5.1.3, point 6 | in | a UUID is compared as the 16 octets it denotes, never as its words (S3, 5.1.3, point 6); its time fields are range-checked as integers and never converted |
| the same fields of the client's host UUID, in the identity replies | S1, 5.5, points 3 and 7 | out | the host UUID is created and stored by the state directory's specification, not yet written (S1 Q9); the identity replies send it as stored. Whether its time fields hold a clock reading is section 13, Q7 |
| a UUID or identity a callback request may carry: ProbeUuid's argument, and anything in a reset's arguments | S1, 5.1.3, points 4 and 6; 5.5, point 5 | in | never decoded: a reset's arguments are never read (S1, 5.1.3, point 4), and ProbeUuid is refused with −455 before its argument is read (S1, 5.5, point 5). Bounded by the message cap. A revision that decodes either adds it here |
| a volume's dates (creation, last update, copy) | S6, 5.1.1 and 5.7, point 3 | in, if any reply carries them | M2a reads none: they are reported by the volume server's procedures, which M2a does not call (S6, section 2, point 3), and whether the volume sync record carries the creation date is MISSING (S3, 5.1.3, point 4; S6 Q2). The record is decoded as fixed words and handed to S6 unread (S3, 5.1.3, point 4). A revision that reads a date adds its row to 5.1 |
| any other word of the status record, the VL entry or the volume status record | S3, 5.1.3, points 2 and 5, and 5.1.4, point 7 | in | decoded, type-checked and not interpreted (S3). The volume status record's fields are MISSING (S3 Q9); none named there is a time. A revision that finds and reads a time field adds its row to 5.1 |

#### 5.2.2 Reserved rows

Rows for procedures M2a does not use. Each is filled by its own
specification, in its own pull request, before its code; nothing here is
a fact to code from.

| Row | Field | Owner, when written | Direction | What is known now |
|---|---|---|---|---|
| WR1 | every time field rxkad's specification names (C6, point 3's rxkad gate) | the rxkad specification, M2b | as it says | nothing: no rxkad specification exists, and no public specification of rxkad has been found (ADR 0004, C6, "Why") |
| WR2 | rxgk's token expiry and every other time field of the rxgk wire profile (N25) | the rxgk specification, M6 | as it says | a lead only: in the rxgk-for-AFS draft an expiry of 0 means a token that never expires, recommended only for printed tokens `[DES-063]`. If that holds, the converter maps 0 to a distinct "never" value, never to 1970-01-01 |
| WR3 | the modification time a client sends when it stores a file's status | the store specification, M3 | out | C38, point 3's example: a time the wire field cannot carry is refused at store with out of range, not truncated |
| WR4 | any time field of the lock procedures | the lock specification, M3 | as it says | a server lock's lifetime is a server policy `[DES-051]`; whether any lock reply carries a time is not known |

### 5.3 The converter

These rules hold for every entry of `wire_time`, today's and every later
specification's. Each is `[OWN]` under C38 unless it names its source.

1. **One module.** Every wire time field is converted in `wire_time` in
   `ironafs-proto`, and nowhere else `[ADR 0004 C38.2: Working]`. A
   decoder hands it the row id and the raw words; no decoder converts a
   time itself, and no 32-bit time value leaves `ironafs-proto`
   `[ADR 0004 C38.1: Working]`.
2. **One entry per row, from the row's cells.** An entry states the
   row's width, signedness, epoch, unit and granularity as constants,
   copied from 5.1.1. A row with any of those cells MISSING has no entry,
   and its words go nowhere: no code is written from a MISSING cell
   `[ADR 0004 C6.4: Firm]`. A row whose cells are filled only by a lead
   (UNVERIFIED) has no entry either (README, "Labelling claims").
3. **Widening is explicit.** A field the row calls signed is widened by
   sign extension; one it calls unsigned is widened by zero extension.
   The choice comes from the row's constant, never from the Rust type the
   codec happened to decode into, and no `as` cast between signed and
   unsigned integers is used `[ADR 0004 C38.3: Working]`: a checked
   conversion (`i64::from`, `u64::try_from`, `i64::try_from`) is the
   only route, so a mismatch is a compile error or a refusal, not a
   silent reinterpretation.
4. **Never a wrap.** A value whose converted result cannot be held is
   refused with out of range: a 64-bit unsigned seconds value above
   `i64::MAX`, an epoch offset whose checked addition overflows, or a
   span above `i64::MAX` nanoseconds. Nothing is clamped into range, and
   no result is computed modulo anything.
5. **A timestamp expiry becomes a span.** If the capture shows WT3 is a
   timestamp on the server's clock, its span is the expiry minus the
   wall time of the granting call's send stamp (the seams
   specification's `Reply`, S7), computed in `i128` nanoseconds and then
   checked into `i64` `[ADR 0004 C37.1: Working]`: an expiry is an AFS
   timestamp, so wall time is the clock it is read against, and the
   deadline it becomes is on the boot clock (C26, point 2). Then the
   keepalive specification's bound applies: a qualified skew sample no
   older than `ka.skew.sample_max` that shows the server ahead by D
   shortens the span by D, and one showing it behind never lengthens it
   (S2, 5.7, point 6). D is S2's lower bound on the server's lead (S2,
   5.7, point 3), so a span corrected by it can still exceed the
   server's intent by up to that sample's uncertainty (its round trip
   plus g); the sample is handed to the converter by S2, which owns its
   age test, and the converter reads no sample state itself (point 10).
   Then the callback specification cuts it to `cb.promise.lifetime_max`
   (S1, section 6, point 5). In M2a no sample exists (S2, 5.7, point 1),
   so no D is ever applied; section 13, Q3, states what that leaves and
   Q4 the order of the steps.
6. **No era is guessed.** A 32-bit seconds field converts to exactly
   one instant, the one its row's signedness gives:
   - signed: 1901-12-13T20:45:52Z to 2038-01-19T03:14:07Z, for an epoch
     of 1970 in seconds `[DES-019]`;
   - unsigned: 1970-01-01T00:00:00Z to 2106-02-07T06:28:15Z
     `[DES-019]`.
   A server that writes a later time into a field that cannot carry it
   has already wrapped; IronAFS shows the value the field holds and never
   adds 2³² to "unwrap" it, since choosing an era by guess is a wrap by
   another name. What each row's wrap does is 5.4.
7. **The agreed range.** For a 32-bit seconds field with a settled
   width, epoch and unit but an unsettled signedness, values 0 to
   2³¹ − 1 convert to the same instant under both readings; values from
   2³¹ convert to 1901 to 1969 under one and 2038 to 2106 under the
   other. The signedness of a 32-bit field is an interpretation, not a
   bit pattern, so a capture of the bytes alone cannot settle it (section
   13, Q2). Whether such a row may have an entry limited to the agreed
   range is section 13, Q2.
8. **Sub-second parts.** A row with a sub-second part states its unit
   (for example microseconds). A part equal to or above one second in
   that unit, or below zero if the part is signed, is refused with out
   of range, never carried into or borrowed from the seconds (S2, 5.8). Granularity is recorded so that a consumer that
   compares two times (S2, 5.7, point 3's interval [S, S + g)) knows how
   wide each one is.
9. **Converted at receipt.** Each row is converted when its reply is
   decoded, after the XDR decode and before the reply's values are handed
   on `[ADR 0004 O40.2: Working]`. The result, a time or a refusal, is
   kept per field: a refusal fails what needs that field (5.1.2, the
   "Refusal" column), not the decode, and not the reply's other fields
   (S3, 5.4, point 8). Section 5.5, G2, records the wording this needs
   in S3.
10. **No clock is read.** The converter is a pure function of the raw
    words, the row and, for point 5 only, the send stamp and the skew
    sample (or its absence) its caller hands it: it reads no clock,
    keeps no state, and draws nothing from `Rng` (ADR 0004, C3, point 1, as
    [xdr.md](xdr.md) quotes it).

### 5.4 Decoding: range and wrap, per row

1. **WT1 and WT2, modification times.** Shown as the field holds them
   (5.3, point 6). If the fields are signed 32-bit seconds, a file a
   server stamps after 2038-01-19T03:14:07Z shows a time in 1901; if
   unsigned, a file stamped before 1970 cannot be represented by the
   server at all. Either is the server's limit, and IronAFS reports it
   faithfully: the time is an attribute, and nothing else depends on it.
   A refusal (a sub-second part out of range, or a 64-bit value beyond
   `WallTime`) fails the attribute (5.1.2).
2. **WT3, a promise's expiry.**
   - If it is a span: no wrap can occur in the field's own meaning. A
     negative value (if signed) or 0 grants no promise; a value above
     `cb.promise.lifetime_max` is cut (S1, 5.1.4).
   - If it is a timestamp: after its field's limit, a server's expiry
     wraps to a time far in the past, the span of 5.3, point 5, is
     negative, and no promise is granted. That fails towards
     revalidation: entries are served under bounded revalidation
     (ADR 0004, C32, point 2), which costs calls and never serves stale
     data.
   - Whatever the form, a server's expiry can make a promise shorter or
     absent, and never longer than `cb.promise.lifetime_max` (S1, 5.2).
     If it is a timestamp, a client clock behind the server's can make
     the promise longer than the server granted, within that cap
     (section 13, Q3).
   - A promise whose deadline cannot be built grants nothing (section 6,
     point 7).
3. **WT4.** Never converted, so it has no range.
4. **WT5, a server's time.** Converted only once the keepalive's gate has
   passed (S2, 5.7, point 1). A server past its field's limit states a
   time that differs from the client's by decades: the sample shows a
   skew far above `ka.skew.warn`, and the warning is the right outcome,
   since that server's clock cannot be compared. It changes nothing but
   diagnostics (S2, 5.7, point 5).
5. **The client's own clock past 2038 or 2106.** Internal time is
   signed 64-bit (C38, point 1), so the client's own times never wrap.
   Only a comparison with a server's field can be affected, and points 1
   to 4 say what each does.

### 5.5 Where the specifications disagree

Each disagreement found while building the table, with the
recommended resolution and the specification that should change. None
changes a wire fact; each changes a sentence or a column. G1 to G4 are
for the participating specifications' next revisions; G5 is for the
clocks' wording.

1. **G1. The columns of the owners' time tables.** C38, point 2, needs
   width, signedness, epoch and granularity for every field. The
   keepalive specification's table (S2, 5.8) has a granularity column;
   the callback specification's (S1, 5.1.4) does not; neither names the
   decoder; the VL and FS read specification has no time table and
   names its fields in 5.1.3, point 2. **Resolution:** S1, 5.1.4, adds a
   granularity column (MISSING, S1 Q1) and names this table's row ids
   WT3 and WT4; S2, 5.8, names WT5; S3 adds a short "Time fields"
   paragraph naming WT1 and WT2, so that every owner points at the same
   rows.
2. **G2. Converted at receipt, or handed on.** O40, point 2, says
   server times are converted at receipt. S3, 5.4, point 8, says times
   are "handed on unmodified" and that a conversion failure "fails the
   attribute that needs it". The two agree in effect only if the
   conversion happens at the decode and its result is kept per field.
   O40, point 2's "server times" may be read as the server's clock
   samples only, not file times; converting every row at receipt keeps
   to either reading, so the resolution does not depend on it.
   **Resolution:** this table's 5.3, point 9; S3, 5.4, point 8, says
   "converted at receipt by the wire time table (O40, point 2); a
   refusal fails the attribute that needs it, not the decode".
3. **G3. The order of the cut and the skew correction.** For an expiry
   that is a timestamp, S2, 5.7, point 6, computes the span, cuts it to
   `cb.promise.lifetime_max`, then shortens it by D. S1, section 6,
   point 5, converts the expiry "into a 64-bit duration" through this
   table, then cuts it. If the conversion includes the correction, S1's
   order is correction then cut. The results differ whenever the
   uncorrected span is above the cap and D is above 0 (a span of 8 000 s,
   a cap of 7 200 s and D of 2 000 s give 6 000 s one way and 5 200 s the
   other), and both are at or below the cap, so neither is unsafe. **Resolution:** correction first, then the cut (this
   table's 5.3, point 5): D is the server clock's bias in its own
   expiry, so removing it gives the server's intended span, and the
   client's cap then applies once; subtracting D after the cut counts it
   against a value that no longer contains it. S2, 5.7, point 6, changes
   to that order (section 13, Q4).
4. **G4. The signedness of the expiry in the callback tests.** S1's test
   C16 expects "the field's maximum" to give a promise of
   `cb.promise.lifetime_max`. Read as the largest value of the field's
   type, that holds under either signedness (2³¹ − 1 or 2³² − 1, both
   cut to the cap). Read as the all-ones word, it holds only if the field
   is unsigned: signed, that word is −1 and grants no promise. The test's
   wording is ambiguous, not wrong. **Resolution:** C16 names its words
   by value once the capture settles the type (the type's largest value,
   and separately the all-ones word with its expected result under that
   type); until then section 13, Q2's recommendation applies (no entry,
   so no promise).
5. **G5. "The monotonic clock".** C37, point 1, names two monotonic
   clocks, the boot clock and the awake clock, and says which each
   timer uses. Several sentences say "the monotonic clock" without
   saying which: the Rx specification for a call's deadline and for the
   retirement window R (S4, 5.7, point 1, and 5.6, point 2), the VL and
   FS read specification for the variant memo's expiry (S3, section 6,
   A9), the callback specification for its rate buckets' refill and its
   tick (S1, section 6, points 2 and 6), and the Rx specification for the
   instant its machine is given with each call into it (S4, 5.12, point
   6). The keepalive specification starts a keepalive call with a
   deadline "on the boot clock" (S2, 5.5, point 2), through the same Rx
   start whose deadline S4 calls monotonic. **Resolution:** each names
   its clock. Recommended: a call's deadline is the caller's `Deadline`
   with its own clock kind (S7), which for the keepalive is the boot
   clock; R, the variant memo, the callback tick and the rate buckets'
   refill are on the boot
   clock, since each measures time that passes for peers while the host
   sleeps; Rx's idle timeout and retransmission timers are on the awake
   clock, as C37, point 1, says for round-trip and retransmission
   timers; the instant given to Rx's machine is a `Stamp`, which carries
   both (S7). Not a wire time field; recorded here because the table's
   "Fed to a deadline?" column depends on it.

Also checked, and found consistent:

- S2, 5.7, point 3, reads wall time to compare a server's time with the
  client's, and 5.3, point 5 here reads an expiry against wall time.
  Both compare an AFS timestamp with the client's wall clock, which C37,
  point 1, allows ("only for AFS timestamps"); neither builds a deadline
  on wall time.
- The second sentence of O40, point 2 (skew detected and warned about),
  against S2's "not measured": S2 already records it as resting on the
  amendment of issue #53, and leaves draft only after #53 is decided.
- S1, S2 and S3 each name "#82" as the owner of the conversion and
  hand it raw words; none converts a time itself.

### 5.6 States and ordering

Not applicable: the converter keeps no state. The order of each
conversion within its reply's handling is its owner's, with 5.3, point
9.

## 6. Arithmetic

Every value read from a reply is untrusted (coding standards rule 2).

1. **Widening** of a 32-bit field into `i64` cannot overflow; it is done
   by `i64::from` on the row's signed or unsigned type (5.3, point 3).
2. **A 64-bit unsigned field** into `i64` seconds uses `i64::try_from`;
   failure is out of range (5.3, point 4).
3. **An epoch other than 1970,** if a row ever has one, is an `i64`
   offset added with `checked_add`; overflow is out of range.
4. **Seconds and a sub-second part** combine into `WallTime` only after
   the part is checked below one second (5.3, point 8); the seconds part
   is not changed by it.
5. **A span in seconds** becomes nanoseconds by `checked_mul` by 10⁹ in
   `i64`; overflow is out of range.
6. **A timestamp expiry minus a send stamp** (5.3, point 5) is computed
   in `i128` nanoseconds, where no difference of two `WallTime` values
   overflows, then checked into `i64`; failure is out of range, and no
   promise is granted. The skew correction D is subtracted in the same
   `i128` before the check.
7. **The promise's deadline.** A span of 0 or less grants no promise
   before anything else. A positive span is cut (S1, section 6, point
   5) and converted from `i64` nanoseconds into the seams
   specification's `Duration` by a checked conversion. The deadline is
   then built by `Deadline::boot_after(now, first_sent, span)` (S7, 5.3,
   points 1 and 2), where `first_sent` is the `Reply`'s send stamp and
   `now` the caller's fresh `Clock` sample. Every way this can fail
   grants no promise, counted as `time_refused` with the reason, and the
   reply's other values stand:
   - **`first_sent` is `None`** (the send sample failed; S7, `Reply`):
     no lifetime is counted from it, so no deadline is built;
   - **the `now` sample fails** (`DeadlineError::Clock`): no deadline;
     the clock failure itself is S7's episode, which makes callbacks
     suspect (C37, point 5);
   - **`DeadlineError::FromAfterNow`** (the send stamp is newer than
     `now` on the boot clock, which only a clock regression produces;
     S7, 5.3, point 2): no deadline, never retried with another stamp,
     as S7 requires;
   - **`DeadlineError::Overflow`** (the sum cannot be represented): no
     deadline;
   - **the span's conversion into `Duration` fails**: no deadline.
   These are all the errors `boot_after` returns (S7, 5.3, point 1:
   `Clock`, `FromAfterNow`, `Overflow`). In each case the entry is served
   under bounded revalidation (ADR 0004, C32, point 2), as for any file
   with no promise.

## 7. Errors and failure

1. **One error:** out of range (C46, point 1), with the row id in the
   side channel. The converter returns it; it never panics, on any word
   and any row (coding standards rule 1).
2. **Fail what needs the field, and only that** (5.1.2, the "Refusal"
   column): an attribute fails; a promise is not granted; a skew sample
   is not taken. The reply's other values stand (5.3, point 9).
3. **Fail towards revalidation.** No refusal grants, extends or keeps a
   promise, raises trust or feeds a deadline. A refused expiry leaves the
   file served under bounded revalidation (ADR 0004, C32, point 2).
4. **Nothing is an error to the peer.** A refused time is never
   reported to the server.
5. **Counters:** each refusal is counted per row (`time_refused`, with
   the row id) and handed to the event catalogue by its owner
   specification's summary event; no event is sent per refusal.

## 8. Security

Who can set a wire time: the server that sends the reply, and anyone on
the path for the null-class replies M2a reads, since nothing protects
their content (ADR 0004, N5, point 2).

| Threat | What stops it |
|---|---|
| A huge or far-future expiry that makes the client trust cached data for a long time | the span is cut to `cb.promise.lifetime_max` (S1, 5.1.4), so no expiry buys more than the cap. Not fully stopped if the expiry is a timestamp: read against a client clock that lags the server's, a promise can outlast the server's grant by the lag, within the cap; in M2a no skew sample exists to correct it (section 13, Q3) |
| A negative, zero or wrapped expiry | no promise is granted; the file is revalidated (5.4, point 2) |
| A value that wraps or sign-extends in conversion to look like a different time | explicit widening by the row's signedness, checked conversions, no `as` casts, no era guessing (5.3, points 3, 4 and 6) |
| A lying modification time | it is an attribute only: never compared for cache validity, never incarnation evidence, never a deadline (5.1.2) |
| A lying server time | diagnostics only, and not read in M2a (WT5; S2, 5.7) |
| A server time used to set or slew the client's clock | the client never sets the system clock `[ADR 0004 C37.6: Working]` |
| A peer's Rx epoch read as its start time, to forge a restart | no specification reads an epoch as a time (5.2.1) |
| A crash from a hostile value | no panic; checked arithmetic (sections 6 and 7); property tests over every word value (10.2) |
| A decoder that converts a time its own way and skips these rules | one module (5.3, point 1); the check of 10.5, T5 |

## 9. How the invariants are kept

| Invariant (section 1) | Kept by | Test that fails if it is broken |
|---|---|---|
| Constraint 4: no panics, checked arithmetic | 5.3; sections 6 and 7 | 10.2, P1; 10.3 |
| C6, point 3: the table, inbound and outbound | 5.1, 5.2 | review of this specification; 10.5, T1 |
| C6, point 4: nothing coded from a source-less fact | 5.3, point 2 | 10.5, T2 |
| C26, point 2: expiry counted from the send on the boot clock | 5.1.2, WT3; 5.3, point 5; section 6, point 7 | 10.5, T3 and T7 |
| C37, points 1 and 6: wall time only for AFS timestamps; no clock setting | 5.1.2; 5.3, points 5 and 10 | 10.5, T3 and T4; S2's KA17 |
| C38, point 1: signed 64-bit wall time; no 32-bit time outside the codecs | 5.3, point 1; section 4 | 10.5, T5 |
| C38, point 2: one module, each field's width, signedness, epoch and granularity | 5.1.1; 5.3, points 1 and 2 | 10.5, T1 and T5 |
| C38, point 3: no accidental sign extension, no wrap; out of range refused | 5.3, points 3, 4, 6 and 8; section 7 | 10.2, P1 to P3; 10.3 |
| C38, point 4: 64-bit arithmetic after conversion | section 6 | 10.2, P4 |
| C38, point 5: tests at 2038, 2106, before 1970 and zero; simulator in 2039 and 2107 | section 10 | 10.1 to 10.5 |
| C46, points 1 and 4: out of range; no made-up attribute | 5.1.2; section 7 | 10.5, T6 |
| O40, point 2: converted at receipt | 5.3, point 9 | 10.5, T6 |
| N31, points 1 and 2 | every MISSING cell; section 12 | review; the capture-backed revision (Q1) |

## 10. Tests

Paths follow coding standards rules 15 and 20. The converter's tests
live with `ironafs-proto` once it exists; the simulator runs live with
the core's crate and `ironafs-sim` (#96).

### 10.1 Golden vectors

None until the capture-backed revision fills a row (Q1). Each row's
vectors then come from independently generated black-box fixtures,
reduced, with the server's clock or the file's time set to known values:
a time before 2038, one at 2³¹ − 1 and 2³¹ seconds, one at 2³² − 1, and
one before 1970, wherever the server can be set to them (C38, point 5).
Each vector records which of them the server accepted.

### 10.2 Round-trip and property tests

The converter has no encoder in M2a, so these are properties of the
decode direction, run over every row with an entry, and over synthetic
test rows covering every combination of width (32, 64), signedness and
sub-second part, so the rules are tested before any real row exists:

- **P1 Total.** For every input word (exhaustively for 32-bit rows,
  randomly and at the boundaries for 64-bit), the converter returns a
  value or out of range, and never panics.
- **P2 No accidental extension.** For a signed 32-bit test row, the
  words `0x7fffffff`, `0x80000000` and `0xffffffff` convert to
  2038-01-19T03:14:07Z, 1901-12-13T20:45:52Z and 1969-12-31T23:59:59Z;
  for an unsigned row, to 2038-01-19T03:14:07Z, 2038-01-19T03:14:08Z
  and 2106-02-07T06:28:15Z. The values are derived by hand from
  `[DES-019]`.
- **P3 Agreed range.** For every word below 2³¹, a signed and an
  unsigned 32-bit test row give the same instant.
- **P4 Order.** For an unsigned row, a larger word gives a later
  instant; for a signed row, the same in two's complement order.

### 10.3 Reject corpus

Each must give out of range, with the reason:
a sub-second part equal to its unit, and one at its maximum (5.3,
point 8); a 64-bit unsigned seconds value of 2⁶³ (5.3, point 4); a span
whose nanoseconds overflow `i64` (section 6, point 5); a timestamp
expiry whose span from the send stamp overflows `i64` (section 6, point
6).

### 10.4 Fuzz target

None of its own: the converter decodes no bytes. Each owner's fuzz
target (S1, S2 and S3, 10.4) drives the converter through its decoder
and asserts that every time either converts or is refused, never wraps.

### 10.5 Acceptance cases

- **T1 Complete by identity.** For each participating specification
  (S1, S2, S3, S4), every field its time table or text names as a time
  is a row of 5.1, a field of 5.2.1, or a reserved row of 5.2.2, and
  every row of 5.1 names a field that specification has. Checked by
  review now (INSPECTED); proposed as a `cargo xtask` check that each
  owner's "Time fields" section names row ids that exist here and that
  every row's owner names it back (PROPOSED, a gap below).
- **T2 No entry from a MISSING cell.** The converter has no entry for a
  row with a MISSING or UNVERIFIED cell: checked by review of the
  module against 5.1.1 at each revision.
- **T3 Expiry on the boot clock.** In the simulator, a fileserver
  grants a promise of 3 600 s; the simulated host suspends for 30
  minutes and its wall clock is moved back one hour. The promise ends 3
  600 s of boot-clock time after the request's send, not later: the
  suspend counts, the wall change does not.
- **T4 Simulator in 2039 and 2107** (C38, point 5). Runs of the
  callback and keepalive acceptance cases (S1, S2, 10.5) with the
  simulated client's clock in 2039 and in 2107: the client's own
  deadlines and trust behave as in 2026. With the simulated server's
  fields wrapped as each reading of 5.3, point 6, says: a wrapped
  timestamp expiry grants no promise (5.4, point 2); a wrapped
  modification time shows the field's value and changes nothing else.
- **T5 One module.** A check that no crate other than `ironafs-proto`'s
  `wire_time` widens a decoded wire integer named as a time row into a
  time type. Until such a check exists (gap), by review.
- **T6 Refusal scope.** A status reply whose modification time is
  refused (a sub-second part out of range, in a test row): the decode
  succeeds, the data version and the promise stand, and an operation
  that needs the time gets out of range with the row id in the side
  channel; an operation that does not need it succeeds.
- **T7 Every deadline failure grants nothing** (section 6, point 7).
  In the simulator, a status reply with a promise, handled with in turn:
  a `Reply` whose `first_sent` is `None`; a failed `now` sample; a
  scripted boot-clock regression so the send stamp is newer than `now`;
  a span whose sum with the send instant overflows (a test row with a
  64-bit span). Each grants no promise, counts `time_refused` with its
  reason, leaves the reply's status and data version in place, and
  serves the entry under bounded revalidation; none is retried with
  another stamp.

### 10.6 Gaps

- Golden vectors and every real row's entry: the capture-backed
  revision (Q1).
- T1's and T5's mechanical checks: proposed for `cargo xtask` once the
  first owner specification is merged; until then by review.
- T3 and T4: the simulator's fileserver model (#96).

## 11. Revisit if

From the working decisions this rests on:

- ADR 0004, C26: "the specification gate of C25 shows a restore
  discriminator."
- ADR 0004, C37: "the M2 sleep, hibernation, Modern Standby and VM-pause
  tests show a clock below does not behave as its documentation says,
  or a supported target lacks a valid pair."
- ADR 0004, C38: "the wire time table (C6) shows a field whose published
  definition this rule cannot represent."
- ADR 0004, C46: "a layer's error mapping cannot express a contract
  error without losing what the user needs to act on it."
- ADR 0004, O40: "the specification work for the file service (C6)
  shows a server behaviour that contradicts a point here."
- Issue #53 is accepted with words other than those quoted in section
  1, or not accepted.

This specification's own choices:

- a capture shows a time field with an epoch other than 1970, a unit
  other than seconds and a sub-second part, or a width above 64 bits
  (5.3);
- a capture shows that the promise's expiry is a timestamp (5.3, point
  5; Q3);
- a supported server is found that relies on a client reading its Rx
  epoch as a time (5.2.1);
- the capture-backed revision of S1, S2 or S3 names a time field not in
  5.1 or 5.2.

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| S1 | DECISION | The callback service specification, `callbacks.md` (#66), draft of 2026-11-21, at the revision on the keepalive branch (commit 0c3b8d7, which adds the lowering count; its section 6 has six points, where the earlier revision at b85db41 has five and numbers the promise lifetime 4 and the tick 5): 5.1.2, 5.1.3 (points 1 and 2), 5.1.4, 5.5, 5.8, section 6 (points 2, 5 and 6), 10.5 (C16) and 13 (Q1, Q9). Its leads K1 to K3 are its own; nothing here takes a wire fact from them | 2, 5, 6, 8, 10 |
| S2 | DECISION | The keepalive and server time specification, `keepalive-and-server-time.md` (#81), draft of 2026-11-21: 5.1, 5.4, 5.5, 5.7, 5.8, 10.5 and 13 | 1, 2, 5, 7, 8, 10 |
| S3 | DECISION | The VL and FS read specification, `vl-fs-read.md` (#65), draft of 2026-11-21: 5.1.2, 5.1.3 (points 2 to 6), 5.1.4, 5.4 (points 5 and 8), section 6 (A9) and 13 (Q1, Q9) | 2, 5, 7, 8 |
| S4 | DECISION | The Rx specification, `rx.md` (#55), draft of 2026-11-21: 5.1 (points 1 and 5), 5.4 (point 12), 5.5 (point 4), 5.6 (points 1, 2, 5 and 6), 5.7 (points 1 and 11) and 5.12 (point 6). Every wire fact there is UNVERIFIED, from the community Rx draft, which was not opened for this specification | 5.1, 5.2, 5.5 |
| S5 | DECISION | [XDR profile](xdr.md): 5.1, point 7 (integers are not converted) | 4, 5.3 |
| S6 | DECISION | The volume incarnation specification, `volume-incarnation.md` (#78), draft of 2026-11-21: section 2 (point 3), 5.1.1, 5.7 (point 3) and 13 (Q2) | 5.1, 5.2 |
| S7 | DECISION | The seams specification, `seams-and-scheduling.md` (#71), draft of 2026-11-21 (the seam types revision): 5.3, points 1 and 2 (`Stamp`, `Deadline`, `DeadlineError` and its three values, `FromAfterNow`'s rule), and 5.4 (`Reply` and its optional `first_sent`) | 2, 5.3, 5.5 |
| C | DECISION | [Claim record](../reviews/2026-11-20-adr-0004-claims.md) row `[DES-019]`, recomputed on 2026-11-21; rows `[DES-051]` and `[DES-063]`, cited only for the reserved rows WR2 and WR4, as leads | 1, 5.2, 5.3, 10 |
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; C3, C6, C26, C32, C37, C38, C46 ([part 1](../adr/0004-architecture-1-core.md)); O40 ([part 3](../adr/0004-architecture-3-operations.md)); N5, N25, N31 ([part 4](../adr/0004-architecture-4-negotiation.md)) | 1, 5, 7, 8, 9, 11 |
| D2 | DECISION | ADR 0006 (Rx unprotected packets), decision 2, point 1, as S4, 5.6, point 1, applies it | 5.2 |
| D3 | DECISION | The amendment of ADR 0004 in issue #53, point 3 (proposed, not yet accepted) | 1, 5.5 |
| D4 | DECISION | [ADR 0001](../adr/0001-licence-and-dont-copy.md), "Don't copy"; [coding standards](../coding-standards.md), rules 1 to 3, 15 and 20 | 6, 7, 10, 12 |
| O | OWN | every point tagged `[OWN]`, and 5.3, with its reason in place | 5, 6, 7 |

**What was not used.** No source code, test data, interface definition
or generated definition of any AFS or Rx implementation was read for
this specification, and none is cited. The community Rx specification
draft was not opened: what this table needs of Rx is taken from the Rx
specification (S4), and its section "Historical Implementation Notes"
was not read (README, "Sources and provenance", rule 12). No copy of
the 1991 AFS-3 programmer's references was used: no copy whose
provenance is independent of every implementation's repository has been
found (rule 10). No registered edition was read for this specification:
every fact about a peer here comes through a participating
specification, with that specification's label, so this specification
adds no row to the register of editions.

### 12.1 Observed behaviour

None yet. The capture-backed revision records here, or takes from the
participating specifications' records, the lab's fixtures for each row
(#51): a test venue with synthetic cell names, users, keys and data; raw
bytes first; each with its venue, server release, date, the values set
and the evidence label EXECUTED.

## 13. Open questions

- **Q1. The capture-backed revision** (#51). Every cell marked MISSING in
  5.1.1: the width, signedness, epoch, unit and granularity of WT1, WT2,
  WT3 and WT5, and whether WT3 is a span or a timestamp. Method, per
  field, varying one value at a time: for WT1 and WT2, files whose times
  are set to known values through a released client of the test venue,
  including 2³¹ − 1 seconds, the latest the venue accepts, and one
  before 1970 if it accepts one; for WT3, two calls at known intervals,
  which tells a span (the same value) from a timestamp (a value that
  grows with the server's clock), and, if it is a timestamp, the same
  calls with the server's clock set to known values, including past
  2038-01-19T03:14:07Z where the venue allows it: the server chooses the
  expiry, so its width, epoch and signedness show only through how the
  value follows the server's clock, and through whether a server past
  the signed boundary sends a word from 2³¹ or a wrapped one. If the
  venue cannot be set past 2038, WT3's signedness stays open, and Q2's
  rule applies to it; for WT5, the server's clock set to
  known values (S2's TK1). Blocks: every converter entry, and so every
  code path that reads a server time. Recommendation: one campaign with
  S1's Q1, S2's Q1 and S3's Q1.
- **Q2. Signedness, which bytes cannot show.** A 32-bit word means the
  same bits under either reading (5.3, point 7), so a capture settles a
  field's signedness only through what the server does at the boundary:
  whether it accepts a time before 1970, or past 2038-01-19T03:14:07Z,
  for a file, and what its own published tools display for it. A
  server that refuses both leaves the question open. Recommendation:
  until a source or such an observation settles a row's signedness, the
  row has no converter entry (5.3, point 2), as C6, point 4, requires;
  if the observation settles width, epoch and unit and leaves only
  signedness open, a spike gives the row an entry limited to the agreed
  range, which refuses values from 2³¹ with out of range and counts
  them. For WT3 that refusal grants no promise, the safer of the two
  readings (5.5, G4). Values from 2³¹ in a seconds-since-1970 field are
  2038 or later, or before 1970, so the cost now is files with such
  times showing an error instead of a time. Decided by the issue's rule
  ("If a field's width or signedness is uncertain, a spike comes
  first").
- **Q3. A timestamp expiry while the server's time is not measured.**
  If Q1 shows WT3 is a timestamp, 5.3, point 5, reads it against the
  client's wall clock. With the server's time "not measured" in M2a (S2,
  5.7, point 1), there is no D: a client whose clock is behind the
  server's by X relies on each promise for up to X longer than the
  server granted it, and the only bound is `cb.promise.lifetime_max`
  (7 200 s), whatever X is. The same holds after S2's gate opens
  whenever no sample, or no sample younger than `ka.skew.sample_max`,
  exists, and a fresh sample still leaves up to its uncertainty (5.3,
  point 5). The skew warning bounds nothing: it is diagnostic only and
  changes no lifetime (S2, 5.7, point 4). A break the server stops
  sending after its own expiry, while the client still relies on the
  promise, is then not delivered, and that stale data is served under
  trust is the residual. S2, 5.7, point 6's bound ("the expiry read
  against the client's own clock") is true but is not the server's.
  Recommendation, a class S question for the callback (#66) and
  keepalive (#81) specifications' revision, with this table's row
  changed in the same pull request: a timestamp expiry is used only with
  a qualified skew sample younger than `ka.skew.sample_max`, corrected
  by D plus the sample's uncertainty; without one, the expiry is treated
  as absent: it grants no promise, or, if that revision prefers, a
  promise of a short fixed lifetime of the client's own choosing that
  takes nothing from the server's value. In M2a, where no sample can
  exist, that means a timestamp expiry grants no promise beyond that
  fixed lifetime. Not needed if WT3 is a span.
- **Q4. Order of the cut and the correction.** 5.5, G3. Recommendation:
  correction first, then the cut, with S2, 5.7, point 6, changed to
  match. Only matters once S2's gate opens.
- **Q5. Which modification time a layer shows.** WT1 and WT2 are both
  converted; which one each layer shows, and whether the other is
  exposed at all, is the contract specification's (#67). Recommendation:
  the contract specification decides, with this table unchanged.
- **Q6. The clocks' names.** 5.5, G5. Recommendation: S1, S3 and S4 name
  the boot or awake clock where they say "monotonic", as 5.5, G5,
  proposes, in their next revisions.
- **Q7. The host UUID's time fields.** The identity replies send the
  host UUID (5.2.1). If the state directory's specification creates it
  with a time-based UUID, its time fields carry the moment of creation,
  without authentication, to any host that can send from a known
  server's address (the server itself, or a host on the path or
  spoofing that address); S1 answers identity calls only from a source
  matching a known server (S1, 5.6, point 9, row I2). Recommendation: the
  state directory's specification creates it from `Rng` alone, so no
  clock reading leaves the host and no row is needed here; if it
  chooses otherwise, it adds an outbound row to 5.1 in its own pull
  request.

## 14. Review

Not yet reviewed. The review record goes in `docs/reviews/` before this
specification leaves draft.
