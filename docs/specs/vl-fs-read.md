<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Volume location and file-service reads, and operation grants

Status: draft (2026-11-21). Raised in issue #65.

Kind: protocol, with one behaviour part (5.7, the operation grants).
Class: S. A hostile or broken volume location server or fileserver
controls every byte decoded here, and the grant rule decides what cached
data a user may read ([README](README.md), "Review class per kind": a
protocol specification, and a behaviour specification touching
authorisation).

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (V1, D1 and so
on) are in section 12.

**This is the documents-only version.** No admissible document gives the
argument and result layouts of the AFS-3 volume location and file-service
procedures (section 12, "What was not used"). So every layout point in
5.1 is MISSING or UNVERIFIED, and nothing is coded from it until the
capture-backed revision (section 13, Q1). What this version does fix is
everything that does not depend on the byte layout: which procedures
M2a uses, their roles, the caps, the hostile-input rule for every field,
the fallback catalogue, the error mapping and the grant rule.

## 1. What this implements

ADR 0004, overview, constraint 4 (a constraint of the whole ADR):

> **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
> local IPC, configuration, requests from the operating system, and SMB
> messages before authentication. No panics, checked arithmetic and byte
> limits apply to all of it (coding standards rules 1 to 3).

ADR 0004, C6, points 1 and 4 (**Firm**):

> 1. Each protocol component is written from a behaviour specification in
>    `docs/specs/` that names its sources, under
>    [ADR 0001](../adr/0001-licence-and-dont-copy.md). No specification, no code.
>
> 4. A fact about the AFS protocols that this ADR could not tie to a public
>    source is an input to a specification, never something to code from.
>    Each decision below says where it leans on one.

ADR 0004, C22, points 1 to 3 (**Firm**):

> 1. Cached data is read only with a **grant**. A read grant is minted only
>    from a status reply for that file, fetched in the reader's own context
>    and generation, carrying the server's access mask for that caller. Each
>    item of a bulk status reply counts on its own. A shared-pool grant is
>    minted only from authoritative evidence (C21, point 6).
> 2. The decision that a mask and mode bits allow reading follows the ACL
>    specification (C6). Where it cannot be made reliably, the reader's own
>    data fetch decides.
> 3. The client never infers a file's rights from its directory.

ADR 0004, C46, points 3 and 4 (**Working**):

> 3. **An AFS abort code is classified by the RPC and the server that
>    returned it, never by its number alone.**
> 4. **A failure never looks like success:** never an empty or short
>    listing, zero bytes or a made-up attribute.

ADR 0004, N5, points 2 and 3 (**Firm**):

> 2. Everything else is a **hint**: DNS answers, rxnull replies, rxkad
>    replies, Rx aborts, timeouts, capability words read without that
>    protection, and anything `aklog` reports.
> 3. A hint may lower trust, or choose between RPC variants the catalogue
>    lists as equivalent (N20). A hint never raises or satisfies a floor,
>    never learns or renews a pin, and never authorises an exception.

ADR 0004, N20, points 1 to 5 (**Working**):

> 1. The protocol crate holds one static catalogue. For each optional RPC it
>    states: what announces it, its fallback, and whether the fallback keeps
>    the same guarantees.
> 2. A fallback is used only where the catalogue lists one.
> 3. Abort codes are classified by RPC and server (C46).
> 4. Example: the bulk status call after a listing, and its fallback, is a
>    catalogue entry whose order of use rests on A8.
> 5. A fallback that keeps the same guarantees may be chosen on a hint
>    (N5, point 3). One that does not requires evidence (N5, point 1); where
>    the caller asked for the stronger guarantee, the operation fails
>    instead.

ADR 0004, N28, points 1 to 4 (**Firm**):

> 1. AFS-3's caller access and mode restrictions are normalised at the
>    protocol boundary into **operation grants**: one internal type, a set
>    of operations. The core never interprets AFS rights letters.
> 2. The AFS-3 mask-to-operations evaluator is confined to one module plus
>    the codecs. An `xtask` check enforces it.
> 3. **Unknown bits grant nothing.** A test proves it.
> 4. Every access answer an OS layer gives (L1) is backed, **per
>    right**, by a current server-derived grant. Mode bits are metadata
>    only.

ADR 0004, N31, points 1 to 3 (**Firm**):

> 1. A wire fact is taken only from an allowlisted specification passage or
>    an independently generated black-box fixture, recorded with its
>    edition, section or fixture provenance.
> 2. Never from implementation history, interface-definition files,
>    generated definitions, or live-cell captures holding credentials or
>    private data.
> 3. Older published AFS documents are cited for facts only, in IronAFS's
>    own words, each edition registered before use, with corroboration for
>    anything that affects interoperability or correctness. A claim that
>    would fix wire or on-disk behaviour stays unverified until
>    corroborated. Counsel reviews this citation policy before the first
>    release. A registered edition can settle assumption A9 as the overview
>    states.

ADR 0004, N38, points 1 to 3 (**Firm**):

> 1. **The data version is unsigned 64-bit** in every internal type, cache
>    key, IPC field and journal field.
> 2. **Volume, vnode and uniquifier are typed unsigned 32-bit** under the
>    `afs3` protocol identifier. They are distinct types, not bare integers.
>    The signedness is IronAFS's reading; the file-service specification
>    (C6) confirms it against its sources before any format that stores
>    them is frozen.
> 3. **Legacy protection ids are typed signed 32-bit**, with checked
>    conversions and tests for negative ids.

ADR 0004, O40, point 1 (**Working**):

> 1. IronAFS assumes the file service offers 64-bit fetch and store calls
>    beside 32-bit ones; the specification work (C6) confirms it. The client
>    uses the 64-bit calls by default, and the 32-bit calls only when a
>    server lacks them. Files
>    across the 2 GiB and 4 GiB boundaries are tested.

ADR 0004, assumption A8 (an assumption of the overview; it changes with
the ADR):

> We assume an AFS-3 fileserver accepts InlineBulkStatus after a directory
> listing, and that BulkStatus is the fallback when a server refuses it.
> Both RPCs exist; their order of use is our assumption `[DES-076]`.

ADR 0006, decision 1, point 5, its last list (**Firm**), the part that
binds an abort code's effect here:

> Whatever its code says (C46, point 3), it never:
> - marks a server down for other calls or other contexts;
> - discards, expires or marks unusable a token or credential; […]
> - changes a negotiation profile, floor or pin (N5, point 3; N6,
>   point 1);

Section 9 shows where each is kept and which test fails if it is broken.

## 2. Scope

1. **Covered:**
   - the procedures M2a calls on the volume location service and the
     file service, for reads only (5.1.2), each with its arguments, its
     results, its caps, the hostile-input rule for every decoded field,
     the error mapping and the grants it needs or mints;
   - the common values those procedures carry: the file id, the status
     record, the callback promise and the volume sync record, as far as
     their decoding and their hostile-input rules go (5.1.3);
   - the static fallback catalogue entries for these procedures (5.8,
     ADR 0004 N20);
   - the mapping of AFS-3 caller access and mode restrictions to the
     core's operation grants for the read-only M2a operations (5.7, ADR
     0004 N28, points 1 and 3);
   - the classification of abort codes for these procedures (5.9, C46,
     point 3).
2. **Not covered, and where it is covered:**
   - Rx, the call lifecycle, abort delivery and which aborts are matched:
     [rx.md](rx.md) (#55) and ADR 0006;
   - the XDR codec, its caps model and strictness: [xdr.md](xdr.md);
   - the directory object fetched with these procedures: the directory
     format specification (#56);
   - the callback promise's meaning and the callback procedures,
     including the procedure that gives callbacks up: the callback
     specification (#66). This specification only decodes the promise and
     hands it over;
   - the conversion of every wire time field: the wire time-field table
     (#82, C38). This specification names the fields and hands them over;
   - what in a reply identifies a volume's incarnation (the volume sync
     record among others): the volume incarnation specification (#78,
     C25);
   - mount points, volume selection and `@sys`: the namespace
     specification (#80);
   - how cell database servers are found: C33 and its DNS specification;
   - writes, stores, creates, locks, ACL reads and writes, and the token
     procedures: later specifications (M2b, M3). The grant rule for write
     operations is theirs;
   - the full ACL and mode-bit rule that the rxkad gate requires (C6,
     point 3; O50, point 3): the ACL specification. 5.7 states the M2a
     subset and is written so that the ACL specification can only narrow
     the grants it mints, or turn a denied case into one the server
     decides on a fresh, uncached call (5.7, point 7);
   - the simulator's volume location and fileserver models: #96, which
     implements this specification's behaviour.
3. **Procedures deliberately not used in M2a** (each `[OWN]`, with its
   reason):
   - the file-service name lookup procedure (registry number 161): names
     are resolved from the directory object (#56), and the status of the
     target comes from a status call in the reader's own context (C22,
     point 1); a second route to the same answer is a second decoder to
     fuzz;
   - the file-service time procedure (153): the keepalive and the server
     probe are C32's and the callback specification's;
   - the file-service capability query (65540): the 64-bit fetch is
     announced by its own answer (5.8, entry F1), so the query adds a
     round trip and a decoder for no decision M2a makes;
   - the volume location capability query (65537) and the older volume
     location lookups (503, 504, 518, 519): see 5.8, entry V1.

## 3. The design in short

1. Every procedure is an Rx call: a 32-bit procedure number, then the
   arguments in XDR; the reply is the results in XDR, or an abort with a
   32-bit code `[X1 "RPC Layer": UNVERIFIED]`.
2. M2a uses nine procedures: two volume location lookups and the
   address list (volume location service), and the status, bulk status,
   inline bulk status, 32-bit and 64-bit data fetch and volume status
   procedures (file service) (5.1.2).
3. Every reply has a byte cap that is also the Rx call's reply limit, so
   no reply larger than the cap is ever buffered. Every variable-length
   field has its own cap. Fixed-size records are decoded strictly, with
   no trailing bytes.
4. Every decoded field has a stated use and a hostile-input rule. A field
   that no decision here depends on is decoded, checked for type, and
   not interpreted.
5. A reply is checked against its own request before anything is minted
   from it: the bulk reply has one item per file asked for, a fetch
   returns no more than was asked and no less than the reply's own status
   says the file holds.
6. Rights come only from the caller's own status reply, item by item. A
   known bit grants the operations its letter allows; an unknown bit
   grants nothing. Mode bits only narrow a file's read and execute
   grants. Where the mask and the mode bits cannot settle it, no grant is
   minted and the server decides on a fresh fetch (C22, point 2).
7. Three optional procedures have catalogue fallbacks: the 64-bit fetch
   falls back to the 32-bit fetch inside its range, the inline bulk
   status to the bulk status, and either bulk status to one status call
   per file. Each keeps the same guarantees, so each may be chosen on a
   hint: the server's "unknown procedure" abort, or a bulk reply that
   fails its checks.
8. An abort code is classified by procedure and service before it means
   anything, and whatever it means, it acts only on the call and the
   context that received it.

## 4. Terms

- **VL service, file service:** the volume location service and the file
  service of an AFS-3 cell, reached as Rx services (5.1.2).
- **Procedure number:** the 32-bit number that starts a request and
  selects the procedure `[X1 "RPC Layer": UNVERIFIED]`.
- **File id (FID):** a volume id, a vnode number and a uniquifier, each a
  distinct unsigned 32-bit type (ADR 0004, N38, point 2).
- **Status record:** the file service's description of one file for one
  caller: type, length, data version, owner, mode, access masks, times
  and the rest (5.1.3).
- **Caller access mask:** the 32-bit word of a status record that gives
  the rights of the caller who made the call. **Anonymous access mask:**
  the word that gives the rights of an unauthenticated caller.
- **Rights letter:** one of the server-defined rights `r l i d w k a`,
  and the auxiliary rights `A` to `H` (5.7, point 1).
- **Known bit:** a bit of an access mask whose rights letter this
  specification names with a source (5.7, point 2). Every other bit is an
  **unknown bit**.
- **Operation grant:** the core's internal type, a set of operations a
  context may perform on one object (ADR 0004, N28, point 1). The
  operations named here are the M2a read operations of 5.7, point 4.
- **Callback promise:** the record a file service returns with a status,
  promising to notify the client of a change (#66).
- **Volume sync record:** a record returned with file-service results
  that describes the volume (#78); that it is of fixed size is
  UNVERIFIED (5.1.3, point 4).
- **VL entry:** the volume location service's record for one volume: its
  name, the ids of its versions, and its sites.
- **Site:** one server and partition that holds a version of a volume
  `[V7: UNVERIFIED]`.
- **Variant memo:** a bounded record, per context, server and catalogue
  entry, that a server refused an entry's preferred procedure (5.6,
  point 2). No memo means "use the preferred procedure".
- **Request cap, reply cap:** the byte caps of 5.2 for a message, which
  are also the Rx call's `rx.call.request_max` and `rx.call.reply_max`
  ([rx.md](rx.md), 5.7, point 1).

## 5. Specification

### 5.1 Layout

#### 5.1.1 Framing

1. **A request** is the procedure number as a 32-bit integer, then the
   procedure's arguments, XDR-encoded, carried as the Rx call's request
   bytes `[X1 "RPC Layer": UNVERIFIED]`. Whether the number is signed or
   unsigned on the wire does not matter for the values used here, which
   are all below 2³¹ (5.1.2).
2. **A reply** is the procedure's results, XDR-encoded, carried as the Rx
   call's reply bytes `[X1 "RPC Layer": UNVERIFIED]`. The receiver checks
   that the sender sent no more than expected `[X1 "RPC Layer":
   UNVERIFIED]`: here, the decoder rejects trailing bytes ([xdr.md](xdr.md),
   5.4, point 10) and the reply cap bounds what Rx accepts.
3. **A failure** reaches the caller as an Rx abort with a 32-bit code
   ([rx.md](rx.md), section 7, `Aborted`). Codes from the server are
   normally positive and codes from the Rx layer are negative `[V1
   errors.html, "AFS com_err tables": UNVERIFIED]`. Whether a procedure
   can also report a failure inside its results (for example per item of
   a bulk status) is MISSING (section 13, Q5).
4. **A data fetch** returns, after any fixed results, a length, then that
   many bytes of file data, then the rest of the results: X1 names this
   as one convention for streaming calls `[X1 "RPC Layer": UNVERIFIED]`.
   For the two fetch procedures here, the width of the length word (32 or
   64 bits), whether the data bytes are padded to a unit, and which
   results come before and after the data are MISSING (section 13, Q3).
5. **Services.** The file service is Rx service 1 at UDP port 7000; the
   VL service is Rx service 52 at UDP port 7003 `[V1 rxservice.txt:
   UNVERIFIED]` `[DES-011]`. The service id cannot change on a connection
   after its first call `[X1 "Rx Connections": UNVERIFIED]`; IronAFS opens
   one connection per service ([rx.md](rx.md), 5.6).

#### 5.1.2 The procedures M2a uses

Numbers are from the registry's procedure-number tables. The registry is
registered (row E7) but no passage of it is allowlisted yet (section 13,
Q2), so every number is UNVERIFIED until a capture shows it. None is
taken from an interface definition or a generated definition (ADR 0004,
N31, point 2). The "Registry source" column is the registry's own note of
where each number came from.

| IronAFS name | Registry name | Service | Number | Registry source | Purpose in M2a | Label |
|---|---|---|---|---|---|---|
| `vl.get_entry_by_name` | GetEntryByNameU | VL | 527 | IBM AFS 3.4 | the VL entry for a volume name (#80 picks the name) | `[V1 rx/VL.txt: UNVERIFIED]` |
| `vl.get_entry_by_id` | getEntryByIDU | VL | 526 | IBM AFS 3.4 | the VL entry for a volume id and version (read/write, read-only or backup) | `[V1 rx/VL.txt: UNVERIFIED]` |
| `vl.get_addrs` | GetAddrsU | VL | 533 | IBM AFS 3.4 | the addresses of a server named in an entry by identity rather than address (5.1.4, point 3) | `[V1 rx/VL.txt: UNVERIFIED]` |
| `fs.fetch_status` | FetchStatus | file | 132 | IBM AFS 3.0 | one file's status record, callback promise and volume sync record | `[V1 rx/RXAFS.txt: UNVERIFIED]` |
| `fs.bulk_status` | BulkStatus | file | 155 | IBM AFS 3.0 | the status records of several files in one volume | `[V1 rx/RXAFS.txt: UNVERIFIED]` |
| `fs.inline_bulk_status` | InlineBulkStatus | file | 65536 | a registration request (`shadow (#600)`) | as `fs.bulk_status`, preferred (5.8, F2) | `[V1 rx/RXAFS.txt: UNVERIFIED]` `[DES-076]` |
| `fs.fetch_data` | FetchData | file | 130 | IBM AFS 3.0 | a byte range of a file, directory or symbolic link, 32-bit offsets | `[V1 rx/RXAFS.txt: UNVERIFIED]` |
| `fs.fetch_data64` | FetchData64 | file | 65537 | a registration request (`reuter (#720)`) | as `fs.fetch_data`, 64-bit offsets, preferred (5.8, F1) | `[V1 rx/RXAFS.txt: UNVERIFIED]` `[V2 §4: UNVERIFIED]` |
| `fs.get_volume_status` | GetVolumeStatus | file | 149 | IBM AFS 3.0 | a volume's quota and usage, for the operating system's file-system statistics | `[V1 rx/RXAFS.txt: UNVERIFIED]` |

`[OWN]` The registry's names are used only to identify the procedure;
the IronAFS names in the first column are the ones code and tests use.
The registry lists the 3.4-era volume location lookups as the latest; the
choice of those over the older ones is 5.8, entry V1.

The registry's source column for the file service's capability query
(65540), not used here, names an implementation's change history; the
procedures used here are noted as IBM releases or as registration
requests, so none of their numbers rests on implementation history as
the registry records it.

#### 5.1.3 Common values

The order, width and signedness of the fields in each record below are
MISSING unless a point says otherwise: no admissible document gives them
(section 12, "What was not used"). The capture-backed revision fills in
5.1.3 and 5.1.4 field by field, from raw bytes, varying one input at a
time ([README](README.md), "Sources and provenance", rule 4; section 13,
Q1). What follows is each field IronAFS needs or must tolerate, its
internal type, and its use. The hostile-input rule for each is in 5.4.
Which fields each record carries on the wire, and whether a record is of
fixed size, are UNVERIFIED as well: the field lists below are the roles
IronAFS needs (ADR 0004, C22, C25, N38, and the leads named), not a
statement of the wire layout. A role the captures do not find is
removed, and its use is redesigned, before any codec merges.

1. **File id.** A volume id, a vnode number and a uniquifier, decoded
   into three distinct unsigned 32-bit types `[ADR 0004 N38.2: Firm]`.
   That each is one 32-bit XDR word, and their order, are MISSING (Q1);
   the signedness is IronAFS's reading and stays UNVERIFIED until a
   capture with values of 2³¹ and above (Q6).
2. **Status record.** The fields IronAFS uses, by role. That the record
   carries each of them is UNVERIFIED (Q1); that it is of fixed size is
   UNVERIFIED (Q1):

   | Field (IronAFS name) | Internal type | Use here | Label |
   |---|---|---|---|
   | `file_type` | closed enumeration: file, directory, symbolic link | the object's kind; selects which grants apply (5.7) | values MISSING (Q1) |
   | `link_count` | `u32` | attribute only | carried: UNVERIFIED; layout MISSING (Q1) |
   | `length` | `u64` | the object's size; bounds the data a fetch reply may hold (5.4, point 9; section 6, A3), never the length a request asks for, which is never 0 (5.5, point 2) | width and split MISSING (Q1, Q3) |
   | `data_version` | `u64` | the version of the bytes; compared for equality only | `[ADR 0004 N38.1: Firm]`; wire carriage MISSING (Q4); a 64-bit data version appears in published extension material `[DES-057]` |
   | `author`, `owner` | protection id, `i32` | attribute only; mapped by the layer | `[ADR 0004 N38.3: Firm]` for the type; carried: UNVERIFIED; wire signedness MISSING (Q1, Q6) |
   | `group` | protection id, `i32` | attribute only | as `owner` |
   | `caller_access` | `u32`, raw | the caller's rights (5.7) | `[ADR 0004 C22.1: Firm]` that the reply carries the caller's mask; layout MISSING (Q1, Q7) |
   | `anonymous_access` | `u32`, raw | recorded; grants nothing (5.7, point 6) | carried: UNVERIFIED; layout MISSING (Q1) |
   | `mode` | `u32`, raw; bits 0o7777 used | metadata, and the owner bits narrow a file's grants (5.7, point 5) | carried: UNVERIFIED `[V8 HDRWQ580: UNVERIFIED]`; layout MISSING (Q1) |
   | `parent_vnode`, `parent_unique` | `u32` each | recorded; grants nothing and identifies nothing (5.4, point 7) | carried: UNVERIFIED; layout MISSING (Q1) |
   | `client_mtime`, `server_mtime` | wire words, handed to the time table | modification times, converted by #82's table (C38) | carried: UNVERIFIED; layout MISSING (Q1; #82) |
   | any other word of the record | wire words | decoded, type-checked, not interpreted | MISSING (Q1) |

   That the access masks are 32-bit words is UNVERIFIED (Q1): the
   capabilities draft's prose describes a flag that depends on the
   insert right `[V3 §4: UNVERIFIED]`, which names the right but not its
   bit.
3. **Callback promise.** A record whose fixed size is UNVERIFIED (Q1); if
   the captures show it is not fixed, this point is rewritten with caps
   before any codec merges. This specification
   decodes it as fixed words and hands them unmodified, with the file id
   and the context, to the callback service (#66), which owns their
   meaning, their caps on time and their trust (C31, C32). Nothing here
   reads them.
4. **Volume sync record.** A record whose fixed size is UNVERIFIED (Q1),
   as point 3; decoded as fixed
   words and handed unmodified to the volume incarnation logic (#78,
   C25). Nothing here reads it.
5. **VL entry.** The fields IronAFS uses, by role
   `[V7 vos_examine(1), "OUTPUT": UNVERIFIED]` `[ADR-008]`:

   | Field (IronAFS name) | Internal type | Use here |
   |---|---|---|
   | `name` | bytes | the volume's base name; checked against the request (5.4, point 12) |
   | `volume_ids` | one volume id per version: read/write, read-only, backup, and a release clone id | the ids of the versions; 0 means that version does not exist `[V7 vos_examine(1), "OUTPUT": UNVERIFIED]`. The page lists the release clone id among the lines it takes from the VL entry, not from the volume header; that the wire entry carries it is still UNVERIFIED (Q8), and M2a never calls it |
   | `sites` | a bounded list of sites | where each version lives |
   | `site.server` | an IPv4 address, or a server identity resolved through `vl.get_addrs` | the server to call (5.1.4, point 3) |
   | `site.partition` | `u32` | attribute (status and diagnostics); kept and compared as incarnation evidence by [volume-incarnation.md](volume-incarnation.md), 5.1.1 |
   | `site.flags` | `u32`, raw | which versions the site holds, and whether a read-only site is not released, of an old release or of a new release `[V7 vos_examine(1), "OUTPUT": UNVERIFIED]` |
   | `entry_flags` | `u32`, raw | which versions exist; recorded |
   | any other word | wire words | decoded, type-checked, not interpreted |

   Which of these the wire entry carries is UNVERIFIED; how a site names
   its server, the site table's size and how many of its slots are
   valid, the flag bits, and the field order are MISSING (Q1, Q8).
6. **Server identity.** Where a site names its server by identity, the
   identity is a UUID. The AFS-3 UUID is encoded as eleven XDR integers:
   three unsigned (the time-low word, then two 16-bit time fields) and
   eight signed (two 8-bit clock-sequence fields widened to 32 bits,
   then six node octets) `[V4 §4.1, §4.2: UNVERIFIED]`. The decoder
   applies the draft's range checks: each 16-bit time field at most
   65 535, each of the eight signed fields within −128 to 127
   `[V4 §4.2: UNVERIFIED]`. (The draft's own decoding text gives a 16-bit
   range for the two clock-sequence fields; IronAFS checks the 8-bit
   range, the narrower reading, and section 13, Q8, records the
   difference.) Each signed field's value v, in −128 to 127, is the
   octet v mod 256 (two's complement), and the encoder writes octet o as
   the signed value o − 256 when o ≥ 128 and o otherwise, so each octet
   has one encoding `[OWN]`, from the draft's statement that the fields
   are sign-extended `[V4 §4.1: UNVERIFIED]`. The UUID is compared as the
   16 octets it denotes, never as the eleven words.
7. **Server address.** An IPv4 address, four octets `[DES-058]`. Its
   byte order within its XDR word is MISSING (Q1).

#### 5.1.4 Arguments and results, per procedure

For each procedure: what IronAFS sends, what it decodes, the grants it
needs before it is sent, what it mints, and its error rows (5.9). Field
order within each message is MISSING (Q1). Caps are in 5.2.

1. **`vl.get_entry_by_name`.**
   - Arguments: the volume name, as bytes. IronAFS sends only names it
     has validated: 1 to `vl.name_max` bytes, none NUL or `/` `[OWN]`;
     whether the name travels as an XDR string or a fixed-size field is
     MISSING (Q1).
   - Results: one VL entry (5.1.3, point 5).
   - Needs no grant: the location of a volume is not data (C22 binds
     reads of cached data). The answer is a hint over the null class
     (N5, point 2).
   - Mints nothing. The entry feeds the location cache (C33, point 7).
   - Errors: 5.9, rows VL.
2. **`vl.get_entry_by_id`.**
   - Arguments: a volume id, and which version it names (read/write,
     read-only, backup) `[V7 vos_examine(1), "OUTPUT": UNVERIFIED]`. The
     encoding of the version selector is MISSING (Q1).
   - Results, grants and errors: as point 1.
3. **`vl.get_addrs`.**
   - Called only for a site that names its server by identity (5.1.3,
     point 5).
   - Arguments: at least the server's identity (5.1.3, point 6); any
     other argument is MISSING (Q1, Q8).
   - Results: a bounded list of IPv4 addresses for that server, and
     whatever else the capture shows (Q8).
   - Needs no grant; mints nothing. Errors: 5.9, rows VL.
4. **`fs.fetch_status`.**
   - Arguments: one file id.
   - Results: the file's status record, a callback promise and a volume
     sync record: the composition is UNVERIFIED (Q1).
   - Sent for an object the caller holds a lookup grant on, through its
     directory (C22, point 6), or for a root the namespace names (#80).
     The server is the authority: the call is always allowed to be sent.
   - Mints: the grants of 5.7 for that file, in the caller's context and
     generation.
   - Errors: 5.9, rows FS.
5. **`fs.inline_bulk_status`, `fs.bulk_status`.**
   - Arguments: a list of 1 to `fs.bulk.fids_max` file ids, all in one
     volume `[OWN]` (a bulk call is made per volume, so its whole reply
     has one volume sync record to hand to #78).
   - Results: a list of status records and a list of callback promises,
     and a volume sync record; that both variants return this
     composition is UNVERIFIED (Q1, Q5). That the lists are positional, item i for
     file id i, is UNVERIFIED (Q5); whether the inline variant reports a
     per-item failure inside the item, and how, is MISSING (Q5). The
     release notes of one implementation record a correction of the
     inline variant's reply in error cases `[V5: UNVERIFIED]`, which is
     why one failed item discards the whole reply (5.4, point 10).
   - Sent after a listing, for the entries of one directory the caller
     holds a read-directory grant on (5.7) `[ADR 0004 A8]`.
   - Mints: the grants of 5.7 for each item on its own, from that item
     only `[ADR 0004 C22.1: Firm]`, once the whole reply has passed 5.4,
     point 10.
   - Errors: 5.9, rows FS, and 5.8, F2 and F3, which say which failures
     lead to which fallback.
6. **`fs.fetch_data64`, `fs.fetch_data`.**
   - Arguments: one file id, an offset and a length. In
     `fs.fetch_data64` both are 64-bit; in `fs.fetch_data` they are
     32-bit `[V2 §4: UNVERIFIED]`; their signedness is MISSING (Q3), so
     IronAFS sends in `fs.fetch_data` only offsets and lengths whose sum
     is below 2³¹, which every reading of a 32-bit field carries `[OWN]`.
   - Results: the data length, the data, then the file's status record,
     a callback promise and a volume sync record (5.1.1, point 4); the
     composition is UNVERIFIED and the order MISSING (Q1, Q3).
   - Sent only with a read grant from a status reply in the caller's own
     context (5.7), except under C22, point 2's fallback, where no grant
     can be minted and the fetch itself decides (5.7, point 7). Directories
     and symbolic links are fetched the same way: a read-directory grant
     for a directory, a read-link grant for a link.
   - Mints: the grants of 5.7 from the status record in the reply, and
     the data, bound to that record's data version (5.4, point 9).
   - Its Rx reply limit is set per call to the bytes asked plus 2 048,
     never above the message cap (5.2).
   - Errors: 5.9, rows FS.
7. **`fs.get_volume_status`.**
   - Arguments: a volume id.
   - Results: a record (whether of fixed size is UNVERIFIED, Q1, Q9) of
     the volume's quota, blocks in use, and
     the partition's free and total blocks, and short strings: the
     volume's name, an offline message and a message of the day. These
     fields are the ones the reference documentation's quota and
     examination commands display; whether this procedure carries all of
     them, their units, widths and order are MISSING (Q1, Q9).
   - Sent for file-system statistics on a volume the caller has reached
     through a lookup grant. Gives a statistics answer only, used once
     and not cached; it is not a grant of the evaluator (5.7, point 4).
   - Its hostile-input rules are 5.4, point 14.
   - Errors: 5.9, rows FS.

### 5.2 Caps

Each cap is IronAFS's own `[OWN]`, with its reason, in the columns of
[xdr.md](xdr.md), 5.2, point 5. "Declared bound" is MISSING for every
field: no admissible document states one (section 12, "What was not
used"). The capture-backed revision records each declared bound found by
observation, and lowers a cap that exceeds it (xdr.md, 5.2, point 4).
Every message's reply cap is passed to Rx as `rx.call.reply_max`, and its
request cap as `rx.call.request_max`, so Rx never buffers more
([rx.md](rx.md), 5.7, point 1; xdr.md, 5.2, point 9.6).

The memory budgets assume the generator's proof (xdr.md, 5.2, point 9.3)
holds with these values; if it fails on a target once the layouts are
known, this table is amended at class S before the codec merges.

| Cap name | Field | Kind | Declared bound | Cap | Direction | Memory budget | Secret | Why |
|---|---|---|---|---|---|---|---|---|
| `vl.get_entry_by_name.request` | message | message | MISSING | 128 bytes | out | — | | a number and one short name |
| `vl.get_entry_by_name.request.name` | name | string or fixed (Q1) | MISSING | 64 bytes | out | — | | a read/write name is at most 22 characters `[V7 vos_create(1): UNVERIFIED]`; with a version suffix it stays far below 64 |
| `vl.get_entry_by_id.request` | message | message | MISSING | 64 bytes | out | — | | a number, an id and a selector |
| `vl.get_entry.reply` | message | message | MISSING | 4 096 bytes | in | 8 192 bytes | | one entry with a bounded site table; the reply of both lookups |
| `vl.get_entry.reply.name` | name | string or fixed (Q1) | MISSING | 64 bytes | in | — | | as the request's name |
| `vl.get_entry.reply.sites` | sites | array or fixed (Q1) | MISSING | 2 048 bytes | in | — | | a site is a few words; 2 KiB holds far more sites than a cell uses, and bounds the loop |
| `vl.get_addrs.request` | message | message | MISSING | 256 bytes | out | — | | a number, a UUID of eleven words and a few fixed words |
| `vl.get_addrs.reply` | message | message | MISSING | 2 048 bytes | in | 4 096 bytes | | one server's address list and fixed words |
| `vl.get_addrs.reply.addrs` | addresses | array | MISSING | 1 024 bytes | in | — | | 256 IPv4 addresses for one server |
| `vl.get_addrs.per_lookup_max` | calls | count | — | 8 calls | out | — | | the `vl.get_addrs` calls one VL entry may cause; sites beyond them stay unresolved and unusable (5.4, point 11), so a hostile entry cannot fan out |
| `fs.fetch_status.request` | message | message | MISSING | 64 bytes | out | — | | a number and one file id |
| `fs.fetch_status.reply` | message | message | MISSING | 1 024 bytes | in | 2 048 bytes | | a status record of at most `fs.status_record_max`, a callback promise of at most `fs.callback_record_max`, and a volume sync record; the decoder rejects trailing bytes |
| `fs.bulk_status.request`, `fs.inline_bulk_status.request` | message | message | MISSING | 512 bytes | out | — | | a number and the file id list |
| `fs.bulk_status.request.fids`, `fs.inline_bulk_status.request.fids` | file ids | array | MISSING | 384 bytes | out | — | | `fs.bulk.fids_max` (32) file ids of 12 bytes |
| `fs.bulk_status.reply`, `fs.inline_bulk_status.reply` | message | message | MISSING | 16 384 bytes | in | 32 768 bytes | | two lists of at most 32 items and one volume sync record (its fixed size UNVERIFIED, 5.1.3, point 4) |
| `fs.bulk_status.reply.statuses`, `fs.inline_bulk_status.reply.statuses` | status records | array | MISSING | 8 192 bytes, and no more items than file ids sent | in | — | | `fs.bulk.fids_max` × `fs.status_record_max`; the item count is checked against the request (5.4, point 10) |
| `fs.bulk_status.reply.callbacks`, `fs.inline_bulk_status.reply.callbacks` | callback promises | array | MISSING | 2 048 bytes, and no more items than file ids sent | in | — | | `fs.bulk.fids_max` × `fs.callback_record_max` |
| `fs.fetch_data.request`, `fs.fetch_data64.request` | message | message | MISSING | 64 bytes | out | — | | a number, a file id, an offset and a length |
| `fs.fetch_data.reply`, `fs.fetch_data64.reply` | message | message | MISSING | the bytes asked + 2 048, at most `fs.fetch.chunk_max` + 2 048 (1 050 624) bytes; set per call as the Rx reply limit (5.1.4, point 6) | in | 1 050 624 bytes | | the data, its length word and padding, and the other records |
| `fs.fetch_data.reply.data`, `fs.fetch_data64.reply.data` | data | opaque stream (5.1.1, point 4) | MISSING | the length requested, at most `fs.fetch.chunk_max` | in | counted in the message's budget | | a reply never holds more than was asked (5.4, point 9) |
| `fs.get_volume_status.request` | message | message | MISSING | 64 bytes | out | — | | a number and a volume id |
| `fs.get_volume_status.reply` | message | message | MISSING | 2 048 bytes | in | 2 048 bytes | | a record of counts (its fixed size UNVERIFIED, Q9) and three short strings |
| `fs.get_volume_status.reply.name` | volume name | string (Q9) | MISSING | 64 bytes | in | — | | as `vl.get_entry.reply.name` |
| `fs.get_volume_status.reply.offline_message` | text | string (Q9) | MISSING | 256 bytes | in | — | | an administrator's one-line message |
| `fs.get_volume_status.reply.motd` | text | string (Q9) | MISSING | 256 bytes | in | — | | as above |

Other limits `[OWN]`:

| Name | Value | Applies to | Why |
|---|---|---|---|
| `vl.name_max` | 64 bytes | a volume name IronAFS sends | the name cap above; a longer name is refused locally (5.5, point 2) |
| `fs.bulk.fids_max` | 32 file ids | one bulk status call | keeps the reply cap small; the server's own bound is unknown (Q5), and a server that refuses a size is handled by 5.8, F3 |
| `fs.fetch.chunk_max` | 1 048 576 bytes | the length of one fetch | bounds one call's buffer; a larger read is several fetches |
| `fs.symlink.target_max` | 4 096 bytes | the length of a symbolic link's target | longer than any path the operating systems IronAFS serves accept; a longer link is refused (5.4, point 9) |
| `fs.status_record_max` | 256 bytes | one status record's encoded size, in every reply that carries one | one value for single and bulk replies, so a record that fits one fits the other; the capture-backed revision checks the captured size against it and amends both caps together |
| `fs.callback_record_max` | 64 bytes | one callback promise's encoded size | as above |
| `fs.variant_memo_ttl` | 3 600 s | how long a variant memo lasts (5.6, point 2) | a server upgraded in place is found again within an hour, and a forged "unknown procedure" abort costs at most that long |
| `fs.variant_memo.per_context_max` | 64 memos | variant memos one credential context holds | a context talks to few fileservers at once; beyond the cap the least recently used memo is evicted |
| `fs.variant_memo.total_max` | 1 024 memos | variant memos the daemon holds | bounds the table whatever the number of contexts; beyond it the least recently used memo is evicted. Each memo is a fixed record of at most 128 bytes, so the table holds at most 128 KiB |

### 5.3 Versions

Not applicable as a format version: these are peer procedures IronAFS
does not control ([README](README.md), "Formats IronAFS does not
control"). Variants of a procedure are catalogue entries (5.8).

### 5.4 Decoding

Every reply is decoded with the strict rules of [xdr.md](xdr.md), 5.4.
No tolerance is declared for any field (xdr.md, 5.4, point 11): none has
a source that shows a supported peer needs one. The checks run in this
order; the first failure ends the decode, and nothing from the reply is
used (xdr.md, 5.4, point 13) `[OWN]`.

1. **Rx outcome.** Only a `Completed` outcome is decoded. Its reply bytes
   are within the reply cap, because the cap was the call's
   `rx.call.reply_max` ([rx.md](rx.md), 5.7, point 5).
2. **XDR decode,** with the message's caps (5.2). A reply that fails is
   `ReplyInvalid` (section 7).
3. **Closed enumerations.** `file_type` is closed: a value outside the
   three known kinds rejects the record (`UnknownFileType`). In a bulk
   reply it fails the whole reply (point 10). `[OWN]`: an object
   IronAFS cannot classify cannot be served as any kind without making
   up an attribute (C46, point 4).
4. **Identifiers.** A file id, volume id or vnode is decoded into its own
   type, never a bare integer `[ADR 0004 N38.2: Firm]`. A protection id
   is decoded as a signed 32-bit value `[ADR 0004 N38.3: Firm]`; its
   wire word is read with a checked conversion, so a wire form other
   than the one captured is an error, not a wrapped value (Q6).
5. **Data version and length.** Each is assembled into a `u64` from its
   wire words with checked arithmetic (section 6, A1). The data version
   is compared for equality only. A data version lower than the one
   cached is never rejected here: it is C25, point 4's ambiguity, handed
   on `[ADR 0004 N29.2: Working]` (the client does not assume that a
   change moves the data version upward).
6. **Access masks and mode.** Kept raw. They are interpreted only by the
   grant evaluator (5.7). Mode bits above 0o7777 are ignored and counted
   (`mode_high_bits`), and never shown `[OWN]`: no source gives them a
   meaning, and a bit IronAFS cannot name is never metadata a user acts
   on.
7. **Parent fields.** `parent_vnode` and `parent_unique` are recorded and
   used for nothing in this specification: not for a grant (C22,
   point 3), not for identity (C11), and not to find a directory `[OWN]`.
   A later specification that uses them states its own rule.
8. **Times, callback promise, volume sync record.** Handed on unmodified
   (5.1.3, points 2 to 4). A failure in #82's conversion fails the
   attribute that needs it (C38, point 3), not the decode.
9. **Fetch consistency.** For a data fetch of `len` bytes at `offset`,
   with the reply's own status record giving `length` `[OWN]`:
   - the reply's data length is at most `len`; more is
     `FetchLengthMismatch`, and the reply is not used;
   - it equals `min(len, length − offset)` when `offset < length`, and 0
     when `offset ≥ length`; anything else is `FetchLengthMismatch`. The
     check is against the status record in the same reply, never a cached
     one: the file may have changed since the request was planned, and
     the reply's own record says which version the bytes belong to;
   - the reply's `file_type` is the kind the fetch was planned for
     (file, directory or symbolic link); otherwise `FetchObjectMismatch`,
     and the reply is not used;
   - the size caps are checked twice: on the status record the fetch was
     planned from, so no fetch is sent for an object over them, and again
     on the reply's own status record, so an object that grew, or a reply
     that claims more, is caught. A directory's `length` meets the
     directory format's caps (#56); a symbolic link's `length` is at most
     `fs.symlink.target_max`, else `SymlinkTooLong`;
   - a symbolic link whose `length` is 0, on the planning record or on a
     fetch reply's record, is `ReplyInvalid`: a link with no target
     cannot be served, and no 0-byte fetch is ever asked for;
   - a file or directory whose planning `length` is 0 needs no data
     fetch: it is empty under that record's data version. An undecided
     read of it (5.7, point 7) asks for status again in the reader's own
     context instead, which decides it;
   - a symbolic link is read whole: one fetch at offset 0 for its whole
     `length`. If the reply's `length` differs from the planned one and
     is still within `fs.symlink.target_max`, the link is fetched once
     more with the new length; a second difference fails the read with
     `FetchLengthMismatch`. A prefix of a link is never served;
   - an object read in several fetches (a directory, or a file read
     whole) is assembled only from replies with one `data_version`; a
     reply with another version restarts the assembly once from offset
     0, and a second change fails the read with `stale`;
   - the data bytes are bound to the reply's `data_version`; nothing is
     cached under any other version.

   A short fetch is never served as the file's content (C46, point 4).
10. **Bulk replies.** `[OWN]`, from C22, point 1:
    - the status list and the callback list each have exactly as many
      items as file ids were sent; otherwise the whole reply is
      `BulkCountMismatch` and unused, and the files are asked again one
      by one (5.8, F3);
    - each item is then checked, points 3 to 8. No item carries its file
      id, so a reply that is misaligned but complete cannot be detected
      item by item, and one implementation corrected its inline replies
      in error cases `[V5: UNVERIFIED]`. So, until Q5 shows that items
      stay positional when one fails, any rejected item, or any item that
      reports a failure, discards the whole reply, status records and
      callback promises alike, and the files are asked again one by one
      (5.8, F3);
    - a reply that passes mints each item's grants from that item alone
      (C22, point 1), and hands callback promise i, with file id i, to
      #66 only together with status record i.
11. **VL entries, field rules.** `[OWN]`:
    - `name` is at most its cap and holds no NUL; otherwise
      `ReplyInvalid`;
    - a volume id of 0 means the version is absent
      `[V7 vos_examine(1), "OUTPUT": UNVERIFIED]`; such a version is never
      called;
    - a site whose flags hold a bit this specification does not name,
      or whose server cannot be resolved, is unusable and counted; the
      other sites stand;
    - a read-only site that is not released or of an old release is
      excluded before replicas are ranked `[ADR 0004 C25.6: Working]`;
    - a site address in 0.0.0.0/8 (this network), 127.0.0.0/8
      (loopback), 169.254.0.0/16 (link-local), 224.0.0.0/4 (multicast)
      or 240.0.0.0/4 (reserved, which holds the limited broadcast
      address) is unusable: none is a fileserver a cell's VL service can
      meaningfully name, and loopback would point the client at the
      local machine. The one exception is loopback when the cell's own
      VL servers are configured at loopback addresses (a test venue);
    - a site whose server is named by identity is resolved with
      `vl.get_addrs` only if it is a candidate: it holds the version
      asked, and survives the release filter above. Each distinct server
      identity is resolved once per lookup, at most
      `vl.get_addrs.per_lookup_max` calls per lookup (a value to revisit
      once Q8 settles the site table's size); a site whose `vl.get_addrs` call fails, by any abort or
      decode failure, is unusable and counted, and the other sites
      stand;
    - an entry with no usable site for the version asked fails the
      lookup with `offline` (5.9).
12. **VL entries, against the request.** `[OWN]`: for a lookup by name,
    the entry's base name equals the name asked, with the version suffix
    the namespace stripped (#80); for a lookup by id, the entry holds
    that id for the version asked. Otherwise the reply is
    `VlEntryMismatch` and unused. An entry for another volume is never
    cached under the one asked for.
13. **UUIDs** follow 5.1.3, point 6's range checks; a failure is
    `ReplyInvalid`. **Address lists** from `vl.get_addrs` follow point
    11's address rule; an empty usable list makes the site unusable.
14. **Volume status.** `[OWN]`, for `fs.get_volume_status`:
    - if the reply names a volume id, it equals the one asked; otherwise
      `ReplyInvalid`;
    - every count (quota, blocks in use, partition free and total) is
      read as the captured type (Q9); a negative value, where the type
      is signed, makes that count unusable;
    - a count is turned into bytes only by checked multiplication with
      the block size Q9 settles (section 6, A10); an overflow makes that
      count unusable;
    - an unusable count fails the statistics answer (C46, point 4: no
      made-up attribute), with the generic failure (Q11);
    - the three strings are at most their caps; a NUL in one is
      `ReplyInvalid`. They are text for display only: never parsed,
      never used as a name or a path, and shown only with control
      characters escaped: octets below 0x20 and 0x7F, the code points
      U+0080 to U+009F where the text is valid UTF-8, and every octet of
      a string that is not valid UTF-8, so a server cannot write
      terminal escapes into the command line's output.

### 5.5 Encoding

1. Requests are encoded in the canonical form of [xdr.md](xdr.md), 5.5,
   with the request caps of 5.2 `[OWN]`.
2. The encoder refuses, before anything is sent `[OWN]`:
   - a volume name of 0 or more than `vl.name_max` bytes, or one holding
     NUL or `/`: `NameInvalid`, and the lookup fails with `not found`;
   - a volume id of 0, in a file id, in `vl.get_entry_by_id` or in
     `fs.get_volume_status`, or a bulk list that is empty, longer
     than `fs.bulk.fids_max`, or spans two volumes: a fault in the caller,
     `RequestInvalid`;
   - a fetch whose `offset + len` overflows `u64`, whose `len` is 0 or
     above `fs.fetch.chunk_max`, or, for `fs.fetch_data`, whose
     `offset + len` is 2³¹ or more (5.1.4, point 6): `RequestInvalid`,
     except the last, which is `RangeNotSupported`, the catalogue's
     out-of-range case (5.8, F1).
3. No request carries a reserved field whose value IronAFS chooses until
   the capture-backed revision shows one; that revision states each
   reserved field's value on encode (README, "What every format
   specification states", point 5).

### 5.6 States and ordering

1. **Order of a read** `[OWN]`, from C22: status first, in the reader's
   own context (point 4 of 5.1.4); then the grant (5.7); then the fetch.
   A fetch's reply mints again from its own status record, and the data
   is delivered only under a grant minted from that record or one with
   the same data version (C22, point 4), except under 5.7, point 7, where
   the server's own answer to this caller is delivered once and nothing
   is cached.
2. **Variant memo** `[OWN]`, for 5.8's entries F1 and F2:
   - keyed by the catalogue entry (F1 or F2), the credential context,
     the cell with its protocol identifier and configuration generation
     (N27, point 6), and the server's address; entries never share a
     memo, so a refusal of one procedure says nothing about another;
   - only a refusal is stored. A memo is created only by the "unknown
     procedure" abort code (5.9, row R6) delivered as `Aborted` for that
     entry's preferred procedure on an eligible connection ([rx.md](rx.md),
     section 4, "Eligible connection"). No memo means "preferred"; a
     decoded reply to the preferred procedure deletes the memo, and
     creates nothing;
   - a memo lasts `fs.variant_memo_ttl`. The table is bounded by
     `fs.variant_memo.per_context_max` per context and
     `fs.variant_memo.total_max` in all (5.2); at either cap the least
     recently used memo is evicted first. An evicted memo only makes the
     next call try the preferred procedure again, so eviction never
     weakens anything and a flood of memos costs at most the table's
     fixed size;
   - it is a hint (N5, point 2): it chooses only between variants that
     keep the same guarantees (N5, point 3), it is never shared with
     another context (ADR 0006, decision 1, point 5), and it never
     changes a profile, a floor or a pin (N6, point 1);
   - when it expires, the next call tries the preferred variant again.
3. **Location cache.** A VL entry replaces the cached entry for that
   volume only when it passes 5.4, points 11 and 12. An abort code that
   asks for a fresh location (5.9) makes the client ask the VL service
   again; it never removes the cached entry by itself `[OWN]`, from ADR
   0006, decision 1, point 5: an unprotected abort evicts nothing.

### 5.7 Operation grants

The evaluator that turns an access mask and mode bits into operation
grants is one pure function in one module of `ironafs-proto`
`[ADR 0004 N28.2: Firm]`. Its inputs are one status record's
`file_type`, `caller_access` and `mode`, and nothing else: no owner, no
caller id, no other record. For each **mask operation** of point 4 that
applies to that `file_type`, its output says one of three things:
**granted**, **denied**, or **undecided** (the server decides, point 7).
The grant is the set of granted operations. The core sees only that
output `[ADR 0004 N28.1: Firm]`. The same module holds one more pure
function, the rights report (point 4), which reads only `caller_access`.

1. **The rights the server defines** for a directory's ACL `[V6
   fs_setacl(1), "-acl": UNVERIFIED]`, in IronAFS's own words:
   - `r`: read the contents of files in the directory, and see their
     attributes;
   - `l`: list the directory, see the directory's own attributes, and
     see its ACL;
   - `i`: add entries; `d`: remove or move entries out; `w`: change the
     contents and mode of files in it; `k`: set locks on files in it
     `[DES-052]`; `a`: change the ACL;
   - `A` to `H`: no meaning to the server; for applications
     `[V6 fs_setacl(1), "-acl": UNVERIFIED]`.
2. **Known bits.** The bit position of each letter in an access mask is
   MISSING (Q7). The evaluator's table of known bits is filled only from
   a capture that sets one right at a time and reads the mask back
   (Q7). Until it is filled, the table is empty, so every bit is unknown
   and the evaluator grants nothing: every mask operation is undecided,
   and goes to the server (point 7). This is the safe state, not a
   placeholder that guesses.
3. **Unknown bits grant nothing** `[ADR 0004 N28.3: Firm]`. A bit not in
   the known table, and each of `A` to `H` once their bits are known,
   adds no operation, whatever other bits are set. Setting or clearing
   an unknown bit never changes the output, granted, denied or
   undecided. The test is `unknown_bits_grant_nothing` (10.5, G1).
4. **The read-only M2a operations** `[OWN]`, each from point 1's lead
   and C22. For a mask operation, "needs" means the known bits of the
   object's own `caller_access`, from a status record fetched in the
   reader's own context and generation, and for a file also point 5's
   mode rule. With all the needed bits known: all set (and the mode rule
   met) is granted, any clear is denied. With any needed bit not yet in
   the known table, the operation is undecided.

   **Mask operations**, the evaluator's output:

   | Operation | Object | Needs | Lead |
   |---|---|---|---|
   | `lookup` (resolve a name in it, use a cached lookup result) | directory | `l` | `[V6 fs_setacl(1), "l": UNVERIFIED]`; C22, point 6 |
   | `read_directory` (open, list, resume a listing, fetch the directory object) | directory | `l` | as above; C22, points 6 and 7 |
   | `search` (an `X_OK` access answer on a directory) | directory | `l` | as above |
   | `read` (an `R_OK` answer, open for reading, read cached data) | file | `r` and `l`, and the owner read bit | `[V8 HDRWQ580: UNVERIFIED]` `[ADR-017]` |
   | `execute` (an `X_OK` answer) | file | as `read`, and the owner execute bit | `[V8 HDRWQ580: UNVERIFIED]` |
   | `read_link` (read a symbolic link's target) | symbolic link | `r` and `l` | `[OWN]`: no source states it; the stricter of the file rules, without mode bits (point 5); Q7 checks it |

   **Answers that are not mask operations** (not the evaluator's output,
   and not counted by G1):

   | Answer | Object | Rests on | Lead |
   |---|---|---|---|
   | attributes (serve a status record's attributes) | any | the server returned the record to this caller, in its context and generation; a cached record is served only under a current `lookup` on every directory consulted (C22, point 6) | C22, points 1 and 6 |
   | file-system statistics | volume | a `fs.get_volume_status` reply to this caller, used once and not cached | `[OWN]` |
   | the rights report (C7's `rights`) | any | the rights report function: the known letters set in `caller_access`, and whether unknown bits are present, without naming them as rights; with the table empty it reports no letters and "unknown bits present" for any non-zero mask | `[OWN]`; N28, point 3 |

   No other operation is granted in M2a. Every write-side operation
   (`i d w k a`) is outside M2a and outside these tables; M2a is
   read-only (O50, point 1), and the write grants are the M3
   specification's.
5. **Mode restrictions** `[V8 HDRWQ580: UNVERIFIED]` `[ADR-017]`:
   - for a directory, mode bits narrow nothing;
   - for a file, only the owner's bits count: without the owner read bit
     no one reads the file, whatever the ACL, and without the owner
     execute bit `execute` is not granted;
   - for a symbolic link, mode bits narrow nothing `[OWN]`: no source
     says the server applies them to a link;
   - mode bits never add an operation `[ADR 0004 N28.4: Firm]`.
6. **Masks that grant nothing.** `anonymous_access` is recorded and
   shown in diagnostics; it grants nothing to anyone `[OWN]`, because a
   grant comes only from the caller's own mask (C22, point 1). Rights are
   never taken from a directory's mask for a file in it (C22, point 3):
   a file's grants come from the file's own status record, even where
   that record's mask was computed by the server from the directory's
   ACL (Q7).
7. **Undecided operations** `[ADR 0004 C22.2: Firm]`. No grant is
   minted for an undecided operation, and the reader's own fresh call
   decides `[OWN]`:
   - `read` and `read_link`: the reader's own data fetch (5.1.4,
     point 6);
   - `read_directory` and `lookup`: the reader's own fetch of the
     directory object; a name is resolved from that fetch alone;
   - `search` and `execute`: these are access answers, and no call can
     decide them, so they are not granted (the closing paragraph of this
     point).

   The reply's status record is evaluated again. Data is delivered under
   a grant if that evaluation grants the operation; if it is still
   undecided, the data the server returned to this caller is delivered
   to that caller once, for that one request, and nothing from it is
   cached. A denied operation sends no call. A server refusal of the
   call is the answer (5.9).

   An undecided case exists today only while the known-bit table is
   unfilled (point 2). The case where a server lets a file's owner read
   it without `r` in a directory where the owner has `l` and `i`
   `[V6 fs_setacl(1), "DROPBOXES": UNVERIFIED]` needs the owner and the
   caller's id, which the evaluator does not take; in M2a such a read is
   denied when the bits are known, and the ACL specification (M2b)
   decides whether it becomes undecided. An access answer
   (`R_OK`, `X_OK`) is given only from a grant `[ADR 0004 N28.4: Firm]`,
   so where this point applies the access answer is "not permitted",
   even though a read may then succeed. Q7 must be settled before M2a's
   acceptance, or every access answer on a real cell is "not permitted".
8. **What a grant binds.** The grant minted here carries, besides its
   operations, what C22, point 4, lists: the context and generation, the
   file id, the volume's incarnation (#78), the record's data version,
   and an expiry (C23, with the callback promise's validity from #66).
   This specification supplies the operations, the file id and the data
   version; the other parts come from those specifications.
9. **M2a and the null class.** Over the null class every reply is a hint
   (N5, point 2) and nothing unauthenticated is cached (C20). So on a
   real cell in M2a a grant is minted for one request and used once.
   The rule above is exercised as written in the simulator's
   authenticated test profile (O50, point 1; #96).

### 5.8 The fallback catalogue

Entries of N20's static catalogue for these procedures. Each states what
announces the preferred variant, the fallback, whether the fallback keeps
the same guarantees, and what may choose it `[ADR 0004 N20.1: Working]`.
No other fallback exists for these procedures (N20, point 2).

| Entry | Preferred | Announced by | Fallback | Same guarantees? | Chosen on |
|---|---|---|---|---|---|
| F1 | `fs.fetch_data64` | a decoded reply to it from that server (5.6, point 2). A capability flag in the file service's capability query also announces it `[V2 §4: UNVERIFIED]` `[V1 cap/viced.txt: UNVERIFIED]`; M2a does not read it (section 2, point 3) | `fs.fetch_data`, for ranges whose end is below 2³¹ (5.5, point 2) | yes, inside that range: the same data, the same status record, the same grant rule. Beyond it there is no fallback: the read fails with `not supported` (`RangeNotSupported`) | a hint: the "unknown procedure" abort (5.9, R6) on `fs.fetch_data64`; nothing else |
| F2 | `fs.inline_bulk_status` | a decoded reply to it from that server | `fs.bulk_status` | yes: every item still counts on its own (C22, point 1), and the reply is checked as a whole (5.4, point 10) | a hint, as F1: the "unknown procedure" abort on `fs.inline_bulk_status`, and nothing else `[ADR 0004 A8]`. Other failures of the inline call go to F3 or to 5.9, as F3 says |
| F3 | either bulk call: `fs.bulk_status`, or `fs.inline_bulk_status` when its failure is not F2's | always available as a request; no announcement | `fs.fetch_status`, once per file | yes: a single status call is the base case of C22, point 1 | any of: the "unknown procedure" abort on `fs.bulk_status`; `BulkCountMismatch`, `ReplyInvalid` or any rejected item of either bulk reply (5.4, point 10); an abort of either bulk call in rows R5, R8, R21 or R22, which concern one file or the encoding and so may not apply to the others. Every other abort of a bulk call is classified by 5.9 for the whole listing, and the per-file calls are not made: a volume or server failure would fail each of them the same way |
| V1 | `vl.get_entry_by_name`, `vl.get_entry_by_id` | — | none in M2a | — | — |

Entry V1 `[OWN]`: the registry marks these lookups as from IBM AFS 3.4
`[V1 rx/VL.txt: UNVERIFIED]`, older than every server in the capture
matrix (#51). An older lookup would add decoders for entries without the
server-identity form, for servers IronAFS does not support. A VL server
that answers "unknown procedure" fails the lookup with `not supported`.

On F1: [O40](../adr/0004-architecture-3-operations.md), point 1, makes the
64-bit fetch the default and the 32-bit fetch the fallback, "only when a
server lacks" the 64-bit one. That a server lacks it is learned only from
its abort (5.6, point 2); that the 64-bit procedures exist beside the
32-bit ones is UNVERIFIED (Q3). A forged "unknown procedure" abort can at
most make files of 2 GiB and more unreadable from one server, for one
context, for `fs.variant_memo_ttl` (section 8).

### 5.9 Abort codes

Every abort code is classified by the procedure and the service that
returned it, never by its number alone `[ADR 0004 C46.3: Working]`. Every
code is a hint (N5, point 2). Whatever it means, it acts only on the call
that received it, in the context that owns the connection, within ADR
0006, decision 1, point 5: it never marks a server down for other
contexts, never touches a token, never changes a profile, floor or pin,
and never removes a cached entry. An abort on a connection that is not
eligible changes only counters (ADR 0006, decision 1, point 6).

Values are from the registry's error tables `[V1 errors.html: UNVERIFIED]`;
which of them each procedure returns, and when, is UNVERIFIED until the
lab's error scenarios (Q10). The descriptions are IronAFS's own.

| Row | Code | Registry table | Procedures | Meaning to IronAFS | Contract error (C46) | Action `[OWN]` |
|---|---|---|---|---|---|---|
| R1 | −1 | Rx | all | the peer's Rx considers the connection dead | `timed out` | none beyond the outcome; Rx's own timeout rules |
| R2 | −3 | Rx | all | the peer timed the call out | `timed out` | retry once under the call's deadline (reads only) |
| R3 | −5 | Rx | all | the peer saw a protocol error | the generic failure (Q11) | counted; not retried on that connection |
| R4 | −100 | Rx and volume | all | the server is restarting | `busy` | retry with jittered backoff to the deadline (C33, point 7); for the VL service, try the cell's next VL server |
| R5 | −450 to −454, −456, −457 | rxgen | all | a marshalling failure at either end | the generic failure (Q11) | counted; on a bulk call, 5.8, F3 |
| R6 | −455 | rxgen | all | the server does not know the procedure | per 5.8: fallback for F1, F2, F3; `not supported` for V1 and for a non-catalogue procedure | for F1 or F2, sets that entry's variant memo (5.6, point 2); for F3 and V1, no memo |
| R7 | 101 | volume | file service | the volume needs salvage | `offline` | try another usable site of a read-only version, else fail |
| R8 | 102 | volume | file service | no such vnode | `stale` for a known handle; `not found` for a fresh lookup; on a bulk call, 5.8, F3 finds which file | none |
| R9 | 103 | volume | file service | the volume is not on this server, or not on line | `offline` | ask the VL service again (5.6, point 3), then retry once at the new site |
| R10 | 106 | volume | file service | the volume is off line | `offline` | as R7 |
| R11 | 110 | volume | file service | the volume is busy for a while | `busy` | as R4 |
| R12 | 111 | volume | file service | the volume has moved | `offline` | as R9 |
| R13 | 112 | volume | file service | a temporary I/O failure at the server | `busy` | as R4 |
| R14 | 113 | volume | file service | the volume is being salvaged | `busy` | as R4 |
| R15 | 105 | volume | file service | the volume is not in service | `busy` | as R4 |
| R16 | 120 | volume | file service | the volume's residency is restricted | `offline` | as R7 |
| R17 | 363 524 | VL | the two VL lookups | no such entry | `not found` | none; a negative entry is not cached unauthenticated (C20) |
| R18 | 363 527 | VL | the two VL lookups | the name is not a valid volume name | `not found` | none |
| R19 | 363 521, 363 547, 363 552 | VL | VL service | the VL server's own I/O, memory or database failure | `busy` | try the cell's next VL server (C33) |
| R20 | any other code in the VL table (363 520 to 363 552) | VL | VL service | a failure that a read-only lookup does not expect | the generic failure (Q11) | counted |
| R24 | any code, and any decode failure | any | `vl.get_addrs` | that one server cannot be resolved | none for the lookup: only that site is unusable (5.4, point 11) | counted; R4 and R19 also try the cell's next VL server for that call |
| R21 | 1, 13 | system range (below 256) | file service | permission refused | `access`; on a bulk call, 5.8, F3 finds which file | none; no grant is minted (5.7) |
| R22 | 2 | system range | file service | no such object | `not found`, or `stale` for a known handle; on a bulk call, 5.8, F3 finds which file | none |
| R23 | any other code | — | any | not classified | the generic failure (Q11) | counted with procedure and code, never retried in a loop |

`[OWN]`, and the reasons:

- Rows R4, R7 to R16, R21 and R22 apply to every file-service procedure
  in 5.1.2, whole bulk calls included; rows R19 and R20 to every VL
  procedure; R17 and R18 to the two lookups, and R24 to `vl.get_addrs`,
  which comes first for that procedure.
- Rows R7 to R16 are classified as volume codes only for file-service
  procedures, because volume codes collide with system error numbers
  `[DES-055]` `[V1 errors.html, "AFS Volume Package Errors": UNVERIFIED]`.
  The same number from another service falls to R23.
- Rows R21 and R22 read a code below 256 as a system error number
  `[V1 errors.html, "Reserved codes": UNVERIFIED]`. System error numbers
  differ between operating systems, and a server may translate them
  `[V3 §4: UNVERIFIED]`; only the three numbers above are read, as the
  values the capture matrix's Linux servers are expected to return, and
  each is UNVERIFIED until a scenario shows it (Q10). Every other number
  in that range falls to R23.
- No row turns an abort into success, an empty listing or zero bytes
  (C46, point 4).
- A code that IronAFS's M2b and later token work will see (the rxkad and
  token tables) is outside M2a; it falls to R23 here.

## 6. Arithmetic

Every value from a reply is untrusted. Checked arithmetic and fallible
conversions throughout (coding standards rule 2); nothing saturates.

| Id | Operation | Operands' range | Bound and result at overflow |
|---|---|---|---|
| A1 | assembling `length` or `data_version` into a `u64` from wire words (5.4, point 5) | two 32-bit words, or one 64-bit word (Q3, Q4) | shift and `checked_add` in `u64`; cannot overflow for two 32-bit halves; a signed word is converted with `try_from`, failure is `ReplyInvalid` |
| A2 | `offset + len` for a fetch request (5.5, point 2) | `offset` < 2⁶⁴, `len` ≤ `fs.fetch.chunk_max` | `checked_add` in `u64`; overflow is `RequestInvalid` |
| A3 | `length − offset` and `min(len, length − offset)` (5.4, point 9) | both `u64` | computed only when `offset < length`, so it cannot underflow; else the expected length is 0 |
| A4 | the 32-bit fallback's range test, `offset + len < 2³¹` (5.5, point 2) | from A2 | compared in `u64`; never narrowed before the test; the narrowing after it is `try_from` and cannot fail |
| A5 | the fetch reply limit, `len + 2048`, at most `fs.fetch.chunk_max + 2048` (5.1.4, point 6) | `len` ≤ `fs.fetch.chunk_max` (5.5, point 2) | `checked_add` in `u64`; cannot overflow after 5.5's check; the bound is a compile-time constant assertion |
| A6 | a bulk list's byte size, `n × 12` (5.5, point 2) | n ≤ `fs.bulk.fids_max` | `checked_mul`; cannot overflow |
| A7 | bulk item counts compared with the request (5.4, point 10) | from the decoder, each ≤ its array cap | compared as `usize`; no arithmetic |
| A8 | protection ids, wire word to `i32` (5.4, point 4) | one 32-bit word | `try_from` or a bit-exact reinterpretation chosen by Q6, never `as` |
| A9 | the variant memo's expiry, `now + fs.variant_memo_ttl` (5.6, point 2) | the monotonic clock's 64-bit nanoseconds (C37) | `checked_add`; overflow treats the memo as expired |
| A10 | a volume status count in bytes, `count × block size` (5.4, point 14) | count from the reply; block size from Q9 | `checked_mul` in `u64`, after a negative count is refused; overflow makes the count unusable |
| A11 | a UUID octet from its signed field, `v mod 256` (5.1.3, point 6) | v in −128 to 127, after the range check | `i8::try_from` then a bit-exact `u8` reinterpretation through `from_ne_bytes`; cannot fail after the check |

`[OWN]` for every row; each follows from the section it names.

## 7. Errors and failure

1. **One closed error set for this layer** `[OWN]`; tests assert the
   exact kind (coding standards rule 15). Each carries the procedure's
   IronAFS name and the cap or field name; none carries a value read
   from the reply (xdr.md, section 7, point 2):

   | Kind | When | Contract error (C46) |
   |---|---|---|
   | `ReplyInvalid` | the XDR decode failed (xdr.md, section 7), or a field rule of 5.4 failed | the generic failure (Q11) |
   | `UnknownFileType` | 5.4, point 3 | `not supported` |
   | `FetchLengthMismatch` | 5.4, point 9 | the generic failure (Q11) |
   | `FetchObjectMismatch` | 5.4, point 9: the reply's file type is not the kind planned | `stale` |
   | `SymlinkTooLong` | 5.4, point 9: a link over `fs.symlink.target_max` | `not supported` |
   | `RangeNotSupported` | 5.5, point 2: a 32-bit fetch ending at 2³¹ or beyond (5.8, F1) | `not supported` |
   | `NameInvalid` | 5.5, point 2: a volume name that cannot be sent | `not found` |
   | `BulkCountMismatch` | 5.4, point 10 | none: the files are asked again one by one (5.8, F3) |
   | `VlEntryMismatch` | 5.4, point 12 | the generic failure (Q11) |
   | `RequestInvalid` | 5.5, point 2 | the generic failure (Q11); a fault in IronAFS, counted and logged |
   | `Aborted` with a code | the Rx outcome; 5.9 classifies it | per 5.9 |
   | the other Rx outcomes | [rx.md](rx.md), section 7 | `timed out` for `Timeout` and `DeadlineExceeded`; `interrupted` for `Cancelled`; for `Fenced`, the result C16 gives a fenced request; the generic failure for the rest |

2. **The reads that fail without a call** `[OWN]`: `SymlinkTooLong` when
   the planning record already shows it, `RangeNotSupported` and
   `NameInvalid`, with the contract errors in the table.
3. **Fail closed.** A reply that fails any check mints nothing, caches
   nothing and serves nothing; the operation fails, or takes 5.8's
   fallback where one is listed (C46, point 4).
4. **No panic** is reachable from any reply (coding standards rule 1).

## 8. Security

Who can send what this specification decodes: the cell's VL servers and
fileservers; and, because M2a runs over the null class, anyone on the
path, who can forge or change any reply (N5, point 2; ADR 0004, A13 and
O23). Unprotected aborts are matched by Rx first ([rx.md](rx.md), 5.9;
ADR 0006). They can try:

| Threat | What stops it |
|---|---|
| Exhausting memory with large lists, names or data | per-message reply caps enforced by Rx while it buffers, field caps and memory budgets (5.2); the fetch data never more than asked (5.4, point 9) |
| Growing the variant memo table with forged aborts from many addresses | only refusals are stored, per catalogue entry; per-context and total caps with least-recently-used eviction, which only restores the preferred procedure (5.2; 5.6, point 2) |
| Fanning out `vl.get_addrs` calls from one entry | `vl.get_addrs.per_lookup_max` (5.2; 5.4, point 11) |
| A link or directory that grows between status and fetch, or a reply of another kind | caps and kind checked again on the reply's own record; links read whole (5.4, point 9) |
| Server text with terminal escapes | volume status strings shown only escaped, never parsed (5.4, point 14) |
| Widening a user's rights with extra mask bits | unknown bits grant nothing (5.7, point 3); grants only from the caller's own record (5.7, point 6); an empty known-bit table until captures fill it (5.7, point 2) |
| Using one user's reply for another | grants bind the context and generation (5.7, point 8; C22, point 4); variant memos are per context (5.6, point 2) |
| Mode bits that seem to allow more | mode bits only narrow (5.7, point 5) |
| A short or long fetch served as the file | 5.4, point 9, against the reply's own status record |
| A bulk reply with misaligned or missing items | count check; any rejected item discards the whole reply; per-file fallback (5.4, point 10; 5.8, F3) |
| A VL reply naming another volume, or steering to loopback, link-local, multicast or reserved addresses | 5.4, points 11 and 12 |
| A forged "unknown procedure" abort forcing the 32-bit fetch | the fallback keeps the same guarantees inside its range (5.8, F1); the effect is one context, one server, files of 2 GiB and more, for at most `fs.variant_memo_ttl` |
| A forged abort code used to make the client drop state or mark a server down | 5.9's preamble and ADR 0006, decision 1, point 5 |
| A forged VL entry or status over the null class steering a read to a hostile server, or changing data | not stopped in M2a: that is the null class's residual (N5; O23; A13). Nothing from it is cached (C20) or reaches an authenticated path (C19) |
| A data version that goes backwards to replay old bytes | never assumed monotonic; equality only; C25, point 4 handles a lower version (5.4, point 5) |
| Crashing the daemon | no panics, checked arithmetic (section 6); the fuzz targets (10.4) |

## 9. How the invariants are kept

| Invariant (section 1) | Kept by | Test that fails if it is broken |
|---|---|---|
| Constraint 4 | 5.2; 5.4; 5.6, point 2; section 6; section 7, point 4 | fuzz targets (10.4); reject corpus (10.3); C5 (10.5) |
| C6, points 1 and 4 | every point labelled; every layout MISSING until captures (5.1); section 13, Q1 | review of this specification (section 14); no codec for a MISSING layout merges (README, "Labelling claims", point 4) |
| C22, point 1 | 5.4, point 10; 5.7, points 4 and 6 | G3, G4 (10.5) |
| C22, point 2 | 5.7, point 7 | G6 (10.5) |
| C22, point 3 | 5.4, point 7; 5.7, point 6 | G5 (10.5) |
| C46, point 3 | 5.9 | E1, E2 (10.5) |
| C46, point 4 | 5.4, points 9 and 10; 5.9; section 7, point 3 | R8, R9, R10 (10.3); E3 (10.5) |
| N5, points 2 and 3 | 5.6, point 2; 5.8 | C1, C3 (10.5) |
| N20, points 1 to 5 | 5.8 | C1 to C4 (10.5) |
| N28, points 1 and 2 | 5.7, preamble | PROPOSED: an `xtask` check that only the evaluator's module and the codecs read `caller_access` and `mode` (gap, section 10.6) |
| N28, point 3 | 5.7, point 3 | `unknown_bits_grant_nothing` (G1) |
| N28, point 4 | 5.7, point 5 | G2 (10.5) |
| N31, points 1 to 3 | section 12; every wire fact UNVERIFIED or MISSING until a fixture | review; each fixture's provenance row (10.1) |
| N38, points 1 to 3 | 5.1.3; 5.4, points 4 and 5 | R11, R12 (10.3); the type checks of the compiler |
| O40, point 1 | 5.8, F1 | C1, C2 (10.5); the 2 GiB and 4 GiB boundary reads in the simulator (#96) |
| A8 | 5.8, F2 and F3 | C3 (10.5); Q5's capture |
| ADR 0006, decision 1, point 5 | 5.6, points 2 and 3; 5.9 | E2 (10.5) |

## 10. Tests

Paths: `crates/ironafs-proto/tests/vl.rs`, `crates/ironafs-proto/tests/fs.rs`,
`crates/ironafs-proto/tests/grants.rs`; fixtures under
`crates/ironafs-proto/tests/fixtures/vl/` and `.../fs/` (coding standards
rule 15).

### 10.1 Golden vectors

None yet. A golden vector needs expected bytes, and no layout here is
known (5.1). The capture-backed revision adds, per procedure, request and
reply fixtures reduced from the lab's stage C captures, with synthetic
names, ids and data, each with its fixture provenance (venue, server
release, date, scenario id) and a `docs/provenance.md` row if needed
([README](README.md), "Test vectors and fuzz targets", point 1). Each
vector varies one input from a known baseline (Q1).

### 10.2 Round-trip properties

Once the layouts are known: every request IronAFS encodes decodes, with a
test decoder written from this specification, to the values given (P1);
every accepted reply re-encodes, with a test encoder, to the bytes
received (P2: no field is accepted in two encodings, xdr.md, 5.4,
point 8).

### 10.3 Reject corpus

Each case is a reply or request that must fail, with the kind and the
section. The layout-dependent bytes are built from the fixtures once
they exist; the rules are fixed now.

| Case | Input | Expected | Section |
|---|---|---|---|
| R1 | a reply one byte over its cap | Rx `ReplyTooLarge`; nothing decoded | 5.2; 5.4, point 1 |
| R2 | a reply with trailing bytes | `ReplyInvalid` | 5.4, point 2 |
| R3 | a status record with an unknown file type | `UnknownFileType` | 5.4, point 3 |
| R4 | a VL entry name over 64 bytes, or holding NUL | `ReplyInvalid` | 5.4, point 11 |
| R5 | a VL entry for a different name, or without the id asked | `VlEntryMismatch` | 5.4, point 12 |
| R6 | a site at 0.0.0.1, 127.0.0.1, 169.254.1.1, 224.0.0.1, 240.0.0.1 or 255.255.255.255 (with the VL servers not at loopback) | the site unusable; no call to it | 5.4, point 11 |
| R7 | a UUID with a node field of 128 | `ReplyInvalid` | 5.1.3, point 6 |
| R8 | a fetch reply with one byte more than asked | `FetchLengthMismatch` | 5.4, point 9 |
| R9 | a fetch reply one byte short of what its own status record implies | `FetchLengthMismatch` | 5.4, point 9 |
| R10 | a bulk reply with one item fewer, or more, than asked | `BulkCountMismatch`; per-file calls | 5.4, point 10 |
| R11 | a status record whose data version or length words exceed the captured form | `ReplyInvalid` | 5.4, point 5 |
| R12 | a protection id word outside the captured form | `ReplyInvalid` | 5.4, point 4 |
| R13 | a request: a 65-byte volume name (`NameInvalid`); a volume id of 0, an empty or 33-item bulk list, a fetch of 0 bytes or over 1 MiB (`RequestInvalid`); a 32-bit fetch ending at 2³¹ (`RangeNotSupported`) | refused before sending | 5.5, point 2 |
| R14 | a symbolic link whose planning status length is 4 097 | `SymlinkTooLong`; no fetch sent | 5.4, point 9 |
| R15 | a link planned at 10 bytes whose fetch reply's own status says 4 097 | `SymlinkTooLong`; nothing served | 5.4, point 9 |
| R16 | a fetch planned for a link whose reply's status says directory | `FetchObjectMismatch` | 5.4, point 9 |
| R17 | a link planned at 10 bytes whose reply says 20, then 30 | one re-fetch, then `FetchLengthMismatch`; no prefix served | 5.4, point 9 |
| R18 | a bulk reply with the right counts and one item of unknown file type | the whole reply discarded; per-file calls | 5.4, point 10 |
| R19 | a volume status reply with a negative count, a count whose bytes overflow, a string holding NUL, or another volume's id | the count unusable and the answer failed, or `ReplyInvalid` | 5.4, point 14 |
| R20 | a VL entry with 9 candidate sites of the version asked, each naming a different server identity | 8 `vl.get_addrs` calls; the ninth site unusable. Two candidate sites naming one identity: one call | 5.4, point 11 |

### 10.4 Fuzz targets

In the top-level `fuzz/` crate (coding standards rule 20):
`ironafs-proto_vl_decode` and `ironafs-proto_fs_decode` decode arbitrary
bytes as each reply of 5.1.2, and assert: no panic; no allocation past
the memory budget (xdr.md's ledger); an accepted fetch reply holds at
most the length asked; an accepted bulk reply has the count asked.
`ironafs-proto_grants` runs the evaluator (5.7) on arbitrary masks,
modes and types, and asserts G1 and G2's properties on every input.

### 10.5 Acceptance cases

The grant cases run on the evaluator alone; the others in the simulator
(#96), each as a start state, the events, and the result.

- **G1 `unknown_bits_grant_nothing`.** For every mask m and every bit b
  not in the known table, and every mode and type: the output for
  `m | b` equals the output for `m & !b`, for every mask operation
  (granted, denied or undecided). With the table empty, every mask gives
  no granted operation, and every mask operation of the type is
  undecided. (N28, point 3.)
- **G2 mode narrows only.** For every mask, type and mode: the grant
  (the set of granted operations) with any mode is a subset of the grant
  with mode 0o7777. A file with the owner read bit clear gets neither
  `read` nor `execute`. A directory's whole output (granted, denied and
  undecided) does not depend on mode. (N28, point 4.)
- **G3 per item.** A bulk reply that passes, where item 1 grants `read`
  and item 2 does not: file 1 gets its grant, file 2 none. A bulk reply
  where item 2 is rejected: neither file gets a grant from it, and both
  are asked again alone. (C22, point 1; 5.4, point 10.)
- **G4 own context.** Two contexts list one directory; only the context
  whose status reply carried the mask holds the grant. (C22, point 1.)
- **G5 no inference from the directory.** A directory whose record
  grants `l` and `r`, and a file in it whose own record lacks `r`: the
  file gets no `read`. (C22, point 3.)
- **G6 the fetch decides.** With the known-bit table empty, a read, a
  link read and a listing each send a fresh fetch every time, deliver
  what the server returned to that caller once, and cache nothing; an
  `X_OK` answer is "not permitted". (C22, point 2.)
- **C1 64-bit default.** A server that answers `fs.fetch_data64` is
  never sent `fs.fetch_data`. (O40, point 1.)
- **C2 32-bit fallback.** A server that answers `fs.fetch_data64` with
  code −455: a read below 2³¹ uses `fs.fetch_data`; a read at 2³¹ and at
  2³² fails with `not supported`; after `fs.variant_memo_ttl` the 64-bit
  call is tried again; another context still tries the 64-bit call
  first; the same server's inline bulk call is still tried first (the
  memo is per entry). (N20; O40, point 1.)
- **C3 bulk fallbacks.** A server that answers the inline bulk call with
  −455 gets the bulk call; one that answers both with −455 gets one
  status call per file; one that returns a short bulk list, an
  undecodable one or one with a rejected item gets one status call per
  file; one that answers the bulk call with 103 gets no per-file calls,
  and the listing follows R9. (A8; N20.)
- **C5 memo bounds.** −455 aborts from 2 000 distinct server addresses
  in one context leave at most `fs.variant_memo.per_context_max` memos,
  and from many contexts at most `fs.variant_memo.total_max`; a decoded
  reply to the preferred procedure deletes its memo. (Constraint 4.)
- **C4 no fallback for the VL lookups.** A VL server answering −455 to
  `vl.get_entry_by_name` fails the lookup with `not supported`; no other
  lookup is sent. (N20, point 2.)
- **E1 classified by service.** Code 103 from a fileserver asks the VL
  service again; code 103 from a VL server is R23's generic failure.
  (C46, point 3.)
- **E2 an abort acts narrowly.** Code 111 on one context's fetch makes
  that context ask the VL service again; the cached entry stays until a
  valid VL reply replaces it, and another context's calls to that server
  are unchanged. (ADR 0006, decision 1, point 5.)
- **E3 failure never looks like success.** Code 102 on a fetch of an open
  file fails the read with `stale`; it never returns zero bytes. (C46,
  point 4.)

### 10.6 Gaps

- Every golden vector, and every reject case's bytes: section 13, Q1
  (#65's capture-backed revision).
- The `xtask` check that confines the evaluator (N28, point 2): to be
  added with the evaluator's code; its issue is to be filed with the
  first `ironafs-proto` pull request that adds the evaluator.

## 11. Revisit if

From the working decisions this rests on:

- **N20:** an optional RPC needed for a supported cell has a fallback
  that cannot be described by the catalogue's fields.
- **O40:** the specification work for the file service (C6) shows a
  server behaviour that contradicts a point here (for example, no 64-bit
  fetch on a server in the capture matrix).
- **C46:** a layer's error mapping cannot express a contract error
  without losing what the user needs to act on it.
- **N29:** the specification for the callback service finds a callback
  whose effect cannot be stated as grant revocation plus scope.
- **C25:** the specification gate (C6) shows no reply field tells a
  restored volume from the original, or shows one that makes point 3
  unnecessarily broad.
- **C33:** (point 7, the location cache) the M2 tests show an
  out-of-scope query on any supported configuration.
- **A8:** the capture of a listing shows a different order of use.

This specification's own choices are reopened if:

- a capture shows a server whose access mask already applies the owner
  mode bits, or one that does not apply them at all (5.7, point 5);
- a server in the capture matrix answers a bulk call larger than 32 file
  ids, or refuses one smaller (`fs.bulk.fids_max`);
- a server needs the older VL lookups (5.8, V1);
- fetch chunks of 1 MiB measurably slow reads (`fs.fetch.chunk_max`).

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| X1 | SPECIFICATION | "Rx protocol specification draft", N. Zeldovich, updated by J. Altman, 24 July 2026, register of editions row E5 ([README](README.md)). Cited by heading: "RPC Layer", "Rx Connections". Read only as the mechanically excerpted copy; its section "Historical Implementation Notes" was not read and is not cited (README, "Sources and provenance", rule 12). A lead only: every fact from it is UNVERIFIED (rule 14) | 3, 4, 5.1.1 |
| V1 | DOCUMENTATION | AFS Assigned Numbers Registry, register of editions row E7: procedure-number tables `rx/VL.txt` and `rx/RXAFS.txt`, the Rx service list `rxservice.txt`, the error tables `errors.html` and `et/VL.html`, the fileserver capability list `cap/viced.txt`, retrieved 2026-11-21. A lead until a passage is allowlisted (section 13, Q2) | 5.1.1, 5.1.2, 5.8, 5.9 |
| V2 | SPECIFICATION | T. Keiser et al., "AFS-3 Protocol Capabilities Query Mechanism", draft-keiser-afs3-capabilities-00, register of editions row E9: §3, §3.1, §3.2 and §4 prose only. Appendices A and B are excluded by name. A lead only | 5.1.2, 5.1.4, 5.8 |
| V3 | SPECIFICATION | as V2, §4, on the insert right named by one capability flag, and on error translation | 5.1.3, 5.9 |
| V4 | SPECIFICATION | T. Keiser and A. Deason (ed.), "AFS-3 Rx RPC XDR Primitive Type Definitions", draft-keiser-afs3-xdr-primitive-types-01, register of editions row E8: §3, §4.1 and §4.2 prose only; the structure shown in §4 is excluded by name. A lead only | 5.1.3 |
| V5 | DOCUMENTATION | OpenAFS 1.8.1 release notes, register of editions row E12. A lead only | 5.1.4 |
| V6 | DOCUMENTATION | OpenAFS reference page `fs_setacl(1)`, sections on the `-acl` option and "DROPBOXES", register of editions row E10. A lead only (rule 11) | 5.7 |
| V7 | DOCUMENTATION | OpenAFS reference pages `vos_examine(1)` ("OUTPUT") and `vos_create(1)` (the name length), register of editions row E10. A lead only | 5.1.3, 5.1.4, 5.2, 5.4 |
| V8 | DOCUMENTATION | OpenAFS Administration Guide, "How AFS Interprets the UNIX Mode Bits" (HDRWQ580) and "About Volumes" (HDRWQ177), register of editions row E11. A lead only | 5.7 |
| C | DECISION | [Claim record](../reviews/2026-11-20-adr-0004-claims.md) rows `[DES-011]`, `[DES-052]`, `[DES-055]`, `[DES-057]`, `[DES-058]`, `[DES-076]`, `[ADR-008]`, `[ADR-017]`, re-read against their sources on 2026-11-21, except `[DES-058]` (section 12.2) | 5 |
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; A8, A13; C6, C7, C11, C16, C19, C20, C22, C23, C25, C33, C37, C38, C46 ([part 1](../adr/0004-architecture-1-core.md)); O23, O40, O50 ([part 3](../adr/0004-architecture-3-operations.md)); N5, N6, N20, N27, N28, N29, N31, N38 ([part 4](../adr/0004-architecture-4-negotiation.md)) | 1, 2, 5, 8, 9, 11 |
| D2 | DECISION | [ADR 0006](../adr/0006-rx-unprotected-packets.md), decision 1, points 5 and 6 | 1, 5.6, 5.9, 8 |
| D3 | DECISION | [rx.md](rx.md) and [xdr.md](xdr.md) | 4, 5, 7 |
| D4 | DECISION | [ADR 0001](../adr/0001-licence-and-dont-copy.md), "Don't copy"; [coding standards](../coding-standards.md), rules 1 to 3, 8, 15 and 20 | 5.4, 6, 7, 10, 12 |
| O | OWN | every point tagged `[OWN]`, with its reason in place | 5, 6, 7 |

**What was not used.** No source code, test data, interface definition or
generated definition of any AFS or Rx implementation was read for this
specification, and none is cited. No copy of the 1991 AFS-3 programmer's
references was used: the drafts above cite them as donated developer
documentation `[V4 §1]`, but no copy whose provenance is independent of
every implementation's repository has been found, so none is registered
(README, "Sources and provenance", rule 10; section 13, Q12). The
registry's tables, the drafts and the documentation pages are cited for
facts only, in IronAFS's own words; none is quoted, and no figure,
structure, constant name or example from any of them is reproduced. The
names of caps, fields, grants and errors here are IronAFS's own; the
procedure names in 5.1.2's second column are the registry's names for
the procedures, used only to identify them. Two drafts embed code
fragments under another licence: the structure in V4 §4, and V2's
Appendices A and B. They are excluded by name in the register.

### 12.1 Observed behaviour

None yet. The capture-backed revision records the observation lab's
stage C captures here (#51): a test venue with synthetic cell names,
users, keys and data; raw bytes first, a protocol analyser only as a
later cross-check; each observation with its venue, server release,
date, scenario and the evidence label EXECUTED.

### 12.2 Every procedure and field, with its source

The acceptance table #65 asks for. "Register" is the row in
[README](README.md)'s register of editions; "Corroboration" is the
independent source or capture that would confirm the fact, and its state.

| Item | Fact | Source and register | Corroboration | Label |
|---|---|---|---|---|
| Framing | procedure number, then XDR arguments; XDR results | X1 "RPC Layer", E5 | stage C capture of any call (Q1) | UNVERIFIED |
| Framing | server errors positive, Rx errors negative | V1 errors.html, E7; `[DES-055]` | stage C error scenarios (Q10) | UNVERIFIED |
| Services | file service 1 at 7000, VL service 52 at 7003 | V1 rxservice.txt, E7; `[DES-011]` | stage C capture | UNVERIFIED |
| Numbers | 526, 527, 533 (VL); 130, 132, 149, 155, 65536, 65537 (file service) | V1 rx/VL.txt, rx/RXAFS.txt, E7 | stage C capture of each call; Q2 | UNVERIFIED |
| Number | −455, "unknown procedure" | V1 errors.html, E7 | a stage C call of an unassigned number (Q10) | UNVERIFIED |
| Abort codes | every other code in 5.9 (Rx −1, −3, −5, −100; rxgen −450 to −457; volume 101 to 120; VL 363 520 to 363 552; system 1, 2, 13) | V1 errors.html and et/VL.html, E7; `[DES-055]` | stage C error scenarios (Q10) | UNVERIFIED |
| 64-bit fetch | the 64-bit fetch exists beside the 32-bit one, with 64-bit offsets | V1 rx/RXAFS.txt, E7; V2 §4, E9 | stage C capture of both calls (Q3) | UNVERIFIED |
| File id | three unsigned 32-bit values | D1, N38.2 (decision); layout MISSING | Q1, Q6 | MISSING |
| Status record | every field of 5.1.3, point 2 | none admissible | Q1, Q4, Q6, Q7 | MISSING |
| Data version | 64-bit | D1, N38.1 (decision). `[DES-057]` is cited through the claim record only: its source draft is not cited for any fact here, so it has no register row | Q4 | UNVERIFIED (wire) |
| Callback promise, volume sync | fixed-size records | none admissible | Q1 | MISSING |
| VL entry | the roles of 5.1.3, point 5; a 0 id means no version | V7 vos_examine(1), E10; `[ADR-008]` | Q1, Q8 | UNVERIFIED (roles); MISSING (layout) |
| Volume name | at most 22 characters for a read/write volume | V7 vos_create(1), E10 | not load-bearing: the cap is IronAFS's own | UNVERIFIED |
| UUID | eleven XDR integers with range checks | V4 §4.1, §4.2, E8 | Q8 | UNVERIFIED |
| Address | four-octet IPv4 | `[DES-058]`, through the claim record only (a mailing-list post, rule 13; not re-read for this version, so no register row) | Q1 | UNVERIFIED (wire) |
| Bulk calls | both exist; inline preferred; per-item checks | V1, E7; `[DES-076]`; V5, E12 | Q5 | UNVERIFIED |
| Rights letters | the meanings of `r l i d w k a`, and `A` to `H` | V6 fs_setacl(1), E10; `[DES-052]` | Q7 | UNVERIFIED (meaning); MISSING (bits) |
| Mode rule | owner bits only; read needs `r`, `l` and the owner read bit | V8 HDRWQ580, E11; `[ADR-017]` | Q7 | UNVERIFIED |
| Dropbox reads | an owner may read a file without `r` | V6 "DROPBOXES", E10 | Q7 | UNVERIFIED |
| Volume status | quota, usage, partition sizes, name, messages | V7 (the reference pages' displays), E10 | Q9 | MISSING (layout) |

**Re-check of the cited claims (2026-11-21).** `[DES-011]`, `[DES-055]`
and `[DES-076]` were re-read at their sources (the registry's service
list and error page, and the 1.8.1 release notes): each still says what
the claim record states. `[DES-057]`: the extended callback draft
(draft-benjamin-extendedcallbackinfo-02) still shows a 64-bit data
version in its own records; it does not define the file service's status
record, so it does not settle Q4. `[DES-052]`, `[ADR-008]`, `[ADR-017]`:
the `fs_setacl(1)` page and the two Administration Guide pages still say
what the claim record states. `[DES-058]` (a mailing-list post) was not
re-read; it is a lead either way (rule 13). No contradiction with ADR
0004 N28, N31, C22 or O40 was found.

## 13. Open questions

Each is settled by the observation lab's stage C (#51) unless it says
otherwise. Q1 blocks all code written from this specification.

- **Q1. The capture-backed revision.** Every layout in 5.1.3 and 5.1.4
  is MISSING. The second version of this specification derives each
  from raw bytes of the capture matrix's baseline (#51), varying one
  input at a time against known values: a file, a directory and a link
  of known sizes, ids and modes; one ACL right at a time; a volume with
  read-only and backup versions; a read-only site not yet released.
  Every field more than one reading fits stays open. Until then no
  codec for these procedures merges, and #65 stays open (its "Order"
  paragraph). Recommendation: run it as one campaign with Q3 to Q10.
- **Q2. Allowlisting the registry.** The registry (E7) is a published
  list of assigned numbers, kept apart from any implementation's
  repository, and the issue's failure case names "a published source"
  as acceptable for procedure numbers. But some of its rows name an
  implementation's change history as their source, and the register's
  rule 14 needs an allowlisted passage. This version allowlists nothing,
  as the directory format did for its draft. Recommendation: allowlist
  the "Code" column of `rx/VL.txt` and `rx/RXAFS.txt` for rows whose
  source column names a release or a registration request (every row
  used here), the service ids and ports in `rxservice.txt`, and the code
  values in `errors.html` and `et/VL.html`, for those numbers only; keep rows that cite implementation history as
  leads. It is the owner's decision on clean-room policy (ADR 0001);
  either way the captures confirm each number used.
- **Q3. The two fetches.** Do both exist on every server in the matrix?
  The width and signedness of their offset and length arguments, the
  width of the returned data length, whether the data is padded to a
  unit, and the order of the results around the data. Settled by
  fetches at offsets 0, 2³¹ − 1, 2³¹, 2³² − 1 and 2³², of files across
  those sizes (O40, point 1's boundaries).
- **Q4. The 64-bit data version on the wire.** Whether, and how, the
  status record carries more than 32 bits of it. A version above 2³²
  cannot be reached by stores in a lab. Recommendation: craft a volume
  dump with a high data version from the registry's published dump
  format (`dump/dump_format.txt`, to be registered when used), restore
  it in the lab, and fetch the status.
- **Q5. The bulk calls.** Are reply items positional? How does the inline
  variant report a failed item, and the plain variant a failed call?
  What is each server's bound on the list (`fs.bulk.fids_max`)? Is the
  inline call accepted after a listing on every server in the matrix
  (A8)?
- **Q6. Signedness of ids.** Volume ids and vnodes of 2³¹ and above (a
  volume created with a chosen id; a vnode number past 2³¹ may not be
  reachable, and if not, N38, point 2 keeps IronAFS's reading as
  stated); protection ids that are negative (a group as a file's owner
  or group, set by an administrator). Settles N38, point 2's
  confirmation before any format that stores them is frozen.
- **Q7. The access mask.** The bit of each rights letter, and of `A` to
  `H`, in `caller_access` and `anonymous_access`; whether a file's mask
  is the caller's rights on its directory's ACL; whether the server
  already applies the owner mode bits to it; what a dropbox owner's mask
  shows; and, for a symbolic link, which rights its target read needs
  and whether its mode bits matter (5.7, point 4's `read_link` row).
  Settled by one ACL right at a time, then each mode bit, as one user,
  on a file, a directory and a link. Until settled the known-bit table
  is empty (5.7, point 2).
- **Q8. Sites and server identity.** How a site names its server, the
  site table's size and how its valid slots are counted, the flag bits,
  and `vl.get_addrs`'s arguments and results; whether the wire entry
  carries a release clone id. Also which reading of the
  UUID draft's clock-sequence range a server sends (5.1.3, point 6):
  IronAFS checks the 8-bit range and widens it only if a capture shows a
  server sending more.
- **Q9. Volume status.** Which fields `fs.get_volume_status` returns,
  their units (blocks of what size) and signedness, whether the reply
  names the volume id, and whether its strings are XDR strings.
- **Q10. Error scenarios.** Which codes each procedure returns for: a
  missing vnode, a missing volume, an off-line volume, a moved volume, a
  busy volume, a refused permission, an unknown procedure number, and an
  unknown VL entry. Confirms or corrects 5.9's rows and their UNVERIFIED
  labels.
- **Q11. The generic failure.** C46's list "includes" its values, and
  names none for "the server failed in a way the contract cannot
  classify". Several rows here need one. Recommendation: the contract
  specification (C7) adds an I/O failure value, reported as `EIO`, an
  `NSError` with POSIX `EIO`, or `STATUS_UNEXPECTED_IO_ERROR`, with the
  procedure and code in the side channel. It is a contract decision, not
  this specification's.
- **Q12. The 1991 references.** The AFS-3 programmer's references for
  the file server and cache manager interface and for the volume server
  and volume location server interface may state the 1991-era layouts.
  No copy whose provenance is independent of every implementation's
  repository has been found, so none is registered or read (README rule
  10). A qualifying edition, if found, is registered and corroborated
  before use; until then observation decides. This does not block Q1.

## 14. Review

The review record is
[2026-11-21-m2a-65-vl-fs-read](../reviews/2026-11-21-m2a-65-vl-fs-read.md).
