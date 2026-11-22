<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Specifications

A specification says exactly how one part of IronAFS behaves, or how one
format is laid out, in enough detail to implement and test it. Code is
written from the specification, and tests check the code against it.

| Spec | Kind | Class | Status | Implements |
|---|---|---|---|---|
| [AFS directory format](directory-format.md) | protocol | S | draft (2026-11-21) | ADR 0004, C4 point 1, C6 points 1, 3 and 4, L1 points 2 and 5, L25 point 2, L29 point 1, N31 points 1 to 3; constraint 4 |
| [First-party DNS client](dns-client.md) | protocol | S | draft (2026-11-21) | ADR 0004, C4 point 6, C12 point 3, C33 points 1, 3 to 6 and 8, C34 point 3, C37 point 1; ADR 0005, Decision, point 6 |
| [Local IPC framework](local-ipc.md) | format | S | draft (2026-11-21) | ADR 0004, C43 points 1 and 2, C44 points 1 to 6, C45 points 1 to 3, N41 points 1 to 9, C16 point 6 (what a catalogue declares), C5 point 5 as amended by ADR 0005; constraint 4 |
| [Local IPC framework: resources and scheduling](local-ipc-resources.md) | behaviour | S | draft (2026-11-21) | ADR 0004, C39 points 2 and 3, O28 points 2 and 3, C16 point 6 (the reserved space and the order); constraint 4 |
| [Local IPC framework: the Windows named pipe](local-ipc-windows.md) | behaviour | S | draft (2026-11-21) | ADR 0004, C45 point 1, C5 point 5 as amended by ADR 0005; ADR 0005, sheets 17 to 20 |
| [The `Rpc` and `Disk` seams and the scheduling model](seams-and-scheduling.md) | behaviour | S | draft (2026-11-21) | ADR 0004, C3 points 1 and 3 to 6, C4 points 1, 4 and 7, C16 points 2 and 3, C24 point 1, C27 point 2, C28 points 3, 4 and 6, C29 point 9, C32 points 1, 4 and 5, C37 points 4 and 5, C39 points 3 and 4, C46 points 3 and 4, N7 point 2, N32 point 6, O45 point 5 |
| [Rx transport and the null security class](rx.md) | protocol | S | draft (2026-11-21) | ADR 0004, C3 point 1, C4 points 1, 3 and 4, C5 point 1, C6 points 1 and 4, C19 point 1, C24 point 7, C30 points 1 and 2, C31, C32 points 1 and 5, C39 point 3, N5 point 2, N6 point 3, N7 point 5, N31 points 1 and 2, N45; constraint 4; ADR 0006, decisions 1 and 2 and specification items 1 to 10 |
| [Test strategy](testing.md) | procedure | P | draft (2026-11-21) | ADR 0004, constraint 4, C3 points 2 and 6, C4 point 7, O22, O45 points 3 and 6, O46 points 1 and 3, O47 points 1 to 3, O48 point 5, O49 points 1 to 3; coding standards rules 7 and 15 |
| [Volume location and file-service reads, and operation grants](vl-fs-read.md) | protocol | S | draft (2026-11-21) | ADR 0004, constraint 4; A8; C6 points 1 and 4, C22 points 1 to 3, C46 points 3 and 4, N5 points 2 and 3, N20, N28 points 1 to 4, N31, N38 points 1 to 3, O40 point 1; ADR 0006, decision 1, points 5 and 6 |
| [XDR profile](xdr.md) | protocol | S | draft (2026-11-20) | ADR 0004, C3 point 1, C4 points 1 and 4, C6 points 1, 2 and 4, N31 points 1 and 2, O47 points 2 to 4; constraint 4 |

## Frozen formats

A format listed here is one IronAFS promises to keep reading. Changing its
layout is class R ([docs/workflow.md](../workflow.md) §1).

| Format | Spec and section | Magic | Versions read | Versions written | Frozen |
|---|---|---|---|---|---|
| *(none yet)* | | | | | |

## What a specification is, and is not

- **It is** the mechanism: layouts, field types and order, caps, states,
  orderings, error results, and the tests that prove them.
- **It is not an ADR.** An ADR decides, with alternatives and a review
  ([docs/adr/README.md](../adr/README.md)). A specification carries a
  decision out. It makes no architecture choice and weakens none.
- **It is not research.** Findings about platforms and protocols live in
  `docs/research/`. A specification cites them; it does not repeat their
  argument.
- **It is not a plan.** Tasks are issues. A specification describes the
  thing as it must be, never when or by whom it is built.
- **It is not code documentation.** Doc comments describe the code as it
  is. Where code and specification differ, one of them has a defect, and
  the pull request that finds it says which.

When writing a specification turns up a choice that an ADR has not made,
stop. A choice between designs that is hard to change later goes to an ADR,
or to an amendment of a working decision. A detail that follows from the
decisions already made is the specification's own, and is written with its
reason.

## Kinds

Each specification is one kind. The kind sets its sections and its least
review class.

| Kind | Describes | Examples |
|---|---|---|
| **protocol** | Bytes and exchanges with a peer IronAFS does not control | Rx packets, XDR, an RPC interface, callbacks, SMB messages |
| **format** | Bytes IronAFS itself defines and stores or sends | A state directory record, a cache format, the local IPC messages |
| **behaviour** | What a component does, where no byte layout is the point | An ordering of events, a state machine, a layer's handling of a case |
| **procedure** | Steps a person or a package follows | Packaging, key rotation, a restore |

A specification that needs two kinds is split, or takes the kind with the
higher class and says so.

## Naming and numbering

- **One file per subject:** `docs/specs/<subject>.md`, lower case, words
  joined by hyphens, named for the thing it specifies:
  `stable-object-key.md`, `rx-packet.md`. The name never carries a
  milestone, a task or an ADR item id.
- **Specifications are not numbered.** They describe things that live side
  by side, not a sequence of decisions. The file name is the identifier.
- **Sections are numbered,** and the numbers are stable once the
  specification is reviewed. Code, tests and other documents cite
  `stable-object-key §4.3`. A removed section keeps its number, with one
  line saying where the content went. New sections take new numbers.
- **Requirements that tests cite are numbered points** within a section,
  cited as `§4.3, point 2`, the way ADR decisions are cited.
- **A large subject is split by subject,** not by size: one file per
  interface or per record, with a short parent that lists them.
- **Vectors and corpora** live with the tests, under the crate that owns
  the format or interface (coding standards rule 15), not in
  `docs/specs/`. The specification names their paths. A published conformance corpus has a
  directory of its own ("Published conformance corpora", below).

## Status

The status line is the first line after the title, with a date.

| Status | Meaning | What may change |
|---|---|---|
| `draft` | Being written. Nothing rests on it. | Anything |
| `reviewed` | Passed the review its class requires, and was merged | Any part, by a pull request of the same class and review |
| `frozen` | A format IronAFS promises to keep. Formats only. | Nothing in the frozen layout. A new version is added beside it (below). |
| `superseded by <spec>` | Replaced. Kept for the versions that used it. | Only the status line and links |

Rules:

- A specification is `draft` until its review record exists in
  `docs/reviews/` and its row is in the index above.
- Code that implements a `draft` specification does not merge, except a
  spike that says so and is not shipped.
- `frozen` applies per format version, not per file. A file may hold a
  frozen version 1 and a draft version 2. The status line names each.
- Freezing is the owner's decision (class R). It is recorded on the status
  line with its date and in the frozen formats table.
- A `frozen` or `superseded` specification is never deleted while any
  released version reads or writes its format.
- A specification of the kind `behaviour` or `procedure` is never frozen.
  It stays `reviewed` and changes by its class.

## How a specification cites ADR decisions

A specification starts by naming what it implements.

1. **Cite by id and point:** `ADR 0004, N38, point 8`. Cite the
   specification item too, where the ADR names one (`ADR 0004, S5`).
2. **Quote each invariant it must keep,** word for word, under the id. A
   paraphrase drifts; a quotation can be compared.
3. **Say whether each cited decision is firm or working.** A specification
   resting on a working decision copies that decision's "Revisit if"
   trigger into its own "Revisit if" list.
4. **Show the invariant is kept.** For each quoted invariant, the
   specification names the section that keeps it and the test that would
   fail if it were broken.
5. **Never restate a decision as the specification's own.** The ADR stays
   the one place a decision is made.

When an ADR is amended or superseded, its supersession sweep
([docs/adr/README.md](../adr/README.md), "Changing a decision") covers
`docs/specs/`. Every specification that quotes the changed decision is
updated in the same pull request, or gets an issue that names it.

A specification that cannot keep an invariant does not work around it. Its
author stops and raises a design issue, as for any contradiction of a
decision.

Before a task implements a specification, it re-checks each cited source
and each working decision the section rests on. A source that has changed
is raised as an issue first.

## Sources and provenance (ADR 0001)

Every specification ends with a Sources section. The rules come from
[ADR 0001](../adr/0001-licence-and-dont-copy.md) and are not relaxed here.

1. **Every source is named.** Title, publisher or author, version or date,
   and a link or other locator, down to the section used. "Common
   knowledge" is not a source.
2. **Every source is labelled by kind:**
   - **SPECIFICATION:** a published standard, draft or protocol document;
   - **DOCUMENTATION:** a vendor's or project's published user or
     administrator documentation, or an operating system's own interface
     documentation and headers;
   - **OBSERVED:** behaviour IronAFS's contributors watched (rule 4);
   - **DECISION:** an IronAFS ADR or another IronAFS specification;
   - **OWN:** IronAFS's own design, where no outside source applies. The
     format of a record IronAFS invents is OWN.
3. **No other implementation's source.** Nothing in a specification is
   copied, ported, translated or derived from the source code of another
   AFS implementation or of an Rx implementation outside this project, and
   no such file or function name appears. This includes their tests, test
   data, comments and interface definition files.
   - Published specifications and observation come first. Facts from
     permissive or unnamed other implementations reach a specification
     only through ADR 0001's isolated specification-from-source procedure
     below. GPL, IPL and proprietary AFS source (OpenAFS, AuriStor, and
     the Linux kernel's AF_RXRPC and kafs) stays barred (ADR 0001).
   - Where a permissive or unnamed implementation's source is genuinely the only route to a
     behaviour, ADR 0001's specification-from-source procedure is used,
     with all of its isolation:
     - a separate, fresh session reads the source and writes a facts-only
       specification;
     - that session has no memory or transcript shared with any other
       session, and no write access to the repository; the controller
       places its text;
     - a different model family reviews it for copied expression;
     - neither that session nor its reviewer writes or reviews IronAFS
       code, and the session that wrote a specification does not
       implement it;
     - no one has another implementation's source open while writing
       IronAFS code.

     The result is still only a lead for a wire fact: rule 14 applies to
     it. Until that is done, the gap is recorded under "Open questions" as
     MISSING.
   - That procedure is not used for OpenAFS, AuriStor, or the Linux
     kernel's AF_RXRPC or kafs while `AGENTS.md` bars their source
     ("nothing generated from their files"): no person or model reads
     their source to write a specification or code. ADR 0001's similarity
     audit is separate and follows ADR 0001.
     A protocol fact that no other admissible source answers stays
     MISSING until observation settles it.
   - A wire fact is never taken from interface-definition files or
     generated definitions, by any route (ADR 0004, N31, point 2).
4. **Observed behaviour is recorded,** so that someone else can repeat it:
   - what was done, and what was seen;
   - the venue: the kind of peer and its released version, the client's
     operating system and version, and the tool used to observe;
   - the date;
   - the evidence label EXECUTED
     ([docs/workflow.md](../workflow.md) §3c), and where the reduced
     capture or log is kept, if it is kept.

   The venue is described, never identified: no hostnames, cell names,
   addresses, user names or keys (AGENTS.md, "Public by default"). An
   observation from one venue says so. It is not written as a rule for all
   peers until a second venue or a published source agrees.

   A layout learned from captures:
   - records the operation, the server release, the security class and
     the cases exercised, and claims nothing beyond those cases;
   - varies one input at a time against known values;
   - keeps a field that more than one reading fits as an open question;
   - is drafted from the raw bytes. A protocol analyser's decoded view is
     that tool's interpretation, not an observation: it may be used only
     as a cross-check after the draft, with the tool and its version
     recorded, and a disagreement is settled by fresh wire evidence.

   Raw captures stay out of the repository and out of model prompts. A
   checked-in fixture holds only the bytes a test needs, with synthetic
   data (ADR 0001, "Test tools and test data").

   Captures are made only against a test venue with synthetic cell
   names, users, keys and data. A capture from a live cell, or one
   holding credentials or private data, is never made for a
   specification and never used, however it is reduced (ADR 0004, N31,
   point 2). A reduced capture counts as an independently generated
   black-box fixture (rule 14) only when IronAFS's contributors produced
   it by running released software as a black box, and its fixture
   provenance (venue, versions, date, what was exercised) is recorded.
5. **Each requirement traces to a source.** A numbered point that states a
   fact about a peer or a platform names its source inline or by a
   reference to the Sources table. A point that is IronAFS's own choice
   says so, with its reason.
6. **Facts only, in IronAFS's own words.** A specification states what
   holds. It does not reproduce a source's text beyond a short, marked
   quotation where the exact words matter. A source with no licence
   notice, or an unclear one, is cited for its facts and not quoted in a
   specification at all; short attributed quotations of it belong in
   research notes and review records. A figure under a licence IronAFS
   cannot take in (for example an IPL-1.0 figure inside an Internet-Draft)
   is never copied; its field facts are cited and described independently.
7. **Sources disagree sometimes.** The specification says which it
   follows and why, and tests the case.
8. **Old evidence keeps its date.** It is carried forward with one line
   saying why it still applies.
9. **Third-party material** that is included under a permissive licence,
   such as a table from a standard, keeps its notice and gets a
   `docs/provenance.md` row in the same commit.
10. **Older published AFS documents** (for example the 1991 AFS-3
    programmer's references) are used only as a verified public edition
    whose provenance is established independently of any implementation's
    source repository (ADR 0004, N31, point 3):
    - the edition is in the register of editions below before any
      specification uses it, with its origin and licence status;
    - functional facts only, in IronAFS's own words;
    - a fact that affects interoperability or correctness is corroborated
      by an independent source or a reproducible observation, and the
      corroboration is named in the register;
    - a claim that would fix wire or on-disk behaviour is labelled
      UNVERIFIED until it is corroborated, and nothing is coded from it.

    A copy held in, or relocated from, an implementation's repository is
    never used. Until a qualifying edition is found, observation decides.
    Counsel reviews this citation policy before the first release
    (ADR 0004, N31, point 3).
11. **Documents kept inside another implementation's repository** are
    assessed one by one, by origin, revision, contents and licence. An
    independently published copy is preferred. A document whose prose
    cannot be read without opening source, headers or interface
    definitions is skipped.
12. **The community Rx specification draft**
    ([claim record](../reviews/2026-11-20-adr-0004-claims.md),
    `[DRV-084]`) is cited by its
    exact revision and the date read; the retrieval URL and any reuse
    statement obtained from its editor or registrar are recorded. Its
    section "Historical Implementation Notes" contains fragments of
    another implementation's code: it is excluded by name. That section is never
    read, never put in a model's prompt, and never cited. Every
    specification that cites the draft says so in its Sources section.
13. **Mailing-list archives:** attributable public prose only. A post
    that mixes prose and a patch is read through a separately available
    prose-only copy, or skipped. The message is cited, and any claim used
    as a protocol requirement is corroborated by a specification or a
    reproducible observation.
14. **Every wire fact takes one of ADR 0004 N31's two routes** (point 1,
    Firm): an allowlisted specification passage, or an independently
    generated black-box fixture (rule 4), recorded with its edition and
    section, or its fixture provenance.
    - An allowlisted passage is a section of a registered edition that
      the register's "Allowlisted passages" column lists.
    - A source under rules 3 and 10 to 13 (a specification written from
      source, an older AFS document, a document kept in another
      implementation's repository, the community Rx draft, a mailing-list
      post) is not enough on its own. It is a lead: the fact it gives is
      labelled UNVERIFIED, and nothing is coded from it, until an
      allowlisted passage or such a fixture confirms it.
    - Implementation history, interface-definition files, generated
      definitions, and live-cell captures holding credentials or private
      data are never a route (N31, point 2).

## Labelling claims

Every numbered point is labelled, so a reader can tell a fact from a
choice without asking.

1. **A fact about a peer, a standard or a platform** ends with a tag
   naming its source row and section, and its verdict:
   `[X1 §4.10: CONFIRMED]`.
   - CONFIRMED: the cited section of the registered edition states it,
     and the register allowlists that section (Sources, rule 14); or an
     independently generated black-box fixture shows it.
   - UNVERIFIED: no reachable source settles it yet. Nothing is coded
     from it (ADR 0004, C6, point 4, and N31, point 3).
   - OBSERVED: an observation in the specification's "Observed behaviour"
     section shows it, in the cases recorded there.
2. **A decision** ends with the ADR id and point, and whether it is firm
   or working: `[ADR 0004 C6.1: Firm]`.
3. **IronAFS's own choice,** where no ADR decides and no source applies,
   ends with `[OWN]`, and the point or its paragraph gives the reason.
4. **Evidence labels** from [docs/workflow.md](../workflow.md) §3c are
   used where a claim is about work rather than about the protocol:
   EXECUTED, INSPECTED, PROPOSED, MISSING or NOT INSPECTED. A requirement
   resting on a MISSING fact is marked, and the code for it does not merge
   until the fact is found or the requirement is removed.

## Register of editions

Every edition of an outside document that a specification cites is
registered here before a specification uses it. ADR 0004, N31, point 3,
requires this for older published AFS documents; IronAFS registers every
cited edition, so one table answers "which text did we read". A
specification's Sources table names the register id.

- **Document and edition:** title, author or editor, number, revision and
  date.
- **Origin:** where the copy was obtained. For an older AFS document, how
  its provenance is independent of every implementation's repository
  (rule 10).
- **Retrieved:** the date the copy was read.
- **Licence status:** what reuse of its text is allowed, or "none
  assumed". This sets whether a specification may quote it (rule 6).
- **Updates checked:** errata, updates and obsoleting documents, with the
  date they were checked.
- **Corroboration:** for an older AFS document, the independent source or
  observation that corroborates each load-bearing fact, or "none yet".
- **Allowlisted passages:** the sections a wire fact may be taken from
  (Sources, rule 14), and any section excluded by name.
- **Cited by:** the specifications that cite it.

A new edition of a document gets a new row. A row stays while any
specification cites it.

| Id | Document and edition | Origin | Retrieved | Licence status | Updates checked | Corroboration | Allowlisted passages | Cited by |
|---|---|---|---|---|---|---|---|---|
| E1 | RFC 4506, "XDR: External Data Representation Standard", M. Eisler (ed.), May 2006; STD 67, Internet Standard; obsoletes RFC 1832 | RFC Editor, <https://www.rfc-editor.org/rfc/rfc4506.txt>, DOI 10.17487/RFC4506 | 2026-11-20 | © The Internet Society (2006), under BCP 78. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). Its examples' schemas, values and byte tables are not reused; a specification's vectors are its own. | 2026-11-20: updated by none, obsoleted by none. Errata 76 (held, editorial), 6382 and 7101 (reported): all concern the description language or typing errors; none changes an encoding rule. | Not an AFS document. Which AFS fields use which XDR type is corroborated in each protocol specification. | §§2 to 6 and §8; §7 as a cross-check of method only | [xdr.md](xdr.md) |
| E2 | The Rust standard library documentation for `std::vec::Vec` and `std::boxed::Box`, Rust 1.100.0 (the workspace's pinned toolchain) | The Rust project, <https://doc.rust-lang.org/1.100.0/std/vec/struct.Vec.html> and <https://doc.rust-lang.org/1.100.0/std/boxed/struct.Box.html>; the same text ships with the toolchain's `rust-src` component | 2026-11-20 | MIT OR Apache-2.0. Cited for facts only, and not quoted. | 2026-11-20: read for the pinned release; 2026-11-21: `Vec::into_boxed_slice` and `Box`'s conversion from a `Vec<T>` read for the same release; a toolchain bump re-checks the cited sections. | Not an AFS document, and not a source of wire facts. | `Vec`'s "Guarantees" section, `Vec::capacity`, `Vec::try_reserve_exact` and `Vec::into_boxed_slice`, and `Box`'s conversion from a `Vec<T>` into a `Box<[T; N]>`, for facts about allocation behaviour only | [xdr.md](xdr.md) |
| E3 | T. Keiser, "AFS-3 Directory Object Type Definition", Internet-Draft draft-keiser-afs3-directory-object-00, 23 April 2012; individual submission, Informational, expired 25 October 2012 | IETF Internet-Draft archive, <https://www.ietf.org/archive/id/draft-keiser-afs3-directory-object-00.txt> (SHA-256 of the text read: `93fee4f06561e815940bdebd55107e289b46d808153c00b3585f09db71de9582`); record <https://datatracker.ietf.org/doc/draft-keiser-afs3-directory-object/>. Published through the IETF, not obtained from any implementation's repository. | 2026-11-21 | © 2012 IETF Trust and the authors, under BCP 78 and the IETF Trust Legal Provisions. No licence for reuse of its prose or figures is assumed: cited for facts only, and not quoted (rule 6). Its Appendix B is a code component; nothing is taken from it. | 2026-11-21: -00 is the only revision; no replacement, no working-group adoption (datatracker). The AFS-3 standardisation group's draft list shows its comment period as pending, with no target date (<https://afs3-stds.central.org/drafts.html>). | None yet. Not self-consistent on the name hash (directory-format.md, section 13, Q5), and differs from E4 on the name octets one record holds (Q7). Treated under rule 10's discipline: every load-bearing fact is UNVERIFIED until a black-box fixture confirms it. | None: a lead only (rule 14), pending corroboration and the owner's decision (directory-format.md, section 13, Q11). Appendix B ("C Implementation of Directory Hash Function") is excluded by name: it is code, it is never read for a specification, never put in a model's prompt, and never cited. Appendix A (a worked example) is never reused as a vector. | [directory-format.md](directory-format.md) |
| E4 | OpenAFS, `fileserver(8)` reference page, section "Cautions", as served on the retrieval date (the page carries no release number) | <https://docs.openafs.org/Reference/8/fileserver.html> (SHA-256 of the page read: `61a499c142408fcd58309250b001443d1003918beb2c7858d3fee530b86804f1`). Published documentation built from an implementation's repository: assessed under rule 11; its prose is readable without opening source, headers or interface definitions. | 2026-11-21 | The page states © IBM Corporation 2000, under the IBM Public License 1.0. Cited for facts only, and not quoted (rule 6). | 2026-11-21: the current page only; earlier editions not compared. | Not an independent source for E3's layout. Its 15-octet first-record figure is a lead; its entry count is consistent with E3's page limits, which corroborates neither. | None: a lead only (rules 11 and 14) | [directory-format.md](directory-format.md) |
| E5 | "Rx protocol specification draft", Nickolai Zeldovich, updated by Jeffrey Altman, dated 24 July 2026 (no revision number; no section numbers, so it is cited by heading) | AFS Assigned Numbers Registry, <https://registrar.central.org/rx-spec.txt>; claim record row `[DRV-084]`. No reuse statement has been obtained from its editor or registrar. Read only as a copy with the section "Historical Implementation Notes" removed mechanically before reading (rule 12) | 2026-11-20 (copy retrieved); 2026-11-21 (read for [rx.md](rx.md)) | No licence notice: cited for facts only, and not quoted in a specification (rule 6) | 2026-11-21: the copy read is the revision the claim record names; no later revision was looked for, because this edition's readers do not fetch the draft; the next revision is registered as a new row | Not an older AFS document (rule 10 does not apply). Every fact taken from it is a lead under rule 14 and stays UNVERIFIED until an independently generated black-box fixture confirms it | None: the draft is a lead (rules 12 and 14), so no passage is allowlisted. Sections read: "Introduction", "Protocol Overview", "Rx Connections", "Packet Types", "Call Layer", "Debugging", "Jumbograms", "RPC Layer", "Packet Formats and Protocol Constants". **Excluded by name: "Historical Implementation Notes"**, never read or cited; also not used: the references in "Debugging" and "References" to another implementation's files and structures | [rx.md](rx.md), [vl-fs-read.md](vl-fs-read.md) |
| E6 | RFC 4291, "IP Version 6 Addressing Architecture", R. Hinden and S. Deering, February 2006; Draft Standard; obsoletes RFC 3513 | RFC Editor, <https://www.rfc-editor.org/rfc/rfc4291.txt>, DOI 10.17487/RFC4291 | 2026-11-21 | © The Internet Society (2006), under BCP 78. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6) | 2026-11-21: obsoleted by none; updated by RFC 5952, 6052, 7136, 7346, 7371 and 8064 (text form, translation prefixes, interface identifiers and multicast scopes and flags; §2.4 and §2.5.5 unchanged). Errata 3480 (verified, §2.7, interface-local multicast), 6848 (verified, appendix A), 5716 and 8373 (reported, §2.5 and §2.5.6 wording), 6596 and 1627 (held, §2.2 and §2.7): none changes the address blocks cited | Not an AFS document | §2.4, §2.5.5.1, §2.5.5.2 | [rx.md](rx.md), [dns-client.md](dns-client.md) |
| E7 | AFS Assigned Numbers Registry: the procedure-number tables `rx/VL.txt` and `rx/RXAFS.txt` (both last modified 2023-06-15), the Rx service list `rxservice.txt`, the error tables `errors.html` (last modified 2024-03-16) and `et/VL.html`, and the fileserver capability list `cap/viced.txt` (last modified 2011-04-27), as served on the retrieval date | <https://registrar.central.org/> (SHA-256 of the pages read: `rx/VL.txt` `043ee8df87732beb6e8f2ab85c5e951feca9296c1a545d7fed3d67afc2e84729`; `rx/RXAFS.txt` `08f694b5f64c847aef6447284bf24541bbf5e2b958cd9562cbf9f977029885f2`; `rxservice.txt` `4dba1763f39362e49c07f0746ccf538946fa79edbd755461c2b746bcbbede707`; `errors.html` `adba06dbff02f7f8e8355c9f5a96bd6e05fbf555bf8c5e8704ed62b3763cf9a1`; `et/VL.html` `de386652fe7cba038d171060dabab26b78cf07f56c14ca55a0ccfe65b0d10221`; `cap/viced.txt` `06a4ffd6eac5d9b00d18f225c814de62b11c65c630594e6da326491a9c2c80c7`). A registry kept by the AFS registrar, not an implementation's repository; claim record rows `[DES-011]`, `[DES-055]` and `[ADR-046]` cite it. Some table rows name an implementation's change history as their source; those rows are not used | 2026-11-21 | No licence notice: cited for facts only, and not quoted (rule 6) | 2026-11-21: the pages as served; the registry keeps no editions, so a later change is registered as a new row | Not an older AFS document. The numbers used are corroborated only by the stage C captures, pending ([vl-fs-read.md](vl-fs-read.md), section 13, Q1 and Q10) | None yet: allowlisting the procedure and error numbers is the owner's decision ([vl-fs-read.md](vl-fs-read.md), section 13, Q2). Rows whose source column names an implementation's change history are never allowlisted | [vl-fs-read.md](vl-fs-read.md) |
| E8 | T. Keiser and A. Deason (ed.), "AFS-3 Rx RPC XDR Primitive Type Definitions", Internet-Draft draft-keiser-afs3-xdr-primitive-types-01, 10 September 2012; Informational, expired 14 March 2013 | IETF Internet-Draft archive, <https://www.ietf.org/archive/id/draft-keiser-afs3-xdr-primitive-types-01.txt> (SHA-256 of the text read: `f7d3f8c0252933ae02512742c951515191a719914b4c3f99d9c6c9e33b5201bd`). Published through the IETF, not obtained from any implementation's repository | 2026-11-21 | © 2012 IETF Trust and the authors, under BCP 78 and the IETF Trust Legal Provisions. No licence for reuse of its prose or figures is assumed: cited for facts only, and not quoted (rule 6). The C structure shown in §4 carries a notice under the IBM Public License 1.0: it is excluded by name, and nothing is taken from it | 2026-11-21: -01 is the latest revision (datatracker); listed as "comment period pending" by the AFS-3 standardisation group | None yet; the UUID encoding is a lead for [vl-fs-read.md](vl-fs-read.md), section 13, Q8 | None: an expired individual draft is a lead only (rule 14), as row E5's reasoning and the directory format's question on its draft. Sections read: 1 to 4 and Appendix A. **Excluded by name:** the C structure in §4 | [vl-fs-read.md](vl-fs-read.md) |
| E9 | T. Keiser et al., "AFS-3 Protocol Capabilities Query Mechanism", Internet-Draft draft-keiser-afs3-capabilities-00, September 2012; expired 14 March 2013 | IETF Internet-Draft archive, <https://www.ietf.org/archive/id/draft-keiser-afs3-capabilities-00.txt> (SHA-256 of the text read: `d67d7162df9db6ed594eff6b6bed423b3c4f4addbe2a90cfce6ce5a26e786b0b`). Published through the IETF | 2026-11-21 | © 2012 IETF Trust and the authors, under BCP 78. Cited for facts only, and not quoted (rule 6) | 2026-11-21: -00 is the only revision (datatracker) | None. The draft says it records interfaces as one implementation built them, so its facts are leads under rule 3's spirit, never corroboration of the registry | None: a lead only (rule 14). Prose read and used: §1 to §4. **Excluded by name: Appendices A and B**, which the draft states are excerpts of an implementation's interface definitions under the IBM Public License; nothing is taken from them | [vl-fs-read.md](vl-fs-read.md) |
| E10 | OpenAFS reference pages `fs_setacl(1)`, `vos_create(1)` and `vos_examine(1)`, as served on the retrieval date (the pages carry no release number) | <https://docs.openafs.org/Reference/1/fs_setacl.html>, <https://docs.openafs.org/Reference/1/vos_create.html>, <https://docs.openafs.org/Reference/1/vos_examine.html> (SHA-256 of the pages read: `af262786da852ff0f59305b00032995ba7a3403f94740cabf8e8e1c03e7aeef9`, `b050337acea297b5eed1227598727acd6643728149e1a5eea5d517542b9da024`, `c7d4588a6a41a19a9be1a0b9d74a2f1f5efc1176a43518f15968aa812562dae8`). Published documentation built from an implementation's repository: assessed under rule 11; the prose is readable without opening source, headers or interface definitions | 2026-11-21 | The pages state they are covered by the IBM Public License 1.0. Cited for facts only, and not quoted (rule 6) | 2026-11-21: the current pages only | Behaviour leads: claim record rows `[DES-052]` and `[ADR-008]` cite pages of the same set. Each load-bearing fact waits on [vl-fs-read.md](vl-fs-read.md), section 13, Q7 and Q8 | None: a lead only (rules 11 and 14). Used: `fs_setacl(1)` on the `-acl` option and "DROPBOXES"; `vos_create(1)` on the name length; `vos_examine(1)` "OUTPUT" | [vl-fs-read.md](vl-fs-read.md) |
| E11 | OpenAFS Administration Guide (from the IBM AFS Administration Guide, revision 3.6, first IBM edition April 2000), pages "How AFS Interprets the UNIX Mode Bits" (HDRWQ580) and "About Volumes" (HDRWQ177), as served on the retrieval date | <https://docs.openafs.org/AdminGuide/HDRWQ580.html>, <https://docs.openafs.org/AdminGuide/HDRWQ177.html> (SHA-256 of the pages read: `9be64bce29798a89952366c02fd0acd37055e168704ae4a60c1efe96a46cb78b`, `394b8157900fc5d583bd0ec4ba653277654f2909b3462bf2d9aa89febf0d5565`). Published documentation; assessed under rule 11 as row E10 | 2026-11-21 | The guide's front page states © 2000 IBM Corporation, all rights reserved, and names no licence. No reuse of its text is assumed: cited for facts only, and not quoted (rule 6) | 2026-11-21: the current pages only | Claim record rows `[ADR-017]` and `[ADR-008]` cite these pages. The mode rule waits on [vl-fs-read.md](vl-fs-read.md), section 13, Q7 | None: a lead only (rules 11 and 14) | [vl-fs-read.md](vl-fs-read.md) |
| E12 | OpenAFS 1.8.1 release notes (RELNOTES-1.8.1) | <https://www.openafs.org/dl/openafs/1.8.1/RELNOTES-1.8.1> (SHA-256 of the file read: `1f82b3e11b9f1906037e43c4d59a0ab1070a6df8525913eae71a6ed1902c1998`). A release's published notes, not source; claim record row `[DES-076]` cites it | 2026-11-21 | No licence notice in the file: cited for facts only, and not quoted (rule 6) | 2026-11-21: this release's notes only | Not corroboration of any layout: it records only that one release corrected the inline bulk status reply in error cases | None: a lead only (rule 14) | [vl-fs-read.md](vl-fs-read.md) |
| E13 | RFC 1035, "Domain names - implementation and specification", P. Mockapetris, November 1987; STD 13, Internet Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc1035.txt>, DOI 10.17487/RFC1035 | 2026-11-21 | The text carries no copyright notice. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by 29 RFCs (RFC Editor metadata), of which RFC 2181, 2308, 4343, 7766 and 9619 are registered here (E15, E16, E20, E27, E29) and the rest concern zone transfer, update, notify, DNSSEC, IQUERY or the master file, none used. Errata: 2130 (verified, technical) makes §3.2.1's TTL unsigned, as §4.1.3 already says and E15 §8 settles; 8101 (held) adds an example to §4.1.2; 1813, 3421, 5626 (held, technical) concern the master file; the rest are editorial. | Not an AFS document. | §2.3.2, §2.3.4, §3.1, §3.2.4, §3.3.1, §3.3.13, §3.4.1, §4.1 to §4.1.4, §4.2, §4.2.1, §4.2.2 | [dns-client.md](dns-client.md) |
| E14 | RFC 1183, "New DNS RR Definitions", C. Everhart, L. Mamakos, R. Ullmann, P. Mockapetris (ed.), October 1990; Experimental | RFC Editor, <https://www.rfc-editor.org/rfc/rfc1183.txt>, DOI 10.17487/RFC1183 | 2026-11-21 | No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by RFC 5395, 5864, 6195 and 6895 (RFC 5864 is E22; the others are IANA procedures). Erratum 1478 (held, editorial, §2, not used). | Defines the AFSDB record. Its AFS meaning is corroborated by E22 §5 and claim record row `[DES-008]`. | §1 only | [dns-client.md](dns-client.md) |
| E15 | RFC 2181, "Clarifications to the DNS Specification", R. Elz, R. Bush, July 1997; Proposed Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc2181.txt>, DOI 10.17487/RFC2181 | 2026-11-21 | The text carries no copyright notice. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by RFC 2535, 4033 to 4035 (DNSSEC), 4343 (E20), 5452 (E21) and 8767 (serving stale data, not used). Erratum 6446 rejected. | Not an AFS document. | §5.2, §8, §9, §11 | [dns-client.md](dns-client.md) |
| E16 | RFC 2308, "Negative Caching of DNS Queries (DNS NCACHE)", M. Andrews, March 1998; Proposed Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc2308.txt>, DOI 10.17487/RFC2308 | 2026-11-21 | © The Internet Society (1998), under BCP 78. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by RFC 4033 to 4035 (DNSSEC), 6604, 8020, 8499 and 9499 (terminology) and 9520 (E35). Errata 461 and 4489 (verified) and 4627, 4628, 4632, 4983 (held) are editorial. | Not an AFS document. | §2.1, §2.2, §2.2.1, §3, §5, §7.2 | [dns-client.md](dns-client.md) |
| E17 | RFC 2782, "A DNS RR for specifying the location of services (DNS SRV)", A. Gulbrandsen, P. Vixie, L. Esibov, February 2000; Proposed Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc2782.txt>, DOI 10.17487/RFC2782 | 2026-11-21 | © The Internet Society (2000), under BCP 78. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by RFC 6335 and 8553 (service-name and underscore registries; no change to the record). Errata 6600 (verified, editorial) and 8120 (held, technical) are in the fictional example, not used; 2984 rejected. | Not an AFS document. Its use for AFS is RFC 5864 (E22); claim record row `[DES-009]`. | the sections "The format of the SRV RR" and "Usage rules" | [dns-client.md](dns-client.md) |
| E18 | RFC 3596, "DNS Extensions to Support IP Version 6", S. Thomson, C. Huitema, V. Ksinant, M. Souissi, October 2003; Internet Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc3596.txt>, DOI 10.17487/RFC3596 | 2026-11-21 | © The Internet Society (2003), under BCP 78. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by none; no errata. | Not an AFS document. | §2.1, §2.2 | [dns-client.md](dns-client.md) |
| E19 | RFC 3597, "Handling of Unknown DNS Resource Record (RR) Types", A. Gustafsson, September 2003; Proposed Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc3597.txt>, DOI 10.17487/RFC3597 | 2026-11-21 | © The Internet Society (2003), under BCP 78. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by RFC 4033 to 4035, 5395, 6195 and 6895 (DNSSEC and IANA procedures; §4 unchanged). | Not an AFS document. | §4 | [dns-client.md](dns-client.md) |
| E20 | RFC 4343, "Domain Name System (DNS) Case Insensitivity Clarification", D. Eastlake 3rd, January 2006; Proposed Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc4343.txt>, DOI 10.17487/RFC4343 | 2026-11-21 | © The Internet Society (2006), under BCP 78. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by RFC 5890 (internationalised names, not used). | Not an AFS document. | §3, §4 | [dns-client.md](dns-client.md) |
| E21 | RFC 5452, "Measures for Making DNS More Resilient against Forged Answers", A. Hubert, R. van Mook, January 2009; Proposed Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc5452.txt>, DOI 10.17487/RFC5452 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2009), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by none; no errata. | Not an AFS document. | §3, §5, §6, §9.1 to §9.3 | [dns-client.md](dns-client.md) |
| E22 | RFC 5864, "DNS SRV Resource Records for AFS", R. Allbery, April 2010; Proposed Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc5864.txt>, DOI 10.17487/RFC5864 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2010), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by RFC 8553 (underscore registry). Errata: 2260 (verified, editorial, §4, wording only); 2767 (held, technical) corrects the §6 example, not used. | A published standard; claim record rows `[DES-008]` and `[DES-010]`. | §2, §4, §4.1, §5, §7 | [dns-client.md](dns-client.md) |
| E23 | RFC 6761, "Special-Use Domain Names", S. Cheshire, M. Krochmal, February 2013; Proposed Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc6761.txt>, DOI 10.17487/RFC6761 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2013), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by none. | Not an AFS document. | §6.2 | [dns-client.md](dns-client.md) |
| E24 | RFC 6840, "Clarifications and Implementation Notes for DNS Security (DNSSEC)", S. Weiler (ed.), D. Blacka (ed.), February 2013; Proposed Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc6840.txt>, DOI 10.17487/RFC6840 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2013), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by RFC 8749 (DLV, not used). | Not an AFS document. | §5.7, §5.8 | [dns-client.md](dns-client.md) |
| E25 | RFC 6891, "Extension Mechanisms for DNS (EDNS(0))", J. Damas, M. Graff, P. Vixie, April 2013; STD 75, Internet Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc6891.txt>, DOI 10.17487/RFC6891 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2013), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by none. Errata: 3604 (verified, editorial); 8348 (held, technical) renames the CLASS field of §6.1.2 to the sender's payload size, which this register's citing specification follows; 6982 rejected. | Not an AFS document. | §6.1.1 to §6.1.4, §6.2.2, §6.2.3, §6.2.5, §7, §9 | [dns-client.md](dns-client.md) |
| E26 | RFC 6895, "Domain Name System (DNS) IANA Considerations", D. Eastlake 3rd, April 2013; BCP 42 | RFC Editor, <https://www.rfc-editor.org/rfc/rfc6895.txt>, DOI 10.17487/RFC6895 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2013), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by none. | Not an AFS document. | §2, §2.1, §2.2 | [dns-client.md](dns-client.md) |
| E27 | RFC 7766, "DNS Transport over TCP - Implementation Requirements", J. Dickinson, S. Dickinson, R. Bellis, A. Mankin, D. Wessels, March 2016; Proposed Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc7766.txt>, DOI 10.17487/RFC7766 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2016), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by RFC 8490 and 9103 (stateful operations and zone transfer over TLS, not used); no errata. | Not an AFS document. | §5, §6.2.1, §6.2.3, §7, §8 | [dns-client.md](dns-client.md) |
| E28 | RFC 8200, "Internet Protocol, Version 6 (IPv6) Specification", S. Deering, R. Hinden, July 2017; STD 86, Internet Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc8200.txt>, DOI 10.17487/RFC8200 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2017), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by RFC 9673 (hop-by-hop options; §5 unchanged). | Not an AFS document. | §5 | [dns-client.md](dns-client.md) |
| E29 | RFC 9619, "In the DNS, QDCOUNT Is (Usually) One", R. Bellis, J. Abley, July 2024; Proposed Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc9619.txt>, DOI 10.17487/RFC9619 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2024), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by none. | Not an AFS document. | §4 | [dns-client.md](dns-client.md) |
| E30 | RFC 9715, "IP Fragmentation Avoidance in DNS over UDP", K. Fujiwara, P. Vixie, January 2025; Informational | RFC Editor, <https://www.rfc-editor.org/rfc/rfc9715.txt>, DOI 10.17487/RFC9715 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2025), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by none; no errata. | Not an AFS document. | §3.2, §5 | [dns-client.md](dns-client.md) |
| E31 | "Use of Bit 0x20 in DNS Labels to Improve Transaction Identity", P. Vixie, D. Dagon, draft-vixie-dnsext-dns0x20-00, March 2008; an Internet-Draft, expired September 2008 | IETF archive, <https://www.ietf.org/archive/id/draft-vixie-dnsext-dns0x20-00.txt> | 2026-11-21 | © The IETF Trust (2008). No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: no later revision and no RFC found; it is a work in progress and is cited only for the method, never as a requirement on peers. | Not an AFS document. Whether a given resolver keeps letter case is an observation (the citing specification's 12.1). | §5.1, §5.4, §5.5, §6.3 | [dns-client.md](dns-client.md) |
| E32 | resolv.conf(5), Linux man-pages project, release 6.19, page dated 2026-08-22 | man7.org, <https://man7.org/linux/man-pages/man5/resolv.conf.5.html> (rendered 2026-09-09 from the man-pages 6.19 tarball) | 2026-11-21 | No licence for reuse of its text is assumed: cited for facts only, and not quoted. | 2026-11-21: read for this release; a new man-pages release re-checks the cited parts. | Not an AFS document, and not a source of wire facts: it documents the resolver configuration file's format. | DESCRIPTION, the `nameserver`, `search` and `options` entries, the comment and line rules, SYNOPSIS | [dns-client.md](dns-client.md) |
| E33 | systemd-resolved.service(8), systemd 262 | freedesktop.org, <https://www.freedesktop.org/software/systemd/man/latest/systemd-resolved.service.html> | 2026-11-21 | No licence for reuse of its text is assumed: cited for facts only, and not quoted. | 2026-11-21: the "latest" page, for systemd 262; a supported release's own page is checked by the observation of the citing specification's 12.1. | Not an AFS document. Claim record row `[DES-002]`. | the description of the stub listener, and "/etc/resolv.conf" (the four modes) | [dns-client.md](dns-client.md) |
| E34 | NetworkManager.conf(5), NetworkManager 1.58.0 | networkmanager.dev, <https://networkmanager.dev/docs/api/latest/NetworkManager.conf.html> | 2026-11-21 | No licence for reuse of its text is assumed: cited for facts only, and not quoted. | 2026-11-21: the "latest" page, for 1.58.0. Red Hat's own dnsconfd page (claim record row `[DES-007]`) refused the request on 2026-11-21. | Not an AFS document. | the `dns` key of the `[main]` section (its `dnsmasq`, `systemd-resolved` and `dnsconfd` values and the note on caching local name servers) | [dns-client.md](dns-client.md) |
| E35 | RFC 9520, "Negative Caching of DNS Resolution Failures", D. Wessels, W. Carroll, M. Thomas, December 2023; Proposed Standard | RFC Editor, <https://www.rfc-editor.org/rfc/rfc9520.txt>, DOI 10.17487/RFC9520 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2023), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by none. | Not an AFS document. | §3.1, §3.2 | [dns-client.md](dns-client.md) |
| E36 | RFC 6890, "Special-Purpose IP Address Registries", M. Cotton, L. Vegoda, R. Bonica (ed.), B. Haberman, April 2013; BCP 153; obsoletes RFC 4773, 5156, 5735 and 5736 | RFC Editor, <https://www.rfc-editor.org/rfc/rfc6890.txt>, DOI 10.17487/RFC6890 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2013), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by RFC 8190, which renames "Global" and changes the limited broadcast block's "Reserved-by-Protocol" and the TEREDO row; no "Destination" value cited here changes. Errata 6404 (verified) splits 0.0.0.0/8 into "this network" and 0.0.0.0/32, both still not a destination; 3921 (verified) is the TEREDO row; 6881 and 6882 (verified, editorial). | Not an AFS document. | §2.2.2 and §2.2.3: the rows for 0.0.0.0/8, 127.0.0.0/8, 169.254.0.0/16, 240.0.0.0/4, 255.255.255.255/32 and ::ffff:0:0/96 | [dns-client.md](dns-client.md) |
| E37 | RFC 5771, "IANA Guidelines for IPv4 Multicast Address Assignments", M. Cotton, L. Vegoda, D. Meyer, March 2010; BCP 51 | RFC Editor, <https://www.rfc-editor.org/rfc/rfc5771.txt>, DOI 10.17487/RFC5771 | 2026-11-21 | © IETF Trust and the persons identified as the document authors (2010), under BCP 78 and the IETF Trust's Legal Provisions. No licence for reuse of its text is assumed: cited for facts only, and not quoted in a specification (rule 6). | 2026-11-21: obsoleted by none; updated by none. Errata 2733 and 2734 (held, editorial) concern reference names. | Not an AFS document. | §3, the table of the IPv4 multicast space (224.0.0.0 to 239.255.255.255) | [dns-client.md](dns-client.md) |
| E38 | nextest documentation, "Machine-readable output: test lists" (current, unversioned page) | The nextest project, <https://nexte.st/docs/machine-readable/list/> | 2026-11-21 | None assumed: cited for facts only, and not quoted. | 2026-11-21: the page says a JSON schema for the list is planned and not yet published; re-read when the workspace's pinned nextest version changes. | Not an AFS document, and not a source of wire facts. | The whole page, for facts about the list's command and fields only | [testing.md](testing.md) |
| E39 | The rustc book, "Instrumentation-based Code Coverage", Rust 1.100.0 (the workspace's pinned toolchain) | The Rust project, <https://doc.rust-lang.org/1.100.0/rustc/instrument-coverage.html> | 2026-11-21 | MIT OR Apache-2.0. Cited for facts only, and not quoted. | 2026-11-21: read for the pinned release; a toolchain bump re-checks it. | Not an AFS document, and not a source of wire facts. | The statistics a report gives, and the profiler runtime and LLVM tools it needs | [testing.md](testing.md) |
| E40 | cargo-llvm-cov 0.9.1: its README at the tag `v0.9.1` and its release page | The cargo-llvm-cov project, <https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.1/README.md>, <https://github.com/taiki-e/cargo-llvm-cov/releases/tag/v0.9.1> | 2026-11-21 | Apache-2.0 OR MIT, as the project states. Cited for facts only, and not quoted. | 2026-11-21: the README read is the one at the pinned release's tag. | Not an AFS document, and not a source of wire facts. | JSON export; running tests through nextest; the default report exclusions; excluding files by a file-name pattern; branch and doc-test coverage needing a nightly compiler; prebuilt binaries | [testing.md](testing.md) |
| E41 | cargo-mutants user guide, pages "nextest", "Sharding", "mutants.out", "Filtering functions and mutants" and "Hangs and timeouts" (current, unversioned pages) | The cargo-mutants project, <https://mutants.rs/nextest.html>, <https://mutants.rs/shards.html>, <https://mutants.rs/mutants-out.html>, <https://mutants.rs/filter_mutants.html>, <https://mutants.rs/timeouts.html> | 2026-11-21 | None assumed for the guide: cited for facts only, and not quoted. The tool is MIT-licensed, as its repository states. | 2026-11-21: the issue that pins the tool re-reads the pages for the pinned version. | Not an AFS document, and not a source of wire facts. | Selecting nextest; `--shard`, and its use with `--in-diff`; the four outcome files; name filters in the configuration file; what happens to a mutant whose tests time out | [testing.md](testing.md) |
| E42 | GitHub Docs, "Events that trigger workflows" (current page) | GitHub, <https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows> | 2026-11-21 | None assumed: cited for facts only, and not quoted. | 2026-11-21: a current page; re-read by the issue that builds the scheduled workflows. | Not an AFS document, and not a source of wire facts. | The `schedule` section | [testing.md](testing.md) |
| E43 | GitHub Docs, "Dependency caching reference" (current page) | GitHub, <https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching> | 2026-11-21 | None assumed: cited for facts only, and not quoted. | 2026-11-21: a current page; re-read by the issue that builds the scheduled workflows. | Not an AFS document, and not a source of wire facts. | Restrictions for accessing a cache, including the caches a pull request's run can restore | [testing.md](testing.md) |
| E44 | LLVM documentation, "libFuzzer – a library for coverage-guided fuzz testing" (current page) | The LLVM project, <https://llvm.org/docs/LibFuzzer.html> | 2026-11-21 | None assumed: cited for facts only, and not quoted. | 2026-11-21: a current page; re-read by the issue that adopts the fuzzing engine's binding (O47, point 5). | Not an AFS document, and not a source of wire facts. | The options `-max_len`, `-max_total_time`, `-merge` and `-artifact_prefix`, and where crash inputs are written | [testing.md](testing.md) |
| E45 | nextest documentation, pages "Test groups for mutual exclusion", "Slow tests and timeouts" and "Retries and flaky tests" (current, unversioned pages) | The nextest project, <https://nexte.st/docs/configuration/test-groups/>, <https://nexte.st/docs/features/slow-tests/>, <https://nexte.st/docs/features/retries/> | 2026-11-21 | None assumed: cited for facts only, and not quoted. | 2026-11-21: re-read when the workspace's pinned nextest version changes. | Not an AFS document, and not a source of wire facts. | A group's thread limit; terminating a test after a number of slow periods, per test; where retries are set (command line, environment variable, profile, per-test overrides) | [testing.md](testing.md) |
| E46 | GitHub Docs, "Control the concurrency of workflows and jobs" (current page) | GitHub, <https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency> | 2026-11-21 | None assumed: cited for facts only, and not quoted. | 2026-11-21: a current page; re-read by the issue that builds the scheduled workflows. | Not an AFS document, and not a source of wire facts. | What `cancel-in-progress` does, and what happens without it | [testing.md](testing.md) |
| E47 | GitHub REST API documentation, "List workflow runs for a workflow", API version 2022-11-28 | GitHub, <https://docs.github.com/en/rest/actions/workflow-runs?apiVersion=2022-11-28#list-workflow-runs-for-a-workflow> | 2026-11-21 | None assumed: cited for facts only, and not quoted. | 2026-11-21: re-read by the issue that builds the freshness check. | Not an AFS document, and not a source of wire facts. | The page size limit, the 1,000-result limit of a filtered search, and the run's attempt number | [testing.md](testing.md) |
| E48 | Microsoft Learn, "CreateNamedPipeW function", page dated 2022-08-05 (updated 2024-11-20) | Microsoft, <https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew> | 2026-11-21 | None assumed: cited for facts only, not quoted (rule 6) | 2026-11-21: the live page is the current edition | Not an AFS document; a platform fact, checked again at the Windows gate | Parameters `dwPipeMode` and `nMaxInstances`; Remarks | [local-ipc.md](local-ipc.md), [local-ipc-windows.md](local-ipc-windows.md) |
| E49 | Microsoft Learn, "ImpersonateNamedPipeClient function", page dated 2018-12-05 (updated 2025-07-01) | Microsoft, <https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-impersonatenamedpipeclient> | 2026-11-21 | None assumed: facts only, not quoted | 2026-11-21: the live page is the current edition | Not an AFS document; the Windows gate runs the sequence | Remarks | [local-ipc.md](local-ipc.md), [local-ipc-windows.md](local-ipc-windows.md) |
| E50 | Microsoft Learn, "System Error Codes (1300-1699)", page dated 2025-07-14 | Microsoft, <https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--1300-1699-> | 2026-11-21 | None assumed: facts only, not quoted | 2026-11-21: the live page is the current edition | Not an AFS document; the Windows gate records the error | The entry for error 1368 | [local-ipc.md](local-ipc.md), [local-ipc-windows.md](local-ipc-windows.md) |
| E51 | unix(7), Linux man-pages 6.19, page dated 2026-02-08 | The Linux man-pages project, <https://man7.org/linux/man-pages/man7/unix.7.html> | 2026-11-21 | None assumed: facts only, not quoted | 2026-11-21: 6.19 is the edition served | Not an AFS document | Socket option `SO_PEERCRED` | [local-ipc.md](local-ipc.md) |
| E52 | unix(4), macOS 26.7 | A manual-page mirror, <https://man.freebsd.org/cgi/man.cgi?query=unix&sektion=4&manpath=macOS+26.7>, not Apple's own site | 2026-11-21 | None assumed: facts only, not quoted | 2026-11-21: the macOS 26.7 page; the macOS gate checks the behaviour on each supported release | Not an AFS document; the macOS gate observes it | `LOCAL_PEERCRED`; the list of supported socket types | [local-ipc.md](local-ipc.md) |
| E53 | FIPS 180-4, "Secure Hash Standard (SHS)", NIST, August 2015 | NIST, <https://doi.org/10.6028/NIST.FIPS.180-4> | 2026-11-21 | None assumed: facts only, not quoted | 2026-11-21: in force, not withdrawn; NIST announced a revision in 2023, not yet published | Not an AFS document | Section 1, Figure 1 (digest sizes) | [local-ipc.md](local-ipc.md) |

## Formats: versions and freezing

These rules apply to the kinds `protocol` and `format`. The
`wire-format` skill (`.claude/skills/wire-format/SKILL.md`) and coding
standards rules 1 to 3 and 9 apply with them.

### What every format specification states

1. **Layout:** every field, in order, with its type, width, byte order and
   alignment or padding.
2. **Caps:** a hard, named cap for every length and count read from input,
   checked before any allocation or loop. Limits are in bytes.
3. **Tolerance, per field:** unknown flags, extra padding, future enum
   values, trailing bytes. Each is accepted, ignored or rejected, with the
   reason. Tolerance is a decision, never a default.
4. **Canonical form:** the one encoding the encoder produces. The encoder
   refuses what the decoder rejects.
5. **Reserved fields:** their required value on encode, and what the
   decoder does with any other value. A reserved field is never
   repurposed.
6. **Integrity:** what is checked, over which received bytes, and before
   which use of the content.
7. **Arithmetic:** every sum, product and offset on untrusted values, and
   what happens at overflow.
8. **Errors:** the result of each rejection, with no panic reachable.

### Formats IronAFS defines

A format that a later version must read:

- starts with a magic value and a version;
- states which versions the current code reads and which it writes;
- states, per older version, whether it is decoded, migrated, or dropped
  and rebuilt, and what a user loses in each case;
- states what the code does on a version newer than it knows: it refuses
  and leaves the bytes untouched, unless the specification says otherwise;
- says how it can be extended without a new version (reserved space,
  skippable records), if it can.

### Freezing

- A format is frozen when the first release that writes it ships, or
  earlier when another party starts to depend on it. Until then it is
  `reviewed` and may still change.
- Freezing is class R. It needs everything class S needs, and the owner's
  decision. The review covers the whole format, not the last diff.
- Before a freeze: the golden vectors, the reject corpus and the fuzz
  target exist and pass; every Open question that touches the layout is
  closed; and the row for the frozen formats table is in the same pull
  request.
- After a freeze, the frozen version's layout, caps on what is accepted,
  and tolerance never change. A defect is fixed by a new version. The one
  exception is a change that only rejects input the frozen text already
  called invalid.
- A new version is a new section beside the old one. The old section
  stays, marked with the versions that used it.
- A correction to wording that changes no byte and no accepted input is
  class E, and its pull request says why no byte changes.

### Formats IronAFS does not control

A `protocol` specification describes a peer's format. IronAFS cannot
freeze it. Its status stays `reviewed`, and it records per field what
IronAFS sends and what it accepts. A change in what IronAFS sends or
accepts is class S.

## Test vectors and fuzz targets

Every `protocol` and `format` specification has a Tests section that
names, with paths:

1. **Golden vectors** with independent expected bytes. They are derived by
   hand from the specification, or from independently generated
   black-box captures (Sources, rule 4) reduced to what the test needs,
   with synthetic identities and keys, inspected before they are added.
   Each vector states its derivation or its fixture provenance. Never
   from another implementation's test files, and never a standard's
   example values or byte tables reused as the vector itself (they may
   cross-check the method).
2. **Snapshots made by IronAFS's own encoder** are regression tests only.
   They are kept apart from golden vectors and are never cited as evidence
   of interoperability.
3. **Round-trip property tests,** where an encoder exists: every accepted
   value survives encode then decode, and every canonical input survives
   decode then encode. Accepted non-canonical inputs are tested for their
   decoded value and their canonical re-encoding.
4. **A reject corpus:** inputs that must fail, each with the reason and
   the section it breaks.
5. **A fuzz target** for each decoder, in the top-level `fuzz/` crate
   (coding standards rule 20), with the invariants it asserts.
6. **Invariant tests:** one named test per quoted ADR invariant.

A `behaviour` specification names its acceptance cases instead: the
start state, the events in order, and the result that must be seen. A
`procedure` specification names how each step is checked.

A requirement with no test is listed as a gap, with its issue. A frozen
format has no gaps.

### Published conformance corpora

A published conformance corpus is a set of test vectors that a standards
body or a conformance project publishes for others to test their code
against. For mainstream cryptography, and for nothing else, IronAFS runs
such corpora as a separate conformance layer, apart from every
specification's golden vectors and reject corpus. A corpus that does not
meet every rule below stays out of the tree. Widening these rules beyond
mainstream cryptography is the owner's decision, in a pull request of its
own.

1. **What qualifies.** A corpus qualifies only if all of these hold:
   - **Mainstream cryptography.** It tests an algorithm that coding
     standards rule 21 and ADR 0004, C5, treat as mainstream cryptography
     (AES, SHA-2, HMAC and the like), which IronAFS implements or puts
     behind `ironafs-crypto`. rxkad's cipher is not mainstream (ADR 0004,
     C5) and has no conformance corpus. Vectors for XDR, Rx, the AFS-3
     RPCs, callbacks, SMB, FUSE or a format IronAFS defines never qualify,
     whoever published them: those are tested only as the list above sets
     out.
   - **Published as conformance material.** A standards body publishes it
     (NIST's CAVP and ACVP vectors, an RFC's test cases), or a project
     whose purpose is to publish vector sets for others to test against
     (Project Wycheproof), at a stable locator.
   - **Not from an implementation's source.** Vectors taken from an
     implementation's source tree, its tests or its test data are not a
     published corpus, whatever their licence. Nothing from OpenAFS,
     AuriStor, or the Linux kernel's AF_RXRPC or kafs ever qualifies, nor
     anything generated from them (ADR 0001; Sources, rule 3).
   - **Its licence is settled** (point 2).
2. **Licence and provenance, before the files enter the tree.** The pull
   request that adds a corpus records, in the same commit as the files:
   - **The terms, read at their source:** the locator and the date read,
     and a verdict. CONFIRMED: the publisher states terms that cover these
     files, quoted or linked. UNVERIFIED: no stated terms are found, or
     they may not cover the files (for example a document licence that
     may not reach values extracted from it).
   - **The licence is on ADR 0001's allowlist and compatible with
     GPL-3.0-or-later.** Anything else, an UNVERIFIED verdict included, is
     the owner's decision for that corpus, recorded with its date in the
     provenance row; until then the corpus does not enter (ADR 0001,
     "Third-party material that is copied in").
   - **The licence text** in `LICENSES/`, under its SPDX id, or under a
     `LicenseRef-` id holding the publisher's own statement where SPDX
     has none. For a corpus the owner admits with an UNVERIFIED verdict,
     the `LicenseRef-` file holds what was found: the publisher's
     statement as published, or a line saying that none was found, with
     the locators read and the date, and the owner's decision with its
     date.
   - **A `REUSE.toml` annotation** over the corpus's files and their
     notices (not `SOURCE.md`, which carries its own header), with the
     publisher's copyright or public-domain statement, or the publisher's
     name where it states none, and that licence id, so that the files
     stay unmodified and `reuse lint` covers them.
   - **A `docs/provenance.md` row:** the IronAFS path, the source pinned
     as point 3 sets out, the licence, and what was taken.
   - **Every notice the licence requires** (an Apache-2.0 `NOTICE` file,
     an attribution the publisher asks for), kept beside the files and
     carried into the release notices file (ADR 0001, "Releases").
   - **A published crate** either leaves the corpus, and the conformance
     tests that read it, out of its package, or declares the combined
     licence expression (ADR 0001).

   Missing or unresolved provenance blocks the merge (ADR 0001).
3. **Pinned.** A corpus is one exact published version, never "the
   latest".
   - **Its place:** `crates/<crate>/tests/conformance/<publisher>-<set>/`
     in the crate whose tests run it, one directory per corpus. It is not
     under `tests/fixtures/`: coding standards rule 15 keeps fixtures
     synthetic or reduced, and a corpus is neither.
   - **Unmodified:** files published as files are copied byte for byte. A
     whole file may be left out of an archive; no file is edited, trimmed,
     reformatted or given a header. Test cases published only inside a
     document, such as an RFC's, are transcribed into one data file whose
     values are those of the document, with the document, its sections and
     the errata checked named in the manifest. A second, independent
     reading of every value against the document is made before the
     merge: who or what made it and its result are recorded in the
     manifest and in the change's review record in `docs/reviews/`.
   - **A manifest,** `SOURCE.md` in the directory (IronAFS's own file,
     with its own SPDX header), records: the publisher and the set's name;
     its version, release or commit; the retrieval locator and date; the
     SHA-256 of what was fetched (the archive, or the document a
     transcription was read from) and of each file kept or transcribed;
     each file left out and why; the number of vectors per file that the
     tests expect to run and to skip; the second reading of a
     transcription; and the licence verdict and provenance row.
   - **A new version** is a pull request of its own: the new files,
     digests, manifest and provenance row together, and the old ones
     removed. Point 2 is done again for the new version: its terms read
     again, its notices checked, and any owner's decision taken again for
     that version. Nothing fetches a corpus at build or test time.
4. **Cited.** A specification names each corpus in its Tests section,
   under a heading of its own after the reject corpus (after the
   acceptance cases, in a specification that has no reject corpus): the
   directory, the manifest, the provenance row, and the requirements the
   corpus exercises.
   - A corpus is never a specification's golden vector, never the
     derivation of one, and never the only check of a requirement. Point 1
     of the list above stands: a standard's example values are not reused
     as golden vectors.
   - A corpus is not a source of requirements: it is not in the Sources
     table, and no point is labelled CONFIRMED from it. It is never cited
     as evidence of interoperability with an AFS peer.
   - The document that publishes a corpus (an RFC, a NIST publication) is
     in the register of editions when a specification cites it for facts,
     as any other edition is.
5. **How tests use it.**
   - Conformance tests are integration tests of the owning crate, in
     `tests/it/<subject>.rs` (coding standards rule 20), named for the
     corpus. They read the files from their stable path, and run in the
     normal test run, offline and deterministic (coding standards rule
     15).
   - The test checks each vector file's SHA-256 against the manifest
     before it reads the file, so a file edited in place fails.
   - Every vector in every vector file kept is run. A vector that the
     interface under test cannot express (for example a verification
     case, where the crate has no verification interface) is skipped by a
     rule written in the test, with its reason. The test counts the
     vectors it ran and skipped, per file, and fails unless both counts
     match the manifest, so an empty, truncated or misread file fails
     instead of passing.
   - A vector whose expected result the corpus leaves to the
     implementation's policy (Project Wycheproof's `acceptable`) is run,
     and the test asserts the one result that IronAFS's specification
     requires for it, by a rule written in the test that cites the
     specification's point. It never passes on either result.
   - A vector file that does not parse fails the test. A corpus file is
     never edited to make a test pass: a mismatch is a defect in IronAFS,
     or a question about the corpus raised as an issue.
   - The code that reads a corpus is test code: in the crate's
     `tests/it/support.rs`, or in `ironafs-test-support` once a second
     crate needs it (rule 20).
   - The specification's own tests (its golden vectors, property tests,
     acceptance cases and any independent vectors it names) check every
     requirement a corpus exercises, so the conformance layer can be
     removed without leaving a requirement untested.

## Review class per kind

The class is the highest that fits ([docs/workflow.md](../workflow.md)
§1). The table gives the least class for each kind; the issue that raises
a specification carries at least that class, raised to the highest
plausible consequence. The specification's second line states its class
and why. A specification that mixes parts (for example a configuration
specification with a public schema, an untrusted decoder and a durable
record) takes the class of its highest part, or is split.

| Specification | Least class | Why |
|---|---|---|
| A `protocol` specification, a protocol or security-mechanism specification, or any decoder of untrusted input | S | Hostile or broken peers send this data |
| A `format` specification, not yet frozen | S | A mistake loses data or serves it wrong |
| Freezing a format, or changing a frozen one | R | Users' data depends on it from then on |
| A `behaviour` specification touching tokens, authorisation, callbacks, the cache, locking, concurrency or shutdown | S | The mistake is a security or consistency failure |
| Any other `behaviour` specification | P | It sets a contract a user, admin or peer sees |
| A `procedure` specification on trust, keys or updates | S | A mistake installs or trusts the wrong thing |
| Any other `procedure` specification | P | Admins follow it |
| A wording fix that changes no requirement | E | Nothing the project claims changes |

The seats for each class are in [docs/workflow.md](../workflow.md) §2. A
specification is reviewed before the code written from it, and the review
record goes in `docs/reviews/`. A specification's own file carries no
review rounds, only a link to its record.

## What every specification contains

Start from [template.md](template.md). House style: short sentences, plain
words, the present tense. A specification describes the thing as it must
be. It carries no development history, no task ids other than a public
issue in an open question, and no reviewer or model attribution.

## The index

The table at the top lists every specification, in alphabetical order by
file name. A specification's pull request adds or updates its row.

- **Spec:** the title, linked to the file.
- **Kind:** protocol, format, behaviour or procedure.
- **Class:** the class it was reviewed at.
- **Status:** as on its status line, with the date.
- **Implements:** the ADR decisions and specification items it carries
  out.

The frozen formats table gains a row in the pull request that freezes a
format. The magic value and version in the row are the bytes in the
specification, and a check compares them.
