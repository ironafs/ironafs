<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0005: `ironafs-os`, the crate for the operating-system calls that need `unsafe`

Status: accepted (2026-11-21), by the owner, after the review rounds
recorded below. Raised in issue #43; the ADR 0004 amendment it needs
is issue #49, in the same pull request (#101). Amended 2026-11-21
(#57): rows 13 and 14 and sheets 13 and 14 (the socket type, `SCM_PIDFD`,
and where a received descriptor is an integer); amended 2026-11-22
(#152): point 5.3 and Decision 4, the target gate; and amended
2026-11-22 (#153): row 23 and its sheet, the route sockets' bounded
wait; amended 2026-11-24 (#146): sheet 14, complete GNU ABI proof,
glibc 2.28 compatibility evidence and musl withholding; amended 2026-11-25
(#187): point 2, the ordered Linux ABI header list.

## Context

- **ADR 0004 decides that this crate exists and what it is for.**
  - C4, point 3, as accepted, lists `ironafs-os` among the drivers and
    bindings: "small operating-system bindings".
  - C5, point 5, as accepted before the amendment this pull request
    makes (Consequences), gave it "the few calls that have no safe
    route": the clock pairs of C37, peer credentials (C43), the service controls, the
    log calls of O8, the dump controls of O26 and, on Windows, the FIPS
    algorithm-mode query (O31, point 4). It needs `unsafe`, so it "gets its
    own ADR with a closed list of named sites and safe interfaces on top,
    before any of it is written". This is that ADR.
  - O8, point 2, decides macOS logging through a one-file C shim whose call
    sites are named here. O26, point 1, puts the dump controls here. Part 3,
    Consequences, point 7, lists this ADR as owed: "the ADR for the
    OS-bindings crate (dump controls, the logging shim, clocks)".
  - C33, point 2, decides DNS discovery on Windows (`DnsQueryEx`) and macOS
    (`DNSServiceQueryRecord`). Neither has a safe route, and C5, point 5,
    as accepted, did not list them. Constraint 2 allows `unsafe` only in
    a named binding crate with its own ADR, and C4, point 3, names no other crate for
    operating-system calls, so they belong here (Decision, point 3). Any
    other home would be a new binding crate, which C4 would have to name
    first.
- **`unsafe` is forbidden workspace-wide** (`unsafe_code = "forbid"` in the
  root `Cargo.toml`; coding standards rule 5; `AGENTS.md`; ADR 0004,
  constraint 2: "`unsafe` only in named binding crates, each with its own
  ADR"). A crate that needs it gets an ADR first, then named sites, each
  with a `// SAFETY:` comment.
- **Dependencies are the supply chain** (coding standards rule 21; ADR 0004,
  constraint 3): IronAFS's own code first, a crate only for mainstream
  cryptography or the operating system's own interfaces, each one the
  owner's decision.
- **What this ADR's first list covers.** The owner decided the scope on
  2026-11-20: the needs below, which the Linux phases (M2a, M2b) and the
  macOS clocks, DNS and logging require, plus the Linux and macOS dump
  controls, plus the control endpoint's peer-credential and named-pipe
  calls on macOS and Windows, which the owner approved for this ADR so
  that they are settled before any IPC code is written (2026-11-20).
  The rest of C5, point 5's list comes by amendment in its phase
  (Consequences, "Not yet on the list"). The owner decided the M2a
  sweep's further needs on 2026-11-20: an IPv6-only callback
  socket, a free-space query and network-change notification join the
  list (item 8); large journal fields need no site, because each sink's
  complete encoded message is capped (Decision 1, point 7); and the
  descriptor hand-off's send side is already row 14's. The needs:
  1. **Two monotonic clocks per OS** (C37). A deadline a server grants (a
     callback, a token lifetime, a location lifetime, a lock) keeps running
     while the machine sleeps, so it is measured on a *boot clock* that
     counts suspend. Round-trip and retransmission timers are measured on
     an *awake clock* that does not. Comparing the two shows that a suspend
     happened (C37, point 4). Rust's `Instant` is one clock whose suspend
     behaviour is unspecified and differs by platform `[DES-017]`, and C37,
     point 3, uses it for neither. C37's third clock, wall time, comes from
     `std::time::SystemTime` and needs no site.
  2. **DNS SRV and AFSDB lookups on macOS and Windows** (C33, point 2;
     `[DES-008]` `[DES-010]`). The standard library resolves addresses
     only. On Linux and other Unix builds there is no DNS site: C33,
     point 3, decides one first-party client in safe Rust that queries the
     host's own local resolver over `std` sockets, outside this crate
     (C4, point 6). On macOS and Windows the operating system's resolver
     API is used, so that the OS chooses server and route, a VPN's
     included (C33, point 2).
  3. **macOS unified logging from the daemon itself** (O8, point 2).
     `os_log` and its siblings are C macros over a private entry point
     (`<os/log.h>` in the macOS SDK: `os_log_with_type` expands to a call of
     `_os_log_impl` with the image's `__dso_handle`, and requires that the
     format "must be a constant string"; INSPECTED 2026-11-19). A macro
     cannot be declared in Rust, so a few lines of C must do the call
     `[DES-020]` `[DES-021]`.
  4. **Dump controls on Linux and macOS** (O25, Firm: dumps disabled
     "from startup until exit"; O26, points 1, 2 and 4; part 3,
     Consequences, point 7, names them for this ADR).
  5. **Three Linux calls with no stable safe route in `std`:** the mount
     step's FUSE descriptor hand-off to `ironafsd` and the mount call
     itself (L7, point 1), and the token sender's peer check with
     `SO_PEERCRED` (C43, point 4). `UnixStream::peer_cred` and the
     `SCM_RIGHTS` ancillary-data methods are nightly-only in `std`
     (`peer_credentials_unix_socket`, #42839;
     `unix_socket_ancillary_data`, #76915;
     [UnixStream](https://doc.rust-lang.org/std/os/unix/net/struct.UnixStream.html),
     INSPECTED 2026-11-20), and the workspace builds on the pinned stable
     toolchain (L42, point 6).
  6. **The operating system's random source** (C4, point 7; C5, point 1).
     The production `Rng` is the cryptographic random source behind
     `ironafs-crypto`; the owner decided on 2026-11-20 that it is the
     operating system's own source, reached through sites here and
     injected through `ironafs-crypto`'s `Rng` seam. `std` has no public
     random-bytes interface.
  7. **The control endpoint's caller and server checks on macOS and
     Windows** (C43, C45, point 1; owner, 2026-11-20). On
     macOS the peer's uid comes from `LOCAL_PEERCRED`, which `std` does
     not expose. On Windows the daemon creates its pipe with a security
     descriptor, serves it with overlapped I/O, and learns the caller by
     impersonating it; a token sender checks that the pipe's server
     process carries the pinned service SID. `std` has no named-pipe
     server, no pipe security descriptor and no token interface. The
     rows are approved now; their code is written in M4 (macOS) and M5
     (Windows), with the control IPC design.
  8. **Three Linux network and file-system calls `std` cannot make**
     (owner, 2026-11-20). `std`'s `UdpSocket` binds when it is
     created and has no IPv6-only setter, so `IPV6_V6ONLY` cannot be set
     before bind through it (C30, point 1; C34, point 2). On Linux, with
     the option off, the IPv6 wildcard overlaps the IPv4 wildcard and a
     second bind on the same port fails with `EADDRINUSE`; whether it is
     off by default depends on the `bindv6only` sysctl; Windows defaults
     it on (CONFIRMED by the batch 5 check: Microsoft's dual-stack
     sockets page, ipv6(7), bind(2)). `std` has no `statvfs` (C27,
     point 3) and no `AF_NETLINK` (C23, point 2; C33, points 4 and 6).
     `fstatvfs` was in the x86-64 glibc ABI before 2.17 (CONFIRMED, LSB
     AMD64). rtnetlink(7) gives event-driven link, address and route
     notifications, with policy rules in groups of their own; an IPv6
     router advertisement can raise a "new address" message on a lifetime
     refresh alone (CONFIRMED by the same check). Whether a plain write to
     an unconnected netlink socket reaches the kernel on every candidate
     kernel is UNVERIFIED, so the site addresses the kernel explicitly.

  Items 4 to 8, and the DNS calls of item 2, are not all named by C5,
  point 5's list. The pull request that proposes this ADR also amends that
  list (C5 is Working; `docs/adr/README.md`, "Amending a working
  decision"), with the text in Consequences, "Amendment to C5, point 5",
  so the two agree.
- **What the operating systems document: clocks.**

  | OS | Interface | Documented behaviour | Source |
  |---|---|---|---|
  | Linux | `clock_gettime`, `CLOCK_MONOTONIC` | "This clock does not count time that the system is suspended." | `[DES-012]`; [clock_gettime(2)](https://man7.org/linux/man-pages/man2/clock_gettime.2.html) |
  | Linux | `clock_gettime`, `CLOCK_BOOTTIME` (since Linux 2.6.39) | "identical to CLOCK_MONOTONIC, except that it also includes any time that the system is suspended" | same |
  | macOS | `clock_gettime_nsec_np`, `CLOCK_UPTIME_RAW` | "does not increment while the system is asleep" | `[DES-013]`; clock_gettime(3), macOS manual |
  | macOS | `clock_gettime_nsec_np`, `CLOCK_MONOTONIC_RAW` | "like CLOCK_MONOTONIC", which "will continue to increment while the system is asleep", but "unaffected by frequency or time adjustments" | `[DES-013]` `[DES-014]` |
  | macOS | `clock_gettime_nsec_np` | returns 64-bit nanoseconds; "A 0 return value indicates an error occurred"; `EINVAL` for an invalid clock; since 10.12 | clock_gettime(3) |
  | Windows | `GetTickCount64` | milliseconds since the system started; no parameters; resolution "typically in the range of 10 milliseconds to 16 milliseconds"; "To retrieve elapsed time that accounts for sleep or hibernation, use the GetTickCount or GetTickCount64 function" | `[DES-016]`; [GetTickCount64](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-gettickcount64), [Interrupt Time](https://learn.microsoft.com/en-us/windows/win32/sysinfo/interrupt-time) |
  | Windows | `QueryUnbiasedInterruptTimePrecise` | "does not include time the system spends in sleep or hibernation"; units of 100 nanoseconds; one out pointer; returns nothing; Windows 10 and later | `[DES-015]`; [QueryUnbiasedInterruptTimePrecise](https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-queryunbiasedinterrupttimeprecise) |

  All rows INSPECTED 2026-11-19. That each clock behaves as documented
  across sleep, hibernation, Modern Standby and a paused virtual machine,
  on each supported release, is MISSING: it is measured in M2 (C37's
  revisit trigger) before any deadline relies on it.
- **What the operating systems document: DNS.**

  | OS | Interface | Documented behaviour | Source |
  |---|---|---|---|
  | Windows | `DNS_STATUS DnsQueryEx(PDNS_QUERY_REQUEST, PDNS_QUERY_RESULT, PDNS_QUERY_CANCEL)` | synchronous when the request's completion callback is NULL; returns `DNS_REQUEST_PENDING` only when it completes asynchronously; on input the result's version "must be DNS_QUERY_RESULTS_VERSION1 and all other members should be NULL"; Dnsapi.dll; Windows 8 and later | `[DES-004]`; [DnsQueryEx](https://learn.microsoft.com/en-us/windows/win32/api/windns/nf-windns-dnsqueryex) |
  | Windows | `DNS_QUERY_REQUEST` (version 1) | `Version`, `QueryName` (`PCWSTR`), `QueryType` (`WORD`), `QueryOptions` (`ULONG64`), `pDnsServerList`, `InterfaceIndex` ("If InterfaceIndex is 0, all interfaces will be considered"), `pQueryCompletionCallback` ("If NULL, DnsQueryEx is called synchronously"), `pQueryContext` | `[DES-005]`; [DNS_QUERY_REQUEST](https://learn.microsoft.com/en-us/windows/win32/api/windns/ns-windns-dns_query_request) |
  | Windows | `DNS_QUERY_RESULT` | `Version`, `QueryStatus`, `QueryOptions`, `pQueryRecords`, `Reserved`; "Applications must free returned RR sets" | [DNS_QUERY_RESULT](https://learn.microsoft.com/en-us/windows/win32/api/windns/ns-windns-dns_query_result) |
  | Windows | `DNS_RECORDW`, `DNS_SRV_DATAW`, `DNS_MX_DATAW` | a linked list (`pNext`); `wType` "determines the format of Data"; SRV data is `pNameTarget`, `wPriority`, `wWeight`, `wPort`, `Pad`; AFSDB uses the MX layout (`pNameExchange`, `wPreference`, `Pad`) | [DNS_RECORDW](https://learn.microsoft.com/en-us/windows/win32/api/windnsdef/ns-windnsdef-dns_recordw), [DNS_SRV_DATAW](https://learn.microsoft.com/en-us/windows/win32/api/windnsdef/ns-windnsdef-dns_srv_dataw), [DNS_MX_DATAW](https://learn.microsoft.com/en-us/windows/win32/api/windnsdef/ns-windnsdef-dns_mx_dataw) |
  | Windows | `VOID DnsFree(PVOID, DNS_FREE_TYPE)`, `DnsFreeRecordList` | frees a record list "and includes subfields of the DNS_RECORD structure"; Dnsapi.dll. `DnsRecordListFree` is documented as a macro in `windns.h`, so the function is declared instead | [DnsFree](https://learn.microsoft.com/en-us/windows/win32/api/windns/nf-windns-dnsfree), [DNS_FREE_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/windns/ne-windns-dns_free_type), [DnsRecordListFree](https://learn.microsoft.com/en-us/windows/win32/api/windns/nf-windns-dnsrecordlistfree) |
  | macOS | `DNSServiceQueryRecord(DNSServiceRef *, DNSServiceFlags, uint32_t, const char *, uint16_t, uint16_t, DNSServiceQueryRecordReply, void *)` | on an error return "the callback is never invoked and the DNSServiceRef is not initialized"; on success the query "will remain active indefinitely until the client terminates it" with `DNSServiceRefDeallocate`; with `kDNSServiceFlagsReturnIntermediates` an authoritative negative answer is returned | `[DES-006]`; `dns_sd.h`; [DNSServiceQueryRecord](https://developer.apple.com/documentation/dnssd/1804747-dnsservicequeryrecord) |
  | macOS | `DNSServiceQueryRecordReply` | the callback receives `rdlen`, "the length, in bytes, of the resource record rdata", and `rdata`, "the raw rdata of the resource record"; `errorCode` "Will be kDNSServiceErr_NoError on success ... Other parameters are undefined if errorCode is nonzero"; `DNSSD_API` is empty outside Windows, so the C calling convention applies | `dns_sd.h` |
  | macOS | `DNSServiceRefSockFD`, `DNSServiceProcessResult`, `DNSServiceRefDeallocate` | the descriptor is for "a select() loop"; `DNSServiceProcessResult` "will block until the daemon's response is received" and calls the callback; the API "does no locking internally", and deallocating a reference while another thread processes it crashes | `dns_sd.h` |
  | macOS | `int poll(struct pollfd *, nfds_t, int)` | `struct pollfd` is `int fd; short events; short revents`; `nfds_t` is `unsigned int`; `POLLIN` is `0x0001` | `<sys/poll.h>` in the macOS SDK; poll(2) |

  `dns_sd.h` and `<sys/poll.h>` were read in the macOS SDK shipped with the
  installed Xcode, INSPECTED 2026-11-20; the Microsoft pages, INSPECTED
  2026-11-20. Whether a NULL `pDnsServerList` means the system's own
  server list, as C33, point 2, requires, is not stated on Microsoft's
  pages for `DnsQueryEx`: MISSING until the Windows check of Decision 5.6
  runs. Two pages point that way without settling it: the
  [`DNS_QUERY_REQUEST3`](https://learn.microsoft.com/en-us/windows/win32/api/windns/ns-windns-dns_query_request3)
  page says custom servers "bypass the system-configured DNS servers",
  that a name matching a Name Resolution Policy Table rule uses "only the
  servers from the NRPT rule", and that "At least one of pCustomServers
  and pDnsServerList must be NULL"; and the archived Windows Embedded
  Compact 2013 page for `DnsQuery` says "If this is NULL, default DNS
  servers for the local computer are used", for a different product.
  Microsoft's
  [DNS client resolution timeouts](https://learn.microsoft.com/en-us/troubleshoot/windows-server/networking/dns-client-resolution-timeouts)
  article gives the default schedule for servers configured on one
  adapter: queries at 0, 1, 2, 4 and 8 seconds, and "client stops
  querying" at 10 seconds. It does not name `DnsQueryEx`, encrypted DNS
  or several adapters.
- **What the operating systems document: the random source.**

  | OS | Interface | Documented behaviour | Source |
  |---|---|---|---|
  | Linux | the `getrandom` system call, through `long syscall(long, ...)` with `SYS_getrandom` | "Linux 3.17, glibc 2.25": glibc's own wrapper exists only from 2.25 (its `NEWS`: "The getentropy and getrandom functions, and the <sys/random.h> header file have been added"), so a build that must run on glibc 2.17 (L42, point 3) uses the raw call; by default it "draws entropy from the urandom source" and, if that "has not yet been initialized", blocks; "If the entropy pool has been initialized and the request size is small (size <= 256), then getrandom() will not fail with EINTR. Instead, it will return all of the bytes that have been requested", but while the pool is not yet initialised a signal makes the call fail with `EINTR` whatever the size; "The user of getrandom() must always check the return value"; `ENOSYS` where the kernel lacks the call. System-call number 318 on x86_64 and 278 on aarch64 (the generic table) | [getrandom(2)](https://man7.org/linux/man-pages/man2/getrandom.2.html); [glibc 2.25 `NEWS`](https://sourceware.org/git/?p=glibc.git;a=blob;f=NEWS;hb=refs/tags/glibc-2.25); [syscall_64.tbl](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/arch/x86/entry/syscalls/syscall_64.tbl), [asm-generic/unistd.h](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/asm-generic/unistd.h); CONFIRMED 2026-11-20 |
  | macOS | `int SecRandomCopyBytes(SecRandomRef, size_t, void *)` | "Return count random bytes in *bytes"; "Return 0 on success, any other value on failure"; `rnd` is nullable, `kSecRandomDefault` "is a synonym for NULL", and "If rnd is unrecognized or unsupported, kSecRandomDefault is used"; Apple: "Generates an array of cryptographically secure random bytes", "Always test the returned status"; no size limit stated; since macOS 10.7; Security.framework | `SecRandom.h` in the macOS SDK, INSPECTED 2026-11-20; [SecRandomCopyBytes](https://developer.apple.com/documentation/security/secrandomcopybytes(_:_:_:)), CONFIRMED 2026-11-20 |
  | macOS | `int getentropy(void *, size_t)` (in libSystem) | since macOS 10.12; `<sys/random.h>`; "The maximum buffer size permitted is 256 bytes. If buflen exceeds this, an error of EIO will be indicated"; 0 or −1 with `errno` (`EINVAL`, `EIO`); "primarily intended for use in the construction and seeding of userspace PRNGs ... Clients who simply require random data should use arc4random(3), CCRandomGenerateBytes() from CC_crypto(3), or SecRandomCopyBytes() from the Security framework instead" | `<sys/random.h>` in the macOS SDK and getentropy(2) in the macOS 27.2 manual, CONFIRMED 2026-11-20 |
  | Windows | `NTSTATUS BCryptGenRandom(BCRYPT_ALG_HANDLE, PUCHAR, ULONG, ULONG)` with a NULL handle and `BCRYPT_USE_SYSTEM_PREFERRED_RNG` (0x00000002) | "Use the system-preferred random number generator algorithm. The hAlgorithm parameter must be NULL"; the default provider follows NIST SP800-90 CTR_DRBG; returns `STATUS_SUCCESS`, `STATUS_INVALID_HANDLE` or `STATUS_INVALID_PARAMETER` among others; Windows Vista SP2 and later for the flag; Bcrypt.dll, Bcrypt.lib. `NT_SUCCESS` is true for 0 to 0x7FFFFFFF only, so a negative value as a signed 32-bit integer is a warning or an error | [BCryptGenRandom](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom); [Using NTSTATUS values](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/using-ntstatus-values); CONFIRMED 2026-11-20 |
  | Linux (RHEL 7) | `getrandom` on the kernel of the conditional RHEL 7.9 artefact | Red Hat backported the call into kernel-3.10.0-544.el7 (RHEL 7.4): "This update adds the getrandom system call to the Linux kernel on AMD64 and Intel 64 architectures"; RHEL 7.9 is later, so its x86_64 kernel has it. Other RHEL 7 architectures are not stated | [Red Hat bug 1330000](https://bugzilla.redhat.com/show_bug.cgi?id=1330000); [Red Hat blog](https://www.redhat.com/en/blog/understanding-red-hat-enterprise-linux-random-number-generator-interface) ("introduced to Red Hat Enterprise Linux in 7.4"); CONFIRMED 2026-11-20 for x86_64 |

  `getrandom(2)` describes `ENOSYS` as what glibc's wrapper reports; the
  raw call on a kernel without it fails with the same `ENOSYS`, which Red
  Hat's article tells callers of `syscall` to check for.
- **What the operating systems document: Linux sockets, mount and dumps.**

  | Interface | Documented behaviour | Source |
  |---|---|---|
  | `SO_PEERCRED` | "returns the credentials of the peer process connected to this socket"; "those that were in effect at the time of the call to connect(2), listen(2), or socketpair(2)"; the argument "is a pointer to a ucred structure" (`pid_t pid; uid_t uid; gid_t gid;`); only "for connected AF_UNIX stream sockets and for AF_UNIX stream and datagram socket pairs" | [unix(7)](https://man7.org/linux/man-pages/man7/unix.7.html); musl's `<sys/socket.h>` (same three fields); CONFIRMED 2026-11-20 |
  | `SCM_RIGHTS` | at most `SCM_MAX_FD` (253) descriptors per message; if the control buffer is too small, `MSG_CTRUNC` is set and "the excess file descriptors are automatically closed in the receiving process" | [unix(7)](https://man7.org/linux/man-pages/man7/unix.7.html); CONFIRMED 2026-11-20 |
  | `MSG_CMSG_CLOEXEC` | `recvmsg` only, since Linux 2.6.23; sets close-on-exec on descriptors received with `SCM_RIGHTS` | [recvmsg(2)](https://man7.org/linux/man-pages/man2/recvmsg.2.html); CONFIRMED 2026-11-20 |
  | `struct msghdr`, `struct cmsghdr` | glibc (as documented): `msg_iovlen`, `msg_controllen` and `cmsg_len` are `size_t`; `CMSG_SPACE` and `CMSG_LEN` are Linux-specific macros. musl: `msg_iovlen` is `int`, `msg_controllen` and `cmsg_len` are `socklen_t`, each next to an `int __pad` field placed by byte order on 64-bit targets, so sizes and offsets match glibc's on the little-endian targets of point 5.3 but the field types differ | [recvmsg(2)](https://man7.org/linux/man-pages/man2/recvmsg.2.html), [cmsg(3)](https://man7.org/linux/man-pages/man3/cmsg.3.html); musl [`include/sys/socket.h`](https://git.musl-libc.org/cgit/musl/tree/include/sys/socket.h); CONFIRMED 2026-11-20 |
  | `mount` for `fuse` | `int mount(const char *source, const char *target, const char *filesystemtype, unsigned long mountflags, const void *data)`; needs `CAP_SYS_ADMIN`; `MS_NOSUID`, `MS_NODEV` as named; the options `fd=` ("must have been obtained by opening the FUSE device"), `rootmode=`, `user_id=`, `group_id=`, `allow_other` | [mount(2)](https://man7.org/linux/man-pages/man2/mount.2.html); [FUSE, kernel documentation](https://docs.kernel.org/filesystems/fuse/fuse.html); CONFIRMED 2026-11-20 |
  | `PR_SET_DUMPABLE`, `PR_GET_DUMPABLE` | the attribute "determines whether core dumps are produced"; values 0 and 1; reset to `/proc/sys/fs/suid_dumpable` when the effective or filesystem user or group id changes, or on an exec of a set-id program or one with file capabilities; non-dumpable processes cannot be attached with `PTRACE_ATTACH`; since Linux 2.3.20 | [PR_SET_DUMPABLE(2const)](https://man7.org/linux/man-pages/man2/PR_SET_DUMPABLE.2const.html), [PR_GET_DUMPABLE(2const)](https://man7.org/linux/man-pages/man2/PR_GET_DUMPABLE.2const.html); CONFIRMED 2026-11-20 |
  | `RLIMIT_CORE` (Linux) | a limit of zero prevents a dump, "However, RLIMIT_CORE will be ignored if the system is configured to pipe core dumps to a program" | [core(5)](https://man7.org/linux/man-pages/man5/core.5.html); CONFIRMED 2026-11-20 |
  | `MADV_DONTDUMP` | since Linux 3.4: "Exclude from a core dump those pages in the range"; "addr must be page-aligned" (`EINVAL` otherwise) | [madvise(2)](https://man7.org/linux/man-pages/man2/madvise.2.html); CONFIRMED 2026-11-20 |
  | `RLIMIT_CORE` (macOS) | "The maximum size of a core file is limited by setrlimit(2). Files which would be larger than the limit are not created"; "Core dumps are disabled by default"; "To disable core dumps, set the limit to 0". macOS has no `PR_SET_DUMPABLE` | core(5) and setrlimit(2) in the macOS 27.2 manual; CONFIRMED 2026-11-20 |
  | `poll` (Linux) | `int poll(struct pollfd *fds, nfds_t nfds, int timeout)`; `struct pollfd` is `int fd; short events; short revents`; "Specifying a negative value in timeout means an infinite timeout", zero returns at once, and the interval "will be rounded up to the system clock granularity" and "may overrun by a small amount"; 0 means the call "timed out before any file descriptors became ready"; `EINTR`: "A signal occurred before any requested event"; `POLLERR`, `POLLHUP` and `POLLNVAL` are "only returned in revents; ignored in events" | [poll(2)](https://man7.org/linux/man-pages/man2/poll.2.html); CONFIRMED 2026-11-22. The width of `nfds_t` in glibc and musl is not on the page: UNVERIFIED until point 5.6's check |
  | `eventfd` | `int eventfd(unsigned int initval, int flags)`; `EFD_CLOEXEC` and `EFD_NONBLOCK` since Linux 2.6.27; the wrapper since glibc 2.8. A read of fewer than 8 bytes fails with `EINVAL`; a read of a non-zero counter returns it and "the counter's value is reset to zero"; a zero counter fails a non-blocking read with `EAGAIN`; a write adds its 8-byte value, and one that would pass 0xfffffffffffffffe fails a non-blocking write with `EAGAIN`; the descriptor is readable "if the counter has a value greater than 0"; an overflowed counter makes `poll` return `POLLERR` | [eventfd(2)](https://man7.org/linux/man-pages/man2/eventfd.2.html); CONFIRMED 2026-11-22 |
  | `clock_gettime` in glibc 2.17 | "The `clock_*' suite of functions ... is now available directly in the main C library. Previously it was necessary to link with -lrt" (2.17) | glibc `NEWS`, 2.17 entry; CONFIRMED 2026-11-20 |

- **What the operating systems document: the control endpoint.** The
  facts checked for the control IPC specification (planned), and the macOS SDK
  headers.

  | OS | Interface | Documented behaviour | Source |
  |---|---|---|---|
  | macOS | `getsockopt` with `SOL_LOCAL` (0) and `LOCAL_PEERCRED` (0x001), into a `struct xucred` | the peer's effective "user ID and group list" on a `SOCK_STREAM` socket, as they were at `connect` or `listen`; `struct xucred` is `u_int cr_version; uid_t cr_uid; short cr_ngroups; gid_t cr_groups[NGROUPS]`, `NGROUPS` is `NGROUPS_MAX` (16), `XUCRED_VERSION` is 0; `LOCAL_PEERPID` (0x002) is in the header but not on the manual page | unix(4), macOS 26.7; `<sys/un.h>`, `<sys/ucred.h>`, `<sys/param.h>`, `<sys/syslimits.h>` in the macOS SDK, INSPECTED 2026-11-20 |
  | Windows | `CreateNamedPipeW` | `PIPE_REJECT_REMOTE_CLIENTS` rejects remote clients; `FILE_FLAG_FIRST_PIPE_INSTANCE` makes a later creation fail with `ERROR_ACCESS_DENIED`; `nMaxInstances` is 1 to 255, and 255 (`PIPE_UNLIMITED_INSTANCES`) means no limit but the system's; the type, mode, instance count and time-out must match on every instance | [CreateNamedPipeW](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew); CONFIRMED 2026-11-20 (control IPC draft, 5.1) |
  | Windows | pipe security | `FILE_APPEND_DATA` and `FILE_CREATE_PIPE_INSTANCE` are the same bit, so `FILE_GENERIC_WRITE` must not be granted; with no descriptor the default grants read to Everyone and the anonymous account | [Named Pipe Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights); CONFIRMED 2026-11-20 (same) |
  | Windows | `ImpersonateNamedPipeClient` | impersonates "the security context of the last message read from the pipe"; error 1368, `ERROR_CANNOT_IMPERSONATE`: "Unable to impersonate using a named pipe until data has been read from that pipe"; on failure "no client requests should be executed" | [ImpersonateNamedPipeClient](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-impersonatenamedpipeclient), [System Error Codes 1300-1699](https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--1300-1699-); CONFIRMED 2026-11-20 (same, and a second family's check) |
  | Windows | token classes | `TokenUser`; `TOKEN_STATISTICS.AuthenticationId`, "an LUID assigned to the session this token represents"; `TokenLinkedToken` returns a handle that must be closed; at `SecurityAnonymous` the server "cannot impersonate or identify the client" | [TOKEN_INFORMATION_CLASS](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ne-winnt-token_information_class), [TOKEN_STATISTICS](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-token_statistics), [Impersonation Levels](https://learn.microsoft.com/en-us/windows/win32/secauthz/impersonation-levels); CONFIRMED 2026-11-20 (same) |
  | Windows | `GetNamedPipeServerProcessId`, `GetNamedPipeClientProcessId` | return a process id; a process id alone proves nothing, and it can be reused before a handle to the process is opened | [GetNamedPipeServerProcessId](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeserverprocessid), [Process Handles and Identifiers](https://learn.microsoft.com/en-us/windows/win32/procthread/process-handles-and-identifiers); CONFIRMED 2026-11-20 (same) |
  | Windows | overlapped I/O and `CancelIoEx` | an overlapped operation's `OVERLAPPED` structure and buffer must stay valid until the operation completes, a cancelled one included | [Synchronous and Overlapped I/O](https://learn.microsoft.com/en-us/windows/win32/ipc/synchronous-and-overlapped-input-and-output), [CancelIoEx](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelioex); cited by a second family's check, not read here: UNVERIFIED |

  `OpenThreadToken`, `RevertToSelf`, `GetTokenInformation`,
  `CheckTokenMembership`, `OpenProcess`, `OpenProcessToken`,
  `ConnectNamedPipe`, `DisconnectNamedPipe`, `ReadFile`, `WriteFile` and
  the completion calls were not read for this ADR: their prototypes,
  libraries and failure rules are UNVERIFIED here, and each is read at
  source before its row's code (point 5.6 then proves the declaration).

- **Rust's rules for foreign declarations** (edition 2024, which the
  workspace uses):
  - an `extern` block must be written `unsafe extern`;
  - a function in it is unsafe to call unless it is declared `safe fn`
    ([Reference, external blocks](https://doc.rust-lang.org/reference/items/external-blocks.html);
    `[DES-018]`);
  - the compiler cannot check a declaration against the C library. "Calling
    a function with the wrong call ABI" is undefined behaviour
    ([Reference](https://doc.rust-lang.org/reference/behavior-considered-undefined.html)),
    and a wrong parameter type, struct layout or constant fails just as
    silently. Whoever writes the declaration owns its correctness;
  - since Rust 1.81, "the non-unwind ABIs (e.g., "C") will now abort on
    uncaught unwinds"
    ([Rust 1.81.0](https://blog.rust-lang.org/2024/09/05/Rust-1.81.0/);
    INSPECTED 2026-11-20), so a panic in a Rust function that foreign code
    calls ends the process instead of crossing the foreign frame.
- **Lint mechanics.**
  - `forbid` cannot be lowered in source: an `#[expect(unsafe_code)]` under
    `forbid` is error E0453.
  - Under `deny`, the `unsafe_code` lint fires on an `unsafe extern` block
    as well as on an `unsafe` block, and a named `#[expect]` on each
    satisfies it.
  - A `safe fn` in such a block is called with no `unsafe` block.
  - The same lint fires on an `unsafe impl` ("implementation of an `unsafe`
    trait"), and a named `#[expect]` satisfies it there too (EXECUTED
    2026-11-20 with rustc 1.97.1 on a scratch file; to be repeated on the
    pinned toolchain with the crate's first change).

  The first three: EXECUTED 2026-11-19 with the pinned rustc 1.100.0 (L42, point 6) on a
  macOS arm64 development machine, on scratch files outside the repository.
  Cargo's documentation shows `[lints] workspace = true` only as
  whole-table inheritance
  ([Cargo, workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)),
  and rule 5 records that it cannot be combined with a crate-level
  override.
- **Miri** "has no access to most platform-specific APIs or FFI"
  ([Miri README](https://github.com/rust-lang/miri)). It can check the Rust
  on either side of a foreign call, not the call.
- **The owner decided** on 2026-11-19 that these needs are met by `unsafe`
  foreign calls confined to one crate, with a closed list of sites, and that
  the macOS C file is built without the `cc` crate. ADR 0004 records the
  same in C5, point 5, and O8, point 2.

## Decision

Each decision is marked **Firm** or **Working**, with the meaning ADR 0004
gives them (overview, "How this ADR is organised", point 2). The firm ones
are security invariants: where `unsafe` may appear, and what every site
must prove.

### 1. One crate, safe on the outside

**Firm** (points 1, 2 and 5, security invariants: where `unsafe` may be,
and that nothing unsafe leaves the crate). **Working** (points 3, 4, 6
and 7).
Revisit point 3 if: a caller needs policy inside a site to stay sound.
Revisit point 7 if: a sink needs a site (a large-entry route), or a
receiver qualifies a larger message limit than point 7's fallback.
Revisit point 4 if: the upkeep of point 5.2 proves larger than expected
(the `libc` crate is the named fallback, point 5.1).
Revisit point 6 if: C33 is revisited.

1. Until another binding crate's ADR is accepted, `ironafs-os` is the only
   crate in the workspace that may contain `unsafe`, an `extern` block, a
   `#[link]` attribute, a build script that compiles or runs foreign
   code or a native helper, or non-Rust source. Not covered here, each with its own ADR (ADR 0004,
   constraint 2): Kerberos and GSS (`ironafs-gss`, C4, point 3; C5,
   point 4) and the Cloud Files replica (`ironafs-cloudfiles`, L27). The
   FSKit extension is Swift and contains no Rust (C4, point 5), so it has
   no binding here; it sets its own dump limit in Swift (O26, point 4).
2. **Its public API is safe Rust.** It exports no `unsafe fn`, no `unsafe
   trait`, no raw pointer, no foreign type and no foreign declaration. Every
   function returns a value in IronAFS's own types, a standard-library
   owner type (`OwnedFd`, `OwnedHandle`), or a typed error (rule 4; C46). No caller can
   cause undefined behaviour through it, whatever it passes, from whatever
   thread. No function of the crate may be called from a signal handler,
   and the crate installs none; it never forks.
3. **It holds mechanism, not policy.** It reads clocks, fills a buffer
   with random bytes, runs one DNS query, writes one log entry, applies
   and reads back the dump settings, reads a peer's credentials, passes or
   receives one descriptor, makes one mount call, creates and serves one
   named pipe, reads what a pipe's caller or server token says, creates
   one IPv6-only UDP socket, reads one filesystem's free space, opens,
   writes to and reads from one route-notification socket, waits for it
   or a wake with a finite timeout, and overwrites
   one byte buffer with zeros in a way the compiler keeps.
   Which caller is an administrator, and which server is the daemon, is
   decided by its callers. What a clock jump means (C37, points 4 and 5),
   how DNS answers are ranked and cached (C33, points 1 and 6) and what is
   logged (O1 to O8) are decided by its callers. It parses no fileserver
   data. DNS answers leave it as bounded, owned copies (Decision 3) and are
   decoded and validated outside it, in safe, fuzzed code (rules 1 and 9).
4. **It has no dependencies**, in any dependency table.
5. **Lint levels.** `ironafs-os` is the one crate that does not inherit the
   workspace lint table. Its manifest carries its own `[lints]` table, equal
   to the workspace's except for `unsafe_code = "deny"`. A `cargo xtask
   check` step fails if the two tables differ in anything else, if any other
   crate does not inherit the workspace table, or if an `unsafe` token (a
   block, a function, a trait or an `unsafe impl`), an `extern` block or a
   `#[link]` attribute appears outside this crate (or
   outside a binding crate whose ADR is accepted and named in the check's
   own list). `unsafe_op_in_unsafe_fn` and
   `clippy::undocumented_unsafe_blocks` stay denied. Point 1's other two
   clauses (a build script that compiles or runs foreign code or a
   native helper, and non-Rust source, outside this crate) are not in
   this check: they are enforced by review, under point 6.6, which makes
   any change in any crate that adds non-Rust source, or adds or edits
   a build script so that it does either, class S, and by coding
   standards rule 20, which asks a written reason for any `build.rs`
   (a named list of build scripts comes with the pending layout change;
   Consequences). A build script can run native code without any
   `extern` block, so the check above does not catch it.
6. **DNS on Linux and other Unix builds is not in this crate** (C33,
   point 3; C4, point 6). One first-party DNS client in safe Rust, the same
   for glibc and musl builds, queries only the host's own established local
   resolver, detected at run time, through `std` UDP and TCP sockets; on a
   flat list of name servers only after an administrator confirms it as the
   intended policy (C33, point 4, and the overview's open question 1);
   otherwise explicitly configured servers (C33, point 5). Where a query
   goes stays the host resolver's decision. It needs no site.
7. **The log transport is a safe module of this crate** (owner,
   2026-11-20). On Linux, the journald native-protocol and syslog
   datagram writes (O8, point 1) reach `std`'s `UnixDatagram` with no
   `unsafe`, so they need no site; the module that makes them carries
   `#![forbid(unsafe_code)]` and holds transport only. It takes an
   already-encoded message and a sink, and returns "sent" or a typed
   error. Encoding and redaction stay in `ironafs-events` (O1, point 1;
   O5); bounded delivery and loss accounting stay in the callers (O7,
   point 3). `ironafsd` and `afs` share it (O4, point 1), and the macOS
   shim (point 4) and, later, the Windows Event Log (O8, point 3) are the
   other sinks of the same crate. The module is named in C4, point 3's
   driver table by the amendment in Consequences. There is no large-entry
   route (journald's sealed-`memfd` hand-off): each sink's *complete*
   encoded message, after escaping and framing, is capped by a named
   constant:
   - `SYSLOG_FALLBACK_MAX` = 480 octets for the RFC 5424 fallback (the
     size RFC 5424, section 6.1, guarantees across conforming
     receivers), until a larger limit is qualified on the selected
     receiver and transport;
   - `JOURNALD_MAX_DATAGRAM` for the journald native protocol: 16 KiB
     (value confirmed by the owner, 2026-11-20), below the datagram limits the receiver and the socket
     buffer impose (UNVERIFIED; qualified on each supported systemd
     before the sink is used);
   - `OSLOG_MAX_TEXT` for each of the macOS shim's two texts: 1,024
     bytes each (value confirmed by the owner, 2026-11-20), NUL included (UNVERIFIED against unified
     logging's own truncation of dynamic strings); the shim's one
     constant format adds a fixed amount. Like journald's, this limit is
     qualified on each supported macOS release before the sink is used:
     until the read-back test below passes there, the macOS sink is not
     enabled in a release build for that release, and no audit or stable
     event is written through it.
   The module checks the cap and nothing else: a message over its sink's
   cap is not sent, and the call returns the typed error `OverSizeLimit`.
   Fitting an event to the cap is the encoder's (`ironafs-events`): a
   truncated field is marked, the event's identity and required fields
   are kept, and the loss is counted (O7, point 3). If the identity and
   required fields alone, encoded, exceed the cap, the event is **not
   delivered**, and its caller treats it as a loss, never as sent:
   - a diagnostic event with its own catalogue id (O2, point 1), "event
     dropped: over the size limit", carrying the dropped event's id (and,
     for an audit event, its sequence number), is sent in its place. It
     is a separate event: sending it never marks the original delivered;
   - for an audit event, the sequence assigned to it is recorded as a
     known loss and reported in a gap event with that exact range (O9,
     point 4); the checkpoint's delivered ranges never include it (O10,
     point 2);
   - the loss is counted.
   The replacement diagnostic and every gap event must themselves fit
   the cap of each sink they are sent to, `SYSLOG_FALLBACK_MAX` the
   smallest, so neither can fall under the rule above. The encoder
   (`ironafs-events`) splits a gap event whose ranges do not fit into
   several gap events, each holding whole ranges and fitting the cap; a
   range is never truncated, merged or dropped. Its specification
   states the encoded size of the replacement diagnostic and of a gap
   event holding one range, at the largest values their fields can
   take, and shows both within `SYSLOG_FALLBACK_MAX`; if that cannot be
   shown, the encoding is a design issue raised before the sink is
   enabled.
   The tests, per sink, measure after escaping and framing: a message at
   the cap, one octet over it (`OverSizeLimit`, nothing sent), and
   required fields alone over it, for a diagnostic and for an audit
   event: the audit case produces a gap event naming its sequence as a
   known loss, the diagnostic carries its own id, and the checkpoint
   never shows the original as delivered. A gap with more disjoint
   ranges than one gap event can hold under `SYSLOG_FALLBACK_MAX` is
   sent through the fallback, and every range is read back exactly
   through the fallback receiver. Each sink's test reads the
   message at the cap back through the sink's own retrieval (the journal,
   the fallback receiver, `log show` on macOS) and checks that the
   event's identity and required fields arrive whole. The fallback's
   test also reads back `MSGID` and the direct flag through the actual
   fallback receiver: that `/dev/log` handles RFC 5424 input is
   UNVERIFIED (systemd documents it as a classic RFC 3164 input).

### 2. The closed list

**Firm:** that the list is closed, and that the inventory check of point
6.7 enforces it. **Working:** each row. Revisit a row if: an operating
system ships a safe interface that removes it (C5's trigger), a supported
release changes the item's header or library (point 5.7), or the decision
it serves is revisited (C23, C27, C33, C34, C37, O8, C43, C45),
or the ABI proof needs a different direct C header. A row changes only
as point 7 says.

Every foreign item and every site is in this table. Anything not in it is
not allowed. Rows 21 to 23 are the M2a sweep's further needs (owner,
owner, 2026-11-20). Row 24 is the owner's cryptography decision
(2026-11-20): the wipe that O28, point 2, needs. A *site* is one `unsafe extern` block, one `unsafe` block or
one `unsafe impl`; its id is the text of its
`#[expect(unsafe_code, reason = "…")]`. The `unsafe` blocks column counts the
`unsafe` blocks as written in the source (a private function called from
two places is one block). Each row's safe interface, obligations and test
are in its sheet below the table, under the same number.

The **Linux ABI headers** cell gives the ordered list of direct,
row-specific C includes for point 5.6's Linux proof. It also governs
a qualified absence probe, even for a row whose Linux implementation
is withheld. The shared preamble's common includes are separate.
A row with no approved Linux ABI headers says `none`. Otherwise the
cell uses one code span per header, separated by exactly a comma and
space. A name is one or more `/`-separated path components and ends in
`.h`. Each component starts with an ASCII letter, digit or underscore
followed by zero or more ASCII letters, digits, underscores, periods or
hyphens. Empty spans, trailing delimiters and duplicate names fail.

The inventory compares the parsed list byte for byte, in order and
case-sensitively, with its sole mirror, `code.headers`. The five
existing text columns do not carry a second copy of the header cell.
Validate both lists independently before comparing them, rejecting
malformed and duplicate names even if both lists match. A row without
a code record has an empty effective mirror list. The approved cell
retains its own list, so a nonempty cell with no code record fails.

On Linux targets, the macro scan, C assertions and any qualified
absence probe include the row-specific headers in order from that
checked list. A mismatch fails inventory validation before any of those
C inputs is generated. The list does not supply includes for macOS or
Windows, which use their own approved proof inputs under point 5.
A row covering Linux and another operating system uses this list only
for its Linux C inputs. A row whose approved target scope excludes
Linux, or that declares no foreign items, must say `none`. The
inventory rejects headers on either kind of row even when the mirror
matches the cell.

Adding, removing, replacing or reordering a header changes the row
under point 7 and requires owner acceptance before the code is written.
The cell alone does not determine implementation status. An approved
row may have no proof inputs yet, as for row 23. Before that row's
Linux implementation is written, its amendment must name each needed
header here. The cell does not replace any platform proof.

| # | Site id | Targets | Foreign item, as the OS documents it | Linked from | Declared | `unsafe` blocks | Linux ABI headers |
|---|---|---|---|---|---|---|---|
| 1 | `linux-clock` | Linux, glibc and musl | `int clock_gettime(clockid_t, struct timespec *)`, called with `CLOCK_BOOTTIME` and `CLOCK_MONOTONIC` only | the C library | `unsafe fn` | one, in one private function taking the clock id | `time.h` |
| 2 | `darwin-clock` | macOS | `uint64_t clock_gettime_nsec_np(clockid_t)`, called with `CLOCK_MONOTONIC_RAW` and `CLOCK_UPTIME_RAW` only | libSystem | `safe fn` | none | `none` |
| 3 | `windows-tick` | Windows | `ULONGLONG GetTickCount64(void)` | Kernel32.dll | `safe fn` | none | `none` |
| 4 | `windows-awake` | Windows | `VOID QueryUnbiasedInterruptTimePrecise(PULONGLONG)` | see point 5.4 | `unsafe fn` | one | `none` |
| 5 | `darwin-oslog` | macOS | two functions of IronAFS's own C shim (point 4): create the log handles once; emit one entry (level, event id, public text, private text) | the shim, linked statically; it calls libSystem | `unsafe fn` | one per function | `none` |
| 6 | `windows-dns` | Windows | `DNS_STATUS DnsQueryEx(PDNS_QUERY_REQUEST, PDNS_QUERY_RESULT, PDNS_QUERY_CANCEL)`, called asynchronously with a completion routine and a cancel handle (point 3.2); `DNS_STATUS DnsCancelQuery(PDNS_QUERY_CANCEL)`; `VOID DnsFree(PVOID, DNS_FREE_TYPE)`, called with `DnsFreeRecordList` only; one Rust `extern "system"` function passed as the completion routine | Dnsapi.dll | `unsafe fn` | one per call (three); one that takes back the context's reference (`Arc::from_raw`); one per read of memory Windows wrote (the result, a record header, SRV data, MX data, a wide string: five); and two `unsafe impl` (`Send` and `Sync` for the query block) | `none` |
| 7 | `darwin-dns` | macOS | `DNSServiceQueryRecord`, `DNSServiceRefSockFD`, `DNSServiceProcessResult`, `DNSServiceRefDeallocate`, and `int poll(struct pollfd *, nfds_t, int)` on that one descriptor; one Rust `extern "C"` function passed as the reply callback (point 3.3) | libSystem | `unsafe fn` | one per call (five), and two in the callback (the context, the rdata) | `none` |
| 8 | `linux-random` | Linux, glibc and musl | `long syscall(long, ...)` called with `SYS_getrandom`, the caller's buffer, a length of at most 256 and flags 0 only, every argument passed at register width | the C library | `unsafe fn` | one | `errno.h`, `unistd.h`, `sys/syscall.h` |
| 9 | `darwin-random` | macOS | `int SecRandomCopyBytes(SecRandomRef, size_t, void *)` with a NULL `rnd` (`kSecRandomDefault`) only | Security.framework | `unsafe fn` | one | `none` |
| 10 | `windows-random` | Windows | `NTSTATUS BCryptGenRandom(BCRYPT_ALG_HANDLE, PUCHAR, ULONG, ULONG)` with a NULL handle and `BCRYPT_USE_SYSTEM_PREFERRED_RNG` only | Bcrypt.dll | `unsafe fn` | one | `none` |
| 11 | `unix-nodump` | Linux, macOS | `int setrlimit(int, const struct rlimit *)` with `RLIMIT_CORE` and `{0, 0}` only; on Linux also `int prctl(int, ...)` with `PR_SET_DUMPABLE` and 0 only, and `PR_GET_DUMPABLE` for the read-back | the C library; libSystem | `unsafe fn` | one per call: one on macOS (`setrlimit`), three on Linux (`setrlimit`, and `prctl` to set and to read back) | `sys/resource.h`, `sys/prctl.h` |
| 12 | `linux-dontdump` | Linux | `int madvise(void *, size_t, int)` with `MADV_DONTDUMP` only, on the crate's own secret buffers, allocated in safe Rust with 64 KiB alignment and size (sheet 12) | the C library | `unsafe fn` | one | `sys/mman.h` |
| 13 | `linux-peercred` | Linux | `int getsockopt(int, int, int, void *, socklen_t *)` with `SOL_SOCKET` and `SO_TYPE` (into an `int`, which must be `SOCK_STREAM`), then `SOL_SOCKET` and `SO_PEERCRED` (into a `struct ucred`), only, on a borrowed `UnixStream` | the C library | `unsafe fn` | two (one per option) | `sys/socket.h` |
| 14 | `linux-fdpass` | Linux, glibc (withheld on musl, sheet 14) | `ssize_t sendmsg(int, const struct msghdr *, int)` and `ssize_t recvmsg(int, struct msghdr *, int)` carrying exactly one `SCM_RIGHTS` descriptor and one data byte, with `MSG_NOSIGNAL` on send and `MSG_CMSG_CLOEXEC` on receive; each received descriptor becomes an `OwnedFd` at once (L7, point 1), a pidfd the kernel installed with `SCM_PIDFD` included, which is then closed (sheet 14) | the C library | `unsafe fn` | one per call (two), and one that adopts a received descriptor (`OwnedFd::from_raw_fd`) | `sys/socket.h`, `sys/uio.h` |
| 15 | `linux-mount` | Linux | `int mount(const char *, const char *, const char *, unsigned long, const void *)` for type `fuse` with `MS_NOSUID` and `MS_NODEV`, and the options `fd=`, `rootmode=`, `user_id=`, `group_id=` and `allow_other` (L7, point 1) | the C library | `unsafe fn` | one | `sys/mount.h` |
| 16 | `darwin-peercred` | macOS | `int getsockopt(int, int, int, void *, socklen_t *)` with `SOL_LOCAL` and `LOCAL_PEERCRED` only, on a borrowed `UnixStream`, into a `struct xucred` (C43, point 4; control endpoint) | libSystem | `unsafe fn` | one | `none` |
| 17 | `windows-pipe-create` | Windows | `CreateNamedPipeW` for the one control pipe, with a `SECURITY_ATTRIBUTES` naming a self-relative security descriptor built in safe Rust; the returned handle adopted as an `OwnedHandle` (C43, point 4) | see point 5.4 | `unsafe fn` | one for the call; one that adopts the handle (`OwnedHandle::from_raw_handle`) | `none` |
| 18 | `windows-pipe-io` | Windows | overlapped `ConnectNamedPipe`, `ReadFile`, `WriteFile` and `CancelIoEx` on a pipe instance the driver owns (sheet 18); `DisconnectNamedPipe`; and the completion calls of the fixed driver, whose family the driver's own design chooses (sheet 18; an open owner decision, the pipe completion method). No completion routine (`ReadFileEx`, an APC) is used | see point 5.4 | `unsafe fn` | one per call; the count is fixed with the completion family, before code (point 7) | `none` |
| 19 | `windows-pipe-caller` | Windows | `ImpersonateNamedPipeClient`, `OpenThreadToken` (with `TOKEN_QUERY` and `OpenAsSelf` true only), `RevertToSelf`, `GetTokenInformation` (classes `TokenUser`, `TokenStatistics`, `TokenImpersonationLevel`, `TokenElevationType`, `TokenLinkedToken` only), `CheckTokenMembership` (an explicit token handle and the built-in Administrators SID only), and `GetCurrentThread` (`safe fn`); optionally `GetNamedPipeClientProcessId`, for diagnostics only (C41, point 1) | see point 5.4 | `unsafe fn` (`GetCurrentThread`: `safe fn`) | one per call; one per adopted token handle (two); one per bounded read of a returned token structure or SID | `none` |
| 20 | `windows-pipe-server` | Windows (in `afs`, the session agents and any token sender) | `GetNamedPipeServerProcessId` (twice), `OpenProcess` with `PROCESS_QUERY_LIMITED_INFORMATION` only, `OpenProcessToken` with `TOKEN_QUERY` only, `GetTokenInformation` (`TokenUser` only) (C43, points 1, 2 and 4) | see point 5.4 | `unsafe fn` | one per call; one per adopted handle (two); one bounded SID read | `none` |
| 21 | `linux-udp6` | Linux, glibc and musl | `int socket(int, int, int)` with `AF_INET6`, `SOCK_DGRAM \| SOCK_CLOEXEC` and 0 only; `int setsockopt(int, int, int, const void *, socklen_t)` with `IPPROTO_IPV6`, `IPV6_V6ONLY` and the value 1 only; `int bind(int, const struct sockaddr *, socklen_t)` with a wrapper-built `struct sockaddr_in6`; the descriptor adopted as an `OwnedFd` and returned as a `std::net::UdpSocket` (C30, point 1; C34, point 2) | the C library | `unsafe fn` | one per call (three); one that adopts the descriptor (`OwnedFd::from_raw_fd`) | `errno.h`, `sys/socket.h`, `netinet/in.h` |
| 22 | `linux-statvfs` | Linux, glibc and musl | `int fstatvfs(int, struct statvfs *)` on a borrowed descriptor of the chunk store's directory (C27, point 3) | the C library | `unsafe fn` | one | `sys/statvfs.h` |
| 23 | `linux-rtnetlink` | Linux, glibc and musl | `socket` with `AF_NETLINK`, `SOCK_RAW \| SOCK_CLOEXEC` and `NETLINK_ROUTE` only; `bind` with a wrapper-built `struct sockaddr_nl` (the fixed group set on the notify socket, none on the request socket); `ssize_t sendto(int, const void *, size_t, int, const struct sockaddr *, socklen_t)` to the kernel's address only, with `MSG_DONTWAIT` only; `ssize_t recvmsg(int, struct msghdr *, int)` with `MSG_DONTWAIT` only, a `struct sockaddr_nl` for the sender and `MSG_TRUNC` checked; `int eventfd(unsigned int, int)` with 0 and `EFD_CLOEXEC \| EFD_NONBLOCK` only, for the wake descriptor; `int poll(struct pollfd *, nfds_t, int)` on exactly two entries, one route socket and its wake descriptor, `POLLIN` requested on each, with a timeout of 0 or more only; each descriptor adopted as an `OwnedFd` (C23, point 2; amended 2026-11-22) | the C library | `unsafe fn` | one per call (six); one that adopts a descriptor, a route socket's or the wake descriptor's (one private function) | `none` |
| 24 | `mem-wipe` | every target | none: no foreign call. `core::ptr::write_volatile` of a zero byte through each `&mut u8` of the caller's slice, then `core::sync::atomic::compiler_fence(SeqCst)` (O28, point 2) | none | none | one (the volatile write, in a loop of safe code) | `none` |

- **The boot clock** is `CLOCK_BOOTTIME`, `CLOCK_MONOTONIC_RAW` and
  `GetTickCount64`. **The awake clock** is `CLOCK_MONOTONIC`,
  `CLOCK_UPTIME_RAW` and `QueryUnbiasedInterruptTimePrecise`. These are
  C37, point 2's pairs. Wall time is `std::time::SystemTime` (no site).
- **The three `safe fn` declarations** are safe because no argument value
  can make the call undefined: `GetTickCount64` takes nothing and cannot
  fail; `clock_gettime_nsec_np` takes an integer by value and answers an
  invalid one with 0 and `EINVAL`; and `GetCurrentThread` (row 19) takes
  nothing and returns a pseudo-handle that needs no closing (to be
  confirmed at source with the row's code). The crate still passes only
  the two listed clock ids, from constants.
- **Rules common to every sheet.**
  - *Errors.* A Unix call that reports failure through `errno` is followed
    at once, on the same thread and with no call between, by
    `std::io::Error::last_os_error`; the crate declares no `errno` symbol
    of its own. Every status, length and count a call returns is checked
    before any value it wrote is used.
  - *Pointers.* Every pointer passed in points to memory the wrapper owns
    or borrows for the whole call, and no foreign code keeps one after the
    call returns, except as sheets 6, 7 and 18 state (the DNS query block,
    the macOS collector, and an overlapped pipe operation's block). The crate never forms a
    Rust reference to memory an operating system owns: it copies out of it
    through raw pointers, after checking for NULL and bounds, and reads
    unaligned where the documents promise no alignment.
  - *Variadic calls* (`syscall`, `prctl`): every argument is converted to
    the register-width type the kernel reads (`c_long`, `c_ulong`, a
    pointer or `usize`) before the call.
  - *Threads.* Unless its sheet says otherwise, a wrapper may be called
    from any thread at once: it shares no state.
  - *Tests* follow point 6.4, on the OS itself; the sheet names what is
    particular to the site. The ABI check of point 5.6 applies to all.

1. **`linux-clock`.**
   *Interface:* read the boot clock or the awake clock as 64-bit
   nanoseconds, or an error.
   *Obligations:* the out value is a zero-initialised local `timespec`,
   passed by exclusive pointer for the length of the call; the clock id is
   one of two constants. A non-zero status, a negative second count or a
   nanosecond field outside 0 to 999 999 999 is an error. Conversion uses
   checked arithmetic (rule 2; C38, point 1). What a failed read means is
   C37, point 5's.
   *Test:* both clocks advance and never go back across reads; the stand-in
   returns each invalid `timespec` and a failure.
2. **`darwin-clock`.**
   *Interface:* as sheet 1.
   *Obligations:* none on memory (`safe fn`: an integer by value; an
   invalid one is answered with 0 and `EINVAL`). The crate passes only the
   two listed ids, from constants. A zero result is an error.
   *Test:* as sheet 1.
3. **`windows-tick`.**
   *Interface:* the boot clock in milliseconds, converted with checked
   arithmetic.
   *Obligations:* none (`safe fn`: no parameter, cannot fail).
   *Test:* advances and never goes back.
4. **`windows-awake`.**
   *Interface:* the awake clock in 100-nanosecond units, converted with
   checked arithmetic.
   *Obligations:* the out value is a zero-initialised local `u64`, passed
   by exclusive pointer for the call. The function returns nothing, so a
   value of zero is treated as an error.
   *Test:* advances and never goes back; the DLL check of point 5.4.
5. **`darwin-oslog`.**
   *Interface:* write one entry: a level and a category from fixed Rust
   enums (O8, point 2: `default`, `error` and `fault` persist; `info` and
   `debug` for the debug audience), an event id, a public text and a
   private text.
   *Obligations:* the handles are created exactly once, behind a
   `std::sync::OnceLock`, before the first entry; after that the shim only
   reads them, so entries may be written from any thread at once. That
   Apple's log handles may be used from several threads at once is
   UNVERIFIED (Context cites only the macro expansion in `<os/log.h>`):
   it is inspected in Apple's documentation at source before code, and
   until Apple's documented support is INSPECTED the sink is not
   enabled (`docs/workflow.md` §3c). The concurrent-use test below
   checks the shim; a passing run does not stand in for that
   documentation. Both
   texts are bounded by `OSLOG_MAX_TEXT` (Decision 1, point 7), copied into wrapper-owned NUL-terminated
   strings that live through the call, and an interior NUL is replaced
   before the copy, never passed. The shim checks the level and category
   numbers against its own table again and ignores a value outside it, so
   an error in the Rust declaration cannot index past the handles. No
   pointer is kept by either side after the call.
   *Test:* an entry of each level is read back from the log store with the
   private text redacted; an entry whose texts are at `OSLOG_MAX_TEXT` is
   read back with its event id and required fields whole (the release
   gate of Decision 1, point 7); concurrent first use, and concurrent
   writes, from several threads, also under the thread sanitizer; the
   shim built with the address and undefined-behaviour sanitizers.
6. **`windows-dns`.** Point 3.2.
7. **`darwin-dns`.** Point 3.3.
8. **`linux-random`.**
   *Interface (all three random sites):* fill a caller's `&mut [u8]`
   completely, or return an error. The result must be used
   (`#[must_use]`); after an error the buffer's content is unspecified and
   is never treated as filled. An empty buffer makes no call.
   *Obligations:* the pointer and length come from the caller's exclusive
   slice, at most 256 bytes per call; the wrapper loops until the slice is
   full. `EINTR` is retried. Any other failure, `ENOSYS` and `EPERM` (a
   sandbox filter) included, or a return of zero or of more than was
   asked, is an error. Flags are 0, so the call blocks until the kernel's
   pool is initialised; the wrapper sets no time limit on that wait.
   *What the daemon does* (owner, 2026-11-20; the policy is
   `ironafsd`'s, Decision 1, point 3):
   - **Absent or filtered** (`ENOSYS`, `EPERM`): the process refuses to
     start and reports "random source unavailable or filtered" with the
     real errno. A seccomp filter can return `ENOSYS` too, so the errno
     alone does not tell a kernel without the call from a sandbox. There
     is no `/dev/urandom` fallback: it can return before the pool is
     initialised (random(4)). A default systemd filter can kill the
     process before it reports anything; the tests use errno-returning
     filters.
   - **Blocking early in boot:** flags stay 0. A separate worker, on the
     boot-clock seam, emits one wait event if the first fill has not
     returned. The systemd unit is `Type=notify` with a finite
     `TimeoutStartSec`, and `READY=1` is sent only after the fill and the
     remaining startup checks succeed (the daemon's composition). Network ordering gives no
     guarantee that the pool is ready (UNVERIFIED that it normally is by
     then on the target guests).
   *Test:* buffers of 0, 1, 256, 257 and 4096 bytes; the stand-in returns
   short counts, `EINTR`, `ENOSYS`, zero and an over-long count. At the
   daemon: `EPERM` and `ENOSYS` returned by errno-returning filters give
   the named refusal; an injected blocking provider gives the one wait
   event, and then the unit's start timeout.
9. **`darwin-random`.** `SecRandomCopyBytes`, decided by the owner
   (owner, 2026-11-20; Alternatives).
   *Obligations:* pointer and length from the caller's exclusive slice;
   `rnd` is NULL; any non-zero return is an error.
   *Test:* the sizes of sheet 8; the stand-in returns a failure.
10. **`windows-random`.**
    *Obligations:* pointer and length from the caller's exclusive slice,
    in pieces of at most `u32::MAX` bytes; the handle is NULL and the flag
    the one constant. Only `STATUS_SUCCESS` (0) counts as filled: every
    other `NTSTATUS`, the informational and warning ranges included, is an
    error.
    *Test:* the sizes of sheet 8; the stand-in returns an error status and
    a non-zero success status.

    **Entropy failure fails closed** on all three: there is no fallback to
    another source (a file, a time-seeded generator). `ironafs-crypto`
    turns the error into a refusal, at startup or in the operation that
    asked (C5, point 1).
11. **`unix-nodump`.**
    *Interface:* one function, "disable dumps for this process", that
    returns success only when every step succeeded.
    *Obligations:* the `rlimit` is a local `{0, 0}` passed by shared
    pointer for the call; a non-zero return is an error. On Linux,
    `prctl(PR_SET_DUMPABLE, 0)` must return 0 and
    `prctl(PR_GET_DUMPABLE)` must then return exactly 0 (O26, point 2);
    1, 2 or −1 is an error. The limit alone is not enough on Linux: it
    "will be ignored if the system is configured to pipe core dumps to a
    program" (core(5)). macOS has no second control and no read-back on
    this list: a zero return from `setrlimit` is the evidence, and the
    marker test of O26, point 5, is the check.
    *Use:* the first action of `main` in every process O25, point 2, names
    on that OS, before any credential is read, which also covers the reset
    on an exec of a set-id program or one with file capabilities. Any
    failure is fatal at startup, because O25 is Firm. The kernel also
    resets the attribute when the effective or filesystem user or group id
    changes `[DES-038]`
    ([PR_SET_DUMPABLE(2const)](https://man7.org/linux/man-pages/man2/PR_SET_DUMPABLE.2const.html)).
    No site on this list changes an id, and safe Rust cannot; an amendment
    that adds one must call this function again after it and check the
    read-back.
    *Test:* the read-back; a child that sets the controls and is then
    killed with a dumping signal leaves no core file, with and without a
    piped dump handler (Linux). The piped case has a positive control: a
    test collector that records every image it is handed first captures
    a planted marker from a child run without the controls, and then
    receives no image at all from a child run with them. An absent core
    file alone is not the pass condition.
12. **`linux-dontdump`.**
    *Interface:* a secret-buffer type that owns its memory, gives safe
    `&mut [u8]` access, and zeroes itself on drop with `wipe` (sheet 24),
    called directly (the same crate, no injection), never a plain
    overwrite.
    *Obligations:* the memory is a boxed slice of a type that is 64 KiB
    in size and 64 KiB in alignment, allocated in safe Rust, so its start
    and length are multiples of every page size the allowlisted Linux
    targets use (4, 16 and 64 KiB) without asking the system for the page
    size and without an unsafe allocation. `madvise` gets that whole
    allocation and `MADV_DONTDUMP` only, which changes no content, so the
    call cannot affect memory safety. A failure is returned as an error
    and the buffer is not handed out. The advice is not undone when the
    buffer is freed: at worst, later non-secret data in the same pages is
    also left out of a dump. It is an additional layer, never the only one
    (O25, points 2 and 3; O26, points 2 and 5): the startup dump controls
    and the marker tests stay. It costs 64 KiB per buffer, which the owner
    accepted (2026-11-20); callers budget concurrent secret buffers
    by that full allocation size, not by the bytes they hold. A failed
    exclusion fails closed: the buffer is not handed out.
    *Test:* the call succeeds on each supported kernel; the marker test of
    O26, point 5, with the first layer switched off in a test build.
13. **`linux-peercred`.**
    *Interface:* given a borrowed `UnixStream`, the peer's `pid`, `uid`
    and `gid` as IronAFS's own types, or an error. Safe Rust can wrap any
    descriptor as a `UnixStream`, so the wrapper checks what it was given
    before it reads the credentials:
    - the peer's address, through the standard library (`getpeername`),
      which a listener, an unconnected socket and a socket of another
      family do not have;
    - the socket's type, read with `SO_TYPE` into a zero-initialised
      local `int`, with a local length set to `sizeof(int)`, both passed
      by exclusive pointer for the call. Each of three outcomes is its
      own error: a non-zero return (the call failed, with its errno), a
      returned length other than `sizeof(int)` (a length error), and a
      type other than `SOCK_STREAM` ("not a Unix stream", with the type
      read). The last refuses a datagram socket, a pair's included: a
      datagram socket of a pair can be connected to another peer without
      the kernel replacing the pair's credentials, which would otherwise
      be reported as the new peer's;
    - a `uid` or `gid` of −1 in the credentials read, which the kernel
      reports for a socket with no peer credentials, is an error (the
      `pid` is not checked).
    *Obligations:* the `ucred` is a zero-initialised local and the length
    a local set to its size, both passed by exclusive pointer for the
    call; the descriptor is borrowed from the stream for the call. A
    non-zero return, or a returned length other than the size of
    `struct ucred`, is an error. The type read's safety argument is the
    same, for one `int`: the kernel writes at most `*optlen` bytes and
    the length. The values are those "in effect at the
    time of the call to connect(2), listen(2), or socketpair(2)"
    (unix(7)), not the peer's current ones. Policy
    (which uid may send a token, C43, point 4) is the caller's.
    *Test:* a socket pair in one process; a peer under another uid; a
    listener, and a reconnected datagram socket of a pair, each wrapped
    as a stream and refused; the stand-in returns a short length and an
    over-long length for each read, and a type other than
    `SOCK_STREAM`, each an error.
    *Revisit if* a site adds another descriptor type that carries peer
    credentials.
14. **`linux-fdpass`.**
    *Interface:* send one borrowed descriptor over a borrowed
    `UnixStream`; receive exactly one as an `OwnedFd`. `ironafs-fuse`
    receives the `OwnedFd` and needs no `unsafe` (C4, point 3, gives it no
    ADR).
    *Obligations:*
    - The `msghdr` is zero-initialised (musl's padding fields included),
      with one `iovec` over one wrapper-owned data byte: a stream socket
      carries ancillary data only with data. The control buffer is a
      wrapper-owned, zero-initialised `#[repr(C)]` value made of a
      `cmsghdr` followed by two `int`s, so it is aligned for a `cmsghdr`
      and is read back as ordinary Rust fields, with no pointer walk.
    - `CMSG_SPACE` for one `int` is 24 bytes on the 64-bit targets of
      point 5.3, which is also `CMSG_LEN` for two. A sender can therefore
      deliver two descriptors without `MSG_CTRUNC`.
    - **Authenticate the header, then adopt, then decide.** The receiver
      first checks the returned `msg_controllen` (at least `CMSG_LEN` of
      one `int`, at most the buffer), then the header's `cmsg_len`
      (within `msg_controllen`, and exactly one or two `int`s' worth
      after the header), its level `SOL_SOCKET` and its type
      `SCM_RIGHTS`. Only a header that passes all of these holds
      descriptors; an integer from any other control message is never
      adopted or closed. It then adopts **every** non-negative
      descriptor the valid `SCM_RIGHTS` message delivered, as an
      `OwnedFd`, before it rejects anything for `MSG_CTRUNC`, for a
      second descriptor or for the data byte, so a delivered descriptor
      is never leaked.
    - **A pidfd is the second descriptor-carrying type.** A receiving
      socket with `SO_PASSPIDFD` set (on it, or inherited from the
      listener it was accepted from; no site sets it, but a socket unit's
      `PassPIDFD=` or foreign code in the process can) gets an
      `SCM_PIDFD` control message, and the kernel can install that pidfd
      in the receiver; which control message the kernel writes first, and
      so which one a receive returns, is left open. The buffer holds at
      most one descriptor-bearing header, and only the first header is
      read; for what does not fit the kernel sets `MSG_CTRUNC`. A first
      header of level `SOL_SOCKET`, type `SCM_PIDFD` and length exactly
      `CMSG_LEN` of one `int`, within the returned control length, holds
      one descriptor. Its value is checked (non-negative) before it is
      adopted as an `OwnedFd`, like a delivered descriptor, whatever else
      the receive reported. Then the message is refused, which closes it:
      at the end of the stream as the end of the stream, with
      `MSG_CTRUNC` as a truncated message, and otherwise (a wrong data
      byte included) as "the peer's pidfd arrived instead of a
      descriptor". *Safety, for the
      adoption:* the kernel installed the pidfd in this process for this
      receive, after writing its control message, and no other code has
      seen it, so the `OwnedFd` is its only owner, as for `SCM_RIGHTS`.
    - `sendmsg` uses `MSG_NOSIGNAL`, so a closed peer is an error and not
      a signal. `recvmsg` uses `MSG_CMSG_CLOEXEC`. `EINTR` is retried on
      both. A return of 0 (end of stream), a short send, `MSG_CTRUNC`, no
      control message, a header that fails the checks above, more than
      one descriptor, or a negative descriptor value is an error. On every
      error path the adopted descriptors are dropped, which closes them; a
      negative value is never adopted (`OwnedFd`'s precondition).
    - A descriptor exists as a raw integer only between the checked read
      of the control buffer and its adoption: the receive, and the checks
      it hands what it read to by value, with no fallible step or early
      return between them. The checks are a safe function over owned
      values, so that the tests and the fuzz target `ironafs-os_fdpass`
      (point 6.5) reach every branch without a socket.
    - The wrapper says nothing about what the descriptor refers to, nor
      who receives it. The receiving caller checks the sender (sheet 13)
      and the object. The sending caller checks the recipient **before it
      sends**: the root mount step reads the peer's credentials (sheet 13)
      on the same connected stream it will send on, and sends only if the
      peer's uid is the daemon's service account; any other peer, or a
      failed check, gets no descriptor and the step closes the stream and
      its copy of the descriptor. Which account that is, and the step's
      other inputs (the mount point, how the stream is reached), are the
      mount step's policy (L7, point 1; threat-model U7, owner row C13,
      open).
    - Both directions of the FUSE descriptor hand-off are this site (owner,
      2026-11-20): the root mount step sends through it and
      `ironafsd` receives (L7, point 1). Inheritance across `exec` is not
      used, so the root step does not set up the daemon's sandbox (L7,
      point 2).
    *Test:* one descriptor; none; two (both closed, counted in
    `/proc/self/fd`); a closed peer; end of stream; a wrong control type
    whose payload holds the number of an open descriptor of the receiver
    (that descriptor stays open); `MSG_CTRUNC` with one descriptor
    delivered (closed, not leaked); a short `msg_controllen` and a
    `cmsg_len` past it. Each case counts `/proc/self/fd` before and
    after. The sender's check: a peer under a uid other than the daemon's
    service account connects on the stream the mount step uses; the
    peer's receive returns no descriptor (end of stream, nothing in its
    `/proc/self/fd` from the step), and the step's own descriptor count
    shows both its copies closed. The checks over an `SCM_PIDFD` header
    adopt and close its descriptor; and, where the kernel offers
    `SO_PASSPIDFD` (Linux 6.5 and later), a live receive on a socket on
    which a helper process has set the option (no site sets it) closes
    the pidfd the kernel installed, counted in `/proc/self/fd`.
    *Constants, per target:* `SCM_PIDFD` (4) is named by glibc 2.39 and
    later in `<bits/socket.h>`, only under `_GNU_SOURCE`, which the row's
    check defines; older glibc headers and musl's (1.2.5 included) do not
    name it, and no installed user-space header does. It is proved by name
    on each target of point 5.3 (a target triple), against the headers of
    that target's ABI check (point 5.6), never by a value taken from
    another target or library, and never through a C expression.
    An implemented GNU target requires complete ABI evidence under
    point 5.6 from its lane whose headers declare the name. The
    glibc 2.28 compatibility lane compiles the row using the Rust
    constant and runs the native descriptor-passing test. When those
    headers omit the name, the lane proves the other ABI assertions
    and reports partial evidence. MISSING applies to the named
    assertion in that lane. The GNU triple's row remains implemented
    and its complete ABI evidence comes from the lane whose headers
    name the constant. The older and newer glibc lanes share the
    target triple and the value is the kernel's, independent of a
    glibc symbol. The partial run alone gives no release clearance.
    Points 5.5 to 5.7 still apply. If the compatibility lane's
    headers name the constant, its partial check fails and the
    exception is revisited.
    On the musl targets the row stays approved-only and the crate's
    glibc gate excludes it. `check-os` holds the gate to those statuses.
    On each musl target, `check-abi` uses its own compiler and headers
    to establish whether `SCM_PIDFD` is absent. It reports the row
    withheld for a proved absence of that name. If the name becomes
    available, the check fails and the withholding decision is
    revisited. `SO_TYPE` (3) and `SOCK_STREAM` (1), of row 13, are
    declared by both libraries.
    *Revisit if* musl's headers or the glibc compatibility lane's
    headers name `SCM_PIDFD`, or a site sets `SO_PASSPIDFD`.
15. **`linux-mount`.**
    *Interface:* mount the FUSE filesystem at a path, given the FUSE
    device as a borrowed descriptor, the root mode and the numeric ids.
    *Obligations:* all four strings are wrapper-owned, NUL-terminated and
    live through the call. The source and type are constants. The options
    string is built by the wrapper from typed values only (the borrowed
    descriptor's number, the root mode in octal, the ids in decimal, and
    `allow_other`), never from caller text. The target path is rejected if
    it holds a NUL. The flags are the two constants. The descriptor is
    borrowed for the call, so it is open while the kernel looks it up. A
    non-zero return is an error. The call needs `CAP_SYS_ADMIN`, so only
    the privileged mount step of L7, point 1, can succeed with it; which
    path is mounted, and that nothing can swap it, is that step's policy.
    *Test:* in a disposable namespace or virtual machine: a mount that
    serves one request; each refusal; a failure without privilege.

    Sheets 16 to 20 serve the control endpoint (C43, C45, point 1). They
    were approved by the owner for this ADR before any IPC code (owner,
    2026-11-20); their obligations follow the control IPC specification
    specification (planned), as changed by the owner's decisions of
    2026-11-20 on the pipe server and its I/O. The policy around them (who is an administrator,
    the connection state table, caps, deadlines and admission budgets) is
    the control IPC design's, a class S document.
16. **`darwin-peercred`.**
    *Interface:* given a borrowed `UnixStream`, the peer's effective uid
    and group list as IronAFS's own types, or an error; the same shape as
    sheet 13, so the daemon's caller check and the sender's daemon check
    (C43, point 4) share code.
    *Obligations:* the `xucred` is a zero-initialised local and the
    length a local set to its size, both passed by exclusive pointer for
    the call; the descriptor is borrowed from the stream for the call. A
    non-zero return, a returned length other than the structure's size, a
    `cr_version` other than `XUCRED_VERSION`, or a `cr_ngroups` below 0 or
    above `NGROUPS` is an error; only that many groups are copied. The
    values are those at `connect` or `listen` (unix(4)). `LOCAL_PEERPID`
    is not on the list: a pid is never used for a decision, and adding it
    for diagnostics is a change to this row.
    *Test:* a socket pair; a peer under another uid; the stand-in returns
    a short length, a wrong version and an over-large group count.
17. **`windows-pipe-create`.**
    *Interface:* create the first or a later instance of the one control
    pipe, given "first or later" and the pinned service SID; returns an
    `OwnedHandle`.
    *Obligations:*
    - The name, open mode, pipe mode, instance count, buffer sizes and
      time-out are constants in the wrapper, the same on every instance,
      as `CreateNamedPipeW` requires: `PIPE_ACCESS_DUPLEX` and
      `FILE_FLAG_OVERLAPPED`, `FILE_FLAG_FIRST_PIPE_INSTANCE` exactly when
      the caller says "first"; `PIPE_TYPE_BYTE`, `PIPE_READMODE_BYTE` and
      `PIPE_REJECT_REMOTE_CLIENTS`.
    - **The instance count** (owner, 2026-11-20) is one constant from 1 to 254. The
      daemon's pipe pool (listening instances, unauthenticated handshakes
      and attested connections together) is sized within it. 1,024
      cannot be passed, and 255 (`PIPE_UNLIMITED_INSTANCES`) is never
      used (owner, 2026-11-20): it would leave the bound to the system, not to the
      daemon. The value comes from the control IPC design's caps (its
      5.2).
    - The service SID is the one packaging pins (owner, 2026-11-20); the crate does not
      look up its own token for it, so the specification's proposed
      `windows-self-sid` row is not added.
    - The name is a wrapper-owned, NUL-terminated UTF-16 string. The
      security descriptor is built in safe Rust as self-relative bytes
      from typed values (the entries and fixed rights masks of the
      control IPC design; never `FILE_GENERIC_WRITE`), and lives,
      with the `SECURITY_ATTRIBUTES` that points to it, through the call;
      `bInheritHandle` is false and `nLength` is the structure's size.
    - The result is compared with `INVALID_HANDLE_VALUE` before adoption;
      adoption is its own block, once.
    *Test:* the descriptor read back from the live pipe equals the
    intended one, entry for entry; a second "first" fails; a pipe of the
    same name created first by an unprivileged user makes "first" fail;
    an unprivileged process can neither create an instance nor open the
    pipe for more than the granted rights.
18. **`windows-pipe-io`.**
    *Interface:* the fixed pipe driver's primitives: start a connect wait,
    a read or a write on an instance; cancel one; collect completions;
    disconnect an instance. The driver **owns** each instance (sheet 17's
    `OwnedHandle`) from its creation until every operation started on it
    has reached its terminal completion; nothing outside the driver holds
    the handle, and no instance is disconnected, given a new connect wait
    or closed while an operation on it is outstanding. A dedicated
    **prefix-and-caller transition** (below) reads exactly the 4-byte
    `Hello` length prefix and captures the caller, so the one read made
    before the caller is known cannot be larger, by construction.
    *Obligations (owner, 2026-11-20):*
    - **Connection generation.** Each instance carries a generation
      number, raised by one each time a connect wait completes. Every
      operation, completion and result is tagged with the generation it
      was started under. A completion or result whose generation is not
      the instance's current one is discarded (its buffer wiped, below),
      never delivered.
    - **The prefix-and-caller transition** (control IPC 5.4, step 1;
      the length prefix first). One driver-owned step on one instance and one generation.
      The driver starts the 4-byte read as the only operation on the
      instance. A byte-mode read can complete with fewer bytes than
      asked: a **short completion** (1 to 3 bytes) stays inside the same
      transition. The driver checks the count against the bytes still
      missing, keeps them in the same 4-byte buffer and, from the same
      driver thread, starts one read for exactly the missing bytes,
      again as the only operation on the instance, same generation,
      under the same handshake deadline, until the buffer holds 4
      bytes. A completion of 0 bytes, an end of stream, a broken pipe or
      any other failure ends the transition with a failure. When the
      handshake deadline passes with a read outstanding, the driver
      requests its cancellation and ends the transition with a failure
      only after that read's terminal completion is observed (the buffer
      is not released or zeroed before it). No partial prefix reaches a
      later step. When the buffer holds all 4 bytes, the same driver
      thread at once runs sheet 19's sequence on
      that instance, before it starts or completes any other operation
      on it and before any disconnect or new connect wait on it. Sheet
      19's result is tagged with the generation and delivered together
      with the 4 buffered bytes, as one result. It is **bound only
      while** its generation is still the instance's current one **and**
      the connection holding that generation is still in the control
      IPC's pre-authentication state; a result of the current generation
      that arrives after that connection was closed (its handshake
      deadline passed, or the daemon is stopping) is discarded, its
      buffer zeroed, and never bound to any connection. No other read on the
      instance precedes the capture, so the "last message read" that
      Windows attests is this connection's prefix. If the client
      disconnects between the read and the capture, sheet 19's calls
      fail or the next read fails, and the connection is closed. No
      later client can be bound to the identity: a new client needs a
      disconnect and a new connect wait, which only the driver makes,
      after the transition has returned, and which raise the generation.
    - **Overlapped I/O on a fixed driver.** The driver's threads are a
      fixed set, counted against C39, point 1's set (an amendment of C39
      first, if they are not already in it); they deliver each completion
      into a C39, point 3, one-shot slot, as the Windows DNS query does
      (point 3.2). Which threads, and which completion calls, stay open:
      the control IPC specification leaves both to the driver's own
      design, which is not written yet (an open owner decision, the pipe completion
      method). This row gains them by a change (point 7) before code.
    - **Lifetimes.** Each operation's `OVERLAPPED` and buffer live in one
      heap block the driver owns from the start of the operation until
      its terminal completion is observed, a cancelled operation
      included: a `CancelIoEx` request does not end the lifetime, only
      the completion that follows it. This is the common pointer rule's
      named exception: Windows keeps both pointers after the start call
      returns. No stack frame owns them, and no Rust reference to a
      buffer exists while Windows may write it. The block is touched only
      by the driver thread, so no `unsafe impl` is needed (point 6.3); if
      the design needs one, this row changes first.
    - **Secrets in buffers** (O28, points 2 and 3). Every read buffer on
      the control pipe, and every write buffer that holds a token or
      other IPC secret, is zeroed with `wipe` (sheet 24, called directly,
      never a plain overwrite) before it is reused or freed, on every
      path: completed, cancelled, failed, discarded for a stale
      generation, discarded after its connection closed, and released
      without being delivered to `ironafs-ipc`. The control IPC
      specification lists these as named zeroisation places.
    - Each start call is followed by its immediate-completion, pending,
      disconnected and failed paths, each named in the design;
      `ERROR_PIPE_CONNECTED` counts as connected; a completed read or
      write's byte count is checked against the buffer before any byte is
      used.
    - **Bounds.** Outstanding operations are bounded in count and bytes,
      and ingress and cancellation stay serviceable while workers wait
      (C39, point 2). A block whose completion never arrives stays
      counted, as in point 3.2.
    - **The completion family is not chosen yet.** The driver's own
      design picks one (a completion port, or an event per operation) and
      records the driver's thread assignment and completion calls (the
      an open owner decision, the pipe completion method); this row then gains those
      items and its block count by a change (point 7) before code.
    *Test:* connect before and after the wait; cancel a pending connect,
    read and write, each with its block still alive until its completion;
    a client that disconnects mid-frame; a burst that reaches every bound;
    close and reconnect with I/O pending (the instance is not reused
    before every terminal completion, and each buffer is zeroed); a
    disconnect, and a reconnect by another client, between the prefix
    read and the capture (a stand-in pause between the two steps, and on
    Windows a client that disconnects as soon as it has written 4 bytes):
    the first identity is never bound to the second client, which is
    attested afresh under the next generation; short prefix completions
    (a client that writes 1 byte, pauses, then 3; one that writes 2 and
    2; one that writes 2 and closes; one that writes 1 and stalls past
    the handshake deadline): the prefix is gathered within the one
    transition, or the transition fails after the outstanding read's
    terminal completion, and no partial prefix is delivered; a result
    after close (a transition whose capture is held by a stand-in pause
    until the connection's handshake deadline has closed it): the
    same-generation result is discarded, its buffer zeroed, and bound to
    nothing.
19. **`windows-pipe-caller`.**
    *Interface:* one function, called only from sheet 18's
    prefix-and-caller transition, on the instance and generation that
    transition holds, after exactly the 4-byte `Hello` length prefix has
    been read on it (the control IPC's prefix rule): the caller's user SID, logon session id,
    impersonation level, elevation type, the linked token's logon session
    id, and whether the connecting token is effectively a member of the
    built-in Administrators group, as owned Rust values, or an error. No
    token handle leaves the crate.
    *Obligations:*
    - **Order.** The prefix is read, under the handshake deadline
      and inside the admission budget of the control IPC design, without
      decoding it or replying. Then, on one OS thread the wrapper
      controls, with no other work between: `ImpersonateNamedPipeClient`;
      `OpenThreadToken` on `GetCurrentThread()` with `TOKEN_QUERY` and
      `OpenAsSelf` true, the handle adopted as an `OwnedHandle` at once;
      `RevertToSelf`. Only then are the token's classes read, as the
      daemon. Per-caller limits are applied, and the rest of `Hello` is
      read and decoded, only after this function has returned.
    - A zero return from `ImpersonateNamedPipeClient` is an error and
      nothing else is called; the connection is closed. After a non-zero
      return, `RevertToSelf` is called on every path, each error path
      included; if it returns zero the process aborts, because a thread
      left under a caller's token must run nothing else.
    - Each `GetTokenInformation` call uses a wrapper-owned buffer aligned
      for the class's structure: one call for the size, one with a buffer
      of that size, the returned length checked. The size is checked
      against `MAX_TOKEN_INFO_LEN` = 1,024 bytes (confirmed by the owner,
      2026-11-20; the classes
      read are far smaller) before the buffer is allocated; a larger size
      is an error. A SID
      is copied out through raw pointers after its revision, its
      sub-authority count (at most 15) and its total length are checked
      against the buffer, and a SID pointer inside a returned structure
      must point inside that same buffer; no Rust reference to memory
      Windows wrote is formed.
    - A level below identification is an error (`CallerUnknown`). The
      linked token's handle, when the elevation type says there is one, is
      adopted as an `OwnedHandle` at once, and only its statistics are
      read (C41, point 1).
    - **Administrators** (owner, 2026-11-20): `CheckTokenMembership` with the captured
      token's handle (never NULL, which would test the daemon's own
      thread) and the built-in Administrators SID, built as constant
      bytes in safe Rust. A failed call is "not a member": the check
      fails closed. There is no separate grant for LocalSystem.
    *Test:* callers at each impersonation level, anonymous refused; a
    standard user, an elevated and a limited administrator token of one
    logon (the linked-token case, one context, different membership); a
    forced failure of each step in the stand-in, each showing the thread
    reverted; impersonation attempted before the prefix read, recording
    whether each supported release answers 1368 (UNVERIFIED until run).
20. **`windows-pipe-server`.**
    *Interface:* given the client's borrowed pipe handle and the pinned
    service SID, "the server is the service" or an error. It runs in the
    sender before any credential is read or sent (C43, point 1).
    *Obligations:*
    - The pid out value is a zero-initialised local. The process handle
      is opened with `PROCESS_QUERY_LIMITED_INFORMATION` only and adopted
      as an `OwnedHandle`, and is **held open until the pid has been read
      a second time from the same pipe handle and found equal**, so the
      pid cannot be reused between the two reads. The interval before
      `OpenProcess` is not closed by this: a server that exits and whose
      pid is reused by another process before the open is the residual
      the owner accepted on 2026-11-20, and the Windows gate must show how it is closed (for
      example that a pipe whose server has exited fails the second read)
      before the row ships.
    - The token is opened with `TOKEN_QUERY` only, adopted at once, and
      read under sheet 19's buffer rules. SIDs are compared as bytes in
      safe Rust.
    - Any failure, access denied on the service's process included, is
      "not the service", and the sender sends nothing. The check is made
      on every new connection and bound to the handshake's instance id
      (C43, point 2), so it is repeated after a reconnect.
    *Test:* the real daemon, checked by an ordinary user (the owner asked the
    gate to show that an unprivileged caller can make the check); a pipe
    of the same name held by another user's process; the server exiting
    and being replaced between connect and check; the stand-in changes
    the pid between the reads. The residual interval before
    `OpenProcess` has its own case, because a changed pid does not
    exercise it. Its setup: the pipe's server exits after the first read
    and another process is observed, independently of the check (by the
    test harness), to hold the same pid before `OpenProcess`. It passes
    only if the check answers "not the service" before any credential is
    read or sent; the run records whether the second read failed or
    returned the reused pid, and which step refused. The run must also
    show why the refusal closes the gap: it counts only when the refusal
    comes from a step that tells the connected pipe's server apart from
    the replacement (for example the failed second read), never from the
    service-SID comparison alone, which a replacement with another SID
    fails by accident. A run in which that reuse is never observed, or
    in which only the SID comparison refused, leaves the gate open and
    the row does not ship.
21. **`linux-udp6`.**
    *Interface:* given an IPv6 address (the wildcard or a specific
    unscoped one) and a port, an IPv6-only, close-on-exec UDP socket bound to them, as
    a `std::net::UdpSocket`, or an error. All I/O after that is safe
    `std`.
    *Obligations:* the option is set to 1 **before** `bind`, and a
    failure of `setsockopt` is an error: the descriptor is dropped
    (closed) and never bound without the option. The `sockaddr_in6` is a
    zero-initialised wrapper-owned local with the family, port (network
    byte order) and address set from typed values; its length is its
    size. The scope id stays zero, so the interface takes unscoped
    addresses only: a link-local address (`fe80::/10`), which needs an
    interface zone, is refused before `socket` is called. Binding a
    scoped address needs an amendment that carries the scope id in the
    typed address (point 7). The descriptor is adopted as an `OwnedFd` straight after
    `socket` returns a non-negative value, so every later error path
    closes it. `EAFNOSUPPORT` from `socket` (IPv6 absent at boot, or a
    sandbox's address-family filter) and `EADDRINUSE` from `bind` are
    returned as distinct errors; what they mean is the caller's (C34; single-family operation only when the administrator selects it; in
    dual-family mode either failure refuses to start).
    *Test:* in a network namespace, an IPv4 socket and this socket bound
    to the same port with `net.ipv6.bindv6only` at 0 and at 1; the port
    held by another socket; IPv6 disabled at boot; a sandbox that denies
    `AF_INET6`; a link-local address, refused with no socket created. macOS joins by amendment in M4.
22. **`linux-statvfs`.**
    *Interface:* given a borrowed descriptor of the chunk store's own
    directory, the bytes available to an unprivileged user, or an error.
    It is the chunk store's filesystem that is measured: a descriptor of
    the state directory can be on a different one.
    *Obligations:* the `struct statvfs` is a zero-initialised local passed
    by exclusive pointer; a non-zero return is an error. The result is
    `f_bavail × f_frsize`, multiplied with overflow checked; an overflow
    is an error. What a failed query means is the caller's: clean-cache
    admission is suspended until a query succeeds (C27, point 3). The
    site is used from M2b, with the first real-host chunk store; M2a's
    floor runs in the simulator behind the `Disk` seam (C3, point 3;
    C20).
    *Test:* a tmpfs of known size, empty and full (a value under the
    floor); a descriptor on another filesystem measures that one.
23. **`linux-rtnetlink`.** Amended 2026-11-22: the bounded wait, the
    wake descriptor and non-blocking sends and receives. **Working.**
    Revisit this row if: point 5.6's check finds a declaration of the
    wait or the wake wrong on a target (`nfds_t` among them); a run
    shows a send to the kernel waiting inside the kernel despite
    `MSG_DONTWAIT`, a wake lost or a wait not ending at its timeout; or
    `std` gains a safe way to wait on a socket and a wake together.
    The owner accepts this amendment before its code is written (point
    7.1). The network-change monitor's redesign that uses it is a
    separate class S change: its design is reviewed by a non-author
    family before code, then its code (point 6.6).
    *Interface:* open a notify socket subscribed to a fixed group set
    (links, the addresses and routes of both families, and the policy
    rules of both families), or a request socket with no groups; send
    one request to the kernel, without waiting for buffer space;
    receive one datagram into a caller's buffer, without waiting for
    one to arrive, returning its length,
    "nothing queued", "interrupted", or a typed error that tells
    overflow (`ENOBUFS`), truncation, an empty datagram and a sender
    other than the kernel apart; make a wake (one `eventfd`), signal it
    from any thread, and clear it; and wait on one route socket and one
    wake together for a given `Duration`, rounded up to whole
    milliseconds and subject to the kernel's rounding and scheduling
    delay (Context, poll(2)), returning, for the route socket, which of
    readable, error and hangup were reported, whether the wake is
    signalled, "timed out" or "interrupted". That wait is the site's
    explicit readiness wait, and always has a finite timeout;
    `MSG_DONTWAIT` keeps the send from waiting for buffer space and the
    receive from waiting for a datagram. Whether a send can still wait
    inside the kernel is UNVERIFIED (below).
    The bytes are not decoded here: the netlink messages go to a pure,
    fuzzed decoder outside this crate (Decision 1, point 3; rules 1 and
    9).
    *Obligations:*
    - The `sockaddr_nl` values are zero-initialised wrapper-owned locals;
      the group set is a constant (the owner named its members, 2026-11-20; a review also named
      the relevant nexthops, settled with the network-change monitor's
      design before the row is final). The members, as the interface
      above names them: links, the addresses, routes and policy rules
      of IPv4 and of IPv6. The nexthop group (`RTNLGRP_NEXTHOP`) is
      proposed by the network-change monitor's specification (PR #143,
      not yet landed; its 5.1, point 3), with an open question there on
      binding that bit on a kernel before Linux 5.3. **Open item for
      the monitor's change:** the group set becomes final only when
      that specification is accepted and the set, nexthops included or
      not, is restated here by amendment (point 7.1). Until then row 23
      stays approved-only on every target; it is marked implemented on
      a target only as a whole row, once that amendment is accepted
      and the full row is implemented there (point 6.7); a
      measurement on a kernel before 5.3 settles the open question.
      The descriptor is adopted as an `OwnedFd` as in
      sheet 21.
    - `sendto` always names the kernel (port id 0, no groups): that a
      plain write reaches the kernel is UNVERIFIED across the candidate
      kernels. It passes `MSG_DONTWAIT`, so it never waits for buffer
      space; `EAGAIN` is a distinct "would block" error, never retried
      here. Whether a send to the kernel can still wait inside the
      kernel (for a lock the kernel's own handler takes) is not stated
      by netlink(7): UNVERIFIED, and the reason the caller never sends
      under its state lock (below). A short send is an error. `EINTR` is
      returned as "interrupted", not retried here, so the caller decides
      again (below).
    - `recvmsg` passes `MSG_DONTWAIT` and uses one `iovec` over the
      caller's slice and a `sockaddr_nl` for the sender. `EAGAIN` is
      "nothing queued", not an error: a ready report from the wait does
      not promise a datagram, and the receive never blocks to find one.
      `MSG_TRUNC` in the returned flags is
      the truncation error. The returned `msg_namelen` must equal the size
      of `struct sockaddr_nl` and its family must be `AF_NETLINK`, both
      checked before the port id is read; then a sender whose port id is
      not 0 is an error. On any of these the datagram is not returned.
      `ENOBUFS` is the overflow error. A return of 0 bytes is the
      "empty datagram" error, never a datagram. `EINTR` is returned as
      "interrupted", not retried here.
    - **The wake** is one `eventfd` made with an initial value of 0 and
      `EFD_CLOEXEC | EFD_NONBLOCK`, adopted as an `OwnedFd` as soon as the
      call returns a non-negative value, then held as a `std::fs::File`.
      Signalling and clearing are safe `std` I/O on that file, as sheet
      21's socket I/O is, so they need no foreign item: a signal writes
      the 8-byte value 1 in native byte order, and `EAGAIN` (the counter
      at its maximum) means already signalled and is success; a clear
      reads 8 bytes, and `EAGAIN` means not signalled. `EINTR`, which a
      non-blocking transfer is not expected to return, is retried once;
      a second `EINTR`, any other error, and a transfer of other than 8
      bytes, are errors. The wake is
      `Send` and `Sync`: any thread may signal it. A signal made before
      the wait starts is kept by the counter, so it is never lost.
    - **The wait** calls `poll` on a wrapper-owned array of two
      zero-initialised `struct pollfd`, the route socket's and the
      wake's descriptor, each borrowed for the call, with `POLLIN`
      requested on both. The `Duration` becomes milliseconds rounded up,
      so a remainder below one millisecond never becomes 0, and is
      clamped at `i32::MAX`; a negative (infinite) timeout is never
      passed. A return of 0 is "timed out". `EINTR` is returned as
      "interrupted" and **not retried here**: the time left is the
      caller's to recompute from its deadline, through the clock seam
      (below). Any other failure is an error. On a positive return, the
      two `revents` values and the returned count are copied out and
      checked by a safe validator over those owned values (fuzzed, point
      6.5): on the route socket, `POLLIN`, `POLLERR` and `POLLHUP` are
      each reported as they came, never merged; on the wake, `POLLIN`
      marks it signalled. `POLLNVAL` on either, any other bit on the
      wake (an overflowed counter's `POLLERR` included), a bit outside
      those named, or a count that does not equal the number of entries
      with a non-zero `revents`, is an error. Both may be ready at once,
      and both are reported. The wait clears nothing: the caller clears
      the wake.
    - Policy is the caller's (owner, 2026-11-20; the network-change monitor): subscribe before the first
      snapshot and reconcile notifications received during it; on
      overflow, truncation, an interrupted dump or a monitor failure,
      invalidate the consumers at once and take a fresh verified snapshot
      before reusing state; ignore only lifetime refreshes that leave an
      address's usability unchanged. A failed signal or clear of a
      wake (below) is not recoverable: consumers are invalidated at once
      and the source stops, with no fresh snapshot; the other failures
      keep the invalidate-and-resnapshot rule. The daemon's unit allows
      `AF_NETLINK` (the Linux units).
    - **What the caller must do with the wait** (owner, 2026-11-22; the
      network-change monitor; amended 2026-11-22). These are
      requirements on the caller's design, reviewed before its code and
      with it under point 6.6. They rest on two documents: the seams
      and scheduling specification (`docs/specs/seams-and-scheduling.md`,
      5.3) for samples and deadlines, and the network-change monitor's
      specification (PR #143, not yet landed) for the attempts and the
      failure count. Where that specification does not yet say what a
      point below needs, the point is an open item that its change
      settles before code.
      - **One wake per blocked worker, cleared only by it.** Each thread
        that waits on a route socket has its own wake; no other thread
        waits on it or clears it. A stop, and anything else that must
        make a worker look again (a new or earlier deadline), is decided
        under the caller's state lock and then signals that worker's
        wake. The wake replaces any netlink message sent only to end a
        receive. Each turn of a worker is, in this order: clear its
        wake; take the state lock and read what it must act on (a stop,
        its outstanding attempts and their deadlines); release the lock;
        wait, with the lock released. So a signal made after the read is
        still in the counter, and ends the wait. After a wait in which
        the wake and the route socket are both ready, the worker handles
        the stop and any deadline change before it receives from the
        socket. A failed signal or clear is fatal, as the policy above
        says.
      - **A finite timeout from an awake-clock deadline.** Each wait's
        timeout is the time left to the worker's earliest outstanding
        deadline on the awake clock, taken from the shared `Clock`'s
        sample (ADR 0004, C37; coding standards rule 7; the seams
        specification, 5.3), and never more than a named horizon
        constant, `ROUTE_WAIT_HORIZON` (1 second, accepted by the
        owner with this amendment, 2026-11-22). With nothing due, the
        worker waits to the horizon and looks again, so an idle worker
        wakes once per horizon.
      - **The time left is recomputed after every `EINTR`.** After an
        "interrupted" return of the wait, a send or a receive, and after
        any other early return, the worker checks its stop, whether the
        attempt it was serving is still live, and the time left from a
        fresh sample, before it calls again; an earlier timeout is never
        passed again, and a send is retried, if at all, with the lock
        released.
      - **Each attempt is registered before its send.** A dump request
        or a barrier is an attempt with its own id, its sequence number
        and its own deadline. The attempt is registered under the state
        lock (sequence and deadline set, the deadline starting from the
        fresh sample taken at registration) before its send, which is
        then made with the lock released and is non-blocking. The
        deadline's duration for a dump and for a barrier are named
        constants of the monitor's specification: **open item**,
        settled there before code, with the evidence of a dump's and a
        barrier's round trip on the supported kernels.
      - **One terminal transition per attempt.** A send failure ("would
        block" included), an expiry and a terminal answer each end an
        attempt only through one transition taken under the state lock,
        which acts once: the first to take it decides; the others find
        the attempt gone and do nothing. A send failure is charged to the
        attempt whose id it carries, never to a newer one. A terminal
        answer is the record that ends the attempt in the monitor's
        specification (a dump's terminator, or the barrier's
        acknowledgement); an intermediate dump record does not end it.
      - **An answer is accepted only while its attempt is live and in
        time.** Before any record of an answer is applied, an
        intermediate dump record included, under the state lock, the
        caller checks that the attempt is live, that the answer came on
        the socket the attempt was sent on (the same socket instance,
        not a replacement), that its sequence is the attempt's, and,
        from a fresh sample, that the attempt's deadline has not passed
        and that its latched awake-clock doubt has not moved (the seams
        specification, 5.3, point 2). An intermediate record that passes
        is applied and the attempt stays open. A record, intermediate or
        terminal, at or past the deadline, or after a moved doubt count,
        is not applied: the attempt's terminal transition retires it and
        counts it as expired. So a
        late acknowledgement cannot publish, even if it takes the lock
        before the expiry path does. A retired attempt's sequence is
        never issued again on the socket that may still carry a reply
        to it.
      - **Each expired or failed attempt is counted once** as a failed
        snapshot under the monitor's failure count: in its
        specification (PR #143, 5.5, point 4), 8 failed snapshots in a
        row, with no snapshot completed between them, stop the source,
        and a completed snapshot resets the run. A dirty snapshot is not
        a failure there. If that specification changes the count, the
        once-only rule here still holds.
      - **Clock failure** (C37, point 5). The monitor uses the shared
        `Clock`, so C37's own rules (startup refused without a valid
        pair, callbacks marked suspect, one event per episode) stay in
        force and are not the monitor's to repeat. A failed sample, and
        a failure seen by any other user of the clock and cleared before
        the worker samples again (shown by the awake doubt count having
        moved past an attempt's latched count), expire every outstanding
        attempt, each counted once, as above; no new deadline is set
        until a valid sample returns. Meanwhile each wait takes
        `ROUTE_WAIT_HORIZON` as its timeout, the bounded fallback
        horizon, and after every return the worker checks its stop and
        whether the clock has returned.
      - **Error and hangup on a route socket.** A wait that reports
        `POLLERR` or `POLLHUP` on the route socket is followed by a
        receive. `POLLERR` is explained only by an error return of that
        receive: `ENOBUFS` takes the overflow path above, and any other
        error is a failed receive. `POLLHUP` is explained by no receive
        outcome. A flag that the receive does not explain (a datagram,
        "nothing queued" or the empty-datagram error after `POLLERR`,
        and any outcome after `POLLHUP`) is a failed receive on that
        socket: a datagram received with it is not applied, consumers
        are invalidated at once, and the socket is made again, before
        any snapshot can publish, as the monitor's specification already
        rules for "any other failed receive on the notify socket"
        (PR #143, section 7). Whether a route socket ever reports an
        error or hangup that a receive does not explain is UNVERIFIED:
        **open item**, measured on the supported kernels by the
        monitor's change.
      - **Deterministic tests.** The caller's design tests, through a
        seam over this site and a test clock: a lost wake; a signal made
        between the state read and the wait; the wake and the route
        socket ready at once; repeated interruption of the wait, a send
        and a receive; a dump that is never answered; a late
        acknowledgement after its expiry, both when the expiry path
        takes the lock first and when the acknowledgement does; a send
        failure racing an expiry; a clock that fails and returns between
        two of the monitor's samples; an intermediate dump record before
        and after its deadline; and an error or hangup followed by an
        empty receive and by a valid datagram.
    *Test:* in a network namespace, an address, a route and a rule added
    and removed, each seen on the notify socket; a dump answered on the
    request socket; an overflow provoked with a small receive buffer,
    where the wait reports the socket ready and the receive returns the
    overflow error; a
    truncated read; a datagram from a non-kernel sender, where the kernel
    lets one be sent; the stand-in returns a short sender address and a
    wrong family, and an error or hangup with nothing queued. For the
    wait: a wait with nothing ready returns "timed
    out"; a wake signalled from another thread ends a blocked wait; a
    wake signalled before the wait starts ends it at once; two signals
    are cleared by one clear; a receive with nothing queued returns
    "nothing queued" without blocking; a timeout below one millisecond is
    not passed as 0. "Interrupted" and the `POLLNVAL` and wake-`POLLERR`
    errors are tested through the stand-in (point 6.5), which returns
    them on demand, and the `revents` validator has its fuzz target
    (point 6.5: invalid bits, a count that disagrees with the entries,
    both entries ready); on the OS, delivering a signal to the waiting thread
    needs a handler that the crate does not install (Decision 1, point
    2), so that path is UNVERIFIED on the OS unless a test can do it
    without a new site.
24. **`mem-wipe`.** Owner, 2026-11-20 (the cryptography decision).
    *Interface:* `wipe(&mut [u8])`, safe, returning nothing. It writes
    zero to every byte with `core::ptr::write_volatile`, one byte per
    write through a pointer made from that byte's `&mut u8`, then calls
    `compiler_fence(SeqCst)`.
    *Contract:* the site's promise rests on `write_volatile` alone: the
    compiler does not remove these stores, nor merge or reorder them
    with other volatile operations, so every addressed byte is written
    with zero before `wipe` returns, even when the buffer is dead
    afterwards. The fence is kept only as an ordering aid; it restricts
    the compiler's reordering of memory operations around atomic
    orderings, and does not guarantee that every later access stays
    after the wipe, so no part of the contract rests on it.
    *Obligations (the safety argument):* each pointer comes from an
    exclusive reference, so it is non-null, aligned (a `u8` needs no
    alignment), valid for one write and unaliased for the write; the
    slice's own iterator bounds the loop, so no pointer arithmetic is
    written; a `u8` has no invalid value and no drop. No foreign code is
    involved, so there is no stand-in: Miri checks the site itself.
    *Limits* (rule 8, written at the site): it wipes the addressed buffer
    only. Copies made by moves, spills to the stack and values in
    registers are not covered; the callers keep key material in one
    place and pass it by reference. A buffer the operating system owns
    until an I/O completes (sheet 18) is wiped only after completion.
    *Callers:* outside this crate, only through injection. `ironafs-crypto`,
    `ironafs-cache` and `ironafs-ipc` are pure and cannot depend on this crate (ADR 0004 C4,
    points 1 and 4), so each declares a small safe wipe type (a
    `fn(&mut [u8])` wrapper), taken at composition beside the `Rng`; the
    production composition passes this function, and a test shows it
    cannot take a test wiper. Inside this crate the owners of secret
    buffers call `wipe` directly: sheet 12's secret-buffer type in its
    drop, and sheet 18's driver on each terminal-completion, discard and
    release path of its read buffers and secret-bearing write buffers
    (sheet 18, "Secrets in buffers"), a buffer never delivered to
    `ironafs-ipc` included. Users: `ironafs-crypto`'s key types
    (HMAC-SHA-256's key, pads and states, and later keys); the host
    identity's installation-key persistence path in `ironafs-cache`, also
    pure and served by the same injection (the random fill buffer, the
    record encoder's and decoder's buffers, and the `Disk` read and
    write buffers on that path, each a named wipe place of the
    state-directory format; a buffer the operating system owns until an
    I/O completes is wiped after completion, as *Limits* says); and the
    control IPC's token buffers. A new kind of user needs a change to
    this sheet (point 7). The class S review of each owner's code
    (point 6.6) checks every drop and terminal-completion path named
    here against O28, points 2 and 3's named places, and that each calls
    this function.
    *Test:* Miri runs the site's real block over every length from 0
    to 257, each buffer first filled with a non-zero pattern, and
    asserts that every byte of the whole slice is zero afterwards.
    *Release gate (in place of point 5.6):* this site has no foreign
    item, so point 5.6's C check has nothing to check; its gate is a
    release-assembly check, run on each shipping target. It builds the
    production `wipe` itself (the crate's function, not a copy in a
    probe), called from a small harness crate under the release
    profile the shipped binaries use (optimisation level, LTO, codegen
    units and panic strategy taken from the workspace's release
    profile, not restated), and checks the emitted assembly for the
    zero stores to the buffer in two cases: a live buffer read after
    the wipe, and a dead buffer wiped just before it goes out of scope
    and is freed. In the dead-buffer case the optimiser must see both
    the production body and the buffer's whole lifetime: the buffer is
    allocated, wiped and freed in one harness function, and the check
    first confirms from the emitted assembly that, under the shipping
    profile, `wipe`'s body is inlined there (no call to `wipe` remains
    in that function). A failing control, the same harness with a
    planted plain overwrite in place of the volatile write, is shown
    failing on each target: in the dead-buffer case its stores are
    absent from the emitted code. If `wipe` is not inlined there, or the
    control keeps its stores, the dead-buffer case shows nothing on that
    target, and it counts as not run. As with point 5.6, a target on which the check has
    not run is MISSING evidence (`docs/workflow.md` §3c), and the site
    does not ship there.
- **Two callbacks, both for DNS.** No other site passes a function
  pointer to foreign code, and no other foreign code calls into Rust. The
  macOS reply callback (point 3.3) is called only from within
  `DNSServiceProcessResult`, on the thread that called it. The Windows
  completion routine (point 3.2) is called by Windows on a thread IronAFS
  does not own. Neither can unwind into foreign code: both are declared
  with a non-unwinding ABI, so a panic would abort the process (Rust 1.81,
  Context), and points 3.2 and 3.3 give both no panic path in the first
  place. Memory they need is reserved with `try_reserve` inside the limits
  of point 3.1, and a failed reservation is recorded as an error.

### 3. DNS on macOS and Windows

**Working.** Revisit if: C33 is revisited; the Windows check shows a NULL
server list is not the system's own (point 3.2); Windows runs show that
`DnsCancelQuery` misbehaves (no completion after a cancel, a completion
after the context is gone, or retained memory that grows past the bound
of point 3.2), in which case the fallback of point 3.2 applies; the
macOS stalled-reply case of point 3.3 can starve work the daemon must
do; or an operating system ships a resolver interface that needs no
site.

1. **Common to both.**
   - One public function: query one name for one type (SRV or AFSDB) by a
     deadline. The name is validated before any call: ASCII letters,
     digits, hyphens and dots, plus an underscore only as the first
     character of a label (the SRV service and protocol labels,
     `_afs3-vlserver._udp.<cell>`,
     [RFC 5864 §5](https://www.rfc-editor.org/rfc/rfc5864.html#section-5)),
     at most 253 octets, at most 63 per label, no empty label, no
     interior NUL. It returns the answer records as owned values, a
     conclusive negative answer, or a typed error; it never returns a
     partial answer as complete. A conclusive negative answer is only
     the resolver's own report that the name or the type is absent
     (point 3.2's two statuses; on macOS, the negative reply of point
     3.3). **Applicable and irrelevant records are told apart** (fix
     round 4). A record is *applicable* when it is of the type asked for
     and in the answer (on Windows, in the answer section); any other
     record (another type, such as a CNAME of an alias chain, or another
     section or class) is *irrelevant*: skipped, counted, and never an
     error by itself. An applicable record that is rejected, by this
     crate's checks or later by the record decoder outside it, makes the
     **whole** answer the typed error "unusable answer": no record of it
     is used, so a record set with one member dropped is never used as
     complete. An answer with no applicable record is "unusable answer"
     too. "Unusable answer" is never absence.
   - **No discovery from an unqualified site.** An answer from either
     site is used for discovery on an operating-system release only
     after that site's release gate (points 3.2 and 3.3, run under point
     6.4) has passed there; until then the site is not called on that
     release, and cells are configured explicitly (C33, point 1, with
     point 8's switch off). This is the interim behaviour for every
     claim this point marks UNVERIFIED or MISSING.
   - Byte limits (rule 3), named constants on both systems:
     `MAX_DNS_RECORDS` = 64 records and `MAX_DNS_ANSWER_BYTES` = 65,536
     bytes of copied data per query (values confirmed by the owner,
     2026-11-20), every copy reserved against them
     before it is allocated; anything beyond is an error, not a
     truncation. Each record is copied with its TTL (C33, point 6).
   - The query is started and waited for on the thread that calls it.
     Each call owns every foreign handle and buffer it creates, and none
     outlives the call except as point 3.2 states for a Windows query
     still pending. On macOS the handle owners are neither `Send` nor
     `Sync`, and every foreign call of one query is made on one thread. On
     Windows the shared query block of point 3.2 is `Send` and `Sync` by
     two `unsafe impl`s, because the completion routine and a shutdown
     cancel run on other threads; point 3.2 gives the argument.
   - Late answers are the caller's to discard (C33, point 6).
2. **Windows (`windows-dns`).**
   - `DnsQueryEx` is called with a zero-initialised `DNS_QUERY_REQUEST`:
     version 1; `QueryName` a wrapper-owned, NUL-terminated UTF-16 copy of
     the validated name; `QueryType` 33 (SRV) or 18 (AFSDB), from
     constants; `QueryOptions` 0 (`DNS_QUERY_STANDARD`); `pDnsServerList`
     NULL and `InterfaceIndex` 0, which is expected to leave server and
     interface, split-DNS routing included, to the system (C33, point 2;
     `[DES-005]`). Context marks that MISSING: it is the expected result
     of the executed check of point 5.6, which records it on each
     supported release; the crate's one completion
     routine; as context a pointer to the query block below; and a cancel
     handle in that block. The `DNS_QUERY_RESULT` is zero-initialised with
     version 1, as Microsoft requires. (Decided by the owner, 2026-11-20:
     the asynchronous call with `DnsCancelQuery`, so no IronAFS thread is
     held by a stalled resolver and C39, point 1's fixed set is kept.)
   - **The query block.** The request, the name, the result, the
     `DNS_QUERY_CANCEL` structure and a one-shot reply slot live in one
     heap block shared by reference count (`std::sync::Arc`). The block is
     built completely, in safe code, before it is shared. The three
     structures Windows reads or writes (request, result, cancel) are each
     inside an `UnsafeCell`, because Windows writes them while Rust holds
     shared references to the block; the name is a separate heap buffer the
     block owns, so the request's pointer to it stays valid when the block
     is shared. The context given to Windows carries one reference
     (`Arc::into_raw`), taken before the call, and exactly one place takes
     it back (`Arc::from_raw`, one private function): the completion
     routine on the pending path, the caller on the synchronous path. None
     of the block's foreign structures moves or is freed while Windows can
     still reach it: Microsoft states that the context and cancel handle
     are valid until the completion routine is invoked, and that the
     result and cancel structure are not freed before then `[DES-004]`;
     and that the cancel handle "is valid until the
     DNS_QUERY_COMPLETION_ROUTINE DNS callback is invoked and
     DnsCancelQuery completes", because `DnsCancelQuery` "does not wait for
     a query to complete before cancelling"
     ([DnsCancelQuery](https://learn.microsoft.com/en-us/windows/win32/api/windns/nf-windns-dnscancelquery),
     CONFIRMED 2026-11-20). The caller therefore drops its own reference
     only after `DnsCancelQuery` has returned.
   - **Who touches the foreign structures, and when.** Rust code touches
     them at three times only, which never overlap: while building the
     block; in the completion routine, after Windows has finished with
     them; and on the synchronous path, after `DnsQueryEx` has returned a
     status other than pending. Everything else in the block is a mutex,
     a condition variable and plain owned data. That is the argument the
     two `unsafe impl`s state.
   - **A cancel is sent at most once, never before the handle exists,
     and never after completion is recorded.** The slot's lock guards a
     lifecycle state, *submitting*, *pending* or *completed*, and two
     flags that only ever go from false to true: "cancel requested" and
     "cancel sent". The block is built in *submitting*. When `DnsQueryEx`
     returns `DNS_REQUEST_PENDING`, the caller takes the lock: if the
     state is still *submitting* it becomes *pending*, and the cancel
     handle is usable from then on; if "cancel requested" is set and
     "cancel sent" is not, the caller then cancels at once, as below. A
     cancel, from the caller's deadline or from shutdown, takes the lock
     and: in *submitting* sets "cancel requested" only (the handle is not
     usable before `DnsQueryEx` has returned); in *pending*, if "cancel
     sent" is already set, does nothing more (fix round 4: the query
     stays *pending* after a cancel is sent, so a deadline and a shutdown
     can both arrive there; the second sends nothing), and otherwise sets
     "cancel requested" and "cancel sent" and calls `DnsCancelQuery`
     before releasing the lock; in *completed* does nothing. Every path
     that may call `DnsCancelQuery` (the caller after `DnsQueryEx`
     returns, the deadline, shutdown) tests "cancel sent" and sets it
     under the same lock hold as the call, so the call is made at most
     once per query. The block is alive
     throughout, so the handle's memory is never freed under the call.
   - **The outcome is decided once, at entry.** Completion is recorded,
     under the lock, by the routine's entry on the pending path and by
     the caller on the synchronous path: the lifecycle becomes
     *completed*, and the outcome is fixed from the flags as they are at
     that moment. If "cancel requested" is set, the outcome is
     **cancelled**, whatever `QueryStatus` and the records say: the copy
     is skipped and the waiter, if any, gets "cancelled", never the
     answer. Otherwise the outcome follows the status and the records.
     Entry never clears either flag, and a cancel that arrives after
     entry changes nothing (the query has completed; the outcome stands).
     So a successful completion after shutdown asked for cancellation is
     reported as cancelled, never as an answer.
   - **Entry, copy, free, publication, reclamation.** The routine's
     steps run in this one order (on the synchronous path the caller runs
     steps 1 to 4 and keeps its own reference): (1) entry, as above;
     (2) copy: unless the outcome is cancelled, the record list is walked
     and copied into owned values under the rules below; (3) `DnsFree` of
     the record list, exactly once and whatever the outcome, when
     `pQueryRecords` is not NULL; (4) publication: the outcome (the copied
     answer, a negative answer, an error, or "cancelled") placed in the
     slot, which wakes the caller; (5) reclamation: the routine's
     reference dropped, last. No pointer into the record list exists
     after step 3. The admission charges of the bounds below belong to the
     block and are released when its last reference drops, so a query
     counts until both the caller and the routine are done with it.
   - **The cancel lock is a candidate, with a release gate per Windows
     floor** (owner, 2026-11-20). Only the query-local lock is held
     across `DnsCancelQuery`; it is a leaf in the lock order (nothing else
     is taken while it is held, and it is outside C39, point 4's server,
     volume, object, chunk order), and the block is kept through the
     return of `DnsCancelQuery` and the completion routine. Microsoft
     documents the cancel handle's lifetime; two things it does not
     document only a run can show: that `DnsCancelQuery` never runs the
     completion routine on the calling thread (it would block on the
     lock), and that Windows tolerates a cancel issued in the instant
     between its invoking the routine and the routine taking the lock,
     which no caller can close. The tests below are that gate, run on
     every supported Windows floor; a release where it fails does not get
     this design, and the fallback below applies.
   - **Two completion paths, kept separate.**
     - *Pending:* `DnsQueryEx` returns `DNS_REQUEST_PENDING`. The result
       arrives only through the completion routine. The caller waits on
       the slot with its deadline (C39, point 3's pattern). At the
       deadline it cancels as above, returns a timeout, and drops its own
       reference; the block lives until the routine has run.
     - *Synchronous completion:* any other return. Some queries complete
       at once even in asynchronous mode (address literals, the local
       name, errors) `[DES-004]`, and no completion routine follows. The
       wrapper records completion as above (the outcome fixed from the
       flags at that moment), takes back the context's reference itself,
       runs steps 2 to 4 on the result in place, and never waits on the
       slot. That return is final: its status is read as
       below, and nothing waits for a later completion.
     - *What a status means*, on both paths. `ERROR_SUCCESS` is an
       answer only when the list holds at least one applicable record
       (point 3.1: the type asked for, in the answer section) and **no
       applicable record is malformed** (fix round 4). An applicable
       record is malformed when its `wDataLength` is below the size of
       its type's data structure or a name in it fails the checks below;
       one malformed applicable record makes the whole answer "unusable
       answer", wherever it stands in the list, and none of the list's
       records is returned. Irrelevant records (another type, or outside
       the answer section) are skipped and counted, and do not change the
       outcome. `DNS_ERROR_RCODE_NAME_ERROR` (the name does not exist)
       and `DNS_INFO_NO_RECORDS` (no record of that type) are a
       conclusive negative answer. **Verified absence is kept apart from
       rejected data:** `ERROR_SUCCESS` with no applicable record (only
       irrelevant ones), with a malformed applicable record, or with no
       list at all, is the typed error "unusable answer", a resolver
       failure, never a negative answer; it is counted. Every other status, a
       timeout and a cancel included, is a resolver failure: a typed
       error, never a negative answer. The
       constants' values are checked by point 5.6. Whether Windows
       reports either negative case through another status is MISSING
       until the gate records it; an unlisted status stays a failure.
     The synchronous path rests on Microsoft's statement that no
     completion routine follows a synchronous return `[DES-004]`. The
     tests below check it on each release; a completion after a
     synchronous return fails the gate for that release, and the site does
     not ship there until the design is amended.
   - **The completion routine** is a Rust `extern "system"` function with
     the documented signature `VOID (PVOID, PDNS_QUERY_RESULT)`. If the
     context is NULL it returns at once. Otherwise it takes back the
     context's reference and checks that the result pointer is the
     address of the block's own result; if it is not, it records
     completion with an error outcome, reads nothing through that
     pointer, publishes and drops the reference. Otherwise it runs the
     five steps above in their order: entry (completion recorded and the
     outcome fixed), `QueryStatus` read and the record list copied
     unless the outcome is cancelled, `DnsFree` exactly once, the slot
     filled, the reference dropped last. It
     does no other work and blocks only on the slot's own lock, which is
     held briefly except across one `DnsCancelQuery` call, and whose
     poisoning it handles without a panic. Its body has no indexing,
     no `unwrap` or `expect`, and only checked arithmetic, as on macOS.
   - **Bounds, cancelled queries included** (rule 3).
     `MAX_DNS_OUTSTANDING` = 16 outstanding queries and
     `MAX_DNS_RETAINED_BYTES` = 1,114,112 bytes (16 × 68 KiB) of memory
     they retain (blocks, names and copied answers), both confirmed by
     the owner (2026-11-20). A
     query is charged its block, its name and `MAX_DNS_ANSWER_BYTES`
     before the block is allocated, so its copies never exceed the
     charge. It counts against both until the block's last reference
     drops (above), whether or not it was cancelled. A new query past either bound gets a
     retryable "discovery busy" error, counted; configuration still wins
     (C33, point 1). A block whose completion never arrives stays counted,
     so a cancel that misbehaves shows as a full bound and an event, never
     as unbounded growth.
   - **Shutdown.** The daemon cancels every outstanding query and waits,
     up to a fixed limit, for their completion routines. A block a routine
     can still reach is never freed; any still outstanding at exit is left
     to process teardown and counted in one event.
   - **Tests** (point 6.4, the release gate, on each supported Windows
     floor): a cancel just before, during and after completion, and
     immediately before and during the routine's entry, with no double
     free and no lost reference, and a record of which thread ran the
     routine; same-thread re-entry (the routine run on the cancelling
     thread), run in a child process under a deadline, so a deadlock is
     recorded as a gate failure and never hangs the run; a cancel from
     shutdown while the query is still *submitting*, and a completion
     routine that runs before `DnsQueryEx` has returned to the caller;
     a **successful** completion (and, separately, a synchronous success)
     after shutdown has requested cancellation, in *submitting* and in
     *pending*: the caller gets "cancelled", never the answer, the list
     is freed once and no record is copied; a cancel that arrives after
     entry: the answer stands; the steps observed in the order of the
     routine bullet (publication after `DnsFree`, reference last);
     each status of the table above, and an unlisted one;
     `ERROR_SUCCESS` with only irrelevant records (another type, a CNAME
     among them; outside the answer section) and with no list: "unusable
     answer", never a negative answer; a **mixed list** (fix round 4):
     valid applicable records with irrelevant ones give the valid records
     as the answer; valid applicable records with one malformed
     applicable record (short data; separately, a bad name), placed
     first, in the middle and last, give "unusable answer" for the whole
     answer, with no record returned and the list freed once; a deadline
     cancel followed by a shutdown cancel, and the reverse, both before
     completion: `DnsCancelQuery` is called exactly once (counted by the
     wrapper's test instrumentation); shutdown
     with queries pending; a stalled resolver (a
     test server that never answers, and an unreachable one), with the
     caller's deadline met; the synchronous-completion cases (an address
     literal, an immediate error) returning with no completion; and a
     burst of cancelled queries that stays inside the bounds. Each run also
     records whether and when the completion arrives after a cancel, and
     its `QueryStatus`.
   - **Fallback, only if the gate cannot establish safe behaviour on a
     floor:** the synchronous call on one fixed discovery thread with a
     bounded queue (a caller past its deadline gets the retryable
     "discovery busy" error). That thread is not in C39, point 1's set, so
     the fallback needs a C39 amendment first, and an amendment of this
     point (point 7) (owner, 2026-11-20).
   - When `pQueryRecords` is not NULL, whatever the status and the
     outcome, the list is freed with `DnsFree(…, DnsFreeRecordList)`
     exactly once (step 3), by the completion routine on the pending path
     and by the caller on the synchronous path; it is read first (step 2)
     only when the outcome is not cancelled.
   - Reading the list: at most 64 entries along `pNext`, each read by
     copying the fixed header out through a raw pointer (no Rust reference
     into the list is formed). **The precondition of each raw read,
     which the wrapper cannot check:** a non-NULL `pQueryRecords` (the
     list head) and a non-NULL `pNext` each address a complete
     `DNS_RECORDW` header, readable in full; an applicable record's data
     holds the whole `DNS_SRV_DATAW` or `DNS_MX_DATAW` structure the
     reader copies, readable in full (`wDataLength` is a length Windows
     reports, not an extent the wrapper can check); a non-NULL
     `pNameTarget` or `pNameExchange` addresses a wide string that is
     NUL-terminated within readable storage; all of these stay readable
     until `DnsFree`. The wrapper relies on Microsoft's reference for these
     structures (`DNS_RECORDW`: `pNext` is a pointer to the next record
     structure; `DNS_SRV_DATAW`, `DNS_MX_DATAW`: the names are pointers
     to strings; `DnsFree` frees the list "and includes subfields of the
     DNS_RECORD structure"; Context), read as stating that the list
     holds whole structures and terminated strings. That reading is
     UNVERIFIED until those pages are re-read at source before code, for
     each of the four (list head, next record, record data, names), and
     the raw reader is not written until it covers all four: if
     they do not support it, this route is reopened as an owner
     decision (the alternative is an interface that returns the answer
     as bytes with a length) before the row ships. The 256-unit scan
     below bounds work only; it supplies no allocation extent, so no
     read past a terminator is ever relied on to stop. An
     entry is applicable if its `wType` is the type asked for and it is
     in the answer section; any other entry is irrelevant and skipped
     (its data is not read). An applicable entry is malformed, and the
     whole answer is "unusable answer" (the status bullet above), if its
     `wDataLength` is below the size of the data structure for that type
     (point 5.6 checks the sizes) or its name (`pNameTarget`,
     `pNameExchange`) is NULL, is not NUL-terminated within 256 UTF-16
     units, or is invalid UTF-16; the copy stops at the NUL. Every field is copied into
     an owned value before the list is freed. No pointer into the list
     survives the free.
3. **macOS (`darwin-dns`).**
   - `DNSServiceQueryRecord` is called with `kDNSServiceFlagsReturnIntermediates`
     only, so a conclusive negative answer is seen (C33, point 2;
     `[DES-006]`); interface 0 (any); class IN; type 33 or 18; the
     validated name as a wrapper-owned C string that lives until
     `DNSServiceRefDeallocate` returns; the crate's macOS reply callback; and as
     context a pointer to a wrapper-owned collector that also lives until
     then. On an error return the reference is not initialised and is
     never used or deallocated, as `dns_sd.h` states. A guard value owns
     the initialised reference and deallocates it in its `Drop`, so every
     exit, an early error included, deallocates exactly once; the name and
     the collector are declared before the guard and so are dropped after
     it.
   - **The collector** has checked interior mutability (`RefCell`). The
     context is a pointer made from a shared reference to it, and the
     callback turns it back into a shared reference only; the wrapper
     itself uses only shared references while the query lives, and holds
     no borrow of the content across `DNSServiceProcessResult`. So no
     exclusive reference is ever aliased.
   - The wrapper takes the descriptor from `DNSServiceRefSockFD` (−1 is an
     error), waits for it to be readable with `poll` (one `pollfd`,
     `POLLIN`, a timeout of the time left to the deadline on the awake
     clock, at most `i32::MAX` ms; `EINTR` retried with the time
     recomputed), and calls `DNSServiceProcessResult` only when `poll`
     reports `POLLIN`. Any other returned event (`POLLHUP`, `POLLERR`,
     `POLLNVAL`) or a failure ends the query with an error, as does a
     non-zero result from `DNSServiceProcessResult`. The descriptor
     belongs to the reference: the wrapper never closes it and never uses
     it after deallocation.
   - **The deadline is a time to readability** (owner, 2026-11-20).
     `poll` bounds the wait until the daemon's reply can be read;
     `DNSServiceProcessResult` then reads one whole message from the
     local daemon, and that read has no deadline of its own. A system DNS
     daemon that starts a reply and stalls holds the calling thread past
     the caller's deadline. The design accepts that residual, states the
     deadline in these words in the interface's documentation, and tests
     the stalled-reply case separately from the no-reply case. If the
     stalled reply can starve work the daemon must do, the design is
     reopened (the trigger above).
   - **Adds, removals and completion (Working; qualified by the tests
     below).** A reply with `kDNSServiceFlagsAdd` adds its record, with
     its TTL (C33, point 6); a reply without it removes the record with
     the same rdata from the collector, and a removal of a record not
     held is ignored. A reply without `kDNSServiceFlagsMoreComing` ends
     the replies queued now; `dns_sd.h` does not say that the answer is
     complete. So the collector holds an answer as complete only when a
     batch has ended, it holds at least one record, and a settle
     interval (`DNS_SETTLE`, 50 ms on the awake clock, confirmed by the owner,
     2026-11-20, within
     the deadline) passes with no further readability; any reply in that
     interval reopens it. At the deadline, records held without a
     completed settle are discarded and the query is a timeout: a
     partial answer is never returned as complete (point 3.1), so SRV
     ranking (C33, point 1) never works from part of a record set that
     IronAFS knows to be still arriving. The settle interval is a
     heuristic, not a completeness signal: a record that reaches the
     collector more than `DNS_SETTLE` after a batch ends would be missed,
     and nothing in `dns_sd.h` rules that out. **So the settle rule never
     completes a positive answer**, of either type: SRV priority comes
     before every other ranking input (C33, point 1), so an SRV set
     missing its higher-priority record would be ranked as whole, and an
     AFSDB set missing a record would be a known subset returned as
     complete (point 3.1). The settle completes no answer at all: an
     SRV set holding a single record with the target "." is held to the
     deadline like any other, because a second record could still
     arrive and C33, point 1, gives "." its meaning only as the sole
     target. The collector does not inspect targets; decoding stays with
     the shared codec (point 1.3). Every query ends only with a
     conclusive negative answer, an error or the deadline; at the deadline the records held are discarded and the
     query is a timeout, as above. AFSDB is asked for only after the SRV
     query has ended with a conclusive negative answer (point 3.1), never
     after a timeout, an error or "unusable answer". Until a
     completeness signal `dns_sd.h` does not give today is qualified and
     this point amended (point 7), this site returns no SRV or AFSDB
     record set, a sole "." included, and macOS discovery of servers uses explicit
     configuration (C33, point 1). The tests below show that records are
     withheld; they give no evidence of completeness, so returning
     records needs both a qualified completeness signal and that
     amendment. Until the tests pass on a release, this site's answers
     are not used for discovery there (point 3.1, "No discovery from an
     unqualified site").
   - It stops when the collector holds a complete answer as above, a
     conclusive negative answer, an error, or the deadline. It then calls
     `DNSServiceRefDeallocate` exactly once, on the same thread, after
     the last `DNSServiceProcessResult` and with the descriptor no longer
     polled.
   - **The callback** is a Rust `extern "C"` function with the documented
     signature. With the Windows completion routine (point 3.2), it is one
     of the two functions in the workspace that foreign code calls. If the
     context is NULL it returns at once. It then reads
     `errorCode` first: if it is not `kDNSServiceErr_NoError`, it records
     that code in the collector and reads no other parameter, because
     `dns_sd.h` says "Other parameters are undefined if errorCode is
     nonzero". Otherwise it checks that `rdata` is not NULL when `rdlen`
     is not zero; it borrows the collector's content with
     `try_borrow_mut` and drops the reply if that fails (it cannot, by the
     rule above, and it cannot panic); it ignores a reply whose type or
     class differs from the query (irrelevant, point 3.1; counted); a
     reply of the asked type whose `rdata` is NULL while `rdlen` is not
     zero marks the collector malformed, so the whole answer is
     "unusable answer" (point 3.1); otherwise it copies `rdlen` bytes of
     `rdata` into the collector within the limits of point 3.1. It keeps no pointer it was given. Its body has no
     indexing, no `unwrap` or `expect`, and only checked arithmetic, so it
     has no panic path. It reads the `flags` and `ttl` parameters only
     after `errorCode` is `kDNSServiceErr_NoError`.
   - The copied rdata is decoded outside this crate by the same safe,
     fuzzed record codec the Unix client uses (C4, point 6), so one decoder
     reads SRV and AFSDB data on every Unix-like system.
   - **Tests** (point 6.4): an existing SRV and AFSDB name; a name that
     does not exist, recording how the conclusive negative answer arrives
     (as an error code or as a reply without the add flag: `dns_sd.h` does
     not say, MISSING until run); a deadline with no answer; a reply that
     starts and stalls, kept separate from the no-answer case, recording
     how long the thread is held; a reply over
     the limits; a test server whose higher-priority SRV record is
     delayed past the first batch, and separately delayed by more than
     `DNS_SETTLE` after it: in both, the gate passes only if the query
     times out at the deadline; a run in which the wrapper returns an
     SRV set, or a conclusive negative answer for this published name,
     fails the gate for that release, and the site stays unused there
     until the design is amended. A sole SRV record with the target "."
     is held to the deadline too: the query times out. So is "." with
     another SRV record, whether the second record arrives before or
     after the settle expires, provided it arrives before the deadline:
     the query times out, never "unusable answer". The collector's
     test instrumentation records, on the awake clock, each batch end
     (a reply without `kDNSServiceFlagsMoreComing`), what the collector
     holds then, each later readability and the deadline. Each case is
     credited against its own timing: the first counts when a batch
     ended with a usable lower-priority record held and the delayed
     record arrived in a later batch within `DNS_SETTLE`; the second
     counts when a batch ended with a usable lower-priority record held
     and the higher-priority record absent, no readability followed for
     at least `DNS_SETTLE` (measured from the last batch end, so two
     batches within `DNS_SETTLE` start the interval at the second), the
     higher-priority record stayed absent throughout that interval, and
     the settle timer expired while the query deadline was still
     ahead. A run in which the resolver delivers both records in
     one batch counts for neither. Until each case is credited on a
     release, the qualification there is MISSING and discovery on that
     release uses explicit configuration only (point 3.1, "No discovery
     from an unqualified site"). The collector's own decision is also
     tested deterministically over fabricated replies and times, as the
     Miri callback test fabricates replies: a lower-priority record
     held, the deadline ahead, `DNS_SETTLE` expiring before the
     higher-priority record arrives: the collector gives no complete
     answer then and the query is a timeout at the deadline; the same
     for AFSDB records, one held and another arriving after the settle,
     and for a sole "." SRV record; a
     record of the
     asked type that the callback itself rejects (`rdata` NULL with
     `rdlen` not zero), alone and mixed with valid records of that type:
     "unusable answer" for the whole answer, never a negative answer and
     never the valid subset; bytes that pass the callback's own checks
     but that the shared decoder would reject are held like any record,
     and the query is a timeout (the decoder's rejections are tested with
     the shared codec); a reply of another type or class mixed with valid records:
     ignored (counted), the valid records stay held, and the query is a
     timeout at the deadline; a
     record removed while the query runs; TTLs read back; and the
     callback under Miri over fabricated replies, a non-zero
     `errorCode` with poisoned other parameters included.

### 4. The build script: the target gate and the macOS C shim

**Working.** Revisit if: O8's shim decision is revisited, Apple ships a
non-macro entry point for unified logging, the build fails on a
supported Xcode (L37, point 2), a build of the crate is found that
does not run its build script (then point 5.3's gate has a gap), or
the pinned toolchain starts admitting custom targets in its normal
stable mode, or the workspace's standard builds enable unstable
custom-target support.

The crate's one build script, `build.rs`, does two things, in this
order: of the targets Cargo selects by a built-in triple, it refuses
every one outside point 5.3's list (the target gate, amended
2026-11-22; its scope is below), and on a macOS host building for macOS it
builds the shim. It is written in IronAFS's own Rust with the standard
library only: it has no `[build-dependencies]` (point 1.4 covers every
dependency table), runs no other program but the two `xcrun` commands
below, and compiles or runs no foreign code but the shim (point 1.1).

- **The target gate** (amended 2026-11-22). The script reads Cargo's
  `TARGET` variable, the short name Cargo reports for the target: the
  triple of a built-in target, or, for a custom JSON target, the stem
  of the specification file's name (`foo` out of `foo.json`), never
  its path. It compares that name, as an exact string, with the seven
  triples of point 5.3. On any other value, an
  absent or non-UTF-8 one included, it prints one
  `cargo::error=` line naming the triple and point 5.3, and exits with
  a nonzero status, so Cargo fails the build before the crate is
  compiled. The value is never printed raw: `TARGET` can be the stem
  of a target specification file's name, which may itself hold a line
  break, and a raw line break would let the rest of the value become a
  Cargo directive.
  A UTF-8 value is printed in Rust's debug form (`{:?}`), which escapes
  every line break and control character, so the refusal is always one
  line; an absent value is printed as "unset" and a non-UTF-8 one as
  "not UTF-8", with none of its bytes. **Scope.** The comparison
  identifies the target by the value Cargo reports, which is the
  target itself only when Cargo selects it by a built-in triple. The
  pinned toolchain rejects custom JSON targets in its normal stable
  mode: Cargo accepts a `.json` target only with unstable target
  support enabled (`-Z json-target-spec`, usually with
  `-Z build-std`), on a nightly toolchain or with `RUSTC_BOOTSTRAP=1`
  set. Whenever unstable target support is enabled, a builder can name
  a specification after a listed triple and copy its cfg values, and
  neither the gate nor point 5.3's `compile_error!` guard can tell it
  apart. The refusal is guaranteed for targets Cargo selects by a
  built-in triple; custom targets are outside it. The gate compiles nothing,
  writes nothing and reads no
  other variable, so it runs the same on every host, for `cargo build`,
  `cargo check`, `cargo clippy`, `cargo doc` and Miri alike.
- **When Cargo reruns the script.** While the script holds the gate
  alone, it tells Cargo to rerun it only when `build.rs` changes
  (`cargo::rerun-if-changed=build.rs`); a new `TARGET` is a new build
  in any case. When the shim is added (M4), the same script also names
  the shim's C file and every other file it reads in
  `cargo::rerun-if-changed=` lines, so an edit to any of them reruns
  the script and rebuilds the shim; a stale archive is never linked.
  **A requirement on the M4 shim change:** the build reruns, and the
  shim archive is rebuilt, whenever any effective input of the shim
  changes that Cargo does not already track. That includes the
  headers the compiler reads (the SDK's `<os/log.h>` and what it
  includes), the selected SDK and Xcode, and the deployment version
  and every other build setting the script passes to `xcrun`. How the
  script tracks them is that change's design, reviewed under point
  6.6.

The macOS C shim:

- One C file inside `ironafs-os`, IronAFS's own, with an SPDX header. It
  includes only `<os/log.h>` and `<stdint.h>`. It uses one constant format
  string with a public event id, a public string and a private string
  (O8, point 2; `[DES-020]`), allocates nothing, and keeps no state except
  the log handles it creates once. Its exported functions take fixed-width
  integers and `const char *` only, and check each integer against the
  shim's own tables (sheet 5). It does no DNS work (point 3.3).
- **Built by the same `build.rs`**, after the target gate (the written
  reason rule 20 asks for is this ADR, O8, point 2, and point 5.3). On a
  macOS host building for a macOS target, it runs `xcrun clang`
  `[DES-021]` for Cargo's target architecture and deployment version,
  with warnings as errors, then `xcrun libtool -static`, and tells Cargo
  to link the result statically. It reads only the named Cargo
  variables, writes only under `OUT_DIR`, and uses no network.
- On any other host or target it compiles nothing (the gate still runs),
  so `cargo check` and `cargo clippy --target` for macOS still run from
  Linux. Linking the macOS daemon needs a macOS host. Until the shim
  lands (M4), the build script holds the target gate alone.
- **The shim is linked into the daemon only.** O8, point 2, leaves open
  whether other Rust binaries (the `afs` CLI) get it; this ADR does not
  link it into them, so the gap O8 states remains until an amendment.
- **No `cc` crate.** `cc` 1.11.2 brings `shlex` and `find-msvc-tools` with
  it (crates.io, INSPECTED 2026-11-19): three crates of build-time code for
  one file on one OS.

### 5. Our own declarations, and how they are proved

**Firm** (points 6 and 7, a security invariant: a site whose check has
not run on a target does not ship there). **Working** (points 1 to 5).
Revisit points 1 and 2 if: the upkeep proves larger than expected, or an
ABI error reaches `main` that an upstream crate's tests would have caught.
Revisit point 2's per-C-library declaration of `struct pollfd` and
`nfds_t` if point 5.6's check shows the glibc and musl declarations
equal on every Linux target, or a declaration wrong on one (amended
2026-11-22).
Revisit point 3 if: a row of L37 to L39 is added, removed or qualified,
or a compiler update changes which targets share a listed target's cfg
set.
Revisit point 4 if: the link-and-run check fails on a supported Windows
release, or the pinned toolchain's `raw-dylib` support changes. Revisit
point 5 if: L42's glibc floor moves, or a supported kernel lacks
`getrandom`.

Point 1 was **decided by the owner on 2026-11-20**: our own
declarations, proved by the per-target ABI check of point 6, with the
`libc` crate as the named fallback (Alternatives). A dependency stays the
owner's to decide (rule 21), so taking the fallback is a new owner
decision and a change to this point (point 7).

1. **Every foreign declaration is written in `ironafs-os`**, from the
   documents cited in Context and the platform's own headers. The `libc`,
   `windows-sys` and `rustix` crates are not used.
2. **The cost is accepted and stated:** IronAFS owns the ABI correctness of
   each item: the symbol name and library, the calling convention, every
   parameter and return type, the layout of `struct timespec`,
   `struct pollfd`, `struct rlimit`, `struct ucred`, `struct msghdr`,
   `struct iovec`, `struct cmsghdr` (and the `CMSG_*` arithmetic, which
   the header defines as macros), `struct sockaddr_in6`,
   `struct sockaddr_nl`, `struct statvfs`, `DNS_QUERY_REQUEST`,
   `DNS_QUERY_RESULT`, `DNS_QUERY_CANCEL`, the parts of `DNS_RECORDW` the
   wrapper reads, `DNS_SRV_DATAW` and `DNS_MX_DATAW`, `struct xucred`,
   `SECURITY_ATTRIBUTES` and the self-relative security descriptor's
   layout, `OVERLAPPED` and the chosen completion family's structures,
   the token classes' structures (`TOKEN_USER`, `TOKEN_STATISTICS`,
   `TOKEN_LINKED_TOKEN`, the elevation and impersonation-level
   enumerations) and the `SID` layout, and the value of
   every constant, the system-call number of `getrandom` on each Linux
   target included (318 on x86_64, 278 on aarch64; Context). `struct
   msghdr` and `struct cmsghdr` are declared per C library: glibc and musl
   give the same sizes and offsets on these targets but different field
   types and musl's padding fields (Context), which the wrapper zeroes.
   `struct pollfd` and `nfds_t` are declared per target: row 7's on
   macOS, and row 23's per C library on Linux, as `struct msghdr` is,
   since `nfds_t`'s type differs from macOS's and its width in glibc and
   musl is UNVERIFIED until point 5.6's check (Context). No
   upstream crate's tests stand behind them.
3. **A target allowlist.** For Cargo builds that select a built-in
   triple, the crate compiles for these targets only, all 64-bit, and
   the build refuses every other built-in triple (amended 2026-11-22;
   the refusal and its scope are below the list):
   - `x86_64-` and `aarch64-unknown-linux-gnu` (L37, point 1; L42, point 3);
   - `x86_64-` and `aarch64-unknown-linux-musl` (the static musl candidate,
     L39, point 5);
   - `aarch64-apple-darwin` (L8, point 2: Intel Macs are not targets);
   - `x86_64-` and `aarch64-pc-windows-msvc` (L37, point 3; L38).

   The list holds L37 and L38's targets, and an L39 candidate only when
   it needs no site the list does not already have (static musl uses
   `linux-clock`). Other candidates, FreeBSD among them (L39, point 6),
   are added when their row gains a venue, by a change to the list: the
   overview lists the BSDs as a non-goal in this phase, and FreeBSD's
   clock_gettime(2) changes between releases: up to 14.3 `CLOCK_BOOTTIME`
   "increments monotonically in SI seconds while the machine is running"
   and "is an alias for CLOCK_UPTIME"; in 15.0 it "Increments in SI
   seconds, even while the system is suspended" and "is an alias for
   CLOCK_MONOTONIC", while `CLOCK_UPTIME` increments "while the machine is
   running" ([clock_gettime(2), 14.3](https://man.freebsd.org/cgi/man.cgi?query=clock_gettime&sektion=2&manpath=FreeBSD+14.3-RELEASE),
   [15.0](https://man.freebsd.org/cgi/man.cgi?query=clock_gettime&sektion=2&manpath=FreeBSD+15.0-RELEASE);
   CONFIRMED 2026-11-20). No release documents an awake clock that stops
   in suspend, so C37's pair for it needs its own evidence first.
   Adding a target is a change to the list (point 7).

   **The refusal has two parts, and both stay** (amended 2026-11-22).
   Both cover the targets Cargo selects by a built-in triple; a custom
   target, which Cargo accepts only with unstable target support
   enabled, is outside the
   guarantee (Decision 4, "Scope"):
   - **The target gate** of Decision 4 refuses, by exact triple, every
     target not on the list that Cargo selects by triple. It is the part that makes the refusal
     exact. Some of the compiler's targets have exactly the cfg set of a
     listed one (vendor variants, `arm64e`, and sanitizer variants of
     `x86_64` glibc: six on the pinned compiler, as the review of the
     crate's first change counted them; UNVERIFIED until the check
     below runs), so no `cfg` in source can tell them apart; the
     triple can.
   - **The `compile_error!` guard** in the crate root refuses every
     target whose cfg set differs from every listed target's. It is
     kept as a second net: it holds even where a build script is not
     run, and it names the property that differs.

   `cargo xtask check-os`, which `cargo xtask check` runs beside point
   6.7's inventory, checks both layers:
   - the gate's seven triples must equal this list exactly. The
     check's reference copy of the list is kept apart from the gate,
     never read from the gate's own code, and is tied to this point's
     text by review: a change to either the gate's list or the
     reference copy without the other fails the check;
   - the production gate itself must accept each listed triple: an
     affirmative result from the gate's own code for each of the seven,
     obtained before any shim compilation, linking or ABI check (how
     the check isolates the gate is the follow-up change's, reviewed
     under point 6.6);
   - the guard's expected verdict is derived from this list, not from
     the guard: a target is expected to pass the guard exactly when its
     whole cfg set (every name and every `key = "value"` pair that
     `rustc --print cfg --target` reports on the pinned compiler)
     equals a listed target's. For every built-in target of the pinned
     compiler, the guard's evaluated verdict is compared with the
     expected one, and any disagreement, in either direction, fails the
     check. So the guard must accept every listed target and refuse
     every target whose cfg set differs from every listed one's;
   - every target the guard accepts but the list does not name (the
     cfg-equal targets above) is built through Cargo, which must reach
     the gate and fail in the build script with the gate's message, not
     later and not for another reason. A target that fails that way is
     refused; one that builds, or fails for another reason, fails the
     check.

   Its negative tests: a gate that accepts one cfg-equal extra triple, a
   gate list missing one listed triple, a gate that refuses one listed
   triple, and a guard widened by one target, each make it fail. Until this check has run on the pinned compiler, the
   cfg-equal targets are MISSING evidence of refusal
   (`docs/workflow.md` §3c), and a compiler update reruns it (as point
   5.7 does for headers).
4. **Windows.** Declarations are `extern "system"`. Microsoft's reference
   names Kernel32.dll as the DLL for both clock functions, `mincore.lib` as
   the library for `QueryUnbiasedInterruptTimePrecise`, and its Interrupt
   Time page says to link `OneCore.lib`; it names Dnsapi.dll and
   Dnsapi.lib for `DnsQueryEx`, `DnsCancelQuery` (both Windows 8 and
   later) and `DnsFree`, and Bcrypt.dll and Bcrypt.lib for
   `BCryptGenRandom` (CONFIRMED 2026-11-20 on each function's page). The
   pipe, token and process functions of rows 17 to 20 have not had their
   pages read for this ADR: their DLLs and import libraries are MISSING
   until then (Context), and the link-and-run check below settles each.
   The
   `QueryUnbiasedInterruptTimePrecise` page names Kernel32.dll and lists
   further API-set and KernelBase hosts among its locations, so the pages
   do not settle which DLL to import it from: MISSING. The DLL each
   declaration names is set by an executed link-and-run check on each
   supported Windows release, and recorded in the site's comment. The
   declarations use `kind = "raw-dylib"`, so that no import library from a
   crate is needed.
5. **The C library on Linux.** Every Linux site links its function from
   the C library (glibc, or musl in a static build), and only functions
   glibc offers in 2.17 at the latest (`clock_gettime` is in libc itself
   from 2.17; before that it was in librt; Context). `getrandom` is
   reached through `syscall` with the call's number, not through glibc's
   wrapper, which glibc 2.17 lacks (added in 2.25). Each site's check
   runs the built binary on the oldest and the newest supported glibc
   (L42, point 3: 2.28 as the build goal, 2.17 for a conditional RHEL 7.9
   artefact), and on musl. MISSING until executed. The RHEL 7.9 kernel
   offers the `getrandom` call on x86_64 (backported in RHEL 7.4;
   CONFIRMED, Context); Red Hat states no other RHEL 7 architecture.
   Where a kernel lacks it, the call returns `ENOSYS` and the process
   refuses to start (point 2, *Random source*), rather than falling back
   to `/dev/urandom` as Red Hat's article suggests; the refusal names
   "unavailable or filtered" with the real errno, and a call that blocks
   early in boot is waited for under the unit's start timeout (sheet 8;
   owner, 2026-11-20). Rows 21 to 23 use `socket`, `setsockopt`, `bind`, `sendto`,
   `recvmsg`, `poll`, `eventfd` and `fstatvfs`; `fstatvfs` was in the
   x86-64 glibc ABI before 2.17 (CONFIRMED, Context item 8), `eventfd`
   is in glibc from 2.8 (CONFIRMED, Context), and the others are older
   POSIX and BSD calls (UNVERIFIED at a glibc 2.17 source; the
   run on 2.17 settles it). That musl offers `eventfd` with the same
   flags is UNVERIFIED; the musl run settles it.
6. **An ABI check per target, in CI.** For each site with a foreign item
   (a site with none, `mem-wipe`, is gated by sheet 24's release-assembly
   check instead, under the same MISSING rule), a small C file is
   compiled with the platform's own compiler and headers on that platform's
   runner. It contains only compile-time assertions:
   - the size, alignment and field offsets of every struct the site uses;
   - the type of every member the site reads or writes, by a C11
     `_Generic` selection on the member (an array by its element type and
     count, a pointer by the type it points to), so a field of the wrong
     type with the right size and offset fails, as the glibc and musl
     `msghdr` fields would;
   - the value of every constant;
   - each function's type, by the same `_Generic` selection with no
     `default` branch, on the function's address against a pointer to
     the declared prototype, so an incompatible type is a hard compile
     error, not a diagnostic beside an object file; for `darwin-dns`
     also the callback's type, against `DNSServiceQueryRecordReply`, and
     for `windows-dns` the completion routine's, against
     `DNS_QUERY_COMPLETION_ROUTINE`. The check is compiled as C11 with
     warnings as errors, as the shim is (point 4).

   The expected values are generated from the Rust declarations by an xtask
   command, so the two cannot drift apart unseen. The check has its own
   negative tests: a declaration whose member type is changed without
   changing its size or offset, a constant changed by one, a function
   with one parameter or its return type changed, and a callback
   signature changed, each make it fail, on each target's compiler. The C file is never linked
   into a product. A site whose check has not run on a target is MISSING
   evidence there (`docs/workflow.md` §3c), and the site does not ship on
   that target. For `windows-dns`, the executed check also records what a
   NULL `pDnsServerList` does (Context).
7. **A header or library change** on a supported platform (a new glibc, SDK
   or Windows release in the test matrix) reruns the check before that
   platform is called supported.

### 6. What every site requires

**Firm.**

1. **A named expectation:** `#[expect(unsafe_code, reason = "<site id>")]`,
   with the id from the table, on the `unsafe extern` block, on each
   `unsafe` block and on each `unsafe impl`.
2. **A `// SAFETY:` comment on each `unsafe` block and `unsafe impl`,**
   and a comment of the same standing on each `unsafe extern` block. It
   states, from the site's sheet (point 2), as facts a reader
   can check: every precondition the foreign function documents and what in
   the code meets it; who owns each pointer and for how long; the thread
   rule; and the document the declaration was written from. A `safe fn`
   declaration carries the argument for why no value can make the call
   undefined.
3. **One private function per site block.** Each `unsafe` block holds
   exactly one of four things and nothing else: one foreign call; one
   bounded read of memory a foreign call returned or passed in (the DNS
   reads of point 3); one adoption of ownership from a raw value
   (`Arc::from_raw` in `windows-dns`, `OwnedFd::from_raw_fd` in
   `linux-fdpass`, `linux-udp6` and `linux-rtnetlink`,
   `OwnedHandle::from_raw_handle` in rows 17, 19 and 20); or one volatile
   write of a zero byte through a pointer made from an exclusive
   reference (`mem-wipe`, row 24). The fourth kind was added before this
   ADR's acceptance (owner, 2026-11-20): O28, point 2 (Firm) needs key
   material zeroed, a plain overwrite of memory about to be freed may be
   removed by the compiler, and no safe Rust operation promises the
   write is kept; the kind admits nothing that touches foreign code or
   foreign-owned memory. Validation and conversion sit in safe code around it.
   The only `unsafe impl`s are the two of `windows-dns`.
4. **A test per site, on the OS itself** (rule 15, "Platforms"): the
   success path, every documented failure the test can provoke, and the
   wrapper's own refusals (a level outside the set, an invalid name, a
   deadline that passes, a reply over the limits). The change that adds
   the first site for an OS adds that OS's runner to CI (O49).
5. **Miri.** The Miri job rule 5 requires starts with this crate. Under
   `cfg(miri)` each foreign call is replaced by an in-crate stand-in, so
   Miri checks the Rust on both sides: how buffers and pointers are made,
   how results are validated, and, for DNS, the record-list walk over a
   fabricated list and the callback over fabricated replies. It does not
   check the call itself; points 5.6 and 6.4 do. The shim's own test is
   also built with the compiler's address and undefined-behaviour
   sanitizers. **Fuzzing** does not stand in for Miri, nor Miri for
   fuzzing (O47, point 3; coding standards rule 15): every validator of
   data a foreign call wrote is a safe function over owned input and has
   a fuzz target in the top-level `fuzz/` crate, over owned fixtures (a
   byte arena in which pointers are offsets the fixture resolves). They
   are: the Windows DNS record-list walk (short headers, bad
   `wDataLength`, unterminated names, more than 64 entries along
   `pNext`; each fixture's arena has a known extent, and a short or
   unterminated fixture is rejected without any read outside it, which
   Miri checks over the same fixtures); the macOS reply callback's checks; the token and SID
   readers of sheets 19 and 20 (short buffers, bad sub-authority counts,
   a SID pointer outside the token buffer); the ancillary-data checks of
   sheet 14 (bad `msg_controllen` and `cmsg_len`, wrong level and type);
   the netlink sender check of sheet 23 (short addresses, a wrong
   family); and the `revents` validator of sheet 23's wait (invalid
   bits, a count that disagrees with the entries, both entries ready).
6. **Class S** (`docs/workflow.md` §1 lists `unsafe` code as S), for every
   change to a site, a declaration, the shim or the build script, and for
   any change in any crate that adds non-Rust source, or adds or edits
   a build script so that it compiles or runs foreign code or a native
   helper (point 1.1): the design
   reviewed by a non-author family before code, the code reviewed, a
   security review, and the reviewer given this ADR's table to check the
   change against. A change elsewhere in the crate takes its own class.
7. **The inventory is checked.** A machine-readable copy of this ADR's
   table (site ids, block counts, and the "Foreign item" and "Linked
   from" columns), kept in the crate, holds every row of the table and
   is checked against it by review, so the copy matches the full table.
   `cargo xtask check` compares the code with the copy: the site ids,
   the `unsafe` blocks and `unsafe impl`s found in the crate, and each
   `extern` block's item names, link names and library, per target,
   must match the copy's implemented rows exactly (below), and any
   difference fails.
   - **Approved and implemented rows.** Sites land in stages (rows used
     from M2b, macOS joining in M4, row 18's count fixed by a change
     before code), so the machine-readable copy marks each row of this
     table, per target, as *approved* or *implemented*. Every row of the
     table is in the copy. The check requires an exact match for each
     implemented row; it fails on any site, block or foreign item in the
     crate that belongs to an approved-only row or to no row, and on an
     implemented row with an item missing. A row becomes implemented in
     the change that adds its code, reviewed under point 6.6. A row
     whose table entry is still open (row 18's completion calls and
     block count) cannot be marked implemented until its change (point 7)
     has fixed them.
   - **Each block's operation.** The copy also records, for each
     `unsafe` block, its kind (one of the four of point 6.3) and the
     one operation it holds: the foreign item it calls, the read it makes, the adoption,
     or the volatile write. The check fails on a block whose body holds
     anything other than that one operation.
   Its negative tests swap one declaration for another without changing
   the block count; add one foreign function to an existing block; add
   code for an approved-only row; remove one item of an implemented row;
   and add a second foreign call or raw read to a block's body while
   keeping its id, the block count and the foreign-item set. Each makes
   the check fail.

### 7. Changing the list

**Firm.**

1. Adding, removing or changing a foreign item, a site, a constant, a
   target, the shim's interface or the build script's commands needs a
   change to this ADR, accepted by the owner **before the code is
   written**.
2. A row of point 2, and points 3, 4 and 5.1 to 5.5, are Working: they
   change by an amendment in place (`docs/adr/README.md`, "Amending a
   working decision"), reviewed as this ADR was, as class S on the
   strongest row, with the supersession sweep.
3. The firm points (1.1, 1.2, 1.5, that the list is closed, 5.6, 5.7, 6
   and this one) change only by a superseding ADR. An amendment never
   weakens them: a new row meets point 6 in full.
4. A change that alters no row (a comment, a test, safe code around a
   site) needs neither.

## Consequences

- **`unsafe` exists in the workspace, in one place** until another binding
  crate's ADR is accepted. A reader can audit all of it by reading one
  crate and one table. Every other crate still forbids it, and the check in
  point 1.5 keeps it so.
- **IronAFS carries the ABI work** that binding crates usually carry:
  point 5, on every target, for as long as the sites exist. The DNS sites
  are most of it: six foreign structures on Windows with a union read in
  part, a callback on each of the two systems, and on Windows a block
  shared with a thread IronAFS does not own. The Linux socket structures
  (rows 14, 21 and 23) and `struct statvfs` are declared once per C
  library. The Windows control pipe adds the
  next largest share: a security descriptor, overlapped I/O whose buffers
  Windows owns until completion, and token structures. In exchange the
  crate has no dependencies and no third-party build-time code.
- **The Windows DNS site is the hardest proof in the crate.** Its
  soundness rests on Microsoft's documented lifetimes, on two `unsafe
  impl`s argued in prose, and on two behaviours only a run can show
  (point 3.2). Until those runs pass on a release, DNS discovery does not
  ship there and cells are configured explicitly (C33, point 1). The
  same holds on macOS until point 3.3's completion gate passes (point
  3.1, "No discovery from an unqualified site").
- **Costs the controls bring.** A non-dumpable Linux process cannot be
  attached with `ptrace` by its own user and leaves no core file, so a
  crash in the field is diagnosed from logs alone (O25 accepts this).
  Flags 0 on `getrandom` can block startup early in boot until the
  kernel's pool is ready. Each secret buffer costs 64 KiB (accepted by the owner, 2026-11-20).
  A kernel without `getrandom`, or a sandbox that filters it, stops the
  process from starting, with "random source unavailable or filtered" and
  the real errno, and no fallback (owner, 2026-11-20). Each sink's complete message is
  capped (480 octets on the RFC 5424 fallback until a larger limit is
  qualified), so long detail is truncated, marked and counted, and
  reaches the support bundle instead (Decision 1, point 7).
  The route sockets' bounded wait (row 23; amended 2026-11-22) costs
  one `eventfd` descriptor per waiting worker (two for the
  network-change monitor) and one wake per worker per
  `ROUTE_WAIT_HORIZON` while idle.
- **Coding standards rule 5 changes** in the same pull request as this ADR:
  "every crate inherits the workspace lints" gains its one named exception.
  Rule 20 is not edited here: it changes with the layout pull request, a
  separate change that is pending. Until then this ADR, O8, point 2,
  and point 5.3's target gate are the written reason for this crate's
  `build.rs` that rule 20 asks for (Decision 4).
- **The target gate** (amended 2026-11-22; point 5.3, Decision 4).
  Before this amendment the crate's own comments read point 1.4 as
  forbidding any build script and stated the cfg-equal targets as an
  accepted limit; point 1.4 forbids dependencies, and point 1.1 only a
  build script that compiles or runs foreign code or a native helper,
  so neither forbids the gate. The gate comes as its own class S change
  (point 6.6) after the owner accepts this amendment, not inside
  another change's fix round. That change writes the build script,
  teaches `cargo xtask check-os` the check of point 5.3, corrects the crate's
  comments that cite point 1.4 for "no build script" and state the
  limit, and adds the crate's own build script to `deny.toml`'s
  build-script allowlist (`[bans.build]`, which covers workspace
  crates). It adds no dependency.
- **CI grows:** the ABI check and the per-site tests on Linux, macOS and
  Windows runners, a C compiler on each, the Miri job, and the inventory
  check. None of them is a Cargo dependency.
- **Measurements still owed:** before any deadline relies on the clocks,
  each pair across sleep, hibernation, Modern Standby and a paused virtual
  machine, on each supported OS release (C37's trigger); before DNS
  discovery ships on Windows, what a NULL server list does, how long a
  query to an unreachable resolver runs before Windows completes it, and
  the asynchronous query's cancel and completion behaviour (point 3.2's
  tests); the random-source sites' failure paths on each OS (`ENOSYS` on
  the oldest supported Linux kernel); the dump marker test of O26,
  point 5, on Linux and macOS; on macOS, the stalled DNS reply of point
  3.3; on Linux, rows 21 to 23's runs in a network namespace (both
  `bindv6only` values; a netlink overflow and truncation, and the wait's
  report of an overflow; whether a plain write reaches the kernel, which
  the site does not rely on; whether a send to the kernel can wait inside
  the kernel despite `MSG_DONTWAIT`); and before
  the control pipe ships on Windows, the gates of sheets
  17 to 20 (a pre-created pipe name, the 1368 answer before the prefix
  read, revert on every path, a disconnect and reconnect between the
  prefix read and the caller's capture, an ordinary user's check of the
  server, and the server replaced between connect and check); on macOS,
  the DNS completion rule of point 3.3 (a record delayed past the first
  batch and past `DNS_SETTLE`, a removal, a rejected record);
  the per-sink log caps of Decision 1, point 7, read back through each
  sink (macOS included) before that sink is enabled; and the dump marker
  test's positive control (sheet 11) on each OS. All recorded as
  executed evidence, each by the issue of the code that needs it.
- **C5, point 5's list is amended in the same pull request** (owner,
  2026-11-20; C5 is Working; `docs/adr/README.md`, "Amending a working
  decision"). It was reviewed in the scoped rounds 15 to 17 (GPT
  `gpt-6.2-sol` at xhigh and GLM `glm-5.3` at high; see Review), with
  GPT on `gpt-6.2-sol` rather than the strongest row's
  `gpt-6.2-astra` by the owner's choice (recorded in the review record),
  and the owner accepted it with this ADR. It gains
  the items this table holds that the list does not name: the resolver
  calls of C33, point 2; the random source of C5, point 1; the Linux
  mount call and FUSE descriptor hand-off of L7, point 1, both
  directions; the control endpoint's named-pipe calls (C45, point 1),
  next to the peer credentials it already names, with both Windows
  attestations named: the caller's token in the daemon and the server's
  process token in the sender (C43, point 4; sheets 19 and 20); and the M2a sweep's
  three Linux calls (owner, 2026-11-20): the IPv6-only callback socket (C34, point 2),
  the chunk store's free-space query (C27, point 3) and network-change
  notification (C23, point 2); and the volatile wipe of key material and
token buffers (O28, point 2; row 24; owner, 2026-11-20). The same amendment names the safe
  log-transport module in C4, point 3's driver table (C4 is
  Working). The amendment is issue #49; it runs the supersession sweep,
  adds `amended 2026-11-21 (#101)` to ADR 0004's status line and an
  entry to its Review section. The text:

  > 5. `ironafs-os` holds the few calls that have no safe route: the
  >    clock pairs of C37; the cryptographic random source of point 1,
  >    from the operating system; the resolver calls of C33, point 2, on
  >    macOS and Windows; peer credentials (C43) and, on Windows, the
  >    control endpoint's named pipe: its creation, its I/O and its
  >    caller's token (C45, point 1), and, in the sender, the pipe
  >    server's process token checked against the daemon's service
  >    identity (C43, points 1, 2 and 4); the Linux mount call and the FUSE
  >    descriptor hand-off (L7, point 1); on Linux, the IPv6-only socket
  >    of the callback service (C34, point 2), the free-space query of
  >    C27, point 3, and network-change notification (C23, point 2); the
  >    service controls; the log calls of O8, with a safe module that
  >    carries encoded messages to the operating system's log sinks; the
  >    dump controls of O25 and O26; the volatile wipe that zeroes key
  >    material and token buffers (O28, point 2); and, on Windows,
  >    the query of the operating system's FIPS algorithm mode (O31,
  >    point 4). It needs `unsafe`, so it gets its own ADR with a closed
  >    list of named sites and safe interfaces on top, before any of it
  >    is written. Since Rust 1.82 a foreign declaration can be marked
  >    safe to call, with the proof left to whoever writes the
  >    declaration `[DES-018]`; that ADR states the proof for each.

  C5's "Revisit if" line is kept and gains: "or the `ironafs-os` ADR's
  closed list adds a kind of call this point does not name". C4,
  point 3's row for `ironafs-os` becomes: "small operating-system
  bindings (C5), and the safe transport that carries encoded events to
  the operating system's log sinks (O8)", and C4's "Revisit if" gains:
  "or no log sink outside Linux still needs a site in `ironafs-os`
  (then the safe log transport could live in a crate without
  `unsafe`)". Large journal fields have no
  site (Decision 1, point 7), so the text names none. The C34, point 2,
  amendment for single-family operation is a separate Working
  amendment, not part of this pull request.
- **Not yet on the list.** C5, point 5, puts these in `ironafs-os` too.
  Each needs an amendment (point 7) before its code is written:
  - **Windows dump protection** (O25, Firm; O26, point 3). Required
    before any Windows run of a process that can hold credentials **or
    handle data beyond the invoking user's own** (O25, point 2: both
    classes), the SMB identity spike included: O25 holds from startup
    to exit on every OS, and the spike is not an exception. Its marker
    test has the positive control of sheet 11: a collector shown to
    capture the marker without the protection receives no image with
    it. `WerAddExcludedApplication` writes a persistent
    exclusion list, not a per-process switch, and Windows' LocalDumps
    setting can write a dump without Windows Error Reporting's reporting
    path, so suppressing reporting alone is not enough. The mechanism is
    researched and amended in before that first run;
  - **service controls on macOS and Windows** (C5, point 5; L14, L23), in
    M4 and M5;
  - **the Windows Event Log and ETW provider** (O8, point 3), in M5;
  - **the Windows FIPS algorithm-mode query** (O31, point 4), in M5. The
    Linux flag is a file read and needs no site.
- **Linked components and GPLv3's System Libraries:** see the next
  section.

## Linked operating-system components and GPLv3

ADR 0001 asks each platform's ADR to say which linked components meet
GPLv3's "System Libraries" definition, checked per component. This is
that check for the closed list. It is not legal advice, and it decides
no macOS or Windows component's status: that is the owner's (below).

- **The definitions** (GPLv3 §1, `LICENSES/GPL-3.0-or-later.txt`,
  quoted exactly, CONFIRMED):
  - "A “Standard Interface” means an interface that either is an
    official standard defined by a recognized standards body, or, in the
    case of interfaces specified for a particular programming language,
    one that is widely used among developers working in that language."
  - "The “System Libraries” of an executable work include anything,
    other than the work as a whole, that (a) is included in the normal
    form of packaging a Major Component, but which is not part of that
    Major Component, and (b) serves only to enable use of the work with
    that Major Component, or to implement a Standard Interface for which
    an implementation is available to the public in source code form. A
    “Major Component”, in this context, means a major essential
    component (kernel, window system, and so on) of the specific
    operating system (if any) on which the executable work runs, or a
    compiler used to produce the work, or an object code interpreter
    used to run it."
  - "The “Corresponding Source” for a work in object code form means
    all the source code needed to generate, install, and (for an
    executable work) run the object code and to modify the work, [...].
    However, it does not include the work's System Libraries, [...]."
- **How each row is read.** A component qualifies only if (a) holds for
  a named Major Component and one of (b)'s two routes holds: "serves
  only to enable use of the work with that Major Component" (the
  *serves-only route*), or implementing a Standard Interface of which
  some implementation, by anyone, is public in source form (the
  *Standard Interface route*). Each is assessed separately below. OPEN
  marks what is not verified for this ADR: the normal packaging, the
  Major Component's boundary, and which Standard Interfaces each called
  function belongs to and whether a public-source implementation exists.

| Component | Linked how | (a): candidate Major Component | (b): serves-only route | (b): Standard Interface route | Status |
|---|---|---|---|---|---|
| Linux kernel (`syscall` for `getrandom`; the socket, `mount`, `madvise` and `prctl` calls through the C library) | not linked: system calls | The kernel is itself a Major Component; IronAFS talks to it and links none of it (ADR 0001, "Out of scope"). | not needed | not needed | CONFIRMED (GPLv3 §1 names the kernel; ADR 0001) |
| glibc (rows 1, 8, 11 to 15, 21 to 23) | dynamically | OPEN: the kernel, as each distribution packages it, or another component; packaging and boundary not verified | OPEN: not relied on | Reads on its ISO C and POSIX functions (official standards; glibc's own source is public); OPEN per Linux-specific function | System Library reading UNVERIFIED. Its licence is LGPL-2.1-or-later, which is GPLv3-compatible, so the outcome does not depend on the reading: compatibility CONFIRMED (licence) |
| musl (static Linux build) | statically, into the binary | not assessed | not assessed | not assessed | The static build is assessed under musl's own licence (MIT, GPLv3-compatible) and its notices, which travel with the binary (ADR 0001, "Releases"), without relying on the System Libraries exception. CONFIRMED (licence); notices are the Linux packaging decision's |
| macOS libSystem (clocks, dump limit, peer credentials, `poll`, `dns_sd`, unified logging) | dynamically | OPEN: ships with macOS; whether it is packaged with a Major Component (the kernel or the operating system's core) and not part of it is not verified. Apple's statement that it does not support statically linked binaries is UNVERIFIED (to be inspected at source). | OPEN: libSystem offers much more than enabling use with the kernel | OPEN: some calls are POSIX (`poll`, `setrlimit`, `getsockopt`; rows 7, 11, 16), others Apple's own (`clock_gettime_nsec_np`, `dns_sd`, unified logging; rows 2, 7, 5); per interface, not verified | owner's decision; UNVERIFIED |
| macOS Security.framework (`SecRandomCopyBytes`, row 9) | dynamically | OPEN, as for libSystem | OPEN: a framework with broad functions | OPEN: not verified for this interface | owner's decision; UNVERIFIED |
| The macOS C shim | statically, IronAFS's own code | Part of the work, under GPL-3.0-or-later; it calls only libSystem. Built by the platform compiler, which is a Major Component when used to produce the work. | not needed | not needed | CONFIRMED (own code) |
| Windows Kernel32.dll, Dnsapi.dll, Bcrypt.dll (rows 3, 6, 10) | dynamically (`raw-dylib`, no import library from a crate) | OPEN. Candidate: the Windows operating system's kernel, with which each DLL ships (each DLL ships with Windows and is its documented interface: Microsoft's function pages name each DLL; Context). Whether each DLL is included in that component's normal packaging and is not part of it, and where the component's boundary lies, is not verified | OPEN | OPEN: whether each called interface is a Standard Interface, and whether any public-source implementation of it exists (a third party's counts), is not verified | DLL names CONFIRMED; owner's decision; UNVERIFIED |
| Windows DLL of row 4 (`QueryUnbiasedInterruptTimePrecise`) | dynamically | OPEN. Candidate: the same as for Kernel32.dll (the Windows kernel, with which the DLL ships); packaging and boundary not verified, and the assessment completes only once point 5.4's check names the import target | OPEN | OPEN: whether the called interface is a Standard Interface with a public-source implementation is not verified | Import target MISSING: the function's page names Kernel32.dll and lists API-set and KernelBase hosts, so point 5.4's link-and-run check sets it; owner's decision; UNVERIFIED |
| Windows DLLs of rows 17 to 20 (expected Kernel32.dll and Advapi32.dll) | dynamically | OPEN. Candidate: the Windows kernel, with which each DLL that point 5.4's check names ships; packaging and boundary not verified, and the assessment completes only once that check names the import targets | OPEN | OPEN: whether each called interface is a Standard Interface with a public-source implementation is not verified | DLL names MISSING until point 5.4's check runs; owner's decision; UNVERIFIED |

- **Owner item.** Whether the proprietary macOS and Windows components
  (libSystem, Security.framework and the Windows DLLs) meet the
  definition needs (a) for a named Major Component and one of (b)'s two
  routes, and every one of those is OPEN above. The Free Software
  Foundation's own FAQ is commonly read as allowing a GPL program to
  link the operating system's own libraries (UNVERIFIED: not read for
  this ADR). How IronAFS relies on this, before the first macOS or
  Windows release, is the owner's decision; ADR 0001's research note
  already lists the System Libraries reading among the questions for a
  lawyer. Nothing here blocks the Linux phases: glibc is compatible
  either way, and the static build rests on musl's own licence.

## Alternatives considered

- **`std::time::Instant` for both clocks.** It is one clock per platform, so
  it cannot give a pair; its suspend behaviour is unspecified and differs
  by platform `[DES-017]`; and the documented calls behind it can change.
  A deadline that silently stops during sleep serves stale data. C37,
  point 3, rules it out.
- **`/proc/uptime` for the Linux boot clock.** It is safe to read and counts
  suspend ("including time spent in suspend",
  [proc_uptime(5)](https://man7.org/linux/man-pages/man5/proc_uptime.5.html)).
  Not taken: it needs procfs mounted in every container and sandbox the
  daemon runs in, costs a file read and a text parse per sample, has coarse
  resolution, and still leaves the awake clock to `Instant`. macOS and
  Windows have no such file, so the crate would exist anyway.
- **The `libc` crate.** No dependencies of its own, maintained by the Rust
  project, and it has the Unix declarations. Not taken (owner, 2026-11-20;
  point 5.1): the whole crate (about 0.85 MB
  of source, with a build script) for about a dozen functions and seven
  structures, under rule 21; and it does not remove the `unsafe` blocks,
  the shim or the Windows items. Its case is stronger than when this crate
  held only clocks: the socket structures differ between glibc and musl,
  and `libc`'s declarations are tested upstream on both. It stays the
  first fallback if point 5's upkeep proves larger than expected.
- **The `rustix` crate.** A safe API for clocks on Linux. Not taken: it
  brings `bitflags` plus `linux-raw-sys`, or `libc` and `errno` (crates.io,
  version 1.1.6, INSPECTED 2026-11-19); it has no `os_log` and no
  `dns_sd`; and the `unsafe` moves into a dependency instead of going away.
- **The `windows-sys` crate.** Microsoft's own bindings, which rule 21
  permits as an owner decision. Not taken: about 2.5 MB of generated
  declarations plus `windows-link` for six functions and six structures.
  It would remove the Windows half of point 5.2's upkeep, and is the
  fallback there for the same reason `libc` is on Unix.
- **The `cc` crate for the shim.** See point 4.
- **Stating the cfg-equal targets as a residual of point 5.3** (the
  crate documents them, `cargo xtask` names them, and every build
  matrix and release excludes them). Not taken (owner, 2026-11-22):
  point 5.3 asks that any other target be refused, and a pure-Rust
  build script reading the triple refuses them exactly, at the cost of
  one build script that runs on every build and an allowlist entry for
  it. A residual held only by build matrices would hold only as long
  as every matrix and every user's command line stays correct.
- **`SO_RCVTIMEO` on the route sockets** for row 23's bounded receive
  (one `setsockopt` and `struct timeval`, a fixed period such as one
  second). Not taken (owner, 2026-11-22): it bounds one receive call
  only, so a retried `EINTR` or a send outside the receive is not
  bounded by it; a stop waits up to a period instead of ending the wait
  when signalled; and the period is a constant rather than the caller's
  deadline. Both designs wake an idle worker once per period (here, the
  horizon). `poll` with a wake ends a wait on a local signal, and
  chooses each interval from the current deadline, subject to poll's
  rounding and scheduling delay, for two more foreign items.
- **A row of its own for the wake** (`eventfd`, and `poll` with it).
  Not taken: the wake serves only the wait on a route socket, and row 7
  keeps its `poll` in the row whose descriptor it waits on. A second
  user of the wake is a change to the list (point 7), which can split
  it into its own row then.
- **A Swift relay process for macOS logging.** The daemon would send each
  event over local IPC to a small Swift program that calls the logging API
  natively. Not taken: entries would be attributed to the relay, not the
  daemon; a second process must be installed, supervised and kept alive;
  and an event is lost when the relay is down, which is when it matters. It
  remains the fallback if this exception is refused, without audit events
  (O8, point 2).
- **A helper process for these needs** (a small C or Swift program per
  OS, queried over a pipe). It keeps `unsafe` out of the Rust workspace by
  moving the same calls into C that no Rust lint or test covers, adds a
  process and an IPC protocol to the trusted base, and makes a clock read a
  round trip through the scheduler, which is the delay the awake clock
  exists to measure.
- **Spreading the calls across the crates that use them.** Rejected: the
  audit surface becomes every crate, and "forbidden workspace-wide" stops
  being checkable.
- **The C library's resolver on glibc hosts** (`res_ninit`, `res_nquery`
  and `res_nclose`, as a Linux site in this crate). Rejected by C33 and
  ADR 0004, part 1, Alternatives: it sends the query where its
  configuration names and applies no routing of its own `[DES-001]`, so it
  is no better than the first-party client on a host with a local resolver;
  one client is needed for static musl builds regardless; and the resolver
  symbols moved in glibc 2.34 ("formerly in libresolv have been renamed and
  no longer have a `__` prefix. They are now available in libc", glibc
  2.34 `NEWS`; INSPECTED 2026-11-19), so it would have been the site with
  the most ABI to own.
- **The first-party DNS client on macOS and Windows too.** No site would be
  needed. Rejected by C33, point 2: on those systems only the OS resolver
  applies the system's per-interface and VPN routing, and IronAFS leaves
  that choice to the OS.
- **Synchronous `DnsQueryEx`.** No completion routine and a simpler
  proof. Not taken: the call cannot be cancelled, so a stalled resolver
  holds the calling thread for as long as Windows retries. A thread per
  outstanding query breaks C39, point 1's fixed set; one fixed discovery
  thread needs a C39 amendment and lets one unreachable resolver delay
  discovery for every cell. That one thread is the fallback of point 3.2,
  taken only if Windows runs show `DnsCancelQuery` misbehaves (owner,
  2026-11-20).
- **A mainstream crate for the random source.** Rule 21 would allow one
  as an owner decision, and C5, point 1, leaves the cryptography library
  "to be chosen". Not taken (owner, 2026-11-20): every candidate reaches
  the same operating-system calls, so it would add a dependency and move
  the `unsafe` into it without removing the need, and no cryptography
  library is chosen yet that could serve both.
- **Reading `/dev/urandom` for the Linux random source.** Safe Rust and no
  site. Not taken: it needs the device node in every sandbox the daemon
  runs in, a descriptor that can be exhausted, and it does not wait for
  the pool to be initialised early in boot, which the system call does
  with flags 0 ("When read during early boot time, /dev/urandom may
  return data prior to the entropy pool being initialized"; such
  applications "getrandom(2) must be used instead, because it will block
  until the entropy pool is initialized",
  [random(4)](https://man7.org/linux/man-pages/man4/random.4.html),
  CONFIRMED 2026-11-20).
- **`getentropy` or `arc4random_buf` for the macOS random source.** Both
  are in libSystem, so no framework link. Not taken: Apple's getentropy(2)
  caps a call at 256 bytes and says clients "who simply require random
  data" should use `arc4random(3)`, `CCRandomGenerateBytes()` or
  `SecRandomCopyBytes()` instead; `arc4random_buf` returns nothing ("These
  functions are always successful"), so it is acceptable on its own terms
  but gives the wrapper no status to check, unlike the other two random
  sites. `SecRandomCopyBytes` is the one Apple documents as generating
  "cryptographically secure random bytes", with a status to test. Its
  cost is a link to Security.framework, which ships with macOS. The owner
  chose it (2026-11-20).
- **The macOS DNS callback in the C shim.** The shim would run the query
  and copy replies into a buffer, so no foreign code calls Rust. Not taken:
  it puts the handling of untrusted lengths in C, outside Rust's lints and
  Miri, and grows the shim beyond one constant-format logging file. It is
  the fallback if a callback into Rust is refused.
- **`kDNSServiceFlagsTimeout` instead of `poll`.** One fewer foreign item.
  Not taken: the timeout "is determined by the system and cannot be
  configured by the user" (`dns_sd.h`), so it cannot meet the caller's
  deadline, and a blocking `DNSServiceProcessResult` with no answer would
  wait until then.

- **`deny` workspace-wide, and `#![forbid(unsafe_code)]` in every other
  crate root** (instead of point 1.5's copied lint table). Not taken: it
  weakens the workspace default, and a new crate that forgets the
  attribute is silently allowed `unsafe`. The copied table fails closed.
- **Waiting for `std`'s peer-credential and ancillary-data interfaces.**
  They would remove `linux-peercred` and `linux-fdpass`. Not taken: both
  are nightly-only (Context) and the workspace builds on a pinned stable
  toolchain. Their stabilisation is a revisit trigger for those rows.
- **The system's `fusermount3` helper instead of `linux-mount`.** No
  mount call of IronAFS's own. Rejected by L7, point 2: "no setuid
  helper", and it would still need the descriptor hand-off.
- **Asking the system for the page size** (`sysconf`) and allocating
  secret buffers to it. Not taken: it adds a foreign item and an unsafe
  allocation; a 64 KiB-aligned type in safe Rust covers every page size
  the Linux targets use, at 64 KiB per buffer (accepted by the owner,
  owner, 2026-11-20).
- **`getrlimit` as a read-back of the core limit.** One more foreign item
  on both systems. Not taken: O26, point 2, asks for verification of the
  non-dumpable setting, which `PR_GET_DUMPABLE` gives; a zero return from
  `setrlimit` and the marker test cover the limit.
- **Blocking pipe I/O on Windows, one thread per connection.** A pipe
  without `FILE_FLAG_OVERLAPPED`, read and written through `std::fs::File`
  on the adopted handle, needs fewer sites. Not taken (owner,
  2026-11-20): a thread per connection breaks C39, point 1's fixed set,
  and a blocked read still needs a cancel call to close a connection.
- **`windows-self-sid`** (the daemon reads its own token to learn the
  SID for the pipe's descriptor). Not added: packaging pins the service
  SID (owner, 2026-11-20), so it is a constant.
- **A watchdog for the macOS stalled DNS reply.** Not taken: the owner
  accepted the time-to-readability deadline (owner, 2026-11-20); the
  stalled-reply test and point 3's trigger stand in for it.
- **Accepting any successful `NTSTATUS` from `BCryptGenRandom`**
  (`NT_SUCCESS`). Not taken: only `STATUS_SUCCESS` is documented as
  filling the buffer, and the random source fails closed.
- **The `zeroize` crate, or a safe overwrite followed by
  `core::hint::black_box`, instead of `mem-wipe`.** Not taken (owner,
  2026-11-20, the cryptography decision): `zeroize` would be this crate's
  first dependency (point 1.4) for about as many `unsafe` lines as row
  24 holds; `black_box` is documented as a hint with no guarantee, so it
  cannot carry O28, point 2 (Firm). A safe wipe type injected into the
  pure crates keeps them free of `unsafe` and of this crate (C4).


## Review

The full record, with every round and the findings of the final rounds,
is [docs/reviews/2026-11-21-adr-0005-review.md](../reviews/2026-11-21-adr-0005-review.md).
The ADR was drafted and fixed before it was published, so rounds 1 to
14 each reviewed a frozen copy identified by its SHA-256, not a commit.
Rounds 15 to 17 reviewed commits on the pull request's branch.

- **Seats.** GPT (`gpt-6.2-astra`, xhigh) for design, the strongest-tier
  seat; GLM (`glm-5.3`, max) for security. Both are families that wrote
  none of the text. Earlier rounds used GPT at xhigh through the same
  lane.
- **Rounds 1 to 6** (GPT): the first design review (2 blockers, 7
  majors, 1 minor), two verifications, the control pipe's transition
  aligned with the control IPC design, the cryptography decision's diff
  (`mem-wipe`, 2 majors) and a cross-document check (1 blocker here, the
  wipe's callers). Every finding was fixed, or deferred to a gate this
  ADR names.
- **Round 7** (GPT, the whole ADR): FAIL, 1 blocker and 8 majors, among
  them the descriptor sender's check of its recipient, the ABI check of
  prototypes and the inventory of staged rows. All fixed.
- **Rounds 8 to 10** (GPT) verified each fix: 5 majors, then 2, then
  PASS.
- **Round 11** (GLM, security, the whole normative text): PASS, with 1
  major (macOS DNS completion could rank a partial SRV set) and 3 minors.
  The fix makes the macOS DNS site fail closed: it returns no record set
  until a completeness signal is qualified (point 3.3).
- **Rounds 12 to 14** (both seats) verified only each diff. GLM passed
  each. GPT found 2 majors, then 1, then 3 minors only; the minors were
  fixed and recorded, and the rounds stopped under the stop rule.
- **Round 15** (GPT and GLM, scoped to the text new in this pull
  request: the GPLv3 section, point 1.5, the rule 5 and rule 20
  consequences, this section, the review record and the ADR 0004
  amendment): FAIL, 4 majors and 5 minors (GPT), 2 majors and 4
  minors (GLM), mostly the same. The GPLv3 section now quotes the
  definitions exactly and assesses (a) and each route of (b) per
  component, leaving every macOS and Windows reading open for the
  owner; C4 gains its trigger; the citations and the sweep are
  corrected. Two minors were deferred: the status line's pull request
  number, fixed once the pull request was opened (#101), and the record's descriptions of
  earlier findings were added in round 16's fix, except round 1's
  minor, which the record leaves undescribed.
- **Round 16** (GPT `gpt-6.2-sol` xhigh and GLM `glm-5.3` high, the
  scoped verification of `434e573..aef2fac`): GLM PASS with 1 minor;
  GPT FAIL, 1 major and 4 minors (GLM's minor among them). The major:
  the Windows rows named no candidate Major Component; each now names
  one, the Windows kernel with which the DLL ships, as a candidate
  only, with (a) still OPEN. The minors: the
  libSystem row's call names, the ADR 0004 sweep's read-through and
  rule 5, the review record's summaries, and the description of the
  amendment's review above. All fixed in `2f0660f`.
- **Round 17** (GPT `gpt-6.2-sol` xhigh, the scoped verification of
  `aef2fac..2f0660f`): PASS with 3 minors (the Windows candidate's
  wording, round 15's sweep entry in the record, and round 1's
  undescribed minor), fixed in the next commit and recorded; under the
  stop rule, no further round.
- **Amendment of 2026-11-21 (#57; rows 13 and 14, sheets 13 and 14).**
  The class S review of #57's parts B and C (GPT, two rounds; then a
  second-family pass, Gemini and DeepSeek) found two exposures that the
  closed list did not allow closing: a datagram socket of a pair,
  reconnected and wrapped as a stream, passed sheet 13's checks with the
  pair's stale credentials; and a pidfd installed with `SCM_PIDFD` was
  refused without adoption, so it stayed open. It also recorded that
  sheet 14's raw-descriptor span crosses two functions. The owner
  accepted the new constants `SO_TYPE`, `SOCK_STREAM` and `SCM_PIDFD`
  under point 7.1 (2026-11-21), with `SCM_PIDFD` qualified per target
  (sheet 14, *Constants, per target*). No foreign function, library,
  target or flag is added; row 13 gains a second block. Sweep: ADR 0004
  C43, point 4, and L7, point 1, cite these sheets for the checks only,
  and stand. The amendment is reviewed with #57's pull request, on the
  strongest row; its rounds are in that change's review record,
  `docs/reviews/2026-11-21-m2a-57-os-parts-bc.md`.
- **Amendment, 2026-11-22 (#152): point 5.3 and Decision 4, the
  target gate.** Design issue #151.
  - What changed: point 5.3's refusal has two parts. A target gate in
    the crate's build script refuses, by exact triple, every target
    outside the list that Cargo selects by triple, the targets whose cfg
    set equals a listed one's included; the `compile_error!` guard
    stays as a second net. A custom target, which Cargo accepts only
    with unstable target support enabled, is outside the guarantee.
    `cargo xtask check-os` checks both layers: the gate's list against
    a reference copy kept apart from it, the gate's acceptance of each
    listed triple, the guard's verdict against one derived from the
    list for every built-in target, and Cargo's refusal of each
    cfg-equal target in the build script. The refusal prints the
    target in an escaped, one-line form. Decision 4 now describes the
    build script's two jobs (the gate, then the macOS shim), its rerun
    rule while it holds the gate alone, and a requirement on the M4
    shim change to rerun on every effective input Cargo does not
    track. Consequences name the follow-up class S change (the script,
    the check, the crate comments, the `deny.toml` build-script
    allowlist entry); Alternatives record the residual route not
    taken. Both decisions stay Working, with new triggers.
  - Evidence: the first `ironafs-os` change (PR #118) found that on
    the pinned compiler six built-in targets have exactly the cfg set
    of a listed one, so the `compile_error!` guard cannot refuse them,
    as point 5.3 requires (open item O3 of that change's review record,
    on the round that reviewed `8183d8f`, 2026-11-21). The crate's
    comments (`crates/ironafs-os/Cargo.toml`,
    `crates/ironafs-os/src/lib.rs`) read point 1.4 as forbidding the
    build script that could. Point 1.4 forbids dependencies. Point 1.1
    keeps a build script that compiles or runs foreign code or a
    native helper out of every crate but this one, and this crate may
    have one (the shim's, Decision 4); the gate compiles and runs
    nothing, so neither point forbids it. The owner chose the gate over
    stating a residual on 2026-11-22 (decision comment on PR #118).
  - Sweep: the repository at `origin/main` (`3f73854`) was searched for
    "no build script", "point 1.4", "compile_error", "build.rs", "build
    script", "5.3" and "0005"; so were the open branches of PR #118
    (`m2a-57-os`, `ff71ace`), PR #132 (`m2a-57-osc`, `291bc50`), PR #136
    (`m2a-57-ose`, `33bc576`) and PR #143 (`m2a-97-netgen`, `e6c9eae`).
    - On `main`, point 5.3 itself is the old rule, rewritten above. No
      other file states it. The "no build script" hits are the pure
      crates' rule (ADR 0004, C4, point 1, unchanged):
      `crates/ironafs-crypto/Cargo.toml`, `crates/ironafs-time/Cargo.toml`,
      `crates/xtask/src/pure.rs`, `docs/specs/local-ipc.md` (for
      `ironafs-ipc`) and the review record
      `docs/reviews/2026-11-21-m2a-13-check-deps.md`; and
      `crates/xtask/src/bans.rs` uses the words in a check's report
      about the build scripts clippy runs, a different rule. Coding
      standards rules 5, 20 and 21, `AGENTS.md`, `docs/workflow.md`
      and the skills were read in full: none says this crate has no
      build script. The `add-dependency` skill puts each build script
      in `deny.toml`'s `[bans.build]` allowlist, which covers
      workspace crates; that entry comes with the follow-up code, as
      Consequences say. Unchanged.
    - On the four branches, each a follow-up for its pull request, not
      edited here. Each carries main's point 5.3 and Decision 4
      unchanged, which take this amendment when it is merged. Three
      carry main's ADR 0005 exactly; PR #132's branch (`291bc50`) is
      the exception: it carries its own amendment of rows and sheets
      13 and 14, which touches neither point 5.3 nor Decision 4, and
      that pull request must reconcile both amendments in the ADR's
      status line and Review section on merge. Each also carries:
      `crates/ironafs-os/Cargo.toml` ("no build script (ADR 0005,
      point 1.4)") and `crates/ironafs-os/src/lib.rs` (the cfg-equal
      targets stated as a limit, citing point 1.4), corrected by the
      follow-up change; `crates/xtask/src/os/allowlist.rs` (the same
      reading) and `crates/xtask/src/os.rs` (the guard comparison that
      accepts the cfg-equal targets as "equivalent", the matrix check
      that forbids naming them, and its messages) and their tests,
      replaced there by point 5.3's check. `crates/xtask/src/deps.rs`
      cites point 1.4 for dependencies only, unchanged.
    - Actors. The rule binds the maintainer's session, and contributors
      and their agents, who build and review the crate. CI: on the four
      branches it runs today's `check-os` (the guard and the matrix
      check); `main` does not run it; enforcing the amended check in CI
      comes with the follow-up change. Dependabot proposes dependency
      updates only and never adds a build script; the `deny.toml`
      allowlist would refuse one.
  - Review rounds, class S, from families that wrote neither the
    amendment nor the decision. The design seat was GPT at
    `gpt-6.2-sol` (xhigh), not the strongest row's `gpt-6.2-astra`,
    by the owner's standing choice to conserve GPT quota; ADR 0005's
    own rounds 16 and 17 used the same pin. The security seat was GLM
    `glm-5.3`, at high in rounds 1 to 3, and at max for the final pass
    over the whole amendment (round 4) and its verification, the
    judgement seat, and rounds 5 and 6. This departure from
    point 7.2's strongest row was accepted by the owner with the
    amendment (2026-11-22). Each finding, its disposition and the convergence
    note are in the review record,
    [docs/reviews/2026-11-21-adr-0005-review.md](../reviews/2026-11-21-adr-0005-review.md),
    section "Amendment: the target gate".
    - Round 1 on `3f73854..225145f`: GPT FAIL, 3 major and 1 minor;
      GLM FAIL, 1 major (GPT's first) and 2 minor. Fixed in `a661d2c`.
    - Round 2 on `225145f..a661d2c`: GLM PASS, and, as refutation
      seat, it accepted moving the method that isolates the gate's
      verdict to the follow-up change; GPT FAIL, 1 major (the shim's
      indirect inputs). Fixed in `26ee981` as a requirement on the M4
      shim change.
    - Round 3 on `a661d2c..26ee981`: GPT PASS and GLM PASS.
    - Round 4, the final pass over the whole amendment,
      `3f73854..26ee981`: GLM (`glm-5.3`, max) PASS with 1 minor (the
      check's reference list must be kept apart from the gate), fixed
      in `cdc8cb3` and verified by GLM at max.
    - Round 5, the check of the Review entry, the sweep and the issue
      draft, on `a9f3886`: GPT (`gpt-6.2-sol`, xhigh) FAIL, 1 blocker,
      3 major and 2 minor; GLM (`glm-5.3`, max) PASS with 1 minor.
      (An earlier version of this entry used "Round 5" for this round
      and the judgement below together; they are now listed apart.)
    - Judgement on I-GPT-A-2 (for a custom JSON target, `TARGET` holds
      the specification file's stem, not a path, and the gate cannot
      tell such a target from a listed one): the judgement seat, GLM
      (`glm-5.3`, max), ruled A, with high confidence: a defect of
      wording and scope, not a bypass possible under the pinned
      toolchain. The controller checked its premises in Cargo's source
      at commit `3acc79c0c841844675bc53a605922dbfc441e750`: `TARGET` is
      set from the target's short name
      (`src/compiler/custom_build.rs:375`), which is the triple or a
      JSON target's file stem (`src/compiler/compile_kind.rs:241-245`),
      and a `.json` target is refused without `-Zjson-target-spec`, an
      unstable flag (`compile_kind.rs:204-205`). Fixed in `8a529d9`:
      the description of `TARGET`, the basis of the no-raw-print rule,
      a scope sentence in Decision 4 and point 5.3, and a new trigger.
    - Round 6, the verification of the fixes, on `8a529d9`: GPT
      (`gpt-6.2-sol`, xhigh) FAIL, 1 major (the opening rules still
      stated an unconditional refusal) and 4 minor (unstable target
      support is also enabled by `RUSTC_BOOTSTRAP`, not only on
      nightly; PR #132's branch carries its own amendment; two on the
      issue draft); GLM (`glm-5.3`, max) PASS with 1 minor (the same
      PR #132 point). Fixed in the next commit.
    - Round 7, the verification of `8a529d9..a5d0919`: GPT
      (`gpt-6.2-sol`, xhigh) FAIL, 1 major (point 5.3's opening still
      limited the crate before the built-in-triple qualifier) and 2
      minor (`RUSTC_BOOTSTRAP=1`; the revisit trigger split into two
      conditions); GLM (`glm-5.3`, max) PASS, no findings. All wording,
      fixed in `07c08f0`.
    - Round 8, GPT's scoped check of `a5d0919..07c08f0` (`gpt-6.2-sol`,
      xhigh): all three fixed; 2 record minors, fixed in the next
      commit. Under the stop rule the rounds stop here.
- **Amendment, 2026-11-22 (#153): row 23 and sheet 23, the route
  sockets' bounded wait.** Design issue #154.
  - What changed: row 23 gains `poll` on one route socket and its
    wake, and `eventfd` for the wake. Its sends and receives pass
    `MSG_DONTWAIT`, which keeps a send from waiting for buffer space
    and a receive from waiting for a datagram; `poll`, always with a
    finite timeout, is the site's explicit readiness wait. Whether a
    send can still wait inside the kernel is UNVERIFIED and a
    qualification measurement of the row. `EINTR` from the wait, a send
    or a receive is returned to the caller, not retried. The wait
    reports readable, error and hangup separately, through a fuzzed
    validator (point 6.5). Sheet 23 states the caller's requirements:
    one wake per worker, cleared only by it, before it reads state;
    the timeout from an awake-clock deadline, capped by
    `ROUTE_WAIT_HORIZON`; attempts registered before their unlocked
    send; one terminal transition per attempt; a record applied only
    while its attempt is live, on its socket, with its sequence and in
    time; each expired attempt counted once; C37's clock failure;
    unexplained error or hangup as a failed receive; failed wake I/O
    fatal. Open items for the monitor's change: the deadline
    durations, the final group set (nexthops), and whether an
    unexplained error or hangup occurs. Point 5.2 declares `struct
    pollfd` and `nfds_t` per C library on Linux, settled by point
    5.6's check. The row, the sheet and point 5.2 stay Working, with
    new triggers.
  - Evidence: on PR #143's branch, the network-change monitor's
    threads block in a receive that returns on a datagram or a socket
    error (such as `ENOBUFS`), and nothing else ends it. A wake whose
    send fails, or whose acknowledgement is lost, can leave an
    otherwise idle worker blocked, and a drop of the handle from a
    thread other than a worker can then wait for it indefinitely, if
    no later datagram or error ends the receive (`RouteSocket::receive`
    in `crates/ironafs-os/src/linux_rtnetlink.rs` at `e6c9eae`, lines
    140-172, with `receive_message` from line 281, a single blocking
    `recvmsg`, and `retried` in `crates/ironafs-os/src/fdpass.rs`,
    lines 283-290, which retries `EINTR`: so it returns only on a
    datagram or an error) (a drop
    on a worker only asks for the stop). A lost barrier acknowledgement
    has no deadline or retry, and can leave the generation unavailable
    until another loss or a stop (open item B1 of that change's review
    record, `docs/reviews/2026-11-21-m2a-97-netgen.md` at `e6c9eae`).
    Bounding the wait needs a call row 23 did not hold. The owner chose
    `poll` with an `eventfd` wake over `SO_RCVTIMEO` on 2026-11-22,
    with the conditions sheet 23 now carries (decision comment on PR
    #143).
  - Sweep: the repository at `origin/main` (`3f73854`) was searched for
    "SO_RCVTIMEO", "EINTR", "is retried", "wake", "receive timeout",
    "blocked receive", "rtnetlink", "poll", "eventfd" and "0005"; so
    were the open branches of PR #118 (`m2a-57-os`, `ff71ace`), PR #132
    (`m2a-57-osc`, `291bc50`), PR #136 (`m2a-57-ose`, `33bc576`) and PR
    #143 (`m2a-97-netgen`, `e6c9eae`).
    - On `main`, only this ADR states row 23's rules, rewritten above.
      Within it, sheets 8 and 14 and Decision 3.3 retry `EINTR` for
      their own calls, unchanged; the other "is retried" hits are
      other rules. Unchanged.
    - On the four branches: each hit below is annotated on its pull
      request as a follow-up; how that pull request reworks its code,
      inventory, tests and CI is its own change, reviewed there. These
      branches are not edited here. Each carries main's row 23 and sheet
      23 unchanged, which take this amendment when it is merged. Three
      carry main's ADR 0005 exactly; PR #132's branch (`291bc50`) is the
      exception: it carries its own amendment of rows and sheets 13 and
      14. The two amendments touch different rows and sheets, and
      combine on merge; both edit only the status line and the Review
      section in common, which that pull request reconciles.
      - `crates/ironafs-os/sites.json`, the machine-readable copy of
        point 6.7, holds the old row 23 on all four: approved-only on
        `m2a-57-os` and `m2a-57-osc`; implemented on every Linux target
        on `m2a-57-ose` and `m2a-97-netgen`. Each must take the amended
        row's items and counts; on the last two, the row goes back to
        approved-only until the group-set amendment is accepted and the
        whole amended row is implemented (point 6.7).
      - `crates/ironafs-os/src/linux_rtnetlink.rs` and its `abi/`
        files on `m2a-57-ose` and `m2a-97-netgen`: blocking sends and
        receives with no flags, which retry `EINTR`, and no `poll` or
        `eventfd`. Rewritten to the amended sheet by the redesign.
      - Their tests (`linux_rtnetlink/tests/stand_in.rs`, `kernel.rs`)
        on the same two branches, including the stand-in test that an
        interrupted send and receive are each made again, which CI
        names (`.github/workflows/ci.yml`); replaced by the amended
        sheet's tests.
      - `crates/xtask/src/os.rs` and its inventory and ABI checks on
        all four branches: unchanged in rule, but they check the copy
        above, so they follow it.
      - On `m2a-57-ose`: `docs/reviews/2026-11-21-m2a-57-os-part-e.md`,
        the review record of row 23's first implementation under the
        old sheet, including its open item O2 (the group set without
        nexthops). A dated record, not rewritten; PR #136 notes the
        amendment in it when the branch takes it.
      - On `m2a-97-netgen` only: the monitor's specification,
        `docs/specs/network-generation.md`, section 1 (its quotation of
        sheet 23's caller obligations), 5.1, point 3 (it decides the
        nexthop group and says "the sheet's text is left as accepted"),
        5.7, point 7 (the netlink wake, and "needs no call beyond
        `linux-rtnetlink`'s"), section 6 (the sequence number 0 kept
        for the wake) and section 13, Q6; and
        `crates/ironafs-net/src/monitor.rs` and
        `crates/ironafs-netgen/src/monitor.rs` (the netlink wake).
        Rewritten by that pull request's class S redesign. For 5.1,
        point 3: under the amended sheet the group set becomes final
        only by an amendment of row 23, so that point's decision, which
        that branch's code and inventory already implement, becomes the
        proposal that amendment restates, and row 23 stays
        approved-only until it is accepted.
      - On `m2a-97-netgen`: open item B1 of
        `docs/reviews/2026-11-21-m2a-97-netgen.md`, an open blocker.
        This amendment supplies the calls B1's fix needs (a bounded
        wait, a wake that needs no netlink acknowledgement, and a
        deadline for each attempt); B1 stays open on PR #143 until its
        redesign implements them, reviewed there.
    - Files read. Every file that states the old row 23 and sheet 23
      rules was read in full, except two read as their diff from a full
      read of the same file (named below): `docs/adr/0005-ironafs-os.md` at
      `3f73854`, `ff71ace`, `33bc576` and `e6c9eae` (main's text) and at
      `291bc50`; at `33bc576`, `crates/ironafs-os/src/linux_rtnetlink.rs`
      and every file under `crates/ironafs-os/src/linux_rtnetlink/`,
      `crates/ironafs-os/sites.json`, `.github/workflows/ci.yml` and
      `docs/reviews/2026-11-21-m2a-57-os-part-e.md`; at `e6c9eae`, the
      four `linux_rtnetlink` files that differ from `33bc576`,
      `docs/specs/network-generation.md`,
      `docs/reviews/2026-11-21-m2a-97-netgen.md`,
      `crates/ironafs-net/src/monitor.rs` and
      `crates/ironafs-netgen/src/monitor.rs`. Read at their diff from
      `33bc576`'s full read: `sites.json` and `ci.yml` at `e6c9eae`.
      Read in full at their own pins, since each is a separate copy of
      the inventory: `crates/ironafs-os/sites.json` at `ff71ace` and at
      `291bc50`; in each, row 23 appears once, as an approved-only entry
      with the old text and no code.
    - Actors. The rule binds the maintainer's session, and contributors
      and their agents, who write and review the monitor and the site.
      CI: on the four branches it runs `check-os` (the inventory
      against the copy above) and the ABI check, and, where the row is
      implemented, the named site tests, all against the old row;
      `main` runs none of them; the amended row is enforced in CI when the
      redesign lands. Dependabot proposes dependency updates only and
      does not act on the row.
  - Review rounds, class S, from families that wrote neither the
    amendment nor the decision. The design seat was GPT at
    `gpt-6.2-sol` (xhigh), not the strongest row's `gpt-6.2-astra`,
    by the owner's standing choice to conserve GPT quota; ADR 0005's
    own rounds 16 and 17 used the same pin. The security seat was GLM
    `glm-5.3`, at high in rounds 1 to 3, and at max for the judgement
    seat, the final pass over the whole amendment (round 4), rounds 5
    and 6, and the refutation seat after round 6. This departure
    from point 7.2's strongest row was accepted by the owner with
    the amendment (2026-11-22). Each finding, its disposition and the
    convergence note are in the review record,
    [docs/reviews/2026-11-21-adr-0005-review.md](../reviews/2026-11-21-adr-0005-review.md),
    section "Amendment: the route sockets' bounded wait".
    - Round 1 on `3f73854..dca6340`: GPT FAIL, 1 blocker, 8 major and
      1 minor (an earlier version of this entry said 7 major, a
      miscount corrected here; the round's output lists 8); GLM FAIL,
      1 major and 3 minor. Fixed in `e74a38c`; four
      points were moved to open items or citations of the monitor's
      specification (M1 to M4).
    - Round 2 on `dca6340..e74a38c`: GLM PASS with 2 minor and, as
      refutation seat, accepted M1 to M4; GPT FAIL, 3 major and 2
      minor. Fixed in `86cc6f6`.
    - Round 3 on `e74a38c..86cc6f6`: GLM PASS; GPT FAIL, 1 major
      (`POLLERR` followed by an interrupted receive). The judgement
      seat, GLM (`glm-5.3`, max), ruled that the finding does not hold
      and that the asked fix would replace healthy sockets on every
      signal. What its sources support: `MSG_DONTWAIT` stops the
      receive from waiting for a datagram; a pending socket error is
      cleared when a receive returns it or when `SO_ERROR` is read, and
      this site never reads `SO_ERROR`. That on a route socket an
      interrupted receive after `POLLERR` leaves the error pending, so
      that the next wait reports it again, is UNVERIFIED by poll(2),
      recv(2) and socket(7); the judgement seat reasoned it from the
      kernel's source. The finding is closed as ruled.
    - Round 4, the final pass over the whole amendment,
      `3f73854..86cc6f6`, GLM (`glm-5.3`, max): PASS, no findings.
    - Round 5, the check of the Review entry, the sweep and the issue
      draft, on `dd054a0`: GPT (`gpt-6.2-sol`, xhigh) FAIL, 1 blocker,
      7 major and 1 minor; GLM (`glm-5.3`, max) FAIL, 1 major and 2
      minor. Fixes attempted in `5b6bacd`; each finding's closure is in
      the review record's table.
    - Round 6, the verification, on `5b6bacd`: GPT (`gpt-6.2-sol`,
      xhigh) FAIL, 3 major and 4 minor; GLM (`glm-5.3`, max) PASS with
      3 minor. A refutation seat, GLM (`glm-5.3`, max), ruled on two of
      the controller's moves: the full-file read-through, PR #132's own
      amendment, PR #136's review record, the monitor specification's
      5.1, point 3, and B1 belong in this record (the move failed;
      added above); how each other pull request reworks its code,
      inventory, tests and CI is that pull request's own change (the
      move stands). Fixes attempted in `d0b358d`; each finding's
      closure is in the review record's table.
    - Round 7, the verification of `5b6bacd..d0b358d`: GPT
      (`gpt-6.2-sol`, xhigh) FAIL, 1 major (two copies of `sites.json`
      were recorded as read only at their row 23 entry) and 2 minor
      (the Evidence paragraph's unconditional wording and missing
      wrapper citation; "fixed in" lines that the table contradicts);
      GLM (`glm-5.3`, max) PASS with 1 minor (the same read-scope
      point, and the bullet's opening claimed every file was read in
      full). The two files were then read in full, the opening says
      which two were read as diffs, and the wording was fixed in the
      next commit. GPT's scoped check of that commit
      (`gpt-6.2-sol`, xhigh) found its findings fixed but for 3 record
      minors (a missing citation of the shared `retried` helper, one
      origin, one overbroad sentence), fixed in the commit after it;
      under the stop rule the rounds stop there.
- **Amendment, 2026-11-24 (#146): sheet 14's ABI evidence.**
  The owner accepted the exact sheet text before implementation, on
  base `a234490` (patch SHA-256 prefix `a9978111da93`). Each implemented
  GNU target requires complete ABI proof. On the same two GNU triples,
  the glibc 2.28 jobs add native descriptor-passing tests and partial
  ABI evidence when their headers omit `SCM_PIDFD`. Each musl lane
  proves that its headers omit the name before reporting the row
  withheld. The working amendment retains points 5.5 to 5.7 and the
  row's target statuses.
  GLM 5.4 max and Gemini Carbon High passed the final design
  verification. DeepSeek Flash max resolved the four remaining
  non-blocking reports. The evidence, supersession sweep, and review
  rounds are in the [#146 review record](../reviews/2026-11-24-146-os-guards.md).
- **Amendment, 2026-11-25 (#187): point 2, Linux ABI headers.**
  The owner accepted the exact amendment before implementation (patch
  SHA-256 prefix `2dde31ef2d3d` on `cd5aef9`). All 24 rows state their
  ordered Linux ABI header list, and the inventory checks the mirror
  before the Linux macro scan, assertions or qualified absence probe.
  The [#159 review record](../reviews/2026-11-25-159-abi-headers.md)
  records the design and code findings, native controls, and the
  supersession sweep from the former rule recorded in the #118 review.
