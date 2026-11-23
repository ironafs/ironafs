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
