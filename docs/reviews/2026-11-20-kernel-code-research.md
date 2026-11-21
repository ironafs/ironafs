<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: kernel code against user space, and README corrections

## The change

Class P.

- **A new research note**,
  [2026-11-20-kernel-code-and-user-space](../research/2026-11-20-kernel-code-and-user-space.md),
  on:
  - the cost of shipping kernel code on Windows, macOS and Linux;
  - how kernel and user-space code fail;
  - what is published about FUSE and FSKit speed.

  It has 30 numbered references.
- **README corrections:**
  - ADR 0004 is accepted, where two places still called it a future step;
  - the earlier FUSE-based AFS client is described without a verdict on it.

  The driverless-clients note's section 4 heading is reworded to match.
- **Written by** `claude-code:claude-opus-5-5`.
- **The first draft** went out as draft PR #34. The owner closed it so the
  facts could be checked by another family first.

## How the check was run

The rounds were a refute-first fact check:

- every quote, number, date and evidence label was checked against its
  cited source, with live web search;
- every repository line the text cites was checked;
- each finding was confirmed at its source by the author before it was
  fixed.

Alongside round 1, the author re-read every cited source a second time;
for the papers, that meant their full text. This was not a review seat.

| Round | Range | What was read | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|---|
| 1, review | `1f44f21..addb23a` | the note, and the website answers | GPT read-only | gpt-6.2-sol | high | FAIL: 1 blocker, about 20 majors, minors |
| 2, fix verification | `addb23a..9864326` | the fixes to both | GPT read-only, same session | gpt-6.2-sol | high | FAIL: 5 majors (partial fixes), 6 minors |
| 3, fix verification | `9864326..f54c6e6` | the fixes to both | GPT read-only, same session | gpt-6.2-sol | high | PASS: 2 minors |
| 4, final-state check | `1f44f21..b67430f` | the whole change: README, indexes, heading, the note, this record | GPT read-only, same session | gpt-6.2-sol | high | FAIL: 3 majors, 4 minors |

Round 4's fixes were made in one batch after it; their verification is
recorded below when it runs.

## Findings

- **Blocker, fixed** (in text written alongside the note, for the
  project's website): counting benchmarks were described as already
  running in CI. They are planned in ADR 0004, O49, and no client exists
  yet.
- **Majors, fixed** (`9864326`, `f54c6e6`). Wording that went beyond its
  source:
  - **Windows signing:** the EV certificate is needed for the dashboard
    account, not to sign the driver, and the policy lists exceptions.
    Attestation-signed retail drivers are not published on Windows
    Update, which is not a ban on every channel.
  - **Cross-signed removal:** the April 2026 update starts an evaluation
    mode; the 250-hour threshold came with the 9 June 2026 servicing
    release.
  - **Driver Quality Initiative:** it enables a move out of the kernel,
    and is not a mandate.
  - **Driver recovery:** it applies only to drivers rejected in
    Microsoft's quality evaluation, and only when an approved replacement
    exists; its dates are a schedule.
  - **Apple's Reduced Security steps** were incomplete.
  - **Swift et al.'s 85% figure** is relayed from a private communication,
    and counts reported failures.
  - **Chou et al.'s ratio** is a Linux result.
  - **FAST 2017:** the workload behind the abstract's –83%, and the
    configurations and thread counts behind each figure.
  - **RFUSE:** which experiment each figure came from.
  - **Evidence labels:** a release summary is RELAYED; a vendor's own
    page is INSPECTED, with "vendor's figure" for unmethodical numbers.
  - **IronAFS's own settings** are stated per platform, and as planned and
    unmeasured.
  - **Several line references** in the website answers were wrong.
- **Added from the round-1 findings:** RFUSE compares a FUSE NFS client
  with Linux's in-kernel NFS client, and the authors treat the in-kernel
  client as the reference for maximum performance. With Ceph's own
  documentation ("the kernel client will always give better
  performance"), it replaces an unsourced anecdote.
- **Also major, fixed:** a sentence in the website answers turned "this
  search found no study" into a claim about the whole literature.
- **Minors, fixed:** reference titles and sections, a man-pages
  attribution, and the FUSE recovery methods' limits.
- **Round 3's two minors, fixed in the website answers** (not tracked
  here):
  - a relayed claim about macOS's name cache was cited so that its label
    seemed to cover the caller-identity evidence too;
  - the answers' summary called every finding fixed while one was open.
- **Round 4, fixed:**
  - **Major:** the README called the Cloud Files replica the Windows
    surface if the SMB gate fails, without L15's rule that, if both fail,
    Windows has no surface until one passes.
  - **Major:** this record did not say what each round read.
  - **Major:** this record did not give round 3's dispositions.
  - **Minors:**
    - the note's preface claimed it said nothing about IronAFS;
    - Apple's steps are now a summary that points to Apple's guide;
    - the Nooks result covers extensions isolated inside the kernel, not
      drivers alone;
    - the FSKit forum timing was on macOS 15.5.
- **Left as stated** (raised as uncertain, not findings):
  - Apple's canonical pages rate-limited the checker, which read indexed
    copies; the author's reads of the canonical pages are the evidence.
  - Whether Microsoft's automatic driver recovery started in September
    2026 is marked NOT VERIFIED in the note.

Not yet complete: round 4's fixes await verification.
