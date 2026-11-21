<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# AFS directory format

Status: draft (2026-11-21), the version from documents only. Raised in
issue #56.

Kind: protocol. Class: S. A fileserver, or anyone on the path while the
null security class is in use, controls every byte of a directory object,
and the client turns those bytes into names and file identifiers that the
whole namespace rests on ([README](README.md), "Review class per kind").

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (I1, F1, D1 and
so on) are in section 12.

**This version is written from documents only.** No published source
found so far may fix the layout on its own (section 12), so every layout
fact in 5.1 is labelled UNVERIFIED, and nothing is coded from it (ADR
0004, C6, point 4; N31, point 3). The reader's checks in 5.4 are IronAFS's
own and hold whatever values the layout is confirmed to have. A revision
backed by black-box captures from the observation lab (#51) confirms or
corrects each layout fact; code written from this specification merges
only after that revision is reviewed (section 13, Q1).

## 1. What this implements

ADR 0004, overview, constraint 4 (a constraint of the whole ADR; it
changes only with the ADR):

> **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
> local IPC, configuration, requests from the operating system, and SMB
> messages before authentication. No panics, checked arithmetic and byte
> limits apply to all of it (coding standards rules 1 to 3).

ADR 0004, C4, point 1 (**Working**), the sentence and the table row:

> **Pure crates** have no I/O and no `unsafe`, and are fuzzed where they
> decode:
>
> | `ironafs-proto` | volume location, file service, callback and protection types and their generated codecs; the AFS directory format; wire time conversion (C38) |

ADR 0004, C6, point 1 (**Firm**):

> Each protocol component is written from a behaviour specification in
> `docs/specs/` that names its sources, under ADR 0001. No specification, no code.

ADR 0004, C6, point 3 (**Firm**), the lead-in and the item this
specification answers:

> **Specification gates.** These are written and reviewed before the code
> that depends on them:
>
> […]
>
> - before the cache: what evidence identifies a volume's incarnation
>   (C25), the directory format and the callback procedures;

ADR 0004, C6, point 4 (**Firm**):

> A fact about the AFS protocols that this ADR could not tie to a public
> source is an input to a specification, never something to code from.

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

ADR 0004, L1 (**Firm**), the lead-in and points 2 and 5:

> On every OS a layer never:
>
> […]
>
> 2. turns an error into success, an empty or short listing, zero bytes or a
>    made-up attribute;
>
> […]
>
> 5. rewrites or normalises a name on the server, or presents a name it cannot
>    reverse;

ADR 0004, L25, point 2 (**Firm**):

> **Never silently partial.** No entry is omitted from a listing. If an
> entry cannot be presented safely, the listing fails explicitly.

ADR 0004, L29, point 1 (**Firm**):

> The core keeps a name's raw bytes and never normalises.

L1 binds the layers and L25 the Windows layer. This specification keeps
the same rules one step earlier, in the decoder, so that no layer is ever
handed a listing that a hostile page has already made short or ambiguous
(5.4, point 13).

Section 9 shows where each is kept and which test fails if it is broken.

## 2. Scope

1. **Covered:** the bytes of one AFS directory object as the client
   receives it, the layout as far as published documents describe it
   (5.1), the caps (5.2), every check the reader applies and their order
   (5.4), the decoded result and how lookup and enumeration use it (5.6),
   the arithmetic (6), the errors (7), and the tests (10).
2. **Not covered, and where it is covered:**
   - how the bytes are fetched, from which call, in how many ranges, and
     how the client knows they all belong to one data version: the
     specification of the volume location and file service read
     procedures (#65). This specification takes one complete object at
     one data version as its input (5.4, point 1);
   - caching the decoded result, directory snapshots, enumeration cookies
     and the rights needed to use them: ADR 0004, C22, points 6 and 7, and
     the cache's specifications;
   - how a name is presented on each operating system, and whether it can
     be reversed: the name codec, specified with the contract (#67; ADR
     0004, L24, L25 and L29). The decoder hands the codec raw bytes;
   - what `.` and `..` resolve to, mount points, and crossing a volume
     root: the namespace specification (#80);
   - producing or editing a directory object: not covered. M2a is
     read-only. If IronAFS ever edits a cached directory, that is a new
     revision with its own review;
   - the format a fileserver uses on its own disk: not IronAFS's concern.
3. **The legacy format** that an older page-count value marks (5.3) is
   not described by any registered source. It is rejected, not specified
   (5.4, point 3; section 13, Q3).

## 3. The design in short

1. A directory is fetched as an object of bytes, and the client looks
   names up in it itself. The object is a sequence of fixed-size pages,
   each a sequence of fixed-size records. One page holds a header that
   points to chains of entries by a hash of their names. An entry is one
   record, followed by more records when its name is long
   `[I1 §§1, 4 to 9: UNVERIFIED]`.
2. Every layout fact above and in 5.1 comes from one expired Internet-Draft
   that no independent source corroborates yet, and that disagrees with
   another lead on one point (5.1, point 14). So the layout is a set of
   named constants and offsets, each UNVERIFIED, which the capture-backed
   revision confirms.
3. The reader is written so that its safety does not depend on those
   values being right, as long as the relations among them that 5.2,
   point 4 asserts at build time hold. Every index read from the object is range-checked
   before use; every chain walk is bounded and detects a record reached
   twice; every entry and its name stay inside one page and inside the
   object; no two entries share a record.
4. The reader decodes the whole object once into one set of entries,
   reached through the hash chains, and both lookup and enumeration use
   that set, so they can never disagree.
5. Names are raw bytes. An empty name, a name with `/`, a misplaced or
   repeated `.` or `..`, a name that appears twice, and a name on the
   wrong chain each make the whole object invalid. The reader never drops
   an entry to make the rest usable: an invalid object is an error, never
   a shorter listing.
6. Fields only a writer needs (free-space bookkeeping, allocation bits,
   flag bits, reserved octets) are not read at all.

## 4. Terms

- **Directory object:** the bytes of one directory at one data version, as
  fetched from the fileserver.
- **Page:** a fixed-size slice of the object, `dir.page_size` octets,
  starting at a multiple of `dir.page_size`.
- **Record:** a fixed-size slice of a page, `dir.record_size` octets.
- **Record index:** a record's position in the whole object, counted in
  records from the start of the object.
- **Slot:** a record's position within its page, from 0.
- **Page header:** the record in slot 0 of every page.
- **Directory header:** the records of page 0 after its page header that
  hold the bucket heads.
- **Bucket:** one of `dir.buckets` hash chains. Its **head** is the record
  index of the first entry on the chain, or 0 for an empty chain.
- **Entry:** one name and the file identifier it maps to. Its **first
  record** holds the fixed fields and the start of the name; its
  **continuation records** hold the rest of a long name.
- **Span:** the records an entry occupies, from its first record through
  the record that holds its name's terminating zero octet.
- **Name:** the octets of an entry's name before the first zero octet.
- **Own identifier:** the vnode and uniquifier of the directory being
  decoded, which the caller passes in.

## 5. Specification

### 5.1 Layout

Every point in this section is a lead from I1, in IronAFS's own words. It
is UNVERIFIED: nothing is coded from it until the capture-backed revision
confirms it (section 13, Q1). The constant names are IronAFS's own.

1. A directory object is between 1 and `dir.pages_max` = 1023 pages
   `[I1 §4: UNVERIFIED]`.
2. A page is `dir.page_size` = 2048 octets, made of
   `dir.records_per_page` = 64 records of `dir.record_size` = 32 octets
   `[I1 §§3, 5: UNVERIFIED]`.
3. Record index *r* is at object offset *r* × 32, so it lies in page
   ⌊*r* ÷ 64⌋, slot *r* mod 64 `[I1 §5.1: UNVERIFIED]`.
4. **Page header,** slot 0 of every page, 32 octets
   `[I1 §5.2: UNVERIFIED]`:

   | Offset | Width | Field | Meaning |
   |---|---|---|---|
   | 0 | 2 | page count | Meaningful in page 0 only: the number of pages in the object, unsigned, most significant octet first. Zero marks the legacy format (5.3). |
   | 2 | 2 | tag | The constant `dir.tag` = 1234, unsigned, most significant octet first. |
   | 4 | 1 | reserved | |
   | 5 | 8 | allocation bits | One bit per slot of the page, for a writer's use. |
   | 13 | 19 | reserved | |

5. **Page 0** holds its page header in slot 0, the directory header in
   slots 1 to 12 (`dir.header_records` = 12 records, 384 octets), and
   entries' records in slots 13 to 63 `[I1 §§3, 6: UNVERIFIED]`.
6. **Directory header,** object offsets 32 to 415 `[I1 §6.1: UNVERIFIED]`:

   | Offset | Width | Field | Meaning |
   |---|---|---|---|
   | 32 | 128 × 1 | page map | A writer's free-record count for each of the first 128 pages. |
   | 160 | 128 × 2 | bucket heads | `dir.buckets` = 128 record indexes, one per bucket; 0 for an empty chain. |

7. **Every other page** holds its page header in slot 0 and entries'
   records in slots 1 to 63 `[I1 §7: UNVERIFIED]`.
8. **An entry's first record,** 32 octets `[I1 §8.1: UNVERIFIED]`:

   | Offset in record | Width | Field | Meaning |
   |---|---|---|---|
   | 0 | 1 | flags | A writer's marker; I1 says it should not be relied on. |
   | 1 | 1 | reserved | |
   | 2 | 2 | next | Record index of the next entry on the same chain; 0 ends the chain. |
   | 4 | 4 | vnode | The target's vnode number, unsigned. |
   | 8 | 4 | uniquifier | The target's uniquifier, unsigned. |
   | 12 | 20 | name | The first 20 octets of the name and its terminator. |

9. **A continuation record** is 32 octets of name. The last record of a
   span holds the name's terminating zero octet `[I1 §8.2: UNVERIFIED]`.
10. An entry's records are contiguous and never cross a page boundary
    `[I1 §8: UNVERIFIED]`.
11. **Byte order.** I1 states the byte order of the page count and the tag
    only. Its figures are drawn most significant bit first, which suggests
    the same order for `next`, the vnode, the uniquifier and the bucket
    heads, but its text does not say so `[I1 §§4 to 8: UNVERIFIED]`
    (section 13, Q2).
12. **The bucket of a name** `[I1 §9: UNVERIFIED]`: an unsigned 32-bit
    value starts at 0; for each octet of the name, in order and before the
    terminator, it is multiplied by 173 and the octet's value is added,
    both modulo 2³². If the result is below 2³¹, the bucket is the result
    modulo 128; otherwise it is 128 minus (the result modulo 128). I1
    does not say whether an octet of 0x80 or more counts as its unsigned
    value, and its rule yields 128, outside the 128 buckets, for some
    names (section 13, Q5).
13. **Derived, not stated by any source:** the longest name that fits in
    one span is 2003 octets (a first record in slot 1 and 62 continuation
    records give 20 + 62 × 32 = 2004 octets, less the terminator), and
    1619 octets in page 0; an object holds at most 51 + 1022 × 63 =
    64,437 entries. These follow from points 1 to 10 and fall with them
    `[OWN]`.
14. **Where the leads disagree.** F1 says a name of up to 15 octets
    needs one record, and that each further record adds 32 octets of
    name `[F1, "Cautions": UNVERIFIED]`. Point 8 gives the first record
    20 octets of name and terminator, so a name of 16 to 19 octets would
    need one record by I1's field layout and two by F1. I1's own worked
    example (its Appendix A) agrees with F1: a name of 18 octets there
    occupies two records. A writer may allocate more records than the
    name needs. The reader in 5.4 does not depend on which is right: it
    finds a name's end by its terminator, and a span is only the records
    the name reaches (5.4, point 9). Which rule writers follow is
    section 13, Q7. F1's count of about 64,000 entries is consistent
    with point 13's figure, but F1 is itself a lead, so it corroborates
    nothing.

### 5.2 Caps

Each cap is a named constant. Its value follows from 5.1 and changes with
it when the capture-backed revision confirms or corrects the layout `[OWN]`:
a cap that rests on an UNVERIFIED value is UNVERIFIED with it.

| Cap | Bounds | Value from 5.1 | Checked |
|---|---|---|---|
| `dir.object_max` | the object's length | `dir.pages_max` × `dir.page_size` = 2,095,104 octets | before anything is read (5.4, point 1) |
| `dir.records_max` | record indexes in any object, chain steps, entries | `dir.pages_max` × `dir.records_per_page` = 65,472 | by the records present, which never exceed it (6, point 3; 5.4, point 6) |
| `dir.name_max` | octets scanned for a terminator | to the end of the entry's page (5.1, point 13) | each name (5.4, point 9) |
| `dir.budget` | bytes the decoder requests for one decode: the result and its working state | what the largest valid object needs, from the reservations of point 3 | before each request (6, point 6) |

1. No cap is read from the input, and every cap is a constant. The
   records present in an object are a bound computed from its length,
   which point 2 of 5.4 has already checked against `dir.object_max`;
   the page count is only compared with that length (5.4, point 3)
   `[OWN]`: a hostile page count must not size anything.
2. Chain walks are bounded by the records present, so by
   `dir.records_max`, in steps in total across all chains, since 5.4,
   point 7 refuses any record reached twice `[OWN]`.
3. `dir.budget` is the decoder's ledger, in the manner of the XDR
   profile's memory budget ([xdr.md](xdr.md), 5.2, point 9): every heap
   request, for the result or for working state (the records reached,
   the spans, the index of names seen), goes through one reservation
   routine that counts requested bytes, cumulatively for the decode, and
   checks the new total before the request is made; nothing is
   subtracted when memory is freed `[OWN]`. So that the count never
   meets growth, each buffer is reserved once, fallibly, for a bound
   computed from the length before decoding starts: entries and the
   working state for the records present, name octets for the length
   itself. A request that would pass `dir.budget` is `BudgetExceeded`; a
   reservation within it that the allocator refuses is
   `AllocationFailed`. The value of `dir.budget` is a constant, fixed
   with the decoded type in the capture-backed revision as what these
   reservations need for an object of `dir.object_max` octets, so the
   largest valid object decodes within it (10.1, G12).
4. **The layout's constants hold together.** The reader's safety holds
   whatever values 5.1 is confirmed to have only while these relations
   hold among them, so each is a compile-time assertion beside the
   constants, and a revision that breaks one does not build `[OWN]`:
   - `dir.record_size`, `dir.records_per_page`, `dir.pages_max` and
     `dir.buckets` are above zero;
   - `dir.page_size` = `dir.records_per_page` × `dir.record_size`;
   - the page header's fields (5.1, point 4) and an entry's fixed fields
     (5.1, point 8) end within one record, and the name's offset in a
     first record is below `dir.record_size`;
   - page 0 keeps at least one slot for entries: 1 +
     `dir.header_records` < `dir.records_per_page`;
   - the bucket heads (5.1, point 6) end within the directory header, at
     or before object offset (1 + `dir.header_records`) ×
     `dir.record_size`;
   - `dir.object_max` and `dir.budget` fit in `usize` on every target,
     and `dir.records_max` is at most 65,536, since no record index the
     object can hold is larger.

### 5.3 Versions

1. This is a peer's format. IronAFS defines no version of it and freezes
   nothing (README, "Formats IronAFS does not control").
2. I1 names two variants: the format it describes, marked by a non-zero
   page count in page 0, and an older format marked by a page count of
   zero, which it does not describe `[I1 §5.2: UNVERIFIED]`.
3. The reader accepts only the first. An object whose page count is zero
   is rejected with `LegacyFormat` (5.4, point 3) `[OWN]`: no admissible
   source describes the older format, so it cannot be read safely. Whether
   supported fileservers still send it is section 13, Q3.

### 5.4 Decoding

The input is the object's bytes and the directory's own identifier.
Points 1 to 5 apply to the object once. Points 6 to 13 then apply to one
record index at a time, in the walk order of point 7: each index, and
the entry it starts, passes every check in point order before the walk
reads the next index. The first check that fails ends decoding with its
error (section 7), and nothing decoded so far is returned `[OWN]`. The
order is part of the contract: one object gives one error in every
implementation, and no octet is read before the checks that guard it.

1. **One whole object.** The caller passes the complete object of one
   data version. Assembling it, and proving every range came from the same
   data version, is the fetch specification's duty (#65). This decoder
   never sees a partial object `[OWN]`: a directory decoded from ranges of
   two versions could show an entry twice or not at all.
2. **Length.** The length is at least `dir.page_size`, a whole multiple of
   it, and at most `dir.object_max`. Otherwise `BadLength` `[OWN]`.
3. **Page 0's header.** Its tag is `dir.tag`, or `BadTag` with page 0. Its
   page count is not zero (`LegacyFormat`), not above `dir.pages_max`, and
   equal to the length divided by `dir.page_size` (`PageCountMismatch`)
   `[OWN]`. The equality is strict until observation shows a supported
   server sends pages past the count (section 13, Q4); a count above the
   pages present must always fail, since it would index past the object.
4. **Every page's header.** Each page's tag is `dir.tag`, or `BadTag` with
   that page `[OWN]`. Strict for the same reason and with the same
   question as point 3.
5. **Ignored fields.** The page map, the allocation bits, the flags, every
   reserved octet, and the octets of a span after the terminating zero
   are never read `[OWN]`. They are a writer's bookkeeping or undefined
   (I1's example shows undefined octets after a name). The reader's
   safety rests on its own checks (points 6 to 10), and its tolerance of
   these fields is total: any value is accepted. Their bit order and
   meaning are unconfirmed, and trusting them would buy nothing.
6. **Each record index** read from the object, as a bucket head or a
   `next`, is checked before use `[OWN]`:
   - 0 is the end of a chain, never a record;
   - otherwise it is below the records present (6, point 3), or
     `IndexOutOfRange`;
   - its slot is not 0 (a page header), and in page 0 not within the
     directory header (slots 1 to `dir.header_records`), or
     `IndexIntoHeader`.
7. **Chains.** The decoder walks the bucket heads in bucket order, and
   each chain through `next`. A record index reached a second time, on
   the same chain or another, is `RecordReused` `[OWN]`: it stops a
   cycle, and a chain that joins another, in at most `dir.records_max`
   steps. Then, before any octet of its record is read, an index that
   lies inside the span of an entry already decoded is `SpanOverlap`
   (point 9). An entry's `next` is read with its fixed fields (point 8),
   and the walk takes it, through point 6, only after the entry has
   passed points 9 to 13.
8. **Each entry's fixed fields** are read from its first record: `next`,
   the vnode and the uniquifier, at the offsets of 5.1, point 8, in the
   byte order of 5.1, point 11.
9. **Each entry's name and span.** The name starts at offset 12 of the
   first record. The decoder reads it one record at a time, from there to
   the end of the first record's page and no further, and stops at the
   first zero octet. Before it reads any octet of a continuation record,
   that record must not lie inside the span of an entry already decoded:
   otherwise `SpanOverlap` `[OWN]`. No zero octet before the page's end:
   `NameUnterminated` `[OWN]`. The span is the first record through the
   record holding that zero octet; the octets after it are never read
   (point 5). With point 7's check of the first record, this keeps every
   record in at most one span, whichever entry was reached first. Because
   the search stops at the page's end, a span never crosses a page or
   leaves the object.
10. **Each name** `[OWN]`, from ADR 0004, L1, point 5, and L29, point 1:
    - empty (the zero octet is the first octet): `NameEmpty`;
    - contains `/` (0x2F): `NameHasSlash`, since no layer could present
      it as one path component;
    - an embedded zero octet cannot occur here, since the name ends at
      the first; lookup applies the same rule to the name it is given and
      refuses one that contains a zero octet with `NameHasZero` (5.6,
      point 3);
    - the octets are kept exactly as they are: never normalised, case
      folded, trimmed or decoded as text.
11. **The chain is the name's chain.** The bucket of each name, computed
    as 5.1, point 12 says, equals the bucket whose chain reached it.
    Otherwise `WrongBucket` `[OWN]`: an entry on the wrong chain is one
    that a client looking it up by hash would not find, so a listing and a
    lookup would disagree. A computed bucket outside 0 to
    `dir.buckets` − 1 is `WrongBucket` too. This check rests on the hash
    of 5.1, point 12, which section 13, Q5 must settle first.
12. **Names are unique.** A name, compared as raw octets, that appears in
    two entries is `DuplicateName` `[OWN]`: lookup could not choose, and
    the exact-match rules of ADR 0004, L25, point 4 and L29, point 4 rest
    on an exact name naming one entry.
13. **`.` and `..`** `[OWN]`:
    - more than one entry named `.`, or more than one named `..`:
      `DuplicateName` (point 12);
    - an entry named `.` whose identifier is not the own identifier:
      `DotWrong`;
    - an entry with any other name whose identifier is the own
      identifier, `..` included: `SelfEntry`, since it would make the
      directory its own child and loop every walk of the tree. Whether a
      volume's root directory has a `..` naming itself is section 13, Q6;
      until it is settled, this rule applies to every directory, and Q6
      may add one named exception;
    - whether `.` and `..` must be present, and where, is section 13,
      Q6. This version does not require them, and the decoder does not
      use their targets for anything: the namespace specification (#80)
      decides how `.` and `..` resolve.
14. **The result** is the set of entries, each its name's octets, its
    vnode and its uniquifier, ordered by the record index of its first
    record (5.6, point 1). Records no chain reaches are not entries and
    are not read `[OWN]`.
15. **What is not checked here:** whether a vnode or uniquifier value is
    one a fileserver could issue (section 13, Q9), and whether a name can
    be presented and reversed on an operating system: the name codec
    decides that, and an entry it cannot present makes the listing fail
    explicitly (ADR 0004, L1, point 5; L25, point 2), never vanish.

### 5.5 Encoding

Not applicable: the client does not produce directory objects (section 2,
point 2). Tests build objects with a test-only builder written from 5.1,
whose output is checked by hand (10.1).

### 5.6 States and ordering

1. **One decode per object.** A directory object is decoded once, as a
   whole, before any name in it is used. Lookup and enumeration both use
   the one decoded set, never the raw bytes `[OWN]`: so a name that a
   listing shows is a name a lookup finds, and the reverse.
2. **Enumeration order** is by the record index of each entry's first
   record, which is stable for a given object and needs no hash `[OWN]`.
   Cookies and snapshots are the cache's (section 2).
3. **Lookup** takes a name as raw octets and checks it before any
   search, by the rules of 5.4, point 10, in this order: an empty name
   is `NameEmpty`, a name containing a zero octet is `NameHasZero`, and
   a name containing `/` is `NameHasSlash`. So the decoder and lookup
   apply one rule `[OWN]`. Otherwise the result is the one entry whose
   name is equal octet for octet, or none. `.` and `..` are entries like
   any other when the object holds them, so lookup finds them and point
   1 holds for them too. What they resolve to is the namespace's (#80):
   the decoder checks the target of `.` but not of `..` (5.4, point 13),
   so the target lookup returns for `..` is not a verified parent
   `[OWN]`.
4. **Nothing partial survives an error.** A failed decode leaves no
   entries behind, and the object is not used for lookup or listing
   (section 7) `[OWN]`.

## 6. Arithmetic

Every value below comes from the object. Each operation is checked; none
wraps, except the hash, whose wrapping is the defined behaviour `[OWN]`.

1. **Record index to offset:** *r* × `dir.record_size`. *r* is at most
   65,535 (a 16-bit field), so the product is below 2²¹ and fits a 32-bit
   value; it is computed with checked multiplication in `usize`, after
   *r* is checked against the records present (5.4, point 6), so the
   record ends within the length.
2. **Page and slot:** *r* ÷ `dir.records_per_page` and *r* mod
   `dir.records_per_page`. The divisor is a non-zero constant.
3. **Pages and records present:** the pages present are the length ÷
   `dir.page_size`, after 5.4, point 2; the records present are the
   pages present × `dir.records_per_page`, computed with checked
   multiplication in `usize`. Both come from the length, never from the
   page count, and the records present are at most `dir.records_max`
   (5.2, point 1).
4. **Name search end:** the end of the first record's page, (page + 1) ×
   `dir.page_size`, checked against the length. A span's last record is
   the offset of the zero octet ÷ `dir.record_size`.
5. **The bucket of a name:** multiplication by 173 and addition, modulo
   2³², with explicit wrapping operations, since 5.1, point 12 defines the
   value that way. The final step's subtraction, 128 minus a value from 0
   to 127, cannot underflow. Its result is checked against `dir.buckets`
   (5.4, point 11).
6. **The ledger:** each allocation request is added with checked addition
   and compared with `dir.budget` before the allocation (5.2, point 3).
   An overflow is `BudgetExceeded`.
7. **Reads:** every read of a header field, a record or a name octet
   takes a bounds-checked slice of the object (`get`, never indexing),
   so a fault in any check above becomes an error, never a read past the
   object (coding standards rules 1 to 3).

## 7. Errors and failure

1. Every rejection is a value of one error type, `DirError`, with these
   kinds: `BadLength`, `LegacyFormat`, `PageCountMismatch`, `BadTag`,
   `IndexOutOfRange`, `IndexIntoHeader`, `RecordReused`,
   `NameUnterminated`, `SpanOverlap`, `NameEmpty`, `NameHasZero`,
   `NameHasSlash`, `WrongBucket`, `DuplicateName`, `DotWrong`,
   `SelfEntry`, `BudgetExceeded` and `AllocationFailed` `[OWN]`.
   `NameHasZero` comes from lookup only (5.6, point 3).
2. An error carries the kind and the location where the check failed, as
   the table below sets. A location is a position, never a value: a page
   number, a record index that has passed 5.4, point 6, or the object
   offset of a field. An error never carries octets of a name, an index
   that failed 5.4, point 6, a vnode, a uniquifier, a page count or any
   other value read from the object `[OWN]`: names can be sensitive (I1
   §12 says as much), and errors reach logs and events.

   | Kinds | Location |
   |---|---|
   | `BadLength`, `BudgetExceeded`, `AllocationFailed` | none |
   | `LegacyFormat`, `PageCountMismatch` | page 0 |
   | `BadTag` | the page whose tag is wrong |
   | `IndexOutOfRange`, `IndexIntoHeader`, `RecordReused` | the object offset of the bucket head or `next` field that held the index |
   | `NameUnterminated`, `SpanOverlap`, `NameEmpty`, `NameHasSlash`, `WrongBucket`, `DuplicateName`, `DotWrong`, `SelfEntry` | the record index of the first record of the entry being checked: for `SpanOverlap` and `DuplicateName`, the entry reached second |
   | `NameEmpty`, `NameHasZero`, `NameHasSlash` from lookup | none: they describe the caller's name, not the object |
3. No panic is reachable from any input (coding standards rules 1 to 3).
4. A failed decode fails closed: the caller gets the error, and every
   lookup and listing that needed the object fails explicitly; none
   returns an empty or short listing or a made-up entry (ADR 0004, L1,
   point 2) `[OWN]`. What the cache then does (fetching again, marking
   the directory unusable, raising an event) is the cache's
   specification's choice; it never retries the same bytes in a loop.

## 8. Security

Who can change the input: the fileserver; anyone on the network path
while the null security class is in use (M2a reads unauthenticated, so
this is the case at first); and, for a cached copy, whatever the cache's
own integrity rules allow (the cache's specifications).

| What a hostile object tries | What it could gain | What stops it |
|---|---|---|
| An index past the object, into a header, or a page count larger than the object | Out-of-bounds read, a panic, a crash of the daemon | 5.4, points 2, 3 and 6 |
| A cycle in a chain, or two chains joined | An endless walk, a hang | 5.4, point 7; 5.2, point 2 |
| A name with no terminator, or one that runs past its page | A read past the page or the object | 5.4, point 9 |
| Two entries sharing records | One name's octets read as another's fields | 5.4, points 7 and 9 |
| A huge object or many entries | Memory exhaustion | 5.2, points 1 and 3; 5.4, point 2; 6, point 6 |
| A name with `/`, an empty name | A path that means something else to a layer or a tool | 5.4, point 10 |
| A repeated name, a name on the wrong chain | A listing and a lookup that disagree; an ambiguous lookup that a destructive operation could hit | 5.4, points 11 and 12; 5.6, point 1 |
| A `.` naming another directory, an entry naming the directory itself | A loop in the tree; a walk that never ends; `.` pointing a tool elsewhere | 5.4, point 13 |
| A name that cannot be presented on one operating system | An entry silently hidden from one platform | 5.4, point 15; ADR 0004, L25, point 2 |
| A bucket value outside the table | An index past the bucket table | 5.4, point 11; 6, point 5 |
| Names in errors and logs | Leaking a private file name | 7, point 2 |

What the format cannot protect: a server, or an attacker on an
unprotected path, can send a different, well-formed directory. That is
the security class's concern, not this decoder's. The decoder guarantees
only that a well-formed object is read safely and that a malformed one is
refused.

## 9. How the invariants are kept

| Invariant | Kept by | Test that fails if it is broken |
|---|---|---|
| Constraint 4: no panics, checked arithmetic, byte limits | 5.2, 5.4, 6, 7; the compile-time assertions of 5.2, point 4 | the fuzz target (10.4); `decode_rejects_*` cases of 10.3 |
| C4, point 1: `ironafs-proto` is pure, has no I/O and no `unsafe`, and is fuzzed | the decoder is a function of bytes and the own identifier (5.4) | the pure-crate check of `cargo xtask check` (#46); the fuzz target (10.4) |
| C6, points 1 and 3: no code before this specification is reviewed | Status line; README "Status" | review: no decoder merges while the status is `draft` |
| C6, point 4, and N31, points 1 to 3: nothing coded from an unconfirmed fact | 5.1, every point UNVERIFIED; section 13, Q1 | review of the capture-backed revision: each constant in 5.1 cites a fixture or an allowlisted passage before code uses it |
| L1, point 2, and L25, point 2: no empty or short listing in place of an error | 5.4, points 13 to 15; 5.6, point 4; 7, point 4 | `decode_rejects_whole_object_on_one_bad_entry` (10.3, R23) |
| L1, point 5, and L29, point 1: raw octets, never normalised | 5.4, point 10; 5.6, point 3 | `decode_keeps_name_octets_exactly` (10.1, G6) |

## 10. Tests

All paths are in the `ironafs-proto` crate, laid out as coding standards
rules 15 and 20 set out. The tests build objects with a test-only
builder, `crates/ironafs-proto/tests/it/directory/builder.rs`, written
from 5.1 and taking every constant from the decoder's named constants, so
the capture-backed revision changes values in one place `[OWN]`.

Until section 13, Q1 closes, these tests prove only that the decoder
meets this specification. They prove nothing about interoperability. The
capture-backed revision adds reduced, inspected fixtures under
`crates/ironafs-proto/tests/fixtures/directory/`, each with its fixture
provenance (README, "Sources and provenance", rule 4). No fixture or
vector is ever taken from an implementation's test data, and none from
I1's worked example (README, "Test vectors and fuzz targets", point 1).

### 10.1 Golden vectors

In `crates/ironafs-proto/tests/it/directory/golden.rs`. Each object is
built by hand from 5.1 at the offsets given; the derivation is a comment
beside the vector. Names and identifiers are IronAFS's own.

| Id | Object | Expected result | Proves |
|---|---|---|---|
| G1 | One page: page count 1 at offset 0, tag at offset 2, all bucket heads 0 | an empty set | the smallest valid object; 5.4, points 2 to 4 |
| G2 | One page, one entry in slot 13 (record index 13) named `a` (bucket 97: 0x61), head of bucket 97 = 13, `next` 0, vnode 7, uniquifier 9 | `{a → (7, 9)}` | the first record's offsets; the bucket rule's first branch |
| G3 | As G2, plus `ab` (bucket 111) in slot 14 | two entries, `a` then `ab` | enumeration order by first record (5.6, point 2) |
| G4 | Two entries in one bucket, linked by `next` | both, in first-record order | chain walking (5.4, point 7) |
| G5 | One entry with a 40-octet name: first record plus continuation records to the terminator | the full name | spans and continuation records (5.4, point 9) |
| G6 | Names with octets 0x01 and 0x7F, the name 0x61 0xC3 0xA9 (`aé` in UTF-8; bucket 57 whether octets of 0x80 or more count as unsigned or signed), a name with a trailing space and one with a trailing `.` | each name's octets unchanged | raw names (5.4, point 10) |
| G7 | Two pages, page count 2, an entry in page 1 slot 1 (record index 65) | the entry | record index across pages (5.1, point 3) |
| G8 | The name `hello`, whose 32-bit value is 0xCFFB95C8 (at least 2³¹), in bucket 56 | the entry | the bucket rule's second branch |
| G9 | A 2003-octet name from page 1 slot 1 to slot 63 | the name | the longest span (5.1, point 13) |
| G10 | Non-zero page map, allocation bits, flags and reserved octets, and octets after a terminator | the same set as without them | 5.4, point 5: these fields are ignored |
| G11 | `.` naming the own identifier, and `..` naming another directory | both entries; lookup of each finds it | 5.4, point 13; 5.6, point 3 |
| G12 | `dir.pages_max` pages, every entry slot holding an entry of one record (64,437 entries) | the full set, with the ledger within `dir.budget` | 5.2, point 3: the largest valid object fits the budget |

Every vector that holds an entry (G2 to G12) places it on its name's
chain, so all of them rest on the bucket rule of 5.1, point 12 and change
if section 13, Q5 corrects it; G2, G3 and G8 also state bucket values.
G6's high-octet name is chosen so that its bucket is the same under both
readings that Q5 leaves open, so G6 does not wait on that part of Q5.

### 10.2 Round-trip properties

In `crates/ironafs-proto/tests/it/directory/props.rs`, with the
project's own seeded generator.

1. **P1:** for any set of distinct, valid names with identifiers, placed
   by the builder, decoding gives exactly that set.
2. **P2:** decoding the same bytes twice gives the same result, or the
   same error.
3. **P3:** changing any one octet of a valid object gives either a valid
   set or one of the errors of section 7, never a panic, and a valid set
   only if every check of 5.4 still holds for it.
4. **P4:** changing only ignored fields (5.4, point 5) never changes the
   result.
5. **P5:** lookup of every listed name finds that entry, and lookup of a
   name not listed finds nothing (5.6, points 1 and 3).

### 10.3 Reject corpus

In `crates/ironafs-proto/tests/it/directory/reject.rs`, each as
`decode_rejects_<case>`, asserting the exact error kind and its location
(section 7, point 2).

| Id | Input | Error | Breaks |
|---|---|---|---|
| R1 | 0 octets; 2047 octets; 2049 octets | `BadLength` | 5.4, point 2 |
| R2 | 1024 pages | `BadLength` | 5.4, point 2 |
| R3 | page count 0 | `LegacyFormat` | 5.4, point 3 |
| R4 | page count 2, one page present; page count 1, two pages present | `PageCountMismatch` | 5.4, point 3 |
| R5 | page 0's tag 1235 | `BadTag`, page 0 | 5.4, point 3 |
| R6 | page 1's tag wrong in a two-page object | `BadTag`, page 1 | 5.4, point 4 |
| R7 | a bucket head of 64 in a one-page object | `IndexOutOfRange` | 5.4, point 6 |
| R8 | a bucket head of 5 (inside the directory header); of 64 in a two-page object (page 1's header) | `IndexIntoHeader` | 5.4, point 6 |
| R9 | an entry whose `next` is itself | `RecordReused` | 5.4, point 7 |
| R10 | two buckets whose chains reach the same record | `RecordReused` | 5.4, point 7 |
| R11 | a chain of three that loops to its first | `RecordReused` | 5.4, point 7 |
| R12 | a name with no zero octet before its page's end | `NameUnterminated` | 5.4, point 9 |
| R13 | an entry whose first record lies inside another entry's span, once with that entry reached first and once with it reached second | `SpanOverlap` | 5.4, points 7 and 9 |
| R14 | an entry whose span covers another entry's first record | `SpanOverlap` | 5.4, point 9 |
| R15 | a first octet of zero | `NameEmpty` | 5.4, point 10 |
| R16 | `a/b` | `NameHasSlash` | 5.4, point 10 |
| R17 | `a` placed on bucket 98's chain | `WrongBucket` | 5.4, point 11 |
| R18 | `baacy`, whose bucket by 5.1, point 12 is 128 | `WrongBucket` | 5.4, point 11; section 13, Q5 |
| R19 | the name `x` twice, on one chain | `DuplicateName` | 5.4, point 12 |
| R20 | two entries named `.`; two named `..` | `DuplicateName` | 5.4, point 13 |
| R21 | `.` naming another identifier | `DotWrong` | 5.4, point 13 |
| R22 | `sub` naming the own identifier; `..` naming the own identifier | `SelfEntry` | 5.4, point 13 |
| R23 | one valid entry and one entry with `/`, on different chains | `NameHasSlash`, and no entries returned | 5.4, points 10 and 14; 5.6, point 4 |
| R24 | a budget set below what a valid object needs | `BudgetExceeded` | 5.2, point 3; 6, point 6 |
| R25 | lookup of an empty name; of `a\0b`; of `a/b` | `NameEmpty`; `NameHasZero`; `NameHasSlash`; each with no location | 5.6, point 3; section 7, point 2 |

R18 and R22's `..` case rest on open questions Q5 and Q6, and change if
those close otherwise.

### 10.4 Fuzz target

`fuzz/fuzz_targets/ironafs-proto_directory.rs`, named `<crate>_<subject>`
under coding standards rule 20; where `fuzz/` sits rests on ADR 0004's
assumption A12. It decodes arbitrary bytes, with an own identifier taken
from the input, and asserts:

1. no panic, no overflow, no out-of-bounds access;
2. the ledger stays within `dir.budget`;
3. on success: every span lies inside one page of the object; no two
   spans share a record; every name is non-empty, without `/`, and
   unique; every name's bucket is the bucket that reached it; no entry
   other than `.` carries the own identifier;
4. the same input always gives the same result.

Its seed corpus is the golden vectors and the reject corpus.

### 10.5 Acceptance cases

Not applicable: this is a protocol specification; sections 10.1 to 10.4
are its tests. Interoperability is shown by the capture-backed revision's
fixtures (section 13, Q1).

### 10.6 Gaps

- Every test above checks the decoder against UNVERIFIED values until
  section 13, Q1 closes (#51).
- No fixture from a real fileserver yet (#51).

## 11. Revisit if

From the working decisions this rests on:

- ADR 0004, C4: "a boundary below forces a dependency that point 4
  forbids, or the first implementation of a crate shows it has no content
  of its own."

This specification's own choices:

- a capture shows any value in 5.1 is other than stated: 5.1, the caps of
  5.2 and the vectors of 10.1 change together;
- a supported fileserver sends a page count of zero (Q3), pages past the
  page count (Q4), or a page within the count whose tag is not `dir.tag`
  (Q4): 5.4, points 3 and 4 gain a tolerance, with its source;
- a supported fileserver sends an entry on a chain other than its name's
  bucket, or two entries with one name: 5.4, points 11 and 12 are
  reconsidered, since rejecting them would make a real directory
  unreadable;
- a volume's root directory is shown to carry `..` naming itself (Q6):
  5.4, point 13 gains that one exception;
- an independently provenanced edition of an older AFS document that
  describes the format is found (Q10), or I1's passages are allowlisted
  (Q11).

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| I1 | SPECIFICATION | T. Keiser, "AFS-3 Directory Object Type Definition", Internet-Draft draft-keiser-afs3-directory-object-00, 2012-04-23, expired, register of editions row E3 ([README](README.md)), <https://datatracker.ietf.org/doc/draft-keiser-afs3-directory-object/>. A lead only: no passage is allowlisted (register row E3), so every fact taken from it is UNVERIFIED. Its Appendix B is excluded by name. | 3, 5.1, 5.3, 5.4, 7 |
| F1 | DOCUMENTATION | OpenAFS, `fileserver(8)` reference page, section "Cautions", register of editions row E4 ([README](README.md)), <https://docs.openafs.org/Reference/8/fileserver.html>. A lead only (README, "Sources and provenance", rules 11 and 14). | 5.1, point 14 |
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; C4, C6, C22 ([part 1](../adr/0004-architecture-1-core.md)); L1, L24, L25, L29 ([part 2](../adr/0004-architecture-2-os-layers.md)); N31 ([part 4](../adr/0004-architecture-4-negotiation.md)); A12 | 1, 2, 5.4, 7, 9, 10, 11 |
| D2 | DECISION | [ADR 0001](../adr/0001-licence-and-dont-copy.md), "Don't copy" and "Test tools and test data" | 10, 12 |
| D3 | DECISION | [Coding standards](../coding-standards.md), rules 1 to 3, 15 and 20 | 6, 7, 10 |
| D4 | DECISION | [XDR profile](xdr.md), 5.2, point 9, for the ledger's method | 5.2 |
| O | OWN | Every point tagged `[OWN]`, with its reason in place | 5, 6, 7 |

No source code, test data or interface definitions of any AFS
implementation were read for this specification, and none are cited. No
copy of the 1991 AFS-3 programmer's references was used: the only copies
found sit in, or were relocated from, an implementation's repository,
which README rule 10 bars (section 13, Q10). I1 and F1 are cited for
facts only, in IronAFS's own words; neither is quoted, and no figure,
constant name, example or code from either is reproduced. The constant
names in this specification are IronAFS's own.

### 12.1 Observed behaviour

None yet. The capture-backed revision records its observations here, from
the observation lab (#51): a test venue with synthetic cell names, users,
keys and data, raw bytes first, a protocol analyser only as a later
cross-check.

## 13. Open questions

Each question below is answered by observation in the lab (#51) unless it
says otherwise. Q1 blocks all code written from this specification; the
others block the point they name.

- **Q1. Corroborate the layout.** Every point of 5.1 is UNVERIFIED. The
  capture-backed revision confirms or corrects each, from reduced
  black-box fixtures: directories of one and of several pages, short and
  long names, names on one chain, names with octets of 0x80 and more, an
  empty directory, a volume's root directory. Until then nothing is coded
  from 5.1, and the decoder does not merge.
- **Q2. Byte order** of `next`, the vnode, the uniquifier and the bucket
  heads (5.1, point 11). Settled by directories whose entries have known
  vnode numbers above 255.
- **Q3. The legacy format.** Does any supported fileserver send an object
  whose page count is zero, for example a directory created long ago and
  never rewritten? If so, 5.3 needs a source for that format; until one
  exists, such a directory cannot be read and fails explicitly.
- **Q4. Length and page count.** Is the object's length always the page
  count times `dir.page_size`? Can a page within the count be
  uninitialised? How a directory is fetched (whole, or in ranges) is #65's.
- **Q5. The bucket rule.** As I1 §9 is written, an octet of 0x80 or more
  has no stated value (as an unsigned octet, `é` in UTF-8, octets 0xC3
  0xA9, gives bucket 112; as a signed one, 16), and the plain ASCII name
  `baacy` gives bucket 128, outside the table. So I1's rule cannot be
  complete as written. Settled by fixtures with crafted names, including
  `baacy` and names with high octets. Until then 5.4, point 11 cannot be
  coded.
- **Q6. `.` and `..`.** Are they stored as entries? On which chains, and
  in which slots? What does a volume's root directory's `..` name? The
  answer may add a required presence rule or one named exception to 5.4,
  point 13.
- **Q7. Records per name.** Which rule do writers follow: I1's field
  layout or F1's 15-octet rule (5.1, point 14)? The reader does not
  depend on it; a future writer would.
- **Q8. Name contents.** Which octets do supported fileservers accept in a
  name a client creates (control octets, `/`, octets of 0x80 and more)?
  The reader's rules stand either way; the answer tells how often 5.4,
  point 10 fires on real directories.
- **Q9. Identifier values.** Is a vnode or uniquifier of 0 ever valid in
  an entry? Is there a parity or range rule? Until settled, the decoder
  accepts any value (5.4, point 15), and the file service's status reply
  for the target decides.
- **Q10. Older AFS documents.** The 1991 AFS-3 programmer's reference for
  the file server and cache manager interface may describe directories.
  No copy whose provenance is independent of every implementation's
  repository has been found, so none is registered or read (README,
  "Sources and provenance", rule 10). A qualifying edition, if found, is
  registered and corroborated before use.
- **Q11. Allowlisting I1.** I1 is a published Internet-Draft, but an
  expired individual draft at revision -00, never adopted (the AFS-3
  standardisation group's draft list shows its comment period as
  pending), whose own text is not self-consistent (Q5) and disagrees with
  F1 (Q7). This specification therefore allowlists none of it. Whether
  its passages may be allowlisted once fixtures agree with them, or
  fixtures alone are cited, is the owner's decision on clean-room policy
  (ADR 0001; README, "Sources and provenance"). Either way no code waits
  on it: a confirming fixture is itself a route (ADR 0004, N31, point 1).

## 14. Review

Not yet reviewed. The record will be in `docs/reviews/`.
