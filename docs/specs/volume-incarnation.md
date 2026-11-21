<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Volume incarnation evidence and the restore discriminator

Status: draft (2026-11-21). Raised in issue #78.

Kind: behaviour. It reads values that the VL and FS read specification
([vl-fs-read.md](vl-fs-read.md)) and the callback service
specification (#66) decode, and decodes no bytes itself. Class: S. It
decides when cached data may be served again, so a mistake serves stale
or wrong data ([README](README.md), "Review class per kind": a behaviour
specification touching the cache and callbacks).

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (I1, D1 and so
on) are in section 12.

**This is the documents-only version.** No observation exists yet. Every
fact here about what a server does on a move, release, restore or
re-creation is a lead from documentation, labelled UNVERIFIED, and every
wire layout it touches is MISSING in [vl-fs-read.md](vl-fs-read.md). The
capture-backed revision (section 13, Q1) waits on the observation lab's
stage B record (#51); the restore experiment against the test cell
(planned) records the answer of 5.7 as EXECUTED. Until then the answer of
5.7 stands as INSPECTED, and ADR 0004 C26, point 4's rule holds: no clean
chunk is reused across a restart.

## 1. What this implements

ADR 0004, C6, point 3, the second gate (**Firm**):

> - before the cache: what evidence identifies a volume's incarnation
>   (C25), the directory format and the callback procedures;

ADR 0004, C25, points 1 to 6 (**Working**; its "Revisit if" is in
section 11):

> 1. We assume a restored or recreated volume can repeat a data version
>    with different bytes. So the client keeps, per cell, server and volume, the
>    evidence that the volume is still the same one, with where each
>    observation came from.
> 2. Cached chunks are reused only when that evidence matches.
> 3. When it is ambiguous (after the volume is seen offline, a server
>    reset, a move, a release, or a restart of the client), data is fetched
>    again. The observing context starts a fresh local epoch for the file or
>    volume, so no old key in that scope can be used, and fetches in flight
>    are fenced.
> 4. A data version lower than the cached one is ambiguity about that file.
>    The observing context drops its own chunks for it and stops using the
>    file's shared entries. For other contexts and the pool the
>    observation is weak evidence: it marks their entries suspect (C24,
>    point 2). Evicting them, or changing the file's shared epoch, needs
>    authoritative evidence.
> 5. An incarnation change invalidates the volume's handles, directory
>    snapshots and Windows identities, and quarantines its pending journal
>    records for explicit recovery (C28, point 3; C29).
> 6. Read-only sites marked as an old release, or not yet released, are
>    excluded before replicas are ranked.

ADR 0004, C26, points 3 and 4 (**Working**):

> 3. Callbacks never persist across a restart.
> 4. Clean chunks are not reused across a restart, and are not kept for
>    that purpose, until the specification shows how a restore made while
>    the client was down can be detected. Once it does, chunks persist with
>    their full key, generation and provenance; an entry is reused only if
>    all still match, and every object starts as "known version,
>    unverified" and is revalidated on first use.

ADR 0004, C28, point 3 (**Firm**):

> 3. A **deferred store** (a record left by a stalled or failed commit, a
>    restart or an upgrade, or an upload of a change seen by the Windows
>    replica, L27, point 3) is never stored automatically. It is kept for
>    explicit recovery.

ADR 0004, C24, points 2 to 6 (**Firm**):

> 2. Anything weaker, including a reply protected only by a user's key and
>    everything the callback service receives before rxgk (C31), may
>    **suspend** callback promises, revoke grants and mark entries suspect.
>    A suspect entry is not delivered until its reader has revalidated it
>    under the reader's own credentials.
> 3. Weak evidence never evicts another context's data or the pool's,
>    never retires their handles (save point 6's purge-failure case) or
>    journal records, and never establishes
>    that two contexts see the same volume incarnation.
> 4. Evicting, retiring or changing the epoch of another context's
>    entries, of the pool's, or of shared incarnation state needs
>    authoritative evidence.
> 5. Suspect bytes and journal records are kept, not destroyed.
> 6. **What weak evidence may do across contexts** is this closed list:
>    suspend callback promises, revoke grants, mark entries suspect, and
>    purge operating-system caches (C16, point 4; C17). Each only forces
>    the reader to revalidate under its own credentials and obtain a fresh
>    grant. Each keeps bytes, epochs, handles and journal records. The one
>    further effect, after a demonstrated purge failure, is revocation of
>    the handles on that node (C15, point 6): the operating system's cache
>    for the node is shared, so local evidence, not the weak input, decides
>    it, and it keeps bytes, epochs and journal records. Anything else
>    across contexts falls under point 4.

ADR 0004, C16, point 1 (**Firm**):

> 1. **Record first, notify after.** Break counters, staleness and
>    credential generations change in the core before any event is queued.
>    Every later operation sees the change, whether or not a layer has
>    applied it.

ADR 0004, C20, point 1 (**Working**):

> 1. Data from an unauthenticated call lives only in that request's buffers:
>    no cache entry, no shared fetch, no reuse.

ADR 0004, N38, point 8 (**Firm**), its second part, on the incarnation
component of the stable object key:

> The incarnation component is a value drawn from a daemon-wide counter
> persisted in the state directory. It never repeats within one state
> directory; a new state directory or a declared restore (N11, point 6;
> O42, point 5) never reuses a key an earlier one issued (S5), and an
> undetected rollback of all checked state is O42, point 6's bound.
> It changes for a context when that context observes ambiguity (C25,
> point 3), and for a view shared by several contexts only on
> authoritative evidence (C24, point 4).

ADR 0004, O13, point 1 (**Working**), the part this specification
supplies:

> 1. `afs cache show <path>` reports what the client holds for that file: the
>    cached data version and the server's, the volume incarnation evidence,
>    […]

ADR 0006, decision 1, point 5, its last list (**Firm**):

> - Whatever its code says (C46, point 3), it never:
>   - marks a server down for other calls or other contexts;
>   - discards, expires or marks unusable a token or credential; […]
>   - settles, drops or replays a journal record;
>   - changes a negotiation profile, floor or pin (N5, point 3; N6,
>     point 1);
>   - raises callback trust (C24, point 7), counts as evidence that the
>     server reset the client's callback state, advances a suspect epoch,
>     evicts or retires anything (C24, point 3), or records absence
>     (C19, point 2).

ADR 0004, overview, constraint 4 (a constraint of the whole ADR), as
quoted in [vl-fs-read.md](vl-fs-read.md), section 1: untrusted input
everywhere; no panics, checked arithmetic and byte limits.

Section 9 shows where each is kept and which test fails if it is broken.

## 2. Scope

1. **Covered:**
   - which values from the replies M2a decodes are incarnation evidence,
     which are recorded only, and which are not evidence, with what the
     documentation says happens to each on a move, a release, a restore
     and a re-creation (5.1);
   - the answer to the gate's question: whether any of them tells a
     restore made while the client was down (5.7);
   - the evidence record each credential context keeps per volume, its
     provenance, its caps and its eviction (5.1.2, 5.2);
   - the comparison of a new observation with the record, and its four
     outcomes (5.4, 5.6);
   - the events that make the evidence ambiguous for the observing
     context, the inputs that only mark entries suspect, and the inputs
     that change nothing here (5.6);
   - how a suspect chunk is revalidated when the suspicion could hide a
     new incarnation (5.6, point 6);
   - what a restart of the daemon does to every record (5.8).
2. **Not covered, and where it is covered:**
   - the wire layout of every value read here: [vl-fs-read.md](vl-fs-read.md),
     whose 5.1.3, point 4, hands the volume sync record here unread;
   - the callback service, the callback state record, the suspect epoch
     and the fence tuple: the callback specification (#66), whose 5.6,
     points 1 to 5, this specification reads and never restates;
   - the node table, the chunk store and its keys, entry validation and
     the serve path: the cache (C16, C26, C27; #91 and the chunk store
     task). This specification states what the cache must compare and
     when;
   - what an incarnation change does to handles, directory snapshots and
     Windows identities: the contract specification (#67) and the layer
     specifications; to journal records: the journal specification (C28,
     C29; M3). This specification emits the change and names its scope;
   - the daemon-wide counter behind the local volume epoch, its
     persistence, and what a new state directory or a declared restore
     does to it: the stable object key's specification item S5 and the
     state directory's specification (N11, N38, point 8; O42, point 5);
   - the shared pool's incarnation state, which changes only on
     authoritative evidence (C24, point 4): not before rxgk (C21; N5,
     point 6). No shared record exists in this version (5.6, point 9);
   - the IPC and command-line form of `afs cache show`: the IPC
     specification (#62) and O13. This specification says what the
     evidence is and who may see it (5.6, point 11).
3. **Not in M2a's procedure set, and not used:** the volume server's
   procedures, which report a volume's creation, copy and update dates to
   administrators `[I1 vos_examine(1), "OUTPUT": UNVERIFIED]`. M2a calls
   only the VL and file services ([vl-fs-read.md](vl-fs-read.md), 5.1.2).
   Whether a client may use them, and whether they would help, is
   section 13, Q8.

## 3. The design in short

1. Data version identifies bytes only within one incarnation of a volume
   (C25, point 1). The documentation read gives no value in the replies
   M2a decodes that a restore, made while the client was down, is sure to
   change: an administrator can restore a dump over a volume in place,
   keeping its id, its site, and every file's vnode, uniquifier and data
   version (5.7). So the restore discriminator does not exist in this
   version, and nothing about a volume's incarnation survives a restart
   of the daemon (5.8; C26, point 4).
2. Within one run, each credential context keeps one bounded **evidence
   record** per volume: the version ids of its VL entry, the server and
   partition serving it, the release state of that site, the volume's
   type, and, recorded but not compared, the volume sync record. Each
   value keeps where it came from (C25, point 1).
3. Every reply the context itself decodes for that volume is compared
   with the record. Equal values confirm it. A different value or a
   release in progress is **ambiguity**: the context takes a new local
   volume epoch, so no chunk cached under the old one is reachable in
   that context, and emits an incarnation change for its own handles,
   snapshots and journal records (C25, points 3 and 5; N38, point 8). A
   volume with no record (never seen, evicted, or after a restart) gets a
   new epoch, so nothing cached under an earlier one is reachable. A
   reply that cannot be decoded is not an observation: it serves nothing
   and changes no record. Fail safe: whenever the evidence cannot show
   the volume is the same, nothing cached under the old epoch is served.
4. Nothing another context, the callback service or an Rx abort reports
   can change a context's epoch (C24; ADR 0006). Those inputs only mark
   entries suspect, through the fence tuple the cache compares: another
   context's ambiguity raises the volume's break counter, and the
   callback service moves the counters the callback specification names.
5. A suspect chunk whose suspicion is server-wide or volume-wide (a reset,
   a whole-volume break, another context's ambiguity, a pause, a suspend)
   is revalidated by fetching its bytes again in the reader's own
   context, not only its status. Equal data version and equal bytes keep
   the chunk. Equal data version with other bytes, or a lower data
   version, is the reader's own observation of a different incarnation
   (point 3). So a restore that such an event could hide is never served
   from the old bytes, and weak input never destroys or re-epochs
   anything. A server reset therefore costs a refetch, not an epoch
   change, unless the refetch shows a lower data version, or other bytes
   under an equal one (5.6, point 12).
6. The cost is stated in each place: a cold cache after every restart, a
   refetch of a whole volume after a move, a release, a replica switch or
   an evicted record, and, after a server-wide or volume-wide event, one
   fetch of each cached chunk on its next use.

## 4. Terms

- **Incarnation:** one continuous existence of a volume's contents under
  one volume id, within which a data version names one set of bytes. A
  restore over the volume, or its removal and re-creation under the same
  id, may begin a new incarnation that repeats ids and data versions
  (C25, point 1).
- **Restore discriminator:** a value the client can read that is sure to
  differ between an incarnation and any later one, including one made by
  a restore while the client was down (C26, point 4).
- **Observation:** one decoded reply, on one of the observing context's
  own connections, that carries evidence values for a volume: a VL entry
  ([vl-fs-read.md](vl-fs-read.md), 5.1.4, points 1 and 2) or a
  file-service reply with a volume sync record (its 5.1.4, points 4 to
  6). Only a reply that passed every check of vl-fs-read.md, 5.4, is an
  observation.
- **Observing context:** the credential context that owns the connection
  the observation arrived on (C25, point 3).
- **Evidence value:** a value of 5.1.1 marked "compared".
- **Evidence record:** the per-context, per-volume record of 5.1.2.
- **Local volume epoch:** the value of the record's `epoch` field. It is
  the incarnation component of N38, point 8, as that context sees the
  volume, and the "volume and its incarnation" part of a chunk's key
  (C27, point 1).
- **Outcome:** the result of comparing an observation with the record:
  **confirmed**, **ambiguous**, **new** or **unrecorded** (5.4).
- **Incarnation change:** what ambiguity causes in the observing context:
  a new local volume epoch and C25, point 5's effects there (5.6,
  point 3).
- **Weak input:** an input that is not the observing context's own
  observation: another context's observation, anything the callback
  service receives (C24, point 2), and an Rx abort (ADR 0006).
- **Wide suspicion:** a change, since a chunk was validated, in any
  shared value of its fence tuple: its volume's break counter (raised by
  the callback service, or by another context's ambiguity, 5.6, point 4),
  the server's suspect epoch, the pause generation or the resume
  generation (the callback specification, 5.6, point 2). A change in the
  object's own break counter alone is **narrow**.
- **Holder:** a handle, a directory snapshot or a journal record that the
  context holds for the volume; a record with a holder is **pinned**
  (5.2).
- **Verify by refetch:** fetching a chunk's byte range again in the
  reader's own context and comparing the bytes with the cached chunk
  (5.6, point 6).

## 5. Specification

### 5.1 Layout

This specification defines no wire or stored layout. 5.1.1 lists the
values it reads, by their IronAFS names in
[vl-fs-read.md](vl-fs-read.md), and what each is used for. 5.1.2 gives the
in-memory record. Neither is persisted (5.8).

#### 5.1.1 The values, and what changes them

What the documentation says happens to each value on each event. Every
entry in the four event columns is a lead from documentation and
UNVERIFIED until the capture-backed revision (Q1) observes it; "?" means
no document read says. "Kept" means the value can be the same after the
event as before it.

| Value (vl-fs-read.md name) | Role here | Move | Release | Restore over the volume | Removal and re-creation |
|---|---|---|---|---|---|
| volume id of the file id, and the id asked for | the key of the record; not evidence | kept `[I2 vos_move(1): UNVERIFIED]` | kept: the read-only id is allocated with the read/write volume `[I1 vos_create(1): UNVERIFIED]` | kept unless an id is given `[I2 vos_restore(1): UNVERIFIED]` `[I3 "About Restoring Volumes": UNVERIFIED]` | may be kept: an administrator can give the old id `[I1 vos_create(1), "-id": UNVERIFIED]` `[I3: UNVERIFIED]` |
| `volume_ids`: the read/write, read-only, backup and release clone ids of the VL entry | compared | ? | the release clone id appears during a release and is removed after it `[I2 vos_release(1): UNVERIFIED]` `[I1 vos_examine(1), "OUTPUT": UNVERIFIED]`; whether the wire entry carries it is vl-fs-read.md's Q8 | kept for a restore in place, unless an id is given `[I2 vos_restore(1): UNVERIFIED]` | ? : the read-only and backup ids are allocated when the read/write volume is created `[I1 vos_create(1): UNVERIFIED]` `[I2 vos_backup(1): UNVERIFIED]`; whether they can be chosen too is not stated |
| the serving site: `site.server` (identity or address) and `site.partition` | compared | changes `[I2 vos_move(1): UNVERIFIED]` | ? for the read-only sites; their set changes only by adding or removing a site | kept for a restore in place; changes for a restore to a new site `[I2 vos_restore(1): UNVERIFIED]` | may be kept |
| `site.flags`: the release state of the serving site | compared: any release flag is a release in progress | ? | new release and old release flags are set during a release and cleared after a complete one; a new site is not released until its first release `[I2 vos_release(1): UNVERIFIED]` `[I1 vos_examine(1), "OUTPUT": UNVERIFIED]` | ? | ? |
| the volume's type (read/write, read-only, backup), from the id asked and the entry | compared | kept: a move is of a read/write volume `[I2 vos_move(1): UNVERIFIED]` | kept: a release makes read-only copies `[I2 vos_release(1): UNVERIFIED]` | the restored volume is read/write, unless flagged read-only `[I2 vos_restore(1): UNVERIFIED]` | may be kept |
| `entry_flags` | recorded only (5.4, point 5); its bits are vl-fs-read.md's Q8 | ? | ? | ? | ? |
| the volume sync record, as words | recorded only (5.4, point 5) | ? | ? | ? | ? |
| a file's `data_version` | not volume evidence; C25, point 4, and 5.6, point 6 | kept: a move and a release carry the volume's contents as a dump stream `[I4 "Summary": UNVERIFIED]`, which carries each file's data version `[I4 "Vnode Data Version": UNVERIFIED]` | kept, as for a move | kept: a full dump carries each file's data version `[I4 "Vnode Data Version": UNVERIFIED]` | restarts: a new file's data version starts at zero `[I4 "Vnode Data Version": UNVERIFIED]` |
| a file's vnode and uniquifier | not volume evidence; part of the file id | kept, as for the data version `[I4 "Summary", "Full vs Incremental Volume dumps": UNVERIFIED]` | kept, as for a move | kept: a dump carries both, and the volume's next uniquifier `[I4 "Full vs Incremental Volume dumps", "Volume Next Uniquifier": UNVERIFIED]` | ? : a new volume is empty `[I1 vos_create(1): UNVERIFIED]`, so its files take vnodes and uniquifiers anew, and no document read says they cannot repeat an old pair |
| a file's times, length and mode | attributes only | ? | ? | ? | ? |
| the volume's dates (creation, last update, copy) | not read by M2a (section 2, point 3) | the copy date is the time the volume was created on its present server and partition `[I1 vos_examine(1), "OUTPUT": UNVERIFIED]` | a read-only volume's creation date is its last release time `[I1: UNVERIFIED]` | the creation date is the restore time `[I3 "About Restoring Volumes": UNVERIFIED]`, or, by the administrator's choice, the dump's date or the old volume's `[I2 vos_restore(1), "-creation": UNVERIFIED]`; the last update date likewise `[I2 "-lastupdate": UNVERIFIED]`; the copy date is not stored in dumps `[I1: UNVERIFIED]` | new |
| the fields of `fs.get_volume_status` (name, quota, usage, messages) | not evidence: they change for reasons other than incarnation, and a dump carries the name `[I4 "Volume Name": UNVERIFIED]` | | | | |

`[OWN]`, the reason for each role:

1. **The volume id keys the record and is not evidence.** It is how the
   client names the volume, and every row above says it can be kept
   across a new incarnation.
2. **The VL version ids are compared.** A re-creation that does not
   choose every id, and a release in progress (the release clone id), show
   in them. They cost four words.
3. **The serving site is compared,** so a move, a restore to a new site,
   and a switch between replica sites are each ambiguity (C25, point 3,
   names a move). A switch between replica sites of one release is
   ambiguity too, though it may be the same incarnation: no value read
   here shows that two sites hold one release (Q6). The cost is a refetch
   of that volume's data in that context after a replica failover.
4. **A release flag on the serving site is ambiguity, not just a change.**
   C25, point 3, names a release; the flags are the release as the VL
   entry shows it. A site flagged old release or not released is never
   served at all (C25, point 6; [vl-fs-read.md](vl-fs-read.md), 5.4,
   point 11).
5. **The volume sync record is recorded, not compared,** until the
   capture-backed revision says what its words mean (Q2). Comparing words
   of unknown meaning could make every store an incarnation change; not
   comparing them loses nothing this version relies on.
6. **Per-file values are not volume evidence.** A restore keeps them
   (I4), and a re-creation can repeat them. They stay in the chunk key
   (C27, point 1), and a lower data version is C25, point 4's file-level
   ambiguity, applied by the cache.

#### 5.1.2 The evidence record

One per (credential context and its generation; cell with its protocol
identifier and configuration generation, N27; volume id). In memory only
(5.8). Its fields `[OWN]`, each of fixed size, with the width each takes
in the size bound of 5.2:

| Field | Type | Bytes | Meaning |
|---|---|---|---|
| key | context id and generation (`u64` each), cell index (`u32`), the cell's protocol identifier (`u32` code) and configuration generation (`u64`), volume id (`u32`) | 36 | the record's key (C40, point 3, and N27 put the protocol identifier beside the cell) |
| `epoch` | `u64` | 8 | the local volume epoch, from N38, point 8's counter |
| `vl_ids` | four optional `u32` (read/write, read-only, backup, release clone), each a presence octet and a value | 20 | 5.1.1, row 2 |
| `volume_type` | closed enumeration: read/write, read-only, backup | 1 | 5.1.1, row 5 |
| `site_server` | tagged: a server identity (16 octets) or an IPv4 address (4 octets), one tag octet | 17 | 5.1.1, row 3 |
| `site_partition` | `u32` | 4 | 5.1.1, row 3 |
| `site_release` | closed enumeration: none, new release, old release, not released | 1 | 5.1.1, row 4 |
| `entry_flags` | `u32`, raw | 4 | recorded only (5.4, point 5) |
| `volsync` | at most `inc.volsync_keep_max` bytes, with a length octet, or "not kept" | 65 | recorded only (5.4, point 5) |
| `provenance` | two entries: one for the values a VL entry carries (`vl_ids`, `volume_type`, the site fields, `entry_flags`), one for `volsync`. Each holds the procedure that carried them ([vl-fs-read.md](vl-fs-read.md), 5.1.4's IronAFS name, as a one-octet code), the connection's security profile (N39's class, level and suite: one, one and four octets; "null class" in M2a), the boot-clock instant of the reply (C37, `u64`), and the server's suspect epoch when the call was sent (the callback specification, 5.6, point 1, `u64`) | 2 × 23 = 46 | C25, point 1's "where each observation came from"; shown by O13 (5.6, point 11) |
| `suspicion` | the last suspicion seen: closed enumeration (none; wide suspicion found at a serve) and the boot-clock instant it was found | 9 | for display only (5.6, point 11): it says when a serve last found one, not whether a chunk is suspect now. A chunk's own suspicion is its fence tuple (5.6, points 4 to 6), never this field |
| `last_outcome` | closed enumeration (confirmed, ambiguous, new), the trigger id of 5.6, point 2 (one octet, 0 for none) and the boot-clock instant | 10 | for display (5.6, point 11) |
| `holders` | `u32` count | 4 | the holders the context has on the volume; a record with a holder is pinned (5.2) |
| `last_used` | boot-clock instant, `u64` | 8 | for eviction (5.2) |
| total | | 233 | at most `inc.record_max` |

The record holds no value read from another context and no value from a
reply that failed any check.

### 5.2 Caps

The values read here are capped where they are decoded:
[vl-fs-read.md](vl-fs-read.md), 5.2 (the VL entry reply at 4 096 bytes,
its site table at 2 048 bytes, its name at 64 bytes, the UUID at eleven
words, every file-service reply that carries a volume sync record at its
reply cap), and the callback specification, 5.2. This specification adds
caps only on what it keeps `[OWN]`:

| Name | Value | Applies to | Why |
|---|---|---|---|
| `inc.volsync_keep_max` | 64 bytes | the copy of one volume sync record kept in a record | a guess: the record's size is MISSING ([vl-fs-read.md](vl-fs-read.md), 5.1.3, point 4; Q2). A longer one is "not kept", which changes no outcome while the record is not compared (5.4, point 5); Q2 resets the cap from the captured size |
| `inc.record_max` | 256 bytes | one evidence record, all fields | the fields of 5.1.2 at their widths sum to 233 bytes; a compile-time size assertion |
| `inc.records.per_context_max` | 1 024 records | the records one credential context holds | a context reads few volumes at once; each record is charged to the context's metadata budget (C27, point 4) |
| `inc.records.total_max` | 65 536 records | the records the daemon holds beyond each context's floor | bounds that part of the table at 16 MiB whatever the number of contexts |
| `inc.records.floor_per_context` | 32 records | the records a context with a floor may always hold, outside `inc.records.total_max` | C27, point 4's floor: no other context's use can take them, so a context with a floor can always record its first 32 volumes. The floor is charged to the context's metadata budget |
| `inc.records.floors_max` | 1 024 floors | how many contexts hold a floor at once | bounds the floors at 32 768 records (8 MiB), whatever the number of contexts, so the whole table is at most `inc.record_max` × (`inc.records.total_max` + `inc.records.floors_max` × `inc.records.floor_per_context`) = 24 MiB (A3). A floor is granted to a context when it first needs a record, in that order, and returned when its generation retires (point 4); while all floors are taken, a further context has no floor and takes every record from the shared part (point 3) |
| `inc.records.idle_reclaim_after` | 600 s | how long a context's records must go unused before another context may reclaim its unpinned records above its floor | C27, point 4: idle contexts are evicted first |

Eviction and the full table `[OWN]`:

1. A record with a holder is **pinned**: it is never evicted. The owner
   of each holder (the contract specification for handles and
   snapshots, the journal specification for records) raises and lowers
   `holders`. Pinned records count against the caps like any other.
2. **The floor.** A context that holds a floor (`inc.records.floors_max`)
   keeps its first `inc.records.floor_per_context` records in it: they
   do not count against `inc.records.total_max`, and nothing but the
   context's own activity evicts them. A context's other records are its
   **shared part**, which does count. A context without a floor has only
   a shared part.
3. **Making room,** when a new record is needed, by the first branch
   that applies:
   1. the context holds a floor with a free slot: the record is made in
      its floor;
   2. the context is at `inc.records.per_context_max`: its own least
      recently used **unpinned** record is evicted, and the new record
      takes its slot (in the floor or the shared part, wherever the
      evicted one was); with no unpinned record, branch 5;
   3. the shared part is below `inc.records.total_max`: the record is
      made in the shared part;
   4. the shared part is at `inc.records.total_max`: the first of these
      that exists is evicted, and the new record takes its slot: the
      context's own least recently used unpinned shared-part record; the
      least recently used unpinned shared-part record of the context idle
      longest, provided that context has used none of its records for
      `inc.records.idle_reclaim_after` (a reclaim); the context's own
      least recently used unpinned floor record. With none, branch 5;
   5. the outcome is **unrecorded** (point 5).
4. **What eviction and reclaim do.** Evicting or reclaiming a record
   abandons its `epoch`: no later record of that context holds that value
   (N38, point 8's counter never repeats), so no chunk cached under it is
   reachable again, and that context's next observation of the volume is
   "new". Nothing else needs retiring, because an unpinned record has no
   holder. Neither destroys bytes or journal records. Reclaim from an
   idle context is C27, point 4's budget policy ("idle contexts are
   evicted first"), applied to a context's metadata as the store applies
   it to chunks; it is not evidence about the volume and infers nothing
   from any input, so it is not weak evidence under C24, and it costs
   the idle context what evicting its chunks would: a refetch when it
   comes back. It never reaches a pinned record or a floor record of
   another context. The records of a retired context generation are
   removed when the generation retires, and its floor is returned: no
   later observation can carry that key. Its handles and snapshots end with the generation; its
   journal records are detached from it first (C18, point 9), and from
   then on their recorded incarnation is treated as 5.8, point 3, treats
   a record from an earlier run: unconfirmed, and kept quarantined.
5. **Unrecorded.** A context reaches it only when the shared part is
   full or it is at its own cap, every record it holds is pinned (it
   holds a holder on each volume it has a record for), and no idle
   context's shared-part record can be reclaimed. The
   observation's values are compared with nothing, a fresh `epoch` is
   taken from the counter for that one request and held only by it, and
   nothing the request fetches is cached, as C20, point 1, treats
   unauthenticated data. A request that would make a holder on an
   unrecorded volume (an open, a directory snapshot, a journal record) is
   refused with the retryable busy error, as a pin over budget is (C11,
   point 10), because a holder needs a record to pin. **Bound:** the
   state ends as soon as the context releases one holder, or another
   context's shared-part records go idle for
   `inc.records.idle_reclaim_after`; a context with a floor that holds
   holders on fewer than `inc.records.floor_per_context` volumes never
   reaches it. The cost falls on that context alone. **Residual:** an
   active context is never reclaimed, so other contexts that keep using
   their records can hold the shared part full indefinitely; a context
   is then limited to its floor, and a context without a floor (more
   than `inc.records.floors_max` contexts in use) can be left with no
   record at all, reading uncached and refused busy for holders.
6. **Holders.** `holders` is a `u32`. Adding a holder that would overflow
   it is refused with the retryable busy error, and the count is never
   wrapped (section 6, A2). Each holder's own owner bounds how many
   exist (C27, point 4's caps on pinned bytes, metadata and journal
   bytes), far below 2³².

### 5.3 Versions

Not applicable: nothing here is stored or sent (5.8). The values read are
peer formats IronAFS does not control ([vl-fs-read.md](vl-fs-read.md),
5.3).

### 5.4 Decoding: comparing an observation

This specification decodes no bytes. It compares the values an
observation carries, after vl-fs-read.md, 5.4, has accepted the reply,
in this order `[OWN]`:

1. **Admission.** Only an observation is compared (section 4): a reply on
   the observing context's own connection that passed every check of
   vl-fs-read.md, 5.4. An unauthenticated call is not an observation for
   cached data, because its data is never cached (C20, point 1); its
   values are compared with nothing and kept nowhere.
2. **Lookup.** The record of (context and generation, cell, volume id).
   If none exists, the outcome is **new** if 5.2 lets a record be made,
   and **unrecorded** otherwise (5.2, point 5); points 3 to 6 do not
   apply. "New" is safe whatever the context held before: an earlier
   record's epoch is never reused (5.2, point 4; 5.8), so nothing cached
   under it is reachable, and a record with a holder is never missing
   within a run (5.2, point 1). Journal records from an earlier run are
   5.8, point 3's.
3. **Compared values.** Each value of 5.1.1 marked "compared" that the
   observation carries is compared for equality with the record's. A
   value the observation does not carry (a file-service reply carries no
   VL entry) is not compared and keeps the record's value. Any
   difference is **ambiguous**. The comparison is exact, field by field,
   in each value's decoded form ([vl-fs-read.md](vl-fs-read.md), 5.1.3):
   a server identity is compared as its 16 octets (vl-fs-read.md, 5.1.3,
   point 6), an address as its four octets; a server named by identity
   in one entry and by address in the next is a difference.
4. **Release state.** A `site_release` other than "none" for the serving
   site is **ambiguous**, also when it equals the record's (T3).
5. **Recorded values.** `entry_flags` and `volsync`, when the
   observation carries them, replace the record's copy and its provenance
   entry; a value the observation does not carry keeps the record's copy,
   as in point 3. They are compared with nothing in this version. The capture-backed
   revision may make a word of the volume sync record compared (Q2); it
   then becomes a row of 5.1.1 with its own trigger.
6. **Otherwise** the outcome is **confirmed**.

A reply that vl-fs-read.md could not decode never reaches this
comparison: the whole reply is unused there (its 5.4 and section 7,
point 3), so no observation is ever half compared. Such a reply is not
an observation: it changes no record and serves nothing, and the read
that needed it fails or retries as vl-fs-read.md says. Nothing cached is
served on its account either: what was confirmed before it stays exactly
as confirmed, and no later decision rests on it `[OWN]`. It is not
ambiguity, because a reply that cannot be read says nothing about the
volume, and making it ambiguity would let any corrupted packet on the
path re-epoch the context.

### 5.5 Encoding

Not applicable: nothing is encoded or sent.

### 5.6 States and ordering

1. **Outcomes** `[ADR 0004 C25.2, C25.3: Working]`:
   - **confirmed:** the record's compared values are unchanged, its
     provenance and `last_used` are updated from the observation, and
     chunks under its `epoch` stay reachable in this context. Reachable
     is not servable: a chunk under suspicion is still revalidated as
     points 4 to 6 say, and a confirmed outcome clears no chunk's
     suspicion. Only point 6's refetch, or the cache's status
     revalidation for a narrow suspicion, does;
   - **new:** a record is made from the observation with `epoch` set to
     the next value of N38, point 8's counter;
   - **ambiguous:** an incarnation change in the observing context
     (point 3), then the record's evidence values, their provenance and
     `last_outcome` are replaced from the observation, with the new
     `epoch`, and `last_used` is updated as for any use. Its `holders`
     is kept: the holders are still held, and the record stays pinned
     while they are, so the replacement never makes a held volume
     evictable (5.2, point 1);
   - **unrecorded:** 5.2, point 5.
2. **Triggers of ambiguity,** each the observing context's own
   observation. Each is C25, point 3's ambiguity for that context
   `[ADR 0004 C25.3: Working]` unless it says `[OWN]`:

   | Id | Trigger | C25, point 3's case |
   |---|---|---|
   | T1 | a VL version id differs, including a release clone id that appears, changes or goes (5.4, point 3) | a release (the release clone id); a re-creation that did not choose every id is `[OWN]`, from 5.1.1's reason 2 |
   | T2 | the serving server or partition differs: a move, a restore to a new site, a switch of replica site, including C33, point 7's refresh after "moved" or "offline" | a move; a replica switch is `[OWN]`, from 5.1.1's reason 3 |
   | T3 | the serving site carries a release flag (5.4, point 4) | a release |
   | T4 | the volume's type differs | `[OWN]`: a type change can only be a new volume under the id |
   | T5 | not a trigger: a volume with no record (never seen, evicted, or after a restart) takes the outcome "new" or "unrecorded" (5.4, point 2), and nothing cached under an earlier epoch is reachable (5.2, point 4; 5.8) | a restart of the client, and `[OWN]` for an evicted record |
   | T6 | not used: a restart is the "new" outcome of 5.8, point 2 | — |
   | T7 | verify by refetch returned bytes that differ from the cached chunk under the same file id and data version (point 6) | `[OWN]`: the only observation that shows a repeated data version with other bytes, C25, point 1's assumption made visible |
   | T8 | verify by refetch returned a data version lower than the chunk's (point 6) | `[OWN]`: under a wide suspicion a lower version is read as the volume's, not only the file's (point 6's reason) |

   T5 and T6 keep their numbers so that the review record's references
   stay valid. "Seen offline" and "a server reset", the other cases of
   C25, point 3, have no trigger of their own: point 7 says why for an
   off-line volume seen through aborts, point 12 for a server reset, and
   section 13, Q3, Q4 and Q10, what settles them.
3. **An incarnation change,** in this order `[ADR 0004 C16.1: Firm]`.
   Steps 1 to 3 change state in the core, in one step under the core's
   lock for the volume; only when all three are done does step 4 queue
   anything, so every later operation, in any context, sees all three:
   1. the record's `epoch` takes the counter's next value, so every later
      operation of the context sees it; a chunk key under the old epoch
      is unreachable in the context from that moment (C27, point 1), and
      its bytes are not destroyed here (they are clean data the store may
      evict by its own policy);
   2. the volume's break counter is raised, which is the weak input every
      other context gets (point 4);
   3. every fetch of the context in flight for the volume fails its
      fence: the cache records the local volume epoch in the fetch when
      it is sent and compares it when the reply is installed, as it
      compares the fence tuple (the callback specification, 5.6,
      point 3), and C16, point 2, applies to a reply that fails. The one
      exception is the reply whose own check caused the change (T7 or
      T8, point 6): it is installed under the new epoch as part of this
      step, with the new fence tuple, and is not fetched again;
   4. the incarnation change event names the context, its generation,
      the cell and the volume id, and is queued to the context's
      handles, directory snapshots and Windows identities, and to its
      journal records for the volume, whose owners apply C25, point 5
      `[ADR 0004 C25.5: Working]`. It names nothing in another context.
4. **Another context's ambiguity** is weak input for every other context
   `[ADR 0004 C25.4: Working; C24.2: Firm]`. It reaches them through the
   one counter the cache already compares on every serve: the volume's
   break counter of the fence tuple (the callback specification, 5.6,
   point 2) is raised, in the core, as step 2 of point 3, before any
   event is queued (C16, point 1). So every other context's chunk of the
   volume is under wide suspicion at its next serve, and is not served
   until point 6 revalidates it, whatever that context's own next
   observation shows; its next observation is still compared as 5.4
   says. Its `epoch`, its handles and its journal records are unchanged
   `[ADR 0004 C24.3, C24.6: Firm]`. Raising the counter also fails, by
   C16, point 2, any fetch of the volume in flight in every context:
   each is retried within C16, point 2's bounded budget, and a read
   whose retries are all fenced fails with `Stale`.
5. **What the callback service reports** reaches this specification only
   through the fence tuple the cache compares on every serve (the
   callback specification, 5.6, points 2 to 5). It never triggers
   ambiguity and never changes an `epoch` `[ADR 0004 C24.6: Firm]`. A
   change in a shared value of a chunk's fence tuple is a wide suspicion
   (section 4): point 6 applies. When a serve finds one, the record's
   `suspicion` field records it, with the instant, for display.
6. **Revalidating a chunk under wide suspicion** `[OWN]`. A suspect entry
   is revalidated by its reader under its own credentials (C24, point 2);
   for a clean data or directory chunk under wide suspicion, whatever
   raised it (the callback service, a pause or suspend, or another
   context's ambiguity, point 4), this specification sets what that
   revalidation is:
   - the reader fetches the chunk's byte range again, in its own context,
     with a status in the same reply (vl-fs-read.md, 5.1.4, point 6), and
     the reply passes vl-fs-read.md, 5.4, point 9;
   - if the reply's data version is higher than the chunk's, the chunk is
     stale: the new bytes are installed under the new version, as for any
     changed file;
   - if the reply's data version is lower than the chunk's, that is T8:
     point 3 applies in the reader's context, and the new bytes are
     installed under the new epoch;
   - if the data version is equal and the bytes are equal, the chunk is
     kept, and records the fence tuple of the fetch;
   - if the data version is equal and the bytes differ, that is T7: the
     reader's context has observed a different incarnation, and point 3
     applies in that context; the new bytes are installed under the new
     epoch;
   - a chunk under a narrow suspicion only (its object's own break
     counter) is revalidated as the cache specifies (a status call in the
     reader's context, with C25, point 4, for a lower data version); this
     point does not apply to it.

   Why: a server reset, a whole-volume break, an Rx pause and a suspend
   of the client each open a window in which the client cannot know what
   happened at the server; a restore in that window keeps the data
   version (5.7), so a status call cannot revalidate the bytes. Fetching
   them again is the only revalidation that cannot serve old bytes, and
   it keeps everything C24, point 6, requires kept: bytes, epochs,
   handles and journal records change only on the reader's own
   observation (T7, T8). A lower data version is read as the volume's
   here, not only the file's as point 8 does, because the window is the
   volume's or the server's: an older dump restored in it lowers or
   repeats versions across the volume. The comparison costs reading the
   cached chunk once.
   The cost of the rule is that, after each wide suspicion, every cached
   chunk of the affected server or volume is fetched once more, in each
   context, on its next use: the cache saves latency but no bandwidth
   across such an event.
7. **Rx aborts change nothing here** `[ADR 0006 D1.5: Firm]`. An abort
   that says a volume is off line, moved, busy, being salvaged, or not on
   the server, or that the server is restarting
   ([vl-fs-read.md](vl-fs-read.md), 5.9, rows R4, R7 to R16), ends its
   call and does what vl-fs-read.md, 5.9, says (try another site, ask the
   VL service again); it never triggers ambiguity, marks nothing suspect
   here, and changes no `epoch`, because ADR 0006, decision 1, point 5,
   bars an abort from evicting or retiring anything, and an incarnation
   change retires handles and quarantines journal records (C25, point 5).
   What an abort may lower is ADR 0006, decision 1, point 6's, which the
   callback specification carries out (its 5.6, point 11). The VL entry
   the client then fetches is an observation, and may trigger T2. So a
   volume "seen offline" only through aborts, and restored in place while
   it was off line, is not detected by this version unless a wide
   suspicion covers the same window: section 13, Q3.
8. **A lower data version** for one file, outside a wide suspicion, is
   C25, point 4's file-level ambiguity, applied by the cache in the
   observing context, with weak input to other contexts `[ADR 0004
   C25.4: Working]`. It is not volume ambiguity here: one file's version,
   seen outside any window that covers the volume, says nothing about the
   others. Under a wide suspicion it is T8 (point 6).
9. **Shared incarnation state.** None exists in this version. Before
   rxgk the pool is empty (C21), no record is shared between contexts,
   and no observation establishes that two contexts see the same
   incarnation `[ADR 0004 C24.3: Firm]`. The pool's record, changed only
   on authoritative evidence (C24, point 4; N5, point 6), is a revision
   of this specification for M6.
10. **Replica ranking** excludes a read-only site marked old release or
    not released before ranking `[ADR 0004 C25.6: Working]`; this is
    vl-fs-read.md, 5.4, point 11, and is cited here, not restated.
11. **Showing the evidence** `[ADR 0004 O13.1, O13.2: Working]`. `afs
    cache show` gives, for the caller's own context only, the record's
    compared and recorded values, their provenance (5.1.2), the record's
    `epoch`, its `last_outcome`, and its `suspicion` field as "the last
    suspicion a serve found, and when" (point 5). Whether each of the
    path's chunks is suspect now is computed when the command runs, by
    comparing the chunk's recorded fence tuple with the current one, as
    a serve would, without serving it. Server text never reaches it: no
    value of the record is a string.
12. **A server reset** `[OWN]`, within C24 (Firm). C25, point 3, names a
    server reset among the cases that need a fresh local epoch in "the
    observing context". The client learns of a reset only from the
    callback service (the callback specification, 5.6, point 9, rows R1
    and B0) or from an abort (point 7); neither arrives on a context's
    own call, so no context observes it. For the other contexts and the
    shared state, C24 bars an epoch change on such input: points 3 and 4
    reserve changes to another context's or shared state for
    authoritative evidence, and point 6's closed list keeps epochs,
    handles and journal records. For a context's own state, C24, point
    2, lists what weak input may do (suspend promises, revoke grants,
    mark entries suspect) and an epoch change is not on it; the list is
    not called closed, so whether a context may re-epoch itself on a
    reset is not barred in so many words. This version reads it
    conservatively, as not authorised, and leaves the choice to the
    owner (section 13, Q10). So a reset moves
    the server's suspect epoch, every chunk of that server is under wide
    suspicion, and point 6 revalidates each by refetch: an epoch changes
    only when a reader's own refetch shows a lower version or other
    bytes (T7, T8). This keeps C24 and serves no old bytes; what it does
    not do is invalidate a context's handles and journal records on the
    reset alone. A restore hidden by a reset that raised every data
    version it touched is read as ordinary changes. Section 13, Q10,
    records the departure from C25, point 3's wording for the owner.

### 5.7 The restore discriminator: the answer

**Answer** (INSPECTED, from I1 to I4; the restore experiment against the
test cell is to record it as EXECUTED, section 13, Q9): **no value in
the replies M2a decodes is known to tell a restore made while the client
was down.** The reasons, each a lead from documentation:

1. A restore can overwrite a volume at its own site and keep its volume
   id `[I2 vos_restore(1): UNVERIFIED]` `[I3 "About Restoring Volumes":
   UNVERIFIED]`. So the volume id, the VL version ids and the serving
   site can all be equal before and after.
2. A full dump carries each file's vnode, uniquifier and data version,
   and the volume's ids and next uniquifier
   `[I4 "Volume IDs", "Full vs Incremental Volume dumps", "Vnode Data Version", "Volume Next Uniquifier": UNVERIFIED]`.
   So a file's id and data version, the values a chunk is keyed by, can
   be equal, with older bytes: the case C25, point 1, assumes.
3. The volume's creation date is the one candidate documentation names
   as changed by a restore `[I3: UNVERIFIED]`. But an administrator can
   restore with the dump's date or the old volume's
   `[I2 vos_restore(1), "-creation": UNVERIFIED]` (the same page's
   description says only that a restore sets the restore time), and
   whether any reply M2a decodes carries the date at all is MISSING (the
   volume sync record's layout is MISSING in
   [vl-fs-read.md](vl-fs-read.md), 5.1.3, point 4; section 13, Q2).
4. A dump can be made by tools other than the volume server
   `[I4 "Summary": UNVERIFIED]`, so a restored volume's values are
   whatever its dump says.

Therefore, for C26, point 4: no restore discriminator exists in this
version, and clean chunks are not reused across a restart (5.8). If the
capture-backed revision finds a value a restore changes, it still cannot
serve as the discriminator unless no administrator's option and no dump
can preserve it (point 3); a value that only usually changes is a further
trigger within a run (5.6, point 2), never a reason to reuse a chunk
after a restart.

**Failure cases this answer avoids** (#78): it declares no discriminator
that a restore with equal data versions defeats (points 1 and 2), and it
never treats "cannot tell" as "unchanged": a changed value and a release
in progress are ambiguous, a volume with no record gets a new epoch, and
a reply that cannot be decoded serves nothing (5.4; 5.6, point 2). Where
the evidence cannot speak at all (a reset, a pause, a suspend), the bytes
are fetched again before they are served (5.6, points 6 and 12).

### 5.8 Restart

1. No evidence record survives a restart of the daemon: the table is in
   memory only `[ADR 0004 C26.4: Working]`. (A journal record keeps the
   volume's incarnation as it was recorded, C29, point 1; point 3 says
   what that value is worth after a restart.)
2. After a restart, every volume's first observation in each context
   finds no record. The context holds no chunk, handle or directory
   snapshot from the earlier run (C11, point 2, ends every binding at a
   restart; C26, point 4, keeps no clean chunk for reuse), so the outcome
   is **new**, with a fresh epoch from N38, point 8's counter, which
   never repeats within one state directory.
3. **Journal records from an earlier run.** No evidence survives to show
   that the volume a journal record names is the incarnation it was
   written over (5.7). So the journal treats the incarnation in every
   such record as unconfirmed, whatever any later observation shows:
   the record is quarantined for explicit recovery, as C25, point 5,
   requires of an incarnation change. This is already C28, point 3's
   rule for a record left by a restart (never stored automatically); it
   is stated here so that no later "confirmed" or "new" outcome is read
   as confirming it `[ADR 0004 C28.3: Firm]`. The journal specification
   carries it out.
4. The cache does not look up a clean chunk written by an earlier run,
   whatever its key (C26, point 4). Whether it removes them at start or
   keys them so that no lookup can reach them is the chunk store's
   specification; either way no such chunk is read.
5. **Cost.** Every daemon restart starts with a cold cache: each file is
   fetched again on its first read in each context, and the first reads
   after a restart cost a round trip and the file's bytes each.
6. The host UUID persists (C26, point 5), and callbacks do not (C26,
   point 3; the callback specification, 5.6, point 1). Neither is
   incarnation evidence.

## 6. Arithmetic

| Id | Operation | Operands' range | Bound and result at overflow |
|---|---|---|---|
| A1 | the next local volume epoch (5.6, points 1 and 3) | N38, point 8's `u64` counter | `checked_add`; at overflow the counter is exhausted and the observation fails with the generic failure ([vl-fs-read.md](vl-fs-read.md), section 13, Q11), the volume is unusable in that context, and an event is emitted; the counter never wraps, so no epoch repeats. At one new epoch per microsecond it lasts over 500 000 years |
| A2 | record counts against the caps, and the `holders` count (5.2) | `u32` counters | `checked_add` and `checked_sub`. A record count cannot exceed its cap, because a record is made only after the cap check. A `holders` add that would overflow refuses the new holder with the retryable busy error (5.2, point 6); a subtract below zero is a fault in IronAFS, counted and reported, and leaves the count at zero, never wrapped |
| A3 | the table's bytes, `inc.record_max × (inc.records.total_max + inc.records.floors_max × inc.records.floor_per_context)` (5.2) | constants | a compile-time assertion that the product fits in `usize` and equals 24 MiB (16 MiB shared, 8 MiB floors); a floor is granted only while fewer than `inc.records.floors_max` are held, a count checked with `checked_add` |
| A4 | comparing bytes in verify by refetch (5.6, point 6) | two chunks, each at most `fs.fetch.chunk_max` | the data versions are compared first (a lower one is T8); under an equal data version, equal lengths are checked first: a different length is T7; then a constant-memory comparison with no arithmetic on untrusted values |

`[OWN]` for every row.

## 7. Errors and failure

1. **No outcome is an error to the caller** `[OWN]`. Ambiguity itself
   fails no read: after an incarnation change the read goes on under the
   new epoch. The fetches it causes can fail, as any fetch can (point 3).
   The one refusal is 5.2, points 5 and 6's busy error for a holder on an
   unrecorded volume.
2. **Exhausted epoch counter** (A1): the generic failure, counted and
   reported; nothing is served under an old epoch.
3. **A failed verify by refetch** (a timeout, an abort, a reply that
   fails vl-fs-read.md, 5.4) leaves the chunk suspect and unserved; the
   read fails with the fetch's own error (vl-fs-read.md, section 7). The
   old bytes are not served instead, and are not destroyed.
4. **Fail safe.** Every state here that cannot show the volume is the
   same (no record, an evicted record, a table with no room, a release
   in progress, a changed value, a restart, a wide suspicion) leads to a
   new epoch, to an uncached read or to a refetch, never to reuse of
   unverified bytes. A reply that cannot be decoded changes nothing and
   serves nothing (5.4).
5. **No panic** is reachable from any compared value (coding standards
   rule 1): every comparison is on decoded, typed values, and the table
   allocates nothing after its cap checks.

## 8. Security

Who can change what this specification reads: the cell's VL servers and
fileservers; under the null class, anyone on the path (vl-fs-read.md,
section 8); anyone who can send to the callback service (the callback
specification, section 8); and local users through their own contexts.
In M2a no unauthenticated data is cached (C20), so this logic protects
authenticated contexts first (M2b), whose replies are hints protected by
the user's own key (N5, point 2).

| Threat | What stops it, or what it costs |
|---|---|
| A forged VL entry or volume sync record that makes a new incarnation look like the old one | under the null class, not stopped: such a forger can forge the data too (vl-fs-read.md, section 8), and nothing it sends is cached (C20). Under rxkad the forger needs the user's session key. No outcome of 5.4 is better for the forger than serving its own bytes |
| A forged or changed value that makes the same incarnation look new | costs the observing context a refetch of the volume; it reaches no other context except as a suspicion (5.6, point 4) and never another context's epoch, handles or journal records (C24, points 3 and 6) |
| Using one context's observation to move another's state | 5.6, points 4 and 5: weak input only marks suspect; every epoch change is the context's own observation |
| Forged callback resets, breaks naming a volume, or overloads, to force refetches | each wide suspicion costs at most one refetch per cached chunk, on its next use. How often suspicion can be renewed is the callback specification's: a server's suspect epoch moves at most once per `cb.advance_interval` (its 5.6, point 7), but a break naming a volume from a source that is a known server raises that volume's break counter at once, within that server's rate bucket (its 5.6, points 8 and 9, row B1). So a forger who keeps renewing it can make every read of that volume a refetch: the cache gives no bandwidth saving for that data while it lasts, and at most doubles the bytes read (a refetch per use instead of a fetch per use); integrity is unaffected |
| A forged abort to retire a volume's handles or journal records | 5.6, point 7; ADR 0006, decision 1, point 5 |
| Filling the record table | each context's floor is outside the shared cap and evicted only by its own activity; a context at the shared cap evicts its own unpinned records first, and reclaims only unpinned shared-part records of a context idle for `inc.records.idle_reclaim_after`, never a pinned or floor record (5.2, point 3). Another context needs only to keep its own records active (used within `inc.records.idle_reclaim_after`), not pinned, to hold the shared part full; the fairness bound is the floor: a victim with a floor keeps 32 records, and reaches "unrecorded" only when all its records are pinned. A context without a floor, past `inc.records.floors_max` contexts, can be starved entirely: the residual of 5.2, point 5 |
| A restore made while the client was down, with equal data versions | no clean chunk survives a restart (5.8) |
| A restore in place during a run, inside a window that only aborts show | not detected: section 13, Q3 records it and its remedy |
| A restore hidden by a server reset that raised every data version it touched | read as ordinary changes: new bytes are served, but handles and journal records are not invalidated on the reset alone (5.6, point 12); section 13, Q10 |
| A restore in place during a run, with no event the client sees at all (no reset, no break, no move) | not detected; it is the residual C25 accepts by naming its cases. Section 13, Q4 asks whether servers break callbacks on a restore, and Q5 records the decision this needs |

## 9. How the invariants are kept

| Invariant (section 1) | Kept by | Test that fails if it is broken |
|---|---|---|
| C6, point 3 (the gate) | this specification; 5.7; no cache code merges before its review (README, "Status") | review of this specification (section 14) |
| C25, point 1 | 5.1.2 (per context, cell and volume, with the server as a compared value, and provenance) | AC1, AC12 (10.5) |
| C25, point 2 | 5.4; 5.6, point 1 | AC2, AC3 |
| C25, point 3 | 5.6, points 2, 3 and 12 (the server reset, with its departure in Q10) | AC3 to AC6, AC8, AC9, AC16 |
| C25, point 4 | 5.6, point 8 (the cache applies it) | AC11 |
| C25, point 5 | 5.6, point 3, step 4 (the event and its scope) | AC4 (the event names only the observing context) |
| C25, point 6 | 5.6, point 10 (vl-fs-read.md, 5.4, point 11) | vl-fs-read.md's acceptance cases |
| C26, point 3 | 5.8, point 6 (the callback specification) | the callback specification's restart case |
| C26, point 4 | 5.7; 5.8 | AC1 |
| C28, point 3 | 5.8, point 3 | AC17 |
| C24, points 2 to 6 | 5.6, points 4 to 6 and 12 | AC7, AC8, AC10 |
| C16, point 1 | 5.6, point 3, steps 1 to 3, before step 4 | AC9 |
| C20, point 1 | 5.4, point 1 | AC13 |
| N38, point 8 | 5.6, points 1 and 3; 5.8, point 2; A1 | AC1, AC3; the counter's own tests (S5) |
| O13, point 1 | 5.6, point 11 | AC14 |
| ADR 0006, decision 1, point 5 | 5.6, point 7 | AC10 |
| Constraint 4 | 5.2; section 6; section 7, point 5 | AC6, AC15; the table's model test (10.2) |

## 10. Tests

Paths: `crates/ironafs-cache/tests/it/incarnation.rs` for the record, the
comparison and the table (coding standards rule 15; the cache crate's
test layout, #91); the simulator's scenarios for the acceptance cases
that need a server (O45; the simulator's fileserver model, #96).

### 10.1 Golden vectors

None: nothing is encoded or decoded here.

### 10.2 Properties

- **Comparison:** for random pairs of observations, the outcome is
  confirmed if and only if every compared value is equal and no release
  flag is set (5.4); a change in any one compared value alone is
  ambiguous.
- **Epochs never repeat:** over random sequences of observations, outcomes,
  evictions and restarts, no two records ever hold the same `epoch`, and
  no `epoch` is reused after its record is replaced or evicted (N38,
  point 8).
- **Table bounds (model test):** against a reference model, the table
  never holds more than its caps; an eviction at either cap removes only
  an unpinned record of the context making the observation, or an
  unpinned shared-part record of a context idle for
  `inc.records.idle_reclaim_after`; a pinned record and another
  context's floor record are never evicted; a context with no record
  that 5.2, point 3, lets it evict gets the outcome "unrecorded" and no
  record, and its request to make a holder is refused as busy; an
  ambiguous outcome keeps `holders`.

### 10.3 Reject corpus

Not applicable: no bytes are decoded here. vl-fs-read.md's reject corpus
covers the replies whose values reach 5.4.

### 10.4 Fuzz target

Not applicable for the same reason. The property tests of 10.2 run the
comparison over arbitrary values.

### 10.5 Acceptance cases

In the simulator, under an authenticated test profile with several
credential contexts (C20; O50, point 1), with the simulated fileserver
able to restore a volume from a dump it holds:

- **AC1. Restore while the daemon is down.** Context A reads file F
  (vnode, uniquifier and data version v); the daemon stops; the server
  restores an older dump in place, so F has data version v with other
  bytes; the daemon starts; A reads F. Result: the new bytes are served,
  fetched from the server (the fetch count shows it), and no chunk from
  the earlier run is read.
- **AC2. Same incarnation, same run.** A reads F twice with no event
  between. Result: the second read is served from cache; outcome
  confirmed.
- **AC3. Move.** A reads F; the volume moves to another server; A's VL
  entry is refreshed and shows the new site. Result: T2; A's `epoch`
  changes; F is fetched again; the old chunk is not reachable in A.
- **AC4. Release.** A reads a file of a read-only volume; a release
  starts, and A's VL entry shows the serving site flagged. Result: T3;
  A's epoch changes; the incarnation change event names A only.
- **AC5. Re-creation under the same id.** The read-write volume is
  removed and created again with the same read/write id and new
  read-only and backup ids, and F's vnode, uniquifier and data version
  recur with other bytes. Result: T1 on A's next VL observation; new
  bytes served.
- **AC6. Evicted record.** A reads more volumes than
  `inc.records.per_context_max`, holding no handle on the least recently
  used one, V, whose record is evicted while A still has chunks of V
  cached; A reads V again. Result: the outcome is "new", with an epoch
  that no earlier record held; V's old chunks are not reachable; the
  data is fetched again. Variant: A is at
  `inc.records.per_context_max` with a handle on every volume it has a
  record for, and opens a file on one more. Result: no record is
  evicted; the open is refused as busy.
- **AC7. Another context's ambiguity.** A and B both cache F; A observes
  T2; B reads F. Result: the volume's break counter was raised before
  A's event was queued; B's chunk is under wide suspicion and is not
  served until B's own refetch revalidates it, even when B's own next VL
  entry confirms its record; B's epoch, handles and journal records are
  unchanged unless B's own observation shows a difference (T2 in B if
  B's refreshed entry shows the move; T7 if B's refetch shows other
  bytes under the same version). Variant: A's observation was forged and
  B's shows no change: B's refetch returns equal bytes and B serves them
  with its epoch unchanged.
- **AC8. Server reset hides a restore.** A caches F; the server restores
  an older dump in place and sends a callback reset; A reads F. Result:
  the chunk is under wide suspicion; verify by refetch returns equal data
  version and other bytes; T7; A's epoch changes; the new bytes are
  served. Variant with equal bytes: the chunk is kept, A's epoch is
  unchanged, and no handle or journal record of A is touched.
- **AC9. Fetch in flight.** A starts a fetch of F; T2 is observed in A
  before the reply is installed. Result: the reply fails its fence (C16,
  point 2) and is never served.
- **AC10. Aborts.** A's fetch of F is aborted with the "off line" and
  then the "moved" codes, from a matching source on an eligible
  connection. Result: no epoch changes and nothing is marked suspect
  here; A asks the VL service again; T2 follows only if the new entry
  shows another site.
- **AC11. Lower data version.** A caches F at data version v; a status
  in A shows v − 1. Result: the cache's file-level rule (C25, point 4);
  the volume's record is confirmed if its values match.
- **AC12. Provenance.** After each of AC2 to AC5, A's record shows for
  each value the procedure, the security profile, the instant and the
  suspect epoch of the reply that set it.
- **AC13. Unauthenticated reads.** An anonymous context reads F twice.
  Result: no record is made; both reads fetch.
- **AC14. Showing the evidence.** `afs cache show` for F as A shows A's
  record (5.6, point 11); as user B, it shows only B's.
- **AC15. Table full of other contexts.** Floors are free. The shared part is at
  `inc.records.total_max`, all of it other contexts' records in active
  use; a new context C observes volumes. Result: C's first
  `inc.records.floor_per_context` volumes get "new" records in its
  floor, and no other context's record is evicted; C's next volume
  evicts C's own least recently used unpinned floor record. Variant 1:
  C holds a handle on each of its floor volumes and observes one more.
  Result: "unrecorded": a read of a file there fetches and caches
  nothing; an open of a file, and a directory listing that would make a
  directory snapshot, are each refused as busy. When C closes one
  handle, its next observation gets a record. Variant 2: one other
  context D has used none of its records for
  `inc.records.idle_reclaim_after`. Result: C's next record reclaims
  D's least recently used unpinned shared-part record; D's floor and
  pinned records are untouched; D's next observation of that volume is
  "new". Variant 3: the shared part has room and C holds a full floor
  below `inc.records.per_context_max`. Result: C's next record is made
  in the shared part, and nothing is evicted. Variant 4: all
  `inc.records.floors_max` floors are held; a further context E observes
  a volume. Result: E's record is made in the shared part if it has
  room; otherwise E reclaims only as 5.2, point 3, branch 4, allows, and
  is "unrecorded" if nothing qualifies.
- **AC16. Reset with a lower version.** A caches F at data version v;
  the server restores an older dump in which F has version v − 1, and
  sends a callback reset; A reads F. Result: wide suspicion; the refetch
  shows v − 1; T8; A's epoch changes and the incarnation change event
  names A. Variant with v + 1: the new bytes are served as an ordinary
  change, with A's epoch unchanged (5.6, point 12's residual).
- **AC17. Journal record across a restart.** A has a journal record for
  a file of V; the daemon restarts; A's first observation of V is
  "new". Result: the record stays quarantined (5.8, point 3); nothing
  stores it automatically.

### 10.6 Gaps

- AC8's "server sends a reset on a restore" is the simulator's
  assumption; whether real servers do is section 13, Q4.
- The case of section 8 "restore in place with no event" has no test
  that passes: it is a residual, recorded in Q5. AC16's variant asserts
  the residual of 5.6, point 12 (Q10) as it stands, so that a change to
  it is a visible test change.

## 11. Revisit if

From the working decisions this rests on:

- **C25:** the specification gate (C6) shows no reply field tells a
  restored volume from the original, or shows one that makes point 3
  unnecessarily broad. **This version shows the first:** no field is
  known to tell (5.7). The trigger is raised as section 13, Q5, for the
  owner's design review of C25.
- **C26:** the specification gate of C25 shows a restore discriminator.
  Then clean chunks persist under point 4. Not met (5.7).
- **C27:** M2a measurements show the filesystem-as-index layout or the
  eviction policy misses its targets on a supported filesystem.
- **C33:** the oldest supported Linux release ships systemd 256 or later,
  the M2 tests show an out-of-scope query on any supported configuration,
  or the status of RHEL's local resolver changes (this specification
  uses only point 7's refresh after "moved" or "offline").
- **C20:** measurements of anonymous reads of public cells show the
  refetch cost matters to users, and a design is shown that keeps C19.
- **O13:** stale-data reports are not settled by what the command shows.
- **O42** (cited by N38, point 8): a platform offers reliable detection
  of a restored or cloned machine.

This specification's own choices are reopened if:

- the captures show a volume sync word that a restore always changes and
  no option preserves (Q2): it becomes a compared value;
- the captures show servers break callbacks for the whole volume, or
  reset, on a restore (Q4): AC8 becomes the case that covers it, and
  Q5's residual narrows;
- replica failover refetches measurably slow reads (5.1.1, reason 3;
  Q6);
- verify by refetch measurably costs too much after server restarts
  (5.6, point 6);
- a context needs more than `inc.records.per_context_max` volumes at
  once, or contexts are measured reaching "unrecorded" in ordinary use
  (`inc.records.floor_per_context`, `inc.records.idle_reclaim_after`).

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| I1 | DOCUMENTATION | OpenAFS reference pages `vos_create(1)` (the `-id` option and the ids it allocates) and `vos_examine(1)` ("OUTPUT": the version ids, the site flags, the creation, copy and last update dates), register of editions row E8 ([README](README.md)). A lead only (rules 11 and 14) | 2, 5.1.1, 5.7 |
| I2 | DOCUMENTATION | OpenAFS reference pages `vos_restore(1)`, `vos_release(1)`, `vos_move(1)` and `vos_backup(1)`, register of editions row E14: "DESCRIPTION" and "OPTIONS" of each. A lead only | 5.1.1, 5.7 |
| I3 | DOCUMENTATION | OpenAFS Administration Guide, "Dumping and Restoring Volumes" (HDRWQ240), section "About Restoring Volumes", register of editions row E15. A lead only | 5.1.1, 5.7 |
| I4 | DOCUMENTATION | J. Altman, "AFS Dump Stream Format", last updated 4 August 2024, AFS Assigned Numbers Registry, register of editions row E16: "Summary", "Volume ID", "Volume Name", "Volume IDs", "Volume Timestamps", "Volume Next Uniquifier", "Full vs Incremental Volume dumps", "Vnode Data Version". A lead only | 5.1.1, 5.7 |
| C | DECISION | [Claim record](../reviews/2026-11-20-adr-0004-claims.md) rows `[ADR-008]` and `[DES-057]`, re-read on 2026-11-21 (section 12.2) | 12.2 |
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; C6, C11, C16, C18, C20, C21, C24, C25, C26, C27, C28, C29, C33, C37, C40 ([part 1](../adr/0004-architecture-1-core.md)); O13, O42, O45, O50 ([part 3](../adr/0004-architecture-3-operations.md)); N5, N27, N38, N39 ([part 4](../adr/0004-architecture-4-negotiation.md)) | 1, 2, 5, 8, 9, 11 |
| D2 | DECISION | [ADR 0006](../adr/0006-rx-unprotected-packets.md), decision 1, points 5 and 6 | 1, 5.6, 8, 9 |
| D3 | DECISION | [vl-fs-read.md](vl-fs-read.md): 5.1.3, 5.1.4, 5.2, 5.4, 5.9 and section 7; the callback specification (#66): 5.2 and 5.6, points 1 to 7 and 11 | 2, 4, 5, 8 |
| D4 | DECISION | [ADR 0001](../adr/0001-licence-and-dont-copy.md), "Don't copy"; [coding standards](../coding-standards.md), rules 1, 2 and 15 | 6, 7, 10, 12 |
| O | OWN | every point tagged `[OWN]`, with its reason in place | 5, 6, 7 |

**What was not used.** No source code, test data, interface definition
or generated definition of any AFS or Rx implementation was read for this
specification, and none is cited. No copy of the 1991 AFS-3 programmer's
references was used (README, "Sources and provenance", rule 10). The
community Rx draft was not read for this specification. The documentation
pages and the dump format document are cited for facts only, in
IronAFS's own words; none is quoted, and no option text, tag value,
constant name, figure or example from any of them is reproduced beyond
the names of the commands, options and sections that locate a fact. The
dump format document's statements about one vendor's extensions (wider
ids, finer timestamps) are not used. No fact here is a wire fact: the
layouts stay MISSING in vl-fs-read.md.

### 12.1 Observed behaviour

None yet. The capture-backed revision records the observation lab's
stage B captures here (#51): for a move, a release, a restore in place, a
restore to a new site and a re-creation under the same id, every value of
5.1.1 before and after, and every callback the server sends; a test
venue with synthetic cell names, users, keys and data; raw bytes first;
each observation with its venue, server release, date, scenario and the
evidence label EXECUTED. The restore experiment against the test cell
(planned; section 13, Q9) records the answer of 5.7.

### 12.2 Re-check of the cited claims (2026-11-21)

The claim record has no row on volume incarnation or restore behaviour;
the two rows this specification leans on were re-read at their sources.
`[ADR-008]`: the Administration Guide's "About Volumes" page (served with
the hash register row E9 records) still says each version of a volume has
its own id. `[DES-057]`: as [vl-fs-read.md](vl-fs-read.md), 12.2 records
on the same date; it bears on the width of the data version, not on
incarnation. No contradiction with ADR 0004 C25 or C26 was found. One
tension between ADR 0004 C25, point 3, and ADR 0006, decision 1, point 5,
was found and is raised as section 13, Q3.

## 13. Open questions

- **Q1. The capture-backed revision.** Stage B of the observation lab
  (#51) records, per server release in the matrix, the values of 5.1.1
  before and after: a move; a release (and a release interrupted, leaving
  flags); a restore in place with each creation-date option; a restore to
  a new site; a removal and re-creation with the same id; a backup
  volume replaced by a new clone. Also the callbacks each sends (Q4) and
  what a client reading during each sees. Blocks: the closing of #78
  (its "Order" paragraph); not the cache's code, which follows this
  version's conservative rules. Recommendation: one campaign with
  vl-fs-read.md's Q1, since it needs the same layouts.
- **Q2. The volume sync record.** What its words are; whether one of them
  is the volume's creation date, or another value a restore changes; and
  whether any option or dump can preserve it (5.7, point 3).
  Recommendation: if a word always changes on a restore and on a
  release, compare it as a further trigger within a run (5.6, point 2);
  do not treat it as a restore discriminator for C26, point 4, unless
  the restore experiment (Q9) shows that no creation-date option and no
  hand-made dump can preserve it.
- **Q3. C25, point 3's "seen offline" against ADR 0006, decision 1,
  point 5.** In M2a's procedures the client learns that a volume is off
  line mostly from abort codes, and ADR 0006 (Firm, later) bars an abort
  from evicting or retiring anything, while an incarnation change
  retires handles and quarantines journal records. This version keeps
  ADR 0006 (5.6, point 7), so an off-line period seen only through aborts
  is not ambiguity. Blocks: nothing in M2a (no real-cell cache, C20);
  the authenticated cache of M2b. Recommendation, a design issue for the
  owner: amend C25, point 3, to say that "seen offline" means a decoded
  reply that shows it (the VL entry, or volume status once vl-fs-read.md's
  Q9 is settled), and record the residual of section 8; or amend ADR 0006,
  decision 1, point 6, to let a matching abort on an eligible connection
  mark that volume's chunks for verify by refetch in the owning context
  only (5.6, point 6), which retires nothing and keeps C24. The second
  closes the window at the cost of a refetch after each off-line abort,
  and is the recommendation.
- **Q4. What servers send on a restore, move, release and removal:**
  breaks naming files, a whole-volume break, a reset, or nothing, to
  clients holding promises on the volume. MISSING; settled by Q1.
  Blocks: Q5's choice.
- **Q5. The in-run residual (C25's revisit trigger).** A restore in place
  during a run, between two reads, with no move, release, reset,
  whole-volume break, pause or suspend seen by the client, keeps every
  value compared here; a status revalidation then sees an equal data
  version and the cache serves the old bytes. C25 accepts this by naming
  its cases; #78's failure case ("ambiguous treated as unchanged") is
  met only by the cases named. Recommendation, a design issue for the
  owner under C25's "Revisit if": once Q4 is known, extend 5.6, point 6's
  verify by refetch to a chunk whose object had a break and whose data
  version did not change (a status or ACL change, or a restore), at the
  cost of a refetch after each status-only change; if servers send
  nothing on a restore, document the residual in the administrator's
  restore procedure (flush clients after a restore in place), as O42,
  point 6, does for client images.
- **Q6. Replica failover.** Whether a value shows that two read-only
  sites hold one release (the release clone id at release time, a
  volume sync word), so that a switch between them need not be T2.
  Recommendation: keep T2 until the captures show such a value and the
  restore experiment shows a release cannot be mimicked.
- **Q7. The counter.** N38, point 8's counter and its persistence are
  S5's (the stable object key) and the state directory's. This
  specification needs only that it never repeats (A1). Blocks: nothing
  here.
- **Q8. The volume server's dates.** The copy date is said not to be
  stored in dumps `[I1: UNVERIFIED]`, so it may change on every restore.
  Reading it needs the volume server's procedures, a service M2a does
  not call, and its trust under the null class would be a hint like any
  other. Recommendation: measure it in the restore experiment (Q9); using
  it would add a service and is an ADR decision, not this
  specification's.
- **Q9. The restore experiment.** Run against the test cell: restore an
  older dump over a volume in place, with each creation-date option,
  while a client is down and while one runs; record which values of
  5.1.1 changed, and whether the client served old bytes. Records 5.7's
  answer as EXECUTED. Until it runs, C26, point 4's rule holds.
- **Q10. C25, point 3's "server reset" against C24 (Firm).** C25, point 3,
  asks the observing context to start a fresh local epoch after a
  server reset, and C25, point 5, makes that an incarnation change. No
  context observes a reset on its own call (5.6, point 12). C24, points
  3, 4 and 6, bar weak input from changing another context's or shared
  state, its epoch, handles or journal records; for a context's own
  state, C24, point 2's list does not include an epoch change, but is
  not called closed, so the own-context case is an interpretation, not
  a Firm bar. This version reads it as not authorised and keeps C24:
  a reset makes every chunk of the server wide-suspect, the bytes are
  fetched again before they are served, and an epoch changes only on a
  reader's own T7 or T8. The residual: a restore hidden by a reset that
  raised every version it touched is read as ordinary changes, so open
  handles and pending journal records of the volume are not invalidated
  on the reset alone. Blocks: nothing in M2a (C20); the authenticated
  cache of M2b. Recommendation, a design issue for the owner: amend C25,
  point 3, to say that a server reset is handled by revalidating bytes
  (5.6, points 6 and 12) rather than by an epoch change, since the reset
  is weak input; or, if the owner wants handles invalidated on a reset,
  decide whether C24, point 2, lets a context change its own epoch on
  weak input. Saying so, for the own context only, would be an
  interpretation of point 2's open list, recorded by amending C25
  (Working); treating the reset as authoritative for other contexts
  would change C24, points 3, 4 or 6, and so need a superseding ADR.
  **Decided** (controller, 2026-11-21): C24 is kept as this version
  reads it, and the amendment of C25, point 3's wording to match is
  taken up in #53. The owner may override.

## 14. Review

The review record is
[2026-11-21-m2a-78-incarnation](../reviews/2026-11-21-m2a-78-incarnation.md).
