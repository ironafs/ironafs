<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Claim ledger: refute check of the driverless-clients note (2026-11-19)

A redacted, public form of the private merged matrix behind
[the review record](2026-11-19-research-refute-check.md). For each of the
75 claims: each checking seat's verdict, how they related (AGREED, DISAGREE,
SINGLE or UNVERIFIED, with the record's rules applied), and the primary
source the seats cited. The private matrix also quotes measurements from the
maintainer's earlier work, which stay private; the corrections themselves
are in the note, claim by claim, with their labels.

**Re-checked at source by the controller: none of these rows**, beyond the
specific items named in the review record. The architecture ADR re-checks
at its source every claim it relies on.

| # | Claim (few words) | GPT | GLM | Claude | Relation | Primary source cited |
|---|---|---|---|---|---|---|
| 1 | Method: INSPECTED, nothing executed, no prior cross-check | UNVERIFIED | — | — | UNVERIFIED | internal |
| 2 | Most FSKit facts come from forums and third-party listings, not Apple docs | AMEND | — | AMEND | AGREED | [link](https://developer.apple.com/documentation/updates/fskit?changes=_1) |
| 3 | ADR 0001 allows facts from documentation; source-derived items are background only | CONFIRMED | — | — | SINGLE | — |
| 4 | Third-party kexts need Reduced Security on Apple silicon | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://support.apple.com/guide/security/securely-extending-the-kernel-sec8e454101b/web) |
| 5 | FSKit is Apple's replacement for filesystem kexts | AMEND | CONFIRMED | — | DISAGREE | [link](https://developer.apple.com/support/kernel-extensions/) |
| 6 | FSKit from macOS 15.4 | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://developer.apple.com/documentation/macos-release-notes/macos-15_4-release-notes) |
| 7 | URL resources from macOS 26 | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://developer.apple.com/documentation/fskit/fsgenericurlresource) |
| 8 | Handler APIs in macOS 27 | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://developer.apple.com/documentation/fskit/fsvolume/handler) |
| 9 | GPL-only kernel interfaces are OpenAFS's stated reason to relicense | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://www.openafs.org/pages/relicense/relicensing-openafs.html) |
| 10 | FUSE writeback caching in Linux 3.15 | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://lkml.rescloud.iu.edu/hypermail/linux/kernel/1404.0/01763.html) |
| 11 | FUSE passthrough in 6.9 | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://docs.kernel.org/filesystems/fuse/fuse-passthrough.html) |
| 12 | FUSE io_uring transport in 6.14 | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://docs.kernel.org/6.14/filesystems/fuse-io-uring.html) |
| 13 | Cloud Files API from Windows 10 1709 | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine) |
| 14 | Cloud Files: on-demand placeholders without a driver of our own | CONFIRMED | — | CONFIRMED | AGREED | [link](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine) |
| 15 | All three offer a user-space filesystem interface, so one design avoids kernel code | AMEND | CONFIRMED | AMEND | DISAGREE | [link](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine) |
| 16 | File Provider: no `/afs`, `~/Library/CloudStorage/<provider>` only | AMEND | CONFIRMED | AMEND | DISAGREE | [link](https://developer.apple.com/documentation/fileprovider/nsfileproviderdomain/init(displayname:userinfo:volumeurl:) |
| 17 | File Provider is per-user | AMEND | — | UNVERIFIED | SINGLE | [link](https://developer.apple.com/documentation/fileprovider) |
| 18 | FSKit mount at `/afs` unconfirmed, for the spike | AMEND | — | AMEND | DISAGREE | [link](https://developer.apple.com/documentation/fskit/fsclient) |
| 19 | Localhost NFSv4 server gives `/afs` | AMEND | UNVERIFIED | — | SINGLE | — |
| 20 | macOS NFS client sends the caller uid | AMEND | UNVERIFIED | CONFIRMED | DISAGREE | [link](https://datatracker.ietf.org/doc/html/rfc5531#appendix-A) |
| 21 | macOS NFS client has a per-user access cache | AMEND | UNVERIFIED | AMEND | DISAGREE | — |
| 22 | NFSv4 locks reach the server | AMEND | UNVERIFIED | UNVERIFIED | SINGLE | [link](https://datatracker.ietf.org/doc/html/rfc7530#section-16.10) |
| 23 | macOS NFS client delegation support unverified | AMEND | UNVERIFIED | AMEND | AGREED | — |
| 24 | xattrs: `FSVolume.XattrHandler` | CONFIRMED | CONFIRMED | AMEND | DISAGREE | [link](https://developer.apple.com/documentation/fskit/fsvolume/xattrhandler) |
| 25 | Kernel access checks: `AccessCheckHandler` | AMEND | CONFIRMED | AMEND | DISAGREE | [link](https://developer.apple.com/documentation/fskit/fsvolume/accesscheckhandler) |
| 26 | Caller identity: `FSContext` uid/gid, no pid | AMEND | CONFIRMED | CONFIRMED | DISAGREE | [link](https://developer.apple.com/documentation/fskit/fscontext) |
| 27 | No PAGs (uid only) | AMEND | AMEND | — | AGREED | [link](https://developer.apple.com/documentation/fskit/fscontext) |
| 28 | `DataCacheHandler` + `setCacheState(.invalidate/.revoke)`; "reportedly" purges a directory's name cache | AMEND | CONFIRMED | CONFIRMED | DISAGREE | [link](https://developer.apple.com/documentation/fskit/fsvolume/kernelcachecoherencyaction/invalidate) |
| 29 | Inode eviction: `reclaimItem` | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://developer.apple.com/documentation/fskit/fsvolume/commonoperations/reclaimitem(_:replyhandler:) |
| 30 | No ioctl/pioctl; DTS suggests XPC or a Unix socket | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://developer.apple.com/forums/thread/831338) |
| 31 | FSKit has no byte-range or advisory locks; they stay kernel-local | AMEND | CONFIRMED | CONFIRMED | DISAGREE | [link](https://developer.apple.com/documentation/fskit/fsvolume) |
| 32 | ACLs not supported as macOS ACLs | AMEND | UNVERIFIED | CONFIRMED | DISAGREE | — |
| 33 | "One report says FSKit cannot guarantee cross-uid cache isolation" | UNVERIFIED | CONFIRMED | AMEND | DISAGREE | [link](https://github.com/vaulthalla/vaulthalla/issues/200) |
| 34 | Negative-lookup purge is an open risk | CONFIRMED | — | — | SINGLE | [link](https://developer.apple.com/documentation/fskit/fsvolume/kernelcachecoherencyaction/invalidate) |
| 35 | macFUSE release notes document a workaround for an FSKit data-corruption issue | CONFIRMED | AMEND | REFUTED | DISAGREE | [link](https://macfuse.github.io/2026/10/17/macfuse-5.5.0.html) |
| 36 | `fskitd` Mach port leak, FSKit alone, no workaround, macOS 26/27 | AMEND | AMEND | AMEND | AGREED | [link](https://github.com/macfuse/macfuse/issues/1220) |
| 37 | (source-derived claim, removed from the note; ADR 0001) | AMEND | UNVERIFIED | UNVERIFIED | SINGLE | — |
| 38 | OpenAFS documents a Windows mode using whole-file server locks | AMEND | CONFIRMED | CONFIRMED | DISAGREE | [link](https://docs.openafs.org/ReleaseNotesWindows/ch03s19.html) |
| 39 | FUSE in the kernel since 2.6.14 | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://lkml.rescloud.iu.edu/hypermail/linux/kernel/0509.1/1945.html) |
| 40 | libfuse 3 since 2016 | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://github.com/libfuse/libfuse/blob/master/ChangeLog.rst) |
| 41 | `fuser` is pure Rust and speaks `/dev/fuse` directly | CONFIRMED | AMEND | AMEND | DISAGREE | [link](https://docs.rs/fuser/latest/fuser/) |
| 42 | Rust std needs kernel 3.2+, glibc 2.17+ | AMEND | CONFIRMED | AMEND | DISAGREE | [link](https://doc.rust-lang.org/rustc/platform-support.html) |
| 43 | Roughly the RHEL 7 floor | AMEND | CONFIRMED | UNVERIFIED | DISAGREE | [link](https://docs.redhat.com/en/documentation/red_hat_enterprise_linux/7/html/7.0_release_notes/chap-red_hat_enterprise_linux-7.0_release_notes-kernel) |
| 44 | Older systems keep the OpenAFS kernel module | UNVERIFIED | — | — | UNVERIFIED | — |
| 45 | FUSE passes uid, gid, pid; per-process tokens possible | AMEND | CONFIRMED | AMEND | DISAGREE | [link](https://libfuse.github.io/doxygen/structfuse__ctx.html) |
| 46 | Writeback: delayed writes lack caller identity, maybe on another handle | CONFIRMED | other | CONFIRMED | DISAGREE | [link](https://libfuse.github.io/doxygen/structfuse__file__info.html) |
| 47 | `afsd.fuse` in OpenAFS 1.5.74, 2010 | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://docs.openafs.org/Reference/8/afsd.html) |
| 48 | `afsd.fuse` runs over `libuafs` | CONFIRMED | UNVERIFIED | UNVERIFIED | SINGLE | [link](https://workshop.openafs.org/afsbpw10/talks/thu_2/201005_adeason_libuafs.pdf) |
| 49 | Experimental; "once" packaged read-only, no init scripts | CONFIRMED | CONFIRMED | AMEND | DISAGREE | [link](https://packages.debian.org/sid/openafs-fuse) |
| 50 | (source-derived claim, removed from the note; ADR 0001) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED | — |
| 51 | 2005: FUSE passed uid/gid, not the group list, where Linux PAGs lived | AMEND | CONFIRMED | AMEND | DISAGREE | [link](https://linux-kernel.vger.kernel.narkive.com/GytY698E/using-fuse-for-afs-dfs-was-re-openafs-devel-openafs-opendfs-collaboration) |
| 52 | 2007 openafs-info poster noted Arla as an existing FUSE-style client | UNVERIFIED | CONFIRMED | CONFIRMED | AGREED | [link](https://openafs-info.openafs.narkive.com/E8NU7V5g/using-fuse-for-openafs) |
| 53 | Arla: independent BSD user-space AFS client with nnpfs module | CONFIRMED | — | — | SINGLE | [link](https://www.stacken.kth.se/project/arla/html/arla.html) |
| 54 | MacFUSE was itself a kext | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://macfuse.github.io/2026/09/07/macfuse-5.4.0.html) |
| 55 | Why FUSE never caught on (slow; no help on macOS/Windows; module worked) | UNVERIFIED | — | — | UNVERIFIED | — |
| 56 | (source-derived claim, removed from the note; ADR 0001) | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED | — |
| 57 | Rust gives memory safety and data-race checking | CONFIRMED | — | — | SINGLE | [link](https://doc.rust-lang.org/nomicon/races.html) |
| 58 | One cargo build for all three OSes | AMEND | — | — | SINGLE | [link](https://doc.rust-lang.org/cargo/faq.html) |
| 59 | Bindings exist: windows-rs CloudFilters, objc2-fs-kit, fuser, tokio, RustCrypto | CONFIRMED | CONFIRMED | AMEND | DISAGREE | [link](https://docs.rs/crate/objc2-fs-kit/0.3.2/features) |
| 60 | Rust costs: learning curve, less mature Kerberos/GSS, fewer AFS developers | UNVERIFIED | UNVERIFIED | UNVERIFIED | UNVERIFIED | — |
| 61 | FFI to C Rx no longer a cost; ADR 0001 has Rx in Rust | CONFIRMED | — | — | SINGLE | internal |
| 62 | OpenAFS is IPL-1.0, moving to dual IPL-1.0 / GPLv2 | AMEND | CONFIRMED | CONFIRMED | DISAGREE | [link](https://www.openafs.org/pages/relicense/relicensing-openafs.html) |
| 63 | Sun RPC licence was GPLv2-incompatible; now 3-clause BSD | AMEND | CONFIRMED | CONFIRMED | DISAGREE | [link](https://openafs.org/pages/relicense/IBM_OpenAFS_License_Approval.pdf) |
| 64 | IBM announced 2025-02-05 that OpenAFS 1.0 is IPL/GPLv2 | AMEND | CONFIRMED | CONFIRMED | DISAGREE | [link](https://openafs.org/pages/relicense/IBM_OpenAFS_License_Approval.pdf) |
| 65 | Contributors are being contacted | AMEND | CONFIRMED | CONFIRMED | DISAGREE | [link](https://www.openafs.org/pages/relicense/relicensing-openafs.html) |
| 66 | Note's reading: much of the tree likely still IPL-only | AMEND | — | — | SINGLE | [link](https://www.openafs.org/pages/relicense/relicensing-openafs.html) |
| 67 | OpenAFS's Rx is under the same terms | AMEND | — | UNVERIFIED | SINGLE | [link](https://openafs.org/frameless/relicense/relicensing-openafs.html) |
| 68 | Kernel AF_RXRPC and kafs are separate and GPLv2 | AMEND | CONFIRMED | AMEND | DISAGREE | [link](https://docs.kernel.org/process/license-rules.html) |
| 69 | GPLv3 incompatible with GPLv2-only and IPL-1.0; closed both ways whatever relicensing does | AMEND | CONFIRMED | CONFIRMED | DISAGREE | [link](https://www.gnu.org/licenses/license-list.html) |
| 70 | AuriStor is proprietary; check terms before testing | CONFIRMED | UNVERIFIED | UNVERIFIED | SINGLE | [link](https://www.auristor.com/filesystem/auristor-comparison) |
| 71 | Rx spec draft by Zeldovich, updated by Altman; packets, sequencing, ACKs | AMEND | CONFIRMED | CONFIRMED | DISAGREE | [link](https://registrar.central.org/rx-spec.txt) |
| 72 | rxgk Internet-Drafts: expired, historical; GSS-API, key negotiation, packets | AMEND | CONFIRMED | AMEND | DISAGREE | [link](https://datatracker.ietf.org/doc/html/draft-wilkinson-afs3-rxgk-11) |
| 73 | "Integrating rxgk with AFS" is an expired draft | CONFIRMED | CONFIRMED | CONFIRMED | AGREED | [link](https://datatracker.ietf.org/doc/html/draft-wilkinson-afs3-rxgk-afs-08) |
| 74 | Kernel AF_RXRPC documentation describes the protocol | CONFIRMED | CONFIRMED | AMEND | DISAGREE | [link](https://docs.kernel.org/networking/rxrpc.html) |
| 75 | Observed behaviour: captures reduced to synthetic fixtures | CONFIRMED | — | — | SINGLE | internal |
