<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Configuration syntax: the strict TOML subset, and the cell name

Status: draft (2026-11-21). Raised in issue #93.

Kind: format. Class: S, because the parser decodes input that an
administrator, a management system or a local user controls, and the cell
name it constructs keys connections, caches and the generation record;
the schema that gives keys their meaning is a separate contract (§2).

Sections keep their numbers once the specification is reviewed.

## 1. What this implements

- **ADR 0004, O18, point 1** (Working): "The format is a strict subset of
  TOML, parsed by our own code (rule 21): `ironafs.toml` plus
  `ironafs.d/*.toml` drop-ins."
- **ADR 0004, O18, point 2** (Working): "An error names the file, line,
  column, what was expected and a hint."
- **ADR 0004, O18**, "Why" (Working): "a typo in a security setting must
  fail loudly. A small in-house parser is easier to audit and fuzz than a
  general one."
- **ADR 0004, C4, point 1** (Working; `no_std` as issue #53, point 7,
  amends it, on the owner's decision of 2026-11-20): `ironafs-config` is
  a pure crate, "the configuration parser, schema, layering and
  validation (O17, O18)".
- **ADR 0004, O47, point 3** (Working): "Fuzzing covers every decoder:
  ... the configuration parser".
- **The canonical cell name** (owner's decision, 2026-11-20, recorded in
  issue #93): "ASCII lower case (upper-case ASCII input is lower-cased at
  input ...), at most one final dot stripped at input, 1 to 253 bytes
  after that, labels of 1 to 63 bytes of letters, digits and inner
  hyphens, no underscores, an IDN only in its ASCII form and not
  validated further. Every cell name, from local files, policy, the
  classic files or discovery ..., passes it".
- **Coding standards rules 1 to 3**: no panic reachable from
  configuration files, checked arithmetic, caps in bytes before
  allocation.

## 2. Scope

1. **Covered:** the bytes of one configuration file and what they parse
   into (§5.1 to §5.4); the caps (§5.2); the errors and their positions
   (§7); the cell-name rule and its one constructor (§5.7).
2. **Not covered, and where it is:**
   - which keys exist, their types, defaults and who may set them; the
     unknown-key rules of O18, point 4 and O19, points 2, 3 and 6: the
     configuration schema (issue #93, part (b));
   - the order of `ironafs.toml`, the drop-ins and OS policy, and how a
     later layer may change an earlier one (O17): the same part;
   - the policy envelope and the four policy states (O19): part (c);
   - the classic `CellServDB` and `ThisCell` files, which are a second
     grammar (O18, point 3): part (d);
   - reload outcomes: part (e); the last good policy record: part (f);
   - finding and reading the files, their ownership, and the file name
     an error names: the caller. `ironafs-config` parses bytes.
   - what a cell name must satisfy beyond §5.7 for a particular use (for
     example, that a DNS lookup needs more than one label): the user's
     specification (the DNS client specification, #60, 5.6.6, point 1).

## 3. The design in short

1. A file is a list of lines. Each line holds nothing, a table header
   `[a.b]`, or one `key = value`, each with an optional comment.
2. A value is a string, a decimal integer, a boolean, or an array of
   values of one of those types.
3. Every other TOML construct is refused with its position and a hint. A
   file this parser accepts is a valid TOML 1.0.0 document with the same
   meaning; a file it refuses may be valid TOML. Nothing is skipped or
   repaired.
4. The result is a document: the tables declared by headers, and the
   entries, each with its table's path, its key, its value and the
   positions of the key and the value.
5. Cell names are a separate type, built only by one constructor that
   applies §5.7.

## 4. Terms

- **Position:** a line and a column, both counted from 1. The column
  counts Unicode scalar values from the start of the line, so a tab or a
  non-ASCII character counts as one.
- **Segment:** one part of a table path or a key: a bare key or a quoted
  key.
- **Path:** the segments of a table header, in order. The root table has
  the empty path.
- **Definition:** a key with its value, or a table, explicit or created
  implicitly by a deeper header.

## 5. Specification

### 5.1 Layout

1. **Encoding.** The file is UTF-8 `[S1 "Spec": CONFIRMED]`. A file that
   starts with the byte order mark (EF BB BF) is refused: TOML 1.0.0 does
   not say whether one is allowed `[S1: CONFIRMED]`, so the subset does
   not accept one `[OWN]`.
2. **Lines.** A line ends at LF or at CR LF `[S1 "Spec": CONFIRMED]`. The
   last line may end without one. A CR not followed by LF is refused.
3. **Characters.** No control character appears anywhere in the file
   other than tab (U+0009), LF, and CR before LF: U+0000 to U+0008,
   U+000B to U+001F, U+007F and U+0080 to U+009F are refused wherever
   they appear, comments included. TOML refuses the C0 set and DEL in
   comments and strings `[S1 "Comment", "String": CONFIRMED]`; the
   subset also refuses the C1 set, which TOML allows, because a value
   may be echoed to a terminal or a log `[OWN]`.
4. **Whitespace** is space and tab `[S1 "Spec": CONFIRMED]`.
5. **A line** is, after optional whitespace, one of:
   - empty;
   - a comment: `#` and the rest of the line;
   - a table header, then optional whitespace and an optional comment;
   - a key/value pair, then optional whitespace and an optional comment.

   Anything else after a header or a value is refused (expected the end
   of the line).
6. **Table header:** `[`, optional whitespace, a segment, then any number
   of: optional whitespace, `.`, optional whitespace, a segment; then
   optional whitespace and `]` `[S1 "Table": CONFIRMED]`. `[[` opens an
   array of tables, which is refused (§5.5).
7. **Key/value pair:** a segment, optional whitespace, `=`, optional
   whitespace, a value. A `.` after the key (a dotted key) is refused: a
   nested key is written under its table's header `[OWN]` (one way to
   write each key; dotted keys create tables implicitly, which makes
   §5.4's rules harder to audit).
8. **Segments:**
   - a **bare key** is one or more of `A` to `Z`, `a` to `z`, `0` to `9`,
     `_` and `-` `[S1 "Keys": CONFIRMED]`;
   - a **quoted key** is a basic string (point 9) of at least one byte.
     TOML allows an empty quoted key `[S1 "Keys": CONFIRMED]`; the subset
     refuses it `[OWN]`;
   - a quoted key and a bare key with the same text are the same key
     `[S1 "Keys": CONFIRMED]`. Keys are compared byte for byte; case
     matters.
9. **Basic string:** `"`, characters, `"`, all on one line. A character
   is any Unicode scalar value other than `"`, `\`, a control character
   (point 3) or tab, or one of the escapes `\"` and `\\`. Every other
   escape is refused, though TOML defines `\b`, `\t`, `\n`, `\f`, `\r`,
   `\uXXXX` and `\UXXXXXXXX` `[S1 "String": CONFIRMED]`: no
   configuration value needs a control character, and a value that holds
   one could hide text in a log `[OWN]`. Tab is refused inside a string
   for the same reason, though TOML allows it.
10. **Integer:** an optional `-`, then `0`, or a digit 1 to 9 followed by
    digits, where single `_` may stand between two digits
    `[S1 "Integer": CONFIRMED]`. The value is a signed 64-bit integer; a
    value outside that range is refused. `-0` is zero
    `[S1 "Integer": CONFIRMED]`. A leading `+` is refused `[OWN]` (one
    way to write a number).
11. **Boolean:** `true` or `false` `[S1 "Boolean": CONFIRMED]`.
12. **Array:** `[`, then values separated by `,`, with an optional
    trailing `,`, then `]` `[S1 "Array": CONFIRMED]`. Before and after
    each value and comma, and before `]`, any whitespace, line ends and
    comments may appear `[S1 "Array": CONFIRMED]`. Each element is a
    string, an integer or a boolean, and all elements have the same
    type. TOML allows mixed types and nested arrays
    `[S1 "Array": CONFIRMED]`; the subset refuses both `[OWN]`: no key
    needs them, and the schema can then check an element's type once.
    An empty array is allowed.

### 5.2 Caps

Every cap is checked before the bytes it bounds are stored `[OWN]`.

| Cap | Value | Applies to | Reason |
|---|---|---|---|
| `MAX_FILE_BYTES` | 262,144 bytes | the whole file | a configuration file is a few kilobytes; the caller reads at most this plus one byte, so a larger file is seen and refused `[OWN]` |
| `MAX_LINE_BYTES` | 4,096 bytes | one line, without its line end | bounds every column count and every token `[OWN]` |
| `MAX_KEY_BYTES` | 255 bytes | one segment, after escapes are applied | a quoted cell name (253 bytes) fits `[OWN]` |
| `MAX_DEPTH` | 8 segments | one table header | the deepest planned path is two segments (`cells."<name>"`) `[OWN]` |
| `MAX_STRING_BYTES` | 1,024 bytes | one string value, after escapes | holds every planned value: cell names, addresses, interface names and paths of up to 1 KiB; a longer need is a reason to revisit (§11) `[OWN]` |
| `MAX_ARRAY_ELEMENTS` | 256 | one array | the planned lists hold at most tens of entries `[OWN]` |
| `MAX_DEFINITIONS` | 4,096 | key/value pairs plus tables, explicit or implicit, in one file | bounds the document's size independently of the file's `[OWN]` |

What a file can make the parser allocate is bounded by these: every
stored key and string is at most its cap; each table stores its path,
at most `MAX_DEPTH` segments of at most `MAX_KEY_BYTES` each, and a
header creates at most `MAX_DEPTH` tables, so the paths stored for one
header are at most `MAX_DEPTH` times that header's line.

### 5.3 Versions

The file carries no version. The subset may grow, only by accepting TOML
that it refuses today, never by changing the meaning of an accepted file
`[OWN]`.

### 5.4 Decoding

1. The checks run in this order, and the first failure is the error:
   the file size; the byte order mark; UTF-8; the line, character and
   line-length checks of §5.1, points 2 and 3, and §5.2, from the start
   of the file; then the lines in order.
2. **Tables.** A header defines its table. Each shorter prefix of its
   path that is not yet a table is created as an implicit table. A
   header whose table was already defined by a header is refused
   (redefined); a header for a table that exists only implicitly makes
   it explicit, so `[a.b]` followed by `[a]` is accepted
   `[S1 "Table": CONFIRMED]`.
3. **Keys.** A key/value pair belongs to the table of the last header
   before it, or to the root table before any header. A key already
   defined in that table, as a value or as a table, is refused
   (redefined) `[S1 "Keys", "Table": CONFIRMED]`. A header whose path
   passes through a key that holds a value is refused (not a table).
4. A redefinition error names the position of the first definition.
5. **The result** is the document: the explicit tables in the order of
   their headers, each with its path and its header's position; and the
   key/value pairs in file order, each with its table's path, its key,
   its value and the positions of its key and value. Each array element
   carries its own position.

### 5.5 Refused TOML constructs

Each is refused at its first character, with its own error and a hint
`[OWN]`:

| Construct | Seen as | Hint |
|---|---|---|
| literal string, single or multi-line | `'` where a key or value starts | use a double-quoted string |
| multi-line basic string | `"""` | write the value on one line |
| inline table | `{` where a value starts | use a `[table]` header |
| array of tables | `[[` | use one table per item |
| dotted key | `.` after a key | put the key under its table's header |
| float, `inf`, `nan` | a `.`, an `e` or `E` in a number, or the words | write a whole number |
| date or time | a number containing `:`, or four digits then `-` | write the value as a string |
| hexadecimal, octal or binary integer | `0x`, `0o`, `0b` | write the number in decimal |
| `+` sign | `+` where a value starts | write the number without `+` |
| escape other than `\"` and `\\` | `\` and the next character | write the character itself |
| mixed or nested array | an element of another type, or `[` inside an array | use one type per array |
| empty quoted key | `""` as a key | give the key a name |
| byte order mark | EF BB BF at the start | save the file as UTF-8 without a byte order mark |

### 5.6 States and ordering

Does not apply: parsing has no state that outlives one call.

### 5.7 The cell name

The one constructor takes octets, from any source, and applies these
steps in order `[DEC-1]`:

1. Empty input is refused.
2. Input that ends with two dots is refused. Otherwise one final dot, if
   present, is removed.
3. What remains must be 1 to 253 bytes; empty is refused, more is
   refused (too long).
4. It is split at each dot into labels. Labels are checked from the
   first; in each label:
   - an empty label is refused (this refuses a leading dot and two dots
     together);
   - a label longer than 63 bytes is refused;
   - its octets are checked from the first: `A` to `Z` are lower-cased;
     `a` to `z` and `0` to `9` are accepted; `-` is accepted except as
     the label's first or last octet; `_` is refused (underscore); an
     octet of 0x80 or above is refused (not ASCII: an internationalised
     name is accepted only in its ASCII form); any other octet is
     refused.
5. A label in ASCII form of an internationalised name (`xn--...`) is
   accepted under point 4 and not validated further.
6. The result is the lower-cased name without its final dot. Two inputs
   that differ only in ASCII case or in one final dot construct equal
   names.

Each refusal names the byte offset in the input where it was found:
for empty input, 0; for two final dots, the first of them; for a name
too long, 253, the first byte over the limit. A
name of one label is a valid cell name; whether a use accepts it is that
use's rule (§2).

## 6. Arithmetic

| Value | Bound | Method |
|---|---|---|
| file, line, key and string lengths | §5.2 | compared before storing; no sum of untrusted values |
| line and column numbers | at most the file size | counted while scanning; `usize` cannot overflow below the file cap |
| integer value | signed 64-bit | `checked_mul` and `checked_add` (or `checked_sub` for a negative value), digit by digit; overflow is refused |
| definition and element counts | §5.2 | compared before each new definition or element |

## 7. Errors and failure

1. Every error carries a kind and, except for a file over
   `MAX_FILE_BYTES`, the position where it was found. Its message says
   what was found or expected, and the refused constructs of §5.5 add a
   hint. The caller adds the file name (O18, point 2).
2. Parsing stops at the first error and returns no document: nothing of
   a refused file is used `[OWN]`. Reporting more than one error per file
   is not offered.
3. The cell-name constructor returns the reason and the offset; no
   partial name exists.

## 8. Security

1. **Who writes the input:** an administrator (the files under the
   system's configuration directory), a management system (policy, in
   part (c)), and, for a misconfigured system, any user who can write a
   configuration file. The caller decides which files count (DNS client
   specification, 5.6.4, point 6); the parser treats every file as
   untrusted.
2. **Crash or exhaustion:** no panic is reachable (coding standards rule
   1, enforced by the workspace lints); every allocation is capped
   (§5.2); the parser does not recurse, and an array holds no array.
3. **Silent misreading:** the subset accepts only constructs whose
   meaning in TOML is unambiguous and the same here (§3, point 3), and
   refuses the rest. A duplicate key is refused, never "last one wins".
4. **Text that hides from a reader:** control characters, including C1
   and tab, are refused inside strings (§5.1, points 3 and 9).
5. **Two spellings of one cell:** §5.7, point 6 makes case and a final
   dot not matter, so one cell has one name everywhere.

## 9. How the invariants are kept

| Invariant | Kept by | Test that fails if broken |
|---|---|---|
| O18, point 1: a strict subset, our own code | §5.1 and §5.5; the crate has no dependencies | the refusal tests of §10.3 |
| O18, point 2: line, column, expected, hint | §7 | each §10.3 test asserts the position and kind |
| rule 1: no panic | the workspace lints; §6 | the property test "arbitrary bytes never panic" |
| rule 3: caps before allocation | §5.2 | the cap tests at the cap and one above |
| one cell name rule | §5.7, one constructor, private field | the cell-name boundary tests; the compile-fail doc-test |

## 10. Tests

Unit tests are in `crates/ironafs-config/src/syntax/tests.rs` and
`crates/ironafs-config/src/cell_name/tests.rs`; property tests are in
`crates/ironafs-config/tests/it/`.

### 10.1 Golden vectors

Each accepted-file test states its document by hand from §5: scalars,
headers, quoted segments, CR LF line ends, a last line without a line
end, multi-line arrays with comments and a trailing comma.

### 10.2 Round-trip properties

There is no encoder. A property test renders a generated document model
in the subset and checks that it parses back to that model.

### 10.3 Reject corpus

Every construct of §5.5; every cap at its value (accepted) and one above
(refused); a CR without LF; a control character in a comment, a string
and a key position; invalid UTF-8; text after a value; each
redefinition and not-a-table case of §5.4. Each test asserts the error
kind and position.

### 10.4 Fuzz target

`ironafs-config_syntax`: any bytes; asserts that the parser returns,
that an error's position lies within the input, and that an accepted
document respects every cap of §5.2. It is added with the fuzz crate
(issue #45).

### 10.5 Acceptance cases

The cell-name boundaries named by issue #93: a 253-byte name accepted; a
254-byte name with no final dot refused; a 254-byte input ending in one
dot accepted as its 253-byte name; 63 and 64-byte labels; an underscore;
two final dots refused; mixed-case input stored lower-cased.

## 11. Revisit if

- O18: "the TOML subset cannot express a setting a later phase needs, or
  administrators' tools cannot write it."
- C4: "a boundary below forces a dependency that point 4 forbids, the
  first implementation of a crate shows it has no content of its own" (C4,
  "Revisit if", first two triggers).
- A planned key needs a value longer than a cap of §5.2, or a type the
  subset refuses.
- The cell-name rule is checked against public cell lists before the
  generation record is frozen (issue #93), and that check finds a
  cell the rule refuses.

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| S1 | SPECIFICATION | TOML v1.0.0, <https://toml.io/en/v1.0.0>, sections "Spec", "Comment", "Keys", "String", "Integer", "Boolean", "Array", "Table" (read 2026-11-21) | §5.1, §5.4, §5.5 |
| DEC-1 | DECISION | Issue #93, the canonical cell-name rule (owner's decision, 2026-11-20) | §1, §5.7 |
| ADR | DECISION | ADR 0004, C4, O17, O18, O19, O47 | §1, §2 |
| OWN | OWN | This specification's own choices, each with its reason where it is made | throughout |

### 12.1 Observed behaviour

None.

## 13. Open questions

1. Whether the cell-name rule refuses a cell named in public cell lists:
   checked before the generation record is frozen (issue #93).

## 14. Review

No review record yet.
