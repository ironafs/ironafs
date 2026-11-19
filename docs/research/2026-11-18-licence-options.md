<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Licence options: research notes (2026-11-18)

**How this note was made.**

- Web research on 2026-11-18, with primary sources where they exist: licence
  texts, the FSF's FAQ and licence list, store agreements, and projects' own
  statements.
- Each claim is marked:
  - INSPECTED: the source text was downloaded and read;
  - RELAYED: read through a tool that summarises pages, so quotes were not
    checked against the raw text;
  - NOT VERIFIED: from memory or secondary reports only.
- **Not legal advice.** The note reports what the texts say and what their
  publishers say about them. Whether a clause binds in a given country, and
  how a court would read it, is a lawyer's question. Section 7 lists where a
  lawyer's view matters most.
- Store agreements change often. The Apple and Microsoft texts are the
  versions served on 2026-11-18.
- The note has not yet been cross-checked by a second model family.
- This is research. The operative decision (the licence and the contribution
  model) is recorded in [ADR 0001](../adr/0001-licence-and-dont-copy.md).

## Answer

**Apache-2.0 does not meet the owner's goal.** The goal is that IronAFS, and
every improvement made to it, stays free and open source: no company can take
IronAFS, improve it and ship it closed. Apache-2.0 lets anyone ship a closed,
improved IronAFS.

**`GPL-3.0-or-later` meets the goal as far as any licence can.** Whoever
distributes IronAFS or a modified version must give the recipients the
source, under the same terms. Its limits:

- No licence examined here forces release of changes a company uses only
  internally.
- The AGPL adds little for a filesystem client, which nobody uses over a
  network.
- The GPL has one real cost for IronAFS, the Mac App Store. Apple's terms
  conflict with it, so GPL code the owner does not own cannot safely go
  there. Direct, notarized distribution is not affected.

Every dependency class the project plans is compatible with GPLv3.

**Recommendation:** `GPL-3.0-or-later`. Contributions keep the DCO; a short
written grant (relicensing under other free licences, app stores) is an
option for later.

**The alternative is `MPL-2.0`.** It protects IronAFS's own files, has no
store conflict and needs no extra grant. But it lets a company build a closed
product around the code.

## Comparison

"Yes" means the licence requires source release in that case.

| Case | Apache-2.0 | MPL-2.0 | LGPL-3.0 | GPL-3.0 | AGPL-3.0 | EUPL-1.2 |
|---|---|---|---|---|---|---|
| A company ships a modified IronAFS to others | No | Yes, all MPL-covered files, changed or not | Yes, for the library | Yes, whole program | Yes, whole program | Yes, but see row 6 |
| A company links IronAFS code into its own closed product and ships it | No | No, for its own new files | No, if the user can relink | Yes | Yes | Yes, but see row 6 |
| A company modifies it and uses it only internally | No | No | No | No | No | No |
| A company runs a modified copy as a hosted service | No | No | No | No | Yes, if remote users interact with it | Arguably yes |
| A vendor ships it in a locked consumer device | No | No | Yes, installation information | Yes, installation information | Yes | No such clause |
| A recipient can weaken the copyleft | n/a | Only to the GNU licences | Up to GPL | No | No | Yes, to a listed weaker licence |
| Patent grant | Yes | Yes | Yes | Yes | Yes | Yes |
| Mac App Store, with outside contributors' code | No conflict | No conflict known | Conflict reported | Conflict (FSF) | Conflict (FSF) | Not examined |
| Rust static linking by a downstream user | Free | Free | Awkward (relink duty) | Whole work is GPL | Whole work is AGPL | Whole work is EUPL |
| Meets the owner's goal | No | Partly | Partly | Yes | Yes, with extra friction | Weakly |

## 1. What each licence does and does not prevent

### GPLv3

- **It covers distribution.** INSPECTED, FSF FAQ: "Using the GNU GPL will
  require that all the released improved versions be free software. This means
  you can avoid the risk of having to compete with a proprietary modified
  version of your own work."
- **It does not cover private or internal use.** INSPECTED, FSF FAQ: "The GPL
  does not require you to release your modified version, or any part of it.
  You are free to make modifications and use them privately, without ever
  releasing them. This applies to organizations (including companies), too".
  Providing copies to contractors for use off-site is distribution.
- **It does not cover hosted services.** INSPECTED, FSF FAQ: a company running
  a modified GPL program on a web site "does not have to release the modified
  sources", unless the program is under the AGPL.
- **Nobody can be made to publish.** Source goes to the recipients of the
  binary, not to the public or to the owner.
- **Locked devices.** INSPECTED, GPLv3 §6. Someone who conveys the program in,
  with or for a "User Product" (consumer or household property), as part of
  transferring it, must give "Installation Information" so modified versions
  can be installed. This note's reading: an appliance sold only to businesses
  is likely outside that definition; the source duty still applies.
- **`-only` or `-or-later`.** INSPECTED, GPLv3 §14. "Or any later version"
  lets recipients follow later GPL versions published by the FSF, which "will
  be similar in spirit". §14 also allows naming "a proxy" who decides which
  future versions apply. `-only` fixes the terms: any later move then needs
  every contributor's consent, unless a contributor grant covers it
  (section 4).

### AGPLv3

- INSPECTED, AGPLv3 §13: a modified version must offer its source to "all
  users interacting with it remotely through a computer network".
- INSPECTED, FSF FAQ: that fits a program "expressly designed to accept user
  requests and send responses over a network".
- **For IronAFS** (this note's reading): a client's users are local. §13 would
  apply only to someone building a hosted gateway on a modified IronAFS core.
- Some organisations reportedly forbid AGPL software (NOT VERIFIED), which
  could cost adoption at the sites an AFS client is for.

### LGPLv3

- It lets closed programs link the library; changes to the library stay open.
- INSPECTED, FSF FAQ: static linking requires providing the application in a
  form that lets the user "modify the library and relink the application".
  Rust links statically by default, so this is awkward, and it protects less
  than the GPL.

### MPL-2.0

- **File-level copyleft.** INSPECTED, MPL-2.0 §1.10 and §3.2: whoever
  distributes an executable must make all the covered source (the MPL files,
  including any changes to them) available to its recipients, under the MPL.
- **It allows closed larger works.** INSPECTED, Mozilla's FAQ: "new files
  containing no MPL-licensed code are not Modifications", which allows MPL
  code "to be statically linked to and distributed as part of a larger
  proprietary piece of software".
- **Internal use is free** ("inside a company or organization counts as
  'private'").

### EUPL-1.2

- INSPECTED, EUPL-1.2 §1: distribution includes "providing access to its
  essential functionalities", which reaches towards hosted services.
- INSPECTED, FSF licence list: recipients can relicense under some weaker
  licences, so "developers can't rely on this license to provide a strong
  copyleft."

### Apache-2.0, for contrast

- No copyleft. A company may modify, close and sell IronAFS, keeping only the
  notices and the licence text.
- INSPECTED, FSF licence list: its patent termination provision is "a good
  thing".

### Limits that apply to every option

- **Enforcement is the owner's job.** INSPECTED, FSF: "only the copyright
  holder or someone having assignment of the copyright can enforce the
  license."
- **Copyleft rests on copyright.** ADR 0002 records that purely AI-generated
  output may not be protected. Where code is model-written with little human
  authorship, a copyleft claim over it may be weak. This note's reading; a
  lawyer's view matters.
- **Ownership.** The owner must actually hold the copyright in the code they
  license. A lawyer's view matters here too.
- **Ideas are free.** Anyone may read IronAFS and write their own client. No
  licence prevents that, and ADR 0001 relies on the same principle.

## 2. Compatibility for IronAFS

- **Apache-2.0 crates in a GPLv3 binary: allowed.** INSPECTED, FSF licence
  list: Apache-2.0 is "compatible with version 3 of the GNU GPL", but "not
  compatible with GPL version 2". INSPECTED, ASF: "Apache 2 software can
  therefore be included in GPLv3 projects".
- **MIT crates: allowed.** INSPECTED, FSF licence list: Expat (MIT) is
  "compatible with the GNU GPL."
- **The rest of the current allowlist** (BSD-2-Clause, BSD-3-Clause, ISC,
  Zlib, 0BSD, BSL-1.0, the Unicode licences) is commonly treated as
  GPL-compatible. NOT VERIFIED per licence.
- **MPL-2.0 dependencies: allowed,** through MPL-2.0 §3.3 (INSPECTED, FSF
  licence list), unless the package is marked "Incompatible With Secondary
  Licenses", which rules out combining it with GPL code.
- **Static linking.** The linked binary is one work, conveyed under GPLv3. The
  permissive crates keep their own licences and notices, so ADR 0001's release
  notices file is still needed.
- **MIT Kerberos on Linux:** permissive, no exception needed (INSPECTED).
- **Apple and Microsoft system frameworks.** INSPECTED, GPLv3 §1: "System
  Libraries" that come with "a major essential component … of the specific
  operating system" are excluded from the Corresponding Source. FSKit,
  GSS.framework, SSPI and the Cloud Files API ship with the operating system.
  This note's reading is that they qualify. A proprietary runtime bundled
  inside the app would not.
- **GPLv2-only code stays out.** GPLv2 and GPLv3 "are not compatible"
  (INSPECTED, FSF FAQ). The kernel's kafs and AF_RXRPC are GPLv2-only, and
  IronAFS takes none of it.
- **OpenAFS code stays out.** IPL-1.0 "has a choice of law clause which makes
  it incompatible with the GNU GPL" (INSPECTED, FSF licence list).
- **ADR 0001's don't-copy rule is unchanged by any option here.**
- **Rule 21 still decides dependencies.** A GPL licence would permit LGPL and
  other GPL-compatible crates in law; the allowlist need not grow.

## 3. Distribution

### Mac App Store

- **The FSF's position.** INSPECTED, FSF, 2010-05-26: "Apple's Terms of
  Service impose restrictive limits on use and distribution for any software
  distributed through the App Store, and the GPL doesn't allow that." The
  analysis "would apply to all versions of the GNU GPL and AGPL."
- **Apple's current text still limits use.** INSPECTED, Apple Developer
  Program License Agreement, Schedule 1, Exhibit B. GPLv3 §10 forbids
  "further restrictions". Apple's standard licence has a carve-out for
  "open-sourced components"; whether it removes the conflict is untested. A
  lawyer's view matters. The agreement's signing-keys clause could also be
  read against GPLv3's installation information.
- **VLC.** RELAYED: one VLC developer's complaint led Apple to remove VLC for
  iOS in 2011. It returned in July 2013 after VideoLAN worked with its major
  copyright holders (VideoLAN's press release,
  https://images.videolan.org/press/ios2out.html), dual-licensed MPL-2.0 and
  GPL-2.0, with the MPL used for the App Store (RELAYED, press reports). One
  contributor's copyright was enough to end store distribution.
- **What the owner can do alone.** INSPECTED, FSF FAQ: a copyright holder "can
  release it under various different non-exclusive licenses". While the owner
  holds all the copyright, the owner may submit IronAFS to the store on the
  store's terms. That ends with the first outside contribution under plain
  GPL, unless contributors grant a store permission (section 4).
- **Precedent.** INSPECTED, Nextcloud iOS (`COPYING.iOS`), GPLv3 or later: "we
  have committed not to pursue any license violation that results solely from
  the conflict between the GNU GPLv3 or any later version and the Apple App
  Store terms of service."

### Direct, notarized macOS distribution

- No GPL conflict was found. The owner signs and notarizes their own build,
  and Apple does not convey the software.
- INSPECTED, FSF FAQ: signing keys must be released only when GPL software is
  conveyed inside a User Product whose hardware checks signatures. A download
  is not that.
- A user who rebuilds the FSKit extension signs it with their own Apple
  developer identity. That limit is Apple's, not IronAFS's.
- NOT VERIFIED: that an FSKit extension can ship in a Developer ID app outside
  the store. The macOS spike must confirm it, because a GPL choice leans on
  it.

### Microsoft Store and Windows signing

- INSPECTED, Microsoft App Developer Agreement v8.11: the rights granted to
  Microsoft exclude "any material subject to any FOSS licenses", and the
  developer is responsible for "any source code availability requirements".
  No documented GPL conflict was found.
- Authenticode signing with the owner's own certificate raises no GPL
  question.

### Linux packaging

- INSPECTED, Debian: the GPL is common in Debian main.
- NOT VERIFIED: Fedora's allowed-licence list (the page refused automated
  access). Fedora and RHEL ship GPLv3 software such as GCC and Bash.

## 4. Contribution model if the licence may change again

- **DCO only (today's rule).** INSPECTED, DCO 1.1 (a): the contributor has
  "the right to submit it under the open source license indicated in the
  file". GitHub's terms make this "inbound=outbound" (INSPECTED, Terms of
  Service D.6). A later licence change needs each contributor's consent, or a
  rewrite of their code.
- **What that costs later.** RELAYED, OpenSSL, 2017: moving to Apache-2.0 meant
  reaching "nearly 400 individuals". INSPECTED, VideoLAN, 2011: one developer
  "refused to answer. His code was therefore rewritten."
- **DCO plus a stated relicensing permission.** INSPECTED, Samba's Developer's
  Declaration adds a clause to the DCO granting the work under the GPL and
  LGPL, "version 3 … or (at the option of the project) any later version". It
  is light, and it covers only what it names.
- **A contributor licence agreement.**
  - INSPECTED, Apache ICLA §2: a broad licence including the right to
    "sublicense", which permits any later licence, including a closed one.
  - RELAYED, Harmony agreements: the project chooses how far it may
    relicense, from the original licences through OSI- or FSF-approved
    licences to any licence.
  - INSPECTED, WinFsp: contributors "assign to us joint ownership".
- **Copyright assignment.** INSPECTED, FSF: it requires assignment so it can
  enforce. It is the heaviest model, and rare outside foundations.
- **The friction argument.** INSPECTED, Software Freedom Conservancy: CLAs
  make a contributor's first interaction "a complex legal negotiation", while
  a clear statement that the contribution is offered under the project's
  licence is enough. GitLab moved from a CLA to the DCO in 2017 for that
  reason (INSPECTED).
- **The trust argument** (opinion): under copyleft, a grant that lets one
  party relicense under "any licence" lets that party close the code.
  Contributors notice. A grant limited to free licences keeps most of the
  flexibility and removes that fear.
- **"Or later" as a path** moves the project only along the GPL's own versions
  (GPLv3 §14). It gives no path to other licences or a store exception.
- **Before the first outside contribution,** the sole owner can change the
  licence at will, dual-license, grant exceptions and submit to stores. Every
  copy already published keeps the licence it was published under.

## 5. Documentation

Documentation under another licence (for example CC BY 4.0) is possible, and
REUSE supports it. INSPECTED, FSF: CC BY 4.0 is "compatible with all versions
of the GNU GPL" but "should not be used on software". It would let other AFS
implementers reuse the documents freely, at the cost of a second licence and
care where documents hold code. One licence for everything is simpler.

## 6. Precedents

| Project | Licence | What happened, and why | Source |
|---|---|---|---|
| Samba | GPLv3-or-later, LGPLv3-or-later | Copyright held by individuals; a DCO-style declaration with an "at the option of the project" clause | INSPECTED |
| WinFsp (Windows filesystem layer) | GPLv3, a FLOSS linking exception, commercial licences | One owner; contributors assign joint ownership | INSPECTED |
| macFUSE | Some components closed since 2019 (version 3.9) | "In an ideal world macFUSE would be open source and companies making a profit from it would be supporting its continued development." | INSPECTED |
| Nextcloud iOS | GPLv3-or-later | A written promise not to enforce over the App Store conflict alone | INSPECTED |
| Syncthing | GPLv3, then MPL-2.0 (2015) | To allow app-store distribution and inclusion in other works | INSPECTED |
| VLC engine | GPLv2-or-later, then LGPLv2.1-or-later (2011) | Every developer asked; one did not answer, and that code was rewritten | INSPECTED |
| OpenSSL | Own licence, then Apache-2.0 | About 400 contributors to contact | RELAYED |
| Grafana, Loki, Tempo | Apache-2.0, then AGPLv3 (2021) | Easy because a CLA was already in place | INSPECTED; CLA detail RELAYED |
| Terraform | MPL-2.0, then BUSL (2023) | The community forked it as OpenTofu | INSPECTED |
| Stratis (Linux storage, Rust) | MPL-2.0 | Contributions "must also be licensed under the MPL 2.0" | INSPECTED |

What they show (this note's reading):

- **Copyleft is livable for filesystem projects.** Those close to IronAFS's
  position chose GPLv3 (Samba, WinFsp) and have lived with it for years.
- **macFUSE is the owner's fear in reverse.** Companies took without giving,
  and the maintainer's remedy was to close some of the source.
- **Moving later is easy only with the rights in hand.** Permissive-to-copyleft
  was easy only where one party already held the rights. Without them it took
  years, or a rewrite.
- **App stores** are the usual reason to step down from the GPL.

## 7. Recommendation

### Primary: `GPL-3.0-or-later`

**Why:**

- It is the weakest licence that fully meets the goal: whoever distributes
  IronAFS, or a work based on it, must license it under the same terms and
  make its complete corresponding source available to the recipients (GPLv3
  §6). Separate programs merely shipped alongside it are not covered (§5).
- It has an explicit patent grant.
- It is compatible with every dependency class in rule 21.
- `-or-later` keeps future GPL versions open without asking anyone.

**What it does not do:** it does not reach internal use, hosted use or
independent rewrites.

**Contribution model.** Keep the DCO, and add a short contributor grant to
`CONTRIBUTING.md`, confirmed in each pull request. The grant lets the
maintainer:

1. also license the contribution under other licences approved by the OSI or
   recommended by the FSF;
2. distribute it through app stores whose terms conflict with the GPL.

This is a lightweight CLA, and its own terms rule out a closed-source future.
A lawyer should write it.

**Consequences:**

- **ADRs.** ADR 0001 records the licence, the contribution model and how the
  licence could change later. The don't-copy rule, the people rule,
  the AI safeguards and the provenance rule stay. It is a class R change, with
  its own review.
- **Licence files and labels.**
  - `LICENSE` holds the GPLv3 text, and `LICENSES/` gains
    `GPL-3.0-or-later.txt`.
  - Every SPDX header and the `REUSE.toml` annotations change, and
    `reuse lint` must pass again.
  - Cargo's `license` field changes.
- **`deny.toml`.** The allowlist stays. The workspace's own crates need their
  licence accepted, by exception or by ignoring private workspace crates.
- **Dependency policy.** GPL-family dependencies stay excluded under rule 21,
  although they would be lawful.
- **Releases** carry the source or a written offer (GPLv3 §6), the GPL text
  and the third-party notices file.
- **Documentation.** `CONTRIBUTING.md`, the PR template, `README.md` and
  `LICENSING.md` name the licence and the grant.
- **macOS.** Direct notarized distribution becomes the main channel. The Mac
  App Store stays possible only while the owner holds, or is licensed for,
  every line. The macOS spike must confirm that FSKit extensions can ship
  outside the store.

### Alternative: `MPL-2.0`

- **What it gives:** no store conflict found (Syncthing moved to it for that
  reason; VLC for iOS added it alongside the GPL); suits Rust's static
  linking; plain DCO works; an explicit patent grant.
- **What it gives up:** a company may build a closed product around IronAFS,
  making available only the source of IronAFS's own MPL-covered files,
  changed or not, while its new files stay closed. That is weaker than the
  goal as stated.

### Not recommended

- **Apache-2.0** as the lasting licence: it does not meet the goal.
- **AGPL-3.0:** it adds friction for a case that barely applies.
- **LGPL-3.0:** it protects less, and is awkward with Rust.
- **EUPL-1.2:** a recipient can step its copyleft down.

### Where a lawyer's view matters

- the wording of the contributor grant, and whether it binds a contributor's
  employer;
- the Mac App Store: Apple's carve-out and the signing-keys clause;
- the "System Libraries" reading for FSKit and Cloud Files, and the "User
  Product" reading for appliances;
- how far copyleft holds over code written mostly by AI models.

## Decisions this research informed

Recorded in [ADR 0001](../adr/0001-licence-and-dont-copy.md):

- `GPL-3.0-or-later`, for code and documentation alike;
- DCO only at launch. Any contributor grant, its scope, and whether it covers
  app stores, is decided later and drafted by a lawyer;
- GPL-family dependencies stay excluded under rule 21;
- direct, notarized macOS distribution as the main channel.

## Sources

- FSF: [GPL FAQ](https://www.gnu.org/licenses/gpl-faq.html), [licence list](https://www.gnu.org/licenses/license-list.html), [why the FSF gets copyright assignments](https://www.gnu.org/licenses/why-assign.html)
- Licence texts: [GPLv3](https://www.gnu.org/licenses/gpl-3.0.txt), [AGPLv3](https://www.gnu.org/licenses/agpl-3.0.txt), [LGPLv3](https://www.gnu.org/licenses/lgpl-3.0.txt), [MPL-2.0](https://www.mozilla.org/media/MPL/2.0/index.txt), [EUPL-1.2](https://joinup.ec.europa.eu/sites/default/files/custom-page/attachment/2020-03/EUPL-1.2%20EN.txt), [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0.txt)
- Mozilla: [MPL 2.0 FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)
- ASF: [Apache License v2.0 and GPL compatibility](https://www.apache.org/licenses/GPL-compatibility.html), [ICLA](https://www.apache.org/licenses/icla.pdf)
- FSF on the App Store: [GNU Go compliance](https://www.fsf.org/news/2010-05-app-store-compliance), [more about App Store enforcement](https://www.fsf.org/blogs/licensing/more-about-the-app-store-gpl-enforcement), [VLC enforcement](https://www.fsf.org/blogs/licensing/vlc-enforcement)
- Apple: [Developer Program License Agreement](https://developer.apple.com/support/downloads/terms/apple-developer-program/Apple-Developer-Program-License-Agreement-English.pdf), [standard end-user licence](https://www.apple.com/legal/internet-services/itunes/dev/stdeula/)
- Microsoft: [App Developer Agreement](https://go.microsoft.com/fwlink/?linkid=528905), [Store Policies](https://learn.microsoft.com/en-us/windows/apps/publish/store-policies)
- Contribution models: [DCO 1.1](https://developercertificate.org/), [GitHub Terms of Service D.6](https://docs.github.com/en/site-policy/github-terms/github-terms-of-service), [Software Freedom Conservancy on CLAs](https://sfconservancy.org/blog/2014/jun/09/do-not-need-cla/), [Harmony agreements](https://harmonyagreements.org/guide), [GitLab's move to the DCO](https://about.gitlab.com/blog/gitlab-switches-to-dco-license/)
- Precedents: [Samba copyright policy](https://www.samba.org/samba/devel/copyright-policy.html), [WinFsp licence](https://github.com/winfsp/winfsp/blob/master/License.txt), [macFUSE open source status](https://github.com/macfuse/macfuse/wiki/Open-Source-Status), [Nextcloud iOS `COPYING.iOS`](https://github.com/nextcloud/ios/blob/master/COPYING.iOS), [Syncthing's move to MPLv2](https://forum.syncthing.net/t/syncthing-is-now-mplv2-licensed/2133), [VideoLAN on the LGPL engine](https://www.videolan.org/press/lgpl-libvlc.html), [OpenSSL licensing update](https://mirror.openssl-library.org/post/2017-03-20-license/), [Grafana relicensing](https://grafana.com/blog/2021/04/20/grafana-loki-tempo-relicensing-to-agplv3/), [OpenTofu manifesto](https://opentofu.org/manifesto/), [Stratis README](https://github.com/stratis-storage/stratisd/blob/master/README.md)
- Others: [Debian licence information](https://www.debian.org/legal/licenses/), [MIT Kerberos licence](https://web.mit.edu/kerberos/krb5-latest/doc/mitK5license.html), [REUSE FAQ](https://reuse.software/faq/)
