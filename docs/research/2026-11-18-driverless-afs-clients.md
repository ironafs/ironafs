<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Driverless AFS clients: research notes (2026-11-18)

**How this note was made.**

- Web research. Most claims are INSPECTED from the cited sources
  (`docs/workflow.md` §3c); nothing was executed.
- Most FSKit facts come from Apple developer forum posts, release notes and
  third-party SDK listings, not Apple's own documentation. Check the SDK
  headers before relying on any of them.
- Three items came from reading OpenAFS source, and are marked "(from
  source)". Under ADR 0001 they are background only, never an input for
  writing IronAFS code:
  - how OpenAFS clients treat byte-range locks;
  - `libuafs`'s single lock;
  - the line counts in §5.

  Everything else is from public documentation and discussion, which ADR 0001
  allows as input.
- The note has not yet been cross-checked by a second model family. Claims
  are re-checked before a decision rests on them.

## 1. Why now

- **macOS:** third-party kernel extensions need reduced security on Apple
  silicon; Apple's replacement for filesystem kexts is FSKit (macOS 15.4+, URL
  resources from macOS 26, handler APIs in macOS 27).
- **Linux:** new kernel interfaces are increasingly GPL-only, which is the
  stated reason OpenAFS is relicensing. FUSE has gained writeback caching (3.15),
  passthrough (6.9) and io_uring transport (6.14).
- **Windows:** the Cloud Files API (Windows 10 1709+) gives placeholder-based,
  on-demand files without a driver of our own.

All three platforms now offer a supported user-space filesystem interface, so
one design can avoid kernel code everywhere.

## 2. macOS options

| Option | Gives `/afs`? | Notes |
|---|---|---|
| File Provider | No: `~/Library/CloudStorage/<provider>` only | Sync model like Cloud Files; per-user; good Finder companion, not a full client |
| FSKit | Unconfirmed at `/afs`; to be tested in the spike | Apple's VFS replacement; gaps below |
| Localhost NFSv4 server | Yes | Kernel NFS client sends the caller uid; per-user access cache; NFSv4 locks reach the server; delegation support in the macOS client unverified |

### FSKit gaps and their status (macOS 27 SDK)

| Gap | Status |
|---|---|
| xattrs | `FSVolume.XattrHandler` |
| Kernel access checks | `FSVolume.AccessCheckHandler` |
| Caller identity | `FSContext` with uid/gid; no pid |
| Cache invalidation (AFS callback breaks) | `DataCacheHandler` + `setCacheState(... .invalidate/.revoke)`; reportedly purges a directory's name cache |
| Inode eviction | `reclaimItem` |
| ioctl (pioctl) | None; Apple DTS suggests XPC or a Unix socket |
| Byte-range / advisory locks | None; stay kernel-local |
| ACLs | Not supported as macOS ACLs |

Open risks: cross-uid kernel cache isolation (one report says FSKit cannot
guarantee it), no PAGs (uid only), negative-lookup purge, mounting at `/afs`,
and FSKit maturity: macFUSE's release notes document a workaround for a
data-corruption issue, and an `fskitd` Mach port leak has been reported,
reproduced with FSKit alone, with no workaround (macOS 26/27).

Locks remain an open question for the spike. OpenAFS's Unix clients keep
byte-range locks local to the machine (from source), while OpenAFS documents a
Windows client mode that uses whole-file server locks to make byte-range
locking visible across clients
([release notes](https://www.openafs.org/dl/openafs/1.7.11/winxp/ReleaseNotes/html/ch03s19.html)).
FSKit offers neither byte-range nor advisory locks. Whole-file and byte-range
compatibility both stay open until the spike measures them.

## 3. Linux

- FUSE is in the kernel since 2.6.14; libfuse 3 since 2016. The pure-Rust
  `fuser` crate speaks `/dev/fuse` directly.
- Rust's standard library needs kernel 3.2+ and glibc 2.17+: roughly the
  RHEL 7 floor. Older systems keep the OpenAFS kernel module.
- FUSE passes uid, gid and pid with most requests, so per-process tokens are
  possible (unlike FSKit). Not with writeback caching: delayed writes reach
  the daemon without a valid caller identity, possibly on a different file
  handle. Authorisation through deferred I/O is a design question for the
  architecture ADR, and writeback stays off until it is answered.

## 4. Why FUSE never caught on for OpenAFS

- `afsd.fuse` (OpenAFS 1.5.74, 2010) exists over `libuafs`, the cache manager
  built for user space. Documented as experimental; once packaged as read-only,
  without init scripts.
- `libuafs` serialises everything under a single global lock (from source).
- 2005: FUSE passed uid/gid but not the group list, where Linux PAGs lived.
- 2007, openafs-info: a poster noted that a FUSE-style AFS client already
  existed in Arla, the independent BSD user-space AFS client with its own
  (nnpfs) kernel module.
- FUSE was slow then, did not help macOS (MacFUSE was itself a kext) or Windows,
  and the kernel module already worked.

## 5. Size of what exists (OpenAFS tree, approximate lines of C)

From source. Counted from an OpenAFS checkout whose revision was not
recorded; treat the figures as orders of magnitude.

| Part | Lines |
|---|---|
| Unix cache manager | ~49k (+ ~10k vnode operations, ~5k macOS glue) |
| Windows cache manager | ~47k |
| Rx | ~29k |
| rxkad / rxgk | ~16k / ~3.5k |

## 6. Language

Rust: memory safety for a client that parses fileserver data and serves
callbacks; data-race checking for a concurrent cache manager; one cargo build
for all three OSes; bindings exist (`windows-rs` CloudFilters, `objc2-fs-kit`,
`fuser`, tokio, RustCrypto). Costs: learning curve, less mature Kerberos/GSS bindings, and fewer AFS
developers who already work in Rust. (An FFI boundary to a C Rx is no longer a
cost: ADR 0001 has Rx written in Rust.)

## 7. Licences of the existing implementations

- **OpenAFS** is under the IBM Public License 1.0 (IPL-1.0) and is moving to a
  dual licence: distribution under either IPL-1.0 or GPLv2. The reasons:
  - newer Linux kernel interfaces are often limited to GPL code;
  - code derived from Sun RPC was under a Sun licence incompatible with GPLv2
    (now available under the 3-clause BSD Oracle Sun RPC licence).
- IBM announced on 2025-02-05 that OpenAFS 1.0 is distributed under IPL/GPLv2.
  The project says contributors are being contacted to agree to the change
  for later contributions. This note's reading: until that is done, much of
  the current tree is likely still IPL-1.0 only.
- **OpenAFS's Rx**, in its user-space libraries and its kernel module, is
  part of OpenAFS and under the same terms.
- **The Linux kernel's AF_RXRPC and kafs** are a separate implementation of
  Rx and AFS written for the kernel, under GPLv2. They are not IPL.
- **What this means for IronAFS** (ADR 0001): IronAFS is GPL-3.0-or-later and
  uses no code from any of these.
  - Moving code between them and IronAFS is closed both ways: GPLv3 is
    incompatible with GPLv2-only code (the kernel's) and with IPL-1.0
    (OpenAFS's). That holds whichever way OpenAFS's relicensing ends.
  - Separately, AuriStor is proprietary: before running its software or
    capturing traffic from its cells, the applicable terms are checked
    (ADR 0001, test tools).

## 8. Protocol documentation

What IronAFS can implement from, under ADR 0001. Each document has its own
licence: facts are taken from it, not its text.

- **The community Rx specification draft**, which describes Rx packets,
  sequencing and acknowledgements. It was originally written by Nickolai
  Zeldovich and later updated by Jeffrey Altman.
- **The rxgk Internet-Drafts** (expired, historical), which describe the
  GSS-API-based security class, its key negotiation and its packet types.
- **"Integrating rxgk with AFS"** (an expired draft), which describes how rxgk
  plugs into AFS services.
- **The Linux kernel's AF_RXRPC documentation**, which describes the protocol
  as the kernel exposes it. The documentation, not the kernel code, is the
  source.
- **Observed behaviour:** traffic captured against a test cell, reduced to
  synthetic fixtures (ADR 0001).

## Sources

- Apple developer forums: [FUSE compat surface plans?](https://developer.apple.com/forums/thread/831338), [Porting VFS kext to FSKit](https://developer.apple.com/forums/thread/765424), [FSKit tag](https://developer.apple.com/forums/tags/fskit)
- [What's new in FSKit, macOS 27 SDK](https://miniswift.run/whats-new/fskit/)
- [macFUSE](https://macfuse.github.io), [macFUSE 5.4.0](https://macfuse.github.io/2026/09/07/macfuse-5.4.0.html), [macFUSE #1220](https://github.com/macfuse/macfuse/issues/1220)
- [vaulthalla #200 (cross-uid cache isolation)](https://github.com/vaulthalla/vaulthalla/issues/200), [JuiceFS FSKit RFC](https://github.com/juicedata/juicefs/issues/7814)
- [Using FUSE for OpenAFS? (2007)](https://openafs-info.openafs.narkive.com/E8NU7V5g/using-fuse-for-openafs), [Using FUSE for AFS/DFS (2005)](https://linux-kernel.vger.kernel.narkive.com/GytY698E/using-fuse-for-afs-dfs-was-re-openafs-devel-openafs-opendfs-collaboration), [afsd.fuse man page](https://manpages.debian.org/stretch/openafs-fuse/afsd.fuse.8.en.html)
- [Relicensing OpenAFS](https://www.openafs.org/pages/relicense/relicensing-openafs.html)
- Protocol documentation: [Rx specification draft](https://registrar.central.org/rx-spec.txt), [rxgk draft 09](https://datatracker.ietf.org/doc/html/draft-wilkinson-afs3-rxgk-09), [Integrating rxgk with AFS](https://datatracker.ietf.org/doc/draft-wilkinson-afs3-rxgk-afs/08/), [Linux AF_RXRPC documentation](https://docs.kernel.org/networking/rxrpc.html)
