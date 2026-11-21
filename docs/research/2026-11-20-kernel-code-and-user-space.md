<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Kernel code against user space: shipping cost, failure and speed (2026-11-20)

**How this note was made.**

- Evidence labelled as in the [research index](README.md).
  - Every claim here is about operating systems and published studies,
    not about IronAFS, which has no working client yet.
  - A vendor's own page, documentation or repository is INSPECTED for
    what the vendor says about its own product. A figure the vendor
    states without a published method is marked "vendor's figure".
- Every source was read on 2026-11-20, and each quote was checked on the
  page, or in the paper's full text, by at least two separate reads.
  Sources are numbered and listed under [References](#references).
- Quotes are verbatim.
- This note backs public statements about why IronAFS avoids kernel code.
  No ADR decision rests on it.

## 1. The cost of shipping kernel code

### Windows

- **Microsoft signs kernel drivers, through an account that needs an EV
  certificate.** "Starting with Windows 10, version 1607, Windows will not
  load any new kernel-mode drivers which are not signed by the Dev
  Portal", and "an EV code signing certificate is required to establish a
  dashboard account" [1]. The policy lists exceptions under which
  cross-signed drivers are still permitted [1].
- **Attestation signing does not reach retail users through Windows
  Update.** "Attestation signed drivers that target retail audiences
  aren't published on Windows Update"; "You must have an extended
  validation (EV) code signing certificate" [2].
- **Trust in cross-signed drivers is being removed (2026).** Microsoft
  announced "the removal of trust for all kernel drivers signed by the
  deprecated cross-signed root program" for Windows 11 24H2, 25H2 and
  26H1 and Windows Server 2025, with "All future versions of Windows 11
  and Windows Server will enforce the new kernel trust policy" [3].
  - The April 2026 Windows update starts an evaluation mode. The policy
    is enforced only after a period of uptime and restarts, and the
    period resets if a cross-signed driver would fail.
  - An update to the post (June 2026) raised the uptime from 100 to 250
    hours; the restarts are 3 on Windows 11 and 2 on Server 2025 [3].
  - By default, only drivers that passed the Windows Hardware
    Compatibility Program load, plus an explicit allow list of reputable
    cross-signed drivers.
  - The stated reason: the old program's certificate handling "led to
    abuse and credential theft" [3].
- **The vulnerable driver blocklist is on by default.** "Since the
  Windows 11 2022 update, the vulnerable driver blocklist is enabled by
  default for all devices". Except on Windows Server 2016, it is also
  enforced when memory integrity, Smart App Control or S mode is active.
  "HVCI is on by-default for most new Windows 11 devices"; "The blocklist
  is updated quarterly" [4].
- **Microsoft's Driver Quality Initiative (WinHEC 2026)** has four
  pillars: Architecture, Trust, Lifecycle and Quality Measures [5]. Its
  Architecture pillar includes "enabling the third-party kernel mode
  driver transition to either user mode driver or Microsoft authored
  class drivers"; the post also covers hardening drivers that stay in
  the kernel. It calls the initiative "a partnership", and states no
  requirement that drivers leave the kernel [5].
- **Microsoft can roll back a driver itself.** Cloud-initiated driver
  recovery "proactively rolls back problematic drivers delivered through
  Windows Update" when Microsoft's quality evaluation rejects a driver,
  because otherwise "devices may remain on a low-quality driver for an
  extended period" [6].
  - The announcement (May 2026) scheduled manual validation for May to
    August 2026, and targeted automatic inclusion for September 2026.
  - Whether either happened is NOT VERIFIED.

### macOS

- **A third-party kernel extension on Apple silicon needs Reduced
  Security.**
  - "Kexts need to be explicitly enabled for a Mac with Apple silicon by
    holding the power button at startup"; "it requires that the secure
    boot be configured to Reduced Security"; "Rebuilding the AuxKC
    requires the user's approval and restarting of the macOS".
  - The same page: "By running in user space, system extensions increase
    the stability and security of macOS" [7].
- **What a user does** [8]:
  1. Start up holding the power button until "Loading startup options"
     appears.
  2. Open Startup Security Utility in Recovery, and choose the startup
     disk's security policy.
  3. Choose Reduced Security, and enable "Allow user management of kernel
     extensions from identified developers" ("Allows installation of
     software that uses legacy kernel extensions").
  4. Authorise the change with an administrator's password, and restart.

  Reduced Security "Allows any version of signed operating system software
  ever trusted by Apple to run" [8].
- **Kernel extensions are deprecated.** Apple: "kernel extensions have
  been deprecated"; from macOS Big Sur, "macOS releases no longer load
  kernel extensions that use deprecated kernel programming interfaces
  (KPIs) by default" [9]. Apple's word is deprecation, not removal.

### Linux

- **No stable in-kernel interface.** Linux "does not have a binary kernel
  interface, nor does it have a stable kernel interface"; the system-call
  interface "is very stable over time, and will not break". A driver in
  the kernel tree is "fixed up by the person who did the kernel change in
  the first place" [10]. A module outside the tree is kept working by its
  own maintainers (INFERRED).
- **GPL-only symbols.** For a module tagged `MODULE_LICENSE("Proprietary")`,
  "the kernel module loader refuses to link such modules against symbols
  which are exported with `EXPORT_SYMBOL_GPL()`" [11]. The rule follows
  the module's declared tag, not a legal finding about its licence.
- **Module signing.** With enforcement on, "the kernel will only load
  validly signed modules for which it has a public key"; with it off,
  unsigned modules "are permitted, but the kernel will be marked as being
  tainted" [12].
- **Secure Boot and DKMS on Debian.**
  - "Using Secure Boot activates "lockdown" mode in the Linux kernel",
    which blocks loading modules not signed by a trusted key: "By
    default, this will block out-of-tree modules including DKMS-managed
    drivers" [13].
  - DKMS modules can be signed with a machine owner key (MOK). Enrolling
    it takes `mokutil --import`, a one-time password, a reboot, and a
    confirmation in UEFI's screens [13].
  - This is Debian's documented behaviour. The man-pages project's
    lockdown page describes lockdown's effects without saying that Secure
    Boot turns it on [14].

## 2. When the code fails

- **Drivers and reported failures (2003).** Swift, Bershad and Levy
  write: "In Windows XP, for example, drivers account for 85% of recently
  reported failures" [15].
  - The paper cites a private communication from a Microsoft
    vice-president for it ("Short 2003"), so the figure itself is RELAYED.
  - It counts reported failures, not crashes.
  - No later Microsoft percentage was found.
- **Isolating drivers.** In the same paper's fault-injection tests: "Of
  the 317 crashes observed with native Linux, Nooks eliminated 313, or
  99%" [15].
- **Driver error rates (2001), and ten years later.**
  - Chou et al. found, in Linux, that drivers "have error rates up to
    three to seven times higher than the rest of the kernel", depending
    on the kind of error [16].
  - Palix et al. repeated the study on Linux 2.6.0 to 2.6.33. Drivers
    still contain "the most faults", but "its fault rate is now below
    that of other directories, such as arch (HAL) and fs (file systems)"
    [17].
  - The 2001 ratio is not a current one.
- **19 July 2024.**
  - Microsoft: "We currently estimate that CrowdStrike's update affected
    8.5 million Windows devices, or less than one percent of all Windows
    machines" [18].
  - The crashing module, `csagent.sys`, "is registered as a file system
    filter driver"; "Kernel drivers provide the above properties at the
    cost of resilience" [19].
  - What was pushed was a content file that the driver read: "Although
    Channel Files end with the SYS extension, they are not kernel
    drivers" [20].
- **A user-space file system can still hang its mount.** "There are a
  couple of ways in which to deadlock a FUSE filesystem" [21]. The ways
  out need no reboot, but differ in reach:
  - killing the daemon works only for some of the deadlocks;
  - `umount -f` works only while the filesystem is still attached;
  - aborting the connection through the FUSE control filesystem is the
    "Most powerful method, always works" [21].
- **And a user-space API can still need a reboot.** With Windows' Cloud
  Files API, killing a provider inside `CfExecute` wedged its sync root
  until reboot in 9 of 10 trials (MEASURED,
  [Cloud Files note](2026-11-19-windows-cloud-files.md), §6).
  - Moving data production into a separate process made 10 of 10 kills
    of that process clean.
  - Killing the process that holds the connection during a call can
    still need a reboot.
- **What follows (INFERRED).** User space narrows what a bug can take
  down; it does not remove the need to test failure paths. ADR 0004
  counts "a hang that needs a reboot" as a failure even if everything
  else works.

## 3. Speed

### Where FUSE costs show

- **Vangoor, Tarasov and Zadok, FAST 2017** [22]. A FUSE file system
  that passes operations through to ext4, measured on Linux 4.1.13 with
  an HDD and an SSD.
  - The abstract: degradation "can be completely imperceptible or as
    high as –83% even when optimized; and relative CPU utilization can
    increase by 31%".
  - The worst case is single-threaded file creation (`files-cr-1th`).
    Taking the better of the two FUSE configurations for each workload,
    only the two file-creation workloads, out of 45, lost more than half
    their throughput.
  - "Stackfs performs visibly worse for metadata-intensive and macro
    workloads ... than for data-intensive workloads. The performance is
    especially low for SSDs."
  - With 32 threads reading one file at random, the loss was up to 83%
    for the unoptimised configuration and 28% for the optimised one.
- **RFUSE, FAST 2024** [23]. A research framework, with new kernel and
  user-space parts, that gives FUSE per-core ring buffers.
  - The paper calls stock FUSE's overhead "notable", "even more
    prominent in modern hardware environments with high-performance
    storage devices".
  - Against stock FUSE: 53% lower latency for an empty CREATE on a null
    file system, and "2.27x higher throughput than FUSE in the random
    read workload" on a pass-through file system.
  - It also runs a FUSE NFS client against Linux's in-kernel NFS client.
    The authors treat in-kernel NFS "as a reference point for theoretical
    maximum performance", while warning that the two implementations
    differ.
- **FSKit (macOS)** [24]. In a forum thread about a minimal FSKit file
  system whose `getdirentries` call averaged 121 µs, Apple's engineer
  replied: "that's about where things stand at the moment", "most of
  this is overhead of repeated XPC traffic, which probably could be
  avoided", and "the performance is definitely being looked at"
  (RELAYED: a forum post, not documentation).
- **Windows loopback SMB.** This search found no current measurement.

### How the gap is closing on Linux

- **Passthrough.**
  - It is "designed to improve the performance of FUSE filesystems for
    I/O operations": reads, writes, `splice` and `mmap` on a FUSE file
    "bypass the userspace daemon".
  - It needs `CAP_SYS_ADMIN` [25].
  - It arrived in Linux 6.9, per the repository's earlier research
    ([driverless clients note](2026-11-18-driverless-afs-clients.md)).
- **io_uring.**
  - Linux 6.14 "adds FUSE support for io-uring communication between
    kernel and userspace, with the purpose of increasing FUSE
    performance" (RELAYED: a release summary) [26].
  - The kernel's documentation: "every CPU core has its own fuse-io-uring
    queue", and the interface "is still in development and might change"
    [27].

### Network clients

- **The project with both clients says the kernel one is faster.** Ceph's
  documentation: "The FUSE client is the most accessible and the easiest
  to upgrade to the version of Ceph used by the storage cluster, while
  the kernel client will always give better performance" [28].
- **Production FUSE network clients exist.**
  - Mountpoint for Amazon S3 describes itself as "a simple,
    high-throughput file client" and credits "The fuser bindings for
    FUSE" [29].
  - Cloud Storage FUSE is "an open source FUSE adapter" [30]. Its README
    claims "up to 9x faster model load times" from parallel downloads,
    which use the file-cache directory as a prefetch buffer, and "up to
    2.3x faster training time and 3.4x higher throughput" from its file
    cache (vendor's figures) [30].
- **What this search did not find:** a study comparing one network file
  system's FUSE client with its in-kernel client that shows the
  user-space cost disappearing behind the network.

### What this means for IronAFS (INFERRED)

- A FUSE client pays FUSE's cost most on metadata: lookups, attribute
  reads and creates, as in `ls -l` on a large directory, a build or
  `find` [22]. How much of that cost the network hides depends on the
  workload and the server; this note has no measurement of it.
- On Linux, IronAFS starts with the kernel's entry and attribute caches
  off and uses direct I/O (ADR 0004, L5). So every lookup and attribute
  read reaches the daemon, which costs speed on exactly those workloads.
  - The other layers start conservative in their own ways: no kernel
    data cache on macOS (L11), and no leases or oplocks on SMB (L19).
  - Each relaxation needs acceptance evidence for isolation and callback
    freshness. The FUSE writeback cache is never enabled (L4).
- No IronAFS numbers exist. ADR 0004, O49 plans counting benchmarks in
  CI and timed benchmarks on fixed hardware.

## References

All accessed 2026-11-20.

1. Microsoft. "Driver Signing Policy." Microsoft Learn.
   <https://learn.microsoft.com/en-us/windows-hardware/drivers/install/kernel-mode-code-signing-policy--windows-vista-and-later->
2. Microsoft. "Attestation Sign Windows Drivers." Microsoft Learn.
   <https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/code-signing-attestation>
3. Microsoft. "Advancing Windows driver security: Removing trust for the
   cross-signed driver program." Windows IT Pro Blog, 26 March 2026,
   updated 22 October 2026.
   <https://techcommunity.microsoft.com/blog/windows-itpro-blog/advancing-windows-driver-security-removing-trust-for-the-cross-signed-driver-pro/4504818>
4. Microsoft. "Microsoft recommended driver block rules." Microsoft Learn.
   <https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/design/microsoft-recommended-driver-block-rules>
5. Microsoft. "Raising the bar together: Introducing the Driver Quality
   Initiative at WinHEC 2026." Windows Experience Blog, 14 May 2026.
   <https://blogs.windows.com/windowsexperience/2026/05/14/raising-the-bar-together-introducing-the-driver-quality-initiative-at-winhec-2026/>
6. Microsoft. "Introducing Cloud-Initiated Driver Recovery for Windows
   Update." Hardware Dev Center Blog, 12 May 2026.
   <https://techcommunity.microsoft.com/blog/hardware-dev-center/introducing-cloud-initiated-driver-recovery-for-windows-update/4519075>
7. Apple. "Securely extending the kernel in macOS." Apple Platform
   Security.
   <https://support.apple.com/guide/security/securely-extending-the-kernel-sec8e454101b/web>
8. Apple. "Change security settings on the startup disk of a Mac with
   Apple silicon." Mac User Guide.
   <https://support.apple.com/guide/mac-help/change-security-settings-startup-disk-a-mac-mchl768f7291/mac>
9. Apple. "Deprecated Kernel Extensions and System Extension
   Alternatives." Apple Developer.
   <https://developer.apple.com/support/kernel-extensions/>
10. Greg Kroah-Hartman. "The Linux Kernel Driver Interface."
    The Linux kernel documentation.
    <https://www.kernel.org/doc/html/latest/process/stable-api-nonsense.html>
11. "Linux kernel licensing rules." The Linux kernel documentation.
    <https://docs.kernel.org/process/license-rules.html>
12. "Kernel module signing facility." The Linux kernel documentation.
    <https://docs.kernel.org/admin-guide/module-signing.html>
13. "SecureBoot." Debian Wiki. <https://wiki.debian.org/SecureBoot>
14. "kernel_lockdown(7)." Linux man-pages project.
    <https://man7.org/linux/man-pages/man7/kernel_lockdown.7.html>
15. Michael M. Swift, Brian N. Bershad and Henry M. Levy. "Improving the
    Reliability of Commodity Operating Systems." *ACM Transactions on
    Computer Systems* 23(1), February 2005, pp. 1–34.
    <https://pages.cs.wisc.edu/~swift/papers/improving-reliability.pdf>
16. Andy Chou, Junfeng Yang, Benjamin Chelf, Seth Hallem and Dawson
    Engler. "An Empirical Study of Operating Systems Errors."
    *Proceedings of the 18th ACM Symposium on Operating Systems
    Principles (SOSP)*, 2001.
    <https://www.read.seas.harvard.edu/~kohler/class/aosref/chou01empirical.pdf>
17. Nicolas Palix, Gaël Thomas, Suman Saha, Christophe Calvès, Julia
    Lawall and Gilles Muller. "Faults in Linux: Ten years later."
    *ASPLOS XVI*, March 2011, pp. 305–318. doi:10.1145/1950365.1950401.
    <https://pure.psu.edu/en/publications/faults-in-linux-ten-years-later/>
18. David Weston. "Helping our customers through the CrowdStrike
    outage." Official Microsoft Blog, 20 July 2024.
    <https://blogs.microsoft.com/blog/2024/07/20/helping-our-customers-through-the-crowdstrike-outage/>
19. David Weston. "Windows Security best practices for integrating and
    managing security tools." Microsoft Security Blog, 27 July 2024.
    <https://www.microsoft.com/en-us/security/blog/2024/07/27/windows-security-best-practices-for-integrating-and-managing-security-tools/>
20. CrowdStrike. "Technical Details: Falcon Content Update for Windows
    Hosts." 20 July 2024.
    <https://www.crowdstrike.com/blog/falcon-update-for-windows-hosts-technical-details/>
21. "FUSE Overview." The Linux kernel documentation, section "Aborting a
    filesystem connection".
    <https://docs.kernel.org/filesystems/fuse/fuse.html>
22. Bharath Kumar Reddy Vangoor, Vasily Tarasov and Erez Zadok. "To FUSE
    or Not to FUSE: Performance of User-Space File Systems." *15th
    USENIX Conference on File and Storage Technologies (FAST 17)*,
    February 2017.
    <https://www.usenix.org/conference/fast17/technical-sessions/presentation/vangoor>
23. Kyu-Jin Cho, Jaewon Choi, Hyungjoon Kwon and Jin-Soo Kim. "RFUSE:
    Modernizing Userspace Filesystem Framework through Scalable
    Kernel-Userspace Communication." *22nd USENIX Conference on File and
    Storage Technologies (FAST 24)*, February 2024, pp. 141–157.
    <https://www.usenix.org/conference/fast24/presentation/cho>
24. "FSKit caching by kernel and performance." Apple Developer Forums,
    thread 793013, July 2025.
    <https://developer.apple.com/forums/thread/793013>
25. "FUSE Passthrough." The Linux kernel documentation.
    <https://docs.kernel.org/filesystems/fuse/fuse-passthrough.html>
26. "Linux 6.14." Kernel Newbies. <https://kernelnewbies.org/Linux_6.14>
27. "FUSE-over-io-uring design documentation." The Linux kernel
    documentation.
    <https://docs.kernel.org/filesystems/fuse/fuse-io-uring.html>
28. Ceph. "Mount CephFS: Prerequisites." Ceph documentation (development
    version). <https://docs.ceph.com/en/latest/cephfs/mount-prerequisites/>
29. Amazon Web Services. "mountpoint-s3." GitHub README.
    <https://github.com/awslabs/mountpoint-s3>
30. Google Cloud. "gcsfuse." GitHub README.
    <https://github.com/GoogleCloudPlatform/gcsfuse>
