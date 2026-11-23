<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Title: the thing specified, in a few words

Status: draft (YYYY-MM-DD). Raised in issue #NNN.

Kind: protocol, format, behaviour or procedure. Class: the letter, and one
sentence saying why ([README](README.md), "Review class per kind").

Sections keep their numbers once the specification is reviewed. A section
that does not apply stays, with one line saying why.

## 1. What this implements

List each ADR decision and specification item by id and point. Quote each
invariant it must keep, word for word, and mark it firm or working.

## 2. Scope

Say what this specification covers, what it leaves out, and which
specification covers each part left out.

## 3. The design in short

Give the whole mechanism in a few points, so a reader knows the shape
before the detail.

## 4. Terms

Define each term used with a special meaning, once, and use it the same
way throughout.

## 5. Specification

State the requirements as numbered points under subsections (5.1, 5.2),
each one checkable, each fact about a peer or platform with its source.
Label every point as [README](README.md), "Labelling claims", sets out:
`[X1 §4.10: CONFIRMED]`, `[ADR 0004 C6.1: Firm]` or `[OWN]` with its
reason.

### 5.1 Layout

Protocols and formats: list every field in order, with type, width, byte
order and padding.

### 5.2 Caps

Protocols and formats: name the hard cap, in bytes, for every length and
count read from input.

### 5.3 Versions

Formats IronAFS defines: give the magic value, the versions read and
written, and what happens to each older and each newer version.

### 5.4 Decoding

Give the checks in order, the tolerance decided for each field, what is
done with trailing bytes, and what integrity is verified over which bytes.

### 5.5 Encoding

Give the canonical form, the values the encoder refuses, and the required
value of each reserved field.

### 5.6 States and ordering

Behaviour: give each state, each event, the order events must take, and
what holds at every step.

## 6. Arithmetic

List every sum, product and offset on untrusted values, with its bound and
the result at overflow.

## 7. Errors and failure

Give the result of each rejection and each failure, what state is left
behind, and whether it fails closed.

## 8. Security

Say who can send or change the input, what they could gain, and which
requirement stops each case.

## 9. How the invariants are kept

For each invariant quoted in section 1, name the section that keeps it and
the test that fails if it is broken.

## 10. Tests

Name each test and its path, and list any requirement that has no test,
with its issue.

### 10.1 Golden vectors

List each vector, its expected bytes' derivation (by hand from this
specification, or an independently generated black-box capture, reduced,
with its fixture provenance), and what it proves. Values are this
project's own, never a source's example reused ([README](README.md),
"Test vectors and fuzz targets").

### 10.2 Round-trip properties

State each property the encoder and decoder must hold together.

### 10.3 Reject corpus

List each input that must fail, with the reason and the section it breaks.

### 10.4 Fuzz target

Name the target in `fuzz/` for each decoder and the invariants it asserts.

### 10.5 Acceptance cases

Behaviour and procedure: give each case as a start state, the events in
order, and the result that must be seen.

## 11. Revisit if

Copy the trigger of each working decision this rests on, and add any
evidence that would reopen this specification's own choices.

## 12. Sources

List every source with its kind (SPECIFICATION, DOCUMENTATION, OBSERVED,
DECISION, OWN), version or date, locator and the sections that use it.
An outside document names its row in the register of editions
([README](README.md), "Register of editions"), which exists before the
specification cites it. Every wire fact comes from an allowlisted passage
of a registered edition or an independently generated black-box fixture
([README](README.md), "Sources and provenance", rule 14). No other implementation's source
([README](README.md), "Sources and provenance").

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| | | | |

### 12.1 Observed behaviour

For each observation, record what was done and seen, the venue (peer kind
and version, client system and version, tool) without identifying it, the
date, and where the reduced evidence is kept. Observe only a test venue
with synthetic identities, keys and data; never use a live-cell capture
or one holding credentials or private data ([README](README.md),
"Sources and provenance", rules 4 and 14).

## 13. Open questions

List each undecided or MISSING point, what it blocks, and its public
issue. A specification with an open question on its layout is not frozen.

## 14. Review

Link the review record in `docs/reviews/`. No rounds or findings are
written here.
