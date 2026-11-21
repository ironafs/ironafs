<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Specifications

A specification says exactly how one part of IronAFS behaves, or how one
format is laid out, in enough detail to implement and test it. Code is
written from the specification, and tests check the code against it.

| Spec | Kind | Class | Status | Implements |
|---|---|---|---|---|
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
  the format (coding standards rule 15), not in `docs/specs/`. The
  specification names their paths.

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
   - That procedure is never used for OpenAFS, AuriStor, or the Linux
     kernel's AF_RXRPC or kafs: their source is not read, by people or by
     models, and `AGENTS.md`'s stricter wording ("nothing generated from
     their files") applies. A protocol fact that no other admissible
     source answers stays MISSING until observation settles it.
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
    another implementation's code: it is excluded by name. It is never
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
| E2 | The Rust standard library documentation for `std::vec::Vec`, Rust 1.100.0 (the workspace's pinned toolchain) | The Rust project, <https://doc.rust-lang.org/1.100.0/std/vec/struct.Vec.html>; the same text ships with the toolchain's `rust-src` component | 2026-11-20 | MIT OR Apache-2.0. Cited for facts only, and not quoted. | 2026-11-20: read for the pinned release; a toolchain bump re-checks the cited sections. | Not an AFS document, and not a source of wire facts. | The type's "Guarantees" section, `Vec::capacity` and `Vec::try_reserve_exact`, for facts about allocation behaviour only | [xdr.md](xdr.md) |

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

**Published conformance corpora.** For mainstream
cryptography, published test vector sets, such as NIST's CAVP and ACVTS
files, Wycheproof's vectors and an RFC's test cases, may be run as a
separate conformance layer, apart from every specification's golden
vectors. Each set is used under its own licence, checked and recorded
with a `docs/provenance.md` row before it is added, and kept in its own
directory under the owning crate's tests, unmodified, with its source and
version. A conformance set is never a specification's golden vector,
never the only check of a requirement, and never cited as the derivation
of one; point 1's rule against reusing a standard's example values as
golden vectors stands.

A `behaviour` specification names its acceptance cases instead: the
start state, the events in order, and the result that must be seen. A
`procedure` specification names how each step is checked.

A requirement with no test is listed as a gap, with its issue. A frozen
format has no gaps.

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
