<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The namespace: mount points, volume selection and `@sys`

Status: draft (2026-11-21). Raised in issue #80.

Kind: behaviour, with one protocol part (5.1, the mount-point target, a
string a fileserver sends). Class: S. A hostile or broken fileserver, or
anyone on the path while the null security class is in use, controls the
target string, and the selection rule decides which volume, in which
cell, a path reaches ([README](README.md), "Review class per kind": a
decoder of untrusted input, and a behaviour specification touching the
cache and authorisation).

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (M1, G1 and so
on) are in section 12.

**This is the documents-only version.** The published documents
registered here describe mount points and `@sys` as an administrator sees
them: the notation a command prints, and the rules a client follows. None
of them states how a mount point is stored or marked in the file service,
what a volume's root directory is called, or what default system-name
list a client starts with (section 12, "What was not used"). So every
point that depends on those facts is MISSING, and nothing is coded from it
until the observation lab's captures settle it (section 13, Q1 to Q5).
What this version does fix is everything that does not depend on them:
the grammar IronAFS accepts and everything it refuses, the depth and
cycle rule, the selection rule, the substitution rule and every bound.

## 1. What this implements

ADR 0004, overview, constraint 4 (a constraint of the whole ADR):

> **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
> local IPC, configuration, requests from the operating system, and SMB
> messages before authentication. No panics, checked arithmetic and byte
> limits apply to all of it (coding standards rules 1 to 3).

ADR 0004, C6, points 1 and 4 (**Firm**):

> 1. Each protocol component is written from a behaviour specification in
>    `docs/specs/` that names its sources, under
>    [ADR 0001](0001-licence-and-dont-copy.md). No specification, no code.
>
> 4. A fact about the AFS protocols that this ADR could not tie to a public
>    source is an input to a specification, never something to code from.
>    Each decision below says where it leans on one.

ADR 0004, C11, points 1, 3, 5, 8 and 9 (**Working**):

> 1. A node id is 64 bits. It maps to a key of cell, volume, vnode and
>    uniquifier, plus an **anchor**: how the volume was entered (the dynamic
>    root, a cell root, or a mount point). A mount point anchor is the node
>    id of that mount point, whose own key carries its own anchor, so the
>    anchor names the whole chain of mounts.
>
> 3. The anchor keeps every directory single-parented when one volume is
>    mounted in two places. A reverse index from file id to node ids sends
>    each callback break and each local change to every alias.
>
> 5. A file's read-write, read-only and backup volumes have different ids
>    ([OpenAFS administration guide, "About Volumes"](https://docs.openafs.org/AdminGuide/HDRWQ177.html)),
>    so their paths are different nodes.
>
> 8. A chain of mounts is bounded in depth and checked for cycles, as L32,
>    point 3, states. A chain that exceeds the bound or repeats a mount
>    point is refused, never followed.
> 9. A mount point resolves to a volume in the requesting context (C18,
>    point 1). The volume in a node's key is the one the mount point
>    resolved to in that context. Two contexts that resolve one mount point
>    to different volumes get different nodes.

ADR 0004, C12, points 1 to 3 (**Working**):

> 1. `/afs` is a dynamic root. It lists configured and discovered cells
>    without contacting them.
> 2. The attributes of a mount point that has not been resolved are
>    synthesised.
> 3. A lookup of a name that is not a cell is answered from a negative
>    cache, so an indexer or a mistyped path does not cause a DNS query each
>    time. This cache is machine-wide and part of cell discovery (C33), not
>    a cache entry of any context: it holds only that a name is not a cell.
>    It is filled only from a conclusive negative DNS answer, kept no longer
>    than that answer's time to live, never filled from a timeout or an
>    error, and cleared on a network change (C33, point 6).

ADR 0004, C18, point 1 (**Firm**):

> 1. Every private cache entry carries the id and credential generation of
>    the credential context (C40) that fetched it, and is read only under
>    them. Private entries are: data chunks, status, rights, directory
>    snapshots, lookups and negative lookups, the volume and location data
>    used to reach a file, fetches in flight, and dirty data. […]

ADR 0004, C19, point 1 (**Firm**):

> 1. Bytes, status, lookups, routing and grants obtained without
>    authentication (Rx's null class) are never used to answer a request
>    made under a credential, and never enter the cache of a context that
>    has one.

ADR 0004, C20, point 1 (**Working**):

> 1. Data from an unauthenticated call lives only in that request's buffers:
>    no cache entry, no shared fetch, no reuse.

ADR 0004, C22, point 6 (**Firm**):

> 6. A lookup hit (positive or negative), a listing, a resumed enumeration
>    and a step through a cached mount point each need the reader's own
>    current grant on every directory consulted.

ADR 0004, C46, point 4 (**Working**):

> 4. **A failure never looks like success:** never an empty or short
>    listing, zero bytes or a made-up attribute.

ADR 0004, L32, points 1, 3 and 4 (**Working**):

> 1. An AFS mount point appears as a directory. The core crosses it.
>
> 3. Mount-point and link depth are bounded everywhere. The visited set
>    covers mount points and the links the core resolves itself; links the
>    kernel resolves rely on the operating system's own hop limit.
> 4. The root lists cells without contacting them. Lookup, get attributes
>    and access on a cell's entry in the root return synthesised attributes
>    without contacting the cell (C12, point 2). Contact starts at a read of
>    the cell's directory or a lookup of a name inside it.

ADR 0004, L34, point 4 (**Working**):

> 4. On every surface except the replica: `@sys` and `@cell` substitution in
>    the core, with an ordered, configurable system-name list. On the
>    replica, `@sys` and `@cell` apply only through the explicit
>    resolution of a link the replica ADR specifies (L27, points 6
>    and 11). Everywhere: the read-only path by default and the conventional
>    dotted path for read-write.

**The depth bound** is the owner's decision of 2026-11-20, recorded in
issue #80: crossing 32 mount points is allowed, crossing the 33rd is
refused, and a mount point repeated at any depth is refused. This
specification states where the count starts (5.6.1), so that the bound is
exact, and the compatibility cost (5.6.1, point 6).

Section 9 shows where each is kept and which test fails if it is broken.

## 2. Scope

1. **Covered:**
   - the mount-point target: the string a fileserver holds for a mount
     point, the grammar IronAFS accepts, its caps and every form it
     refuses (5.1 to 5.4);
   - crossing a mount point: the depth and cycle rule, the anchor chain
     each crossing records, and what `.` and `..` resolve to (5.6.1 and
     5.6.4);
   - volume selection: which version (read/write, read-only, backup), in
     which cell, a crossing reaches, in the requesting context (5.6.2);
   - the dynamic root's names: a cell, its dotted read/write name, and
     `@cell` (5.6.3);
   - `@sys` and `@cell` substitution: where it applies, the order of the
     system-name list, the default list per target in M2a's scope, the
     administrator's override, and every bound (5.6.5).
2. **Not covered, and where it is covered:**
   - the VL lookups and their replies, the site filter and the location
     cache: [vl-fs-read.md](vl-fs-read.md) (#65). This specification says
     which lookup a crossing makes and which version it takes;
   - the directory object, and the lookup of a name in it:
     [directory-format.md](directory-format.md) (#56). This specification
     only chooses the name that is looked up;
   - the node table, node ids, pins and the reverse index:
     `ironafs-cache` (#91), under ADR 0004, C11. This specification states
     what an anchor records and what a crossing checks;
   - whether a reply identifies a volume's incarnation: the volume
     incarnation specification (#78, C25);
   - the cell name rule, the configuration keys for the system-name list
     and the home cell, and their syntax: `ironafs-config` (#93). This
     specification states the caps and the refusals those keys must
     enforce;
   - finding a cell's servers and the negative cache of non-cells: C33,
     C12, point 3, and the DNS client specification (#60);
   - how each operating system presents a mount point, a synthesised
     attribute and a lookup result, and its kernel caching: the contract
     (#67) and the FUSE codec (#92). 5.6.5, point 9 states the one
     requirement this specification places on them;
   - symbolic links: a link appears as a link on Linux and macOS (L32,
     point 2), and the kernel follows it with its own hop limit. Links the
     core resolves itself exist only on SMB (L32, point 2), outside M2a;
     5.6.1, point 7 fixes their share of the visited set now, and the SMB
     specification states the rest;
   - making, listing and removing mount points (`afs` commands, L35,
     point 5): M3;
   - direct access to a volume by name or id without a mount point,
     which a reference client offers through a special directory in its
     dynamic root `[M4 "-dynroot": UNVERIFIED]` `[G1 "To access volumes
     directly by volume ID": UNVERIFIED]`: not provided `[OWN]`. It would
     be a second route into every volume of every cell, bypassing the
     depth and cycle rule's starting point; an administrator's use of it
     is served later by the `afs` command line;
   - cell aliases: not provided in M2a `[OWN]`; a later configuration
     specification may add them as dynamic-root names;
   - the replica's explicit link resolution (L27): the replica ADR.

## 3. The design in short

1. A mount point is an object in a directory that names a volume. The
   file service marks it, and holds a short target string: a type mark
   (`#` regular, `%` read/write), an optional cell name and `:`, and a
   volume name with an optional `.readonly` or `.backup` suffix
   `[M2 "OUTPUT": UNVERIFIED]` `[G1 "To display a mount point":
   UNVERIFIED]`. How the object is marked and the exact bytes stored are
   MISSING (Q1).
2. The target is decoded by a strict grammar with a byte cap. Anything
   the grammar does not name exactly is refused, and a refused mount
   point is never shown as a symbolic link, a file or an empty directory:
   it fails explicitly.
3. Crossing a mount point resolves its target in the requesting context:
   a VL lookup by the base name in the target's cell, then a version
   chosen by the traversal rules: the suffix decides first; then a
   read/write mount point takes the read/write version; a regular mount
   point takes the read-only version if the volume is replicated and the
   mount point lies in a read-only volume or names a cell, and the
   read/write version otherwise `[G1 "The Rules of Mount Point
   Traversal": UNVERIFIED]` `[M1 "DESCRIPTION": UNVERIFIED]`.
4. Every volume root reached records its anchor chain: the identities of
   the mount points crossed since the cell root, at most 32. Crossing a
   33rd mount point, or a mount point already in the chain, is refused.
5. `/afs` is IronAFS's own dynamic root. For each cell it lists the cell
   name, entering the cell's root volume on the read-only path, and the
   cell name with a leading `.`, entering it on the read/write path.
   These entries are not mount points and are not counted.
6. A path component that is exactly `@sys` is looked up as each name of
   the ordered system-name list in turn, first match wins; `@cell` is
   looked up as the home cell's name. Substitution happens once per
   component, never on a substituted name, never inside a mount-point
   target, and only in the core's own lookups.
7. Every kept state is bounded: the target by `ns.mount.target_max`, the
   chain by `ns.mount.depth_max`, the list by `ns.sysname.count_max` and
   `ns.sysname.len_max`, and every resolution result by the cache budgets
   it already falls under (C18, C27).

## 4. Terms

- **Mount point:** an object stored in a volume's directory that names a
  volume to enter in its place `[G3 "About Mounting Volumes":
  UNVERIFIED]`.
- **Target, target string:** the bytes a mount point holds, as fetched
  from the file service (5.1).
- **Type mark:** the first octet of the target: `#` for a **regular**
  mount point, `%` for a **read/write** mount point `[M2 "OUTPUT":
  UNVERIFIED]`.
- **Cellular target:** a target that names a cell before a `:` `[M2
  "OUTPUT": UNVERIFIED]`.
- **Base name:** the volume name with any version suffix removed.
  **Version suffix:** `.readonly` or `.backup` `[G3 "About Volume Names":
  UNVERIFIED]`.
- **Version:** one of read/write (RW), read-only (RO) or backup (BK) `[G3
  "Volume Information in the VLDB": UNVERIFIED]` `[ADR-008]`.
- **Replicated:** a volume whose VL entry names a read-only version, that
  is, whose read-only id is not 0 `[OWN]`, from G1's rule 2. Its sites
  play no part: whether a usable site exists decides only whether the
  read-only version can be reached (5.6.2, point 5).
- **Mount-point node, root node, entry node:** the node a lookup of a
  mount point returns; the node of the root directory a crossing entered
  in one context; the node of a dynamic-root name (5.6.2, point 6; 5.6.3,
  point 5).
- **Crossing:** resolving one mount point and entering the volume it
  names, in one context.
- **Cell root:** the root directory of a cell's root volume, entered
  from a dynamic-root entry (5.6.3). Not a crossing.
- **Anchor chain:** for a volume root reached by crossings, the ordered
  list of the mount points crossed since its cell root, each by its
  **mount-point identity**: the cell, and the volume id, vnode and
  uniquifier of the mount-point object itself (not of its target).
- **Depth:** the length of a volume root's anchor chain. A cell root has
  depth 0; the root entered by crossing a mount point in a volume at
  depth d has depth d + 1.
- **Containing version:** the version of the volume that holds the mount
  point being crossed, as this context resolved it: the version chosen by
  this context's own binding into that volume (5.6.2, point 9), never a
  value stored on a node.
- **Containing root:** for a node inside an entered volume, the root node
  of that volume in the context that found it (5.6.1, point 4).
- **System-name list:** the ordered list of names that `@sys` stands for
  (5.6.5). **Home cell:** the cell `@cell` stands for, from the
  configuration (#93).
- **Requesting context:** the credential context and generation of the
  request that causes the crossing (C18, C40).

## 5. Specification

### 5.1 Layout

1. **How a mount point is marked.** A reference client tells a mount
   point from a directory or a symbolic link before it reads the target
   `[G3 "About Mounting Volumes": UNVERIFIED]`, which describes a mount
   point as structured like a symbolic link and read for the volume's
   name. What marks it in the status record (a file type value of its
   own, the symbolic-link type with a distinguishing mode, or something
   else) is MISSING (Q1). [vl-fs-read.md](vl-fs-read.md), 5.1.3, point
   2, has `file_type` as a closed enumeration of three kinds; if the
   captures show a fourth kind or a mode rule, that table and this point
   are amended together, at class S, before any code.
2. **How the target is fetched.** As the data of the mount-point object,
   by the same data fetch as a symbolic link's target
   ([vl-fs-read.md](vl-fs-read.md), 5.1.4, point 6), read whole in one
   fetch `[OWN]`, under the caps of 5.2. That the file service returns
   the target this way is UNVERIFIED (Q1).
3. **The target string** is, in order `[M2 "OUTPUT": UNVERIFIED]`
   `[G1 "To display a mount point": UNVERIFIED]`:

   | Part | Octets | Present |
   |---|---|---|
   | type mark | one octet: `#` (0x23) or `%` (0x25) | always |
   | cell name | the cell's name, then `:` (0x3A) | only in a cellular target |
   | volume name | a base name, then optionally `.readonly` or `.backup` | always |
   | terminator | MISSING (Q1) | MISSING (Q1) |

   These are the forms a reference command prints for a mount point;
   that the stored bytes are the same octets, with no other octet before,
   between or after them, is UNVERIFIED, and whether the stored form ends
   with an octet the command does not print is MISSING (Q1). The
   grammar of 5.4, points 4 to 8 is written for the printed form; the capture
   that settles Q1 amends it by naming the terminator, if any, exactly.
4. **A volume named by number.** The reference command that makes a
   mount point accepts a volume id number in place of a name `[M1 "-vol":
   UNVERIFIED]`. Whether the stored target then holds the number, and how
   a client tells a number from a name made only of digits, is MISSING
   (Q1). Until it is settled, a target whose base name is all decimal
   digits is refused (5.4, point 7).
5. **A shortened cell name.** The same command accepts a shortened cell
   name that the local client can disambiguate `[M1 "-cell":
   UNVERIFIED]`. Whether the stored target holds the full name or the
   shortened one is MISSING (Q1). IronAFS never expands a shortened name:
   a cell part that is not a cell the cell table knows by its full name
   is handled as an unknown cell (5.6.2, point 7) `[OWN]`.
6. **A volume's root directory.** The file id of the root directory of a
   volume entered by a crossing (its vnode and uniquifier) is MISSING
   (Q2). No document registered here states it.

### 5.2 Caps

Each value is IronAFS's own `[OWN]`, with its reason. "Declared bound" is
MISSING for the target: no admissible document states one.

| Cap name | Applies to | Declared bound | Cap | Why |
|---|---|---|---|---|
| `ns.mount.target_max` | the target string, in bytes; checked on the status record the fetch is planned from, and again on the fetch reply's own status record ([vl-fs-read.md](vl-fs-read.md), 5.4, point 9) | MISSING | 512 bytes | the longest target the grammar accepts is 1 + 253 + 1 + 64 + 9 = 328 bytes (a type mark, a cell name of `dns.name_max` less its root and length octets, `:`, a base name of `vl.name_max`, `.readonly`), plus a terminator of at most a few octets once Q1 names it; 512 holds that with room, and is far below `fs.symlink.target_max` |
| `ns.mount.depth_max` | the depth of a volume root (section 4) | — | 32 | the owner's decision of 2026-11-20 (issue #80): crossing 32 is allowed, crossing 33 is refused |
| `ns.mount.chain_bytes_max` | one anchor chain as stored | — | `ns.mount.depth_max` × the encoded size of one mount-point identity | follows from the depth cap; the identity is a cell table index and three 32-bit values, so a chain is at most 32 × 16 = 512 bytes |
| `ns.link.hops_max` | links the core itself resolves in one request (SMB only, L32, point 2) | — | 32 | one number for both kinds of hop in the visited set (5.6.1, point 7); the SMB specification may lower it, never raise it |
| `ns.sysname.count_max` | names in the system-name list | — | 16 | each name is one in-memory lookup in one decoded directory (5.6.5, point 4); 16 bounds the work of one `@sys` lookup and holds every list a site plausibly configures |
| `ns.sysname.len_max` | one name in the list, in bytes | — | 64 | the names the registry records are under 20 octets `[Y1: UNVERIFIED]`; 64 leaves room for site names and bounds the list at 16 × 64 = 1 024 bytes |
| `ns.cell_discovery.per_crossing_max` | cell discoveries one crossing may start | — | 1 | a target names one cell; a crossing never fans out (5.6.2, point 7) |

1. **Nothing is sized from input** `[OWN]`. The target's buffer is
   reserved once, for the status record's length after it has passed
   `ns.mount.target_max`; the chain and the list are fixed-capacity.
2. **Configuration may lower** `ns.sysname.count_max` per site, never
   raise it (xdr.md, 5.2, point 8's rule, applied here) `[OWN]`.

### 5.3 Versions

Not applicable: the target is a peer's format IronAFS does not control
([README](README.md), "Formats IronAFS does not control"). This
specification defines no stored format: the anchor chain lives in the
node table, which does not survive a daemon restart (C11, point 2).

### 5.4 Decoding

The target is decoded by one pure function in `ironafs-proto`, since it
is fileserver data (C4, point 1's row for that crate). Its inputs are
the target's octets and nothing else; its output is a **decoded target**
(type mark, optional cell part as octets, base name, version suffix) or
one refusal. The checks run in this order; the first failure ends the
decode and nothing from the target is used `[OWN]`. Point 1 is checked
by the caller before the decoder runs, on the status record; points 2
to 8 are the decoder's; point 9 is the caller's again.

1. **The marker.** Only an object the status record marks as a mount
   point (5.1, point 1; MISSING, Q1) is decoded as a target. A symbolic
   link whose text happens to look like a target is a symbolic link, and
   is never crossed `[OWN]`: the classification is the server's, never
   inferred from content, so a user who may create links cannot create
   mount points that way.
2. **Length.** 1 to `ns.mount.target_max` bytes, on the planning record
   and on the reply (5.2). Otherwise `MountTargetInvalid` (too long), with
   no fetch sent when the planning record already shows it. A target of
   0 bytes is `MountTargetInvalid` (empty).
3. **The terminator.** MISSING (Q1). Once Q1 settles it, this point
   names the exact terminator octet sequence, and removes it from the
   end of the target before any other check reads an octet. A target
   that does not end with it is `MountTargetInvalid` (terminator); a
   target that holds nothing but the terminator is `MountTargetInvalid`
   (empty). If Q1 shows that no terminator is stored, this point checks
   nothing. Until Q1 is settled no target is accepted, and this decoder
   does not merge (README, "Labelling claims", point 4).
4. **No zero octet and no control octet in what remains.** An octet below
   0x21, or 0x7F, or 0x80 and above, anywhere after point 3, is
   `MountTargetInvalid` (control or non-ASCII octet) `[OWN]`: no form in
   5.1, point 3 holds one, and a target is never text to display
   unescaped (section 8). A terminator octet that appears anywhere but
   at the end is caught here.
5. **Type mark.** The first octet is `#` (regular) or `%` (read/write).
   Any other octet is `MountTargetInvalid` (unknown type) `[M2 "OUTPUT":
   UNVERIFIED]`.
6. **Cell part and volume part.** The rest is split at `:`:
   - no `:`: the whole rest is the volume part, and the target is not
     cellular;
   - exactly one `:`: the octets before it are the cell part, the octets
     after it the volume part; an empty cell part or an empty volume part
     is `MountTargetInvalid`;
   - more than one `:`: `MountTargetInvalid` `[OWN]`, since the printed
     form has at most one (5.1, point 3) and a volume name never holds
     `:` (point 7).
7. **Volume part.** `[OWN]`, with each reason:
   - if it ends with `.readonly` or `.backup`, that suffix is removed
     once and recorded; the rest is the base name;
   - the base name is 1 to `vl.name_max` bytes
     ([vl-fs-read.md](vl-fs-read.md), 5.2), else `MountTargetInvalid`.
     A read/write volume's name is documented as at most 22 characters
     `[G3 "About Volume Names": UNVERIFIED]`; IronAFS's own cap is the
     larger one already used for VL lookups, so a cell whose server
     accepts longer names is not refused here;
   - the base name holds only octets 0x21 to 0x7E other than `/`, `:`,
     `#`, `%` and `@`, else `MountTargetInvalid`. `/` would be a path, `:`
     a second separator, `#` and `%` a second type mark, and `@` the
     start of a substitution name that is never expanded here (5.6.5,
     point 6). Which octets servers accept in volume names is Q8;
   - a base name that itself ends with `.readonly` or `.backup` (a target
     such as `#v.readonly.readonly`) is `MountTargetInvalid`: the
     suffixes are added by the server to a base name that never carries
     one `[G3 "About Volume Names": UNVERIFIED]`. The cost, if a
     supported server does accept such a base name: a target `#v.readonly`
     always means the read-only version of `v`, so a volume whose own
     base name is `v.readonly` cannot be reached through a mount point.
     Q8 checks whether a server accepts such a name;
   - a base name made only of decimal digits is `MountTargetInvalid`
     (numeric) until Q1 settles 5.1, point 4.
8. **Cell part, syntax.** It is 1 to 253 bytes and holds only octets
   0x21 to 0x7E other than `/`, `#`, `%` and `@` (and `:`, by point 6),
   else `MountTargetInvalid` (cell name) `[OWN]`, for the reasons of
   point 7. The decoder returns it as octets: it does not lower-case,
   trim or expand it.
9. **Cell part, name rule** (the caller, after the decoder). The caller
   passes the cell part to the one constructor of a cell name in
   `ironafs-config` (#93); whatever that constructor refuses is
   `MountTargetInvalid` (cell name) `[OWN]`: one rule for every cell name
   the daemon takes in, applied where the cell table is, so the pure
   decoder does not depend on the configuration crate. Point 8 holds
   whatever that constructor allows.
10. **The result** is the decoded target. What may keep it, and for how
    long, is 5.6.2, point 9.

### 5.5 Encoding

Not applicable in M2a: the client does not create mount points. Making a
mount point is an `afs` command in M3 (L35, point 5); its specification
states the encoder, which produces only targets this decoder accepts.
Tests build targets with a test-only builder written from 5.1 and 5.4.

### 5.6 States and ordering

#### 5.6.1 Crossing, depth and cycles

1. **What a crossing checks first** `[ADR 0004 C11.8: Working]`. Before
   any target is fetched or any VL lookup is sent for mount point m, in
   a directory of a volume whose root has anchor chain c of length d:
   - first, if d = `ns.mount.depth_max` (32), the crossing is refused
     with `MountDepthExceeded`;
   - then, if m's mount-point identity equals any identity in c, the
     crossing is refused with `MountCycle`.

   Otherwise the crossing goes on, and the root it enters gets anchor
   chain c followed by m, of length d + 1.
2. **Where the count starts** `[OWN]`, from C11, point 1, and C12,
   point 1. A cell root has depth 0 and an empty chain: entering it from
   the dynamic root (5.6.3) is not a crossing, because the dynamic root's
   entries are IronAFS's own and no fileserver stored them. So, counting
   from the cell root:
   - the root reached through 32 nested mount points has depth 32, and
     is entered;
   - a 33rd mount point found anywhere in that volume is refused, before
     its target is read.

   The dynamic root has no depth and no chain; it is not a volume.
3. **Identity, not target** `[OWN]`, from C11, point 8 ("repeats a mount
   point"). The repeat test compares mount-point identities (section 4),
   never node ids and never target strings or the volumes they resolve
   to:
   - one mount point reached by two paths has two node ids (C11, point
     1) but one identity, so the repeat is caught whatever path led to
     it;
   - the same mount point in a volume's read/write and read-only versions
     has two identities (the volume ids differ, C11, point 5), so a path
     that alternates versions is still bounded by point 1's depth test;
   - two different mount points that name the same volume are not a
     repeat; a cycle through them repeats one of them at the latest one
     loop later, and the depth test bounds it in any case.
4. **What the chain holds** `[OWN]`. The anchor chain is stored with the
   root node (5.6.2, point 6), inline, as at most `ns.mount.depth_max`
   identities (`ns.mount.chain_bytes_max`), so the checks of point 1
   read one record and never walk the table. The chain is fixed when the
   root is entered and never changes. Every node inside an entered
   volume, mount-point nodes included, records its **containing root**:
   the root node of the volume it was found in, in the context that
   found it. A crossing reads the chain from the containing root of the
   mount-point node it crosses, so the checks of point 1 read one record;
   point 8 keeps that record in the table.
   In a mount-point identity the cell is the cell table's index for that
   cell, which is never reused for another cell within one daemon
   instance, so a reload of the configuration cannot make two cells
   compare equal `[OWN]`.
5. **Refused means refused** `[ADR 0004 C11.8: Working]`. A refused
   crossing returns its error to the request that caused it. It caches
   no negative lookup, marks nothing about the mount point or the volume,
   and the same lookup is checked again, from the same rule, next time
   `[OWN]`. Lookup and get-attributes on the mount point itself, without
   crossing, still answer under C12, point 2 (5.6.4, point 3).
6. **The compatibility cost** `[OWN]`, stated as the owner's decision
   asks:
   - a cell whose tree nests more than 32 mount points below a cell root
     has paths that a reference client may follow and IronAFS refuses:
     those paths fail with the loop error (section 7). Whether a reference
     client has its own bound, and where it counts from, is Q7; the
     answer changes only this statement, not the rule;
   - a volume mounted twice along one path, which the reference
     documentation already advises against `[G3 "About Mounting Volumes":
     UNVERIFIED]` `[M1 "DESCRIPTION": UNVERIFIED]`, works so long as no
     single mount point repeats; a mount point that leads back into its
     own volume works for one crossing and is refused at the second.
7. **The visited set** `[ADR 0004 L32.3: Working]`. On a surface where
   the core resolves links itself (SMB, L32, point 2), one request keeps
   one visited set for its whole resolution: the identity of every mount
   point crossed during the request, and the file id of every link the
   core resolved. The request is refused with the loop error when it
   would cross a mount point already in the set, resolve a link already
   in the set, cross more than `ns.mount.depth_max` mount points in all,
   or take more than `ns.link.hops_max` link hops in all. A link whose
   text climbs with `..` to a lower root does not empty the set, so
   links cannot re-cross mount points to multiply work. Point 1's test
   on the root's own chain applies to each crossing as well. On Linux
   and macOS the kernel follows links, with its own hop limit (L32,
   point 3), and this point does not apply.
8. **Ancestors stay in the node table** `[OWN]`, from C11, points 3 and
   4. Every node's record names its parent node (5.6.4, point 2) and,
   inside an entered volume, its containing root (point 4); a root node
   names its mount-point node or dynamic-root entry node as its anchor
   (5.6.2, point 6). A node is not removed from the node table while
   another node in the table names it as its parent, its anchor or its
   containing root, or while a binding of 5.6.2, point 9 names it. So a
   root node stays while any node of its volume, or its mount-point
   node's binding, stays, and its anchor chain with it. Only a node's
   identity record stays this way; its status and data may still be
   evicted (C11, point 4). So `..` and the anchor chain are always
   answered from the table, never from a server's `..` entry. If a root
   node is removed because nothing names it any longer, the next crossing
   makes it again, with the chain read from the containing root of the
   mount-point node crossed, which is still in the table because that
   node names it; a cell root is made again at the next use of its
   dynamic-root entry node, with an empty chain. For one live node whose
   path below `/afs` has n components (the live node's own included),
   the records kept this way are the dynamic root, one node per
   component, and one root node per volume entered on the path (at most
   n − 1 of them), so at most 2n; each is charged to the budget of the
   context that pinned the live node (C11, point 10).

#### 5.6.2 Volume selection

The rules are the reference documentation's three traversal rules and its
three kinds of mount point `[G1 "The Rules of Mount Point Traversal",
"The Three Types of Mount Points": UNVERIFIED]` `[M1 "DESCRIPTION":
UNVERIFIED]`, written as one table. Each row's fact is UNVERIFIED until
the lab shows a reference client following it (Q3); the rule IronAFS
follows is the table, `[OWN]` where a row says so.

1. **Inputs:** the decoded target (type mark, cell part or none, base
   name, suffix or none); the containing version; and, from the target
   volume's VL entry, whether it is replicated (section 4).
2. **The version chosen**, first matching row:

   | Row | Suffix | Type mark | Cellular | Containing version | Version chosen | Source |
   |---|---|---|---|---|---|---|
   | S1 | `.readonly` | any | any | any | RO | G1 rule 1; M1 |
   | S2 | `.backup` | any | any | any | BK | G1 rule 1; M1 |
   | S3 | none | `%` | any | any | RW | G1, read/write mount points; M1 |
   | S4 | none | `#` | yes | any | RO if replicated, else RW | G1, cellular mount points; M1 |
   | S5 | none | `#` | no | RO | RO if replicated, else RW | G1 rule 2 |
   | S6 | none | `#` | no | RW | RW | G1 rule 3 |
   | S7 | none | `#` | no | BK | RO if replicated, else RW | `[OWN]`: no registered source states the default for a mount point inside a backup volume (Q3) |

   Row S4 applies to every target that names a cell, the cell holding the
   mount point included `[OWN]`: the documents describe the cellular rule
   by the target's form, and a reference client counts a mount point
   naming its own cell as cellular for at least one other purpose `[M4
   "-fakestat": UNVERIFIED]`; Q3 checks it for selection. Row S7 is
   IronAFS's choice: a backup volume is a read-only clone, so it stays on
   the read-only path; a reference client's option that prefers backup
   volumes below a backup volume `[M4 "-backuptree": UNVERIFIED]` is not
   offered.
3. **The cell** is the target's cell part if it has one, else the cell
   of the volume that holds the mount point `[M1 "-cell": UNVERIFIED]`.
4. **The lookup.** One `vl.get_entry_by_name` for the base name, in that
   cell, in the requesting context ([vl-fs-read.md](vl-fs-read.md), 5.1.4,
   point 1), with that specification's checks: the entry names the base
   name asked (its 5.4, point 12), and its sites are filtered (its 5.4,
   point 11). The chosen version's id is taken from the entry; an id of 0
   means the version does not exist (its 5.1.3, point 5).
   **On every row**, S1 to S7, and for both dynamic-root names (5.6.3), a
   chosen version whose id is 0 fails the crossing with `VersionAbsent`,
   before any site is chosen and before any node is made: no volume id 0
   ever reaches a node key or a file-service request `[OWN]`.
5. **No silent change of version** `[OWN]`. Once a row has chosen RO,
   RW or BK, the crossing reaches that version or fails with that
   version's error. "Replicated" depends only on the entry's read-only
   id (section 4), never on its sites, so a volume whose read-only sites
   are all unusable is still replicated: if the site filter leaves no
   usable read-only site, the crossing fails with `offline`
   ([vl-fs-read.md](vl-fs-read.md), 5.4, point 11); if the sites that
   remain fail when called, it fails with the error vl-fs-read.md, 5.9,
   gives the last call (`offline`, `busy` or another). In neither case is
   it read from its read/write version instead. That would serve other data under the
   same path, and the reference documentation states the read/write
   fallback only for a volume that is not replicated `[G1 rule 2:
   UNVERIFIED]`. A VL entry, forged or broken, can therefore make a
   read-only path fail, never make it serve the read/write volume.
6. **The two nodes of a mount point** `[ADR 0004 C11.1: Working]`
   `[ADR 0004 C11.9: Working]`, and `[OWN]` for the split between them:
   - **The mount-point node** is the node a lookup of the mount point's
     name returns, in every context, before and after any crossing. Its
     key is the cell, the mount-point object's own volume id, vnode and
     uniquifier, and the anchor of the directory that holds it. Its node
     id is the anchor of everything inside the volume entered through it
     (C11, point 1), and it is the node a layer is given for that path;
     a layer presents it as a directory (L32, point 1).
   - **The root node** is the root directory of the volume a crossing
     entered, in one context. Its key is the cell, the chosen version's
     volume id, the root's vnode and uniquifier (5.1, point 6, MISSING),
     and the mount-point node's id as its anchor; it holds the anchor
     chain (5.6.1, point 4). The volume in the key is the one this
     context resolved, so two contexts that resolve one mount point to
     different volumes get different root nodes, and everything below
     them gets different nodes too. The root node records no version:
     the version (RO, RW or BK) a crossing chose belongs to that
     context's binding (point 9), because one root node, keyed by a
     volume id, can be reached by contexts whose VL entries chose
     different versions for that id.
   - **A layer is never given a root node's id.** Every operation a
     layer addresses to the mount-point node that needs the volume (read
     the directory, look up a name in it, get its attributes once
     crossed) is served from the root node this context resolved. A name
     found in the root directory has the mount-point node as its parent
     (5.6.4, point 2). So a path's node id never changes when it is
     crossed, and the same node id leads each context to its own volume,
     which the layer serves per context as C18, point 4 requires of any
     node two contexts share.
7. **An unknown cell.** A cellular target whose cell the cell table does
   not hold is looked up as the dynamic root looks up a name (C12, point
   3; C33), at most `ns.cell_discovery.per_crossing_max` times per
   crossing, through the negative cache of non-cells, and only where
   discovery is enabled (C33, point 8). A name the negative cache holds,
   or a discovery that fails, fails the crossing with `not found`. A
   failed discovery caches nothing here; C12, point 3, says what the
   negative cache keeps `[OWN]`.
8. **Grants on the way** `[ADR 0004 C22.6: Firm]`. Every step through a
   mount point, cached or not, needs the reader's own current grant on
   the directory that holds it (lookup, [vl-fs-read.md](vl-fs-read.md),
   5.7); for a mount point in a volume's root directory, that directory
   is the root node this context entered, not the mount-point or entry
   node the layer holds for it. The target is that object's data: it is read, and a kept
   decoded target is used, only under the reader's current grant for
   that object, minted from its own status reply at that data version,
   the grant a link's target needs under vl-fs-read.md, 5.7 (Q1 settles
   which rights apply). So a revoked right, an ACL change or a callback
   break on the mount point stops the next step through it. The root
   entered is then read under its own status reply, as any directory.
9. **What is kept, and for how long** `[ADR 0004 C18.1: Firm]` `[ADR 0004
   C19.1: Firm]` `[ADR 0004 C20.1: Working]`. Three things, each a
   private entry of the requesting context and generation, charged to its
   budgets (C27), each with its own key and lifetime `[OWN]`:
   - **the decoded target**, keyed by the mount-point identity and the
     object's data version: it is cached data of that object, and lives
     only while the context's grant on it is current (C22, C23), so a
     callback break or a new data version ends it;
   - **the VL entry**, keyed by the cell and base name: the location
     cache, with its lifetime (C33, point 7);
   - **the binding** from the mount-point node, or a dynamic-root entry
     node (5.6.3, point 5), to the root node in this context, with the
     version it chose. For a mount-point node it is computed from the
     decoded target, the containing version (this context's own binding
     into the volume that holds the mount point) and the VL entry, by
     5.6.2's table; for an entry node, from its path kind (5.6.3, point
     1) and the VL entry only, since an entry node has no containing
     version. It is valid only while the entries it was computed from
     are current. The version chosen is held only in this binding, per
     context, never on a node.

   **The containing version is always this context's own** `[ADR 0004
   C18.1: Firm]` `[ADR 0004 C19.1: Firm]` `[ADR 0004 C20.1: Working]`
   `[OWN]`. A crossing of a mount point in a volume uses the version
   chosen by the requesting context's own binding into that volume, and
   never a version another context, or another request of a context
   without a credential, chose:
   - a context without a credential holds a binding only for the request
     that made it (C20, point 1); a later request that reaches a
     directory of that volume through a node still in the table, and
     crosses a mount point there, first resolves its way into that volume
     again in this request, from the nearest ancestor it holds a binding
     for, or from the dynamic root;
   - a request under a credential uses only a binding obtained with
     authentication in its own context (C19, point 1). If it has none for
     the containing volume, for example because it reached the directory
     through a node another context resolved, or its binding expired, it
     first resolves that volume's own entry into it (its mount-point or
     entry node) with authentication in its own context, in the same
     way, up the anchor chain as far as needed; point 10 applies to each
     of those lookups.

   **Each binding must lead to the root it is used for** `[OWN]`, from
   C18, point 1, and C19, point 1. Let the containing root be R_k, with
   the chain of entries that led to it: the dynamic-root entry node e_0
   into R_0, then mount points m_1 into R_1, …, m_k into R_k. A cached
   binding of this context is used for R_i only if the root node it
   leads to is R_i itself. When a binding is missing or expired and is
   resolved again, step by step from the nearest ancestor this context
   holds a usable binding for (or from e_0), each step must resolve, in
   the requesting context, to exactly R_i: same cell, same volume id,
   same root vnode and uniquifier, same anchor. If any step resolves to
   another root node R'_i ≠ R_i, the walk stops there and the request
   fails closed with `AnchorMismatch` (section 7), without crossing the
   mount point that was asked for and without using any binding below
   step i. The containing version used for the crossing is the one chosen
   by this context's binding into R_k, after that check.

   **Order and bounds.** 5.6.1, point 1's depth and repeat tests on the
   mount point being crossed run first, on R_k's stored chain, before any
   ancestor is resolved again, so a crossing that D2 refuses sends no
   call. A crossing that passes has d = depth of R_k at most
   `ns.mount.depth_max` − 1, so re-resolution walks at most the entry
   binding of e_0 and the d mount points of R_k's chain, and with the
   crossing asked for at most `ns.mount.depth_max` crossings in all,
   each counted in the request's visited set where 5.6.1, point 7
   applies.

   An entry is kept only when every reply it rests on (the target fetch,
   the status reply that granted it, the VL lookup) was obtained with
   authentication in that context; a request made under a credential
   uses only such entries and such replies (C19, point 1). A context
   without a credential keeps none of them: each of its requests fetches
   the target and asks the VL service again (C20, point 1). Nothing is
   shared between contexts.
10. **An authenticated crossing without authenticated location data**
    `[ADR 0004 C19.1: Firm]`, and `[OWN]` for the outcome. A crossing for
    a request made under a credential needs a VL entry obtained with
    authentication in that context, for the dynamic root's entries too.
    If the VL lookup can be made only without authentication (every VL
    lookup of [vl-fs-read.md](vl-fs-read.md), 5.1.4, is a null-class call
    as that specification stands), the crossing fails closed with
    `LocationUnauthenticated`: no VL entry from a null-class reply is
    used for it, nothing is kept, and no fallback to an unauthenticated
    lookup is made. The plain consequence: on a real cell, a request
    under a credential cannot enter any cell root or cross any mount
    point until an authenticated VL lookup exists (with M2b's rxkad, if
    the VL service answers it) or ADR 0004 is changed (section 13, Q10).
    M2a has no credentialed requests on a real cell, so M2a's acceptance
    on a real cell (anonymous reads) is not affected; the rule is
    exercised in the simulator's authenticated test profile (O50, point
    1), whose VL model answers authenticated lookups.

#### 5.6.3 The dynamic root

1. **Names** `[ADR 0004 C12.1: Working]` `[ADR 0004 L34.4: Working]`.
   For each cell the dynamic root lists, it lists two names:
   - the cell's name: entering the cell's root volume on the read-only
     path, as a regular cellular target would (row S4);
   - the cell's name with one `.` before it: entering the same volume on
     the read/write path, as a read/write target would (row S3).

   Both names are listed, so a lookup finds what a listing shows `[OWN]`
   (the rule of [directory-format.md](directory-format.md), 5.6, point 1,
   kept at the root). A cell name never starts with `.` (#93's rule), so
   the two names never collide.
2. **The cell's root volume** is the volume a cell's administrators
   conventionally mount at its name, `root.cell` `[G1 "The Three Types of
   Mount Points": UNVERIFIED]` `[M1 "EXAMPLES": UNVERIFIED]`. IronAFS
   enters `root.cell` for every cell `[OWN]`; a per-cell override is not
   offered in M2a (section 11).
3. **`@cell` at the root** is looked up as the home cell's name (5.6.5,
   point 7); `.@cell` is not a substitution and is an ordinary name.
4. **No contact before need** `[ADR 0004 L32.4: Working]`. Lookup, get
   attributes and access on a dynamic-root entry are answered without
   contacting the cell; the first VL lookup is made at a read of the
   cell root or a lookup inside it. Get-attributes on an entry node
   always answers with synthesised directory attributes, also once the
   cell root has been entered in this context `[OWN]`: L32, point 4 makes
   no exception, and it keeps `/afs` listings cheap whatever has been
   entered.
5. **Not a crossing.** Each dynamic-root name has its own **entry
   node**, IronAFS's own, keyed by the cell and the path kind (read-only
   or read/write), the same in every context; it plays the mount-point
   node's part (5.6.2, point 6): a layer is given it, it is the anchor of
   the cell root, and its binding to the cell root in each context is a
   binding of 5.6.2, point 9. Entering the cell root records an empty
   anchor chain (5.6.1, point 2). The two names have two entry nodes, so
   the two paths give different nodes even on one volume `[ADR 0004
   C11.1: Working]` `[OWN]`.

#### 5.6.4 `.`, `..` and an unresolved mount point

1. **`.`** is the node itself `[OWN]`.
2. **`..`** is never taken from a directory object's `..` entry, which
   is not verified ([directory-format.md](directory-format.md), 5.6,
   point 3) `[OWN]`:
   - for any node found by lookup in a directory, mount-point nodes
     included, it is the node a layer holds for that directory: for a
     volume's root directory, the mount-point node or dynamic-root entry
     node through which that volume was entered (5.6.2, point 6), never
     the root node; for any other directory, that directory's node, which
     the node table records (single parent, C11, point 3);
   - for a dynamic-root entry node, and for the dynamic root, it is the
     dynamic root.

   So no `..` answer is ever a root node.

   The parent recorded is kept in the table as long as the node is
   (5.6.1, point 8). A lookup of `..` crosses no mount point and changes
   no depth.
3. **An unresolved mount point** `[ADR 0004 C12.2: Working]`. A lookup
   that names a mount point returns its mount-point node (5.6.2, point 6)
   and does not cross it. Get-attributes on that node, while this context
   has no current binding for it (5.6.2, point 9), answers with
   synthesised attributes of a directory, without fetching the target,
   without a VL lookup, and without minting any grant; once crossed, it
   answers with the root node's attributes in this context. The values
   shown, and how a layer marks them, are the contract's (#67). Crossing
   happens at the first read of the mount point's directory or lookup of
   a name inside it, as at the dynamic root.
4. **A mount point that fails to resolve** fails each operation that
   needs its root, with the error of section 7. It is never presented as
   an empty directory, a symbolic link or a file (C46, point 4).

#### 5.6.5 `@sys` and `@cell`

1. **Where substitution applies** `[ADR 0004 L34.4: Working]`. In the
   core's lookup of one name in a directory under `/afs`, the dynamic
   root included, on every surface except the replica. Not in a listing:
   a listing shows the directory's entries as they are `[OWN]`. At the
   dynamic root, which has no directory object, `@sys` is matched only
   against the names the root already lists, in memory, with no cell
   discovery, no DNS query and no grant check (the root is IronAFS's
   own, L32, point 4) `[OWN]`; `@cell` is point 7.
2. **Which names** `[OWN]`. A component that is exactly the four octets
   `@sys`, or exactly the five octets `@cell`. A component that only
   contains or starts with them (`bin@sys`, `@sys.old`) is an ordinary
   name, looked up as it is. The reference documentation shows `@sys` as
   a whole component `[G2: UNVERIFIED]` `[M3 "DESCRIPTION": UNVERIFIED]`
   and does not say whether other forms are substituted; Q4 checks it.
3. **The order** `[M3 "-newsys": UNVERIFIED]`. The names of the
   system-name list are tried in list order; the first name that the
   directory holds is the result. If the directory holds none of them,
   the lookup fails with `not found`.
4. **The cost of one `@sys` lookup** `[OWN]`. Inside a volume, all
   tries are lookups in the one decoded directory object
   ([directory-format.md](directory-format.md), 5.6, point 1): one
   fetch, if any, and at most `ns.sysname.count_max` in-memory lookups.
   The grant check of C22, point 6 is made once, on the directory, before
   the first try.
5. **No literal fallback** `[OWN]`. A directory entry literally named
   `@sys` or `@cell` is shown in a listing, and is not reached by a
   lookup of that name: the substitution always runs. So a path's meaning
   does not change with what the list happens to match. `afs diagnose`
   reports such an entry. Whether a reference client does the same is Q4.
6. **Once, and only here** `[OWN]`:
   - a substituted name is looked up as it is; it is never substituted
     again, so no recursion is possible;
   - a mount-point target is never substituted (5.4, point 7 refuses
     `@` in a base name, and a cell part is a cell name);
   - in a link the core resolves itself (SMB), each component of the
     link's text is one lookup, so points 1 to 5 apply to it as to any
     lookup, within the visited set of 5.6.1, point 7.
7. **`@cell`** is looked up as the home cell's name, in the directory
   asked (at the dynamic root, the home cell's entry). With no home cell
   configured, a lookup of `@cell` fails with `not found`, and is never
   looked up literally (point 5) `[OWN]`. That `@cell` stands for the
   client's own cell is ADR 0004's decision (L34, point 4); no
   registered document states its meaning, and Q4 checks it against a
   reference client.
8. **The list** `[ADR 0004 L34.4: Working]`:
   - it is machine-wide, from the configuration (#93), and the
     administrator's list replaces the default entirely `[M3 "-newsys":
     UNVERIFIED]`;
   - it holds 0 to `ns.sysname.count_max` names, each 1 to
     `ns.sysname.len_max` bytes; each name obeys the directory format's
     name rules (no zero octet, no `/`, not empty,
     [directory-format.md](directory-format.md), 5.4, point 10), is not
     `.` or `..`, and does not start with `@`; a name appears at most once.
     The configuration refuses a list that breaks any of these, and the
     daemon keeps its previous list (#93's rule for a refused
     configuration);
   - an empty list makes every `@sys` lookup fail with `not found`;
   - **the default list**, used when the administrator sets none, is one
     ordered list per target, recorded in point 10. A default is never
     taken from the lab's own configured list, and never from any site's
     configuration `[OWN]`, as issue #80 requires: a site's list says
     what that site wanted, not what a client starts with.
9. **A list change takes effect at the next lookup** `[OWN]`. The core
   never caches the result of an `@sys` or `@cell` lookup under the name
   `@sys` or `@cell`; it may cache the substituted name's lookup as any
   other. The layers must not let the operating system cache an entry
   for those two names: the FUSE codec answers their lookups with no
   entry validity (#92). How each layer does so is its specification's.
10. **The default per target in M2a's scope.** M2a's targets are Linux
    on x86_64 and on aarch64 (ADR 0004, L37, point 1; O50, point 1).

    | Target | Default list | Evidence | Label |
    |---|---|---|---|
    | Linux x86_64 | MISSING | no clean reference client observed yet (Q5). The registry's system-type list names no Linux name for kernels after 2.4 on this architecture `[Y1: UNVERIFIED]`, so it gives no candidate | MISSING; owner's choice pending (Q5) |
    | Linux aarch64 | MISSING | as above; the registry names no Linux name for this architecture at all `[Y1: UNVERIFIED]` | MISSING; owner's choice pending (Q5) |

    The registry notes that the names it records serve two purposes,
    naming a build platform and serving as `@sys` values, and that sites
    choose their own `@sys` values `[Y1: UNVERIFIED]`: so the registry
    alone cannot give a default. Until Q5 is settled, the default list
    is empty (point 8's empty list), which fails `@sys` lookups closed
    and never guesses `[OWN]`; an administrator's list works at once.

### 5.7 Lookup results

A lookup's result in the core is one of: an entry's node; for a mount
point, its mount-point node, not yet crossed (5.6.2, point 6; 5.6.4,
point 3); for a dynamic-root name, its entry node (5.6.3, point 5); or a
failure of section 7. A lookup never crosses a mount point; crossing
happens when an operation needs the volume behind it. A mount point is
never reported to a layer as anything but a directory `[ADR 0004
L32.1: Working]`.

## 6. Arithmetic

| Id | Operation | Operands' range | Bound and result at overflow |
|---|---|---|---|
| A1 | the target's planned length against `ns.mount.target_max` (5.4, point 2) | the status record's `length`, a `u64` | compared in `u64`; never narrowed before the test; the narrowing to `usize` after it is `try_from` and cannot fail |
| A2 | depth d + 1 for the root entered (5.6.1, point 1) | d ≤ `ns.mount.depth_max` − 1 after the check | `checked_add` in `u8`; cannot overflow after the check |
| A3 | the suffix test, `len(volume part) − len(suffix)` (5.4, point 7) | the suffix length ≤ the volume part's length, tested first | computed only after that test, so it cannot underflow |
| A4 | the split at `:` (5.4, point 6) | indexes within the target | slicing by checked indexes from a search, never by arithmetic on input values |
| A5 | the list's total size, count × name length (5.2) | ≤ 16 × 64 | a constant; no arithmetic on input |

`[OWN]` for every row; each follows from the section it names.

## 7. Errors and failure

1. **Errors of this layer** `[OWN]`; tests assert the exact kind (coding
   standards rule 15). None carries the target's octets; a diagnostic
   shows the target only escaped (section 8):

   | Kind | When | Contract error (C46) |
   |---|---|---|
   | `MountTargetInvalid` | 5.4, points 2 to 8, with the reason (too long, empty, control octet, terminator, unknown type, separator, volume name, numeric, cell name) | `not supported` |
   | `MountDepthExceeded` | 5.6.1, point 1; and 5.6.1, point 7's total of crossings in one request | the loop error (Q6) |
   | `MountCycle` | 5.6.1, point 1; and 5.6.1, point 7's repeat in one request | the loop error (Q6) |
   | `LinkHopsExceeded` | 5.6.1, point 7 (SMB only) | the loop error (Q6) |
   | `VersionAbsent` | 5.6.2, point 4: on any row, or for a dynamic-root name, the version chosen has id 0 | `not found` |
   | the VL lookup's errors | [vl-fs-read.md](vl-fs-read.md), 5.9 and section 7 | as that specification maps them |
   | `CellUnknown` | 5.6.2, point 7 | `not found` |
   | `AnchorMismatch` | 5.6.2, point 9: a binding, cached or resolved again in this context, leads to a root node other than the one the path reached | `stale` |
   | `LocationUnauthenticated` | 5.6.2, point 10: a request under a credential, and no VL lookup with authentication available | `SecurityFloor` (C46, point 1: a permission error to applications, with the cause in the side channel) |
   | `SysnameNoMatch` | 5.6.5, point 3, or an empty list | `not found` |
   | `HomeCellUnset` | 5.6.5, point 7 | `not found` |

2. **Fail closed.** A refused or failed crossing serves nothing from the
   volume it would have entered and caches nothing about it (5.6.1, point
   5). The mount point itself stays a directory entry whose crossing
   fails; it is never shown as a link, a file or an empty directory
   (C46, point 4).
3. **No panic** is reachable from any target, chain or list (coding
   standards rule 1).

## 8. Security

Who can send or change what this specification reads: the fileserver
that holds a mount point; any client user who may create mount points or
links in a directory (the server's ACL decides); the cell's VL servers;
and, because M2a runs over the null class, anyone on the path (N5,
point 2; ADR 0004, A13 and O23). The system-name list and the home cell
come from the configuration, which only the administrator writes.

| Threat | What stops it |
|---|---|
| Exhausting memory with a long target | `ns.mount.target_max`, checked on the planning record before a fetch and again on the reply (5.4, point 2) |
| A mount-point cycle, or a chain deep enough to exhaust the node table or the stack | the depth and repeat checks before any fetch (5.6.1, point 1); the chain is fixed-size and inline (5.6.1, point 4); resolution is iterative per crossing, never recursive across crossings `[OWN]` |
| A user who may create symbolic links making a "mount point" from link text | classification by the server's marker only, never by content (5.4, point 1) |
| A target that smuggles a path, a second separator, a substitution or an encoded octet | the grammar refuses `/`, a second `:`, `#`, `%` and `@` in a base name, every control and non-ASCII octet, and every form it does not name (5.4) |
| A target naming any cell, to make the client send DNS queries for chosen names | one discovery per crossing, through the negative cache, only where discovery is enabled (5.6.2, point 7). A residual stays: a fileserver can make a client look up a name once per negative-cache lifetime, as a user typing a path can |
| Steering a path to the read/write version, or a backup, against the administrator's layout | the selection follows the target's own marks, the containing version and only the VL entry's read-only id (5.6.2); a VL entry that hides every read-only site makes the read-only path fail, never fall back to the read/write version (5.6.2, point 5); a version id of 0 is refused on every row (5.6.2, point 4). A forged target or VL entry under the null class can still choose the volume for that one unauthenticated request: the null class's residual (N5; O23; A13) |
| Unauthenticated location or target data steering an authenticated path | kept entries rest only on replies obtained with authentication in that context, and a request under a credential uses only those (5.6.2, point 9; C19, point 1); without an authenticated VL lookup the crossing fails closed with `LocationUnauthenticated` (5.6.2, point 10); a context without a credential keeps nothing, its version choices included, beyond the request (C20, point 1); a credentialed crossing inside a volume first resolves its own authenticated binding into it, and every binding, cached or resolved again, must lead to the root node already on the path, else `AnchorMismatch` and no crossing (5.6.2, point 9) |
| A cached target outliving the mount point's change or the reader's right | the decoded target is that object's cached data, used only under the reader's current grant at that data version; a callback break, a new data version or a revoked right ends it (5.6.2, points 8 and 9) |
| Links that climb with `..` and re-cross mount points to multiply work (SMB) | one visited set for the whole request, refusing any repeated mount point or link, and capping total crossings and hops (5.6.1, point 7) |
| Silently serving a read/write volume when the read-only sites fail | no silent change of version (5.6.2, point 5) |
| One context's resolution used for another | resolution is per context; root node keys carry the volume each context resolved, and the binding from a mount-point node to a root node, with the version it chose, is per context; a crossing uses only its own context's containing version, and a root node holds none (5.6.2, points 6 and 9; C11, point 9) |
| A `@sys` list or entry used to multiply work | at most `ns.sysname.count_max` in-memory lookups in one decoded directory, no extra calls (5.6.5, point 4); no recursion (5.6.5, point 6) |
| A literal `@sys` entry placed to capture lookups | substitution always runs; the literal entry is reached only from a listing (5.6.5, point 5) |
| A stale `@sys` result after the administrator changes the list | never cached under `@sys`, and the layers do not cache it (5.6.5, point 9) |
| Terminal escapes in a target shown by a diagnostic | a target is shown only with every octet outside 0x21 to 0x7E escaped, as [vl-fs-read.md](vl-fs-read.md), 5.4, point 14 escapes server text; a decoded target holds none (5.4, point 4) |
| A target that fails to decode, shown as something else | a failed mount point is never a link, a file or an empty directory (section 7, point 2) |
| Crashing the daemon | no panics, checked arithmetic (section 6); the fuzz target (10.4) |

## 9. How the invariants are kept

| Invariant (section 1) | Kept by | Test that fails if it is broken |
|---|---|---|
| Constraint 4 | 5.2; 5.4; section 6; section 7, point 3 | the fuzz target (10.4); the reject corpus (10.3) |
| C6, points 1 and 4 | every point labelled; the marker, the terminator, the root file id and the default list MISSING until captures (5.1; 5.4, points 1 and 3; 5.6.5, point 10) | review of this specification (section 14); no decoder for a MISSING layout merges (README, "Labelling claims", point 4) |
| C11, point 1 | 5.6.1, points 1, 4 and 8; 5.6.2, point 6; 5.6.3, point 5 | N3, N4, N6, R4 (10.5) |
| C11, point 3 | 5.6.4, point 2; 5.6.1, point 8 | N4, N5 (10.5) |
| C11, point 5 | 5.6.2, points 2 and 6 | V1, V2 (10.5) |
| C11, point 8 | 5.6.1 | D1 to D5 (10.5) |
| C11, point 9 | 5.6.2, points 6 and 9 | N1, N6 (10.5) |
| C12, points 1 and 2 | 5.6.3; 5.6.4, point 3 | R1, R2 (10.5) |
| C12, point 3 | 5.6.2, point 7; 5.6.3 | V9 (10.5) |
| C18, point 1 | 5.6.2, point 9 | N1, N2, N7, N9, N10 (10.5) |
| C19, point 1 | 5.6.2, points 9 and 10 | N2, N8, N9 (10.5) |
| C20, point 1 | 5.6.2, point 9 | N2, N10 (10.5) |
| C22, point 6 | 5.6.2, point 8; 5.6.5, point 4 | N7, Y5 (10.5) |
| C46, point 4 | 5.6.4, point 4; section 7, point 2 | T1 to T12 (10.3); F1 (10.5) |
| L32, point 1 | 5.7; 5.6.4, point 3 | R2 (10.5) |
| L32, point 3 | 5.6.1, points 1 and 7 | D1 to D5 (10.5); L1 (10.5, SMB, later) |
| L32, point 4 | 5.6.3, point 4 | R1 (10.5) |
| L34, point 4 | 5.6.3, points 1 and 3; 5.6.5 | R3, Y1 to Y9 (10.5) |
| The owner's depth bound | 5.6.1, points 1 and 2; 5.2 | D1, D2 (10.5) |

## 10. Tests

Paths: `crates/ironafs-proto/tests/mount_target.rs` for the decoder (5.4);
`crates/ironafs-cache/tests/it/namespace.rs` for crossing, selection, the
dynamic root and substitution, run against the simulator's VL and
fileserver models (#96) where a case needs replies (coding standards
rule 15).

### 10.1 Golden vectors

None yet. A golden target needs the stored bytes, which are MISSING
(5.1, Q1). The capture-backed revision adds, per form (regular,
read/write, cellular, each suffix), the target bytes reduced from the
lab's captures with synthetic cell and volume names, each with its
fixture provenance (venue, server release, date, scenario id)
([README](README.md), "Test vectors and fuzz targets", point 1).

### 10.2 Round-trip properties

Not applicable in M2a: there is no encoder (5.5). The test-only builder
and the decoder agree on every form 5.4 accepts (B1, a property test over
generated type marks, cell names, base names and suffixes); it is a
regression test, not evidence of interoperability.

### 10.3 Reject corpus

Each target must fail with `MountTargetInvalid` and the reason, at the
point named. Bytes are written in the printed form; once Q1 names the
terminator, each case carries it unless the case is about it.

| Case | Target | Reason | Section |
|---|---|---|---|
| T1 | 513 bytes of a valid form padded with a long base name | too long; no fetch when the planning record shows 513 | 5.4, point 2 |
| T2 | the empty target | empty | 5.4, point 2 |
| T3 | `#vol` with an octet 0x00, 0x0A, 0x1B, 0x20, 0x7F or 0xC3 inside, before the terminator | control or non-ASCII octet | 5.4, point 4 |
| T4 | a valid form without the terminator, or with a wrong octet in its place (terminator); the terminator alone (empty) | terminator, empty (after Q1) | 5.4, point 3 |
| T5 | `$vol`, `vol`, `!vol` | unknown type | 5.4, point 5 |
| T6 | `#:vol`, `#cell:`, `#a:b:c` | separator | 5.4, point 6 |
| T7 | `#a/b`, `#a#b`, `#a%b`, `#@sys` | volume name | 5.4, point 7 |
| T8 | `#` followed by a 65-byte base name; `#.readonly` (empty base) | volume name | 5.4, point 7 |
| T9 | `#v.readonly.readonly`, `#v.backup.readonly` | volume name (suffix in base) | 5.4, point 7 |
| T10 | `#536870913`, `%12` | numeric (until Q1) | 5.4, point 7 |
| T11 | `#a/b:vol`, `#a@b:vol`, `#a%b:vol`, a 254-byte cell part (decoder); `#Bad..Cell:vol`, `#-cell:vol`, any name #93's constructor refuses (caller) | cell name | 5.4, points 8 and 9 |
| T12 | a symbolic link whose text is `#cell:vol` | not a mount point: stays a link, never crossed | 5.4, point 1 |

Accepted forms, each decoded to the stated parts (A-cases): `#vol`
(regular, no cell, base `vol`, no suffix); `%vol` (read/write);
`#cell.example:vol.readonly` (regular, cell part, base `vol`, RO
suffix); `#vol.backup` (BK suffix); a 64-byte base name; a target of 328
bytes with a 253-character cell name (after Q1, plus its terminator).

### 10.4 Fuzz target

`ironafs-proto_mount_target` in the top-level `fuzz/` crate (coding
standards rule 20) decodes arbitrary bytes as a target and asserts: no
panic; no allocation beyond the target's length; every accepted target
has a type mark, a base name of 1 to `vl.name_max` bytes of the allowed
octets, at most one cell part, and at most one suffix; and the test-only
builder rebuilds the accepted input exactly.

### 10.5 Acceptance cases

Each case: the start state, the events, the result that must be seen.
Unless a case says otherwise, the simulator serves a cell whose volumes
are named `v0` to `v40`, each replicated or not as the case says, and
one context does the lookups.

**Depth and cycles (C11, point 8; the owner's bound):**

- **D1 crossing 32 is allowed.** A cell root (`root.cell`) holding mount
  point `m1` to `v1`, whose root holds `m2` to `v2`, and so on to `m32`
  to `v32`. A lookup of a file in `v32`'s root succeeds; that root's
  depth is 32 and its chain is `m1` to `m32` in order.
- **D2 crossing 33 is refused.** As D1, with `v32`'s root holding `m33`
  to `v33`. A lookup through `m33` fails with `MountDepthExceeded`, and
  no target fetch and no VL lookup for `m33` is sent.
- **D3 a repeat is refused.** No volume is replicated. `root.cell`'s
  root holds `m1` to `v1`; `v1`'s root holds `m2` naming `root.cell`. The
  path `m1/m2/m1` enters `root.cell` again at depth 2 and fails at the
  second `m1` with `MountCycle`.
- **D4 a self-mount.** `v1`'s root holds `m` naming `v1`. `m` is
  crossed once; `m/m` fails with `MountCycle`.
- **D5 the order of the two tests.** A chain of 31 distinct mount
  points to depth 31, then a mount point equal to the first:
  `MountCycle`. A chain of 32 distinct mount points to depth 32, then a
  mount point equal to the first: `MountDepthExceeded`, because the depth
  test runs first (5.6.1, point 1).
- **D6 the dynamic root does not count.** `/afs/<cell>` and
  `/afs/.<cell>` both give depth 0, and D1's 32 crossings hold below
  either.

**Selection (5.6.2):**

- **V1 suffix wins.** A regular target `#v1.readonly` in a read/write
  volume reaches `v1`'s RO id; `#v1.backup` reaches its BK id; with no
  RO version, `#v1.readonly` fails with `VersionAbsent` and never reads
  RW.
- **V2 read-only path.** In an RO containing version, `#v1` reaches RO
  when `v1` is replicated and RW when it is not.
- **V3 read/write path.** In an RW containing version, `#v1` reaches RW
  even when `v1` is replicated.
- **V4 read/write mount point.** `%v1` in an RO containing version
  reaches RW.
- **V5 cellular.** `#othercell:v1` in an RW containing version reaches
  RO when replicated; `#owncell:v1` does the same (row S4).
- **V6 backup containing version.** `#v1` in a BK containing version
  follows row S7.
- **V7 no silent change.** `v1` replicated, every RO site answering code
  103 then failing: the crossing fails with `offline`; no RW fetch is
  sent.
- **V8 the base name is asked.** `#v1.readonly` sends
  `vl.get_entry_by_name` for `v1`; an entry for `v2` is
  `VlEntryMismatch`, and nothing is entered.
- **V10 version id 0 on every row.** A VL entry whose read/write id is
  0: `%v1` (S3), `#v1` in an RW containing version (S6), `#v1` in an RO
  containing version with read-only id 0 (S5, which chooses RW), and
  `/afs/.<cell>` on such a `root.cell` each fail with `VersionAbsent`;
  no file-service request names volume 0, and no node is made.
- **V11 hidden read-only sites.** `v1` with a read-only id and every
  read-only site excluded by the site filter (unknown flag bit, not
  released): `#v1` in an RO containing version fails with `offline`; no
  read/write fetch is sent.
- **V9 unknown cell.** `#unknown.example:v1`, with discovery off: fails
  with `not found`, no DNS query; with discovery on and a negative
  answer: one query, then the negative cache answers a repeat.

**Nodes and contexts (C11, C18, C20):**

- **N1 per context.** Two contexts whose VL answers for `v1` differ (one
  sees a replicated `v1`, the other not): one mount point gives one
  mount-point node and two root nodes with different volume ids in their
  keys, and each context's binding holds its own version.
- **N2 nothing kept without a credential.** An anonymous context crosses
  `m1` twice: two target fetches and two VL lookups; an authenticated
  test context crosses it twice: one of each, while the entry lives.
- **N3 two paths, two nodes, two identities.** `v1` mounted by `m1` in
  `root.cell` and by `m2` in `v2`: its root has two root nodes with
  different anchors; the chain through `m1` holds `m1`, the chain
  through `m2` holds `m2`, and neither is a repeat (5.6.1, point 3). One
  mount point reached by two paths (`m1` reached again below `v2`
  through another mount point to `root.cell`) has one identity, and
  crossing it a second time in one chain is `MountCycle` (D3).
- **N4 ancestors stay.** At depth 20, the status and data of every
  directory and mount point above are evicted, every binding above has
  expired, and the OS has forgotten every node but one file in the
  depth-20 volume: the identity records of its parents, its containing
  root and every root node and mount-point node above stay in the table
  (5.6.1, point 8). With the path `/afs/<cell>/m1/…/m20/f` (no other
  directories), that is exactly 44 records: the dynamic root, the entry
  node, 20 mount-point nodes, 21 root nodes and the file, within the
  bound 2n = 44 for n = 22 components. The depth-20 root's chain is
  unchanged (extended on to depth 32, D2's refusal of the 33rd still
  fires, with no call sent), and `..` from that file walks back to
  `/afs` without a server's `..` entry and without ever returning a root
  node. A crossing of a mount point in the depth-20 volume first
  resolves this context's 21 expired bindings again, the `/afs/<cell>`
  entry binding into the cell root and `m1` to `m20`, each checked to
  lead to the root node already on the path (5.6.2, point 9). Once the file is
  unpinned and forgotten, the ancestors may be removed; a later walk
  makes each root node again with the same chain.
- **N5 `..`.** With `m1` in `root.cell`'s root directory, mounting
  `v1`: `/afs/<cell>/m1/..` is the `/afs/<cell>` entry node, never
  `root.cell`'s root node; `m1/f/..`, for a file `f` in `v1`'s root
  directory, is `m1`'s mount-point node, never `v1`'s root node; a
  directory `d` below `v1`'s root gives `d/f/..` = `d`; `/afs/<cell>/..`
  is `/afs`; a directory object whose `..` entry names another vnode
  changes none of these answers.
- **N6 one node id per path.** A lookup of `m1` returns the same node id
  before the crossing, after it, and in a second context whose crossing
  reached another volume; the two contexts list different contents
  under it, and their root nodes differ (C11, point 9).
- **N7 cached target lifetime.** An authenticated test context crosses
  `m1`, then: a callback break on `m1`'s object makes the next step fetch
  the target again; an ACL change that removes the right the target read
  needs makes the next step fail with `access`; a lookup grant lost on
  the directory holding `m1` makes the step fail before the target is
  used; `#v1` and `%v1` mount points in one directory each keep their
  own binding.
- **N8 no unauthenticated input on an authenticated path.** In the
  simulator, an authenticated context whose VL lookup can be made only
  without authentication: a lookup inside `/afs/<cell>` and a crossing
  of `m1` each fail with `LocationUnauthenticated`, reported as
  `SecurityFloor`; no null-class VL lookup is sent for that request,
  nothing is kept, and the same context with an authenticated VL model
  succeeds (5.6.2, point 10; C19, point 1).
- **N9 a version chosen by another context is never used.** `v1` is
  replicated, with read-only id x. An anonymous context crosses mount
  point `m` (target `#v1`) and receives a forged VL entry naming x as
  its read/write id and no read-only id, so its binding chooses RW for
  volume x. An authenticated context crosses the same `m` with an
  authenticated entry and chooses RO for x: both reach the one root node
  keyed by x. The authenticated context then crosses `#subvol` in `v1`,
  with `subvol` replicated: it reaches `subvol`'s RO version (its own
  containing version, row S5), never RW. Repeated with the anonymous
  crossing after the authenticated one, and with the authenticated
  context reaching `v1`'s directory only through a node the anonymous
  context's lookup made: the authenticated context first resolves `m`
  into `v1` with authentication in its own context, and the result is
  the same (5.6.2, point 9). Then with `v1`'s read-only id x and
  read/write id y distinct: the anonymous context's forged entry
  (read/write id y, no read-only id) enters the root node keyed by y,
  and the OS keeps a directory `d` there; the authenticated context,
  reaching `d` through that node, resolves `m` with authentication to
  the root node keyed by x ≠ y, and its crossing of a mount point in `d`
  fails with `AnchorMismatch`: nothing is crossed or kept inside the
  volume keyed by y. The same holds when the forged entry names another
  volume z of the cell.
- **N10 no version kept across anonymous requests.** An anonymous
  context crosses `m` (target `#v1`) (request 1) and the OS keeps a
  subdirectory `d` of `v1`. Request 2 crosses `#subvol` in `d`: it
  fetches `m`'s target and asks the VL service for `v1` again before it
  chooses `subvol`'s version; nothing from request 1 decides it (C20,
  point 1). With `v1`'s read-only id x and read/write id y distinct, and
  request 1's forged entry having entered the root node keyed by y:
  request 2 resolves `m` to the root node keyed by x, and its crossing
  in `d` fails with `AnchorMismatch`.

**The dynamic root (C12, L32, L34):**

- **R1 no contact.** A listing of `/afs`, and lookup, get attributes and
  access on `/afs/<cell>` and `/afs/.<cell>`, send no packet.
- **R2 unresolved mount point.** Get attributes on `m1` before crossing
  returns a directory's synthesised attributes, with no target fetch and
  no VL lookup; reading `m1`'s directory crosses it.
- **R3 both names.** A listing of `/afs` shows `<cell>` and `.<cell>`
  for each cell; `/afs/<cell>` reaches `root.cell` RO when replicated,
  `/afs/.<cell>` reaches its RW version.
- **R4 different nodes.** `/afs/<cell>` and `/afs/.<cell>` on an
  unreplicated `root.cell` reach one volume id with two anchors: two
  node ids.

**Substitution (L34, point 4):**

- **Y1 order.** List `[a, b, c]`; a directory holding `b` and `c`:
  `@sys` reaches `b`. Holding only `c`: `c`. Holding none: `not found`.
- **Y2 whole component only.** `bin@sys` and `@sys.old` are looked up as
  they are.
- **Y3 no recursion.** A list holding `@x`, `.`, `a/b` or a repeated
  name is refused by the configuration, and the previous list stays.
  List `[a]`, and a directory holding a symbolic link `a` whose text is
  `@sys`: a lookup of `@sys` returns the link `a` and nothing more; the
  kernel follows it, each later lookup substitutes once, and the
  kernel's own hop limit ends the loop.
- **Y4 no literal fallback.** A directory holding an entry named `@sys`
  and none of the list: `@sys` fails with `not found`; the listing shows
  `@sys`.
- **Y5 one grant check, no extra calls.** List of 16 names, none present:
  one directory fetch at most, one grant check, 16 in-memory lookups.
- **Y6 list change.** With the list changed from `[a]` to `[b]`, the
  next lookup of `@sys` reaches `b`; the FUSE layer answered the first
  lookup with no entry validity.
- **Y7 `@cell`.** With home cell `h`, `/afs/@cell` reaches `/afs/h`;
  with none configured, `not found`; `/afs/.@cell` is an ordinary name.
- **Y8 empty default.** With no administrator's list and the default
  still MISSING (5.6.5, point 10), `@sys` fails with `not found`.
- **Y9 not in targets.** A target `#@sys` is `MountTargetInvalid` (T7).
- **Y10 at the dynamic root.** List `[a, b]` with `b` a listed cell:
  `/afs/@sys` reaches `/afs/b` with no packet sent; with neither listed,
  `not found`, with no DNS query.

**Failure shape (C46, point 4):**

- **F1 a failed mount point is a failed directory.** A mount point whose
  target is T5's `$vol`: lookup and get attributes answer as R2; reading
  it fails with `not supported`; it never appears as a link or an empty
  directory.

**SMB, later (L32, point 3):**

- **L1** (with the SMB specification) a link loop and a chain of 33 link
  hops are each refused with the loop error, and a crossing reached
  through a link keeps its depth count. Thirty-two links, each climbing
  with `..` back to the cell root and re-crossing one mount point: the
  second crossing of a mount point already in the request's visited set
  is refused, and no request crosses more than 32 mount points in all.

### 10.6 Gaps

- Every golden target, and the terminator cases T4: section 13, Q1.
- D1 to D6, V1 to V9 and N1 to N5 need the simulator's VL and fileserver
  models (#96) and the node table (#91).
- L1: the SMB specification.

## 11. Revisit if

From the working decisions this rests on:

- **C11:** the M2a kernel tests show a kernel mishandles two node ids for
  one file under different anchors, or a layer needs node ids that
  outlive the daemon.
- **C12:** a supported desktop's indexer or file manager still contacts
  cells from a listing of `/afs`, measured in M2a.
- **C20:** measurements of anonymous reads of public cells show the
  refetch cost matters to users, and a design is shown that keeps C19.
- **C46:** a layer's error mapping cannot express a contract error
  without losing what the user needs to act on it (Q6).
- **L32:** an indexer or backup tool on a target OS ignores the
  exclusions, or acceptance tests find a traversal loop the bounds miss.
- **L34:** the macOS gate changes L10, or route qualification fails for
  the name `afs` (L41).

This specification's own choices are reopened if:

- a supported cell needs more than 32 nested mount points, or a
  reference client's own bound is found to count differently (Q7); the
  value is the owner's, so a change goes back to the owner;
- the captures show a stored target form 5.4 refuses (a numeric volume,
  a shortened cell name, an octet outside 5.4, point 7's set) on a
  supported server (Q1, Q8);
- a reference client follows a different version rule in a backup
  volume, or for a target naming its own cell (Q3, rows S4 and S7);
- a reference client substitutes `@sys` inside a component, or falls
  back to a literal `@sys` entry (Q4);
- a site needs a root volume other than `root.cell` for a cell, or cell
  aliases at the root.

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| M1 | DOCUMENTATION | OpenAFS reference page `fs_mkmount(1)`, sections "DESCRIPTION", "OPTIONS" (`-vol`, `-cell`, `-rw`) and "EXAMPLES", register of editions row E58 ([README](README.md)). A lead only (README, "Sources and provenance", rules 11 and 14) | 3, 5.1, 5.6.1, 5.6.2, 5.6.3 |
| M2 | DOCUMENTATION | OpenAFS reference page `fs_lsmount(1)`, section "OUTPUT", register of editions row E58. A lead only | 3, 4, 5.1, 5.4 |
| M3 | DOCUMENTATION | OpenAFS reference page `fs_sysname(1)`, sections "DESCRIPTION" and "OPTIONS" (`-newsys`), register of editions row E58. A lead only | 5.6.5 |
| M4 | DOCUMENTATION | OpenAFS reference page `afsd(8)`, options `-dynroot`, `-fakestat` and `-backuptree`, register of editions row E58. A lead only | 2, 5.6.2 |
| G1 | DOCUMENTATION | OpenAFS Administration Guide, page "Mounting Volumes" (HDRWQ208): "The Rules of Mount Point Traversal", "The Three Types of Mount Points", "To display a mount point", "To access volumes directly by volume ID", register of editions row E59. A lead only | 2, 3, 5.6.2, 5.6.3 |
| G2 | DOCUMENTATION | OpenAFS Administration Guide, page HDRWQ54, section "Using the @sys Variable in Pathnames", register of editions row E59. A lead only | 5.6.5 |
| G3 | DOCUMENTATION | OpenAFS Administration Guide, page "About Volumes" (HDRWQ177): "About Mounting Volumes", "About Volume Names", "Volume Information in the VLDB", register of editions row E9. A lead only | 4, 5.1, 5.4, 5.6.1 |
| Y1 | DOCUMENTATION | AFS Assigned Numbers Registry, "System Type Names and ID's" (`systypes.txt`), register of editions row E60. A lead only | 5.2, 5.6.5 |
| C | DECISION | [Claim record](../reviews/2026-11-20-adr-0004-claims.md) rows `[DES-054]` and `[ADR-008]`, re-read against their sources on 2026-11-21 | 4, 12 |
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; A13; C4, C6, C11, C12, C18, C19, C20, C22, C27, C33, C40, C46 ([part 1](../adr/0004-architecture-1-core.md)); L32, L34, L35, L37 ([part 2](../adr/0004-architecture-2-os-layers.md)); O23, O50 ([part 3](../adr/0004-architecture-3-operations.md)); N5 ([part 4](../adr/0004-architecture-4-negotiation.md)) | 1, 2, 5, 8, 9, 11 |
| D2 | DECISION | The owner's decision of 2026-11-20 on the mount-chain bound (crossing 32 allowed, 33 refused, a repeat refused at any depth), recorded in issue #80 | 1, 5.2, 5.6.1 |
| D3 | DECISION | [vl-fs-read.md](vl-fs-read.md), [directory-format.md](directory-format.md) and [xdr.md](xdr.md) | 2, 4, 5, 7, 8 |
| D4 | DECISION | [ADR 0001](../adr/0001-licence-and-dont-copy.md), "Don't copy"; [coding standards](../coding-standards.md), rules 1 to 3, 15 and 20 | 5.4, 6, 7, 10, 12 |
| O | OWN | every point tagged `[OWN]`, with its reason in place | 5, 6, 7 |

**What was not used.** No source code, test data, interface definition or
generated definition of any AFS or Rx implementation was read for this
specification, and none is cited. No copy of the 1991 AFS-3 programmer's
references was used (README, "Sources and provenance", rule 10). The
community Rx specification draft was not read for it. The reference pages
and the guide are cited for facts only, in IronAFS's own words; none is
quoted, and no example, command output or name from them is reproduced
beyond the octets `#`, `%`, `:`, the suffixes `.readonly` and `.backup`,
the volume name `root.cell` and the names `@sys` and `@cell`, which are
the facts themselves. The names of caps, errors and cases here are
IronAFS's own.

**Re-check of the cited claims (2026-11-21).** `[DES-054]`: the
`afsd(8)` page still says the dynamic root lists cells from the client's
cell list and contacts a cell only when its path is used, and that the
fake-stat options return made-up attributes for mount points; it agrees
with C12. `[ADR-008]`: the "About Volumes" page still says a volume's
entry records one id per version. The claim record holds no other row on
mount points or `@sys`. No contradiction with ADR 0004 C6, C11, C12, L32
or L34 was found.

### 12.1 Observed behaviour

None yet. The capture-backed revision records the observation lab's
captures and reference-client observations here (#51): a test venue with
synthetic cell names, volume names, users, keys and data; raw bytes
first, a protocol analyser only as a later cross-check; each observation
with its venue, server and client releases, date, scenario and the
evidence label EXECUTED.

### 12.2 Evidence per fact

| Fact | Source and register | Corroboration needed | Label |
|---|---|---|---|
| a mount point is marked by the server and read for its target | G3, E9 | Q1 | UNVERIFIED (role); MISSING (marker) |
| the printed target: `#` or `%`, optional `cell:`, volume, optional suffix | M2, E58; G1, E59 | Q1 | UNVERIFIED (form); MISSING (stored bytes, terminator) |
| a volume named by number; a shortened cell name | M1, E58 | Q1 | MISSING (stored form) |
| `.readonly` and `.backup` name the RO and BK versions; base names carry no suffix | G3, E9 | Q3 | UNVERIFIED |
| a read/write volume's name is at most 22 characters | G3, E9 | not load-bearing: the cap is `vl.name_max` | UNVERIFIED |
| traversal rules 1 to 3 and the three kinds of mount point | G1, E59; M1, E58 | Q3 | UNVERIFIED |
| a cell root is `root.cell` | G1, E59; M1, E58 | Q3 | UNVERIFIED |
| the file id of a volume's root directory | none | Q2 | MISSING |
| `@sys` is a whole component; the list is tried in order, first match wins | G2, E59; M3, E58 | Q4 | UNVERIFIED |
| `@cell` is the client's own cell | D1, L34.4 (decision); no document | Q4 | UNVERIFIED |
| the default list for Linux x86_64 and aarch64 | none; Y1, E60 gives no candidate | Q5 | MISSING |
| the depth bound, 32 | D2 (decision) | none: a decision | decision |

## 13. Open questions

Each is settled by the observation lab (#51) unless it says otherwise.
Q1 and Q2 block all code written from 5.1 and 5.4.

- **Q1. The stored mount point.** How the status record marks a mount
  point (a file type value, a mode, or other); the exact bytes of the
  target for each form (regular, read/write, cellular, each suffix),
  including any terminator; whether a mount point made with a volume
  id stores the number, and how it differs from a name of digits; whether
  a shortened cell name is stored shortened; which rights reading the
  target needs. Settled by making each form with a reference client's
  command in the lab, one input varied at a time, and capturing the
  status and the data fetch. Recommendation: one campaign with
  [vl-fs-read.md](vl-fs-read.md)'s Q1, since the status record and the
  fetch are the same calls.
- **Q2. A volume's root.** The vnode and uniquifier of a volume's root
  directory, for read/write, read-only and backup versions. Settled by
  fetching the status of the root of each version of several lab
  volumes.
- **Q3. The traversal rules, observed.** Rows S1 to S7 with a reference
  client: in particular a regular target in a backup volume (S7), a
  cellular target naming the client's own cell (S4), what a reference
  client does when every read-only site of a replicated volume fails or
  is excluded (5.6.2, point 5), and, with the client's dynamic root on,
  which volume and version `/afs/<cell>` and `/afs/.<cell>` reach
  (`root.cell`, 5.6.3, points 1 and 2). IronAFS's rule stands for S7 and point
  5 unless the answer shows a supported workflow that depends on the
  other behaviour; then it is a class S change of this specification.
- **Q4. Substitution, observed.** With a reference client in the lab: a
  list of three names against directories holding different subsets
  (order and result); `bin@sys` and `@sys.old`; a directory holding a
  literal `@sys`; `@sys` inside a mount-point target; and what `@cell`
  resolves to with and without a configured home cell. Records the
  result for each; differences from 5.6.5 are listed as compatibility
  costs or brought back as a change.
- **Q5. The default system-name list for Linux x86_64 and aarch64.** No
  admissible evidence exists yet. Recommendation, in order:
  (a) observe a clean reference client per target: a released client
  package of the distribution matrix (#51), installed fresh in a lab VM
  of that architecture, with no site configuration, and record what its
  system-name query reports before anything is set; record the release,
  package and architecture, and never a lab's configured list;
  (b) if no clean client runs on a target, the owner chooses an ordered
  default for it, recorded here with the decision's date;
  (c) until (a) or (b), the default stays empty (5.6.5, point 10), which
  fails closed. Choosing a default is the owner's decision under issue
  #80; it is not inferred from the registry, whose list has no entry for
  these targets. Issue #80's outcome needs (a) or (b) recorded for each
  target before the issue closes; (c) alone does not complete it.
- **Q6. The loop error.** C46's list has no value for a refused traversal
  loop. Recommendation: the contract (C7, #67) adds a loop value, reported
  as `ELOOP` on Linux and macOS and as `STATUS_STOPPED_ON_SYMLINK` or the
  generic failure on Windows, with the cause (depth or repeat) in the side
  channel. Until then the rows of section 7 that say "the loop error" use
  the generic failure ([vl-fs-read.md](vl-fs-read.md), section 13, Q11).
  It is a contract decision, not this specification's.
- **Q7. A reference client's own bound.** Does a reference client bound
  mount-point depth, and from where does it count? Only the compatibility
  statement of 5.6.1, point 6 depends on it; the owner's bound stands.
- **Q8. Volume name octets.** Which octets supported servers accept in a
  volume name. If a supported server accepts an octet that 5.4, point 7
  refuses, the set is widened at class S, with the octet named; `/`,
  `:`, `#`, `%`, `@`, control and non-ASCII octets stay refused. Also:
  whether a server accepts a base name that itself ends with `.readonly`
  or `.backup` (5.4, point 7 states the cost if one does).
- **Q9. A volume repeated along one path through different mount
  points.** The owner's rule refuses a repeated mount point, not a
  repeated volume, and 5.6.1, point 3 keeps it so. Refusing a repeated
  volume as well would stop a self-mount one crossing earlier, at the
  cost of paths a reference client follows. Recommendation: keep the
  owner's rule; the depth bound already bounds every loop.
- **Q10. Authenticated location data (an owner question).** C19,
  point 1 (Firm) forbids answering a request made under a credential
  with routing obtained without authentication. Every VL lookup in
  [vl-fs-read.md](vl-fs-read.md) is a null-class call, so as written a
  credentialed request on a real cell fails every crossing with
  `LocationUnauthenticated` (5.6.2, point 10). M2a is not affected (no
  credentialed requests on a real cell); M2b's authenticated namespace is
  blocked until this is settled. Options:
  - **(a) Authenticated VL lookups.** In M2b, a credentialed context
    makes its VL lookups on an rxkad connection of its own, and the
    lab confirms that every VL server in the capture matrix answers them.
    No ADR change; this specification stands as written.
  - **(b) Amend C19 for location data.** An ADR 0004 amendment lets a
    null-class VL entry choose the server and volume id for an
    authenticated path as a hint, on condition that the authenticated
    file-service replies that follow confirm the volume (its id in the
    authenticated status and volume sync record). A change to a Firm
    decision: the owner's, through the ADR process.
  - **(c) Keep failing closed.** Authenticated users cannot cross mount
    points on real cells until rxgk or another authenticated location
    source exists.

  **Recommendation: (a)**, with the lab check scheduled as part of M2b's
  rxkad work (the VL lookups of vl-fs-read.md gain an authenticated
  variant in its M2b revision); take (b) to the owner as an ADR
  amendment only if the lab shows a supported VL server refusing
  authenticated lookups. (c) is the interim state, not a choice, since
  it blocks M2b's purpose.

## 14. Review

The review record is
[2026-11-21-m2a-80-namespace](../reviews/2026-11-21-m2a-80-namespace.md).
