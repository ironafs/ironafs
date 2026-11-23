<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: `ironafs-config` part 1, the strict TOML subset and the cell name

## The change

Class S. Issue #93 (plan id M2a-32), part (a), and the canonical cell
name of part (b). The fuzz target came in the security round, once the
fuzz crate was on `main`.

- `docs/specs/config-syntax.md`: the contract, written before the code.
  It covers the grammar of the subset, each refused TOML construct with
  its error and hint, the caps, the error positions and the cell-name
  rule.
- `docs/specs/README.md`: the index row for `config-syntax.md`;
  register rows E57 (TOML v1.0.0) and E58 (Unicode's `PropList.txt`,
  18.0.0); and the rules for formats by who produces the bytes. A format
  IronAFS writes keeps the magic-and-version rules. A format IronAFS only
  reads may opt in to a versionless contract, with its own status,
  freezing and frozen-table rules. The format kind covers both.
- `crates/ironafs-config`: a new pure crate.
  - `parse` turns one file's bytes into a document, or refuses the file
    with a positioned error.
  - `CellName::new` is the one constructor of a cell name.
  - The tests are unit tests and property tests.
- **Written by** `claude-code:claude-opus-5-5` (a Claude Code subagent,
  test first).

## Rounds

The class S seats for Claude-written code: GLM as the first reviewer,
and a second family. Both reviewed the specification and the code in one
round. Both ran read-only, without running the tests, on 2026-11-21
(UTC).

| Round | Range | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|
| 1, review | `ec2b47f..cbf738d` | glm | glm-5.3 | high | PASS: 1 minor |
| 1, review | `ec2b47f..cbf738d` | deepseek | deepseek-flash | max | PASS: 3 minor |

## Findings

GLM, round 1:

- **Minor, fixed (`eb8f60b`):** "Three refusals of CellName::new — Empty,
  TwoFinalDots and TooLong — carry no byte offset, but
  docs/specs/config-syntax.md §5.7 (closing paragraph: "Each refusal
  names the byte offset in the input where it was found") and §7.3 ("The
  cell-name constructor returns the reason and the offset") promise an
  offset for every refusal. All label-level errors (EmptyLabel,
  LabelTooLong, Underscore, HyphenAtLabelEdge, NotAscii, InvalidOctet) do
  carry one; only these three whole-name errors do not."
  - Each of the three now carries an offset: 0 for empty input, the first
    of the two final dots, and 253 for a name that is too long.
  - The specification's section 5.7 names these offsets.
  - Four tests failed against the new variants holding offset 0, then
    passed.

DeepSeek, round 1:

- **Minor, fixed (`eb8f60b`):** "CellName::new returns
  CellNameError::Empty, TwoFinalDots and TooLong with no byte offset,
  while the label-related variants carry one. The specification this
  change introduces requires an offset for every refusal: §5.7 says 'Each
  refusal names the byte offset in the input where it was found' and §7.3
  says 'The cell-name constructor returns the reason and the offset'.
  Fix: add an offset to these three variants (0 for Empty, the first of
  the final dots for TwoFinalDots, MAX_NAME_BYTES for TooLong), include it
  in Display, and update the tests/doctests." This is the same finding as
  GLM's, with the same fix.
- **Minor, fixed (`ba14dbc`):** "Node::first returns
  header.unwrap_or(created), and open_table reports node.header when a
  header's final segment is already an explicit table. For a table first
  created implicitly by a deeper header and later made explicit, the
  reported 'first definition' is the later explicit header, not the
  creating header: [a.b] creates a implicitly at line 1, [a] makes it
  explicit at line 2, and a third [a] reports Redefined { first: (2,1) }.
  Per §4 a table 'created implicitly by a deeper header' is a Definition,
  and §5.4.4 says a redefinition error names the position of the first
  definition. The same wrong first is used when a key conflicts with such
  a table, e.g. [x.a.b], [x.a], [x], then a = 1. Fix: report Node.created
  (which equals the header for a directly defined table and the creating
  header for an implicit one) in both first() and open_table's final
  check, and add the [a.b]/[a]/[a] and key-vs-implicit-then-explicit cases
  to the tests."
  - A redefinition now names the header that created the table. The
    node records only whether a header has made it explicit.
  - The specification's section 5.4, point 4 states the rule.
  - Both cases are tested. The test failed on the old code with
    `first: (2,1)`, then passed.
- **Minor, fixed (`27688bc`):** "within_caps claims to check 'every cap'
  of §5.2 on an accepted document, but its MAX_DEFINITIONS count is only
  explicit tables (document.tables()) plus entries: implicit tables are
  invisible through the public API, so the check is a lower bound and
  cannot observe an implicit-table counting bug. It also cannot check the
  file or line caps because Document retains no source bytes. Fix:
  compute the implicit prefixes from the explicit table paths (every
  implicit table is a proper prefix of some explicit table path) or
  narrow the doc/claim, and add a generated document with a deep header
  at the definitions boundary."
  - `within_caps` now counts every distinct prefix of the explicit
    tables' paths, so each implicit table is counted once.
  - Its documentation names the caps it checks, and says the file and
    line caps are checked on the input by the cap tests.
  - A new property, `parse_near_definitions_cap_counts_every_implicit_table`,
    generates files of keys followed by one header up to 8 segments deep,
    around the cap. A file must be accepted exactly when it is within the
    cap, and otherwise refused as `TooManyDefinitions`.
  - The property failed against a planted builder that did not count
    implicit tables, then passed on the real code.

What the seats could not check, one line each:

- GLM: no test, clippy or xtask check was run (read-only); the results are INSPECTED only. The evidence below covers the runs.
- GLM: issues #93 and #53 could not be read (the sandbox denied `gh`); the rule was checked against the specification's quotation of the owner's decision.
- GLM: the planted-defect runs are not in the diff, so they cannot be checked by reading. They are listed under Evidence. The fuzz target is deferred to #45.
- GLM: the property tests generate inputs far below the file cap; the file and definitions caps are covered by the hand-written cap tests. The new boundary property now also covers the definitions cap.
- DeepSeek: issue #93's text could not be read; the rule was checked against the specification's quotations.
- DeepSeek: whether "first definition" includes implicit creation. The specification now says it does (section 5.4, point 4), which settles that finding in its favour.
- DeepSeek: a duplicate key is reported as a redefinition before the grammar of the rest of its line is checked. Examples: a later lone `a` gives `Redefined`, not `Unexpected`; a later `a.b = 2` gives `Redefined`, not `DottedKey`. Not counted as a defect. Under the specification's "first error" rule (section 7, point 2), the redefinition's column comes first.
- DeepSeek: the guards on builder indexes (`get` with a fallback) cannot fire under the builder's invariants. If one did, it would present a wrong document as a success. Not counted as a defect. Every index comes from the builder itself, and the comments at the guards say so.
- DeepSeek: the error columns for `LineTooLong` (column 1) and for `KeyTooLong` and `StringTooLong` (the token's start) are pinned by the tests but not by the specification's wording. Not counted as a defect.
- DeepSeek: the planted-defect runs and the test runs are not in the diff, so they are INSPECTED only. The evidence below covers them.
- DeepSeek: the test-support property APIs were read only where they are used.

The review stopped there, under the stop rule: only minor findings, fixed
and recorded, so no verification round.

## Evidence

- At `cbf738d` (EXECUTED, macOS 27 on Apple silicon, 2026-11-21 UTC), each with exit 0:
  - `cargo xtask check`, including `check-pure`: `ironafs-config` was compiled against `core` and `alloc` for 6 targets in dev and release;
  - `cargo nextest run --workspace --retries 0 --no-tests fail`: 412 tests passed;
  - `cargo test --doc --workspace`;
  - `cargo clippy --all-targets --workspace`, with no warnings;
  - `cargo xtask check-pure --self-test`, `cargo fmt --all --check` and `reuse lint`.
- At the final head (EXECUTED, same machine and date): the same commands; the results are in the pull request.
- Red before green (EXECUTED, same machine and date):
  - every unit test failed against compiling stubs, except three the stubs happened to satisfy;
  - 23 planted wrong implementations were each caught: off-by-one errors on every cap, a duplicate key where the last one wins, tab and C1 characters allowed, mixed arrays, dotted keys, dates read as integers, no lower-casing, edge hyphens, underscores, two final dots, an escape misread, and an error column shifted;
  - in this round, the three fixes' tests failed first as described above.
- The fuzz target `ironafs-config_syntax` (specification section 10.4) was MISSING at `cbf738d`; it is added in the security round below.

## Security round

The class S security review, GPT read-only (`gpt-6.2-sol`, effort
xhigh, 2026-11-23), read `ec2b47f..980fad5` and returned one major and
six minor findings. The branch first took `main` (`df943f9`,
a merge of `720a9cd`), which brought the fuzz crate. The fixes are
`cf04f93` (code and tests), `7fd8d9d` (fuzz target) and `4a457e2`
(specification).

Findings, each quoted in its defect line, with its disposition:

- **F1, major, fixed (`7fd8d9d`, `cf04f93`, `4a457e2`):** "The
  configuration decoder ships without its required fuzz target."
  - `fuzz/fuzz_targets/ironafs-config_syntax.rs` is registered as a
    `[[bin]]` in `fuzz/Cargo.toml`, with six tracked seeds under
    `fuzz/seeds/ironafs-config_syntax/`.
  - It asserts section 10.4 on arbitrary bytes: the parser returns, an
    error's position is within the input, an accepted document keeps the
    caps a document can show, and the same input gives the same result.
  - Its checks are the property tests' own: they moved to
    `crates/ironafs-config/tests/it/invariants.rs`, which the target
    includes by path, as the XDR target includes its test types.
- **F2, minor, fixed (`cf04f93`):** "`header` decodes and allocates the
  ninth segment before checking `MAX_DEPTH`."
  - A ninth segment that would be decoded (a bare key or a quoted one) is
    refused as `TooDeep` at its first character, before it is read.
    Anything else after the dot is left to the segment reader, so
    `[a.b.c.d.e.f.g.h.]` still reports a missing key.
  - The test failed first, on its first case: an unterminated ninth
    segment was refused as unterminated, not as too deep.
- **F3, minor, fixed (`4a457e2`):** "The stated bound on stored path
  bytes omits the explicit table's copy of its full path."
  - Section 5.2 now counts, for one header with decoded segment text P:
    up to `MAX_DEPTH` created paths, the explicit table's copy, and the
    created tables' names in their parents, at most `MAX_DEPTH` + 2
    times P. It says the bound is decoded text, without container
    overhead.
  - The finding named the explicit copy only. The names stored in each
    parent's map are a second copy of each new segment that the old text
    also left out; the new bound counts both.
- **F4, minor, fixed (`cf04f93`):** "`add_entry` repeats `check_key`
  after `Parser::key_value` already called it."
  - `check_key` now returns the key it cleared, which `add_entry` takes;
    `add_entry` keeps the checked count update. No behaviour changes, so
    no test could fail first; the existing redefinition and definitions
    cap tests cover both calls.
- **F5, minor, fixed (`cf04f93`):** "`within_caps` checks upper bounds
  for explicit table paths and segments but lets both be empty."
  - An empty explicit path or an empty segment is refused before the
    prefixes are counted.
  - Shown by a planted defect: with the parser's empty quoted key check
    removed, the any-bytes property failed on `[""]` with "bad segment".
    The old check would have passed it (INSPECTED: it checked only entry
    keys for emptiness).
- **F6, minor, fixed (`4a457e2`, test in `cf04f93`):** "§5.4 leaves the
  final-segment collision ambiguous when a table header names a key that
  already holds a value."
  - Section 5.4, point 3 now says: at the last segment, `Redefined` at
    the header's `[`; at an earlier segment, `NotATable` at that segment;
    both name the key's position. Point 4 states where each redefinition
    is reported.
  - A new test covers both cases inside a table. It passed at once: the
    code already did this, and the fix is to the text.
- **F7, minor, fixed (`cf04f93`, `4a457e2`):** "The heading about hidden
  text is broader than the filter."
  - The controller decided that the format refuses bidirectional
    controls: Unicode's `Bidi_Control` set, U+061C, U+200E, U+200F,
    U+202A to U+202E and U+2066 to U+2069, checked against Unicode's
    `PropList.txt` (18.0.0).
  - They are refused anywhere in the file, in the whole-file checks, with
    a new error kind, `BidiControl`: they are not control characters
    (category Cf, not Cc), so `ControlCharacter` would misname them.
  - Section 5.1, point 3 states the set, the reason (text that displays
    in a different order than it parses) and the filter's exact scope;
    section 8, point 4 says nothing else is filtered and that a caller
    escapes text that is not printable before echoing it.
  - The test covers each of the twelve characters in a comment, a quoted
    key, a table-path segment, a string, an array element, and after a
    bare key and a value. It failed first, on its first case: U+061C in
    a comment was accepted. A second test accepts characters next to the
    set.

Evidence (EXECUTED, macOS 27 on Apple silicon, 2026-11-23, at
`4a457e2`), each with exit 0:

- `cargo xtask check`; `cargo nextest run --workspace --retries 0
  --no-tests fail` (812 tests passed); `cargo test --doc --workspace`;
  `cargo clippy --all-targets --workspace`, with no warnings;
  `cargo fmt --all --check`; `cargo deny check`; `reuse lint`.
- The fuzz job's steps: `cargo xtask check-fuzz-deny` and its
  `--self-test`; `cargo xtask fuzz --self-test`; in `fuzz/`, `cargo fmt
  --check`, `cargo clippy --all-targets --locked` with no warnings and
  `cargo deny --locked check`; `cargo xtask fuzz`, which replayed six
  seeds of `ironafs-config_syntax` and three of `ironafs-xdr_decode`.
- A fuzzing run of `ironafs-config_syntax` on the pinned nightly, from
  the tracked seeds, for 120 seconds: about 4.8 million inputs, no
  failure.

### Verification

The same seat (GPT read-only, `gpt-6.2-sol`, xhigh, 2026-11-23)
verified the fixes. It found F2 to F7 and the merge fixed, F1 partly
fixed, and one major and four minor findings in the fixes. The fixes are
`20770d7` and `6fc4ccf`.

- **F1 residual, major, fixed (`20770d7`):** "The fuzz target checks
  caps visible in the parsed document and omits the file and line byte
  caps, even though it has the raw input. The fix narrows §10.4 at
  docs/specs/config-syntax.md:384 to match the incomplete check."
  - The shared check now refuses an accepted input over the file cap or
    with a line over the line cap (LF or CR LF ends a line), and requires
    `FileTooLarge` with no position exactly when the input is over the
    file cap.
  - Section 10.4 again states the whole invariant: every cap of section
    5.2, the byte caps on the input and the others on the document.
  - Four new seeds hold the line cap and the file cap each at its value
    and one above.
  - Tests of the check itself failed first: it passed a line over the
    cap with an empty document, and a positioned error for a file over
    the file cap. With the parser's line cap, then its file cap, planted one
    byte too high, `cargo xtask fuzz` failed on the matching over-cap
    seed each time.
- **Minor, fixed (`20770d7`):** "The new fuzz target inherits a
  byte-based column bound from the property test. The format defines
  columns in Unicode scalar values."
  - A column is bounded by the line's Unicode scalar values plus one; in
    a line that is not UTF-8, by its valid prefix, so the latest column
    is the one just after that prefix. The test failed first on a column
    of 4 in a two-character, three-byte line.
- **Minor, fixed (`20770d7`):** "At the depth cap, the new guard returns
  `TooDeep` for an empty quoted segment or a multiline basic string
  opener."
  - The guard refuses only a segment that would be decoded: a bare key,
    or `"` not followed by another `"`. `""` goes to the segment reader,
    which refuses an empty key and a multi-line string without storing
    anything. Section 5.2 states this precedence.
  - The test failed first: both forms gave `TooDeep`. A literal string at
    the cap keeps its error too.
- **Minor, fixed (`20770d7`):** "The new invariant says text displays as
  it parses."
  - Section 9's row now states what is tested: the parser refuses Unicode
    `Bidi_Control` characters anywhere in the file.
- **Minor, fixed (`20770d7`, `6fc4ccf`):** "The test-only
  `BIDI_CONTROLS` list duplicates the one at
  crates/ironafs-config/src/syntax/tests.rs:305."
  - One list remains, in `tests/it/invariants.rs`, apart from the
    production predicate.
  - Sharing it with the unit tests by a path attribute failed
    `check-pure`, which refuses a path attribute in a pure crate's
    source. So the rejection test moved to the integration tests
    (`tests/it/syntax.rs`), which use the same list as the property
    tests and the fuzz target.

Evidence (EXECUTED, macOS 27 on Apple silicon, 2026-11-23, at
`6fc4ccf`), each with exit 0: the same commands as above
(`cargo nextest run` passed 815 tests); the fuzz job's steps, with
`cargo xtask fuzz` replaying ten seeds of `ironafs-config_syntax` and
three of `ironafs-xdr_decode`; and a 120-second fuzzing run of
`ironafs-config_syntax` from its seeds, with no failure.

### Final verification

The same seat (GPT read-only, `gpt-6.2-sol`, xhigh, 2026-11-23) read
`b996092..9043b80` and passed it: the byte caps, the depth precedence,
the section 9 wording and the single bidirectional list FIXED; the
error columns PARTLY; and three minor findings and one recommendation.
Each was fixed, recorded, and the review stopped under the stop rule,
with no further round. The fixes are `0424dcf`.

- **Error columns, PARTLY, fixed (`0424dcf`):** "It can still accept a
  position on a later line after the first invalid sequence in the
  file, and it allows an extra column on CRLF lines. Use the whole-file
  valid prefix and exclude CR when it is part of CRLF."
  - A position is checked against the file's valid UTF-8 prefix, and a
    CR before LF is not counted as a column. The tests failed first on
    the CR LF case (column 4 of `ab` CR LF was accepted).
- **Minor, fixed (`0424dcf`):** "The line-cap check strips a trailing CR
  from the final line even when no LF follows it."
  - A CR is stripped only before an LF. The new case, a cap-length
    comment then a bare CR at the end, failed first.
- **Minor, fixed (`0424dcf`):** "Section 10.4 now says the fuzz check
  enforces the character restrictions on the accepted input. The check
  applies them to parsed keys, segments and strings."
  - Section 10.4 now says the character check is on the parsed keys,
    table-path segments and strings, and that a comment's characters are
    left to the rejection tests.
- **Minor, fixed (`0424dcf`):** "The new invariant tests use bare
  `is_err()` assertions despite coding standards rule 15. The moved
  `refused` helper at line 455 also drops `#[track_caller]`."
  - Each negative case asserts the exact error the check returns, and
    `refused` has `#[track_caller]` again.
- **Recommendation, applied (`0424dcf`):** "Large seeds should be
  replaced. Generate both exact file sizes with the existing `comments`
  helper in a deterministic integration test, parse both, and assert
  acceptance at the cap and unpositioned `FileTooLarge` one byte above
  it."
  - `parse_file_cap_at_and_one_above_in_comment_lines` does this; the
    `file-at-cap` and `file-over-cap` seeds are removed and the
    line-cap seeds kept. It passed at once: it moves coverage, not
    behaviour.

Evidence (EXECUTED, macOS 27 on Apple silicon, 2026-11-23, at
`0424dcf`), each with exit 0: the same commands as above
(`cargo nextest run` passed 816 tests); the fuzz job's steps, with
`cargo xtask fuzz` replaying eight seeds of `ironafs-config_syntax` and
three of `ironafs-xdr_decode`; and a 120-second fuzzing run of
`ironafs-config_syntax` from its seeds, about 3.6 million inputs, with
no failure.

## Landing

All seats ran read-only on 2026-11-23. GLM is `glm-5.3` at high effort;
GPT is `gpt-6.2-sol` at xhigh.

1. **Landing verification** (GLM, `df943f9..1fcb684`): PASS, with one
   minor finding, fixed (`699d893`) and recorded, with no further round.
   - Finding: "crates/ironafs-config/src/syntax.rs:7-9 (module doc) and
     crates/ironafs-config/src/syntax/parser.rs:35-37 (check_file doc)
     still enumerate the whole-file checks as "line ends, control
     characters, line length" without the bidirectional-control check
     that cf04f93 added (parser.rs:104-109), although the check's own
     doc (parser.rs:85-87) was updated."
   - Fix: both summaries now name bidirectional controls.
2. **Index and register.** `config-syntax.md` had no index row. `bb043fb`
   added it, with register rows E57 and E58, and S1 and S2 now name
   them. `0df217e` stated §5.3 against the README's format rules.
3. **Two-family check** of `699d893..0df217e`: GLM PASS, with minor
   notes; GPT FAIL, with two majors and six minors. GPT's findings, quoted
   as written:
   - **Major, fixed (`f38f3b2`):** "docs/specs/config-syntax.md:221.
     Defect: Point 4 promises the position and hint for the first
     unsupported construct. The parser runs whole-file checks before
     parsing the grammar. A literal string on line 1 followed by U+061C
     in a comment on line 2 reports `BidiControl` on line 2. Only
     `Unsupported` errors carry hints. A raw tab inside a basic string
     is valid TOML and produces `ControlCharacter` without a hint. [TOML
     v1.0.0](https://toml.io/en/v1.0.0#string) Consequence: The
     documented diagnostic can differ from the error an administrator
     receives. The parser does return no document on failure and leaves
     the input bytes untouched. Fix asked: Define a newer file in terms
     of syntax the current parser rejects. State that the first error
     follows §5.4’s check order and that §5.5 diagnostics carry hints.
     Preserve the position exception for oversized files and the
     guarantee that rejection leaves the bytes untouched."
     - §5.3, point 3 now gives the first error in §5.4's order, a
       position except for an oversized file, a hint only for §5.5, and
       no document.
   - **Major, fixed (`f38f3b2`):** "docs/specs/config-syntax.md:211.
     Defect: The parser matches the specification’s versionless syntax,
     but `docs/specs/README.md:453` requires magic and a version for a
     format later IronAFS versions must read. Point 1 names the
     departure and gives a rationale. The governing rule contains no
     exception for human-written configuration, and the point cites no
     decision that authorizes one. Consequence: The specification and
     the governing rule prescribe different framing for the same files.
     Fix asked: Record the scoped exception for human-written TOML
     configuration in the governing rule, or cite the decision that
     authorizes it."
     - The controller changed the README rule instead of allowing a
       departure: a format people write and IronAFS only reads states a
       versionless contract, which §5.3 follows.
   - **Minor, fixed (`f38f3b2`):** "docs/specs/config-syntax.md:215.
     Defect: “Every IronAFS version” overstates the compatibility
     guarantee. Earlier development revisions accepted bidirectional
     controls in comments that `cf04f93` began rejecting. At `0df217e`,
     `ironafs-config` takes an immutable byte slice, exposes no encoder,
     and delegates file discovery and reads to callers. The inspected
     tree contains no production writer for these files. Consequence:
     The wording leaves the compatibility baseline unclear and combines
     a future release guarantee with current implementation facts. Fix
     asked: Start the guarantee with the first release of this syntax.
     State that later releases preserve the interpretation of previously
     accepted syntax without migration or data loss. State separately
     that the current implementation writes no version of these files."
     - The promise starts at the first release; pre-release revisions
       are excluded; that IronAFS writes no version of these files is
       stated separately.
   - **Minor, fixed (`f38f3b2`):** "docs/specs/config-syntax.md:215.
     Defect: Points 2 through 5 lack the claim labels required by
     `docs/specs/README.md:314`. Point 1 places `[OWN]` before its
     rationale, while the rule calls for the label at the end.
     Consequence: The list gives implementation facts and compatibility
     choices the same presentation. Fix asked: End each numbered point
     with the applicable label. Use `[OWN]` for the compatibility
     choices and cite the implementation evidence for current parser
     behavior."
     - Each point of §5.3 ends with its label.
   - **Minor, fixed otherwise (`f38f3b2`):** "docs/specs/README.md:14.
     Defect: The new row follows `directory-format.md`, although the
     index rule at line 697 orders rows by filename. Its title, link,
     kind, class, and draft date match the specification. Consequence:
     The new row is out of order under the index rule. Fix asked: Place
     `config-syntax.md` after `callbacks.md` and before
     `directory-format.md`."
     - The rows are in title order, so the index sentence now says title
       order; the row stays where it is.
   - **Minor, fixed (`f38f3b2`):** "docs/specs/README.md:422. Defect:
     E57 records the existence of TOML v1.1.0, but its update check
     omits the publisher’s changelog, which records a later
     clarification about accepting a byte order mark. The pinned v1.0.0
     edition, publication date, origin and tagged MIT licence check out.
     [TOML
     changelog](https://github.com/toml-lang/toml/blob/main/CHANGELOG.md#unreleased)
     [Tagged
     licence](https://github.com/toml-lang/toml/blob/1.0.0/LICENSE)
     Consequence: The register does not show that a later clarification
     relevant to an explicit parser choice was considered. Fix asked:
     Record the changelog check and distinguish the later clarification
     from the pinned v1.0.0 text and IronAFS’s own rejection rule."
     - E57 records the changelog check and its byte order mark
       clarification.
   - **Minor, fixed (`f38f3b2`):** "docs/specs/README.md:423. Defect:
     E58 points to Unicode’s general terms without naming the applicable
     Unicode License v3 or its notice condition. Its update check
     records the latest version but omits Unicode’s errata page, which
     lists no current errata. The pinned file confirms the edition, file
     date, publisher and four `Bidi_Control` ranges, which match the
     parser.
     [PropList.txt](https://www.unicode.org/Public/18.0.0/ucd/PropList.txt)
     [Unicode terms](https://www.unicode.org/copyright.html) [Unicode
     License v3](https://www.unicode.org/license.txt) [Unicode
     errata](https://www.unicode.org/errata/) Consequence: The row
     leaves the reuse terms and errata result implicit. Fix asked: Name
     Unicode License v3 and its notice condition, retain the facts-only
     citation policy, and record the dated errata check."
     - E58 names the Unicode License v3 and its notice condition, and
       records the errata check.
   - **Minor, fixed (`f38f3b2`):** "docs/specs/config-syntax.md:444.
     Defect: S1 names E57 and lists §5.5 as a use, but the passage list
     omits the TOML sections that define forms it describes, including
     inline tables and arrays of tables. The §5.1 `CONFIRMED` tags for
     BOM handling and `Bidi_Control` also omit the passage or property
     locator required by the labelling rule. [TOML
     v1.0.0](https://toml.io/en/v1.0.0) [Unicode
     PropList.txt](https://www.unicode.org/Public/18.0.0/ucd/PropList.txt)
     Consequence: Readers cannot trace each standard fact to its
     specific registered passage. Fix asked: Align E57 and S1 with the
     TOML passages actually used, cite those passages for the standard
     syntax, and label the parser’s refusal choices `[OWN]`. Give the
     BOM claim an exact locator and name `Bidi_Control` in the S2 tag."
     - S1's uses drop §5.5, which cites no TOML passage; the byte order
       mark and `Bidi_Control` tags carry their locators.
   - **Minor, fixed (`f38f3b2`):** "GLM: Three minor notes: E57's title
     drops the comma in "Tom's Obvious, Minimal Language"
     (README.md:422); S1's "Used in" cell lists §5.5 where no S1
     citation appears (config-syntax.md:444, likely predating these two
     commits); and README.md:697 says the index is ordered by file name
     while the table is uniformly ordered by title (pre-existing, the
     new row follows the table's convention). No blocker or major."
     - The title has its comma; S1 no longer lists §5.5; the index
       sentence says title order.
4. **README rule rounds.** GPT's findings, quoted as written:
   - `0df217e..f38f3b2`: GLM PASS; GPT, two majors and two minors:
     - **Major, fixed (`1e8d3f7`, `908bfe9`):**
       "docs/specs/README.md:453. Defect: The two branches leave gaps.
       An IronAFS-defined persistent format can bypass magic and version
       if its specification omits a later-reader requirement, even
       though line 476 freezes it when its first writer ships. An
       IronAFS-defined read-only input produced by another system can
       fall outside the branch for formats people write. The format-kind
       definition at line 67 also assumes IronAFS stores or sends the
       bytes. Consequence: A persistent writer can ship bytes with no
       version discriminator, and a supported read-only input can ship
       without a declared compatibility policy. Fix asked: Make magic
       and version the default for IronAFS-defined persistent output
       from its first shipped writer, including mixed human and IronAFS
       authorship. Require an explicit decision for any
       disposable-output exception. Cover all supported read-only inputs
       with a declared compatibility policy, and reserve versionlessness
       for formats designed that way. Align the format-kind definition
       with those rules."
       - The rule is scoped by who produces the bytes: every format
         IronAFS writes keeps magic and version; a read-only format opts
         in to the versionless points; the format kind covers both
         (`908bfe9`).
     - **Major, fixed (`1e8d3f7`, completed in `52c090c`):**
       "docs/specs/README.md:476. Defect: The freeze rules lack a path
       for the new versionless contract. They key the normal freeze to
       the first writer and permit changes until then. If another
       party's dependence triggers a freeze, the rules lock tolerance,
       require a new version for defects, and require magic and version
       bytes in the table at line 718. Consequence: A released
       versionless reader can be treated as mutable, or its permitted
       extensions can be blocked by a freeze that requires bytes the
       format deliberately omits. Fix asked: Set the compatibility
       baseline at the first supported reader release or an earlier
       explicit commitment. Specify how to record that baseline without
       magic or version bytes and how later releases may extend
       acceptance while preserving the baseline's accepted inputs and
       their meanings."
       - Freezing gained the versionless path; the next round found it
         PARTLY, and `52c090c` completed it.
     - **Minor, fixed (`1e8d3f7`):** "docs/specs/config-syntax.md:69.
       Defect: The overview still promises a position and hint for every
       refused TOML construct. Revised §5.3 correctly limits hints to
       §5.5 diagnostics. A literal tab in a basic string is valid TOML
       and returns `ControlCharacter` without a hint. [TOML v1.0.0
       String](https://toml.io/en/v1.0.0#string) Consequence: The
       overview gives administrators a different diagnostic expectation
       from the detailed contract and the parser. Fix asked: Make the
       overview refer to §7 for positions and limit hints to §5.5
       diagnostics."
       - §3 refers to §7 for positions and gives hints only for §5.5.
     - **Minor, fixed (`1e8d3f7`, completed in `52c090c`):**
       "docs/specs/README.md:422. Defect: E57's new BOM note describes
       v1.0.0 as silent without distinguishing the prose from the linked
       formal grammar. The prose does not mention a BOM. The tagged
       ABNF's document-start productions do not admit a leading U+FEFF.
       [TOML v1.0.0
       ABNF](https://github.com/toml-lang/toml/blob/1.0.0/toml.abnf)
       Consequence: Readers can infer that the pinned grammar leaves a
       leading BOM unspecified. Fix asked: Distinguish prose silence,
       the tagged grammar, and the later changelog clarification in E57
       and §5.1. Keep IronAFS's explicit rejection and diagnostic."
       - E57 and §5.1 separate the prose, the tagged grammar and the
         changelog.
   - `f38f3b2..908bfe9`: GPT, two majors and two minors, and two
     earlier items PARTLY:
     - **Major, fixed (`52c090c`):** "docs/specs/README.md:112. Defect:
       The status rules define `frozen` only per format version. The
       status table at line 103 also directs changes through a new
       version. Consequence: A versionless format has a freeze trigger
       but no defined status entry for its compatibility baseline or
       subsequent extensions. Fix asked: Define how the status line
       identifies the frozen specification revision and date for a
       versionless format. Refer to the versionless extension rule for
       later changes."
       - The status table and rules say a versionless read-only format
         is frozen once, at a named revision and date, and then only
         grows.
     - **Major, fixed (`52c090c`):** "docs/specs/config-syntax.md:219.
       Defect: Section 5.3 excludes every pre-release revision from its
       compatibility promise. The README now expressly freezes a
       versionless format earlier if another party starts to depend on
       it. Consequence: The configuration spec permits a pre-release
       syntax change that can break an already-frozen input or change
       its meaning. Fix asked: Start the compatibility promise at the
       first supported reader release or an earlier freeze. Identify the
       syntax revision that establishes the baseline."
       - §5.3 says no other party depends on the syntax before its first
         release, and that if one starts to, the syntax is frozen from
         then.
     - **Minor, fixed (`52c090c`):** "docs/specs/README.md:440. Defect:
       The checklist still requires every format specification to
       describe a canonical encoder even though the kind now explicitly
       includes read-only formats. Consequence: A read-only
       specification such as configuration syntax has no encoder
       behavior to document. Fix asked: Apply the encoding requirements
       where IronAFS writes the format. Have read-only specifications
       state that IronAFS has no encoder."
       - Canonical form applies to a format IronAFS writes; a read-only
         format says it has no encoder.
     - **Minor, fixed (`52c090c`):** "docs/specs/config-syntax.md:100.
       Defect: The new explanation says an expression starts with a
       space or tab. E57 at README line 422 uses the same shorthand. The
       tagged grammar allows zero whitespace characters. [TOML 1.0.0
       ABNF](https://github.com/toml-lang/toml/blob/1.0.0/toml.abnf)
       Consequence: The explanation implies that inputs beginning
       directly with a key, table header, or comment need leading
       whitespace. The conclusion that a leading BOM is excluded remains
       correct. Fix asked: Say that leading spaces and tabs are optional
       and that the document-start productions exclude U+FEFF."
       - §5.1 and E57 say leading spaces and tabs are optional and no
         production admits U+FEFF.
     - **Partly, fixed (`52c090c`):** "docs/specs/README.md:476 —
       PARTLY. The first-reader trigger and growth rule are present. The
       general freeze bullets still require a new version for defects,
       and the index still unconditionally requires magic and version
       bytes at line 731. Make the versionless path an explicit
       alternative and define its table entry and baseline check."
       - The freeze bullets and the frozen-table row now give the
         versionless path (`52c090c`).
     - **Partly, fixed (`52c090c`):** "docs/specs/README.md:422 —
       PARTLY. E57 distinguishes the prose, tagged grammar, and later
       changelog. Section 5.1 keeps the explicit rejection and hint and
       still needs a reference to the later clarification. [TOML
       changelog](https://github.com/toml-lang/toml/blob/main/CHANGELOG.md)"
       - §5.1 now refers to the changelog clarification (`52c090c`).
   - `908bfe9..52c090c`: GPT FAIL. Every earlier item passed except one
     PARTLY, and two new majors were raised in the README's general
     rule. Item 5 quotes them.
5. **The last round's findings.** The two majors and the one PARTLY
   item from the `908bfe9..52c090c` round, quoted as written:
   - **Major, fixed (`c3d0d6d`):** "docs/specs/README.md:504. Defect: The
     versionless path inherits the exception for input the frozen text
     already called invalid. That text is pinned once, so it can still
     classify syntax added by a later approved extension as invalid.
     Consequence: A subsequent release could invoke the exception to
     reject syntax an earlier release deliberately began accepting. Fix
     asked: Limit the exception to correcting implementation acceptance
     outside the applicable specification. Explicitly protect input
     admitted by the frozen baseline or any approved extension."
     - For a versionless read-only format, the one correction rejects
       only input an implementation accepted outside the applicable
       specification; input the baseline or an approved extension
       admits keeps its meaning and is never rejected again. The status
       table, the status rule and the versionless points say the same.
       `config-syntax.md` §5.3 keeps a stronger promise, with no
       exception.
   - **Major, fixed (`c3d0d6d`):** "docs/specs/README.md:737. Defect: The table
     requires every versionless read-only format to record `none` for
     magic. The opt-in at line 477 permits a format with a magic value
     and no version. Consequence: An eligible format with a magic value
     cannot get an accurate frozen formats row under this rule. Fix
     asked: Record and check the specified magic value when present. Use
     `none` when the specification defines no magic value."
     - The row records the magic value the specification defines, or
       "none" when it defines none, and the check compares it with the
       specification's.
   - **Partly, fixed (`f7168d4`):** "docs/specs/README.md:476 — PARTLY. The
     versionless path has its own freeze trigger, baseline and table
     entry. The index still specifies a check only for magic and version
     bytes. Require a check that the table and status line identify the
     same baseline commit."
     - The index section's check now also compares a versionless row's
       revision with the baseline commit the specification's status line
       names.

   The controller first proposed deferring the two majors to a
   follow-up issue on the specs README, and the refutation seat (GLM)
   ruled that the deferral HOLDS: `config-syntax.md` and the parser are
   correct whichever way the rule is worded. The follow-up issue's
   two-family check split: GLM PASS; GPT a blocker, quoted as written:
   "Opening paragraph and findings 1–2 — The draft justifies deferral
   based on the unfrozen config syntax. Both findings concern rules that
   #137 itself introduces at `52c090c`. Workflow §3 steps 3 and 7
   prohibit deferring a confirmed defect in a contract the PR decides."
   GPT is right, so both majors are fixed in this change (`c3d0d6d`),
   and no follow-up issue is filed for them.

Final head: the commit that updates this section, on top of
`f7168d4`, which ends the reviewed content.
