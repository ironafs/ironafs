<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: refute check of the driverless-clients research note

A refute-first check of
[2026-11-18-driverless-afs-clients](../research/2026-11-18-driverless-afs-clients.md),
before the architecture decision is written from it. The note was written
by a Claude model. Each seat was given the note and asked to prove every
factual claim wrong against the strongest primary source it could find, to
give a verdict (CONFIRMED, AMEND, REFUTED or UNVERIFIED) with a source and
a short quotation, and to list important omissions. The seats ran under the
don't-copy rule: no OpenAFS, AuriStor or Linux kernel AFS source was opened.

The note was split into 75 claims. The controller merged the three seats'
verdicts into one matrix, claim by claim, adding no verdict of its own.

## Seats

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex (web search) | gpt-6.2-sol | high | 32 CONFIRMED, 35 AMEND, 8 UNVERIFIED |
| glm (web) | glm-5.3 | high | 42 CONFIRMED, 5 AMEND, 12 UNVERIFIED, 16 not addressed |
| claude-code (web; extra, same family as the author) | claude-opus-5-5 | default (not reported) | 31 CONFIRMED, 19 AMEND, 1 REFUTED, 10 UNVERIFIED, 14 not addressed |
| deepseek (web; adjudicator of the conflicts) | deepseek-flash | high | 3 CONFIRMED, 8 AMEND, of 11 |

The codex and Claude seats also read the macOS 27.2 SDK headers and the
macOS man pages on the development machine. The Claude seat does not count
as a non-author review.

Across the three checking seats the 75 claims fell into:

- **22 agreed:** two or more seats reached the same verdict;
- **32 disagreed:** 21 on the label only (the seats accepted the same facts
  and differed on whether a remark made the claim AMEND), and 11 on the
  facts;
- **15 single:** only one seat reached a verdict;
- **6 unverified:** no seat reached a verdict.

## Rules applied

- An adjudicated conflict takes the adjudicator's verdict, checked against
  the source it quotes.
- An agreed claim takes the agreed correction. Each agreeing seat quoted
  its source, but the controller did not re-check every agreed claim at its
  source. That is a limit of this round: every claim the architecture ADR
  relies on is re-checked at its source when the ADR is written.
- A single-seat claim takes that seat's verdict, and the note says "one
  family".
- A label-only disagreement was decided on the claim's text, not by votes:
  AMEND where the text as written was inaccurate, CONFIRMED where it stood
  and only a caveat was added.
- An unverified claim stays NOT VERIFIED.

## The 11 adjudicated conflicts

| Claim | Verdict | Settled |
|---|---|---|
| All three platforms offer a user-space filesystem interface | AMEND | No kernel code of IronAFS's own, but Cloud Files is a sync-engine API over Microsoft's NTFS-only `cldflt.sys`, not a filesystem interface |
| FSKit at `/afs` unconfirmed | AMEND | Apple documents `mount(8)` to an arbitrary directory for a URL-backed module; `mountSingleVolume` uses `/Volumes/`; only `synthetic.conf` `/afs` is untested. GLM's "documented against" refuted |
| macOS NFS per-user access cache | CONFIRMED | `mount_nfs(8)` documents an `access(2)` cache for three users by default |
| "One report" of no cross-uid cache isolation in FSKit | AMEND | An unevidenced maintainer assertion whose cited issue does not discuss it; an untested hypothesis |
| macFUSE documents an FSKit data-corruption workaround | CONFIRMED | True in 5.5.0, not the cited 5.4.0, whose item fixes macFUSE's own bug; the citation was fixed. Claude's REFUTED held only against 5.4.0 |
| OpenAFS for Windows "mode" with whole-file server locks | AMEND | The 1.5+ CIFS-AFS gateway's behaviour, not a mode; `EnableServerLocks` only stops server locks. GPT's "historic" and "URL does not load" refuted |
| `fuser` is pure Rust and speaks `/dev/fuse` directly | AMEND | Its own protocol I/O, but libfuse mounts by default; pure Rust only without the `libfuse` feature. GLM's "libfuse required" refuted (superseded 2026-11-19: 0.18.0's manifest has `default = []`, so libfuse is opt-in) |
| Writeback: no caller identity, maybe another file handle | CONFIRMED | Both halves are in libfuse's `fuse_file_info` documentation |
| 2005: FUSE passed uid/gid, not the group list, where PAGs lived | AMEND | The FUSE half is exact; the thread does not mention PAGs |
| Rust bindings exist | AMEND | All exist, but `objc2-fs-kit` 0.3.2 has none of the macOS 27 APIs |
| IBM announced the dual licence on 2025-02-05 | AMEND | IBM's letter is dated 2025-01-21; 2025-02-05 is the project's date for the announcement |

## Outcome for the 75 claims

| Final verdict | Claims |
|---|---|
| CONFIRMED | 39 (several with an added caveat or a corrected citation) |
| AMEND | 30, corrected in place |
| REFUTED | 0 |
| NOT VERIFIED | 6: the method claim, older systems keeping the kernel module, `libuafs`'s lock and the line counts (from source; both removed from the note on 2026-11-19, ADR 0001), the causes of FUSE's low adoption, and Rust's costs |

No claim was refuted outright. One citation was (macFUSE 5.4.0), and one
"from source" claim (OpenAFS Unix clients and byte-range locks) was
replaced by documented statements.

## What changed in the note

- **Evidence labels** on every claim (INSPECTED, RELAYED, NOT VERIFIED, and
  "one family"), with the best primary URL beside it.
- **§1:** FSKit is a user-space alternative, not a replacement; FUSE
  passthrough needs privilege and io_uring does not carry every request;
  Cloud Files runs on Microsoft's NTFS-only minifilter and is not a
  filesystem interface.
- **§2:** File Provider domains can live on external volumes; the `/afs`
  question for FSKit is narrowed to `synthetic.conf` plus `mount(8)`; the NFS
  row says what AUTH_SYS, the access cache, locks and delegations are; the
  FSKit gaps say what existed before macOS 27, `FSContext` gives uids and
  gids only, and the name-cache purge is documented; the cross-uid "report"
  is an untested hypothesis; the macFUSE workaround cites 5.5.0; the
  `fskitd` leak is one report on macOS 27.0.1; the lock paragraph rests on
  documentation instead of source.
- **§3:** `fuser` mounts through libfuse only when built with its opt-in `libfuse` feature (0.18.0's manifest: `default = []`, checked 2026-11-19); Rust's
  floors are per target; FUSE's pid is a thread id and it sends no group
  list.
- **§4:** dates and packaging made current; the PAG link and Arla described
  as the sources support.
- **§6:** safe Rust only; one workspace, per-target SDKs; the FSKit binding
  lacks the macOS 27 APIs.
- **§7:** OpenAFS's tree has mixed licences; IBM's letter and its date; Rx
  and kernel per-file licences not uniform; incompatibility stated for
  GPLv2-only and IPL-1.0 code.
- **§8:** rxgk's latest draft is -11, AFS3 experimental; the AF_RXRPC
  documentation has no packet layout.
- **New §9:** the omissions the seats found, for Windows, FSKit, localhost
  NFS, FUSE and the protocol (rxkad, the AFS client baseline).
- **Sources** fixed (macFUSE 5.5.0, rxgk -11, the rxgk-afs -08 HTML link)
  and grouped.
- **New note:**
  [2026-11-19-windows-cloud-files](../research/2026-11-19-windows-cloud-files.md),
  which takes the Windows omissions further with Microsoft's documentation
  and measurements from the owner's earlier Cloud Files work.

## Still open

- Whether a `synthetic.conf` `/afs` works as an FSKit mount target.
- Whether IBM published anything on 2025-02-05.
- Whether the macOS `access(2)` cache's three-user capacity and the 60-second
  lifetime in `nfs.conf(5)` describe the same cache.
- The corrections were written by Claude Code (`claude-opus-5-5`), the
  note's own family. (Since reviewed by GLM: see "Review of the corrected
  notes" below.)

## Second round: macOS options with only what Apple ships

The new note
[2026-11-19-macos-options](../research/2026-11-19-macos-options.md) was
written from one family's research and refute-checked by a second.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex (web search; research) | gpt-6.2-sol | high | The report: FSKit by version, File Provider, loopback NFS, SMB, WebDAV, a comparison and a recommendation per macOS version |
| glm (web; refute check) | glm-5.3 | high | No claim REFUTED; 6 AMEND; about a dozen minor items UNVERIFIED; agrees with the recommendation |

The checking seat could not run commands, so it read Apple's documentation
and Apple's published source instead of the macOS 27.2 SDK headers and
shipped man pages. Its amendments, and what was done with each:

- **The `access(2)` cache's three-user default** was said to be
  unsupported. Refuted: `mount_nfs(8)` documents `accesscache=` with "The
  default cache size is 3", both in Apple's published man page and in the
  one shipped with macOS 27.2 (INSPECTED by the controller). The claim
  stands, which also agrees with the adjudicated verdict of the first
  round.
- **`mountSingleVolume`'s entitlement and `/Volumes/`:** true, but the cited
  forum thread does not say it; the note cites Apple's documentation page.
- **`enableOpenUnlinkEmulation`** could not be found. It is in `FSVolume.h`
  of the 27.2 SDK, available from macOS 26.0 (INSPECTED by the controller).
- **Unix-domain sockets:** in Apple's `mount_nfs` source but not its man
  page; the note says so.
- **Access-cache timeout of zero:** entries are never fresh rather than
  skipped, and soft mounts can still use cached results; the note says so.
- **Line-number anchors** in links to Apple's source did not match; the
  note links the files without them.

Its new finding is in the note: while Apple's NFSv4 client holds a write
delegation, it grants locks locally and sends no `LOCK` to the server. The
checking seat added that case to the acceptance tests.

Two more items it could not reach were read by the controller: the
directory name-cache purge in `FSVolumeDataCacheHandler.h`, and the `-F`
option in `mount(8)` (both INSPECTED, macOS 27.2). Its other UNVERIFIED
items are NOT VERIFIED in the note, among them File Provider's eviction
failures and the 26.4 MDM sync controls, SMB leases and `F_POSIX` locks,
the WebDAV size limit, the macOS 15 NFS release tag, and the other Apple
pieces besides AFP.

Both families agree on the recommendation: a loopback NFSv4 server for
macOS 15 and 26, FSKit 27 for macOS 27 with NFSv4 as the comparison path,
and File Provider as a Finder companion at most. It is marked INFERRED.

## Evidence

- `reuse lint`: compliant (EXECUTED, 2026-11-19, development machine).
- The three-user `accesscache=` default, `enableOpenUnlinkEmulation`
  (macOS 26.0), the directory name-cache purge and `mount -F`: read in
  Apple's published `mount_nfs(8)`, the macOS 27.2 SDK headers and the man
  pages shipped with macOS 27.2 (INSPECTED, 2026-11-19).

## Review of the corrected notes

The corrections and the two new notes were written by a Claude model, so a
second family reviewed them before landing (`origin/main..392d76c`).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: 1 minor |

The minor finding was fixed and checked by the controller: the macOS note
joined the three-user `access(2)` cache and the 60-second lifetime as one
fact, although whether they are the same cache is still open; they are now
stated separately.


## Review of the Windows UNC note

`docs/research/2026-11-19-windows-unc-without-a-driver.md`, its index row
and a cross-reference in the Cloud Files note were written by a Claude
model (`cd8731f..d766bb7`). The note's measurements were made for it on
disposable virtual machines; its steps are in its appendix.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | PASS: 4 minor |

All four minor findings were fixed and checked by the controller:

- the MEASURED label now follows the research index's definition
  (outside IronAFS code; evidence for design, not proof);
- route 3's side effects are listed in full: `\\127.0.0.1` fails while
  IronAFS is stopped, the hosts entry applies to every program, and the
  back-end port is free while IronAFS is stopped;
- the direct-bind variant (IronAFS's own service on `127.0.0.1:445`, no
  proxy) is listed as a candidate, NOT VERIFIED, with its cost: Windows'
  file sharing would depend on IronAFS at boot;
- the WebDAV row cites the page for its path syntax, as well as the
  deprecation.

It also asked whether the appendix's undo restores the original service
dependencies. The original was read on the 26H2 machine (SamSS and Srv2)
but not on the two x86 machines before the change, and the appendix now
says to read and restore it.

## Review by GPT, 2026-11-19

The pull request's changes against its merge base with `main`
(`41c3ab1..6a7b6f7`, the eight research and review files), reviewed
together with GLM's last three minor findings. The fix verification covered
`6a7b6f7..62bab2b`, and the second fix round's verification the range
after it (below).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex | gpt-6.2-sol | high | FAIL: 13 major, 3 minor |

Fixed, and checked by the controller:

- **Source-derived items removed.** Two items in the driverless note came
  from another implementation's source (a locking claim and line counts).
  They were marked as background, but a note is sent to review lanes, so
  both were removed (ADR 0001).
- **Route 3 stated exactly.** The measurement reached the client's first
  packet; no SMB session was established. Every summary now says so, and
  a full session is listed as still to measure.
- **The Windows options made precise:** route 1's policy is known from a
  third-party copy of Microsoft's template, not Microsoft Learn; port 445
  is the default direct-TCP port; SMB over QUIC is listed; the NFS
  exclusion is limited to the configuration measured; WebDAV's port is
  optional.
- **The reproduction appendix** now has the complete listener, the
  originals to record and a full undo.
- **Cloud Files and ProjFS:** the documented rule that sync roots must not
  overlap, the conflict between Microsoft's two descriptions of the
  partial hydration policy, and ProjFS's documented ranged requests and
  cancellation; whole-file hydration and missing cancellation are no longer
  stated as ProjFS limits.
- **macOS:** the NFS answer no longer promises the caller's uid on every
  request (cached data and saved credentials), compares NLM with NFSv4
  locks, and separates the FSKit floor from a loopback NFS floor; FSKit's
  two close contracts are distinguished, with an acceptance case for
  errors on `write`, `fsync` and `close`.
- **`fuser` 0.18.0** declares `default = []` (checked in its manifest), so
  the open question is closed.
- **Labels:** the index's MEASURED covers measurements made for a note.
- **GLM's three minors:** the platform-support reason for Windows 10's
  tier, the MEASURED definition, and the driverless note's preamble.

Not changed, with reasons:

- **The 75-claim matrix** stays a private working file, because it quotes
  the owner's private measurements. A redacted ledger is public:
  [claim ledger](2026-11-19-research-refute-check-ledger.md), with each
  seat's verdict, how they related and the source cited, and the limit
  stated above (no row re-checked at source by the controller).
- **An acceptance baseline for the platforms** (a real x86-64 EL7 kernel
  probe, interoperability for the first authenticated protocol, two-user
  cache and revocation tests, callback loss) belongs to the architecture
  ADR's test plan, not to these notes.


### Second fix round (the last under the stop rule)

The verification of `6a7b6f7..62bab2b` (same seat, pinned) found 7 majors
and 3 minors, narrower than the first round. Fixed, and checked by the
controller: the redacted [claim ledger](2026-11-19-research-refute-check-ledger.md)
and the pinned ranges above; "probe listener" and the first-packet limit in
every summary, with ADR 0004 named as the decision record; route 1's
documented and measured floors separated; WebDAV qualified by provider
order; per-machine coverage of the route 3 checks; boot order, back-end
ownership and fail-closed behaviour made spike gates, with the persistent
reservation's documented limit; the reproduction's startup trigger,
PowerShell syntax, bounded read and post-undo check; the Cloud Files
placeholder rule stated as the measured default, and the ProjFS comparison
without the shared per-write gap; the macOS answer and floors stated as
spike candidates, with the on-wire credential qualified; the durability
acceptance case recording each result; the superseded `fuser` row; and the
platform note's verdict words and third-party kernel facts.
