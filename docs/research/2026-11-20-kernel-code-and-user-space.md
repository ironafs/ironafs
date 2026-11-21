<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Kernel code against user space: shipping cost, failure and speed (2026-11-20)

**How this note was made.**

- Evidence labelled as in the [research index](README.md). Every claim
  here is about the operating systems and published studies, not about
  IronAFS, which has no working client yet.
- Pages were read on 2026-11-20. Claims marked **(two passes)** were read
  on the page by two separate passes; the rest by one. Microsoft's 2026
  announcements are all two-pass.
- Quotes are verbatim; dates are the page's own.
- This note backs public statements about why IronAFS avoids kernel code.
  No ADR decision rests on it, and it has not yet been re-checked by a
  second model family.

## 1. The cost of shipping kernel code

### Windows

- **Kernel drivers are signed through Microsoft, with an EV certificate.**
  "Starting with Windows 10, version 1607, Windows will not load any new
  kernel-mode drivers which are not signed by the Dev Portal", and "an EV
  code signing certificate is required to establish a dashboard account"
  (INSPECTED, two passes,
  [Driver Signing Policy](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/kernel-mode-code-signing-policy--windows-vista-and-later-),
  updated 2025-07-18).
- **Attestation signing is not for retail.** "Attestation signed drivers
  that target retail audiences aren't published on Windows Update"; "You
  must have an extended validation (EV) code signing certificate"
  (INSPECTED, two passes,
  [Attestation signing](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/code-signing-attestation),
  updated 2026-04-14). Microsoft documents no submission turnaround time.
- **Trust in cross-signed drivers is being removed (2026).** Microsoft
  announced "the removal of trust for all kernel drivers signed by the
  deprecated cross-signed root program" for Windows 11 24H2, 25H2, 26H1
  and Windows Server 2025, from the April 2026 Windows update; "All future
  versions of Windows 11 and Windows Server will enforce the new kernel
  trust policy." By default only drivers that passed the Windows Hardware
  Compatibility Program load, with an explicit allow list of reputable
  cross-signed drivers. Enforcement follows an evaluation period of 250
  hours and 3 restarts on Windows 11 (2 restarts on Server 2025), reset if
  a cross-signed driver would fail; the post was updated from 100 hours.
  The stated reason: the old program's certificate handling "led to abuse
  and credential theft" (INSPECTED, two passes,
  [Windows IT Pro blog](https://techcommunity.microsoft.com/blog/windows-itpro-blog/advancing-windows-driver-security-removing-trust-for-the-cross-signed-driver-pro/4504818),
  published 2026-03-26, updated 2026-10-22).
- **The vulnerable driver blocklist is on by default.** "Since the Windows
  11 2022 update, the vulnerable driver blocklist is enabled by default for
  all devices"; it is enforced when memory integrity, Smart App Control or
  S mode is active (except on Windows Server 2016); "HVCI is on by-default
  for most new Windows 11 devices"; "The blocklist is updated quarterly"
  (INSPECTED, two passes,
  [recommended driver block rules](https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/design/microsoft-recommended-driver-block-rules),
  updated 2026-10-08).
- **Microsoft is moving third-party drivers out of the kernel, by
  partnership.** The Driver Quality Initiative (WinHEC 2026) has four
  pillars: Architecture, Trust, Lifecycle and Quality Measures. The
  Architecture pillar covers "enabling the third-party kernel mode driver
  transition to either user mode driver or Microsoft authored class
  drivers". The post calls it "a partnership"; nothing is described as
  mandatory (INSPECTED, two passes,
  [Windows Experience Blog](https://blogs.windows.com/windowsexperience/2026/05/14/raising-the-bar-together-introducing-the-driver-quality-initiative-at-winhec-2026/),
  2026-05-14).
- **Microsoft can now roll back drivers itself.** Cloud-initiated driver
  recovery "proactively rolls back problematic drivers delivered through
  Windows Update", because otherwise "devices may remain on a low-quality
  driver for an extended period". Manual validation ran May to August 2026;
  automatic inclusion was targeted for September 2026 (INSPECTED, two
  passes,
  [Hardware Dev Center blog](https://techcommunity.microsoft.com/blog/hardware-dev-center/introducing-cloud-initiated-driver-recovery-for-windows-update/4519075),
  2026-05-12). Whether automatic inclusion started is NOT VERIFIED.

### macOS

- **A third-party kernel extension on Apple silicon needs Reduced
  Security.** "Kexts need to be explicitly enabled for a Mac with Apple
  silicon by holding the power button at startup"; "it requires that the
  secure boot be configured to Reduced Security"; "Rebuilding the AuxKC
  requires the user's approval and restarting of the macOS"; and "By
  running in user space, system extensions increase the stability and
  security of macOS" (INSPECTED, two passes,
  [Apple Platform Security](https://support.apple.com/guide/security/securely-extending-the-kernel-sec8e454101b/web)).
- **The steps a user takes.** Shut down, hold the power button until
  "Loading startup options" appears, open Startup Security Utility in
  Recovery, choose Reduced Security, and enable "Allow user management of
  kernel extensions from identified developers" ("Allows installation of
  software that uses legacy kernel extensions"). Reduced Security "Allows
  any version of signed operating system software ever trusted by Apple to
  run" (INSPECTED, two passes,
  [Mac User Guide](https://support.apple.com/guide/mac-help/change-security-settings-startup-disk-a-mac-mchl768f7291/mac)).
- **Kernel extensions are deprecated.** Apple: "kernel extensions have been
  deprecated"; from macOS Big Sur, "macOS releases no longer load kernel
  extensions that use deprecated kernel programming interfaces (KPIs) by
  default" (INSPECTED, two passes,
  [Deprecated Kernel Extensions](https://developer.apple.com/support/kernel-extensions/)).
  Apple's wording is deprecation, not removal.

### Linux

- **No stable in-kernel interface.** Linux "does not have a binary kernel
  interface, nor does it have a stable kernel interface"; the system-call
  interface "is very stable over time, and will not break"; a driver in the
  kernel tree is "fixed up by the person who did the kernel change"
  (INSPECTED, two passes,
  [stable-api-nonsense](https://www.kernel.org/doc/html/latest/process/stable-api-nonsense.html)).
  A module outside the tree is rebuilt and fixed by its own maintainers.
- **GPL-only symbols.** For a module whose licence is not GPL-compatible,
  "the kernel module loader refuses to link such modules against symbols
  which are exported with `EXPORT_SYMBOL_GPL()`" (INSPECTED, two passes,
  [licensing rules](https://docs.kernel.org/process/license-rules.html)).
- **Module signing.** With enforcement on, "the kernel will only load
  validly signed modules for which it has a public key"; with it off,
  unsigned modules load "but the kernel will be marked as being tainted"
  (INSPECTED, two passes,
  [module signing](https://docs.kernel.org/admin-guide/module-signing.html)).
- **Secure Boot and DKMS (Debian).** "Using Secure Boot activates
  "lockdown" mode in the Linux kernel", which blocks loading modules not
  signed by a trusted key: "By default, this will block out-of-tree modules
  including DKMS-managed drivers." DKMS modules can be signed with a
  machine owner key (MOK); enrolling it takes `mokutil --import`, a
  one-time password, a reboot and a confirmation in UEFI's screens
  (INSPECTED, two passes, [Debian wiki](https://wiki.debian.org/SecureBoot)).
  This is Debian's behaviour; the kernel's own
  [lockdown page](https://man7.org/linux/man-pages/man7/kernel_lockdown.7.html)
  does not say that Secure Boot turns lockdown on.

## 2. When the code fails

- **Drivers and reported failures (2003).** Swift, Bershad and Levy:
  "In Windows XP, for example, drivers account for 85% of recently
  reported failures." The figure comes from a Microsoft vice-president's
  private communication, and it counts reported failures, not crashes
  (INSPECTED, one pass,
  [ACM TOCS 2005](https://pages.cs.wisc.edu/~swift/papers/improving-reliability.pdf)).
  No later Microsoft percentage was found.
- **Driver error rates (2001), and ten years later.** Chou et al.: "device
  drivers have error rates up to three to seven times higher than the rest
  of the kernel", depending on the checker, in Linux to 2.4.1 and OpenBSD
  (INSPECTED, one pass,
  [SOSP 2001](https://www.read.seas.harvard.edu/~kohler/class/aosref/chou01empirical.pdf)).
  Palix et al. found by Linux 2.6.33 that drivers' fault rate had dropped
  below that of `arch` and `fs` (INSPECTED, one pass,
  [ASPLOS 2011](https://pure.psu.edu/en/publications/faults-in-linux-ten-years-later/)).
  The 2001 ratio is not a current one.
- **19 July 2024.** Microsoft estimated that a faulty update "affected 8.5
  million Windows devices, or less than one percent of all Windows
  machines" (INSPECTED, one pass,
  [Microsoft](https://blogs.microsoft.com/blog/2024/07/20/helping-our-customers-through-the-crowdstrike-outage/)).
  The crashing component, `csagent.sys`, is "registered as a file system
  filter driver"; "Kernel drivers provide the above properties at the cost
  of resilience" (INSPECTED, one pass,
  [Microsoft Security Blog](https://www.microsoft.com/en-us/security/blog/2024/07/27/windows-security-best-practices-for-integrating-and-managing-security-tools/)).
  What was pushed was a content file that the driver read, not a new
  driver: "Although Channel Files end with the SYS extension, they are not
  kernel drivers" (INSPECTED, one pass,
  [CrowdStrike](https://www.crowdstrike.com/blog/falcon-update-for-windows-hosts-technical-details/)).
- **A user-space file system can still hang its mount.** Linux's FUSE
  documentation describes deadlocks and gives ways out that need no
  reboot: kill the daemon, `umount -f`, or abort the connection through
  the FUSE control filesystem, "Most powerful method, always works"
  (INSPECTED, one pass,
  [FUSE](https://docs.kernel.org/filesystems/fuse/fuse.html)).
- **And a user-space API can still need a reboot.** With Windows' Cloud
  Files API, killing a provider inside `CfExecute` wedged its sync root
  until reboot in 9 of 10 trials (MEASURED,
  [Cloud Files note](2026-11-19-windows-cloud-files.md), §6). User space
  narrows what a bug can take down; it does not remove the need to test
  failure paths. ADR 0004 counts "a hang that needs a reboot" as a failure
  even if everything else works.

## 3. Speed

### Where FUSE costs show

- **Vangoor, Tarasov and Zadok, FAST 2017.** FUSE's "performance
  degradation ... can be completely imperceptible or as high as –83% even
  when optimized; and relative CPU utilization can increase by 31%". The
  –83.1% is single-threaded file creation (`files-cr-1th`); in the better
  of the two FUSE configurations, only the two file-creation workloads
  (out of 45) lost more than half their throughput. "Stackfs performs
  visibly worse for metadata-intensive and macro workloads ... than for
  data-intensive workloads"; random reads by 32 threads lost up to 83%
  unoptimised and 28% optimised. Faster devices expose more of the
  overhead, though not always: sequential reads lost nothing on SSD and
  26–42% on HDD (INSPECTED, two passes for the abstract, one for the body,
  [paper](https://www.fsl.cs.stonybrook.edu/docs/fuse/fuse-performance-fast17.pdf)).
  The hardware and kernel are from 2016–2017, and the file system
  measured is a local pass-through to ext4, not a network client.
- **RFUSE, FAST 2024.** A research kernel with per-core ring buffers
  "reduces communication latency by 53%" and reaches "2.27x higher
  throughput than FUSE in the random read workload" on fast NVMe; the
  paper calls stock FUSE's overhead "notable" on modern hardware
  (INSPECTED, one pass,
  [paper](https://usenix.org/system/files/fast24-cho.pdf)).
- **FSKit (macOS).** In a forum thread on a minimal FSKit file system with
  `getdirentries` averaging 121 µs, Apple's engineer replied: "that's
  about where things stand at the moment", "most of this is overhead of
  repeated XPC traffic, which probably could be avoided", and "the
  performance is definitely being looked at" (RELAYED, two passes,
  [forum thread 793013](https://developer.apple.com/forums/thread/793013),
  July 2025).
- **Windows loopback SMB.** No current measurement was found.

### How the gap is closing on Linux

- **Passthrough (6.9)** is "designed to improve the performance of FUSE
  filesystems for I/O operations": reads, writes, `splice` and `mmap` on
  a FUSE file "bypass the userspace daemon"; it needs `CAP_SYS_ADMIN`
  (INSPECTED, two passes,
  [passthrough](https://docs.kernel.org/filesystems/fuse/fuse-passthrough.html)).
- **io_uring (6.14)** "adds FUSE support for io-uring communication between
  kernel and userspace, with the purpose of increasing FUSE performance";
  "every CPU core has its own fuse-io-uring queue", and the interface "is
  still in development and might change" (INSPECTED, two passes,
  [Linux 6.14](https://kernelnewbies.org/Linux_6.14),
  [io_uring](https://docs.kernel.org/filesystems/fuse/fuse-io-uring.html)).

### Network clients in user space

- No study was found that compares a network file system's FUSE client
  with its in-kernel twin and shows the user-space cost vanishing behind
  the network.
- Production FUSE network clients exist, with vendor figures and no
  published method: Mountpoint for Amazon S3 (Rust, FUSE) and Cloud Storage
  FUSE, whose README credits its speed-ups to its file cache (RELAYED, one
  pass, [Mountpoint](https://github.com/awslabs/mountpoint-s3),
  [Cloud Storage FUSE](https://github.com/GoogleCloudPlatform/gcsfuse)).
- The counter-example: users report Ceph's kernel client much faster than
  `ceph-fuse` on fast clusters (RELAYED, one pass, anecdotal).

### What this means for IronAFS (INFERRED)

- A FUSE network client pays FUSE's cost most on metadata: lookups,
  attribute reads and creates, such as `ls -l` on a large directory, a
  build, or `find`. Bulk reads from a slow or distant server are bounded
  by the network and the Rx window first.
- IronAFS starts with the kernel's caches off (ADR 0004, L5, L11, L19), so
  every lookup and attribute read reaches the daemon. That costs speed on
  exactly those workloads, and is the price of per-user isolation until
  acceptance tests justify each relaxation.
- No IronAFS numbers exist. The benchmarks are planned in ADR 0004, O49.
