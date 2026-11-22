<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Driverless AFS clients: research notes (2026-11-18)

**How this note was made.**

- Web research, checked claim by claim against primary sources (see
  "Checked" at the end). Nothing was executed for this note; its one
  MEASURED Windows claim is carried over from the
  [Windows UNC note](2026-11-19-windows-unc-without-a-driver.md), which
  describes how it was measured.
- Each claim carries an evidence label (`docs/workflow.md` §3c and the
  [research index](README.md)):
  - **INSPECTED:** read in the primary source linked with it: vendor
    documentation, SDK headers, man pages, specifications, release notes or
    licence texts;
  - **RELAYED:** read only in a secondary source: a forum post (including
    replies from Apple's Developer Technical Support, DTS), an issue on a
    third-party tracker, a mailing-list post, or a third-party summary;
  - **INFERRED:** a conclusion drawn from labelled facts;
  - **NOT VERIFIED:** no source found either way;
  - **MEASURED:** carried over from another note, as above.

  "One family" means that only one of the model families that checked the
  note reached a verdict on that claim.
- Nothing in this note comes from reading another AFS implementation's
  source. Two earlier items that did (a locking claim and line counts)
  were removed on 2026-11-19, because a note is sent to review lanes and
  ADR 0001 keeps source-derived material out of them. Everything here is
  from public documentation and discussion, which ADR 0001 allows as input.

## 1. Why now

- **macOS:**
  - third-party kernel extensions need Reduced Security on Apple silicon
    (INSPECTED, [Apple Platform Security](https://support.apple.com/guide/security/securely-extending-the-kernel-sec8e454101b/web));
  - FSKit is Apple's user-space alternative to filesystem kernel extensions,
    delivered as app extensions: from macOS 15.4, with URL resources from
    macOS 26 and the handler APIs from macOS 27 (INSPECTED,
    [macOS 15.4 release notes](https://developer.apple.com/documentation/macos-release-notes/macos-15_4-release-notes),
    [FSGenericURLResource](https://developer.apple.com/documentation/fskit/fsgenericurlresource),
    [FSVolume.Handler](https://developer.apple.com/documentation/fskit/fsvolume/handler)).
    Apple does not present it as a replacement for every VFS operation; §2
    lists the gaps.
- **Linux:**
  - new kernel interfaces are often limited to GPL code, which is a stated
    reason OpenAFS is relicensing (INSPECTED,
    [Relicensing OpenAFS](https://www.openafs.org/pages/relicense/relicensing-openafs.html));
  - FUSE has gained writeback caching (3.15), passthrough (6.9) and an
    io_uring transport (6.14) (INSPECTED,
    [3.15 pull request](https://lkml.rescloud.iu.edu/hypermail/linux/kernel/1404.0/01763.html),
    [passthrough](https://docs.kernel.org/filesystems/fuse/fuse-passthrough.html),
    [io_uring](https://docs.kernel.org/6.14/filesystems/fuse-io-uring.html)).
    Passthrough needs privilege, and io_uring does not carry every request
    type: notifications and interrupts still use `/dev/fuse`.
- **Windows:** the Cloud Files API (Windows 10 1709+) gives placeholder-based,
  on-demand files without a driver of IronAFS's own. The work is done by
  Microsoft's minifilter `cldflt.sys`, which supports NTFS volumes only; the
  provider is a user-mode desktop application (INSPECTED,
  [Build a cloud sync engine](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine)).
  The other driverless route is an online `\\afs` path: Windows' own SMB
  client talking to an SMB 3.1.1 server of IronAFS's own on the loopback
  address, which keeps every request in an authenticated session. On
  Windows 10 and 11, the client's traffic reached a probe listener on that
  path beside Windows' own file sharing; a full SMB session through it is
  not yet measured (MEASURED,
  [Windows UNC note](2026-11-19-windows-unc-without-a-driver.md)).

All of these let IronAFS ship no kernel code of its own. Only FSKit and FUSE are
filesystem interfaces, though. Cloud Files is a sync-engine API: hydrated
files are ordinary NTFS files that are served without calling the provider,
and its callbacks cover placeholders only (INSPECTED,
[Build a cloud sync engine](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine),
[CF_CALLBACK_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type)).
What one design can share is the AFS core. Authorisation, writing and cache
coherency need their own design on Windows
([Windows Cloud Files note](2026-11-19-windows-cloud-files.md)).

## 2. macOS options

| Option | Gives `/afs`? | Notes | Evidence |
|---|---|---|---|
| File Provider | No. No API places a domain at a mount point such as `/afs`. Domains live under `~/Library/CloudStorage/` by default; since macOS 15.0 the domain initializer takes a volume URL, for an "eligible" volume ("external volume" is this note's wording, not Apple's). | Sync model like Cloud Files; per-user; a good Finder companion, not a full client | INSPECTED: [external-volume domains](https://developer.apple.com/documentation/fileprovider/nsfileproviderdomain/init(displayname:userinfo:volumeurl:)), [File Provider](https://developer.apple.com/documentation/fileprovider) (per-user: one family). The default path is RELAYED. |
| FSKit | Not through the programmatic API: `FSClient.mountSingleVolume` mounts under `/Volumes/` and needs the `com.apple.developer.fskit.mount` entitlement. Apple documents `mount(8)` to an arbitrary existing directory for a URL-backed module, and `synthetic.conf` can create `/afs`. That combination is untested; the spike tests it. | Apple's user-space filesystem interface; gaps below | INSPECTED: [passthrough sample](https://developer.apple.com/documentation/fskit/building-a-passthrough-file-system), [mountSingleVolume](https://developer.apple.com/documentation/fskit/fsclient/mountsinglevolume(resource:bundleid:options:completionhandler:)), [`synthetic.conf(5)`](https://man.freebsd.org/cgi/man.cgi?apropos=0&manpath=macOS+26.7&query=synthetic.conf&sektion=5) |
| Localhost NFSv4 server | Documented mechanism: `synthetic.conf` creates `/afs`, and `mount_nfs` takes an explicit mount path and NFS version. A loopback IronAFS server is untested. | With `sec=sys` the kernel client sends the caller's effective uid, gid and a group list: a host assertion, not proof. An `access(2)` cache holds access results for up to three users by default. NFSv4 specifies server-side `LOCK`, but a held delegation can let the client handle locks locally. The `nocallback` option implies the client supports delegations; untested. | INSPECTED: [`mount_nfs(8)`](https://man.freebsd.org/cgi/man.cgi?apropos=0&manpath=macOS+26.7&query=mount_nfs&sektion=8), [RFC 5531 appendix A](https://datatracker.ietf.org/doc/html/rfc5531#appendix-A), [RFC 7530 §16.10](https://datatracker.ietf.org/doc/html/rfc7530#section-16.10) (the `/afs` mechanism and locks: one family) |

The man-page links are a public mirror of Apple's macOS man pages.

### FSKit gaps and their status (macOS 27 SDK)

All INSPECTED in Apple's documentation and the macOS 27.2 SDK headers unless
marked.

| Gap | Status | Source |
|---|---|---|
| xattrs | Available since macOS 15.4 through `FSVolume.XattrOperations`; macOS 27 replaces it with `FSVolume.XattrHandler`, which adds the caller context | [XattrHandler](https://developer.apple.com/documentation/fskit/fsvolume/xattrhandler) |
| Kernel access checks | `FSVolume.AccessCheckOperations` since 15.4, without caller identity; `FSVolume.AccessCheckHandler` (27) adds it. The contract does not promise a fresh decision on every read or cache hit. | [AccessCheckHandler](https://developer.apple.com/documentation/fskit/fsvolume/accesscheckhandler) |
| Caller identity | `FSContext`, macOS 27 only: real and effective uid and gid. No pid, no supplementary groups, no audit session. `FSContext` exists only from macOS 27.0. Earlier FSKit operations take no caller context (INFERRED from the API; Apple does not say so). | [FSContext](https://developer.apple.com/documentation/fskit/fscontext) |
| Cache invalidation (AFS callback breaks) | `DataCacheHandler` and `setCacheState(...)` (macOS 27). Invalidating a directory purges its name cache; Apple does not say whether negative entries go too. `.invalidate` discards dirty data, `.pushInvalidate` flushes it first, `.revoke` reclaims the vnode. The setter can fail, can call back into the module, and must be called without the module's own locks held. | [invalidate](https://developer.apple.com/documentation/fskit/fsvolume/kernelcachecoherencyaction/invalidate), [setCacheState](https://developer.apple.com/documentation/fskit/fsvolume/setcachestate(for:cachemode:coherencytype:action:)) |
| Inode eviction | `reclaimItem`, since 15.4 in `Operations` and carried into `Handler` | [reclaimItem](https://developer.apple.com/documentation/fskit/fsvolume/commonoperations/reclaimitem(_:replyhandler:)) |
| ioctl (pioctl) | None. Apple DTS suggests XPC or a Unix socket (RELAYED). | [forum thread 831338](https://developer.apple.com/forums/thread/831338) |
| Byte-range / advisory locks | No lock operations in the headers, only `supportsOpenDenyModes`, so locks stay kernel-local. A forum poster's Feedback report, not DTS, says `flock(2)` locks are not delegated either (RELAYED). | [FSVolume](https://developer.apple.com/documentation/fskit/fsvolume) |
| ACLs | No ACL API in the public `FSItem` and `FSVolume` interfaces (by absence). AFS authorisation can still be enforced through the access-check hook. | SDK headers |

Open risks:

- **Cross-uid kernel cache isolation:** an untested hypothesis. A Vaulthalla
  maintainer asserted that FSKit on macOS 27 "can't guarantee cross-UID
  cache isolation", citing macFUSE issue #1207, which is about entry and
  attribute timeouts and does not discuss it (RELAYED,
  [vaulthalla #200](https://github.com/vaulthalla/vaulthalla/issues/200),
  [macFUSE #1207](https://github.com/macfuse/macfuse/issues/1207)).
- **No PAGs:** `FSContext` gives uids and gids only (above).
- **Negative-lookup purge:** not documented either way; JuiceFS saw stale
  lookups on macOS 26.6.2, with 27 untested (RELAYED, one family,
  [JuiceFS #7814](https://github.com/juicedata/juicefs/issues/7814)).
- **Mounting at `/afs`** (above).
- **FSKit maturity:**
  - macFUSE 5.5.0 (2026-10-17) mitigates "a data corruption issue in FSKit
    on macOS 26 and 27" by rejecting empty `FUSE_READ` replies with `EIO`.
    The workaround can cause read errors when a file's size changes remotely
    (INSPECTED,
    [macFUSE 5.5.0](https://macfuse.github.io/2026/10/17/macfuse-5.5.0.html)).
  - One report of an `fskitd` Mach port leak on macOS 27.0.1: about eight
    send rights per mount, kept after unmount, reproduced with FSKit alone
    and filed with Apple as FB25230024. It is open, with no known workaround
    (RELAYED,
    [macFUSE #1220](https://github.com/macfuse/macfuse/issues/1220)).

Locks remain an open question for the spike.

- The AFS protocol's file locks are whole-file advisory locks held at the
  fileserver (INSPECTED, one family,
  [AFS file locking draft](https://datatracker.ietf.org/doc/html/draft-mbenjamin-afs-file-locking-06)).
  IBM documented that its AFS client ignored byte-range locks (INSPECTED,
  one family,
  [IBM support](https://www.ibm.com/support/pages/what-does-error-message-mean-afs-byte-range-lockunlock-ignored-make-sure-no-one-else-running-program)),
  and an OpenAFS developer wrote that whole-file `fcntl` locks "are fully
  supported" (RELAYED, one family,
  [openafs-info, 2005](https://lists.openafs.org/pipermail/openafs-info/2005-March/017012.html)).
- OpenAFS for Windows 1.5 and later implements byte-range locks inside its
  CIFS-AFS gateway and takes a whole-file advisory lock at the fileserver,
  so other machines see the whole file locked. This is the client's
  behaviour, not a mode: the `EnableServerLocks` registry value only stops
  it taking server locks, and read-only volumes and users without the lock
  (`k`) right bypass server locking (INSPECTED,
  [release notes §3.19](https://docs.openafs.org/ReleaseNotesWindows/ch03s19.html)).
- FSKit forwards no locks (above), and Cloud Files has no lock callback
  (§9). Whole-file and byte-range compatibility both stay open until the
  spike measures them.

## 3. Linux

- FUSE is in the kernel since 2.6.14 (INSPECTED,
  [2.6.14 merge](https://lkml.rescloud.iu.edu/hypermail/linux/kernel/0509.1/1945.html)).
  libfuse 3.0.0 was released on 2016-12-08 (INSPECTED,
  [libfuse ChangeLog](https://github.com/libfuse/libfuse/blob/master/ChangeLog.rst)).
- The `fuser` crate implements the FUSE protocol in Rust and does its own I/O
  on the `/dev/fuse` descriptor. It calls libfuse only to mount and unmount;
  on Linux that is optional, and only a build without the `libfuse` feature
  is pure Rust (INSPECTED, [fuser](https://docs.rs/fuser/latest/fuser/),
  [crate page](https://docs.rs/crate/fuser/latest)). Version 0.18.0's
  manifest declares `default = []`, so the `libfuse` feature is opt-in and
  a default build is pure Rust (INSPECTED,
  [Cargo.toml at v0.18.0](https://raw.githubusercontent.com/cberner/fuser/v0.18.0/Cargo.toml));
  how the mount is then performed without libfuse is a separate question,
  not checked here. IronAFS writes
  its own `/dev/fuse` layer (coding standards rule 21), so `fuser` is prior
  art, not a dependency.
- Rust's standard library needs kernel 3.2+ and glibc 2.17+ on
  `x86_64-unknown-linux-gnu`, and kernel 4.1+ on `aarch64-unknown-linux-gnu`
  (INSPECTED,
  [platform support](https://doc.rust-lang.org/rustc/platform-support.html)).
  RHEL 7 ships kernel 3.10 and glibc 2.17, so it meets the x86_64 floor
  (INSPECTED,
  [RHEL 7.0 release notes](https://docs.redhat.com/en/documentation/red_hat_enterprise_linux/7/html/7.0_release_notes/chap-red_hat_enterprise_linux-7.0_release_notes-kernel)).
  Older systems keep the OpenAFS kernel module (NOT VERIFIED).
- FUSE passes the caller's uid, gid and pid with most requests (INSPECTED,
  [fuse_ctx](https://libfuse.github.io/doxygen/structfuse__ctx.html),
  [fuse_lowlevel.h](https://libfuse.github.io/doxygen/fuse__lowlevel_8h.html)),
  so per-process tokens are possible (unlike FSKit). Limits:
  - the pid is the calling thread's id, and binding it to a token must
    survive pid reuse;
  - there is no group list: libfuse's `fuse_req_getgroups` reads it from
    `/proc`, which can race;
  - the ids can be zero when the caller is in another namespace.
- Not with writeback caching: delayed writes reach the daemon without a valid
  caller identity, and "the fh value may not match" the handle of the
  original write (INSPECTED,
  [fuse_file_info](https://libfuse.github.io/doxygen/structfuse__file__info.html)).
  Authorisation through deferred I/O is a design question for the
  architecture ADR, and writeback stays off until it is answered.
  (Update, 2026-11-21: answered by ADR 0004, L4: the FUSE writeback cache is
  never enabled.)

## 4. Earlier FUSE work for AFS

- `afsd.fuse` arrived in OpenAFS 1.5.74, released on 2010-04-22 (INSPECTED,
  [afsd](https://docs.openafs.org/Reference/8/afsd.html)). It runs over
  `libuafs`, the cache manager built for user space (INSPECTED, one family,
  [2010 workshop talk](https://workshop.openafs.org/afsbpw10/talks/thu_2/201005_adeason_libuafs.pdf)).
  Debian still packages it as experimental, read-only and without init
  scripts (INSPECTED,
  [openafs-fuse](https://packages.debian.org/sid/openafs-fuse)).
- 2005: FUSE passed the caller's uid and gid but not the group list (RELAYED,
  [linux-kernel archive](https://linux-kernel.vger.kernel.narkive.com/GytY698E/using-fuse-for-afs-dfs-was-re-openafs-devel-openafs-opendfs-collaboration)).
  OpenAFS tracks PAGs through artificial groups "on some platforms"
  (INSPECTED, [Administration Guide](https://docs.openafs.org/AdminGuide/HDRWQ29.html));
  the 2005 thread does not mention PAGs, and that Linux PAGs lived in the
  group list then is NOT VERIFIED.
- 2007, openafs-info: one poster pointed to Arla as an existing FUSE-style
  AFS client, and another replied that Arla uses its own nnpfs kernel module,
  not FUSE (RELAYED,
  [openafs-info archive](https://openafs-info.openafs.narkive.com/E8NU7V5g/using-fuse-for-openafs)).
  Arla is an independent user-space AFS client that ran on the BSDs, Linux,
  Solaris and Darwin, with its own kernel module. Its manual calls the
  module `xfs` (INSPECTED,
  [Arla manual](https://www.stacken.kth.se/project/arla/html/arla.html)),
  and later posts call it `nnpfs`; no primary source states the rename
  (RELAYED).
- MacFUSE was then a kernel extension; macFUSE still ships a kernel backend
  beside its FSKit backend (INSPECTED, [macFUSE](https://macfuse.github.io)).
- This note's reading of the rest, NOT VERIFIED: FUSE was slow then, it did
  not help Windows, and the kernel module already worked.

## 5. Size of what exists (removed)

This section gave approximate line counts from another implementation's
source tree. It was removed on 2026-11-19 (ADR 0001; see the preamble).

## 6. Language

- **Rust gives** memory safety for a client that parses fileserver data and
  serves callbacks, and freedom from data races for a concurrent cache
  manager, in safe Rust; `unsafe` code and logical races are outside that
  guarantee (INSPECTED, one family,
  [Rustonomicon](https://doc.rust-lang.org/nomicon/races.html)).
- **One cargo workspace** builds for all three OSes. Each target still needs
  its own SDK or linker, and the macOS extension needs an app bundle and the
  `com.apple.developer.fskit.fsmodule` entitlement (INSPECTED, one family,
  [Cargo FAQ](https://doc.rust-lang.org/cargo/faq.html),
  [passthrough sample](https://developer.apple.com/documentation/fskit/building-a-passthrough-file-system)).
- **Bindings** (INSPECTED):
  - `windows-rs` and `windows-sys` cover the CloudFilters API
    ([windows-sys](https://docs.rs/windows-sys/latest/windows_sys/Win32/Storage/CloudFilters/index.html));
  - `objc2-fs-kit` 0.3.2 (2025-10-04, the latest release) predates the
    macOS 27 SDK. It has no `FSContext`, `DataCacheHandler`, `XattrHandler`
    or `AccessCheckHandler`, only the earlier `*Operations` protocols, so
    the macOS 27 caller-identity and cache-coherency APIs have no Rust
    binding
    ([docs.rs](https://docs.rs/objc2-fs-kit/0.3.2/objc2_fs_kit/),
    [crates.io](https://crates.io/api/v1/crates/objc2-fs-kit));
  - `fuser` (§3), [tokio](https://tokio.rs/) and
    [RustCrypto](https://github.com/RustCrypto) exist.
- **Costs** (NOT VERIFIED): learning curve, less mature Kerberos/GSS bindings
  (a GSS-API binding,
  [libgssapi](https://docs.rs/libgssapi/latest/libgssapi/), exists), and
  fewer AFS developers who already work in Rust.
- An FFI boundary to a C Rx is not a cost: ADR 0001 has Rx written in Rust
  (INSPECTED, one family). The platform interfaces (FSKit, Cloud Files) are
  still an FFI boundary, under the project's `unsafe` rule.

## 7. Licences of the existing implementations

- **OpenAFS** is mostly under the IBM Public License 1.0 (IPL-1.0), with
  parts under other terms such as BSD and MIT licences. The IPL-covered code
  is moving to a dual licence: distribution under either IPL-1.0 or GPLv2
  (INSPECTED,
  [Relicensing OpenAFS](https://www.openafs.org/pages/relicense/relicensing-openafs.html)).
  The reasons:
  - newer Linux kernel interfaces are often limited to GPL code;
  - code derived from Sun RPC was under a Sun licence incompatible with GPLv2.
    The original Sun RPC code is now available under the 3-clause BSD
    licence; that grant may not cover every later contribution to derived
    code (INSPECTED,
    [IBM's letter](https://openafs.org/pages/relicense/IBM_OpenAFS_License_Approval.pdf)).
- IBM's letter, dated 2025-01-21, makes all IBM-copyrighted source code in
  the IBM OpenAFS 1.0 release available under GPLv2 as an alternative to the
  IPL (INSPECTED, [IBM's letter](https://openafs.org/pages/relicense/IBM_OpenAFS_License_Approval.pdf)).
  The project's page dates IBM's announcement 2025-02-05; no separate IBM
  publication of that date was found.
- The project is asking contributors to agree to the change for later
  contributions, and posted its request for consent covering the current
  code base to openafs-info on 2026-10-22 (INSPECTED,
  [Relicensing OpenAFS](https://www.openafs.org/pages/relicense/relicensing-openafs.html),
  [announcement](https://www.mail-archive.com/openafs-info%40openafs.org/msg41790.html)).
  No published file-level inventory says how much of the tree is still
  IPL-1.0 only (INSPECTED, one family).
- **OpenAFS's Rx**, in its user-space libraries and its kernel module, is
  part of OpenAFS. The project notes its Rx/RPC code's Sun RPC lineage, so no
  single licence conclusion covers every Rx file (INSPECTED, one family,
  [Relicensing OpenAFS](https://openafs.org/frameless/relicense/relicensing-openafs.html)).
- **The Linux kernel's AF_RXRPC and kafs** are a separate implementation of
  Rx and AFS written for the kernel. The kernel is under GPL-2.0 with the
  syscall note, and individual files may be GPL-2.0-or-later; the per-file
  licences were not checked, under the clean-room rule (INSPECTED,
  [kernel licensing rules](https://docs.kernel.org/process/license-rules.html)).
  They are not IPL.
- **What this means for IronAFS** (ADR 0001): IronAFS is GPL-3.0-or-later and
  uses no code from any of these.
  - GPLv3 code cannot be combined for distribution with GPLv2-only code or
    with IPL-1.0 code (INSPECTED,
    [GNU licence list](https://www.gnu.org/licenses/license-list.html)). A
    GPLv2-or-later file, or a future grant, would need its own analysis;
    ADR 0001 rules out copying in every case.
  - Separately, AuriStor's AuriStorFS is a proprietary commercial product
    (INSPECTED, one family,
    [AuriStor](https://www.auristor.com/filesystem/auristor-comparison)):
    before running its software or capturing traffic from its cells, the
    applicable terms are checked (ADR 0001, test tools).

## 8. Protocol documentation

What IronAFS can implement from, under ADR 0001. Each document has its own
licence: facts are taken from it, not its text.

- **The community Rx specification draft**, which describes Rx packets,
  sequencing and acknowledgements. It was originally written by Nickolai
  Zeldovich and later updated by Jeffrey Altman; the current text is dated
  2026-07-24. It does not specify the security classes and carries no
  licence notice (INSPECTED,
  [Rx specification draft](https://registrar.central.org/rx-spec.txt)).
- **The rxgk Internet-Drafts**, which describe the GSS-API-based security
  class, its key negotiation and its packet types. They are expired in the
  IETF; the latest is draft-wilkinson-afs3-rxgk-11 (2014-03-02) (INSPECTED,
  [datatracker](https://datatracker.ietf.org/doc/draft-wilkinson-afs3-rxgk/)).
  The AFS3 Standardization Group promoted -11 to AFS3 experimental status
  (INSPECTED, one family, [AFS3 standardization](https://afs3-stds.central.org/)).
- **"Integrating rxgk with AFS"**, an expired draft (latest -08), which
  describes how rxgk plugs into AFS services (INSPECTED,
  [draft -08](https://datatracker.ietf.org/doc/html/draft-wilkinson-afs3-rxgk-afs-08)).
- **The Linux kernel's AF_RXRPC documentation.** It mostly describes the
  socket API, with a short protocol summary, no packet layout, and a
  security section that covers rxkad only, so it is a limited source
  (INSPECTED,
  [AF_RXRPC](https://docs.kernel.org/networking/rxrpc.html)). The
  documentation, not the kernel code, is the source.
- **Observed behaviour:** traffic captured against a test cell, reduced to
  synthetic fixtures (ADR 0001). None has been captured yet.

## 9. Omissions found by the refute check

Points the note did not cover that change the design. Labels as above.

### Windows (Cloud Files)

The [Windows Cloud Files note](2026-11-19-windows-cloud-files.md) covers
these in full, with measurements.

- **Hydrated files bypass the provider.** Reads and writes of hydrated data
  go to NTFS; NTFS DACLs decide access, and AFS ACLs and tokens are not
  consulted again (INSPECTED,
  [Build a cloud sync engine](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine)).
- **No user identity in callbacks.** On request, a callback carries the
  process id, image path, package, command line and session, but no user or
  token (INSPECTED,
  [CF_PROCESS_INFO](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ns-cfapi-cf_process_info),
  [CF_CONNECT_FLAGS](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_connect_flags)).
- **No write callback.** The provider must find changed files itself, upload
  them and resolve conflicts (INSPECTED,
  [CF_CALLBACK_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type)).
- **Remote changes need a placeholder state machine.** `CfUpdatePlaceholder`
  with `CF_UPDATE_FLAG_VERIFY_IN_SYNC` protects local edits; dehydrating in
  the update needs an exclusive handle that the platform does not validate.
  Callbacks run concurrently, each with a 60-second timeout that any valid
  operation resets (INSPECTED,
  [CfUpdatePlaceholder](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfupdateplaceholder),
  [CF_CALLBACK_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type)).
- **Namespace limits.** NTFS only, sync-root placement rules and per-user
  registration; no lock callback, so no lock delegation (INSPECTED,
  [CfRegisterSyncRoot](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfregistersyncroot)).
- **Partial hydration** (`CF_HYDRATION_POLICY_PARTIAL`) is not supported
  (INSPECTED,
  [CF_HYDRATION_POLICY_PRIMARY](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_hydration_policy_primary)).
- **Alternatives:** ProjFS is compared in the Windows note. WinFsp relies
  on a third-party kernel driver, which IronAFS does not use; it was not
  researched (NOT VERIFIED).

### macOS (FSKit)

- **Per-user identity and cache invalidation start at macOS 27.**
  `FSContext`, the context-aware handlers and `setCacheState` are macOS 27
  APIs (INSPECTED,
  [FSContext](https://developer.apple.com/documentation/fskit/fscontext)).
  That earlier versions have no other way to purge the kernel name cache is
  RELAYED ([macFUSE #1207](https://github.com/macfuse/macfuse/issues/1207)).
  A correct multi-user client therefore needs macOS 27.
- **FSKit is unary:** "The current version of FSKit supports only
  FSUnaryFileSystem", one resource per volume (INSPECTED,
  [FSFileSystem](https://developer.apple.com/documentation/fskit/fsfilesystem)).
- **Operations arrive over XPC on a serial queue,** so the module must
  dispatch work off it (RELAYED,
  [forum thread 831338](https://developer.apple.com/forums/thread/831338)).
- **The kernel data cache bypasses per-request identity.** With caching on,
  cached reads and writes reach no handler; without a `DataCacheHandler`
  the kernel caches as it sees fit, and returning `.noCache` disables it
  (INSPECTED,
  [DataCacheHandler](https://developer.apple.com/documentation/fskit/fsvolume/datacachehandler)).
- **Write errors on close.** AFS stores a file back on close. FSKit has two
  close contracts: the `DataCacheHandler` close takes no error, while the
  `OpenCloseHandler` close replies with one (INSPECTED by a research pass,
  [DataCacheHandler close](https://developer.apple.com/documentation/fskit/fsvolume/datacachehandler/close%28_%3Acontext%3Areplyhandler%3A%29),
  [OpenCloseHandler](https://developer.apple.com/documentation/fskit/fsvolume/openclosehandler)).
  Whether an application then sees that error from `close`, and how a
  deferred close behaves, is NOT VERIFIED; volume-level `synchronize` also
  exists (INSPECTED, one family,
  [synchronize](https://developer.apple.com/documentation/fskit/fsvolume/commonoperations/synchronize(flags:replyhandler:))).
- **Deployment:** the extension needs its entitlements, and the user enables
  it in System Settings (INSPECTED,
  [passthrough sample](https://developer.apple.com/documentation/fskit/building-a-passthrough-file-system)).
- **macFUSE's FSKit backend** exists as a possible substrate; Apple DTS calls
  a FUSE-compatible layer "an excellent 3rd-party opportunity" (RELAYED,
  one family, [forum thread 831338](https://developer.apple.com/forums/thread/831338)).

### macOS (localhost NFS)

- **A loopback NFS bridge is another stateful protocol:** its identity trust
  model, file handles, lock ownership, client recovery, and the mapping of
  AFS callback breaks onto NFS caches and delegations. `nonegnamecache` and
  `nocallback` exist but do not give on-demand revocation (INSPECTED, one
  family, [`mount_nfs(8)`](https://man.freebsd.org/cgi/man.cgi?apropos=0&manpath=macOS+26.7&query=mount_nfs&sektion=8),
  [RFC 7530](https://datatracker.ietf.org/doc/html/rfc7530)).

### Linux (FUSE)

- **Writeback cache is "generally not suitable for network filesystems"**:
  it assumes every change goes through the FUSE kernel module (INSPECTED,
  [FUSE I/O modes](https://docs.kernel.org/filesystems/fuse/fuse-io.html)).
- **Callback breaks map onto FUSE notifications:** inode and entry
  invalidation, delete notification, and entry and attribute timeouts.
  Invalidating an inode under writeback flushes dirty pages and blocks
  (INSPECTED,
  [fuse_lowlevel.h](https://libfuse.github.io/doxygen/fuse__lowlevel_8h.html)).
- **A shared `/afs` needs `allow_other`** (and `user_allow_other` for a
  non-root mount). Without `default_permissions`, a permission result on a
  cached entry can be reused for another user; with it, mode bits apply that
  AFS ACLs do not follow (INSPECTED,
  [libfuse security implications](https://github.com/libfuse/libfuse#security-implications),
  [FUSE](https://docs.kernel.org/filesystems/fuse/fuse.html)).
- **A PAG equivalent** needs `/proc` or another mechanism, since FUSE sends
  no group list (§3).
- **Direct I/O and passthrough change semantics:** direct I/O disables
  shared `mmap` by default, and passthrough bypasses the daemon on a local
  backing file and needs `CAP_SYS_ADMIN` (INSPECTED,
  [FUSE I/O modes](https://docs.kernel.org/filesystems/fuse/fuse-io.html),
  [passthrough](https://docs.kernel.org/filesystems/fuse/fuse-passthrough.html)).

### Protocol and interoperability

- **rxkad is missing from §8.** Existing cells' clients use rxkad with
  Kerberos 5 tokens, and OpenAFS adds rxgk in its 1.9 series, so the first
  authenticated security class follows the target cell (INSPECTED,
  [OpenAFS roadmap](https://openafs.org/frameless/roadmap.html),
  [1.9.x installation notes](https://wiki.openafs.org/admin/OpenAFS_1.9.x_Installation_Notes/)).
  A public source to implement rxkad from is still to be found.
- **The AFS client baseline is not specified here:** cell and volume
  location discovery, volume traversal, ACL evaluation, callback
  registration and loss, version revalidation, write-on-close, and callback
  reachability through firewalls and NAT (INSPECTED, one family,
  [CellServDB](https://docs.openafs.org/Reference/5/CellServDB.html),
  [Administration Guide](https://docs.openafs.org/AdminGuide/HDRWQ17.html)).

## Sources

The links beside each claim are the sources. The main ones:

- Apple: [FSKit](https://developer.apple.com/documentation/fskit), [FSKit updates](https://developer.apple.com/documentation/updates/fskit), [File Provider](https://developer.apple.com/documentation/fileprovider), [Apple Platform Security](https://support.apple.com/guide/security/securely-extending-the-kernel-sec8e454101b/web); man pages [`synthetic.conf(5)`](https://man.freebsd.org/cgi/man.cgi?apropos=0&manpath=macOS+26.7&query=synthetic.conf&sektion=5) and [`mount_nfs(8)`](https://man.freebsd.org/cgi/man.cgi?apropos=0&manpath=macOS+26.7&query=mount_nfs&sektion=8) (mirror)
- Apple developer forums (RELAYED): [FUSE compat surface plans?](https://developer.apple.com/forums/thread/831338), [Porting VFS kext to FSKit](https://developer.apple.com/forums/thread/765424)
- [macFUSE](https://macfuse.github.io), [macFUSE 5.5.0](https://macfuse.github.io/2026/10/17/macfuse-5.5.0.html); RELAYED: [macFUSE #1220](https://github.com/macfuse/macfuse/issues/1220), [macFUSE #1207](https://github.com/macfuse/macfuse/issues/1207), [vaulthalla #200](https://github.com/vaulthalla/vaulthalla/issues/200), [JuiceFS #7814](https://github.com/juicedata/juicefs/issues/7814)
- Microsoft: [Build a cloud sync engine](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine), [Cloud Filter API reference](https://learn.microsoft.com/en-us/windows/win32/cfapi/cloud-filter-reference)
- Linux and libfuse: [FUSE](https://docs.kernel.org/filesystems/fuse/fuse.html), [FUSE I/O modes](https://docs.kernel.org/filesystems/fuse/fuse-io.html), [libfuse](https://libfuse.github.io/doxygen/), [libfuse ChangeLog](https://github.com/libfuse/libfuse/blob/master/ChangeLog.rst), [Rust platform support](https://doc.rust-lang.org/rustc/platform-support.html)
- OpenAFS history (RELAYED): [Using FUSE for OpenAFS? (2007)](https://openafs-info.openafs.narkive.com/E8NU7V5g/using-fuse-for-openafs), [Using FUSE for AFS/DFS (2005)](https://linux-kernel.vger.kernel.narkive.com/GytY698E/using-fuse-for-afs-dfs-was-re-openafs-devel-openafs-opendfs-collaboration); [afsd.fuse package](https://packages.debian.org/sid/openafs-fuse)
- Licences: [Relicensing OpenAFS](https://www.openafs.org/pages/relicense/relicensing-openafs.html), [IBM's letter](https://openafs.org/pages/relicense/IBM_OpenAFS_License_Approval.pdf), [GNU licence list](https://www.gnu.org/licenses/license-list.html), [kernel licensing rules](https://docs.kernel.org/process/license-rules.html)
- Protocol documentation: [Rx specification draft](https://registrar.central.org/rx-spec.txt), [rxgk draft -11](https://datatracker.ietf.org/doc/html/draft-wilkinson-afs3-rxgk-11), [Integrating rxgk with AFS, draft -08](https://datatracker.ietf.org/doc/html/draft-wilkinson-afs3-rxgk-afs-08), [AFS file locking draft](https://datatracker.ietf.org/doc/html/draft-mbenjamin-afs-file-locking-06), [Linux AF_RXRPC documentation](https://docs.kernel.org/networking/rxrpc.html)

## Checked

Refute-checked on 2026-11-19. Three model families each tried to prove every
claim wrong against primary sources: OpenAI GPT, Zhipu GLM, and Anthropic
Claude (the same family as the note's author). A fourth, DeepSeek,
adjudicated the eleven claims where they asserted conflicting facts. The
record, with every seat and verdict, is
[2026-11-19-research-refute-check](../reviews/2026-11-19-research-refute-check.md).
