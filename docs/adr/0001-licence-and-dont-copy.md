<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0001: Licence and the don't-copy rule

Status: accepted (2026-11-18), by the owner, after the review rounds recorded
below.

## Context

- IronAFS is a new AFS client for existing AFS-3 cells (OpenAFS servers
  first; others, AuriStor included, once tested). It needs their wire
  behaviour, not their code.
- OpenAFS is under the IBM Public License 1.0 (IPL-1.0), moving towards a dual
  IPL-1.0 / GPLv2 licence. The Linux kernel's Rx and AFS code (AF_RXRPC,
  kafs) is GPLv2. AuriStor is proprietary. Copying or adapting any of them
  would bind IronAFS to their terms.
- The Rx protocol and its security classes are described outside any
  implementation's source:
  - the community Rx specification draft;
  - the rxgk Internet-Drafts (expired, historical);
  - other published AFS documentation.
  These documents have their own licences, checked per document; much of
  OpenAFS's own documentation is under IPL-1.0. Wire behaviour can also be observed by
  running servers and capturing traffic.
- Large language models have been trained on public code, very likely
  including OpenAFS and the Linux kernel. Keeping source files out of a prompt
  does not stop a model reproducing what it has memorised.
- **The goal for the licence:** IronAFS, and every version of it that is
  distributed, including its improvements, stays free and open source.
  - Anyone may use, study, change and share it.
  - Whoever distributes IronAFS, or a work based on it, must pass on the same
    freedoms and make its complete corresponding source available to the
    people they distribute it to.
  - So distributed improvements stay open and can flow back to IronAFS's
    users and the wider open-source community, instead of disappearing into
    closed products.
- Research: `docs/research/2026-11-18-licence-options.md`.

## Decision

### One licence: GPL-3.0-or-later

- IronAFS's own code and documentation is `GPL-3.0-or-later`. The text is in
  `LICENSE` and `LICENSES/GPL-3.0-or-later.txt`. Third-party material copied
  in keeps its own licence (below).
- **What it does:** anyone who distributes IronAFS, or a work based on it,
  must license it under the same terms and make its complete corresponding
  source available to the recipients, by one of the methods GPLv3 §6 allows
  (§§5–6). Separate programs merely shipped alongside it (an aggregate) are
  not covered (§5). It also carries an explicit
  patent grant (§11). "Or later" lets recipients use later GPL versions
  published by the FSF (§14).
- **What it does not do:** it does not require publishing changes that are
  only used privately or inside one organisation; it does not reach a modified
  copy used only to provide a hosted service (the AGPL would); and it does not
  stop anyone writing their own client from the protocol.
- Every file declares its licence and copyright: an SPDX header where the
  format has comments, and a `REUSE.toml` entry for the rest. Every licence
  used has its text in `LICENSES/`. The repository follows the REUSE
  specification. `reuse lint` must pass in CI.
- A crate made only of IronAFS's own code has `license = "GPL-3.0-or-later"`.
  A crate that contains copied third-party material declares the combined
  expression (for example `GPL-3.0-or-later AND MIT`). Each published crate includes every
  licence text and notice it needs; check with `cargo package --list` before
  publishing.

### Don't copy

**No code from OpenAFS, AuriStor, the Linux kernel's AF_RXRPC or kafs**, or
any other GPL, IPL or proprietary AFS source, enters IronAFS. That rules out:

- copying, porting or translating that code;
- generating code from their files, including OpenAFS's `.xg` interface
  definitions;
- linking their libraries, including OpenAFS's C Rx.

**Facts, not expression.** From documents, other implementations and observed
behaviour, IronAFS takes the functional facts that interoperating needs:

- message layouts, field types and order;
- opcodes, constants and error codes, with the names the protocol requires;
- state sequences and timing.

It does not copy prose, comments, code structure, error-message text or help
text. Where compatibility requires an exact output (for example `fs` output
that scripts parse), that format is a functional requirement: it is written
down as one in a spec, and matched where it must be.

### People

- **Anyone may contribute to any part of IronAFS**, including people who know
  OpenAFS, AuriStor or the kernel's AFS and Rx code well, or have worked on
  them. Knowing how a system works is not a reason to exclude anyone.
- **What everyone agrees to.** When writing IronAFS code:
  - write it from your own understanding, the specifications and IronAFS's
    own specs;
  - don't have another implementation's source open while you write it;
  - translate or port nothing.

  The pull request says so, alongside the DCO sign-off. Familiarity with
  other implementations may be mentioned, as useful context, never as a gate.
- **One exception: confidentiality duties.** Someone bound by an NDA or an
  employment duty covering *non-public* source (AuriStor's proprietary code,
  or unpublished IBM or Transarc code) does not work on the matching IronAFS
  component without their employer's written consent. This is about their
  obligations, not their knowledge.

### AI models

Models can reproduce code they were trained on, which very likely includes
OpenAFS and the Linux kernel. They get safeguards that people do not need:

- **No source in prompts.** Briefs to implementer and review lanes never
  include OpenAFS, AuriStor or kernel AFS/Rx source, and never name their
  files, functions or internal identifiers. Protocol names and constants are
  facts, and they are fine to use.
- **Specs from source, when unavoidable.** When behaviour can only be learned
  from source, a separate session reads it and writes a behaviour spec,
  which the controller places in `docs/specs/`: facts only, in IronAFS's own words, naming the sources it
  used. It is a fresh session with no shared memory or transcript, and no
  write access to the repository. A different family reviews it for copied
  expression. The session that wrote a spec does not implement it.
- **Similarity audit.** At every class S or R milestone in the protocol code
  (Rx, XDR, rxkad, rxgk, the protocol clients, the callback service), IronAFS's
  code is compared with OpenAFS and the kernel's AF_RXRPC/kafs:
  - a mechanical token-level comparison, run outside the repository;
  - a reviewer's read for copied structure, ordering, naming or comments.

  A finding blocks the milestone until the code is rewritten from the spec.
  This applies to code from people and models alike.
- **Residual risk.** These safeguards lower the risk; they cannot remove it.

### Test tools and test data

- OpenAFS binaries (for example `afsd.fuse`, or a test cell) may be run to
  observe and compare behaviour. They are installed from packages or built
  outside the IronAFS workspace, and never linked into, shipped with or
  vendored into IronAFS.
- Test fixtures are IronAFS's own: packet captures reduced to what a test
  needs, with synthetic cell names, users, addresses, tickets and keys, and
  outputs written down as functional facts. Raw captures stay out of the
  repository and out of lane briefs, and each fixture is inspected before it
  is added.
- **AuriStor**: before running AuriStor software or capturing traffic from an
  AuriStor cell, the applicable licence and the cell's terms are checked for
  limits on analysis or testing. Testing waits until that check is recorded.

### Third-party material that is copied in

Code or text incorporated from a third party must be under a licence
compatible with GPL-3.0-or-later and on the dependency allowlist below. Short
quotations used, with their source cited, to report or discuss a source in
research notes and reviews are not incorporated material and need no
provenance row. Incorporated material:

- keeps its own notices and SPDX label;
- adds its licence text to `LICENSES/`;
- gets a row in `docs/provenance.md` giving source, revision, licence and what
  was taken, whether a person or a model copied it (ADR 0002);
- is included in the release notices file (below).

Missing or unresolved provenance blocks the merge.

### Dependencies

The policy covers every Cargo dependency, dev-dependencies and build
dependencies included, for every supported target, and any C code built by a
build script.

- **Allowed** (exact SPDX ids):
  - `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `MIT`;
  - `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `0BSD`, `BSL-1.0`;
  - `Unicode-3.0`, `Unicode-DFS-2016`.

  An `OR` expression passes if one branch is allowed. Every term of an `AND`
  expression must be allowed.
- **Owner decision per package**, naming the package, version, use and
  reason:
  - `CC0-1.0` and `CDLA-Permissive-2.0`, for data only; CC0 grants no
    patent rights;
  - `MPL-2.0`, only when the package carries no "Incompatible With
    Secondary Licenses" notice (MPL-2.0 §3.3), so it can be combined into a
    GPL work, and with its source duty met in releases (below);
  - `OpenSSL` and other old BSD-style licences;
  - `Unlicense`;
  - anything else not listed.
- **Not allowed**: GPL, LGPL, AGPL and IPL code in a Cargo dependency or in
  bundled C code, and anything without a clear licence. GPL-family crates
  would be lawful under GPL-3.0-or-later; they stay excluded under coding
  standards rule 21, and GPLv2-only and IPL code is incompatible with GPLv3 in
  any case.
- **Every allowed licence is GPLv3-compatible**, so the allowed dependencies
  can be combined into IronAFS's GPL-3.0-or-later binaries.
- **Out of scope**: the operating system itself and what IronAFS talks to
  rather than links (the FUSE kernel interface and its mount helper), and
  system libraries that come with the operating system and are linked
  dynamically (the C library, FSKit, Windows APIs). They are platform, not
  dependencies. Each platform's ADR states which linked components meet
  GPLv3's "System Libraries" definition (§1); that is checked per component,
  not assumed.
- **Enforcement**:
  - `cargo-deny` checks licence metadata (`deny.toml`).
  - Crates with build scripts are allowlisted one by one
    (`allow-build-scripts` in `deny.toml`).
  - Bundled native (C or C++) sources are checked by hand, because Cargo
    metadata may describe only the Rust wrapper. Each approval names the
    crate, version and bundled licences, and is recorded in the commit that
    adds the crate.

### Releases

Each release (class R) ships:

- `LICENSE` (the GPLv3 text), and a third-party notices file generated from
  the dependency tree and from `docs/provenance.md`. It holds every licence
  text, copyright notice and Apache `NOTICE` the shipped code requires;
- for a downloadable release, equivalent access to the complete
  corresponding source for exactly that release, from the same place and at no
  further charge (GPLv3 §6(d)): in practice, a source archive published with
  the release. A written offer (§6(b)) is used only for physical media;
- for any MPL-2.0 dependency, all of its covered source, kept available under
  the MPL, with a statement of how recipients obtain it (MPL-2.0 §3.2(a)).

### Contributions

- DCO sign-off from the start, under ADR 0002. There is no CLA and no
  contributor grant at launch.
- Contributions come in under GPL-3.0-or-later, the licence of the files they
  change (inbound equals outbound).
- **Before the first outside contribution,** if app-store distribution or a
  later relicensing matters, a short contributor grant is drafted by a lawyer
  and added to `CONTRIBUTING.md`. Until then the owner is the only author and
  keeps every option.
- Contributors make the "written from my own understanding; nothing ported
  or translated" statement above, in their pull request.

### Changing the licence later

The owner can relicense their own code. Grants on versions already released
stay valid, and outside contributors' code can be relicensed only with their
consent, by rewriting it, or under a contributor grant that covers it. "Or
later" already lets IronAFS follow future GPL versions without asking anyone.

## Consequences

- The plan changes:
  - Rx is written in Rust from the start, with no C Rx through FFI;
  - interface definitions are IronAFS's own;
  - The architecture ADR is written on that basis.
- `docs/specs/` holds behaviour specs, each naming the sources it used.
- **Neither OpenAFS nor the kernel's kafs can take IronAFS code.** GPLv3 is
  incompatible with GPLv2-only code and with IPL-1.0. That route was never a
  goal, and IronAFS has no kernel code: the reason OpenAFS chose GPLv2 is its
  Linux kernel module.
- **macOS distribution:** direct, signed and notarized downloads are the
  main channel. Apple's App Store terms conflict with the GPL, so the Mac App
  Store stays possible only while the owner holds, or is licensed for, every
  line. That an IronAFS-built FSKit extension can be signed, notarized,
  installed and loaded outside the store is assumed until a separate check
  does exactly that with a minimal first-party extension (planned before the
  macOS layer is designed).
- CI runs `cargo-deny`, including the build-script allowlist, and `reuse lint`.
- The first public push grants GPL-3.0-or-later rights that a later
  relicensing cannot withdraw. The going-public conditions in ADR 0002 apply.

## Alternatives considered

- **Apache-2.0:** it lets anyone ship
  a closed, improved IronAFS, the outcome the owner wants to prevent.
- **`MIT OR Apache-2.0`** (the Rust norm, and the first draft): permissive, for
  the same reason.
- **MPL-2.0:** file-level copyleft and no App Store conflict, but a company may
  build a closed product around IronAFS's files. The alternative if App Store
  distribution ever outweighs the goal.
- **GPL-2.0-or-later:** would let GPLv2 projects (kafs, OpenAFS's GPLv2
  option) take the code, but recipients could then choose GPLv2's terms, which
  have no explicit patent grant. Not a goal, so not worth it.
- **GPL-3.0-only:** the same protection today, but every later GPL version
  would need each contributor's consent.
- **AGPL-3.0:** adds coverage of hosted use, which barely applies to a
  filesystem client, at a cost in adoption.
- **IPL-1.0 for code ported from OpenAFS** (the first draft). Rejected: no
  OpenAFS code is used at all.
- **Linking OpenAFS's C Rx through FFI** as a first step. Rejected: IPL code
  in the binary, and an `unsafe` FFI boundary on untrusted network input.
- **A strict clean room for people** (round 2's version): anyone who had read
  OpenAFS, AuriStor or kernel AFS/Rx source barred from protocol code.
  Rejected by the owner, after an independent check (round 3):
  - it excludes nearly everyone qualified to write or review this code;
  - it cannot claim "no access" when the models were trained on that code;
  - comparable projects with open-source references don't do it.

## Review

**Round 1**, 2026-11-18, on the first draft (`MIT OR Apache-2.0`, IPL-1.0 for
derived files).

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex (GPT) | gpt-6.2-sol | xhigh | FAIL: 1 blocker, 6 major, 1 minor |

Most findings concerned IPL-1.0 obligations: release terms, patent and
commercial clauses, mixed binaries, mixed crates, and BSD-licensed files
inside OpenAFS. **The owner's decision to use no OpenAFS code removes all of
them.** Findings that still apply were kept:

- **Dependency policy:** exact SPDX ids enforced by `cargo-deny`.
- **Labels:** REUSE and `reuse lint`.
- **Contributions:** DCO from the start, and the limits on relicensing.
- **Copied material:** provenance as a merge blocker.
- **README:** factual trademark wording.

**Round 2**, 2026-11-18, on the rewrite (Apache-2.0 only, clean room).
Second family; the artefact was given in full, with no tools.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| gemini | gemini-4-carbon-high | high (in the id) | FAIL: 3 blockers, 5 major, 1 minor |

All accepted and folded in.

- **Blocker: the source-side step was leaky.** The spec reviewer must see the
  source, so it is source-side too. Source-side sessions never write or review
  IronAFS code, and they run outside the repository. The rule is per session,
  matching ADR 0002.
- **Blocker: memorised code.**
  - Specs name no source identifiers, and the source mapping lives in an
    off-repository log.
  - Briefs name no source identifiers.
  - A source-side similarity audit runs at protocol milestones.
  - The residual risk is stated.
- **Blocker: people.** The owner and the controller stay on the clean side.
  Outside contributors declare source exposure. How such contributors take
  part is left to the owner.
- **Major: documents leak too.** Only functional facts are taken; no prose,
  error strings, help text or naming. OpenAFS's documentation is IPL-1.0.
- **Major: AuriStor terms, dev-dependencies, fixtures and test builds.** Now a
  terms check before AuriStor testing, every dependency kind in scope,
  fixtures as functional facts, and test tools built outside the workspace.
- **Major: MPL-2.0 source offer.** Now a release duty, and MPL-2.0 is an
  owner decision per package.
- **Major: allowlist gaps.**
  - Added `Apache-2.0 WITH LLVM-exception`.
  - CC0 and CDLA are data-only exceptions; OpenSSL-style licences and
    Unlicense are owner decisions.
  - System libraries linked dynamically are out of scope.
  - Build scripts are allowlisted, with bundled C checked by hand.
- **Major: copied third-party material.** It now goes into `LICENSES/`, the
  Cargo licence expression and the release notices file. "All IronAFS code
  is Apache-2.0" became "IronAFS's own code".
- **Minor: the draft's claim was wrong.** The draft said Apache-2.0-only does
  not affect OpenAFS under IPL-1.0. IPL-1.0 cannot absorb it either. Corrected
  here and in the research note.

**Round 3**, 2026-11-18. The owner questioned round 2's rule barring people
who know OpenAFS or the kernel from protocol code. An independent Claude seat
checked the policy, given the files and the question but not the
controller's view.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| claude-code (subagent) | claude-mythos-5-2 | default | the people rule is disproportionate; keep the copying, facts and AI safeguards |

Basis given by the reviewing seat (its sources are in its report):

- Copyright protects expression, not ideas, facts or what compatibility
  dictates.
- A clean room is evidence against access, not a legal requirement.
- The models were trained on the code anyway, so excluding experts buys no
  "no access" defence and loses the best reviewers.
- Asahi Linux, Samba and kafs rely on "don't copy". Strict exclusion is used
  where the source is proprietary or leaked (ReactOS, Wine).

Owner decision: no one is excluded for knowing or having worked on other
implementations. Changes:

- The people rule became "write from your own understanding; nothing ported
  or translated", plus one exception for confidentiality duties over
  non-public source.
- The source-side machinery shrank: a spec-from-source session, reviewed by
  another family; no off-repository log; no human wall.
- The similarity audit gained a mechanical comparison and now covers human and
  model code alike.

Not legal advice. A lawyer's view would matter for exact contributor wording,
for the AuriStor exception, and for patents, which no clean room covers.

**Round 4**, 2026-11-18, verification by a non-author family. It ran on the
text from just before round 3's rewrite.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| codex (GPT) | gpt-6.2-sol | xhigh | FAIL: 3 blockers, 13 major, 5 minor (across both ADRs and the tree) |

For this ADR:

- **Moot after round 3:** the contradictions around source-side audits and
  the eligibility of people with source exposure. That text no longer exists.
- **Fixed:**
  - the spec session is a fresh session, with no shared memory, transcript or
    repository write access;
  - research-note references to OpenAFS file and function names were
    removed, and a mailing-list quotation paraphrased;
  - fixtures use synthetic identities, and raw captures stay out of the
    repository and the briefs;
  - `deny.toml` checks dev-dependencies (`include-dev`);
  - the `add-dependency` skill inspects build-script and proc-macro targets
    and bundled C, and defers to this ADR for licences;
  - OpenAFS documentation licences are checked per document;
  - the IPL claim is qualified for separate, non-derivative modules.

**Round 5**, 2026-11-18: final verification of both ADRs and the whole tree.

| Seat | Served model | Effort | Verdict |
|---|---|---|---|
| glm | glm-5.3 | high | FAIL: 1 blocker, 8 minor (shared across both ADRs) |

It confirmed every fix from the earlier rounds is present. All findings are
fixed:

- **Blocker:** git's index still listed the deleted dual-licence files. They
  are now removed from the index. `land-change` also builds the root commit
  from an empty index and an explicit file list.
- **Minor:**
  - stale status lines and ADR index rows;
  - the class I seat now defers to the seat table;
  - a seat row for the owner's own unaided changes;
  - the billing wording, which now notes that one lane is usage-billed;
  - the research note's IPL wording, aligned with ADR 0001;
  - the spec session produces the text, and the controller places it.

**Round 6**, 2026-11-18: the licence. The goal: IronAFS and every improvement
to it stay free and open source, rather than disappearing into closed
products (Context).

| Seat | Served model | Effort | Result |
|---|---|---|---|
| claude-code (subagent, research) | claude-mythos-5-2 | default | Research note: recommends GPL-3.0-or-later, alternative MPL-2.0 |
| codex (GPT), independent, without the research note | gpt-6.2-sol | xhigh | Recommends GPL-3.0-or-later, ranked first of nine; contributor model: DCO only at launch |

**Where they agree:**

- GPLv3 meets the goal for anything distributed.
- The planned dependencies (MIT and Apache) and the system libraries are
  compatible with it.
- Direct notarized and signed downloads work.
- Little friction is expected for universities and research sites.

**Owner decisions:**

- **GPL-3.0-or-later.**
- **DCO only at launch.** A lawyer-drafted contributor grant comes before the
  first outside contribution, if stores or relicensing matter.
- **One licence for code and documentation.**

**Not legal advice.** Both reviews name the points where a lawyer's view
matters (`docs/research/2026-11-18-licence-options.md`, section 7).

