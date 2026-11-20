<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0004, part 2: OS layers and Windows

Status: proposed (2026-11-20). Part of ADR 0004 ([overview](0004-architecture.md)); raised in issue #20.

This part decides how IronAFS appears on each operating system: the Linux,
macOS and Windows layers, what all of them share when they present names,
owners, links and locks, where the filesystem is mounted, and which
platforms are targets. The core they translate for is part 1. Operations,
packaging and phasing are part 3. Negotiation and long-lived formats are
part 4.

Conventions, as in the overview:

- A claim id in brackets, such as `[DES-024]`, points to a row of the
  [claim record](../reviews/2026-11-20-adr-0004-claims.md). "Corrected form"
  marks a row labelled AMEND; the text here uses the correction.
- A1 to A14 are the named assumptions in the overview. A statement that
  rests on one says so.
- Each decision is **Firm** (it changes only by a superseding ADR) or
  **Working** (it has a trigger for a second look).
- "The spike" is the early Windows spike and "the macOS gate" is the set of
  measurements that opens the macOS layer. Both are scheduled in O50.
- Process names (`ironafsd`, `ironafs-smbd`) are working names until the
  identifier freeze (O38).

## Context

The overview gives the constraints this part works under. Three matter most
here. IronAFS uses only what each operating system ships: no kernel code of
its own and no third-party driver. One local user must never read data with
another user's AFS rights. A layer is a translator and holds no authority.

### Linux and FUSE

1. Every FUSE request carries the caller's uid, gid and pid `[DES-022]`. It
   carries no group list `[DRV-065]`.
2. A write that the kernel sends later from its page cache has no valid
   caller fields `[DES-023]` `[DRV-066]`. The kernel documentation says the
   writeback cache is generally not suitable for network filesystems
   `[DRV-067]`.
3. Without the `default_permissions` mount option, all permission checking
   is left to the filesystem `[DES-025]`, and the result of the first check
   on a cached entry may be reused for other users `[DRV-072]`. With it, the
   kernel checks the mode bits itself; negotiating POSIX ACL support turns it
   on implicitly `[DES-026]`. AFS uses no mode bits on a directory, whose
   ACL alone governs it, and of a file's mode bits uses only the owner's set
   ([Administration Guide, mode bits](https://docs.openafs.org/AdminGuide/HDRWQ580.html)).
4. If a filesystem answers ACCESS with `ENOSYS`, the kernel treats that as
   permanent success `[DES-024]`. `ENOSYS` for LSEEK or COPY_FILE_RANGE is
   also permanent for the mount `[DES-031]`.
5. Direct I/O bypasses the page cache, and a shared mapping is disabled by
   default in that mode `[DES-027]` `[DRV-068]`.
6. An error from RELEASE is not returned to `close()`, and FLUSH may arrive
   many times for one open, or not at all after writes `[DES-029]`.
7. Filesystems mounted through the libfuse helper are `nodev,nosuid` by
   default, and the helper enforces those limits, so a filesystem running
   as root avoids them `[DES-035]`; such a mount has the flags only if it
   passes them itself. The helper is installed setuid root `[PLT-059]`.
   systemd's `NoNewPrivileges=` stops a service gaining privileges through
   a setuid program `[DES-041]`.
8. Aborting a FUSE connection fails every waiting and new request
   `[DES-036]`. Linux 6.5 can attach a mount beneath the current one
   `[DES-037]`. Protocol 7.40 adds a way to resend requests `[DES-033]`.
9. Upstream Linux 3.10 speaks FUSE 7.22 and RHEL 7's kernel ships 7.23
   `[PLT-M01]` `[PLT-031]`. Invalidation notifications for inodes and entries
   date from 7.12 `[PLT-034]`. The current upstream minor is 47 `[PLT-037]`.
   RHEL 7's kernel reports 7.23 yet carries LSEEK from 7.24, so a minor
   number alone does not say which features exist (`[PLT-M10]`, corrected
   form).
10. libfuse documents a deadlock when an invalidate-entry or delete
    notification is sent from a related operation's own path or under a
    lock that operation needs. Invalidate-inode instead waits for the
    writeback of dirty pages, which matters only with the writeback cache
    (`[DES-032]`, corrected form; `[DRV-069]`).
11. FUSE passthrough lets operations on a file bypass the daemon
    `[DRV-008]`.
12. If a filesystem implements no lock methods, the kernel still lets locks
    work locally `[DES-030]`.

### macOS and FSKit

13. FSKit became available in macOS 15.4 `[MAC-002]`. No FSKit API below the
    macOS 27 level carries a caller's identity or gives cache control
    `[MAC-M04]`. `FSContext` arrives in macOS 27.0 and holds the caller's real
    and effective user and group ids and nothing else `[MAC-013]`
    `[DRV-025]`.
14. `FSContext` is passed to the namespace operations and to the data-cache
    handler's open, close and upgrade calls `[MAC-014]`. Read and write take
    an item, an offset and a buffer: no caller and no per-open handle
    `[MAC-015]`.
15. A volume that adopts the data-cache handler gets every open and close
    through it `[MAC-M06]`. That handler's close takes no error `[DRV-046]`.
    Under the caching coherency types the kernel keeps cache state after a
    file is closed `[MAC-M07]`.
16. `setCacheState` (27.0) sends a cache state change to the kernel
    `[PLT-046]`. Its actions flush or discard dirty data and drop cached
    attributes `[MAC-022]`; `revoke` revokes all access to the item
    `[MAC-023]`. It can fail, and must be called without holding the
    module's own locks `[MAC-021]`. Returning the no-cache coherency type
    from open for every item turns data caching off `[MAC-024]`.
17. No public FSKit 27 volume protocol has a byte-range lock callback
    `[MAC-025]` `[DRV-051]`. FSKit has a volume synchronise operation
    `[DRV-M13]`. Its only public mount option is read-only `[MAC-011]`.
18. Apple's compatibility list for macOS 27 names only Macs with Apple
    silicon (`[MAC-018]`, corrected form).
19. Apple's sample FSKit module is mounted with `mount(8)` at a directory,
    after the user enables the extension in System Settings `[MAC-026]`
    `[MAC-027]`.
    `FSClient.mountSingleVolume` mounts within `/Volumes/` `[MAC-020]`.
    `synthetic.conf` can create an empty directory or a symbolic link at `/`
    during early boot, changed only by a reboot `[MAC-039]` `[MAC-040]`
    `[MAC-M22]`.
20. The extension needs the FSKit module entitlement `[DRV-047]`. Software
    distributed with Developer ID must be signed, notarised and use the
    hardened runtime `[MAC-063]`. A launch daemon registered through
    `SMAppService` does not start until an administrator approves it
    `[MAC-M27]`. Device management can pre-approve the launch daemon and
    install the app; whether it can enable an FSKit extension is open
    (`[MAC-M28]`, corrected form).
21. A third-party project reports that FSKit on macOS 27 forwards the
    effective ids but cannot guarantee cache isolation between users
    `[DRV-M11]`. This is a report, not Apple's statement.

### Windows

22. Windows sends UNC file I/O only to its own kernel redirectors or to a
    third-party driver `[WIN-149]` `[WIN-150]`. IronAFS installs no driver,
    so it can serve a `\\afs\cell\path` path only through one of Windows' own
    redirectors talking to a server IronAFS runs on the same machine.
23. Drive letters belong to one logon session, so services, scheduled tasks
    and elevated processes may not see them `[WIN-192]` `[WIN-193]`
    `[DES-M04]`. Folder Redirection and roaming profiles take UNC paths
    `[WIN-194]` `[WIN-195]`, though neither is supported on Arm PCs
    `[WIN-M55]`. The existing OpenAFS client for Windows documents
    `\\AFS\cell\path` as its canonical name `[WIN-191]`.
24. Windows' SMB client connects to alternative TCP ports only on Windows 11
    24H2 and later, through an explicit mapping made by an elevated command
    `[WIN-162]` `[WIN-163]`. Microsoft's announcement of the feature
    describes a Lanman Workstation policy that allows or blocks the client's
    use of alternative ports (`[WIN-185]`, corrected form). On Windows 10
    the client uses direct TCP on port 445, and NetBIOS over TCP on port 139
    only for dialects up to 3.0.2 (`[WIN-151]`, corrected form;
    `[WIN-169]`). Windows' own SMB server holds port 445 `[WIN-152]` and can
    move only its QUIC port `[WIN-164]`.
25. **Route 3, as measured in public.** With Windows' built-in port proxy
    bound to `127.0.0.1:445` before the kernel's SMB listener, Windows' own
    stack binds `0.0.0.0:445` and `[::]:445` beside it `[WIN-155]`
    `[WIN-180]`. A bare `\\afs\...` from the desktop session then reaches a
    listener on a high loopback port `[WIN-156]`, while the machine's own
    shares, by computer name, LAN address and `localhost`, are still served
    by Windows `[WIN-157]`, as was inbound SMB from another machine (measured
    on the Arm64 machine only, `[WIN-158]`). One service dependency kept
    the bind order across a reboot `[WIN-159]` `[WIN-181]`. This was
    measured on Windows 10 Pro 22H2 and Windows 11 Pro 24H2 (x86-64) and
    Windows 11 26H2 (Arm64) `[WIN-153]`,
    with a listener that speaks no SMB, so the evidence reaches the client's
    first packet only `[WIN-154]`. The steps are in
    [the UNC research note](../research/2026-11-19-windows-unc-without-a-driver.md).
    A full authenticated session through the route is assumption A3.
26. Windows' SMB client refuses a path on a second loopback address
    (`\\127.0.0.2`) without connecting `[WIN-161]`.
27. A hosts entry applies to every program that uses the Windows resolver,
    the SMB client included (`[WIN-182]`, corrected form). We expect a
    literal `\\127.0.0.1\...` to reach IronAFS under route 3 and
    `\\localhost` to keep reaching Windows; neither is measured
    (`[WIN-183]`, corrected form).
28. A persistent TCP port reservation gives no protection between
    components that can obtain reservations `[WIN-187]` `[WIN-188]`.
29. In SMB, TREE_CONNECT, which names the share, follows SESSION_SETUP. That
    passing a session on to another server would therefore mean relaying the
    user's authentication is our inference (`[WIN-172]`, corrected form).
30. An SMB share can be flagged so that offline caching must not occur
    `[DES-070]`. Leasing is optional for a server `[DES-071]`. SMB 3.1.1
    derives signing and encryption keys from the session key and the
    pre-authentication hash `[DES-072]`. A Kerberos service name registered
    on more than one account breaks authentication `[DES-073]`. SMB client
    NTLM blocking exists on Windows 11 24H2 and later `[WIN-190]`.
31. A Windows service can run as a virtual account with its own SID
    `[DES-074]`. A service that is not LocalSystem cannot obtain a user's
    token (`[DES-075]`, corrected form). Microsoft documents Task Scheduler
    triggers that start a task when a user logs on
    ([task triggers](https://learn.microsoft.com/en-us/windows/win32/taskschd/task-triggers)),
    and machine-wide `Run` keys that run a program each time a user logs on
    ([Run keys](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys)).
32. Cloud Files is an NTFS-only minifilter for desktop sync engines, from
    Windows 10 1709 `[WIN-M41]`. Microsoft documents that full files are
    served without calling the provider; that only the NTFS DACL then
    governs access is our inference (`[DRV-014]`, corrected form). No
    callback reports a write, a lock or an open that can be refused
    `[WIN-M43]`. Callback information names no user `[WIN-M48]`
    `[WIN-M12]`. A placeholder is created with no security descriptor field
    `[WIN-047]`. Antivirus scanning of placeholders can hydrate them
    `[WIN-M49]`.
33. ProjFS has no caller identity, no write callback, no byte-range locks
    and no UNC namespace `[WIN-148]`. It is an optional component an
    administrator must enable `[WIN-136]` `[WIN-M22]`, reports neither
    recall progress nor offline state `[WIN-134]`, and Microsoft points slow
    backing stores to Cloud Files `[WIN-135]`.
34. Windows reserves device names such as `CON` and `COM1`, with or without
    an extension, and its shell does not support names ending in a space or
    a period `[DES-069]`. Microsoft's Services for Macintosh mapped
    forbidden characters into the Private Use Area with its own table
    `[DES-068]`, which has no entry for the colon
    ([Microsoft KB Q117258](https://techshelps.github.io/MSDN/KB/Source/winnt/q117258.htm)).
    When Cygwin creates or accesses a file, it converts each forbidden
    character except the backslash to U+F000 plus the character's code
    ([Cygwin User's Guide, special filenames](https://cygwin.com/cygwin-ug-net/using-specialnames.html)).

### AFS

35. AFS-3 file locks cover whole files and are advisory `[DES-050]`. A
    server lock lasts five minutes and must be renewed `[DES-051]`. An ACL
    has a separate lock right `[DES-052]`, and a read-only volume's
    fileserver denies every lock request `[DRV-058]`.
36. On AFS, only administrators may set the set-uid or set-gid bit
    `[DES-053]`. IronAFS does not treat a cell's administrators as this
    machine's.

## Decision

The decisions are numbered L1 to L43: L1 for every layer; L2 to L7 Linux;
L8 to L14 macOS; L15 to L28 Windows; L29 to L33 presentation shared by all
layers; L34 and L35 mounting and namespace; L36 to L43 the supported-platform
matrix, in its own section below.

### L1. A layer is a translator and holds no authority

**Firm.**

Each OS layer decodes a request, attaches the caller the OS reports (or
binds the request to an authority the core already checked), translates
names and metadata, calls one core operation, and encodes the reply. On
every OS a layer never:

1. decides access, or caches an access decision;
2. turns an error into success, an empty or short listing, zero bytes or a
   made-up attribute;
3. acts on a request that has no caller, except through a handle or binding
   the core authorised for that node (C13);
4. holds a lock of its own while it calls the kernel to invalidate;
5. rewrites or normalises a name on the server, or presents a name it cannot
   reverse;
6. stores file data or tokens, or sees a token;
7. waits without a deadline.

A layer depends only on the crates C4, point 4, allows (the contract
crate, C7; `ironafs-events`; `ironafs-ipc`; its own operating-system
bindings under their ADR; and `ironafs-crypto` for cryptography), never
on the cache or the protocol clients.

Why: authorisation is decided once, in the core, and tested once. A layer
that cached a decision or invented a reply could leak across users or hide a
failure, and each OS would need its own proof. Point 4 follows from
libfuse's deadlock warning `[DES-032]` and FSKit's rule against holding
the module's own locks `[MAC-021]`.

### L2. Linux uses FUSE through IronAFS's own protocol layer, floor 7.23

**Working.** Revisit if: assumption A1 fails or the RHEL 7.9 candidate row
lapses (the floor may then rise to the oldest remaining target's level), or
a target kernel lacks a notification this part depends on.

1. The Linux layer speaks the FUSE kernel protocol directly over
   `/dev/fuse`, in IronAFS's own code, and runs inside `ironafsd`.
2. The wire floor is protocol 7.23. The layer negotiates up to what the
   kernel offers.
3. Features are detected from the INIT flags and from per-operation replies,
   never from the minor number alone.
4. The layer never answers `ENOSYS` to ACCESS, nor to an operation that a
   volume could later support.
5. The mount fails if neither inode and entry invalidation nor a tested
   uncached mode works.
6. FUSE passthrough is not used.

Why: 7.23 is what the oldest candidate kernel ships `[PLT-031]`, and
invalidation predates it `[PLT-034]`. Point 3: `[PLT-M10]`. Point 4:
`[DES-024]` `[DES-031]`. Point 6: passthrough takes I/O out of the daemon,
where authorisation lives `[DRV-008]`.

### L3. `/afs` never grants local privilege on a POSIX layer

**Firm.**

1. The mount is always `nosuid,nodev`. The mounting unit passes both flags
   itself.
2. As a second barrier, the attribute mapping clears set-uid and set-gid
   bits and never presents a device node.
3. An acceptance test shows that a set-uid-root file in a cell does not
   change a local process's credentials.
4. Honouring set-id bits for a trusted cell is not offered. Offering it
   needs a superseding ADR.
5. This decision applies to every POSIX layer, macOS included. On macOS,
   point 2 applies as written, and point 1 needs the FSKit equivalent of
   `nosuid,nodev` that the macOS gate looks for (L13, point 6); the macOS
   layer ships only where that equivalent is in force. Point 3's test runs
   on each macOS release in the matrix.

Why: a cell's administrators can set those bits `[DES-053]`, and the
helper's `nodev,nosuid` limits do not bind a filesystem running as root
`[DES-035]`. Local privilege
escalation through `/afs` is a failure even if everything else works.

### L4. Every write on Linux is attributable to a caller

**Firm.**

1. The FUSE writeback cache is never enabled.
2. Shared writable mappings are refused. Shared read-only mappings are
   refused too while direct I/O is set (context, point 5; L5). Private
   mappings work on a kernel where the mapping gate (C15, point 5) passed,
   so programs on `/afs` run there. On a kernel that failed it, the
   affected mappings are refused only through a mechanism demonstrated on
   that kernel (FUSE sends no request when a file is mapped); where none
   is demonstrated, the kernel's row does not qualify for this layer
   (C15, point 5). When the handle's authority is revoked, the
   layer asks the kernel to drop the file's resident pages, so the next
   fault reaches the daemon and fails; pages already resident in a live
   mapping may stay readable until it is unmapped (C15, point 2).
3. The kernel's own permission checks are never used in place of the
   core's: no `default_permissions`, and no POSIX ACL negotiation.

Why: delayed writes arrive with no valid caller `[DES-023]` `[DRV-066]`, so
the core could not tell whose token should store them. The kernel's mode-bit
check does not express AFS ACLs, and POSIX ACL support turns it on
`[DES-025]` `[DES-026]`.

### L5. Linux kernel caches start off and relax only with evidence

**Working.** Revisit if: the acceptance suite shows a relaxed setting keeps
two-user isolation and callback freshness on every tier 1 kernel; each
setting is relaxed separately, with its test.

1. The mount uses `allow_other`. Without `default_permissions`, LOOKUP is
   where traversal is enforced, so entry and attribute timeouts start at
   zero.
2. Every open uses direct I/O.
3. The layer never sets the keep-cache or cache-directory open flags.
4. Invalidations are sent from a dedicated notifier thread, never from an
   operation's own path (C17).

Why: a cached entry's first permission result may be reused for another
user `[DRV-072]`. Direct I/O keeps the data of reads and writes out of a
cache shared between users `[DES-027]`. It does not cover pages that a
private mapping faults in: those are covered by C15, points 3 and 5.
Point 4: `[DES-032]`.

### L6. Commit points on Linux

**Working.** Revisit if: a measured workload shows these points lose an
error an application waited for, or a later protocol level gives RELEASE a
way to report one.

The layer calls the core's one commit operation with a reason:

1. FLUSH commits with reason `close`;
2. FSYNC commits with reason `fsync`;
3. a file opened for synchronous writes commits after each write, reason
   `write_through`;
4. RELEASE of an open with no dirty data only cleans up.
5. **Dirty data at RELEASE.** At the RELEASE of each open whose handle
   holds dirty data that no commit stored, because no FLUSH came after
   the writes, the layer commits that handle's data with reason `close`,
   under the handle's retained authority (C13). If the store fails, the
   data becomes a recovery record of the writing context's durable owner
   (C29, point 5), and the failure is reported through the side channel
   of O35, point 1; `close()` may already have reported success for that
   write (O41, point 6). RELEASE itself still never fails (C9, point 4).
   The M3 tests include writes followed by no FLUSH, and two contexts
   holding the file while the writer releases with no FLUSH and the
   other keeps it open.

Why: RELEASE cannot report an error, and FLUSH is the last call whose error
reaches `close()` `[DES-029]`. FLUSH may also not come at all after writes
`[DES-029]`, so point 5 gives that case a terminal step. The commit and the
journal behind it are C9 and C29.

### L7. Linux hosting: a small privileged mount step, an unprivileged daemon

**Working.** Revisit if: crash tests prove that every outstanding request
either completes once or fails explicitly across a daemon restart (then
`/afs` may survive a restart), or a tier 1 distribution's confinement
blocks the direct mount.

1. A small privileged unit mounts the shared FUSE mount at `/afs` with
   `allow_other,nosuid,nodev`, hands the device descriptor to `ironafsd`, and
   closes its own copy.
2. `ironafsd` runs under its own user, sandboxed, with `NoNewPrivileges`.
   There is no setuid helper and no edit to the system's FUSE configuration.
   `/afs` is hidden inside the daemon's own sandbox.
3. **No transparent recovery at first.** If the daemon exits, the old
   connection is aborted. The dead mount stays and fails every access until
   the replacement is ready. Open files and working directories stay on the
   dead connection.
4. Detach and replacement are serialised over an empty, access-denied mount
   point. A no-gap replacement (mount-beneath) is used only where the kernel
   and the namespace layout are verified. **Residual:** where mount-beneath
   is not used, a process that overrides file permissions (root, or one
   holding `CAP_DAC_OVERRIDE`,
   [capabilities(7)](https://man7.org/linux/man-pages/man7/capabilities.7.html))
   can see, and write to, the empty local
   directory during that window. This is documented with the other
   residuals of a daemon restart.
5. Containers get `/afs` from a stable shared host parent into a slave
   subtree, or remount their bind on replacement.
6. A planned upgrade first stops new mutations, lets stores already in
   progress finish, and reports what stays pending (O33). It stores no
   deferred record (C28, point 3).

Why: the helper's limits do not bind a root mount `[DES-035]`, and
`NoNewPrivileges` would defeat a setuid helper anyway `[DES-041]`
`[PLT-059]`. A dead mount that fails loudly is safer than a path that
silently shows the local disk `[DES-036]`. Mount-beneath needs Linux 6.5
`[DES-037]`.

### L8. macOS uses FSKit on macOS 27 and later, Apple silicon only

**Working.** Revisit if: the macOS gate (L13) fails on loading and
entitlements (point 7), on the extension reaching the daemon (point 9), or
on mapped I/O with the data cache off (point 3), in which case a loopback
NFSv4 server is the fallback to evaluate; or Apple brings the caller and
cache APIs to an earlier release.

1. The macOS layer is an FSKit extension written in Swift, one instance per
   mount. It forwards each operation to `ironafsd` over the adapter endpoint
   (C45) with the caller from `FSContext`, applies
   invalidations and acknowledges them. It contains no AFS logic. Replies
   and invalidations reach it on one ordered stream per binding over the
   adapter endpoint (C17), and it applies C16's submission check for
   out-of-process bindings (C16, point 6) immediately before it hands a
   reply to the kernel.
2. The floor is macOS 27. Earlier releases and Intel Macs are not targets.
3. A loopback NFS server stays a spike only, for macOS 15 and 26 (L43).

Why: below the macOS 27 API level FSKit names no caller and gives no cache
control `[MAC-M04]`, so neither per-user authorisation nor callback
freshness can be built. Apple's compatibility list for macOS 27 names only
Apple silicon Macs (`[MAC-018]`, corrected form).

### L9. A mount whose reads and writes name no caller serves one user

**Firm.**

Where an OS interface delivers reads or writes with no caller and no
per-open handle, a mount through it serves exactly one local user, unless a
recorded measurement shows that the interface isolates users on one mount.
The measurement covers at least: concurrent opens by two users, revocation
of one while the other reads, mapped I/O, and item reuse after reclaim.

A mount that serves one user is bound to that user:

1. Its binding is registered, from the daemon's own registration (C45,
   point 3), for one owner uid and that uid's credential context.
2. The daemon refuses any operation on that binding whose `FSContext` real
   or effective uid differs from the owner's. Uid 0 is no exception
   (C8, point 3).
3. Each item handed to the kernel stays bound to the credential generation
   it was served under until the kernel has released every reference to
   it. An operation on it under a later generation follows C13, point 3.

Why: FSKit's read and write carry neither `[MAC-015]`. On a shared mount,
when one user's authority is revoked, either the other user's authority
would serve that user's reads or `revoke` would cut both `[MAC-023]`. A
third-party report says cross-user cache isolation is not guaranteed
`[DRV-M11]`. `FSContext` carries plain ids the daemon cannot verify
`[DRV-025]`, so points 1 to 3 bound what a mount can be used for: a
compromised extension can act for the uid it serves, on its own binding,
and for no one else.

### L10. Per-user FSKit mounts by default

**Working.** Revisit if: the macOS gate shows per-user isolation on one
mount as L9 requires (then one shared `/afs` mount is used), or macOS does
not allow a per-user link at `/afs`.

1. Each user gets their own FSKit mount, under a per-user path, bound to
   that user as L9 says.
2. `/afs` is a symbolic link resolved per user if macOS allows it. If it
   does not, the per-user paths are documented and `/afs` is not promised on
   macOS.
3. If mounting at boot needs an administrator to enable the extension once,
   that is accepted and documented.

Why: L9. Root-level entries come from `synthetic.conf` `[MAC-039]`
`[MAC-040]`; whether a per-user target works there is a gate question. A
user enables the extension in System Settings `[MAC-027]`.

### L11. macOS mount and cache settings start conservative

**Working.** Revisit if: the acceptance suite shows a caching coherency
type keeps isolation and callback freshness (L13), or FSKit publishes mount
options beyond read-only.

1. Set-id bits and device nodes on macOS follow L3, which is Firm (L3,
   point 5).
2. The kernel data cache starts off: the no-cache coherency type for every
   item.
3. The layer calls `setCacheState` without holding its own locks, and treats
   a failure as "not invalidated". A failed `revoke` then follows C15's
   rule for macOS (C15, point 7), and the event stays unacknowledged
   (C17, point 2).

Why: `[MAC-024]` `[MAC-021]`. FSKit's only public mount option is read-only
`[MAC-011]`, so the flags cannot be assumed.

### L12. macOS writes need a commit that reports errors

**Working.** Revisit if: Apple adds an error result to the data-cache
handler's close, or the gate finds another commit an application waits on.

1. With per-user mounts there is one writing user per mount. On a shared
   mount, if one is ever enabled, there is one writing uid per item, and a
   second gets `EBUSY`.
2. Commits: `fsync(2)` if it reaches a per-item handler (reason `fsync`),
   volume synchronisation (reason `sync`), bounded intervals (reason
   `interval`) and the final close (reason `close`).
3. **Unless the gate shows that `fsync(2)`, or another commit the
   application waits on, reports a store error to the application, the macOS
   layer is read-only.**
4. A store that fails at a close-time commit keeps its data as a
   recovery record (C29) and raises a notification in the user's session
   (O35). A program that closes without calling `fsync(2)` (for example
   `cp`, or a shell redirection) can get success for such a write; the
   data then waits in `afs pending`. This is documented.

Why: the data-cache handler's close takes no error `[DRV-046]`, so a failed
store at close could be reported as saved. Volume synchronisation exists
`[DRV-M13]`, but whether its error reaches the caller is a gate question.

### L13. The macOS gate

**Working.** Revisit if: a macOS release changes any measured answer; each
answer is re-measured on every release named in the matrix.

Before the macOS layer is built beyond a prototype, these are measured on
macOS 27 and recorded:

1. whether a second uid's open reaches a hook that names the caller;
2. revocation of one uid while another holds the item, and what `revoke`
   does to descriptors already open;
3. mapped I/O with the data cache off;
4. item reuse after reclaim;
5. whether `fsync(2)` reaches a per-item handler, and whether its error
   reaches the application, and that a notification of a failed
   close-time store (L12, point 4) reaches the user's session; and a
   write followed by `close` without `fsync(2)` after a failed store,
   which must leave a recovery record and a notification (O41, point 6);
6. the `nosuid,nodev` equivalent. If FSKit offers none, the macOS layer
   does not ship (L3, point 5);
7. signing, notarisation and the FSKit entitlements for a first-party
   extension installed outside the Mac App Store (ADR 0001 assumes this);
8. whether an extension enabled per user can be mounted at boot, and
   whether a per-user `/afs` link is possible;
9. that the signed, notarised, sandboxed extension connects to the launch
   daemon running under its dedicated user over the adapter endpoint
   (C45), and that the daemon's checks of its code signature and peer
   succeed (C45, point 3);
10. a reply revoked after it has crossed the adapter endpoint and before
    the extension hands it to the kernel: it is not served; and, with the
    epoch advance withheld from the extension, the revocation is not
    reported complete and no reply issued after it was recorded is
    served (C16, point 6);
11. a `revoke` that fails while the item is mapped, and that the layer
    then follows C15's rule for macOS (C15, point 7);
12. what the extension can do when it loses the daemon (L14, point 5):
    revoke its items, or fail the mount;
13. that a new fault after revocation reaches the daemon and fails, and,
    where it does not, which demonstrated mechanism refuses the affected
    mappings; with none, macOS 27 does not qualify for mapped I/O
    (C15, point 5);
14. whether a metadata-only descriptor of a directory exists for a caller
    who holds the administer right without read or lookup (L35,
    point 6);
15. the account-instance evidence of C41, point 4: the user name, and
    the account's UUID where the system has one, as the daemon sees them
    (C41, point 6).

Why: these are the points where Apple's documentation is silent or where
only third-party reports exist `[DRV-M11]`. L3, L8, L9, L10, L12 and L14
depend on the answers.

### L14. macOS hosting

**Working.** Revisit if: device management gains a way to enable an FSKit
extension, or the gate shows the extension cannot reach a system daemon.

1. `ironafsd` runs as a launch daemon under a dedicated user, with a minimal
   privileged mount step.
2. The extension lives in the app's container, sandboxed, with the hardened
   runtime.
3. A per-user launch agent handles token renewal and notifications.
4. Distribution is a signed, notarised download (ADR 0001).
5. **When the extension loses the daemon** (an exit, a crash or a
   restart), it revokes every item it has handed to the kernel, or fails
   the mount, so that no open survives without a fresh binding (C11,
   point 2; C45, point 3). Which of the two the extension can do is a gate
   item (L13, point 12).
6. A planned upgrade drains as O33 says, as on Linux (L7, point 6).
7. The launch agent runs `aklog` again when the daemon's instance id
   changes (C42, point 3).
8. **The control socket.** The privileged install or launch step creates
   the socket's directory, owned by the daemon's dedicated user and not
   writable by others. The daemon itself creates the socket there; the
   launch system does not hold it for the daemon. A sender checks that the
   peer's uid is the daemon's user (C43, point 4), as on Linux.

Why: `[MAC-063]` `[DRV-047]`. An administrator must approve the daemon
`[MAC-M27]`; device management can pre-approve it, but enabling the
extension may still need each user (`[MAC-M28]`, corrected form). Point 8
is our own rule: it keeps the peer that a sender checks the daemon's own
user, and keeps any other process from creating the socket while the
daemon is down.

### L15. Windows is SMB first; the Cloud Files replica is a later companion

**Working.** Revisit if: the spike shows assumption A3 or A4 false on every
route, or application compatibility or coherency (L22) cannot be met. Then
the replica (L27) becomes the Windows surface. If both fail qualification,
Windows has no shipping surface until one passes, and the release notes say
so.

1. The first Windows surface is an online path, served by IronAFS's own
   SMB 3.1.1 server on the loopback address and reached through Windows'
   own SMB client: `\\afs\<cell>\...` where the name `afs` is admitted
   (L20, point 3), and otherwise the literal-address form
   `\\127.0.0.1\<cell>\...`, which the session agent presents through a
   drive mapping or a shortcut.
2. Every request arrives in an authenticated SMB session and goes through
   the same core authorisation path as on Linux.
3. With this surface, Windows uses the core's normal disk cache (C18,
   C27).
4. A Cloud Files replica may follow as an optional offline companion. It is
   not built first.
5. No driver, device or third-party redirector is installed.

Why: this is the only driverless route that enforces live per-user AFS
rights at every open, because the other Microsoft-shipped surfaces name no
user `[WIN-M48]` `[WIN-148]` or serve hydrated data without asking
`[DRV-014]`. UNC paths also reach services and elevated processes, which
drive letters may not `[WIN-193]`. The cost is the largest single piece of
work in the project: an SMB server of our own. That a full authenticated
session works through the routes is assumption A3; protection of the
back-end port is A4.

### L16. Routes from Windows' SMB client to the loopback server

**Working.** Revisit if: route qualification (L41) fails for a route on a
target build, a feature update breaks the bind order, or Microsoft documents
a per-name port policy that makes route 1 deployable by policy alone.

1. **Route 3 (port 445 on loopback).** Windows' built-in port proxy listens
   on `127.0.0.1:445` and copies bytes to IronAFS's server on a high
   loopback port. Windows' own SMB server keeps its listeners. Where the
   name `afs` is admitted (L20, point 3), a hosts entry maps it to
   `127.0.0.1` (L21); elsewhere the route is reached by the literal
   address, which `[WIN-183]` expects to reach IronAFS rather than
   Windows' own server (measured in L22, point 5). Route 3 is the only
   route for Windows 10.
2. **Route 1 (alternative port).** On Windows 11 builds that support it,
   an explicit mapping, which the client's alternative-port policy must
   allow, sends `\\afs` to IronAFS's server on another port.
3. On Windows 11 the choice between routes 1 and 3 is made by
   qualification. Both are built behind one interface.
4. **Route 2** (IronAFS's server on `127.0.0.1:445` with Windows' own SMB
   server disabled) costs the machine its inbound file sharing. It is an
   explicit opt-in fallback only, and no matrix row rests on it.
5. The proxy only copies bytes, so negotiation, authentication and signing
   run end to end between Windows' client and IronAFS's server. IronAFS
   never passes a session on to another server.

Why: Windows 10 has no alternative ports `[WIN-151]` `[WIN-165]`, and SMB
3.1.1 cannot run over port 139 `[WIN-169]`. Route 3's transport is measured
to the first packet on three builds `[WIN-153]` to `[WIN-159]`. Route 1:
`[WIN-162]` `[WIN-163]` `[WIN-185]`. A second loopback address is refused
`[WIN-161]`. Passing sessions on would relay authentication `[WIN-172]`.
The full session is A3.

### L17. Over SMB, authority is the session's own account

**Firm.**

1. The authority for a request is the account authenticated in the SMB
   session that carries it. An operation on an open uses the session that
   opened it. AFS tokens are found by the credential context of that
   session's logon session (C40, C41), never by account name alone, so
   they are the tokens that user's own `aklog` set in that logon.
2. **Alternate credentials** come in two forms, with different outcomes:
   - a process started in a separate local logon of another account (for
     example with `runas`) authenticates in NTLM local mode (point 4) as
     that account. That logon is its own context, with its own tokens:
     none, until `aklog` runs there;
   - explicit credentials given for the SMB connection itself (for example
     `net use` with another user, or `runas /netonly`) produce a network
     logon. Over NTLM that is refused (point 4). Over Kerberos it is
     accepted only under point 4's Kerberos rule, with the outcome the
     spike states (L22, point 1), and is refused until then.

   Elevated and unelevated processes of one interactive logon are one
   context and share its tokens. Two logons of one account are two
   contexts (C41).
3. Only SMB 3.1.1 is accepted, with signing in force. Guest, anonymous and
   unsigned sessions are refused.
4. **Accepted authentications.** Over NTLM, only local mode, where the
   resulting logon belongs to the caller's own existing logon session on
   this machine. An NTLM network logon, which is what a relayed handshake
   produces, is refused. Over Kerberos, only a ticket for a service name
   bound to this machine's IronAFS service, and only under a scheme the
   spike has measured (L22).
5. A context the daemon cannot prove is refused.

Why: this is what "per user" means on Windows, and it is the same rule as
on Linux: the OS attests the caller, the core decides. A service name can
belong to one account only `[DES-073]`, so one shared name cannot serve a
fleet; that is why Kerberos waits for a measured scheme. Signing keys come
from the authenticated session `[DES-072]`.

### L18. SMB authentication runs in the daemon; the front end holds transport keys and session capabilities, no AFS tokens and no cache

**Firm.**

1. The SMB front end is its own process, `ironafs-smbd`, under its own
   virtual account. It parses everything that arrives before
   authentication. It holds no AFS tokens and no cache.
2. The front end passes the opaque security blobs of SESSION_SETUP to
   `ironafsd`. The daemon runs the OS's authentication interface, applies
   L17, and returns an opaque, session-scoped capability and the session
   key.
3. The front end cannot mint a capability. Every later request carries the
   capability, never a user identifier.
4. The front end owns the pre-authentication transcript, derives the
   signing and encryption keys, and verifies and signs every message.
5. The daemon records the trees and opens created under each capability and
   checks, before dispatch, that a request's session, tree and open belong
   together. Re-authentication follows point 10; logoff follows point 11;
   a disconnect revokes the capability.
6. The daemon sees and audits every authentication outcome, failures
   included (O9).
7. A test presents an open from one session on another; it is refused.
8. **The daemon-to-front-end channel.** The daemon's replies and its
   invalidations for a binding reach the front end on one ordered stream
   over the adapter endpoint (C45, point 2; C17). The front end applies
   C16's submission check for out-of-process bindings (C16, point 6)
   immediately before it writes a reply to the SMB connection, and on an
   invalidation follows C17, points 2 and 5.
9. A test revokes a context after a reply has crossed the adapter endpoint
   and before the front end writes it to the SMB connection. The reply is
   not served. A second test withholds the epoch advance from the front
   end: the revocation is not reported complete, and no reply issued
   after the revocation was recorded is written (C16, point 6).
10. **Re-authentication.** The daemon verifies that a re-authenticated
    session belongs to the same credential context (L17, point 1). If it
    does, the session's trees and opens are rebound to the new capability;
    if not, the re-authentication is refused. Which keys are used after it
    follows \[MS-SMB2\] and is confirmed in the spike (L22, point 1).
11. **Logoff on a live connection.** A logoff retires that session's
    capability, its keys and its opens, while the connection and its other
    sessions continue.
12. The daemon also revokes a session's capability when the operating
    system reports that its logon session has ended, whether or not the
    front end reports a logoff.
13. The front end zeroes the session key once the signing and encryption
    keys are derived from it, and zeroes the derived keys when the session
    ends (O28).

Why: a parser bug before authentication then reaches a process with no
tokens and no cached data. **Compromise boundary:** the front end holds the
keys and capabilities of every active session, so a compromised front end
can read and forge traffic within all sessions open at that time. It cannot
act as a user it names, reach a session that was not open, or reach another
session's opens (C45, point 4; O23). Points 11 and 12 keep that set no
larger than the sessions actually in use. Virtual accounts: `[DES-074]`.

### L19. SMB scope

**Working.** Revisit if: the spike's captures show a client build needs a
command or behaviour outside this scope, or acceptance evidence supports
granting leases (L22, point 4).

1. The server is written from Microsoft's open specifications (\[MS-SMB2\],
   \[MS-SRVS\]) and from captures of what each target Windows build sends.
   Nothing comes from another SMB implementation's source.
2. Shares: one per cell, plus `IPC$` with the minimal share listing, so
   that listing cells at `\\afs` works in Explorer.
3. Connection mechanics: the 3.1.1 negotiate contexts, credits and
   multi-credit requests, compounds, interim and asynchronous responses,
   cancel, and session, tree and open lifetimes.
4. No durable or persistent handles: a reconnect reopens.
5. Leases and oplocks are declined, and each share is flagged for no
   offline caching. Without leases the front end has no way to recall a
   client's mapped views; what bounds them after a revocation is C15,
   point 8, measured in L22, point 7.
6. Commit: CLOSE commits with reason `close`; FLUSH and write-through
   writes commit as on Linux (L6).
7. The per-open SMB file id is generated per open. The file's own identity
   is reported from the core's stable key (C11).
8. A replacing rename follows C10, point 5, with Windows' share modes
   applied first (L33, point 13). Delete-pending and delete-on-close
   defer the server removal to the last close, with no rename. A request
   for immediate removal that IronAFS cannot deliver is refused
   explicitly (C10). The delete right is checked when delete-pending is
   set. The deferred removal is bound to the
   setter's credential context, the parent directory, the file id and the
   volume's incarnation, and runs under that authority with a live check
   of its rights at the last close. At the last close the core re-reads
   the entry and confirms its file id immediately before the removal, as
   C10, point 2, does; a mismatch is quarantined, and the window between
   check and removal is the same stated residual. If that authority is
   gone, or the
   removal fails, the removal is kept as a pending record for the setter
   (`afs pending`, C29) and reported through O35's side channel. It is
   never retried under another session's authority.
9. Byte-range locks and share modes go to the core's lock service (L33).
10. Encryption is supported, so a site can require it. Cryptography comes
    through the crypto interface (C5).
11. Before the server is built, every command and control code the captures
    show is listed with its outcome: implemented, or refused with a stated
    status. That list is the specification for the SMB front end, and the
    captured request set becomes its baseline acceptance test.
12. Every reply the front end writes, multi-credit and compound responses
    included, passes C16's submission check as C16, point 6 states it for
    partial submission: fencing is at whole-message boundaries, a message
    whose write has started is completed, and when the rest of a response
    must be stopped the connection is torn down (L18, point 8).
    Invalidations reach the front end on the channel of L18, point 8.
13. AFS symbolic links are resolved by the core and never presented as
    reparse points (L32, points 2 and 7). Delete, rename and
    delete-on-close below a resolved directory link are refused (L32,
    point 8).

Why: `[DES-070]` `[DES-071]`. With no leases and no client caching, a
callback break can reach the next open or read without a recall round trip.
Whether the client honours that fully is a gate measurement (L22).

### L20. The loopback endpoint is contained, or the route is off

**Firm.**

A route is enabled on a machine only while all of these hold there. They are
checked independently of the daemon and re-checked when policy refreshes.

1. The server listens on loopback only and binds exclusively.
2. While IronAFS is stopped or being upgraded, no other local process can
   take any listener the route uses, the back-end port included. The path
   then fails closed.
3. The path never resolves off the machine. A route is admitted under
   the name `afs` only where qualification shows that no authentication
   mechanism the row supports makes an attempt to another machine under
   that name: NTLM, and, on rows that are domain-joined or claim
   Kerberos, Kerberos too. That holds even with the hosts entry removed or
   overridden, whether through DNS, the configured suffix search, LLMNR or
   NetBIOS name resolution, and over IPv4 or IPv6 (L41, point 9). The
   NTLM mechanism does not cover Kerberos; such a row shows its own
   Kerberos result or uses the literal address. Otherwise the route is
   admitted only under the literal loopback address, which involves no
   name resolution.
4. The server refuses tree connections to any name it was not configured
   with.
5. Limits on bytes and connections apply before authentication.
6. A machine that cannot pass is not given the SMB surface. An installer
   check or a diagnostic warning is not a substitute.
7. The component that disables a route on a policy refresh removes the
   port proxy and the Windows Filtering Platform rules together, so no
   back-end port is left unprotected behind a live proxy. It is named in
   L23, point 1.
8. **Disabling and uninstalling.** When a route is disabled on a policy
   refresh, the hosts entry and the NTLM restriction stay, so saved paths
   under the name fail closed on loopback. On uninstall containment ends:
   the uninstaller removes the drive mappings and shortcuts the agent
   created and restores settings (L21, point 5), and the administrator's
   guide states that saved paths under the name then resolve as any name
   would.

Why: IronAFS does not control how Windows' SMB client resolves the name or
when it starts authentication. A misresolved `afs`, or a rogue listener on
the port, would receive a user's authentication exchange. The candidate
mechanism for the name is the SMB client's NTLM blocking, available from
Windows 11 24H2 `[WIN-190]`. Microsoft describes it as blocking NTLM for
remote outbound connections
([SMB NTLM blocking](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-ntlm-blocking)),
and its exception list takes IP addresses, NetBIOS names and fully
qualified names `[WIN-190]`. An exception for the name `afs` would
re-allow NTLM to a misresolved `afs`, so a qualified configuration has no
exception for the name: either no exception at all, if NTLM to IronAFS on
loopback is not counted as remote, or an exception for `127.0.0.1` only.
Which of the two holds is measured (L22, point 2), not assumed. Windows
10 has no such mechanism, so it uses the literal address, which is
contained on every build without any machine-wide NTLM change. A persistent port
reservation does not stop another component `[WIN-187]`; that a Windows
Filtering Platform rule does is assumption A4. The hosts entry's reach:
`[WIN-182]`.

### L21. Machine-wide Windows settings each have one owner

**Working.** Revisit if: route qualification shows a setting is not needed,
or Windows offers a per-application way to do what a machine-wide setting
does here.

1. The name is `afs` by default and is a configuration value: a site with a
   real host or service of that name chooses another. Name and port are the
   same for every user on the machine.
2. On rows where the name is admitted (L20, point 3), the hosts entry is
   written by the installer and removed by the uninstaller, unless the site's policy says it manages hosts entries; then
   the installer only validates it. A missing or conflicting entry fails the
   install visibly.
3. Route 3's port-proxy rule and service dependency are made by the
   installer with the administrator's consent. Route 1's policy is deployed
   by the site's policy tooling, or written by the installer only with
   explicit consent.
4. Any machine-wide relaxation that authentication to the loopback name
   turns out to need is listed by the spike and is deployed the same way.
   IronAFS changes no machine-wide SMB client cache setting by default.
5. For every setting it changes, the installer records the previous value
   and the uninstaller restores it.
6. `afs diagnose` reports each setting and its owner. A combination IronAFS
   does not support is a failed health check, never worked around.

Why: these settings affect the whole machine, so an administrator must be
able to see who set each one and get the old value back. The bind order
rests on one service dependency `[WIN-181]`.

### L22. What the Windows spike must show

**Working.** Revisit if: a target build changes (each item is re-measured
per matrix row, L41), or an item cannot be met and the documented guarantee
is narrowed instead.

The spike runs in public, on the builds the matrix names, with a prototype
front end. It settles A3 and A4.

1. **Identity.** Two local accounts with different AFS rights, each after
   its own `aklog`: through its signed session, each reads what its rights
   allow and is refused the rest. One account with two logon sessions,
   with `aklog` run in one only: the other sees no token. A process in a
   separate local logon of another account gets that logon's own context,
   with no token until `aklog` runs there; explicit credentials on the SMB
   connection over NTLM are refused (L17, point 2). Elevated and unelevated
   processes of one user get that user's result. A daemon restart followed
   by a new `aklog` restores each result. Each logon type (local NTLM,
   network logon, a local or domain Kerberos scheme, both forms of
   alternate credentials) has a stated expected outcome. Re-authentication
   on a live session, and the keys used after it, are captured (L18,
   point 10).
2. **Security.** A relayed handshake is refused. L20 holds, with the
   mechanism for point 2 named. Client NTLM blocking `[WIN-190]` is
   measured both on and off, including whether NTLM to IronAFS on
   loopback counts as a remote outbound connection under it, and which
   exception, if any, it then needs (L20, point 3).
3. **Application compatibility.** Explorer browsing of `\\afs` and each
   cell, copy, rename, an office suite's open and save, a compiler and a
   version-control tool; Offline Files confirmed off; a recursive delete
   of a link's name that resolves to a directory, and of a directory
   above it, by Explorer, `rd /s`, PowerShell's `Remove-Item -Recurse`,
   .NET's recursive `Directory.Delete` and `robocopy /MIR`, with the SMB
   requests each sends, the error it shows, whether it continues, and
   what it changed: the target must be unchanged, and whether the link
   itself can be removed is recorded (L32, point 8).
4. **Coherency.** A callback break is visible within one second of its
   arrival to the next open, read, lookup and enumeration, to a mapped view
   opened after the break, and to an active directory watcher (the watch
   completes with the change, or tells the client to rescan). A view mapped
   before the break is measured too; if the client keeps its pages, the SMB
   guarantee is narrowed to "a mapped view sees changes after it is
   reopened". **Reopened** means that every handle and every view of the
   file on that client was closed first. A view mapped before the break
   must then see the new data once it is reopened.
5. **Route.** Identity, dialect and signing through the port proxy; the
   bind order across restarts, a cumulative update and an uninstall, with
   Windows' own sharing checked after each; the two unmeasured name
   expectations of `[WIN-183]`.
6. **Lockout.** What Explorer's retries do against a stale `\\afs` entry
   with a wrong or expired credential, so that IronAFS never locks an
   account out.
7. **Revocation of mapped views,** measured apart from point 4's
   freshness. After a context's authority is revoked (C15), new page
   faults on a view it mapped before the revocation are measured: whether
   they reach IronAFS's SMB server and fail, or are served from the
   client's own pages. A live view is measured across a handle close and
   across session teardown. The result decides whether C15, point 8's
   bound holds on that build, or C15, point 5's rule applies (a
   demonstrated mechanism refuses the affected mappings, or the row does
   not qualify). Where teardown is used to stop access, the outage it
   causes for that session is documented.
8. **Account instance.** The account SID as C41, point 5, uses it, and
   the end of a logon session as the daemon sees it (L18, point 12), are
   confirmed on each build (C41, point 6).

Why: the public measurement stops at the first packet `[WIN-154]`. Each
item is a point where the SMB surface could be unsafe or unusable, and each
is cheaper to learn before the server is written.

### L23. Windows hosting

**Working.** Revisit if: Windows documents a supported way for a
non-LocalSystem service to start a process in a user's session.

1. `ironafsd` runs under its own virtual account. Only the installer is
   elevated. The component of L20, point 7, that disables a route on a
   policy refresh is a scheduled task that the installer's elevated
   helper registers, not running as LocalSystem; it is the daemon's own
   service account instead if the spike shows that account can manage the
   port proxy and the Windows Filtering Platform rules.
2. A per-session agent runs as the user: it obtains tokens through the OS's
   Kerberos interface, repeats that after a daemon restart, shows
   notifications, and handles logoff.
3. The agent is started by a Task Scheduler logon trigger.
4. After every daemon restart, the agent repeats C43's check of the pipe
   server's process identity before it sends anything (C43, points 3
   and 4).

Why: `[DES-074]`. A service that is not LocalSystem cannot obtain the user's
token to start the agent itself; a logon trigger is our choice among the
documented ways to start a program at logon (`[DES-075]`, corrected form).
The daemon never reads a user's credential cache (C42).

### L24. The Windows name table

**Firm.**

Every Windows surface shows a character that Windows forbids in a name as
one code point in the Private Use Area: U+F000 plus the character's code.
The table is exactly this, and these 41 code points are the reserved set:

| In the AFS name | Shown as |
|---|---|
| control characters U+0001 to U+001F | U+F001 to U+F01F |
| `"` | U+F022 |
| `*` | U+F02A |
| `:` | U+F03A |
| `<` | U+F03C |
| `>` | U+F03E |
| `?` | U+F03F |
| `\` | U+F05C |
| `\|` | U+F07C |
| a space as the last character | U+F020 |
| a period as the last character | U+F02E |

1. The two trailing mappings apply to the last character only, and never to
   the names `.` and `..`.
2. A raw name that already contains a reserved code point, or the escape
   prefix (point 5), is escaped, so decoding is unambiguous.
3. The table is IronAFS's own. It coincides with the Services for
   Macintosh table for the control characters (U+F001 to U+F01F) and
   differs elsewhere: that table puts its punctuation at U+F020 to U+F027
   and a final space and period at U+F028 and U+F029 `[DES-068]`, so
   U+F020 and U+F022 mean different characters in the two tables.
4. The table is published in the specification for the name codec and has
   one implementation, in the contract crate, used by every Windows
   surface. That specification includes frozen round-trip test vectors:
   control characters, each forbidden punctuation character, trailing
   spaces and periods, `.` and `..`, and raw names that already contain a
   reserved code point or the escape prefix.
5. **The escape prefix** is one code point outside the reserved set. Every
   occurrence of it in a raw name is itself escaped, as point 2 says, so a
   presented name decodes to exactly one raw name. Its code point and the
   exact escaped forms are fixed in the specification for the name codec,
   with the forms of L25, points 1 and 3, and are then part of this
   format. Until that specification is frozen the escape grammar is
   provisional, and no build that presents escaped names is released.

Why: users' saved paths and scripts will contain these code points, so the
table is a format. For every forbidden character except the backslash,
adding U+F000 is the conversion Cygwin applies when it creates or accesses
a file (context 34), so Cygwin reaches names containing those characters by
their real names. That compatibility does not extend to the two trailing
mappings: Cygwin maps trailing spaces and periods only on some filesystems
or with one mount option `[ADR-020]`. The Services for Macintosh table has
no colon and places punctuation and the trailing characters differently
`[DES-068]` (context 34). Trailing spaces and periods: `[DES-069]`.

### L25. Windows names are total, reversible and never destructive

**Firm.**

1. **Total.** Every AFS name has a presentation. Device basenames, names
   that are not valid Unicode, and names that collide under Windows'
   comparison each get a distinct, stable, reversible form. A form too long
   for Windows gets an alias from a reserved alias namespace, bound to the
   full raw name.
2. **Never silently partial.** No entry is omitted from a listing. If an
   entry cannot be presented safely, the listing fails explicitly.
3. **Reversible.** A presented name or alias decodes to exactly one raw
   name. An alias is derived from the raw name, never from listing order,
   and can never come to mean another raw name. A missing or stale alias
   binding fails the lookup. An alias names a raw name, not a file: after
   the name is replaced, it names the new file, as the name itself does.
   Operations through a handle use the identity of the open (point 5;
   C7, point 3; C13).
4. **Case.** An exact match wins only when no case-insensitive collision
   exists in that directory. Otherwise a unique case-insensitive match is
   used, and an ambiguous one is not found. While a collision exists, only
   the suffixed forms of point 1 resolve; the unsuffixed form resolves to
   nothing rather than to a different file.
5. **Never destructive.** The case-insensitive fallback serves only lookups
   and opens that cannot destroy data. A delete, both names of a rename
   that replaces its target, any other rename's source, an open that
   truncates or overwrites, and any open that grants write or delete
   access, including access used later, need an exact name or an alias.
   The handle forms of disposition (delete-on-close) and rename (C7) are
   covered through the open that granted their access; a rename target is
   checked when the rename is made. An open that asks for the maximum
   allowed access through a case-insensitive match is granted no write or
   delete access. Every component of a destructive operation's path, not
   only the last, needs an exact name or an alias.
6. **New names.** SMB accepts only canonical encodings for names it
   creates. Before any operation that may create a name (a create with the
   create, open-if, overwrite-if or supersede disposition, a directory
   creation, or any rename target), the name is compared case-insensitively
   with the directory's entries. If an entry matches only
   case-insensitively, the operation fails rather than create a collision.
7. **The server is never rewritten.** A name on the fileserver is never
   changed to suit Windows.

The exact forms for points 1 and 3 are fixed in the specification for the
name codec before the first Windows release, and are then part of this
format. Before that release, the decoding of the table by common Windows
tools (shells, archive tools, the Linux subsystem) is measured and
published.

Why: IronAFS treats AFS names as case-sensitive byte strings, while
Windows applications are told not to assume case sensitivity
([naming files](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)).
Without points 3 to 5, a path that once named one file could delete or
overwrite another after a collision comes or goes. Reserved names:
`[DES-069]`.

### L26. Windows metadata

**Working.** Revisit if: application-compatibility testing (L22) shows a
common program needs a different attribute or descriptor.

1. The read-only attribute is set only when the file's owner write bit is
   clear. It is never used to show missing rights.
2. A user is never given the right to change a descriptor or take
   ownership.
3. A drive letter is optional and per session. The UNC path is the
   documented name.

Why: the attribute is a property of the file, not of the caller; rights are
answered per request by the core. Drive letters are per logon session
`[WIN-192]`.

### L27. The Cloud Files replica, if it is built

**Working,** except points 3, 4, 10 and 11, which are **Firm.** Revisit
if: public measurement contradicts assumption A6 or A7, the SMB surface
fails qualification (the replica is then scheduled first, still behind
point 11), or a published AFS-3 specification changes A9.

1. The replica is an optional offline companion: per-account sync roots,
   each bound to one logon session at a time (L28, point 1), under a
   provider that runs as a service account, never as the user.
2. It uses the same name projection (L24, L25). Local changes are accepted
   only where point 3 allows them.
3. **No automatic upload.** An upload of a local change is a deferred
   store (C28, point 3): nothing reaches the server until the user
   publishes it, after seeing any conflicts, and a conflict keeps both
   versions. The replica accepts local changes only where a capture
   mechanism is demonstrated that keeps an immutable copy of each
   acknowledged change before a later change can replace it, across a
   crash. Otherwise the replica is read-only, or accepts changes only
   through an explicit IronAFS staging action whose acknowledgement
   IronAFS controls. How changes are captured and reconciled is specified
   in the replica ADR (point 11).
4. **No second copy.** Clean cached data has no copy beside the hydrated
   files. Only changes waiting under point 3 are kept apart from them.
5. **Offline reads** follow O34, point 6: they meet O22, point 3, or are
   an explicit mode the user enters. How revalidation and dehydration
   work is specified in the replica ADR.
6. **AFS symbolic links** are never presented or followed as a Windows
   link (L28, point 5). How they appear, and how one is resolved on
   request with current authorisation, is specified in the replica ADR.
   Applications that need live traversal use the SMB path.
7. How locks behave on the replica, where the platform has no lock
   callback `[WIN-M43]`, is specified in the replica ADR.
8. It is built only after the replica ADR (point 11) is accepted and
   public measurement on the matrix's Windows builds settles A6 and A7.
   Its build and ship gates, and their tests, are in that ADR.
9. Until then this ADR cites only Microsoft's documentation for it. A
   hydration policy, if named, is progressive or full `[WIN-M44]`.
10. **Fail closed.** Where L28's isolation cannot be enforced or has not
    been demonstrated on a machine or build, the replica is disabled
    there.
11. **A replica ADR, reviewed as class S and accepted, is required before
    the replica is built; it specifies the mechanisms.** These include
    how a root is bound to a logon session and rebound, the descriptors
    and privileges that protect each object, the checks on objects users
    create inside a root, change capture and reconciliation, offline
    revalidation and dehydration, link markers, locks, recovery from a
    provider that fails, and the tests of each.

Why: hydrated files are served without asking the provider `[DRV-014]`, so
live AFS rights cannot be enforced on them; that is why the replica is a
companion and not the first surface. The platform names no user
`[WIN-M48]`, so identity is per root (A7). It reports no write or lock
`[WIN-M43]`, so the provider sees a change only after the application
has finished, and no application waits on the store: that is C28,
point 3's case. Uploads wait for the user because AFS-3 has no
conditional store (A9). The mechanisms rest on NTFS and Cloud Files
behaviour that Microsoft does not document (A6, A7); they are settled
by measurement in their own ADR, not here.

### L28. Replica isolation is per object and absolute

**Firm.**

If the replica is built:

1. **Per-context isolation.** No hydrated byte or replica metadata is
   readable through NTFS by any principal other than the logon session
   the root is bound to, at any time: before the provider starts, while
   it is stopped, and during retirement or rebinding included. A second
   concurrent logon of the account gets no replica access, with a message
   that names the reason.
2. **Rebinding.** A root is rebound only to the same account instance
   with an unchanged credential generation (C18, point 7). Otherwise its
   data is removed before the root serves anyone.
3. **Objects users create.** No user-created object inside a root is
   read, published, hydrated, or has its owner or descriptor changed
   until the provider has verified, on the open object, its identity,
   that it has no name outside the root, and that no handle opened before
   protection remains. An object that cannot be verified is quarantined.
4. Users never hold the right to change a descriptor or take ownership
   inside a root. Whatever privileges the provider holds for this stay
   with its service account, inside the same compromise boundary as the
   daemon (O23).
5. **Links.** No Windows shortcut, NTFS symbolic link or junction is ever
   generated from cell data. The provider never presents or follows a
   link or reparse point from cell data, nor acts through one a user
   created inside a root.
6. If a marker or an entry cannot be published safely, population fails
   with the directory and the reason, and the directory is not marked
   complete.
7. Where points 1 to 6 cannot be demonstrated on a build, the replica is
   disabled there (L27, point 10). The mechanisms that meet them, and
   their gate cases (the provider not yet started or stopped, a
   restricted replacement token, a logon SID recycled after a restart, a
   hard link whose other name is outside the root), are in the replica
   ADR (L27, point 11).

Why: a placeholder is created without a descriptor `[WIN-047]`, and after
hydration the provider is no longer asked `[DRV-014]`, so what NTFS
enforces on each object is the only barrier. Two logons of one account
are two contexts (C18, point 2; C41), and NTFS serves hydrated files to
either without asking the provider, so point 1 binds a root to one logon
session. A link generated from a cell's data, or one a user planted
inside a root, could point the provider or a local program anywhere on
the machine.

### L29. Names on Linux and macOS

**Firm,** except the choice in point 3 of a case-sensitive macOS volume
with a lookup fallback, which is **Working.** Revisit that choice if:
acceptance tests show a common macOS application fails on a case-sensitive
volume, or on the lookup fallback.

1. The core keeps a name's raw bytes and never normalises.
2. Linux shows the bytes.
3. macOS presents a case-sensitive volume, with a lookup fallback that is
   insensitive to Unicode normalisation and never rewrites the server's
   name.
4. **The fallback is never ambiguous.** Exact bytes win only when no other
   entry in the directory is canonically equivalent. Otherwise a unique
   equivalent match is used, and an ambiguous one is not found.
5. **The fallback is never destructive.** An unlink, a removal, an open
   that truncates, any open that grants write access (including access
   used later through its descriptor), both names of a rename that
   replaces its target, and any other rename's source need an exact
   match. An open made through the fallback is granted read access only.
   Every component of a destructive operation's path, not only the last,
   needs an exact match (as L25, point 5). A create (`O_CREAT`)
   that would make a new entry, and a rename that does not replace, fail
   if the new name is canonically equivalent to an existing entry, rather
   than create an equivalent sibling.
6. Lookup and mutation use the same resolved raw name: an operation acts
   on the entry its lookup found, never on a name normalised again.
7. Acceptance tests cover a directory where an NFC and an NFD form of one
   name coexist, a write through a descriptor opened by an equivalent
   name, and a rename through an equivalent parent name.

Why: IronAFS treats AFS names as bytes. A client that normalised them
could not open some files another client created. Points 4 to 6 apply
L25's rules for case to Unicode normalisation, so a path that once named
one file never deletes or overwrites another.

### L30. A displayed owner never grants anything

**Firm.**

1. The owner a layer shows is display only. Tests verify that it never
   grants AFS access, and never grants administrative rights through a
   native permission check.
2. No AFS identity is ever shown as root, a system account or a service
   account.
3. On a mount shared by several users, the mapping is the same for every
   caller.
4. **Groups** follow the same rules. A group a layer shows is display
   only. No AFS group or group field is ever shown as gid 0, a system
   group or a group that grants local privilege; anything unmapped is
   shown as a reserved unmapped group.
5. Showing every caller as the owner is never offered.
6. An object is shown as the local user's own only when its AFS owner is
   the view's verified AFS identity in that cell, and only on a view bound
   to one verified credential context (L31, point 4).
7. An **eligible local account** for point 6 is one the operating system
   classes as an ordinary user: never uid 0 or an administrator's built-in
   account, and never a system or service account (point 2).
8. When a view's credential binding changes, every owner and group it
   showed under the old binding is invalidated in the kernel (C17) before
   the view shows another.

Why: native tools trust an owner field. An owner that changed with the
caller, or mapped to a privileged account, would turn a display choice into
a local privilege.

### L31. Owner mapping

**Working.** Revisit if: sites report that the unmapped default breaks a
common tool that an explicit rule cannot fix.

1. IronAFS's own commands show the cell-qualified AFS owner.
2. A native uid or SID comes only from an explicit per-cell rule
   (alignment, name or table).
3. Otherwise, and for anything L30 forbids, a reserved unmapped id for the
   platform is shown, and for groups a reserved unmapped group (L30,
   point 4).
4. **"Mine as mine"** is an opt-in, offered only on a view bound to one
   verified credential context: a per-user FSKit mount, or an SMB view once
   that surface has qualified. An object is shown as the local user's only
   under L30's conditions (L30, points 6 and 7). It is tied to the
   credential binding and dropped when the binding changes (L30, point 8).
5. Showing every caller as the owner is not offered (L30, point 5).

Why: Git by default refuses a repository owned by someone other than the
current user
([git-config, `safe.directory`](https://git-scm.com/docs/git-config)), so a
"caller owns everything" mapping would make it trust other users'
directories. A stable mapping gives the same answer to every tool.

### L32. Mount points, links and special nodes

**Working**, except point 7, which is **Firm** (a security invariant).
Revisit the rest if: an indexer or backup tool on a target OS ignores the
exclusions, or acceptance tests find a traversal loop the bounds miss.
Revisit point 8 if: research into how Windows clients and other SMB
servers handle deletes through server-resolved directory links shows a
safe narrower rule.

1. An AFS mount point appears as a directory. The core crosses it.
2. A symbolic link appears as a symbolic link on Linux and macOS. On SMB,
   the core resolves an AFS symbolic link whose target lies inside
   `/afs`, within point 3's bounds, and presents the target. A link that
   leaves `/afs`, dangles or loops is listed (L25, point 2) and fails on
   open with a stated status, and `afs diagnose` gives the reason. Delete
   and rename of the link's own name act on the link, never on its
   target. What may be done below a resolved directory link is point 8.
   On the replica, L27 and L28 apply.
3. Mount-point and link depth are bounded everywhere. The visited set
   covers mount points and the links the core resolves itself; links the
   kernel resolves rely on the operating system's own hop limit.
4. The root lists cells without contacting them. Lookup, get attributes
   and access on a cell's entry in the root return synthesised attributes
   without contacting the cell (C12, point 2). Contact starts at a read of
   the cell's directory or a lookup of a name inside it.
5. The filesystem type has a distinct name, and the packages install
   exclusions for the system's indexers.
6. A read-only volume refuses writes with the OS's read-only error.
7. **No Windows link from cell data.** No Windows surface presents a
   reparse point, NTFS symbolic link, junction or shortcut made from a
   cell's data, and no link is ever resolved to a target outside `/afs`.
   (L28, point 5, states the same for the replica.)
8. **No destruction through a resolved directory link on SMB.** If a
   directory link was followed to reach an entry's parent, any request
   that would remove, rename or replace that entry is refused with access
   denied, before the namespace changes or a delete-pending state is set.
   This covers every delete disposition, a create with delete-on-close,
   a create that replaces an existing entry, and both ends of a rename,
   including a destination that does not exist yet. Handle-based
   requests use the path recorded when the handle was opened; a rename's
   destination is resolved on its own. Reads, writes and creates there
   work. A link reached with no directory link above it can be renamed
   or deleted as one entry, under the usual access checks, and a
   deferred delete is bound to that entry's identity, so a later rename
   or retarget cannot redirect it. Deleting or renaming inside the
   target uses the target's own path, and this is documented. The core
   enforces the rule because it resolves the link itself. Windows'
   recursive deletes send a separate request per child, deepest first,
   so a recursive delete of a presented link is expected to stop at its
   first child with access denied and leave the target unchanged;
   whether the link itself can then be removed from Windows is measured
   (L22, point 3). Until then the documented way to remove a directory
   link is the `afs` command line.

Why: a file manager or indexer that walks the root must not contact every
cell or loop through mount-point cycles. On Windows, remote-to-remote and
remote-to-local symbolic-link evaluation are disabled by default
([the symbolic link cannot be followed](https://learn.microsoft.com/en-us/archive/blogs/junfeng/the-symbolic-link-cannot-be-followed-because-its-type-is-disabled)),
so a reparse point presented over SMB would fail on a default machine
unless a site relaxed a machine-wide setting, which L21 does not require.
Resolving links on the server and keeping them inside the share is
established SMB server practice: Samba's defaults are
`follow symlinks = yes` and `wide links = no`
([smb.conf(5)](https://www.samba.org/samba/docs/current/man-html/smb.conf.5.html)).
Because no Windows link reaches the client, cell data cannot point Windows
at a local path or coerce authentication to another host.

### L33. Locks, per layer

**Working.** Revisit if: an OS layer gains a lock callback `[MAC-025]`, or
interoperability tests against the test cell show a server rule that differs
from the lock specification.

1. One lock service in the core keeps separate state for POSIX locks, SMB
   byte-range locks, SMB share modes and the AFS server lock.
2. Each front end's ownership and range rules are enforced locally. SMB
   byte-range locks are mandatory among this machine's opens, checked on
   read and write in the core. SMB share modes are enforced locally only:
   they never take a server lock and never need the AFS lock right.
3. Where the server permits, local locks are backed by one whole-file
   advisory AFS lock: a read lock for shared coverage, a write lock for
   exclusive.
4. **Local-only grants.** When the server lock is unavailable because of
   rights or a read-only volume, an otherwise authorised operation gets a
   local-only lock, shown as such in status and an event. A strict mode
   refuses it instead.
5. **No silent loss of coverage.** No conversion releases server coverage
   for a local lock that is still granted, the upgrader's own included. A
   write lock is held until its local set is empty. Server coverage is never
   handed from one user's authority to another's.
6. An exclusive request on a read-only open may get local exclusivity with
   server read coverage, reported as partial (refused in strict mode). On a
   writable open it is refused without the server write lock. It is also
   refused on a writable open while another user sponsors the server read
   lock, since the write lock could then come only from that user's
   coverage (point 5).
7. **Continuity is never assumed.** A failed renewal, a resume after the
   lease could have lapsed, a volume move, a server restart or a clock
   event of C37, point 4, marks the affected local locks lost. After that,
   these operations fail on the handles of that lock owner: read, write,
   truncate, flush, fsync, and any change to its locks. A new acquisition
   never validates old grants.
8. Server conflicts and transport failures are honoured. Contention is
   polled with bounded backoff, cancellation and deadlines.
9. FSKit and the replica have no lock hook, so locks there are local to the
   machine. FUSE and SMB forward to the lock service.
10. **Who keeps the server lock.** A sponsor is a local holder whose own
    status shows the AFS lock right. The server lock is taken and renewed
    over the first sponsor's connection, under that sponsor's authority,
    while that sponsor holds a local lock on the file. A holder without the
    lock right never takes, upgrades or prolongs it. When the first sponsor
    releases its last local lock on the file, loses the lock right on
    revalidation, or its token is lost or replaced by another identity, the
    server lock is released or treated as lost. It is not passed to another
    sponsor (point 5). Point 7 then applies to every lock that relied on
    the coverage, other sponsors' included; local-only locks continue,
    shown as such.
11. Documented limits: AFS-3 server locks are advisory and whole-file, and
    last five minutes unless renewed `[DES-051]`, so disjoint ranges on two
    machines conflict, and a machine that sleeps past the lifetime while
    holding locks loses them.
12. **Renewal margin.** A server lock is renewed well before its lifetime
    ends, counted on the boot clock (C37); we propose at half the
    lifetime. Before every operation that depends on a lock (point 7's
    list), the core checks that the lock's coverage is still continuous.
13. **Replacing rename on SMB.** A rename whose destination is open on
    this machine without delete sharing is refused with a sharing
    violation, as a Windows file server does. Otherwise the rename follows
    C10, point 5. What Windows' own server does in this case is confirmed
    from the specifications and captures of L19, point 11, not assumed.

Why: `[DES-050]` `[DES-051]` `[DES-052]` `[DRV-058]`. Which rights the
server asks for each lock type is recorded in the specification for the lock
service, from the test cell's observed behaviour.

### L34. Where the filesystem appears

**Working.** Revisit if: the macOS gate changes L10, or route qualification
fails for the name `afs` (L41).

1. Linux: `/afs`.
2. macOS: per-user mounts, with `/afs` as L10 decides.
3. Windows: `\\afs\<cell>\...` on rows where the name is admitted, and
   otherwise `\\127.0.0.1\<cell>\...` behind a drive mapping or shortcut
   the session agent creates (L20, point 3); plus an optional drive
   letter. With the replica: a per-account root served to one logon
   session at a time (L28, point 1), a per-session drive letter and an
   Explorer entry.
4. On every surface except the replica: `@sys` and `@cell` substitution in
   the core, with an ordered, configurable system-name list. On the
   replica, `@sys` and `@cell` apply only through the explicit
   resolution of a link the replica ADR specifies (L27, points 6
   and 11). Everywhere: the read-only path by default and the conventional
   dotted path for read-write.
5. The freeze of `/afs` and `\\afs` as defaults is in O38.

Why: existing AFS users, documentation and scripts use these paths. A UNC
path does not depend on one logon session's drive letters `[WIN-192]`
`[WIN-193]`, and the existing Windows client documents `\\AFS\cell\path` as
its canonical name `[WIN-191]`.

### L35. `fs` command paths are resolved by the caller, not the daemon

**Firm.**

1. A command that names a path opens it itself, so the caller's root,
   current directory, mount namespace and permissions apply.
2. It passes the open descriptor (on Windows, a handle) to the daemon over
   the control endpoint (C45).
3. The daemon identifies the node from the descriptor. It accepts it only
   after checking that the object belongs to an IronAFS mount or share and to
   the current volume incarnation. Otherwise it refuses with "not in AFS".
   It also checks that the descriptor's handle is live and of the current
   credential generation, and uses only that handle's authority (C13,
   point 4); this applies to descriptors with an open behind them. Where
   the operating system cannot identify the open behind such a
   descriptor, the command is refused. Point 6 covers metadata-only
   descriptors.
4. The daemon never resolves a path string on a caller's behalf.
5. **Entry-level commands.** A command that acts on a directory entry
   rather than on what it leads to (making, listing or removing a mount
   point) passes the descriptor of the parent directory, opened by the
   caller as point 1 says, and one entry name. The daemon refuses a name
   that is empty, is `.` or `..`, or contains `/`, `\` or NUL. It acts on
   that entry without crossing a mount point or following a link, and
   checks the caller's rights on the parent directory through the
   parent's handle (point 3). One entry name is not a path, so point 4
   holds.
6. **ACL administration.** A caller who holds the administer right
   without read or lookup on a directory may pass a metadata-only
   descriptor of it: on Linux an `O_PATH` descriptor (which needs only
   search permission on the path's directories,
   [open(2)](https://man7.org/linux/man-pages/man2/open.2.html)), on SMB a handle opened for attributes only. For
   such a descriptor the daemon identifies the node, checks that it
   belongs to an IronAFS mount and the current incarnation, and runs only
   ACL commands, under the caller's current credential context (the
   control endpoint's peer, C45, point 1); the server checks the
   administer right when the command runs. The macOS gate (L13,
   point 14) measures whether such a descriptor exists; where it does
   not, the limitation and the repair path (an administrator, or another
   platform) are documented.

Why: a daemon that resolved paths itself would do so with its own view and
privileges, not the caller's: wrong inside a container or changed root, and
a confused deputy at worst. The exact mechanism per OS is in each layer's
specification. Point 5 exists because opening a mount point crosses it
(L32, point 1), and a new or broken mount point cannot be opened at all.

## Supported-platform matrix

This section is part of the decision. No platform has qualified yet. Dates
and versions are as read on 2026-11-19; the linked pages were read again on
2026-11-20.

### L36. What a row means

**Working.** Revisit if: the maintainer's test capacity changes (a row
without a venue cannot be a target), or a second maintainer adds venues.

1. A **target** has a named venue and a planned tier. A **candidate** has
   no support commitment.
2. A row **qualifies** when the exact release artefact passes its layer's
   smoke test (L40) on the named venue. A claim covers the OS, architecture,
   kernel or build, and access mode listed in the release record.
3. The first release requires all four Ubuntu tier 1 rows. Other targets
   become commitments when they qualify and appear in a release's matrix.
4. Failure or missing evidence for an active tier 1 row blocks the release.
5. A tier 2 row with failure or missing evidence is omitted from that
   release's claims and listed as omitted in its notes. Tier 2 gets a build
   and a smoke test per release, fixes that fit the maintenance budget, and
   no performance promise.
6. Each claim ends at its recorded cutoff, or earlier if withdrawn
   explicitly.

Why: IronAFS supports only what can be tested. A row without a venue is a
hope, not a commitment.

### L37. Tier 1 targets

**Working.** Revisit if: a hosted runner image is retired or replaced, a
vendor moves a servicing date, or a row fails qualification.

1. **Ubuntu 24.04 and 26.04, x86_64 and aarch64, on the hosted runners'
   kernel flavour.** The mount smoke runs on every change on the four
   GitHub-hosted labels (`ubuntu-24.04`, `ubuntu-24.04-arm`, `ubuntu-26.04`,
   `ubuntu-26.04-arm`;
   [runner labels](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)).
   Each run records the image, the booted kernel and the active
   confinement. Release jobs install the exact artefacts being shipped.
   Ubuntu's generic kernels are candidates until tested on a native kernel.
   Standard maintenance ends in May 2029 and May 2031 `[PLT-042]`. When the
   24.04 image is retired, that row moves to a self-hosted runner or a
   manual release smoke.
2. **macOS 27 on Apple silicon, FSKit.** Build and unit tests run on every
   change on a hosted macOS 27 image with Xcode 27 or later (label
   `xcode-27`)
   ([image announcement](https://github.blog/changelog/2026-10-15-xcode-27-runner-image-generally-available/),
   [Xcode requirements](https://developer.apple.com/xcode/system-requirements/)).
   On each release, the maintainer's Apple silicon Mac installs and enables
   the extension, exercises two per-user mounts and verifies cache-state
   changes. Later macOS versions are added after they qualify. Apple
   publishes no support window `[PLT-048]`, so the claim is reviewed as
   Apple publishes security updates.
3. **Windows 11 Pro 26H2 x64, workgroup use at first.** SMB qualification
   (L41) and the release smoke run on a disposable Windows client VM on the
   maintainer's x64 test host. Servicing ends in October 2028 `[PLT-054]`.

Why: these are the rows with a venue that can run on every change or every
release. Apple lists only Apple silicon Macs for macOS 27 (`[MAC-018]`,
corrected form).

### L38. Tier 2 targets

**Working.** Revisit if: the Windows 10 row has not qualified by
2027-04-12, its route fails initial qualification, or the Windows 11 online
route is abandoned (the row is then withdrawn); or a vendor moves a date.

1. **Windows 11 Pro 26H2 Arm64, workgroup use at first.** SMB qualification
   and the release smoke run on a disposable Arm64 Windows client guest on
   the maintainer's Apple silicon Mac. The hosted Arm64 Windows runner
   (label `windows-11-arm`,
   [runner labels](https://docs.github.com/en/actions/reference/runners/github-hosted-runners))
   is used for build and unit tests only. It is GitHub's build image, not
   a client installation of this row's edition and build, so it qualifies
   nothing here. Each run records the image it ran on.
   Uses that need Folder Redirection or roaming profiles do not apply to
   this row `[WIN-M55]`. If no Windows 11 SMB route qualifies (L41,
   point 8), this row is withdrawn, as the trigger above says; a replica
   row on Arm64 would be a separate claim that qualifies on its own.
2. **Windows 10 Pro 22H2 x86_64, workgroup use, SMB only.** A disposable
   client VM on the maintainer's x64 test host, with Extended Security
   Updates active and the current cumulative update
   ([consumer ESU](https://www.microsoft.com/en-us/windows/extended-security-updates),
   [commercial ESU](https://learn.microsoft.com/en-us/lifecycle/faq/extended-security-updates)).
   Only route 3 can qualify it. IronAFS's support for it ends on
   2027-10-12, the end of consumer ESU and of commercial ESU's second year.

Why: Windows 10 left general support in October 2025 `[PLT-053]`, but
Pro, Enterprise and Education in commercial use can receive ESU security
updates until October 2028 (commercial ESU, above), and route 3 keeps its
file sharing (context 25). The row has an end date and withdrawal
conditions so that it cannot linger untested.

### L39. Candidates

**Working.** Revisit if: a named venue for a row exists and passes (the row
becomes a target); assumption A1 fails (the RHEL 7.9 row is dropped, and
the static musl row is looked at again, point 5); or A2 fails (the RHEL 7.9
row is dropped).

1. **RHEL 9 and 10, and Debian 13, x86_64 and aarch64:** tier 1 once
   automated jobs on a booted kernel have qualified them. Those jobs need a
   named self-hosted host. Maintenance ends 2032-05-31, 2035-05-31
   `[PLT-028]` `[PLT-029]` and, for Debian 13's full support, 2028-08-09
   `[PLT-040]`.
2. **RHEL 8.10 and Debian 12, x86_64 and aarch64:** tier 2 once booted
   release venues have qualified them. RHEL 8 maintenance ends 2029-05-31
   `[PLT-027]`; Debian 12's LTS ends 2028-06-30 `[PLT-040]`. A named
   rebuild qualifies under its own distribution name.
3. **RHEL 7.9 x86_64:** tier 2 once a named guest with current Extended
   Life-cycle Support updates runs the exact glibc 2.17 artefact on its
   `3.10.0-1160`-series kernel `[PLT-015]` `[PLT-025]`. Repeated on each
   claimed release, recording the kernel and the negotiated FUSE
   capabilities. Ends by 2029-05-31 `[PLT-024]`, or earlier if a required
   toolchain or dependency exceeds the verified floor. No earlier date is
   set for lack of demand. This row rests on A1 (the artefact runs there
   at FUSE 7.23) and A2: the row is qualified only once a site that runs
   AFS on EL7 has asked for it.
4. **Windows 11 Pro 25H2 x64 and Arm64, and Windows 11 Home, Enterprise and
   Education:** exact edition, version and architecture combinations are
   added after the route passes on a named native venue. Domain-joined and
   Entra-joined use are separate claims (L41).
5. **Static musl, x86_64 and aarch64,** starting with musl 1.2.5
   `[PLT-009]`: needs named guest kernels, a tested Kerberos path and a
   DNS-discovery smoke through the host's local resolver (C33). A static
   binary cannot load Kerberos mechanism modules `[PLT-013]`, so the
   Kerberos path is the open problem. Also depends on A1.
6. **FreeBSD amd64,** the newest supported point release of 14 and of 15:
   native install, mount, authenticated I/O and invalidation on each exact
   release in a named booted guest. Branch support ends 2028-11-30 and
   2029-12-31 `[PLT-058]`; the point release's shorter deadline applies.

Why: each is plausible from the documented floors `[PLT-038]` `[PLT-056]`
but has no venue yet. Naming them says what would be needed, without
promising it.

### L40. The common smoke test and the test cell

**Working.** Revisit if: the maintained OpenAFS 1.8 line moves on (the
fixture follows it, and the minimum tested patch level is published), or a
layer's documented freshness bound changes.

1. **Test cell:** a disposable Debian 13 fixture with OpenAFS 1.8.13.2
   (Debian package 1.8.13.2-1) file, volume and database services
   ([fileserver package](https://packages.debian.org/trixie/openafs-fileserver),
   [database package](https://packages.debian.org/trixie/openafs-dbserver))
   and MIT Kerberos 1.21.3 (package 1.21.3-5+deb13u1,
   [KDC package](https://packages.debian.org/trixie/krb5-kdc)). The image
   and exact package builds are pinned. Built and run under ADR 0001's
   test-tool rule.
2. A fresh fixture starts inside each hosted Linux job. The same definition
   runs on one isolated Linux guest that the maintainer's test clients can
   reach.
3. Two synthetic local users map to distinct Kerberos principals, each with
   private files, plus a shared coherency fixture. A deliberately separate
   authenticated writer session changes file contents and a directory
   entry.
4. **The smoke:** install, then mount or connect through the shipped entry
   point. Verify authenticated reads, and writes for a read-write claim; a
   read-only claim (L36, point 2; L12, point 3) instead verifies that
   writes are refused with the read-only error (L32, point 6), while the
   separate writer session still triggers callbacks. Verify each user's
   allowed access, and denied access to the other's data after the caches
   are warm; the independently triggered callback, and fresh file and
   directory results within the layer's documented bound, in the same
   running client.
5. Per layer: inode and entry invalidation, or the qualified uncached mode,
   on FUSE; cache-state changes on FSKit; client-visible refresh on SMB;
   on the replica, the remote change reconciled into the root's
   placeholders (L41, point 8).
6. **Record:** the venue, OS build or kernel, package versions, results,
   release revision and artefact digest. The maintainer's runs use
   disposable test systems and the release revision.

Why: the two properties that must never fail are cross-user isolation on a
warm cache and freshness after a callback. The smoke tests exactly those,
against a real server. The interoperability policy for servers is O39.

### L41. Windows route qualification

**Working.** Revisit if: Microsoft changes the SMB client's port, signing
or authentication defaults on a target build.

1. Each Windows edition, build, architecture, route and authentication mode
   qualifies separately.
2. With the shipped configuration installed, the test starts from an
   unprepared user session and opens the row's normal path: `\\afs`
   where the name is admitted, the literal-address form elsewhere
   (L20, point 3).
3. Required: authenticated SMB 3.1.1 file I/O with signing in force;
   rejection of guest and unsigned sessions (Windows 11 24H2 Pro,
   Enterprise and Education require signing, and requiring it disables
   guest access:
   [SMB signing](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-signing));
   two-user isolation; callback freshness.
4. Required: the path fails closed with IronAFS stopped, and an ordinary
   local user cannot take over any listener the route uses (L20, A4).
5. Required: Windows' own file sharing still works after a restart, a
   cumulative update and an uninstall.
6. Only route 3 can qualify the Windows 10 row `[WIN-162]`
   ([alternative ports](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-ports)).
7. Domain-joined use is claimed only after its own test.
8. If no Windows 11 SMB route qualifies, Windows SMB is omitted from the
   initial matrix. The next Windows target is then the replica on Windows 11
   Pro 26H2 x64, built only after its ADR is accepted (L27, point 11),
   which must qualify the NTFS sync root, two-user isolation,
   placeholder hydration, offline reads of hydrated files and reconciliation
   of remote changes before its row is published (NTFS only, `[WIN-M41]`;
   [Cloud Files requirements](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine)).
9. **Containment test** (L20, point 3), per row: responders for DNS,
   suffix search, LLMNR and NetBIOS; the hosts entry removed and
   overridden; IPv4 and IPv6; after a cumulative update; with the route
   disabled by a policy refresh (L20, point 8); NTLM blocking on and off;
   whether NTLM to IronAFS on loopback counts as remote; and, on rows
   that are domain-joined or claim Kerberos, Kerberos as well as NTLM. A
   row admits the name `afs` only where qualification shows that no
   authentication mechanism the row supports makes an attempt to another
   machine under that name in any case. The NTLM mechanism does not cover
   Kerberos; such a row shows its own Kerberos result or uses the literal
   address. After an uninstall, the test checks instead that the
   uninstaller removed the drive mappings and shortcuts the agent created
   and restored the settings it changed (L20, point 8; L21, point 5).
10. **Link test** (L32, points 2, 7 and 8): a cell link whose target
    escapes to a local drive, one whose target is a UNC path, a loop, a
    recursive delete through a link's name, and a delete, a rename and a
    delete-on-close of an entry reached through a resolved directory
    link. None reaches outside `/afs`; the recursive delete removes only
    the link, and the operations below the link are refused with access
    denied.

Why: this is where A3 is settled for each row. A route measured on one
build says little about another.

### L42. Build, mount and authentication floors

**Working.** Revisit if: Rust raises its Linux floors `[PLT-001]` (no later
raise was found as of 2026-11-19; `[PLT-005]`, corrected form), RHEL 8
stops being a candidate (the glibc build goal then moves), or assumption A1
is settled either way.

1. **Linux FUSE:** wire floor 7.23 (L2). Required: a successful INIT, the
   shared `allow_other` mount at `/afs`, and two-uid authorisation. The
   mounted `/afs` is checked to carry `nosuid,nodev` (L3). The writeback
   cache stays off (L4). Each run records the negotiated flags,
   and each optional feature that is enabled has its fallback tested.
2. The first qualification uses the shipped privileged direct mount (L7)
   with the system's normal confinement active. A packaged helper-based
   mount would qualify separately.
3. **Linux binaries:** one GNU artefact per architecture, built for the
   baseline x86-64 and ARMv8.0-A instruction sets. glibc 2.28 sysroots are
   the build goal while RHEL 8 is a candidate. glibc 2.17, Rust's own floor
   `[PLT-001]` `[PLT-002]`, is reserved for a conditional RHEL 7.9
   artefact.
4. Each artefact's ELF interpreter, symbol versions and shared-library
   needs are audited. The Kerberos provider, linkage, runtime components and
   tested credential-cache types are published per artefact.
5. **RHEL 7.9:** a claim requires interoperability with its system MIT
   Kerberos, 1.15.1 as CentOS 7.9 ships it `[PLT-017]`, and with that
   system's default credential cache. We take the default to be the
   persistent kernel-keyring type (`KEYRING` is a Linux-only cache type,
   `[PLT-014]`). No reachable Red Hat
   page has been cited for that default, so the statement depends on A1's
   venue: it is confirmed on the named RHEL 7.9 guest, or corrected there.
6. **Toolchain:** the workspace's Rust pin, currently 1.100.0
   (`rust-toolchain.toml`).

Why: a glibc 2.17 build runs wherever Rust's tier 1 Linux targets do
`[PLT-001]`, but building everything at that floor would constrain every
dependency for one candidate row.

### L43. Spikes and exclusions

**Working.** Revisit if: a site asks for an excluded platform and a venue
for it exists, or a loopback-NFS spike shows per-user isolation and
freshness on macOS 15 or 26.

1. macOS 15 and 26 on Apple silicon remain loopback-NFS spikes, with no
   support claim.
2. The initial matrix excludes: Intel Macs; macOS 14 and older; Ubuntu
   22.04; unlisted distributions and kernels; RHEL 6 and older; upstream
   Linux 3.10 and earlier; EL7 architectures other than x86_64; CentOS Linux
   7; 32-bit targets; Windows 10 Arm64 and LTSC; Windows 11 24H2, 26H1, LTSC
   and IoT; Windows Server; and any third-party filesystem driver, on every
   OS.

Why: each exclusion is past its vendor's support or has no venue: CentOS
Linux 7 `[PLT-022]`, RHEL 7 on ARM `[PLT-026]`, Windows 11 24H2 Home and
Pro `[PLT-054]`, Ubuntu 22.04 from May 2027 `[PLT-042]` (its hosted images
retire on 2027-04-17,
[retirement notice](https://github.com/actions/runner-images/issues/14254)).
Upstream 3.10 speaks only FUSE 7.22 `[PLT-M01]`. Third-party drivers are
excluded by the overview's constraint.

## Consequences

- **What an existing AFS user will notice.**
  - Linux: set-uid programs in AFS do not gain privilege, and device files
    do not appear (L3). Shared mappings fail, and private mappings work
    only on kernels that pass the mapping gate (L4, C15). `/afs` goes dead
    until remount if the daemon crashes, and without mount-beneath root
    can see the empty local directory while it is replaced (L7).
  - macOS: each user has their own mount, `/afs` may be a per-user link or
    absent (L10), locks are local to the machine (L33), and the layer is
    read-only if no error-reporting commit exists (L12). An editor that
    calls `fsync(2)` sees a failed save at once; `cp` or a shell
    redirection may report success for a write whose close-time store then
    fails, and the user is notified while the data waits in `afs pending`
    (L12, point 4). If revoking an item fails after a failed purge, that
    user's whole mount ends (C15, point 7). The layer ships only where
    FSKit gives a
    `nosuid,nodev` equivalent (L3, point 5). Only macOS 27 on Apple
    silicon is a target.
  - Windows: `\\afs\cell\path` works through Windows' own SMB client, with
    no driver, on rows where a qualified mechanism contains the name; on
    Windows 10, and wherever a site cannot enable that mechanism, the path
    is the literal `\\127.0.0.1\cell\path`, presented through a drive
    mapping or shortcut. `afs diagnose` names the mechanism in force per
    route. Characters Windows forbids appear as Private Use Area code
    points (L24). A machine that cannot contain the path gets no SMB
    surface (L20). AFS symbolic links inside `/afs` behave like their
    targets; links that leave `/afs` are listed but fail to open, and
    nothing below a resolved directory link can be deleted or renamed
    through the link's path: that is done through the target's own path
    (L32, points 2 and 8). A
    rename over a file another local program holds open succeeds unless
    that program denied delete sharing (L33, point 13). There is no
    offline use until the replica exists. The replica needs an ADR of
    its own before it is built; this part fixes only its invariants: one
    sign-in of an account at a time, no automatic upload, and off where
    its isolation cannot be shown (L27, L28).
  - Everywhere: an owner without a mapping rule shows as a reserved
    unmapped id (L31), and locks without server coverage are marked
    local-only (L33).
- **Cost.** The SMB server is the largest single piece of work: an SMB
  3.1.1 implementation of IronAFS's own, with signing, encryption, credits,
  compounds and share enumeration. The FUSE protocol layer is also IronAFS's
  own.
- **Performance is traded for isolation at first.** Zero kernel timeouts
  and direct I/O on Linux, no kernel data cache on macOS, and no leases on
  SMB all cost speed. Each relaxes only with acceptance evidence (L5, L11,
  L19).
- **Windows needs machine-wide settings** (a hosts entry, a port-proxy rule
  and a service dependency, or a port policy). Each has an owner and an undo
  (L21).
- **Windows may ship late, or not at first.** If no route qualifies, the
  initial matrix has no Windows row (L41).
- **Open assumptions.** A3 and A4 (the SMB routes), A6, A7 and A9 (the
  replica, through its own ADR, L27, point 11), A1 and A2 (RHEL 7.9, static musl) and A10 (File Provider) are
  each settled by the measurement the overview names.
- **Next, each as its own issue under the milestone plan (O50):**
  - the Windows spike (L22) and route qualification (L41);
  - the macOS gate (L13), starting with the signing and loading check;
  - the specifications for the FUSE layer, the name codec (L24, L25), the
    SMB front end (L19) and the lock service (L33);
  - a named self-hosted host for the booted-kernel candidates (L39).
- **Ruled out by this part:** a kernel module or driver of IronAFS's own on
  any OS; a third-party filesystem driver as a dependency; honouring set-id
  bits from a cell; the FUSE writeback cache; a shared FSKit mount without
  the isolation measurement; guest or unsigned SMB sessions; rewriting names
  on the server.

## Alternatives considered

### macOS

- **FSKit on macOS 27, plus a loopback NFSv4 server for older macOS.** Two
  layers to build and test. Apple's NFS client sends the caller's uid
  `[MAC-042]`, but a hit in its data cache sends no read, and buffered I/O
  can use a credential kept from an earlier request `[MAC-047]`, so
  per-user rights on cached data are not enforced by the server. Kept as a
  spike only (L43).
- **Loopback NFSv4 everywhere.** The same cache problem on the primary
  platform. NFSv3 gives the server no way to invalidate the client's cache
  `[MAC-048]`, and Apple's NFSv4.0 client takes only the attribute and
  recall callbacks (`[MAC-049]`, corrected form). It remains the fallback to
  evaluate if FSKit fails its gate (L8).
- **File Provider.** A replicating model: it keeps local copies that open
  without the remote store `[MAC-033]`, its requests describe the
  application, not the user `[MAC-032]`, and it has no byte-range lock
  operation `[MAC-036]`. We also assume it cannot place a domain at a fixed
  path such as `/afs` (A10). Rejected for `/afs`; if A10 fails, it is looked
  at again as an offline companion, not as the live surface.
- **A shared FSKit mount by default.** Rejected by L9 until measured.
- **The core embedded in the FSKit extension.** The cache and tokens would
  die with the extension (C1).
- **A kernel extension, or a third-party FUSE driver.** Excluded by the
  constraint. Third-party kernel extensions on Apple silicon need reduced
  security `[DRV-001]`. A third-party FSKit-backed FUSE layer would still be
  a dependency on someone else's filesystem code `[PLT-049]`.
- **Loopback SMB or WebDAV on macOS.** Apple's WebDAV mount sets every
  file's user and group to unknown (`[MAC-060]`, corrected form), and we
  found in neither a callback path as direct as FSKit's cache-state call. Not
  pursued while FSKit is viable.

### Windows

- **The Cloud Files replica as the only surface.** It cannot enforce live
  AFS rights on hydrated files `[DRV-014]`, gives no `\\afs` path, forwards
  no locks `[WIN-M43]`, and needs a second, Windows-only synchronisation
  engine. Kept as the later companion (L27) and as the fallback (L15).
- **Both surfaces at once.** Twice the unproven work before any Windows
  release.
- **ProjFS.** No caller identity, no locks, no UNC namespace `[WIN-148]`;
  an optional component `[WIN-136]`; no offline state or recall progress
  `[WIN-134]`; Microsoft points slow stores elsewhere `[WIN-135]`.
- **Windows' own SMB server sharing a local folder that IronAFS fills,**
  and **Windows' own NFS client.** We assume the first loses the caller's
  identity and the second, in its usual configuration, presents one identity
  for every user (A5). If a public spike shows either keeps the caller, that
  route is reconsidered.
- **NetBIOS over TCP (port 139).** It carries dialects up to 3.0.2 only
  `[WIN-169]` `[DES-M02]`, so it cannot meet L17.
- **A second loopback address.** The SMB client refuses it `[WIN-161]`.
- **IronAFS's server on port 445, passing other shares to Windows'
  server.** That would relay users' authentication (`[WIN-172]`).
- **The WebDAV redirector.** Its service is deprecated and not started by
  default `[WIN-177]`.
- **SMB over QUIC.** A separate transport that needs a certificate the
  client trusts `[WIN-166]`; its prerequisites name Windows 11 clients only
  `[WIN-167]`.
- **A kernel redirector of IronAFS's own, or a third-party user-mode
  filesystem driver.** Excluded by the constraint.
- **The Services for Macintosh name table.** It has no colon and places
  punctuation and the trailing characters differently `[DES-068]`, while
  Cygwin uses the U+F000 conversion for forbidden characters (context 34).
- **Omitting names Windows cannot show, or rewriting them on the server.**
  A silently partial listing is a wrong answer, and a rename would break
  every other client (L25).
- **Kerberos to one shared service name on every machine.** A service name
  belongs to one account `[DES-073]`.
- **A persistent port reservation to guard the back-end port.** It gives no
  protection between components `[WIN-187]`.
- **Containing `\\afs` by stated checks and a measured residual window,**
  or dropping Windows 10. The first lowers a Firm invariant (L20); the
  second gives up a supported row when the literal loopback address is
  contained on every build (L20, point 3).
- **AFS symbolic links as Windows symbolic-link reparse points.** Remote
  symbolic-link evaluation is off by default on Windows, so they would
  fail unless every site relaxed a machine-wide setting, and a link could
  point Windows at a local path or another host (L32).
- **Non-traversable markers for links over SMB, as on the replica.** It
  breaks common AFS layouts that rely on links (L32).
- **Replica uploads stored as soon as the provider sees a change.** With
  no conditional store (A9), an upload could overwrite a version the
  client never saw, with no person deciding (C28; L27, point 3).
- **Long-name aliases derived from the raw name and the file id.** Every
  save by rename would give the file a new alias and break saved paths
  and shortcuts (L25, point 3).

### Linux

- **Mounting through the setuid helper.** It needs a setuid program and a
  system configuration edit `[PLT-059]` `[PLT-060]`, and conflicts with
  `NoNewPrivileges` `[DES-041]`.
- **`default_permissions`, or kernel POSIX ACLs.** The kernel would decide
  access from mode bits AFS does not use `[DES-025]` `[DES-026]`.
- **The writeback cache, for speed.** Delayed writes lose the caller
  `[DES-023]`, and the kernel documentation advises against it for network
  filesystems `[DRV-067]`.
- **Keeping `/afs` alive across a daemon crash from the start.** It needs
  proof that each outstanding request completes once or fails explicitly.
  Later work, behind crash tests (L7).
- **A third-party FUSE library.** IronAFS's own code first (coding
  standards rule 21). The floor and the never-list of L2 and L5 are easier
  to guarantee in a layer written for them.
- **A raised FUSE floor (dropping RHEL 7.9).** Simpler, and it remains the
  outcome if A1 or A2 fails (L2).

### Presentation

- **Showing every caller as the owner.** L31, point 5.
- **AFS ids shown directly as local uids by default.** An unaligned id
  would name an unrelated local account, including a privileged one (L30).
  Alignment is available as an explicit per-cell rule.
- **Handing the server lock from one user to the next.** The second user
  may lack the lock right; coverage would continue under an authority that
  was never granted it (L33).
- **Resolving `fs` paths in the daemon.** L35.
