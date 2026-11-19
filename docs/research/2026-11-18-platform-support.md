<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Platform support floor: research notes (2026-11-18)

**How this note was made.**

- Web research on 2026-11-18, refute-first, with primary sources where they
  exist.
- Each claim is marked EXECUTED (run in a scratch directory outside the
  repository), INSPECTED (read in the cited source), or NOT VERIFIED.
- The RHEL 7 kernel facts come from a public third-party import of the
  `3.10.0-1160.el7` source, not from Red Hat's own documentation.
- Nothing was run on a real 3.10 kernel. The containers used shared a newer
  host kernel, so every kernel-behaviour claim for RHEL 7 is INSPECTED at
  best.
- Verdicts are against `docs/research/2026-11-18-driverless-afs-clients.md`.
- The note has not been cross-checked by a second model family.
- The probes used the `fuser` crate as a convenient FUSE client. Since this
  research, the owner decided that IronAFS writes its own `/dev/fuse` layer
  (coding standards rule 21). The `fuser` findings stand as facts about FUSE
  protocol negotiation, not as a plan to depend on it.

## Answer

RHEL 7 / CentOS 7 on x86_64 is feasible as a best-effort target. This note
recommends tier 2 rather than tier 1.

- **Toolchain:** Rust 1.97.1 still lists kernel 3.2+ and glibc 2.17+ for
  `x86_64-unknown-linux-gnu`. The toolchain and a FUSE probe built and ran in
  a CentOS 7.9 userland (EXECUTED, aarch64 image).
- **FUSE:** the RHEL 7.9 kernel exposes FUSE protocol 7.23, with the
  writeback cache backported, and it has the invalidation notifications that
  AFS callback breaks need (INSPECTED). It lacks everything from 7.24 up:
  larger requests, per-thread device file descriptors, parallel directory
  operations, directory and symlink caching, passthrough and io_uring.
- **Costs:** an end-of-life build image, a virtual machine for any real
  kernel test, and a floor that Rust may raise, because the distribution
  that justified it left maintenance on 2024-06-30.

Design for current kernels, negotiate features at run time down to FUSE 7.23,
and review RHEL 7 no later than the end of Red Hat's extended support on
2029-05-31.

## Verdict table

| # | Claim | Verdict | Quoted source | URL |
|---|---|---|---|---|
| 1 | Rust std needs kernel 3.2+ and glibc 2.17+ | CONFIRM for x86_64; AMEND for aarch64, which needs kernel 4.1+ | "x86_64-unknown-linux-gnu: 64-bit Linux (kernel 3.2+, glibc 2.17+)"; "aarch64-unknown-linux-gnu: ARM64 Linux (kernel 4.1+, glibc 2.17+)" | https://doc.rust-lang.org/1.97.1/rustc/platform-support.html |
| 2 | The 2022 baseline has not been raised | CONFIRM. The 1.97.1, stable and nightly pages give the same figures. | "glibc >= 2.17 (previously glibc >= 2.11)"; "kernel >= 3.2 (previously kernel >= 2.6.32)" | https://blog.rust-lang.org/2022/08/01/Increasing-glibc-kernel-requirements/ |
| 3 | A future raise is announced | Not found. The stated rationale has expired (A1). | "The new baseline requirements were picked as the least common denominator among long-term-support distributions that still have active support. This is currently RHEL 7" | same |
| 4 | A static musl binary sidesteps glibc on RHEL 7 | CONFIRM (EXECUTED). The kernel floor still applies. | "`*-linux-musl*` targets are only affected by the kernel requirements, not the glibc requirements." | same |
| 5 | "Roughly the RHEL 7 floor" | CONFIRM | "RHEL 7 (glibc 2.17, kernel 3.10)" listed as still supported | same |
| 6 | RHEL 7.9 kernel is 3.10.0-1160 | CONFIRM | "RHEL 7.9, 2020-09-29, 3.10.0-1160" | https://access.redhat.com/articles/3078 |
| 7 | RHEL 7 kernel speaks upstream 3.10's FUSE 7.22 | REJECT: it is 7.23, with the writeback cache backported | `#define FUSE_KERNEL_MINOR_VERSION 23`; `#define FUSE_WRITEBACK_CACHE (1 << 16)` | https://git.zx2c4.com/rhel7-kernel-misery/plain/include/uapi/linux/fuse.h |
| 8 | Writeback cache arrived in 3.15, passthrough in 6.9, io_uring in 6.14 | CONFIRM | "7.23 - add FUSE_WRITEBACK_CACHE"; "7.40 ... add FUSE_PASSTHROUGH init flag"; "7.42 - Add FUSE_OVER_IO_URING" | https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/plain/include/uapi/linux/fuse.h |
| 9 | A FUSE client needs a recent kernel | REJECT. The `fuser` crate's floor is protocol 7.6, and capabilities are negotiated at run time. | "We don't support ABI versions before 7.6" (`src/session.rs`, v0.18.0) | https://github.com/cberner/fuser/blob/v0.18.0/src/session.rs |
| 10 | A pure-Rust client can speak `/dev/fuse` directly | CONFIRM. Mounting still goes through the `fusermount` helper. | "on Linux these calls to libfuse are optional." | https://github.com/cberner/fuser/blob/master/README.md |
| 11 | CentOS 7 is end of life | CONFIRM | "After June 30, 2024, there will be no updates published for CentOS Linux 7." | https://blog.centos.org/2023/04/end-dates-are-coming-for-centos-stream-8-and-centos-linux-7/ |
| 12 | RHEL 7 paid extended support ends in 2028 | AMEND: 2029-05-31 | "This add-on is available from July 1, 2024 through May 31, 2029." | https://access.redhat.com/articles/7060360 |
| 13 | FSKit: macOS 15.4+, URL resources from macOS 26, handler APIs in macOS 27 | CONFIRM, from Apple's documentation data | FSKit `introducedAt` 15.4; `FSGenericURLResource` 26.0; `FSVolume.DataCacheHandler` 27.0 | https://developer.apple.com/documentation/fskit |
| 14 | Cloud Files API: Windows 10 1709+ | CONFIRM | "Minimum supported client: Windows 10, version 1709 [desktop apps only]" | https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfregistersyncroot |
| 15 | FreeBSD comes nearly free with FUSE | AMEND: the FUSE layer does; the rest does not (B3) | "Tested under Linux and FreeBSD using stable Rust" | https://github.com/cberner/fuser/blob/master/README.md |

## A. Linux

### A1. Rust's minimum kernel and glibc

- INSPECTED. Rust 1.97.1 lists tier 1 `x86_64-unknown-linux-gnu` at kernel
  3.2+ and glibc 2.17+, and `aarch64-unknown-linux-gnu` at kernel 4.1+ and
  glibc 2.17+.
- The floor was set in Rust 1.64 (2022-09-22). No later change raising it was
  found, and no announced raise. That is absence of evidence, not a
  guarantee.
- **Risk** (this note's reading): the 2022 rationale was the oldest
  long-term distribution "with active support". RHEL 7 left maintenance on
  2024-06-30, so that rationale no longer protects glibc 2.17 or kernel 3.2.
  The 2022 post's fallback, if the floor moves: "you may have to either
  freeze your current Rust version, or maintain a fork of libstd that
  supports older kernels."
- **EXECUTED,** in a CentOS 7.9 aarch64 container (glibc 2.17, gcc 4.8.5):
  - Rust 1.97.1 installed and ran;
  - a FUSE probe built and ran, needing at most `GLIBC_2.17`;
  - built in a current Rust image instead, it needed `GLIBC_2.34`, and on
    CentOS 7 it failed with "version `GLIBC_2.28' not found".

  So the build host or sysroot sets the glibc floor, not the Rust target
  alone. An x86_64 run under CPU emulation crashed `rustc` and is
  inconclusive.

### A2. Static musl builds

- **The targets:** `x86_64-unknown-linux-musl` and
  `aarch64-unknown-linux-musl` ship with musl 1.2.5 since Rust 1.93. The
  kernel floor applies; the aarch64 musl floor is NOT VERIFIED.
- **EXECUTED:** a static aarch64 musl probe ran on CentOS 7, and resolved
  `localhost`.
- **Trade-offs:**
  - **DNS.** musl's resolver behaves differently from glibc's ("queries them
    all in parallel and accepts whichever response arrives first"; DNS over
    TCP only from 1.2.4).
  - **No dynamic loading.** `dlopen` is a stub in static musl, so glibc NSS
    modules (sssd, LDAP) cannot load (NOT VERIFIED beyond the stub).
  - **Kerberos.** The system GSS-API mechanisms cannot load, because MIT
    krb5 registers them as loadable modules. Static musl would need a Rust
    Kerberos implementation or a helper process.
  - **Allocator.** musl's allocator is reportedly slower under thread
    contention (opinion, NOT VERIFIED).

### A3. RHEL 7 / CentOS 7 facts

| Component (7.9) | Version | How known |
|---|---|---|
| Kernel | 3.10.0-1160 | INSPECTED |
| glibc | 2.17 | EXECUTED |
| MIT krb5 | 1.15.1 | EXECUTED |
| systemd | 219 | EXECUTED |
| OpenSSL | 1.0.2k | EXECUTED |
| gcc | 4.8.5 | EXECUTED |
| libfuse | fuse 2.9.2 in base; fuse3 3.6.1 in Extras and EPEL | INSPECTED and EXECUTED |

**Support status:**

- **CentOS Linux 7:** end of life on 2024-06-30.
- **RHEL 7:** left maintenance on 2024-06-30. Paid Extended Life Cycle
  Support runs through 2029-05-31, on 7.9 only.
- **RHEL 7 for ARM:** retired in 2020. RHEL 7 here means x86_64.

| System | Kernel | Upstream FUSE minor at that kernel | Support end |
|---|---|---|---|
| RHEL 7.9 | 3.10.0-1160 | 22 upstream; 23 as shipped | ELS 2029-05-31 |
| RHEL 8.10 | 4.18.0-553 | 27 | 2029-05-31 |
| RHEL 9.9 | 5.14.0-741 | 34 | 2032-05-31 |
| RHEL 10.3 | 6.12.0-264 | 41 | 2035-05-31 |
| Debian 12 | 6.1 | 37 | not checked |
| Debian 13 | 6.12 | 41 | not checked |
| Ubuntu 24.04 | 6.8 (NOT VERIFIED) | 39 | not checked |

Red Hat backports FUSE features, so the upstream minor is only a lower bound
for RHEL 8, 9 and 10.

### A4. FUSE on RHEL 7

INSPECTED from the `linux-3.10.0-1160.el7` import. The protocol minor is 23,
and the INIT handshake offers the writeback cache.

| Feature IronAFS would want | Upstream protocol / kernel | RHEL 7.9 |
|---|---|---|
| Writeback cache | 7.23 / 3.15 | Present (backport) |
| `notify_inval_inode`, `notify_inval_entry` (callback breaks) | 7.12 | Present |
| readdirplus | 7.21 | Present |
| splice on the device | 7.14 | Present |
| `FOPEN_KEEP_CACHE`, `FUSE_AUTO_INVAL_DATA` | 7.9, 7.20 | Present |
| Caller uid, gid and pid per request (not on delayed writeback writes) | old | Present |
| Requests larger than 128 KiB (`max_pages`) | 7.28 / 4.20 | Missing |
| One device fd per worker (`FUSE_DEV_IOC_CLONE`) | kernel 4.2 | Missing |
| Parallel directory operations | 7.25 | Missing |
| POSIX ACLs, kill-privilege handling | 7.26 | Missing |
| Symlink and directory caching, `copy_file_range` | 7.28 | Missing |
| Explicit data invalidation | 7.30 | Missing |
| Expire-only entry notification | 7.38 | Missing |
| statx | 7.39 | Missing |
| Passthrough to backing files | 7.40 / 6.9 | Missing |
| io_uring transport | 7.42 / 6.14 | Missing |

- **Mounting:** the `fuse` and `fuse3` packages install `fusermount` setuid
  root (EXECUTED). An actual mount on a 3.10 kernel was not run.
- **`allow_other`:** a system-wide `/afs` needs it, gated by
  `user_allow_other` in `/etc/fuse.conf` for non-root users (NOT VERIFIED).
- **The `fuser` crate** (for reference): it accepts protocol 7.6 and up, and
  negotiates capabilities at run time. Its README says "Pull requests are no
  longer being accepted" and "Future releases will be developed primarily by a
  coding agent". Both are further reasons for IronAFS's own FUSE layer.

### A5. Who still runs RHEL 7 / CentOS 7

Citable public sources:

- **Red Hat** sells RHEL 7 extended support through 2029-05-31. Paying
  customers exist; their number is not public.
- **Queen Mary University of London**, 2025-01-22: "The majority of the cluster
  has now been upgraded to Rocky 9." (https://blog.hpc.qmul.ac.uk/rocky-announce/)
- **University of Arizona:** "Ocelote has been decommissioned on August 17th,
  2026." (https://hpcdocs.hpc.arizona.edu/resources/updates)
- **STFC SCARF:** its older nodes "will not be migrated to Rocky 9" and "are due
  for retirement." (https://www.scarf.rl.ac.uk/user-guides/rocky9.html)

No public source counts EL7 machines at AFS sites.

Opinion: research sites moved to EL9 during 2024 and 2025, and what remains on
EL7 is mostly old hardware waiting for retirement. Demand for a new client
there is probably small and shrinking. The owner may know of sites that change
this.

## B. Other platforms

### B1. macOS (FSKit)

| API | Introduced | Why IronAFS needs it |
|---|---|---|
| FSKit, `FSUnaryFileSystem`, `FSVolume` | macOS 15.4 | Base framework |
| `FSGenericURLResource`, `FSPathURLResource` | macOS 26.0 | A volume with no block device |
| `FSVolume.XattrHandler`, `AccessCheckHandler`, `DataCacheHandler` | macOS 27.0 | xattrs, access checks, kernel data cache |
| `setCacheState(...)` | macOS 27.0 | Callback breaks |
| `FSContext` | macOS 27.0 | Caller identity |

A network filesystem needs macOS 26 at least; correct cache invalidation and
caller identity need macOS 27. The practical floor is macOS 27.

### B2. Windows

- The Cloud Files API arrived in Windows 10 version 1709 and Windows Server
  2016.
- "Cldflt.sys currently only supports NTFS volumes", and "No two sync root
  trees are allowed to overlap."
- "Windows 10 will reach end of support on October 14, 2025." The API floor
  (1709) and a sensible support floor (Windows 11) differ.
- Which build each needed feature arrived in is work for the Windows spike.
- Update, 2026-11-19: a second Windows route now exists, an online `\\afs`
  path through Windows' own SMB client and an SMB server of IronAFS's own.
  It was measured working beside Windows' own file sharing on Windows 10
  22H2, Windows 11 24H2 and Windows 11 26H2 (MEASURED,
  [UNC note](2026-11-19-windows-unc-without-a-driver.md)). The Cloud Files
  points above apply to the offline companion.

### B3. FreeBSD and other FUSE systems

- FreeBSD's `fusefs` speaks protocol 7.29 on 14 and 7.35 on 15. The FUSE layer
  is close to free there, at a level above RHEL 7's.
- Service management, packaging, Kerberos credential caches and a CI runner
  are not free.
- FreeBSD's manual warns: "Write-back caching is usually unsafe, especially
  for FUSE file systems that require network access."
- macFUSE on macOS has a kernel-extension backend, outside the project's
  design, and an FSKit backend. IronAFS uses macFUSE only as an external tool
  in the planned spike, never as a dependency.

## C. Recommendation

### What RHEL 7 costs

**Lost on RHEL 7:**

- reads and writes capped at 128 KiB per request;
- one shared `/dev/fuse` descriptor, and no parallel directory operations;
- no kernel caching of directories or symlinks;
- no passthrough, the largest performance gap against newer kernels;
- systemd 219, OpenSSL 1.0.2k and krb5 1.15.1 are old. An all-Rust crypto
  and Kerberos stack avoids the last two.

**Cost of keeping it:**

- **Builds:** release builds linked against glibc 2.17 (an EL7 build image,
  which worked, or a 2.17 sysroot) or static musl.
- **Tests:** a virtual machine booting a 3.10 kernel for FUSE tests, since
  containers share the host kernel.
- **Toolchain:** a frozen compiler if Rust raises its floor.
- **Design:** every FUSE feature above 7.23 must be optional with a tested
  fallback. That discipline is also what RHEL 8 and FreeBSD need, so most of
  it is not specific to RHEL 7.

### Proposed support matrix

| Tier | Platforms | Meaning |
|---|---|---|
| 1, tested in CI | Linux x86_64 and aarch64, glibc: RHEL 9 and 10 (or rebuilds), Debian 13, Ubuntu 24.04 and 26.04. macOS 27 on Apple silicon. Windows 11 x86_64. | Every change is tested. Releases are blocked on failures. |
| 2, best effort | RHEL 7.9 / CentOS 7.9 x86_64. RHEL 8. Debian 12, Ubuntu 22.04. Static musl builds. FreeBSD 14 and 15. macOS 26. | Builds and a smoke test per release. Bugs fixed when the fix is cheap. No performance promise. |
| Out of scope | RHEL 6 and older. Kernels below 3.10. RHEL 7 on aarch64 or POWER. 32-bit targets. macOS 15 and older. Windows 10 and Windows Server. macFUSE. | Not built, not tested. |

Update, 2026-11-19: Windows 10 22H2 was out of scope because of the Cloud
Files route. The SMB route was since measured working on it (B2), so its
tier is reopened, to be settled with the platform floors in the
architecture ADR.

**RHEL 7 placement and justification:**

- **Tier 2, not tier 1.** It cannot be tested on real kernels in ordinary CI,
  it is past vendor maintenance, and it lacks the FUSE features the
  performance design will want.
- **Not out of scope.** Rust 1.97's documented floor matches it (glibc 2.17,
  kernel 3.2+ on x86_64), a userland build worked in an aarch64 CentOS 7
  container (EXECUTED), and FUSE 7.23 has the invalidation primitives the
  cache manager needs. x86_64 RHEL 7 itself is inferred, not executed: before
  claiming support, an x86_64 build linked against glibc 2.17 and a smoke test
  on a real 3.10 kernel are required.
- **Concrete commitment:**
  - release binaries target glibc 2.17;
  - the FUSE layer negotiates down to protocol 7.23;
  - one EL7 build-and-run job runs per release;
  - one manual test on a 3.10 virtual machine runs before a release claims
    RHEL 7.
- **Exit:** when Rust raises its Linux floor, or on 2029-05-31, whichever
  comes first.

This placement is a judgement, not a measured result.

## Open questions for the architecture ADR

1. **Minimum FUSE protocol:** 7.23, with everything newer optional?
2. **Required and optional capabilities,** each optional one with a tested
   fallback.
3. **Release linkage:** glibc 2.17 dynamic, static musl, or both?
4. **Kerberos and GSS:** a Rust implementation or the system libraries? Which
   credential cache types must be read?
5. **Name resolution:** the core's own DNS client, or libc?
6. **Mount model:** a root daemon mounting `/afs` with `allow_other`, or
   per-user mounts?
7. **Rust version policy** when Rust raises its Linux floor.
8. **Kernel test strategy:** which kernels get a virtual-machine job?
9. **macOS floor:** macOS 27 only, or a reduced mode on 26?
10. **Windows floor:** Windows 11 only, and which Cloud Files features?
    (Update, 2026-11-19: reopened by the SMB route, measured on Windows 10
    22H2 and 11; see B2.)
11. **FUSE layer:** decided after this research. IronAFS writes its own
    `/dev/fuse` layer (rule 21), negotiating at run time.
12. **RHEL backports:** what FUSE level the RHEL 8, 9 and 10 kernels really
    expose.

## Sources

- Rust: [platform support 1.97.1](https://doc.rust-lang.org/1.97.1/rustc/platform-support.html), [glibc and kernel requirements (2022)](https://blog.rust-lang.org/2022/08/01/Increasing-glibc-kernel-requirements/), [musl 1.2.5 (2025)](https://blog.rust-lang.org/2025/12/05/Updating-musl-1.2.5/)
- musl: [differences from glibc](https://wiki.musl-libc.org/functional-differences-from-glibc.html), [static `dlopen`](https://git.musl-libc.org/cgit/musl/plain/src/ldso/dlopen.c)
- MIT Kerberos: [host configuration](https://web.mit.edu/kerberos/krb5-latest/doc/admin/host_config.html)
- Red Hat: [release dates and kernels](https://access.redhat.com/articles/3078), [RHEL 7 end of maintenance](https://access.redhat.com/articles/7060360), [ELS announcement](https://www.redhat.com/en/blog/announcing-4-years-extended-life-cycle-support-els-red-hat-enterprise-linux-7), [retired life-cycle dates](https://access.redhat.com/articles/4038291)
- CentOS: [end dates](https://blog.centos.org/2023/04/end-dates-are-coming-for-centos-stream-8-and-centos-linux-7/), [7.9 packages](https://vault.centos.org/7.9.2009/os/x86_64/Packages/)
- RHEL 7 kernel source (third-party import): [`fuse.h`](https://git.zx2c4.com/rhel7-kernel-misery/plain/include/uapi/linux/fuse.h), [`fs/fuse/inode.c`](https://git.zx2c4.com/rhel7-kernel-misery/plain/fs/fuse/inode.c)
- Linux: [upstream `fuse.h`](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/plain/include/uapi/linux/fuse.h)
- `fuser`: [README](https://github.com/cberner/fuser/blob/master/README.md), [v0.18.0 `session.rs`](https://github.com/cberner/fuser/blob/v0.18.0/src/session.rs)
- Debian: [Debian 13 release notes](https://www.debian.org/releases/trixie/release-notes/whats-new.en.html)
- Research sites: [QMUL](https://blog.hpc.qmul.ac.uk/rocky-announce/), [University of Arizona](https://hpcdocs.hpc.arizona.edu/resources/updates), [SCARF](https://www.scarf.rl.ac.uk/user-guides/rocky9.html)
- Apple: [FSKit](https://developer.apple.com/documentation/fskit)
- Microsoft: [cloud sync engine](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine), [CfRegisterSyncRoot](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfregistersyncroot), [Windows 10 lifecycle](https://learn.microsoft.com/en-us/lifecycle/products/windows-10-home-and-pro)
- FreeBSD: [`fuse_kernel.h`](https://cgit.freebsd.org/src/plain/sys/fs/fuse/fuse_kernel.h), [fusefs manual](https://man.freebsd.org/cgi/man.cgi?query=fusefs&sektion=4)
