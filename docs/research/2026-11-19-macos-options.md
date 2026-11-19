<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# macOS options with only what Apple ships: research notes (2026-11-19)

**How this note was made.**

- Scope: what an AFS client can build on macOS from Apple's own interfaces
  and clients alone: no kernel extension, no macFUSE, no other third-party
  driver. The needs are the ones an AFS client has:
  - one global namespace at `/afs`;
  - per-user authorisation, including for cached data;
  - server-driven cache invalidation (callback breaks);
  - byte-range locks;
  - large files;
  - behaviour that applications treat as a normal POSIX filesystem.
- Web research by one model family, refute-checked by a second (see
  "Checked" at the end). Nothing was mounted and no multi-user test was run.
- Labels as in the [research index](README.md):
  - **INSPECTED:** read in a primary source: Apple's developer documentation,
    the macOS 27.2 SDK headers, the man pages shipped with macOS, Apple's
    published source for its NFS, SMB and WebDAV clients, or a protocol
    specification;
  - **RELAYED:** read only in a secondary source, such as an Apple forum
    post;
  - **INFERRED:** a design conclusion drawn from the facts above;
  - **NOT VERIFIED:** stated by one family and not confirmed by the other,
    or not checked.
- Apple's published NFS client source is read for how Apple's client
  behaves. It is not AFS code, and ADR 0001's don't-copy rule is not
  engaged; IronAFS would implement an NFS server, never Apple's client.

## Answer

INFERRED, and both families agree:

- **macOS 15 and 26:** a loopback NFS server, mounted at a
  `synthetic.conf` `/afs` by Apple's NFS client, is the candidate to test
  for a shared mount with the semantics AFS needs (SMB and WebDAV mounts
  also exist, §4, but fit less well). A request that reaches the server
  carries an RPC credential, which may be a saved one rather than the
  current caller's, and cached data is served with no request at all (§3),
  so cross-uid isolation needs the acceptance cases. Range locks reach the server through NLM (NFSv3) or
  `LOCK` (NFSv4), except that NFSv4 grants locks locally while the Mac
  holds a write delegation (§3, Locks).
- **macOS 27:** FSKit 27 leads, because it is the first Apple interface with
  both caller identity and a proactive purge of the kernel's data,
  attribute and name caches. Loopback NFS stays as the comparison path.
  Its range locks (NLM for NFSv3, `LOCK` for NFSv4) reach only IronAFS's
  own loopback server; whether and how that server maps them to AFS locks,
  which other AFS clients would see, is a separate open question (§3,
  Locks).
- **File Provider** is at most a Finder companion: it cannot give a shared
  `/afs`.

Two questions decide production use for any option: whether a second local
user can ever read cached data without being authorised, and whether a
callback break makes an open file, its attributes and directory lookups
fresh (§8).

## 1. FSKit

### By version

| macOS | What the public API offers | Evidence |
|---|---|---|
| 15.0–15.3 | No third-party FSKit: it was removed from the final 15.0 SDK | RELAYED: [DTS, forum thread 765424](https://developer.apple.com/forums/thread/765424) |
| 15.4 | A user-space app extension implements `FSUnaryFileSystem`: one resource, one volume, a block device, with Disk Arbitration integration. `XattrOperations`, `AccessCheckOperations` and `reclaimItem` exist; access checks carry no caller context. | INSPECTED: [15.4 release notes](https://developer.apple.com/documentation/macos-release-notes/macos-15_4-release-notes), [AccessCheckOperations](https://developer.apple.com/documentation/fskit/fsvolume/accesscheckoperations) |
| 26.0 | `FSGenericURLResource` (any URL, including a network address) and `FSPathURLResource` (a file URL, possibly security-scoped); opt-in open-unlink emulation (`enableOpenUnlinkEmulation`). Still the legacy operation signatures, no caller identity, no cache control. | INSPECTED: [FSGenericURLResource](https://developer.apple.com/documentation/fskit/fsgenericurlresource), [FSPathURLResource](https://developer.apple.com/documentation/fskit/fspathurlresource), `FSVolume.h` in the 27.2 SDK |
| 26.4 | `requestedMountOptions`, whose only public option is read-only. The identity and cache-control gaps remain. | INSPECTED: [MountOptions](https://developer.apple.com/documentation/fskit/fsvolume/mountoptions) |
| 27.0 | The `Handler` protocols, `FSContext`, `DataCacheHandler`, `setCacheState`, and the `FSClient` mount and settings APIs. macOS 27 runs on Apple silicon Macs only. | INSPECTED: [FSKit updates](https://developer.apple.com/documentation/updates/fskit), [FSContext](https://developer.apple.com/documentation/fskit/fscontext), [macOS 27 compatibility](https://support.apple.com/en-us/127255) |

No later 15.x or 26.x release adds caller identity or cache control (NOT
VERIFIED: one family looked and found none).

### Caller identity (macOS 27)

- `FSContext` holds the caller's real and effective uid and gid, and
  nothing else (INSPECTED,
  [FSContext](https://developer.apple.com/documentation/fskit/fscontext)).
- It reaches access checks and the `DataCacheHandler` open, close and
  upgrade calls; one family reports it also reaches lookup, enumeration and
  attribute operations (INSPECTED, sampled by the checking family,
  [AccessCheckHandler](https://developer.apple.com/documentation/fskit/fsvolume/accesscheckhandler),
  [DataCacheHandler](https://developer.apple.com/documentation/fskit/fsvolume/datacachehandler)).
- `ReadWriteHandler` read and write take an item, an offset and a buffer:
  no context and no per-open handle (INSPECTED,
  [FSVolume](https://developer.apple.com/documentation/fskit/fsvolume)).
  Authorisation therefore happens at open, upgrade, close and enumeration.
- Open questions (NOT VERIFIED): whether those hooks cover every path to
  cached bytes for a second uid, including the deferred-close path; and how
  to attribute deferred I/O when several uids have a file open. Returning
  `.noCache` takes kernel data caching out of the first experiment
  (INFERRED).

### Cache purge (macOS 27)

- `setCacheState` sends a synchronous request to the kernel, which the
  module may start at any time. It can fail, leaving the state unchanged;
  it must be called without the module's own locks held; and it returns
  `ENOTSUP` unless the volume conforms to `DataCacheHandler` (INSPECTED,
  [setCacheState](https://developer.apple.com/documentation/fskit/fsvolume/setcachestate(for:cachemode:coherencytype:action:))).
- `.pushInvalidate` flushes dirty data and clears the cache; `.invalidate`
  discards dirty data; both invalidate the item's attributes, and for a
  directory both purge the kernel name cache for its entries. `.revoke`
  invalidates every cache, revokes all access and triggers vnode
  reclamation (INSPECTED,
  [KernelCacheCoherencyAction](https://developer.apple.com/documentation/fskit/fsvolume/kernelcachecoherencyaction),
  `FSVolumeDataCacheHandler.h` in the 27.2 SDK).
- Open (NOT VERIFIED): negative name entries, active `mmap` mappings, and
  races between a purge and concurrent writes.

### Namespace, locks and POSIX behaviour

- Apple's sample mounts a module at a directory of the caller's choice with
  `mount -t`, and `mount(8)` has `-F` to force FSKit (INSPECTED,
  [passthrough sample](https://developer.apple.com/documentation/fskit/building-a-passthrough-file-system),
  `mount(8)` on macOS 27.2). `FSClient.mountSingleVolume` (27) needs the
  `com.apple.developer.fskit.mount` entitlement and mounts under `/Volumes/`
  (INSPECTED,
  [mountSingleVolume](https://developer.apple.com/documentation/fskit/fsclient/mountsinglevolume(resource:bundleid:options:completionhandler:))).
- I/O offsets are `off_t` and the maximum file size a `uint64_t`
  (INSPECTED,
  [PathConfOperations](https://developer.apple.com/documentation/fskit/fsvolume/pathconfoperations)).
- No public 27 protocol has a lock callback (INSPECTED, by enumeration of
  [FSVolume](https://developer.apple.com/documentation/fskit/fsvolume)'s
  protocols). FSKit gives IronAFS no way to forward an application's
  byte-range locks to AFS (INFERRED).
- Not tested (NOT VERIFIED): local `fcntl` behaviour; a machine-wide `/afs`
  mounted with `mount(8)` on a synthetic directory, and whether other
  sessions see it; large files and `mmap` end to end.

### Installation and approval

- The module is an app extension in a signed app, with the
  `com.apple.developer.fskit.fsmodule` entitlement. The user enables it in
  System Settings, General, Login Items & Extensions, File System
  Extensions (INSPECTED,
  [passthrough sample](https://developer.apple.com/documentation/fskit/building-a-passthrough-file-system)).
- The `com.apple.NSExtension` MDM payload manages extension allow and deny
  lists (INSPECTED,
  [NSExtensionManagement](https://developer.apple.com/documentation/devicemanagement/nsextensionmanagement)).
  Whether it can enable an FSKit module for every user without a prompt,
  and whether the mount entitlement is available to IronAFS's distribution,
  are NOT VERIFIED. So is Mac App Store distribution of an FSKit module.

## 2. File Provider

- **Domains.** Available from macOS 11. An app registers explicit domains,
  which can represent different accounts; macOS creates each domain's root
  and shows it in Finder (INSPECTED,
  [NSFileProviderDomain](https://developer.apple.com/documentation/fileprovider/nsfileproviderdomain),
  [NSFileProviderManager](https://developer.apple.com/documentation/fileprovider/nsfileprovidermanager)).
  From macOS 15 a domain can be placed on an eligible external volume;
  `volumeURL` picks the volume and macOS picks the directory (INSPECTED,
  [init(displayName:userInfo:volumeURL:)](https://developer.apple.com/documentation/fileprovider/nsfileproviderdomain/init(displayname:userinfo:volumeurl:))).
  That the usual root is under `~/Library/CloudStorage`, and that the
  `pathRelativeToDocumentStorage` initializer makes a non-replicated domain,
  are NOT VERIFIED.
- **`/afs`.** A synthetic `/afs` symlink could reach one chosen domain, but
  it resolves to the same target for every uid, so per-user domains cannot
  give a shared `/afs` (INFERRED).
- **Identity.** Requests carry the requesting executable and flags, but no
  user (INSPECTED,
  [NSFileProviderRequest](https://developer.apple.com/documentation/fileprovider/nsfileproviderrequest)).
  The model is one domain and one AFS sign-in per macOS account, with each
  account's materialised bytes protected only by macOS (INFERRED).
- **Materialisation and change.** Files are dataless until read and can be
  served offline once materialised; remote changes are signalled through
  enumeration (INSPECTED,
  [synchronisation](https://developer.apple.com/documentation/fileprovider/synchronizing-the-file-provider-extension)).
  A callback break can trigger a sync, with no synchronous purge of pages
  and attributes (INFERRED). That eviction fails for an open, unsynced or
  pinned item is NOT VERIFIED.
- **POSIX and large files.** Applications use ordinary file APIs on the
  local copy. From macOS 12.3 partial-content fetching serves POSIX range
  reads; cloning a file or reading it with file coordination fetches all of
  it (INSPECTED,
  [NSFileProviderPartialContentFetching](https://developer.apple.com/documentation/fileprovider/nsfileproviderpartialcontentfetching)).
  There is no lock operation, so locks stay local (INSPECTED, by absence).
- **Deployment.** The extension ships inside an app, and the user can
  disable a domain (INSPECTED,
  [userEnabled](https://developer.apple.com/documentation/fileprovider/nsfileproviderdomain/userenabled)).
  The macOS 26.4 MDM controls over sync are NOT VERIFIED. So is the cost of
  many logged-in accounts syncing at once.

## 3. Loopback NFS

IronAFS would serve NFS on the machine itself, and Apple's NFS client would
mount it.

### Versions and `/afs`

- Apple's client supports NFSv3 and NFSv4.0; the release shipped with macOS
  26.0 adds v4.1 (INSPECTED, `mount_nfs(8)` at
  [NFS-327.140.2](https://raw.githubusercontent.com/apple-oss-distributions/NFS/NFS-327.140.2/mount_nfs/mount_nfs.8)
  and [NFS-339](https://raw.githubusercontent.com/apple-oss-distributions/NFS/NFS-339/mount_nfs/mount_nfs.8),
  [macOS 26.0 release manifest](https://raw.githubusercontent.com/apple-oss-distributions/distribution-macOS/main/release.json)).
  That NFS-327.140.2 is the macOS 15 release is NOT VERIFIED.
- `mount_nfs` tries v3 first by default, so the version is set explicitly
  (INSPECTED,
  [`mount_nfs(8)`](https://raw.githubusercontent.com/apple-oss-distributions/NFS/main/mount_nfs/mount_nfs.8)).
- A one-column `afs` line in `/etc/synthetic.conf` or `/etc/synthetic.d/`
  makes the kernel create an empty `/afs` at boot; an administrator can
  then mount the loopback export there with `mount -t nfs` (INSPECTED,
  [`synthetic.conf(5)`](https://keith.github.io/xcode-man-pages/synthetic.conf.5.html)).
  For on-demand mounting, a direct automount map is referenced from `/-` in
  `/etc/auto_master`, keyed by the full path (INSPECTED,
  [`auto_master(5)`](https://keith.github.io/xcode-man-pages/auto_master.5.html)).
  The exact automount configuration for `/afs`, and its ordering after a
  server restart, are NOT VERIFIED; a direct mount is the first test.

### Caller identity

- With `sec=sys`, Apple's client sends the caller's uid, primary gid and
  supplementary gids: asserted, with no proof (INSPECTED,
  [RPC credential](https://github.com/apple-oss-distributions/NFS/blob/main/kext/nfs_subs.c),
  [RFC 5531](https://www.rfc-editor.org/rfc/rfc5531)).
- `access(2)` checks use the caller's credential, and their results are
  cached per uid (INSPECTED,
  [access checks](https://github.com/apple-oss-distributions/NFS/blob/main/kext/nfs_vnops.c)).
- Buffered I/O can use a credential kept from an earlier request, and a hit
  in the data cache sends no READ at all (INSPECTED,
  [buffered I/O](https://github.com/apple-oss-distributions/NFS/blob/main/kext/nfs_bio.c)).
  So cached data is served without a per-user check by the server.
- A TCP listener on localhost is not a multi-user trust boundary, since any
  local process can assert any uid (INFERRED). Apple's `mount_nfs` source
  supports Unix-domain sockets, though `mount_nfs(8)` does not document
  them (INSPECTED,
  [mount_nfs.c](https://github.com/apple-oss-distributions/NFS/blob/main/mount_nfs/mount_nfs.c)).
  A root-only socket whose server checks the peer's credentials is the
  transport to test (INFERRED). Socket permissions, automount and the v4
  callback path over a socket are NOT VERIFIED.

### Caching and revalidation

- Attribute cache timeouts for files, directories and the root are set with
  `acreg*`, `acdir*` and `acrootdir*`; the defaults are 5 s minimum and
  60 s maximum. `actimeo=0` or `noac` turns attribute caching off, and
  `nonegnamecache` turns off negative name caching (INSPECTED,
  [`mount_nfs(8)`](https://raw.githubusercontent.com/apple-oss-distributions/NFS/main/mount_nfs/mount_nfs.8)).
- The `access(2)` cache holds three users by default (`accesscache=`), in
  first-in, first-out order (INSPECTED, `mount_nfs(8)`, Apple's source and
  the page shipped with macOS 27.2). Separately, `nfs.conf(5)` gives access
  cache entries a 60 s lifetime by default (INSPECTED,
  [`nfs.conf(5)`](https://raw.githubusercontent.com/apple-oss-distributions/NFS/main/files/nfs.conf.5));
  whether the two pages describe the same cache is NOT VERIFIED.
  With `nfs.client.access_cache_timeout` at zero no entry is ever fresh,
  so each check goes to the server, except on soft mounts, which can still
  use cached results (INSPECTED,
  [access cache](https://github.com/apple-oss-distributions/NFS/blob/main/kext/nfs_vnops.c)).
- These settings shorten revalidation; kernel data pages remain a separate
  coherence problem (INFERRED).

### Delegations and invalidation

- NFSv3 has no general way for the server to invalidate the client's data
  cache (INSPECTED, [RFC 1813](https://www.rfc-editor.org/rfc/rfc1813)).
- Apple's NFSv4 client implements `CB_RECALL`: it checks the delegation's
  state id and queues `DELEGRETURN` (INSPECTED,
  [recall handler](https://github.com/apple-oss-distributions/NFS/blob/main/kext/nfs_socket.c),
  [delegation return](https://github.com/apple-oss-distributions/NFS/blob/main/kext/nfs4_vnops.c)).
- A callback break can trigger a recall where IronAFS granted a
  delegation, but a recall does not purge pages, attributes or lookups of
  undelegated files (INFERRED). When macOS drops pages and attributes after
  a recall, especially for an open or mapped file, and what happens to
  cached access results after a token expires or an ACL changes, are NOT
  VERIFIED.

### Locks

- NFSv3 uses NLM for remote locks; `locallocks` keeps locks on the client,
  and `nolocks` turns locking off (INSPECTED,
  [`mount_nfs(8)`](https://raw.githubusercontent.com/apple-oss-distributions/NFS/main/mount_nfs/mount_nfs.8)).
- Apple's NFSv4 client sends `LOCK` and `LOCKU` with 64-bit byte ranges,
  retries in the grace period and has lock recovery (INSPECTED,
  [NFSv4 locks](https://github.com/apple-oss-distributions/NFS/blob/main/kext/nfs4_vnops.c)).
- **While it holds a write delegation, Apple's NFSv4 client grants locks
  locally and sends no `LOCK` to the server** (INSPECTED, found by the
  checking family, same source). NFSv4 requires the client to send its
  lock state to the server before it returns a recalled delegation
  ([RFC 7530](https://www.rfc-editor.org/rfc/rfc7530)), so other NFS
  clients are protected through the recall; what IronAFS's loopback server
  must do on a recall, and how long an AFS client elsewhere can go without
  seeing the lock, are open (INFERRED).
- **Three different things** are easy to conflate: delivery of a lock to
  IronAFS's loopback server; coordination among NFS clients of that server
  (only the Mac's own processes here); and visibility to separate AFS
  clients, which needs IronAFS to map the lock to AFS's own locking, whose
  granularity (whole-file server locks) is a design decision (INFERRED).
- Both versions use 64-bit sizes and offsets, and a file unlinked while
  open may be renamed to `.nfs*` (INSPECTED, same man page and source).
  Hard mounts can hang while the server does not answer (INSPECTED, one
  family, same man page).
- NFS fits applications' POSIX expectations best of the loopback options.
  IronAFS still has to decide whether Mac locks stay local or become
  visible to other AFS clients, for example as whole-file AFS server locks
  (INFERRED).

### Precedent

rclone mounts its own user-space NFSv3 server through Apple's client
([rclone serve nfs](https://rclone.org/commands/rclone_serve_nfs/)), and
DarwinFUSE serves NFSv4 over a private Unix socket in a mode-0700 directory,
checking the peer's credentials
([DarwinFUSE](https://github.com/marcelcotta/DarwinFUSE)) (INSPECTED, each
project's documentation).

## 4. Loopback SMB and WebDAV

- **SMB.** `mount_smbfs` takes a mount path and an authenticated session,
  defaulting to the logged-in user; `nodatacache` and `nomdatacache` turn
  off data and metadata caching (INSPECTED,
  [`mount_smbfs(8)`](https://raw.githubusercontent.com/apple-oss-distributions/SMBClient/main/mount_smbfs/mount_smbfs.8)).
  Its advisory-lock code is built around `flock` (INSPECTED,
  [smbfs_vnops.c](https://github.com/apple-oss-distributions/SMBClient/blob/main/kernel/smbfs/smbfs_vnops.c)).
  Lease support (only lease constants were seen in the source), `F_POSIX`
  locks depending on a CIFS Unix capability, and loopback behaviour are NOT
  VERIFIED. One SMB session cannot choose AFS tokens by local uid, so a
  shared mount cannot do per-user authorisation (INFERRED).
- **WebDAV.** `mount_webdav` takes an `http://` or `https://` URL and an
  existing mount point, reports every file's owner and group as unknown,
  and refuses write opens on servers without DAV `LOCK` (INSPECTED,
  [`mount_webdav(8)`](https://raw.githubusercontent.com/apple-oss-distributions/webdavfs/main/mount.tproj/mount_webdav.8)).
  DAV locks are per resource, with no byte ranges
  ([RFC 4918](https://www.rfc-editor.org/rfc/rfc4918)). A poor fit for a
  shared AFS namespace (INFERRED). Its large-file limit is NOT VERIFIED.
- **Other Apple pieces.** AFP is not supported from macOS 27 (INSPECTED,
  [Apple support](https://support.apple.com/en-us/102423)). autofs, NetFS,
  VirtioFS (host directories into a guest), `mount_9p`, Finder's read-only
  FTP and disk images were described by one family and not checked (NOT
  VERIFIED); none serves a dynamic namespace with caller identity and
  invalidation (INFERRED).

## 5. Deployment, all options

The loopback designs install IronAFS's user-space server and use the client
macOS ships. The `/etc` configuration and the machine-wide mount are
administrator tasks. A launch daemon registered with `SMAppService` needs
administrator approval before it starts; MDM can deploy signed packages and
manage background items. Direct distribution needs Developer ID signing and
notarisation, with the hardened runtime for apps and command-line tools
(NOT VERIFIED: one family,
[SMAppService](https://developer.apple.com/documentation/servicemanagement/smappservice/3945413-registerandreturnerror),
[notarisation](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)).

## 6. Comparison

Labels per cell: **I** INSPECTED, **F** INFERRED, **N** NOT VERIFIED.

| Option | Machine-wide `/afs` | Per-user access to cached data | Callback invalidation | Byte-range locks | Large files and POSIX |
|---|---|---|---|---|---|
| FSKit 15.4–26 | **N** Shared root mount | **I** No caller context | **I** No proactive purge until 27 | **F** No forwarding path | **I** 64-bit offsets, VFS operations |
| FSKit 27+ | **N** `/afs` through `mount(8)` | **I** Context at open and access checks; **N** cached-I/O isolation | **I** Data, attribute and name-cache purge; **N** mappings, negative lookups | **F** No forwarding path; **N** local semantics | **I** 64-bit, richer VFS; **N** application compatibility |
| File Provider 11+ | **F** One alias to one domain | **I** Per-domain copies; **F** per-account replica | **I** Sync and eviction | **F** Local only | **I** Local file APIs, range fetch |
| Loopback NFSv3 | **I** Stock client, synthetic mount point | **I** Uid on the wire, per-uid access cache; **N** revocation end to end | **I** Revalidation tuning; **F** no callbacks | **I** NLM or client-local | **I** 64-bit, native network VFS |
| Loopback NFSv4 | **I** Same | **I** Uid on the wire; **N** cached reads and deferred I/O after an authorisation change | **I** Delegation recall; **N** purge coverage | **I** Server-visible ranges, except under a write delegation | **I** 64-bit, stateful network VFS |
| Loopback SMB | **F** Privileged mount, loopback untested | **F** One session identity | **N** Leases; **N** purge coverage | **F** POSIX mapping unsuitable | **F** SMB semantics |
| Loopback WebDAV | **F** Privileged mount, loopback untested | **F** Mount-level identity | **F** No suitable callback | **I** Resource locks only | **N** Large-file limit |

## 7. Recommendation by macOS version (INFERRED)

Both families agree on this table.

| macOS | Combination | Main risk |
|---|---|---|
| 15.x | NFSv4.0 loopback server, `vers=4` set explicitly, a synthetic `/afs`, a managed launch daemon, a root-only Unix socket; NFSv3 with NLM as a baseline. Attribute timeouts tuned down, or `noac` while correctness is measured. File Provider at most a Finder companion. | Recall and revalidation may leave stale pages or access results (NOT VERIFIED): a prototype until §8 passes. |
| 26.x | The same deployment, testing NFSv4.1 with v4.0 as the baseline. FSKit 26 URL resources only for mount-mechanics prototypes. | v4.1 sessions and callback recovery add state to test; the cache authorisation and freshness questions remain. |
| 27+ | FSKit 27 on the shared AFS core, with `.noCache` for file data at first; NFSv4 kept as the comparison and compatibility path. File Provider at most a companion. | FSKit must prove cross-uid authorisation and acceptable local locks. If byte-range locks must reach AFS, FSKit has no lock callback, and NFSv4 is the candidate. |

## 8. Acceptance cases

Production use of any option hinges on these, run on the same `/afs` mount
(INFERRED, both families):

1. A second uid opens a file whose pages are already cached (for FSKit,
   including through the deferred-close path).
2. A token expires or an ACL changes during buffered and during mapped I/O.
3. A callback break refreshes the file's bytes, its attributes and directory
   lookups, including for an open and a mapped file.
4. Two applications contend for overlapping `fcntl` ranges, distinguishing
   locks local to the Mac from locks visible to other AFS clients, with NLM
   (NFSv3) and `LOCK` (NFSv4) compared.
5. After a server failure, record what an application sees from `write`,
   `fsync` and `close`, each separately, with and without kernel data
   caching, and for FSKit through each close contract; the results define
   the durability contract.

The checking family adds, for NFSv4: the same lock contention while the Mac
holds a write delegation (§3, Locks).

## Sources

The links beside each claim are the sources. The main ones:

- Apple FSKit: [FSKit](https://developer.apple.com/documentation/fskit), [FSKit updates](https://developer.apple.com/documentation/updates/fskit), [FSContext](https://developer.apple.com/documentation/fskit/fscontext), [DataCacheHandler](https://developer.apple.com/documentation/fskit/fsvolume/datacachehandler), [setCacheState](https://developer.apple.com/documentation/fskit/fsvolume/setcachestate(for:cachemode:coherencytype:action:)), [passthrough sample](https://developer.apple.com/documentation/fskit/building-a-passthrough-file-system); the macOS 27.2 SDK headers
- Apple File Provider: [File Provider](https://developer.apple.com/documentation/fileprovider), [NSFileProviderDomain](https://developer.apple.com/documentation/fileprovider/nsfileproviderdomain)
- Apple's NFS, SMB and WebDAV clients: [NFS source and man pages](https://github.com/apple-oss-distributions/NFS), [SMBClient](https://github.com/apple-oss-distributions/SMBClient), [webdavfs](https://github.com/apple-oss-distributions/webdavfs); [`synthetic.conf(5)`](https://keith.github.io/xcode-man-pages/synthetic.conf.5.html) and [`auto_master(5)`](https://keith.github.io/xcode-man-pages/auto_master.5.html) (mirror of the shipped pages)
- Protocols: [RFC 1813](https://www.rfc-editor.org/rfc/rfc1813), [RFC 5531](https://www.rfc-editor.org/rfc/rfc5531), [RFC 7530](https://www.rfc-editor.org/rfc/rfc7530), [RFC 8881](https://www.rfc-editor.org/rfc/rfc8881), [RFC 4918](https://www.rfc-editor.org/rfc/rfc4918)
- Precedent: [rclone serve nfs](https://rclone.org/commands/rclone_serve_nfs/), [DarwinFUSE](https://github.com/marcelcotta/DarwinFUSE)

## Checked

Written from one family's research (OpenAI GPT) and refute-checked by a
second (Zhipu GLM) on 2026-11-19: no claim was refuted, a few were amended,
and both families agree on the recommendation. Where the checking family
could not reach a claim, or amended one against the man page, the macOS
27.2 SDK headers and the man pages shipped with it were read. The record is
[2026-11-19-research-refute-check](../reviews/2026-11-19-research-refute-check.md).
