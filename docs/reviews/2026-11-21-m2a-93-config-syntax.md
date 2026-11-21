<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: `ironafs-config` part 1, the strict TOML subset and the cell name

## The change

Class S. Issue #93 (plan id M2a-32), part (a) without its fuzz target,
and the canonical cell name of part (b).

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
- MISSING: the fuzz target `ironafs-config_syntax` (specification section 10.4). It waits on the fuzz crate and the owner's `libfuzzer-sys` decision (#45).
