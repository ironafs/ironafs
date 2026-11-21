<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# The FUSE codec

Status: draft (2026-11-21). Raised in issue #92.

Kind: protocol. Class: S. The kernel's requests reach this decoder from
every local process that touches `/afs`, so it is a decoder of untrusted
input, and the INIT reply it builds decides which kernel caches and
privileges the mount has ([README](README.md), "Review class per kind").

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (E70 to E74)
are rows of the register of editions; section 12 lists them.

## 1. What this implements

ADR 0004, overview, constraint 4 (a constraint of the whole ADR; it
changes only with the ADR):

> **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
> local IPC, configuration, requests from the operating system, and SMB
> messages before authentication. No panics, checked arithmetic and byte
> limits apply to all of it (coding standards rules 1 to 3).

ADR 0004, L2 (**Working**), points 1 to 4 and 6:

> 1. The Linux layer speaks the FUSE kernel protocol directly over
>    `/dev/fuse`, in IronAFS's own code, and runs inside `ironafsd`.
> 2. The wire floor is protocol 7.23. The layer negotiates up to what the
>    kernel offers.
> 3. Features are detected from the INIT flags and from per-operation replies,
>    never from the minor number alone.
> 4. The layer never answers `ENOSYS` to ACCESS, nor to an operation that a
>    volume could later support.
> 6. FUSE passthrough is not used.

ADR 0004, L3, point 2 (**Firm**):

> 2. As a second barrier, the attribute mapping clears set-uid and set-gid
>    bits and never presents a device node.

ADR 0004, L4 (**Firm**), points 1 and 3, and the first sentences of
point 2:

> 1. The FUSE writeback cache is never enabled.
> 2. Shared writable mappings are refused. Shared read-only mappings are
>    refused too while direct I/O is set (context, point 5; L5).
> 3. The kernel's own permission checks are never used in place of the
>    core's: no `default_permissions`, and no POSIX ACL negotiation.

ADR 0004, L5 (**Working**), points 1 to 3:

> 1. The mount uses `allow_other`. Without `default_permissions`, LOOKUP is
>    where traversal is enforced, so entry and attribute timeouts start at
>    zero.
> 2. Every open uses direct I/O.
> 3. The layer never sets the keep-cache or cache-directory open flags.

ADR 0004, N43 (**Working**):

> 1. On Linux FUSE, no operation that a volume capability (N42) could enable
>    is answered `ENOSYS`.
> 2. Such a request is answered with a per-operation "not supported" error
>    instead.

ADR 0004, N47 (**Working**), point 6:

> 6. A creation time records whether the server supplied it. A layer
>    synthesises one from the modification time only when the server gives
>    none.

The contract between the core and the layers (`vfs-contract.md`, issue
#67) leaves four points to this codec: the Linux error column's re-check
(its section 13, Q8), Linux's name and extended-attribute limits (its Q6),
the reply to an interrupt (its 5.8, point 7), and the kernel's answer to a
notification for an uncached entry (its 5.10, point 4). Sections 5.7,
5.8 and 13 take them up.

## 2. Scope

1. **Covered:** the request header; the opcode table and which requests
   take no reply; INIT: decoding, version negotiation, the INIT-flag
   allowlist and never-list, the function that selects the reply flags,
   and the reply; decoding of the requests a read-only mount needs
   (LOOKUP, FORGET, BATCH_FORGET, GETATTR, STATX, READLINK, OPEN, OPENDIR,
   READ, READDIR, RELEASE, RELEASEDIR, FLUSH, STATFS, ACCESS, INTERRUPT,
   DESTROY); the replies to them except READDIR's, each reachable only
   through its request's reply token; the mapping of the contract's errors
   to Linux error numbers.
2. **Not covered yet, and still this specification's** (section 13,
   Q1): the READDIR reply's records, the invalidation notifications,
   GETXATTR, LISTXATTR, FSYNC, FSYNCDIR and LSEEK, the requests that
   change a volume, and the reserved operations (contract 5.6, point 8).
   Until they are written, the codec decodes each of these opcodes as
   "known, not decoded" (5.4, point 9).
3. **Not covered here:** reading and writing `/dev/fuse`, the mount and its
   options, and which contract operation each request becomes: the Linux
   layer's (issues M2a-30 and M2a-31, planned). The codec is pure: bytes in,
   values out, and values in, bytes out. It reads no clock and does no I/O.

## 3. The design in short

1. One read of `/dev/fuse` holds one request. A session, the
   connection's state, decodes it and later writes its one reply. The decoder checks the
   header's length against the bytes read, then the body against the
   opcode's layout at the negotiated minor, and returns a typed request
   that borrows nothing it has not checked.
2. Before INIT is answered only INIT is accepted. INIT negotiates the
   minor as the smaller of the kernel's and 46, refusing anything below
   7.23, and selects the reply flags as the kernel's offer intersected with
   one reviewed constant, `ALLOWED`. A second constant, `NEVER`, names the
   flags no reply may carry; the two are disjoint, which the compiler
   checks.
3. Replies are built from typed values, through a single-use reply token
   that only the decoder makes and that offers only the shapes its request
   allows. The encoders write the cache rules of L5 themselves (zero
   timeouts, direct I/O, no cache flags) and clear set-id bits (L3), so no
   caller can pass a value that breaks them, or bytes that skip them.
4. Every error the codec or the layer can answer is an `Errno` value, a
   type with no `ENOSYS` in it (N43).

## 4. Terms

- **Request:** what one read of `/dev/fuse` returns: the header and its
  body.
- **Body:** the bytes after the 40-byte header.
- **Minor:** the protocol's minor version. **The negotiated minor** is the
  one INIT settled.
- **Offer:** the INIT flags the kernel sent, as one 64-bit set (5.3,
  point 2). **Selection:** the flags the INIT reply carries.
- **Unique:** the request's `unique` field, which its reply repeats.
- **Frame error:** a request whose header cannot be trusted, so it gets no
  reply. **Request error:** a request whose header is sound and whose body
  is not.

## 5. Specification

### 5.1 Layout

1. **Byte order.** Every field is in the host's byte order: the protocol
   reserves byte-swapped INIT opcodes so that a mismatch can be detected
   `[E70 enum fuse_opcode: CONFIRMED]`. Every target IronAFS builds for
   Linux is little-endian (x86-64 and AArch64, ADR 0004 L42, point 3), so
   the codec reads and writes little-endian on every host, and its tests
   run the same on the development Mac [OWN].
2. **The request header** is 40 bytes `[E70 struct fuse_in_header;
   E71 the same struct: CONFIRMED]`:

   | Offset | Field | Type |
   |---|---|---|
   | 0 | `len`: the whole request, header included `[E73 "The basic protocol": CONFIRMED]` | u32 |
   | 4 | `opcode` | u32 |
   | 8 | `unique` | u64 |
   | 16 | `nodeid` | u64 |
   | 24 | `uid` | u32 |
   | 28 | `gid` | u32 |
   | 32 | `pid` | u32 |
   | 36 | `total_extlen` (u16, from 7.38) and padding (u16); before 7.38, padding (u32) | 4 bytes |

3. **The reply header** is 16 bytes: `len` (u32, the whole reply), `error`
   (i32), `unique` (u64) `[E70 struct fuse_out_header: CONFIRMED]`. An
   error is sent as the negative error number, and an error reply carries
   no payload `[E73 "The basic protocol", FUSE_ACCESS: CONFIRMED]`.
4. **Bodies decoded.** All sizes are in bytes, and each holds from 7.23 to
   7.46 `[E70; E71: CONFIRMED]`, because every change in that range kept
   the size (a field renamed from `unused` or `padding`):

   | Opcode | Number | Body |
   |---|---|---|
   | LOOKUP | 1 | a name and a terminating NUL |
   | FORGET | 2 | `nlookup` u64 (8) |
   | GETATTR | 3 | `getattr_flags` u32, `dummy` u32, `fh` u64 (16) |
   | STATX | 52 | `getattr_flags` u32, reserved u32, `fh` u64, `sx_flags` u32, `sx_mask` u32 (24); from 7.39 |
   | READLINK | 5 | empty |
   | OPEN | 14 | `flags` u32, then a u32: unused before 7.33, `open_flags` from it (8) |
   | READ | 15 | `fh` u64, `offset` u64, `size` u32, `read_flags` u32, `lock_owner` u64, `flags` u32, padding u32 (40) |
   | STATFS | 17 | empty |
   | RELEASE | 18 | `fh` u64, `flags` u32, `release_flags` u32, `lock_owner` u64 (24) |
   | FLUSH | 25 | `fh` u64, unused u32, padding u32, `lock_owner` u64 (24) |
   | INIT | 26 | `major`, then for major 7: `minor`, `max_readahead`, `flags` (u32 each) (16); with `FUSE_INIT_EXT`, from 7.36, also `flags2` u32 and eleven unused u32 (64) |
   | OPENDIR | 27 | as OPEN |
   | READDIR | 28 | as READ |
   | RELEASEDIR | 29 | as RELEASE |
   | ACCESS | 34 | `mask` u32, padding u32 (8) |
   | INTERRUPT | 36 | `unique` u64 of the interrupted request (8) |
   | DESTROY | 38 | empty |
   | BATCH_FORGET | 42 | `count` u32, `dummy` u32, then `count` records of `nodeid` u64 and `nlookup` u64 (8 + 16 × count) |

   STATX exists from 7.39 and its body has one layout `[E70 changelog
   7.39, struct fuse_statx_in: CONFIRMED]`. The empty bodies and LOOKUP's
   name are `[E73 FUSE_LOOKUP, FUSE_STATFS:
   CONFIRMED]` and E70's opcode table; DESTROY and READLINK have no input
   structure in E70, so their bodies are empty [E70: CONFIRMED by the
   absence of a structure; OWN reading].
5. **Replies encoded** `[E70: CONFIRMED]`:
   - `fuse_attr` (88): `ino`, `size`, `blocks`, `atime`, `mtime`,
     `ctime` (u64 each); `atimensec`, `mtimensec`, `ctimensec`, `mode`,
     `nlink`, `uid`, `gid`, `rdev`, `blksize`, `flags` (u32 each). The
     last is padding before 7.32 `[E71]`; the codec writes zero.
   - LOOKUP: `fuse_entry_out` (128): `nodeid`, `generation`,
     `entry_valid`, `attr_valid` (u64 each), `entry_valid_nsec`,
     `attr_valid_nsec` (u32 each), `fuse_attr`.
   - GETATTR: `fuse_attr_out` (104): `attr_valid` u64,
     `attr_valid_nsec` u32, `dummy` u32, `fuse_attr`.
   - STATX: `fuse_statx_out` (288): `attr_valid` u64, `attr_valid_nsec`
     u32, `flags` u32, two spare u64, then `fuse_statx` (256): `mask`,
     `blksize` (u32 each), `attributes` u64, `nlink`, `uid`, `gid` (u32
     each), `mode` u16, a spare u16, `ino`, `size`, `blocks`,
     `attributes_mask` (u64 each), the times `atime`, `btime`, `ctime`,
     `mtime` (each `fuse_sx_time`, 16: `tv_sec` i64, `tv_nsec` u32, a
     reserved i32), `rdev_major`, `rdev_minor`, `dev_major`, `dev_minor`
     (u32 each), fourteen spare u64. The mask's bits are `STATX_BASIC_STATS`
     (`0x7ff`) and `STATX_BTIME` (`0x800`) `[E74 stat.h: CONFIRMED]`.
   - OPEN, OPENDIR: `fuse_open_out` (16): `fh` u64, `open_flags` u32,
     and a u32 that is padding before 7.40 and `backing_id` from it; the
     codec writes zero.
   - STATFS: `fuse_kstatfs` (80): `blocks`, `bfree`, `bavail`, `files`,
     `ffree` (u64 each); `bsize`, `namelen`, `frsize`, padding (u32 each);
     six spare u32.
   - READ: the data; READLINK: the target's bytes, with no NUL
     `[E73 FUSE_READ: CONFIRMED]`; the target carries no NUL [OWN: the
     reply's length bounds it].
   - FLUSH, RELEASE, RELEASEDIR, ACCESS and DESTROY on success: the
     header alone `[E73 FUSE_FLUSH, FUSE_RELEASE, FUSE_ACCESS:
     CONFIRMED]`; DESTROY likewise [OWN: E73 says every request is
     answered unless documented otherwise, and documents none for
     DESTROY].
   - INIT: `fuse_init_out` (64) for every minor from 7.23: `major`,
     `minor`, `max_readahead`, `flags` (u32 each), `max_background`,
     `congestion_threshold` (u16 each), `max_write`, `time_gran` (u32
     each); from 7.28 `max_pages` u16; from 7.31 `map_alignment` u16; from
     7.36
     `flags2` u32; from 7.40 `max_stack_depth` u32; from 7.43
     `request_timeout` u16; the rest unused. Below each field's minor its
     bytes are unused, and the codec writes zero in every field it does
     not set (5.5, point 6) `[E70; E71: CONFIRMED]`.
6. **File type bits** in `mode`: directory `0o040000`, regular file
   `0o100000`, symbolic link `0o120000`; set-uid `0o4000`, set-gid
   `0o2000` `[E74 stat.h: CONFIRMED]`.

### 5.2 Caps

| Value | Cap | Over the cap | Label |
|---|---|---|---|
| A request | the bytes one read returned; `len` must equal them | frame error | [E73 ERRORS: a request larger than the buffer fails the read; OWN: the equality] |
| A name in LOOKUP | as `Name`: 1 to 255 bytes, no `/`, no NUL, not `.` or `..` | `ENAMETOOLONG` if long, else `EINVAL` | [vfs-contract 5.12, point 4]; Linux's `NAME_MAX` is 255 `[E74 limits.h: CONFIRMED]` |
| READ `size` | 1,048,576 bytes | `EINVAL` | [vfs-contract 5.2, point 3; OutOfRange] |
| READ `offset`, and `offset + size` | at most `i64::MAX`, by checked addition | `EINVAL` | [vfs-contract 5.2, point 4] |
| READDIR `size` | 1,048,576 bytes | `EINVAL` | [vfs-contract 5.2, read-directory budget] |
| BATCH_FORGET `count` | the body: `8 + 16 × count` must equal its length (6, point 4) | `EIO`, but no reply (5.6) | [OWN] |
| INIT `max_write` replied | 1,048,576 bytes | refused when the configuration is built | [vfs-contract 5.2, point 3] |
| READ reply data | 1,048,576 bytes | refused by the encoder | [vfs-contract 5.2, point 3] |
| READLINK reply | 1 to 4,095 bytes, no NUL | refused by the encoder | [vfs-contract 5.2, link target] |

READDIR's `offset` is the layer's own cookie, not a byte offset, so it
has no cap [OWN]. Nothing is allocated from a decoded length but one
thing: a LOOKUP's name is copied into its `Name`, only after its 255-byte
cap is checked. Every other part of a decoded request borrows the read
buffer, and BATCH_FORGET's records are walked in place. A reply's buffer
is sized by its payload, which its encoder caps first (for READ, at the
size the request asked for).

### 5.3 Versions

1. **Negotiation** `[E70 "Version negotiation"; E73 FUSE_INIT:
   CONFIRMED]`. The codec's own version is 7.46, `MAX_MINOR`, the minor of
   the header it is written from (E70). For the kernel's INIT:
   - the major is read first, and decides the rest: for a major other than
     7 nothing past it is read, since its layout is not 7.x's [OWN: E73's
     rule that the kernel sends a new INIT after the major-only reply];
   - major 7, minor 23 or higher: the negotiated minor is the smaller of
     the kernel's and 46;
   - major 7, minor below 23: refused (L2, point 2), the reply `EPROTO`
     [OWN: no source names the error; the layer aborts the connection
     after it, section 13, Q4];
   - a major above 7: the reply is the header and one u32, the major 7,
     and the codec waits for a new INIT `[E73 FUSE_INIT: CONFIRMED]`;
   - a major below 7: refused, `EPROTO` [OWN].

   The INIT request's token holds its checked offer, and one call,
   `InitReply::answer`, negotiates with that offer and encodes the one
   reply its result allows: the INIT reply, the major-only reply, or
   `EPROTO`. No INIT reply can carry another offer's result, and the
   major-only reply cannot be chosen for a major-7 offer [OWN: 5.5,
   point 11's rule, for INIT].
2. **The offer** is the `flags` field, plus, when `flags` holds
   `FUSE_INIT_EXT` (bit 30), `flags2` shifted up by 32 bits `[E70 "bits
   32..63 get shifted down 32 bits into the flags2 field": CONFIRMED]`.
   `FUSE_INIT_EXT` marks the extended request `[E70: "extended
   fuse_init_in request"]`, so a major-7 body is exactly 16 bytes without
   it and exactly 64 with it; any other size, trailing bytes included, is
   a request error (`EIO`). The eleven unused words of the 64 are not read
   [OWN: E70 names them unused]. An allowlisted flag offered by a minor
   below the one that defines it (`FUSE_INIT_EXT` before 7.36,
   `FUSE_NO_EXPORT_SUPPORT` before 7.40 `[E70 changelog]`) is a request
   error too (`EIO`) [OWN: the offer contradicts its own version]. Because
   the negotiated minor is the kernel's when the kernel's is at most 46,
   and 46 is above both, every selected flag exists at the negotiated
   minor. Only the decoder builds the `InitRequest` that negotiation
   takes, so negotiation sees only checked offers.
3. **Features come from flags** (L2, point 3). The codec branches on the
   negotiated minor only where E70 gives a field a meaning from a minor on:
   the header's `total_extlen` (5.4, point 3) and OPEN's second word (5.4,
   point 6); every request it decodes has the same size across 7.23 to
   7.46 (5.1, point 4).

### 5.4 Decoding

In this order; the first failure is the result.

1. **Frame.** Fewer than 40 bytes is a frame error. A `len` that differs
   from the number of bytes read is a frame error. A frame error has no
   trusted `unique` and gets no reply; the layer treats it as a broken
   connection [OWN: a reply under a wrong `unique` could answer another
   request].
2. **Phase.** The phase is the session's (5.6, point 3). Before a minor
   is negotiated, a request other than INIT is a request error (`EIO`),
   since INIT is "the first request sent by the kernel" `[E73 FUSE_INIT:
   CONFIRMED]`. After it, a second INIT is a request error (`EIO`) [OWN].
   A request id still owed a reply is refused before anything else of its
   body is read, with no reply (5.6, point 3).
3. **Extensions.** At a negotiated minor of 38 or more, a non-zero
   `total_extlen` is a request error (`EIO`): extensions are sent only for
   flags this codec never selects (`FUSE_SECURITY_CTX`,
   `FUSE_CREATE_SUPP_GROUP`, 5.5's table) `[E70 changelog 7.36, 7.38:
   CONFIRMED]`. Below 38, and before INIT, those four bytes are padding
   and are ignored [OWN: a kernel of that minor defines no meaning for
   them].
4. **Body length.** A fixed body (5.1, point 4) must have exactly its
   size; an empty one must be empty; INIT's as 5.3, point 2, says. Anything
   else is
   a request error (`EIO`) [OWN: a body of another size means the two
   sides disagree on the layout, and guessing would read the wrong
   fields].
5. **Names.** LOOKUP's body must end in a NUL and hold no other; the bytes
   before it become a `Name` through the Linux name codec, and its
   refusal is answered as vfs-contract 5.12, point 4 says.
6. **Flags fields the codec reads.** `getattr_flags` may hold only
   `FUSE_GETATTR_FH` (bit 0), which says `fh` is set; `read_flags` only
   `FUSE_READ_LOCKOWNER` (bit 1); `release_flags` only
   `FUSE_RELEASE_FLUSH` (bit 0) and `FUSE_RELEASE_FLOCK_UNLOCK` (bit 1)
   `[E70: CONFIRMED]`; STATX's `getattr_flags` as GETATTR's. OPEN's and
   OPENDIR's second word is unused below 7.33 and ignored there; from 7.33
   it is `open_flags`, whose one defined bit, `FUSE_OPEN_KILL_SUIDGID`,
   comes only with `FUSE_HANDLE_KILLPRIV_V2`, never selected `[E70
   changelog 7.33: CONFIRMED]`, so from 7.33 any bit in it is refused. Any
   other bit is a request error (`EIO`) [OWN: a bit the negotiated minor
   does not define means the layouts disagree]. Fields the codec hands on
   whole (the `open(2)` flags, ACCESS's `mask`, STATX's `sx_flags` and
   `sx_mask`) are checked by the layer that gives them meaning. Padding,
   `dummy` and reserved words are ignored.
7. **Ranges.** READ's and READDIR's caps (5.2), by checked arithmetic.
   Over a cap: `EINVAL`.
8. **BATCH_FORGET's count** (5.2).
9. **Opcodes.** An opcode of E70's table that this specification does not
   decode yet is returned as "known, not decoded" with its name; an opcode
   not in the table is returned as unknown with its number. Neither is an
   error of the codec. The layer answers both with `EOPNOTSUPP`, never
   `ENOSYS` (N43; L2, point 4) [OWN: neither E72 nor E73 says how to answer
   an opcode a daemon does not know; `ENOSYS` is permanent for some kinds
   (ADR 0004 part 2, context point 4), and a per-operation error never
   is]. `CUSE_INIT` and the byte-swapped INIT opcodes are not FUSE requests
   and count as unknown.
10. **A request the kernel may fall back from is served, not refused
    with `ENOSYS`.** The negotiated minor can be 7.39 or above, so the
    kernel may send STATX. `ENOSYS` is not an answer to it: L2, point 4,
    forbids `ENOSYS` "to an operation that a volume could later support",
    and STATX carries the birth time a volume can supply (N47, point 6).
    Lowering `MAX_MINOR` below 7.39 instead would give up
    `FUSE_NO_EXPORT_SUPPORT` (7.40, 5.5, point 1). So the codec decodes
    STATX and encodes its reply: the basic fields always, and the birth
    time only when the layer has one; statx's own rules let a reply clear
    the bit of a field it does not fill `[E74 stat.h, the comment above
    struct statx: CONFIRMED]`. No registered source says how the kernel
    treats any other error to STATX, so the layer answers STATX and never
    refuses it as not decoded. Before the Linux layer ships, each other
    opcode a kernel may send at the negotiated minor is likewise decoded
    and answered, or refused per operation as the contract says (section
    13, Q1) [OWN].
11. **An opcode above the negotiated minor.** STATX at a negotiated minor
    below 7.39, where E70 does not define it, is refused before its token
    is made, with `EOPNOTSUPP` (`Malformed::AboveMinor`), the per-operation
    "not supported" error of N43 [OWN].

### 5.5 Encoding

1. **The INIT-flag allowlist** is `ALLOWED`, a named constant of the
   crate. Each flag in it, with its reason:

   | Flag (bit) | Why it is accepted |
   |---|---|
   | `FUSE_INIT_EXT` (30) | The reply's `flags2` field is read only with it `[E70 "FUSE_INIT_EXT: extended fuse_init_in request"; changelog 7.36]`, and `FUSE_NO_EXPORT_SUPPORT` lives there. It changes nothing else. |
   | `FUSE_NO_EXPORT_SUPPORT` (38) | "Explicitly disable export support" `[E70]`. A node reached through a file handle (`open_by_handle_at`) arrives without a LOOKUP, and LOOKUP is where traversal is enforced (L5, point 1) [OWN]. Kernels below 7.40 cannot be asked this (section 13, Q2). |

2. **The never-list** is `NEVER`, a second constant. No selection ever
   holds one of its flags; a compile-time check refuses a build in which
   `ALLOWED` and `NEVER` share a bit.

   | Flag (bit) | Why it is never accepted |
   |---|---|
   | `FUSE_WRITEBACK_CACHE` (16) | L4, point 1 (Firm). |
   | `FUSE_POSIX_ACL` (20) | L4, point 3 (Firm): it turns the kernel's own checks on `[DES-026]`. |
   | `FUSE_DIRECT_IO_ALLOW_MMAP` (36) | L4, point 2 (Firm): it allows shared mappings in direct-I/O mode `[E72 fuse-io.rst: CONFIRMED]`. |
   | `FUSE_PASSTHROUGH` (37) | L2, point 6. |

   `default_permissions` is a mount option, not a flag `[E72 fuse.rst
   "Mount options": CONFIRMED]`; it stays off by L4, point 3, which the
   mount applies.

3. **Every other flag is not accepted** now. Each is named here so that a
   later change adds it on purpose, with its own review and test: the
   read path's `FUSE_ASYNC_READ` (0), `FUSE_PARALLEL_DIROPS` (18),
   `FUSE_MAX_PAGES` (22, which needs the page size, a mount fact) and
   `FUSE_ABORT_ERROR` (21) are candidates; the cache flags
   (`FUSE_AUTO_INVAL_DATA` 12, `FUSE_DO_READDIRPLUS` 13,
   `FUSE_READDIRPLUS_AUTO` 14, `FUSE_CACHE_SYMLINKS` 23,
   `FUSE_EXPLICIT_INVAL_DATA` 25) wait on L5's evidence; the write path's
   flags (`FUSE_ATOMIC_O_TRUNC` 3, `FUSE_BIG_WRITES` 5, `FUSE_DONT_MASK` 6,
   `FUSE_HANDLE_KILLPRIV` 19, `FUSE_HANDLE_KILLPRIV_V2` 28,
   `FUSE_SETXATTR_EXT` 29) wait on M3; `FUSE_POSIX_LOCKS` (1) and
   `FUSE_FLOCK_LOCKS` (10) wait on L33; `FUSE_EXPORT_SUPPORT` (4) is the
   opposite of the allowlist's choice; `FUSE_SECURITY_CTX` (32) and
   `FUSE_CREATE_SUPP_GROUP` (34) add request extensions this codec refuses
   (5.4, point 3); `FUSE_ALLOW_IDMAP` (40) changes what the header's uid
   and gid mean `[E70 FUSE_INVALID_UIDGID]`, and the caller's identity is
   what the core authorises; `FUSE_HAS_RESEND` (39) changes what `unique`
   means `[E70 FUSE_UNIQUE_RESEND]`; the splice, ioctl, DAX, mapping,
   submount, io-uring and timeout flags serve nothing a read-only mount
   needs; `FUSE_INIT_RESERVED` (31) is "reserved, do not use" `[E70]`.
   The flags the kernel only reports (`FUSE_NO_OPEN_SUPPORT` 17,
   `FUSE_NO_OPENDIR_SUPPORT` 24, `FUSE_HAS_EXPIRE_ONLY` 35,
   `FUSE_HAS_IO_URING_BUFPOOL` 43) are not echoed [OWN: echoing a report
   asks for nothing this codec uses]. Bits E70 does not define are never
   selected.
4. **The selection function**, `select_flags(offer)`, returns exactly
   `offer & ALLOWED`. It is the one production path to the reply's flags:
   the INIT reply encoder takes only a value it returned (the type
   `Negotiated` holds it and is built only by negotiation) [OWN: the
   issue's requirement that the mount uses this function and no other].
5. **The INIT reply** carries the negotiated minor, the selection (low 32
   bits in `flags`, high in `flags2`), `max_readahead` as the smaller of
   the kernel's and the layer's, and the layer's `max_background`,
   `congestion_threshold`, `max_write` and `time_gran`. `max_pages` is
   zero, since `FUSE_MAX_PAGES` is not selected.
6. **Fields IronAFS never sets** are written as zero: padding, `dummy`,
   unused and spare words, `rdev`, `fuse_attr.flags`, `backing_id`,
   `map_alignment`, `max_stack_depth`, `request_timeout` [OWN].
7. **Cache rules written by the encoder** (L5): every entry and
   attribute timeout is zero, seconds and nanoseconds (L5, point 1); the
   OPEN reply's `open_flags` is exactly `FOPEN_DIRECT_IO` (bit 0) (L5,
   point 2); the OPENDIR reply's is zero, so neither `FOPEN_KEEP_CACHE`
   nor `FOPEN_CACHE_DIR` is ever set (L5, point 3) `[E70 open flags:
   CONFIRMED]`. A later relaxation changes the encoder, with its evidence
   and test.
8. **Attributes** (L3, point 2): the type is a directory, a regular file
   or a symbolic link and nothing else, so no device node can be shown;
   the mode's permission bits are the low twelve with set-uid and set-gid
   always cleared. Times are seconds from 0 to `i64::MAX` and nanoseconds
   below 1,000,000,000; a value outside that cannot be built [OWN: E70
   types the seconds as u64, and no source says how the kernel reads one
   above `i64::MAX`]. STATFS's `namelen` is 255 (5.2). STATX's reply
   follows the same rules: its `mode` carries the same bits, `rdev` and
   `dev` are zero, `attributes` and `attributes_mask` are zero, and its
   mask is `STATX_BASIC_STATS`, with `STATX_BTIME` only when a birth time
   is given (N47, point 6); a time it does not fill is zero.
9. **Errors** are sent as the negative of an `Errno`; `Errno` is a closed
   set of values from E74, none of them `ENOSYS` (N43). A success reply
   has error zero.
10. **Length.** Every reply's `len` is its byte count, by checked
    conversion; no reply exceeds 16 + 1,048,576 bytes.
11. **Only through the encoders.** The session (5.6, point 3) gives each request that
    takes a reply one reply token, `ReplyTo<S>`, whose type parameter names
    the shapes that request allows (an entry for LOOKUP, a handle with
    direct I/O for OPEN, data for READ, and so on), and gives none to a
    request that takes no reply (5.6, point 1). A token cannot be built
    from a request id, cloned or copied, and each encoder consumes it, so
    a request is answered at most once and only in its shapes; every
    token can also answer with an error. READ's token carries the size the
    request asked for, and its data encoder refuses more. An encoder's
    refusal hands the token back, so the layer can still answer with an
    error. Encoders return a `Reply`, which the layer can neither build
    nor read; its bytes come only from the issuing session's `finish`.
    Raw decoding is not public. The compiler shows each of these rules
    (the compile-fail examples of `reply.rs`) [OWN: a layer that could
    write its own bytes, or reuse a request id, could skip L3, L5 and the
    allowlist].

### 5.6 States and ordering

1. **No reply** to FORGET `[E70 "no reply": CONFIRMED]`, to INTERRUPT
   `[E73 FUSE_INTERRUPT: CONFIRMED]`, or to BATCH_FORGET [OWN reading of
   E70: its records are FORGET's, `fuse_forget_one`; section 13, Q3], even
   when the body is malformed: a reply to one would be a reply nobody waits
   for. These requests, and refusals of them, come with no reply token
   (5.5, point 11).
2. **The interrupt** (vfs-contract 5.8, point 7). The codec has no
   encoder for a reply to INTERRUPT, so the layer follows E73: "requires
   no response". E72 describes an `EAGAIN` reply for an interrupt that
   arrives before its target; that cannot happen to a layer that reads the
   device from one thread and records each request before reading the
   next, because the kernel queues an interrupt only after its target was
   transferred to user space `[E72 "Interrupting filesystem operations":
   CONFIRMED]`. An interrupt for a request already answered is ignored,
   which E72 allows. The layer's single reader is M2a-30's to keep [OWN].

3. **The session** is one connection's codec state, and the only public
   decoder. It holds the phase and the ids of the requests owed a reply.
   - **One reply per id.** Decoding a request that takes a reply, or
     refusing one with a reply, records its id. A request whose id is
     already recorded is refused, whatever its opcode, with no token and
     no reply (`Malformed::DuplicateUnique`), since a reply under that id
     would answer the earlier request [OWN: E73, the reply repeats the
     request's `unique`].
   - **One INIT at a time.** While an INIT's token is outstanding, another
     INIT is refused with an error reply (`Malformed::InitPending`,
     `EIO`), which settles nothing; after the first INIT's reply is
     finished, a new INIT is decoded as usual (E73's new INIT after a
     major-only reply) [OWN].
   - **A cap.** At most `MAX_PENDING`, 4,096, ids are recorded. Past it, a
     request that takes a reply is refused with no token and no reply
     (`Malformed::Overloaded`), and the layer ends the connection [OWN:
     above the contract's 1,024-entry request table (vfs-contract 5.8), so
     the table's own `Busy` answers, each needing a token, can always be
     issued; reaching it means replies were lost, a layer fault].
   - **Finishing.** `Session::finish` is the only way to a reply's bytes.
     A reply whose token another session issued, or whose id is not
     recorded, is handed back unwritten (`Unfinished`), and changes
     nothing. Otherwise the id is retired; finishing an INIT reply that
     settled the protocol moves the session to the negotiated minor. A
     reply's `Debug` shows its id and length, never its bytes.
   - **Dropped tokens and replies.** A token or reply dropped without
     `finish` leaves its id recorded until the session ends; the kernel's
     request it answers stays waiting, which is a layer fault either way.
     The cap bounds what such leaks hold [OWN: releasing on drop would
     need shared state between each token and the session].
   - **Cancellation and teardown.** A layer that cancels a request still
     answers it (with `EINTR`, E72) through `finish`; tearing the
     connection down drops the session and every token with it.
   - **Boundary: the session trusts its bytes.** The session cannot tell
     the kernel's bytes from bytes a caller made: it gives a token to any
     well-formed frame whose id is not recorded, and once a reply is
     finished, the same frame decodes again, as the kernel's own reuse of
     an answered id requires. Authority over what reaches the session is
     the transport's: the device descriptor, which only the device reader
     holds (the Linux layer, M2a-30) [OWN].

### 5.7 Errors mapped to Linux

`errno::for_error` maps each contract error by an exhaustive match, with
the numbers of `asm-generic/errno-base.h` and `asm-generic/errno.h`,
which x86-64 and AArch64 both use (neither has its own `errno.h`, and
the generic header list makes one mandatory) `[E74: CONFIRMED]`:

| `VfsError` | Linux | Number |
|---|---|---|
| `NotFound` | `ENOENT` | 2 |
| `Exists` | `EEXIST` | 17 |
| `Access`, `TokenRequired`, `TokenExpired`, `TokenRevoked`, `SecurityFloor` | `EACCES` | 13 |
| `ReadOnly` | `EROFS` | 30 |
| `NoSpace` | `ENOSPC` | 28 |
| `QuotaExceeded` | `EDQUOT` | 122 |
| `CrossDevice` | `EXDEV` | 18 |
| `Stale` | `ESTALE` | 116 |
| `Busy`, `WouldBlock` | `EAGAIN` | 11 |
| `Offline` | `ENETDOWN` | 100 |
| `TimedOut` | `ETIMEDOUT` | 110 |
| `NotYetStored` | `EIO` | 5 |
| `Interrupted` | `EINTR` | 4 |
| `NotSupported` | `EOPNOTSUPP` | 95 |
| `Unreachable` | `EHOSTUNREACH` | 113 |
| `Conflict` | `EBUSY` | 16 |
| `OutOfRange` | `EINVAL` | 22 |

The names are vfs-contract 5.12, point 2's Linux column; the numbers are
E74's. `errno::for_rename_with_flags` is the one per-operation row:
`NotSupported` is `EINVAL` there (vfs-contract 5.6, point 9). The codec
adds `ENAMETOOLONG` (36) for a name over 255 bytes, `EINVAL` for every
other name refusal (vfs-contract 5.12, point 4), and `EPROTO` (71) for
a refused INIT. The contract values `ironafs-vfs` does not hold yet
(`NotADirectory` and the others of vfs-contract 5.12, point 9) are mapped
when they are added; the exhaustive match makes that a compile error
until then.

**The Linux column's re-check** (vfs-contract Q8), in part. Each row's
name was checked against E74's comment on its number, and each comment
describes the condition the contract value names; no row changes. The
re-check against each operation's documented errors (the system-call
pages) is not done here: it needs those pages registered, and stays open
(section 13, Q7) [OWN].

### 5.8 Notifications

Not written yet (section 2, point 2). The kernel's answer to a
notification for an entry it does not cache (vfs-contract 5.10, point 4)
is observed when they are (section 13, Q1).

## 6. Arithmetic

1. `len` against the bytes read: an equality of a u32 widened to `usize`
   and the buffer's length, by `try_from`.
2. Body offsets are constants under 40 bytes; every field is read through
   a checked slice (`get`), never by index.
3. READ: `offset + size` by `checked_add` in u64, then compared with
   `i64::MAX`.
4. BATCH_FORGET: the body after its 8-byte head is split into whole
   16-byte records; a remainder is refused, and the number of records must
   equal `count`, converted by `try_from`, so no product is formed (5.2).
   Each record's two fields are read from its own array, never with a
   default.
5. Replies: each length is a sum of constants and one payload length
   capped at 1 MiB, converted to u32 by `try_from`.
6. The negated error number: positive values below 4,096, negated by
   `checked_neg` (5.5, point 9).

## 7. Errors and failure

1. **Frame error:** no reply; the layer ends the connection.
2. **Request error:** the error 5.4 names, sent under the request's
   `unique`, unless 5.6, point 1, says the opcode takes no reply.
3. **INIT refused:** `EPROTO`; the codec's phase stays "awaiting INIT".
4. **Encoder refusals** (data over 1 MiB, a link target outside its cap,
   a configuration over its cap) are errors returned to the layer, which
   answers `EIO` instead (vfs-contract 5.12, `Protocol`) [OWN].
5. No path panics; the crate inherits the workspace lints that deny
   `unwrap`, `expect`, `panic!` and indexing.

## 8. Security

Any local process that can reach `/afs` chooses the operations the kernel
sends, and through them the names, sizes and offsets; it cannot choose the
header's uid, gid and pid, which the kernel fills `[DES-022]`. A broken or
hostile kernel module is outside the threat model: it owns the machine.

- A request length trusted before it is checked: 5.4, point 1.
- An over-long or path-shaped name: 5.4, point 5, through `Name`.
- A read size that would size an allocation: 5.2; no allocation is
  sized by a READ or READDIR size, and the one copy sized by a decoded
  length is a LOOKUP's name, after its 255-byte cap (5.2).
- A duplicate, stale or wrong-shape reply: 5.5, point 11, and 5.6,
  point 3. Bytes a caller makes are outside what the codec can tell
  apart; the device descriptor is the authority (5.6, point 3,
  "Boundary").
- A cache or privilege flag accepted: 5.5, points 1 to 4; L4 and L5's
  flags are on the never-list or not accepted.
- A set-id bit or device node shown from a cell: 5.5, point 8.
- A permanent refusal: no `Errno` is `ENOSYS` (5.5, point 9).

## 9. How the invariants are kept

| Invariant | Section | Test (`crates/ironafs-fuse`) |
|---|---|---|
| Constraint 4 | 5.2, 5.4, 6 | `request::tests` reject cases; `tests/it/decode.rs` properties `decode_never_panics_on_any_bytes`, `decode_reports_framed_requests_faithfully` |
| L2, point 2 | 5.3 | `init::tests::negotiate_refuses_minor_below_floor`, `negotiate_takes_the_smaller_minor` |
| L2, point 3 | 5.3, point 3; 5.5, point 4 | `init::tests::select_*` |
| L2, point 4; N43 | 5.4, point 9; 5.5, point 9 | `errno::tests::no_errno_is_enosys`, `errno::tests::every_value_maps_to_an_error` |
| L2, point 6; L4, points 1 to 3 | 5.5, points 1 and 2 | `init::tests::never_list_flags_are_never_selected`, the compile-time disjointness check |
| L3, point 2 | 5.5, point 8 | `reply::tests::attr_clears_set_id_bits`, `statx_clears_set_id_bits`, the `Kind` type |
| L2, point 4; N47, point 6 | 5.4, point 10 | `request::tests::statx_decodes`, `reply::tests::statx_reply_layout_without_birth_time`, `statx_reply_reports_a_birth_time_it_has` |
| L2, point 3 (flags checked against their minor) | 5.3, point 2 | `init::tests::decode_init_refuses_flags_before_their_minor`, `decode_init_refuses_bodies_of_other_sizes`, `decode_init_other_major_reads_only_the_major`, `negotiated_flags_are_defined_at_the_negotiated_minor` |
| L3, L5 and the allowlist reached only through the encoders | 5.5, point 11 | the compile-fail examples of `reply.rs`; `reply::tests::data_over_the_request_size_refused`; `request::tests::refused_requests_reply_only_when_owed` |
| One reply per pending request, on its own connection | 5.6, point 3 | `session::tests::a_frame_decoded_twice_is_refused_the_second_time`, `a_duplicate_of_a_refused_request_is_refused_too`, `a_no_reply_request_with_a_pending_id_is_refused`, `requests_without_a_reply_take_no_id`, `finish_refuses_a_reply_of_another_session`, `a_wrong_session_finish_hands_the_reply_back`, `the_phase_moves_only_when_a_settled_init_is_finished`, `a_second_init_while_one_is_pending_is_refused`, `a_new_init_may_follow_a_finished_major_only_reply`, `decode_refuses_past_the_pending_cap`; `reply::tests::reply_debug_shows_no_bytes` |
| INIT answered from its own offer, in its major's shape | 5.3, point 1 | `session::tests::init_answers_in_the_shape_its_major_allows`; `reply::tests::init_major_only_reply_for_a_newer_major`, `init_refusal_is_eproto` |
| N43 for an opcode above the minor | 5.4, point 11 | `session::tests::statx_needs_7_39` |
| L5, points 1 to 3 | 5.5, point 7 | `reply::tests::entry_and_attr_timeouts_are_zero`, `open_reply_is_direct_io_only`, `opendir_reply_sets_no_cache_flag` |
| vfs-contract 5.6, point 9 | 5.7 | `errno::tests::rename_flags_is_einval` |

## 10. Tests

### 10.1 Golden vectors

Each vector is written by hand from section 5.1's tables, byte by byte,
in the test that uses it (`crates/ironafs-fuse/src/*/tests.rs`), with
IronAFS's own values: a LOOKUP of `b"cell"` under node 1, a GETATTR with
and without `FUSE_GETATTR_FH`, a READ of 4,096 bytes at offset 8,192, a
BATCH_FORGET of two records, an INIT at 7.23 (16 bytes) and at 7.46 (64
bytes with `flags2`), a STATX with and without `FUSE_GETATTR_FH`, and
one reply of each kind in 5.1, point 5, STATX's with and without a birth
time.

### 10.2 Round-trip properties

The codec decodes requests and encodes replies, which are different
messages, so there is no round trip. Its properties instead
(`tests/it/decode.rs`): for every byte string `Session::decode` returns a value or
an error and never panics (`decode_never_panics_on_any_bytes`); a
request with a well-formed header and a random body is decoded with that
header, or refused with it, and a decoded LOOKUP's name is the body
without its NUL (`decode_reports_framed_requests_faithfully`); the
selection for every 64-bit offer is `offer & ALLOWED`
(`init::tests::select_is_offer_and_allowed`).

### 10.3 Reject corpus

In `src/request/tests.rs`: fewer than 40 bytes; `len` one more and one
less than the bytes read; a GETATTR before INIT; a second INIT; a
`total_extlen` of 1 at minor 38 (and accepted at 37); each fixed body one
byte short and one long; LOOKUP without its NUL, with a byte after it,
with a `/`, with 256 bytes, of `.` and `..`; unknown bits in each flags
field; a READ of 1,048,577 bytes, at offset `i64::MAX`, and with `offset +
size` over `i64::MAX`; a BATCH_FORGET whose count does not match, and one
whose count is `u32::MAX`; a STATX body one byte short and one long; an
OPEN or OPENDIR with a bit in its second word at 7.33 (and ignored at
7.32). In `src/session/tests.rs`: a frame decoded twice, and a refused one
decoded twice; a FORGET and an INTERRUPT with a pending id; a second
INIT while the first is unanswered; the 4,097th pending request; a reply
finished on another session; STATX at 7.38. In `src/init/tests.rs`: a major-7 INIT of 15 or 17 bytes without
`FUSE_INIT_EXT`, of 16, 20, 63 or 65 bytes with it, and of 64 without it;
`FUSE_INIT_EXT` offered at 7.35 and `FUSE_NO_EXPORT_SUPPORT` at 7.39; a
newer major's body of 4 bytes, read for its major alone, and of 3 bytes,
refused.

### 10.4 Fuzz target

`fuse_codec`, in the top-level `fuzz/` crate: it decodes the input at
each phase and asserts no panic, and that a decoded LOOKUP's name is
the input's bytes before its NUL. It is added when the fuzz crate lands (issue
#45); until then the property of 10.2 is the only random-input check
(section 13, Q5).

### 10.5 Acceptance cases

None in the codec; the mount's are M2a-30's.

## 11. Revisit if

- L2: "assumption A1 fails or the RHEL 7.9 candidate row lapses (the floor
  may then rise to the oldest remaining target's level), or a target kernel
  lacks a notification this part depends on."
- L5: "the acceptance suite shows a relaxed setting keeps two-user
  isolation and callback freshness on every tier 1 kernel; each setting is
  relaxed separately, with its test."
- N43: "the kernel's handling of `ENOSYS` for these requests changes so
  that it is no longer permanent for the mount."
- A kernel above 7.46 is a tier 1 target: `MAX_MINOR` rises only after its
  header is registered and read for layout changes in what the codec
  decodes.
- A big-endian Linux target is added (5.1, point 1).

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| E70 | DOCUMENTATION | Linux `include/uapi/linux/fuse.h` at v7.3 (protocol 7.46); register row E70 | 5.1, 5.3, 5.4, 5.5, 5.6 |
| E71 | DOCUMENTATION | Linux `include/uapi/linux/fuse.h` at v3.15 (protocol 7.23); register row E71 | 5.1 |
| E72 | DOCUMENTATION | Linux `Documentation/filesystems/fuse/fuse.rst` and `fuse-io.rst` at v7.3; register row E72 | 5.4, 5.5, 5.6 |
| E73 | DOCUMENTATION | fuse(4), Linux man-pages 6.19, 2026-02-08; register row E73 | 5.1, 5.3, 5.4, 5.6 |
| E74 | DOCUMENTATION | Linux `include/uapi/linux/limits.h`, `stat.h`, `asm-generic/errno-base.h`, `asm-generic/errno.h` and `asm-generic/Kbuild` at v7.3; register row E74 | 5.1, 5.2, 5.7 |
| ADR 0004 | DECISION | L2 to L5, N43, constraint 4; the claim record's DES-022 and DES-026 | 1, 5, 8 |
| vfs-contract | DECISION | `vfs-contract.md` (issue #67): 5.2, 5.6, 5.8, 5.10, 5.12 | 1, 5.2, 5.6, 5.7 |
| OWN | OWN | IronAFS's choices, each with its reason where it is stated | throughout |

No FUSE library's source and no part of the kernel's FUSE implementation
was read; the uapi headers are read as interface definitions, the way the
kernel publishes them to user space.

### 12.1 Observed behaviour

None yet. Section 13 lists what is to be observed.

## 13. Open questions

- **Q1. The rest of the codec.** Each opcode a kernel may send at the
  negotiated minor must be decoded and answered, or refused per operation,
  before the Linux layer ships (5.4, point 10); STATX is done. READDIR's reply records need the
  directory-entry type values, which no registered source gives yet (the
  uapi headers do not define them); the notifications, the extended
  attribute, sync and seek requests, the requests that change a volume, and
  the reserved operations follow. They block the Linux layer's read path
  (M2a-30), not this part. The kernel's answer to a notification for an
  uncached entry (vfs-contract 5.10, point 4) is observed with the
  notification encoders.
- **Q2. Export below 7.40.** On a kernel that does not offer
  `FUSE_NO_EXPORT_SUPPORT`, a process allowed to call `open_by_handle_at`
  may reach a node without a LOOKUP. Whether the kernel offers export on a
  FUSE mount without `FUSE_EXPORT_SUPPORT` is to be observed on the RHEL
  7.9 and 8 rows (M2a-30); until then it is a stated residual for those
  rows, limited to processes privileged enough to call it.
- **Q3. BATCH_FORGET takes no reply** by this specification's reading of
  E70; no registered source says so. To be observed at the first mount
  (M2a-30): a reply written to it, and the device's answer.
- **Q4. A refused INIT.** No registered source says what the kernel does
  with an error reply to INIT. The layer aborts the connection after
  sending it (M2a-30), so the outcome does not depend on the answer.
- **Q5. Fuzzing.** The fuzz target waits on the fuzz crate (issue #45).
- **Q6. Register rows.** E73 and E72 overlap rows E62 and E63 of
  vfs-contract's register entries; when both land, the duplicates are
  merged into one row each.
- **Q7. The per-operation error check** of vfs-contract Q8: each Linux
  row against the documented errors of the operations M2a serves
  (`stat`, `open`, `read`, `readlink`, `access`), once those pages are
  registered. It can change a row of 5.7, not the codec's structure.

## 14. Review

No review record yet. It will be in `docs/reviews/` when the specification
is reviewed.
