<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# XDR profile

Status: draft (2026-11-20). Raised in issue #42.

Kind: protocol. Class: S. Every AFS reply, callback and token IronAFS
decodes passes through this codec, and a hostile or broken peer controls
those bytes ([README](README.md), "Review class per kind").

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (X1, D1 and so
on) are in section 12.

## 1. What this implements

ADR 0004, overview, constraint 4 (a constraint of the whole ADR; it
changes only with the ADR):

> **Untrusted input everywhere:** fileserver replies, callbacks, XDR,
> local IPC, configuration, requests from the operating system, and SMB
> messages before authentication. No panics, checked arithmetic and byte
> limits apply to all of it (coding standards rules 1 to 3).

ADR 0004, C3, point 1 (**Working**):

> Rx, XDR, the RPC codecs and the security classes are pure state
> machines: bytes, time and randomness in, actions out. They open no
> socket, read no clock and draw no random numbers.

ADR 0004, C4, point 1 (**Working**), the sentence and the table row:

> **Pure crates** have no I/O and no `unsafe`, and are fuzzed where they
> decode:
>
> | `ironafs-xdr` | the XDR codec, with size caps |

ADR 0004, C4, point 4, first item (**Working**):

> no pure crate depends on a driver or on `ironafs-client`;

ADR 0004, C6, point 1 (**Firm**):

> Each protocol component is written from a behaviour specification in
> `docs/specs/` that names its sources, under ADR 0001. No specification,
> no code.

ADR 0004, C6, point 2 (**Firm**), the part this specification rests on:

> The sources are: […] XDR (RFC 4506); […]

ADR 0004, C6, point 4 (**Firm**):

> A fact about the AFS protocols that this ADR could not tie to a public
> source is an input to a specification, never something to code from.

ADR 0004, N31, points 1 and 2 (**Firm**):

> 1. A wire fact is taken only from an allowlisted specification passage
>    or an independently generated black-box fixture, recorded with its
>    edition, section or fixture provenance.
> 2. Never from implementation history, interface-definition files,
>    generated definitions, or live-cell captures holding credentials or
>    private data.

ADR 0004, O47, points 2 and 3 (**Working**), the parts that apply:

> 2. **Property tests** use our own seeded generator and shrinker (rule
>    21).
> 3. **Fuzzing** covers every decoder: XDR, […]

ADR 0004, O47, point 4 (**Working**), the part that applies:

> Fuzz targets follow cargo-fuzz's layout `[LAY-038]`. […] Where the fuzz
> directory sits in the workspace rests on A12.

Section 9 shows where each is kept and which test fails if it is broken.

## 2. Scope

1. **Covered:** the subset of XDR that IronAFS's codecs use (5.1), the
   encoding rules for each type, the cap model for every variable-length
   item (5.2), the decoder's checks and their order (5.4), the encoder's
   canonical form (5.5), the error set (7), and the tests (10).
2. **Not covered, and where it is covered:**
   - which AFS message has which fields and types, its caps' values, and
     its tolerance decisions: the protocol specification for each
     interface (volume location, file service, callbacks);
   - how a reply or a request is delimited in an Rx call, and bulk file
     data that follows an XDR header as raw bytes: the Rx and RPC
     specifications;
   - what the bytes of a string mean (character set, forbidden bytes such
     as NUL or `/` in a name): the specification of the field's consumer,
     for names the name codec (ADR 0004, L24 and L25);
   - integrity of the bytes: the security class, before this codec sees
     them (5.4, point 12);
   - the notation of IronAFS's own interface definitions and the
     generator that turns them into codecs: the generator's
     specification. This specification states only the rules the
     definitions must meet (5.1, points 4 and 5; 5.2, points 1, 6, 9 and 10).
3. **Not part of the profile:** the types in 5.1, point 3.

## 3. The design in short

1. XDR encodes everything big-endian in units of four bytes, with zero
   padding after data whose length is not a multiple of four
   `[X1 §3: CONFIRMED]`.
2. IronAFS uses a closed subset of XDR's types: integers, hypers,
   enumerations, booleans, opaque data, strings, arrays, structures,
   discriminated unions and void. No floating point and no optional data.
3. Every variable-length item has a named cap in bytes. The decoder checks
   it right after reading the length word, before it allocates or reads
   anything else, together with what is left of every enclosing array's
   cap. Every read, fixed-size items and length words included, goes
   through a cursor that stops at the tightest of those limits. An
   array's element count is checked against the bytes actually present,
   so a claimed count can never reserve memory the input does not back.
   Every message has a memory budget on the bytes the decoder asks the
   allocator for, counted by the decoder itself and checked before each
   request; the generator also proves from the caps, before any input
   arrives, that no input can reach it. Inline storage and nesting depth
   have limits of their own.
4. The decoder is strict by default: non-zero padding, a boolean other
   than 0 or 1, an undeclared enumeration value, a union discriminant with
   no arm, and trailing bytes are each rejected. A protocol specification
   may relax one of these for one field, only with a source, and says so.
5. The encoder writes only the canonical form, and refuses every value the
   decoder would reject.
6. The codec is pure, has no dependencies, never panics, and its errors
   carry offsets, names and the specification's own constants, never a
   value read from the input; an error a decode entry point returns
   from a secret-bearing message carries not even an offset. The
   encoder builds its errors with no offset, and returns an error from a
   handwritten encoder as that encoder made it, except a
   `MessageTooLarge` from its size computation, which the encoder
   replaces with the message's own (7, point 2).

## 4. Terms

- **Unit:** four bytes. Every encoded item is a whole number of units
  `[X1 §3: CONFIRMED]`.
- **Length word:** the unsigned integer that precedes variable-length
  opaque data, a string, or a variable-length array; for opaque data and
  strings it counts bytes, for an array it counts elements
  `[X1 §4.10, §4.11, §4.13: CONFIRMED]`.
- **Padding:** the 0 to 3 bytes after data whose length is not a multiple
  of four, which make the item a whole number of units
  `[X1 §3, §4.9 to §4.11: CONFIRMED]`.
- **Declared bound:** the maximum a protocol's own definition states for a
  variable-length item, written `<m>` in XDR's notation: bytes for opaque
  data and strings, elements for arrays. Absent (`<>`), it is 2³² − 1
  `[X1 §4.10, §4.11, §4.13: CONFIRMED]`.
- **Cap:** IronAFS's own limit on a variable-length item or a message,
  always in bytes, with a stable name (5.2).
- **Message:** the top-level value of one call to decode or encode, for
  example one RPC's arguments or results.
- **Input:** the complete, bounded byte buffer handed to the decoder for
  one message.
- **Canonical encoding:** the one encoding of a value that 5.5 defines.
- **Tolerance:** a recorded decision, per field, to accept an input the
  strict rules of 5.4 reject (5.4, point 11).
- **Minimum encoded size** of a type, `s_min`: the fewest bytes any value
  of that type encodes to.
- **Array limit:** for a variable-length array being decoded, the input
  offset its elements may not pass: the offset just after its count word,
  plus its cap. Arrays nest, so the **tightest limit** at any point is the
  smallest limit of the arrays enclosing it. What is left of an
  enclosing array's cap is that limit less the current offset.
- **Bounded cursor:** the decoder's position in the input, with two
  ends: the tightest limit, where a variable-length array encloses the
  position, and the input's end. Every byte the decoder reads or copies
  is taken through it, and no read passes either end (5.4, point 5).
- **Requested bytes:** the bytes the decoder asks the allocator for in one
  reservation: the element count it reserves times the `size_of` of the
  element's Rust type on the target. The decoder counts them itself, from
  its own request, never from the capacity the allocator returns (5.2,
  point 9).
- **Memory budget:** a message's limit, in bytes, on the sum of the
  requested bytes of every reservation made while decoding one input,
  whether the decode succeeds or fails (5.2, point 9). It does not measure
  the allocator's own overhead or any capacity beyond the request.
- **Inline size:** the `size_of` of a decoded value's Rust type on the
  target: the bytes it occupies wherever it is held, on the stack or
  inside its parent (5.2, point 10).
- **Secret field:** a field that a protocol specification marks as holding
  a ticket, a key or other secret material (5.1, point 8). A
  **secret-bearing message** is one whose type's secrecy flag is set: the
  type's own marks, on the rows of its fields or, for a named type, on
  its own row, combined with the flags of its components, elements and
  arms (5.1, point 8; 5.2, point 5). So its type can hold, at any
  depth, a secret field or a value of a named type marked secret. The
  flag belongs to the type and applies to every value of it, including
  one that holds no secret, such as an empty array of a secret named
  type or a union whose selected arm is plain.

## 5. Specification

### 5.1 Layout

The profile's types, each with its encoding. Unless a point says
otherwise, every item starts on a unit boundary relative to the start of
the message, because every item is a whole number of units
`[X1 §3: CONFIRMED]`.

1. **The types in the profile:**

   | Type | XDR notation | Encoding | Size | Source |
   |---|---|---|---|---|
   | Signed integer | `int` | two's complement, most significant byte first | 4 | `[X1 §4.1: CONFIRMED]` |
   | Unsigned integer | `unsigned int` | binary, most significant byte first | 4 | `[X1 §4.2: CONFIRMED]` |
   | Enumeration | `enum` | as a signed integer | 4 | `[X1 §4.3: CONFIRMED]` |
   | Boolean | `bool` | the enumeration FALSE = 0, TRUE = 1 | 4 | `[X1 §4.4: CONFIRMED]` |
   | Hyper, unsigned hyper | `hyper`, `unsigned hyper` | 64-bit, two's complement or binary, most significant byte first | 8 | `[X1 §4.5: CONFIRMED]` |
   | Fixed-length opaque | `opaque x[n]` | n bytes, then padding | n + pad(n) | `[X1 §4.9: CONFIRMED]` |
   | Variable-length opaque | `opaque x<m>` | length word n, n bytes, padding | 4 + n + pad(n) | `[X1 §4.10: CONFIRMED]` |
   | String | `string x<m>` | as variable-length opaque | 4 + n + pad(n) | `[X1 §4.11: CONFIRMED]` |
   | Fixed-length array | `T x[n]` | elements 0 to n − 1 in order | sum of elements | `[X1 §4.12: CONFIRMED]` |
   | Variable-length array | `T x<m>` | element count n, then elements 0 to n − 1 in order | 4 + sum of elements | `[X1 §4.13: CONFIRMED]` |
   | Structure | `struct` | components in declaration order | sum of components | `[X1 §4.14: CONFIRMED]` |
   | Discriminated union | `union switch` | discriminant, then the arm it selects | 4 + arm | `[X1 §4.15: CONFIRMED]` |
   | Void | `void` | nothing | 0 | `[X1 §4.16: CONFIRMED]` |

   pad(n) is (4 − (n mod 4)) mod 4, and the padding bytes are zero
   `[X1 §3: CONFIRMED]`.

2. **A union's discriminant** is a signed integer, an unsigned integer or
   an enumeration, booleans included `[X1 §4.15, §6.4 (5): CONFIRMED]`.
   A discriminant is a value of its own type first (5.4, points 7 and 9)
   `[OWN]`. A valid discriminant selects the explicit arm whose case
   lists it. If no case lists it, it selects the default arm, if present.
   With neither, the union has no valid encoding for that discriminant
   `[X1 §4.15: CONFIRMED]`.
3. **Not in the profile:** single, double and quadruple precision floating
   point `[X1 §4.6 to §4.8]`, and optional data (`T *x`) `[X1 §4.19]`.
   `[OWN]` No IronAFS specification needs them today. Optional data is
   how XDR describes linked lists, and a recursive decoder of a long list
   can exhaust its stack `[X1 §8: CONFIRMED]`; leaving it out keeps every
   decode's depth fixed (point 4). A specification that needs one of these
   types amends this specification first, at class S.
4. **No recursive types.** A type may not contain itself, directly or
   through other types. The definitions' type graph is acyclic, so every
   message's nesting depth is fixed and known when its codec is generated
   `[OWN]`, for the reason in point 3. The elements of an array are decoded
   and encoded by a loop, never by recursion, so an element count never
   sets the stack's depth `[OWN]`.
5. **No zero-width array elements.** A type's `s_min` is 0 when every
   value of it encodes to no bytes: void, a fixed-length opaque of length
   0, a fixed-length array of 0 elements, and a structure or fixed-length
   array made only of such types. Such a type may not be the element type
   of an array, fixed or variable. The generator computes `s_min` for
   every type and refuses a definition that breaks this `[OWN]`: an array
   of zero-width elements would let a count drive a loop that no input
   bytes back (5.2, point 6). Every other type's `s_min` is a whole number
   of units, so at least 4 bytes `[X1 §3: CONFIRMED]`. Structures have at
   least one component `[X1 §6.3: CONFIRMED]`. A union whose selected arm
   is void still encodes its 4-byte discriminant `[X1 §4.15: CONFIRMED]`.
6. **Strings are bytes.** XDR defines a string's bytes as ASCII
   `[X1 §4.11: CONFIRMED]`, and leaves checking which characters are legal
   to the application `[X1 §8: CONFIRMED]`. `[OWN]`, a departure from
   X1 §4.11: the codec decodes and encodes a string as a byte sequence and
   checks nothing about its content: NUL bytes, bytes above 0x7F and `/`
   all pass. This is an IronAFS extension, kept because AFS names are not
   known to be ASCII and the consumer must see the bytes the peer sent.
   The field's consumer decides what is legal (section 2, point 2). The
   codec never hands a string to an interface that stops at NUL.
7. **Integers are not converted.** A signed integer decodes to a signed
   32-bit value, an unsigned one to an unsigned 32-bit value, a hyper to a
   64-bit value. Any widening, narrowing or meaning (a time, an
   identifier) is the protocol specification's `[OWN]`; the codec does no
   arithmetic on them.
8. **Secret fields.** A protocol specification marks each secret field,
   and each named type whose values are secret, in its cap table (5.2,
   point 5). `[OWN]`, from coding standards rule 8,
   and keeping the crate free of dependencies (section 3, point 6):
   - in a secret-bearing message the decoder copies no input bytes into
     the decoded value, whatever the field: a misframed input can put a
     key's bytes into any field, not only the secret one. Every opaque
     and string field, fixed or variable, is borrowed from the input, and
     so is every integer, hyper and open enumeration: the decoded value
     holds a reference to its 4 or 8 bytes, and the generated accessor
     converts them when the caller reads the field. A closed enumeration
     or a boolean is checked and held as the declared value it names. No
     copy of an input byte outlives a failed decode (5.4, point 13); the
     caller owns the input and zeroes it on drop;
   - the caller that keeps a secret beyond the input's life copies it
     into the secret type rule 8 sets;
   - the encoder writes a secret-bearing message only into an output
     buffer the caller supplies and wipes; the entry point that returns
     a buffer of the encoder's own does not build for such a message.
     The caller's buffer either has no allocation, and the encoder
     makes one fallible reservation call for its whole size before
     writing, which acquires at most one allocation and leaves any it
     acquires with the caller, or already has room for the whole
     message and is written in place; an allocated buffer that is too
     small is refused with `AllocationFailed`. The encoder never grows,
     moves or frees an allocation the caller's buffer holds, so no byte
     of the message the encoder writes into it is left in a block the
     caller cannot reach (5.5, point 3). The rule is the same for every
     message, secret-bearing or not, so a buffer that once held a secret
     is safe to pass for any other;
   - **secrecy is one flag per type, and on encode it is structural.**
     Every encodable type and every message type declares, in one
     place, whether a value of it can hold a secret field, or a value of
     a named type marked secret, at any depth.
     A type has that one declaration, so the flag is the same wherever
     it is read. A type's flag is at least that of each of its
     components, elements and arms: a component is written only through
     an encoder typed for the component's own type, so a parent cannot
     write it any other way, and a type that declares itself plain while
     writing a secret component does not build. A message's descriptor
     is its cap table's message row as the generated code carries it for
     the message's type: its cap name, message cap and memory budget
     only, with no secrecy of its own. The encoder's check of each
     component, the refusal by the entry point that returns the
     encoder's own buffer, and the decoder's redaction of errors
     (section 7, point 2) all read the type's one flag, so they cannot
     disagree. The generator sets each type's own flag from the rows of
     its cap table marked in the Secret column, field rows capped or not
     and a named type's own row, and combines it with the flags of its
     components, elements and arms (5.2, point 5, "Which type's
     flag a marked row sets"). So every message that can
     hold a secret, directly, through a generic caller or as a
     component of another message, is refused at build time by the
     entry point that returns the encoder's own buffer;
   - **decoding takes everything from the type.** The decode entry
     points take the message's type and no descriptor: the message cap,
     the memory budget and the secrecy all come from the type, so a type
     is never decoded under another message's caps, budget or secrecy.
     The test entry point may lower the type's memory budget for one
     decode, never raise it (5.2, point 9.2). This binds each call of an
     entry point, and only that call: a decoder that calls an entry
     point again starts a separate decode, under the type it names
     (5.2, point 9.2, "One decode, one ledger");
   - redaction covers the whole decoded value, not only the secret
     field, for the same reason: misframing can put a key's bytes in any
     field. The generated type of a secret-bearing message, every
     structure, union and array inside it, and the wrapper that each of
     its field accessors returns, borrowed bytes and integers alike, have
     a `Debug` that prints the type and field names only, never a value,
     a length or a count, and no `Display`. The raw value is reached only
     through an explicitly named method of the wrapper, so printing it
     takes a deliberate call that review sees. An error that a decode
     entry point returns from a secret-bearing message carries no offset
     or length (section 7, point 2);
   - **limits, written down as rule 8 requires.** The crate cannot zero
     memory without `unsafe` (rule 5). So these are not zeroed: the words
     the decoder reads into registers and stack temporaries during a
     decode; the lengths of borrowed slices and the element counts of
     arrays, which are length and count words from the input, each
     already checked against its cap; and which declared value a closed
     enumeration or boolean holds. On encode, the byte arrays the encoder
     forms from an integer or hyper before it writes them, in registers
     and stack temporaries, are not zeroed either. The encoder's own
     zeroing of the output buffer is best effort (5.5, point 3); the
     caller's wipe is what holds, and it reaches only the bytes written
     into the caller's allocation. The compiler checks secrecy only as
     far as the types carry it, and these rest on the generator, its
     tests and review: a type's own flag comes from its rows marked in
     the cap table (5.2, point 5), and nothing checks it, since the
     primitive writes take bytes and integers, which do not say whether
     they are secret, and that includes bytes a value reveals from a
     borrowed secret and writes as its own field; code that encodes a
     value apart and copies its bytes into a message by hand is not
     stopped by the types; structure is checked on encode only: on
     decode the entry point reads the message type's flag, and that the
     type of a secret-bearing message borrows every field from the
     input, as the first item of this point requires, is the
     generator's to carry and test; and a handwritten decoder that
     starts a separate decode of bytes it holds (5.2, point 9.2, "One
     decode, one ledger") decodes them under that other type's secrecy,
     which may copy them, and nothing stops it; and a handwritten
     encoder that runs a decode and passes on its positioned error,
     which the encode entry points return as it is, a `MessageTooLarge`
     from `encoded_len` excepted (section 7, point 2).
     Review sees all five.

### 5.2 Caps

1. **Every variable-length item has a cap.** Each variable-length opaque,
   string and variable-length array field in an IronAFS definition carries
   a named cap. A definition with a variable-length item and no cap is
   refused when its codec is generated `[OWN]`, from coding standards rule
   3 and ADR 0004's constraint 4. An absent declared bound means 2³² − 1
   `[X1 §4.10, §4.11, §4.13: CONFIRMED]`, so without a cap a peer could
   claim up to 4 GiB.
2. **Every cap is in bytes** (coding standards rule 3):
   - for opaque data and strings, the cap limits n, the data bytes,
     excluding the length word and the padding;
   - for a variable-length array, the cap limits the total encoded bytes
     of its elements, excluding the count word.

   An element count alone is never a cap `[OWN]`: an array of strings
   with a small count can still hold gigabytes.
3. **Every message has a cap** on its total encoded size in bytes `[OWN]`.
   It bounds everything inside it, fixed-size parts included, and is
   checked before decoding starts (5.4, point 1).
4. **Declared bound and cap are separate.** Where a protocol's definition
   states a bound m, the specification of that message records m as the
   declared bound with its source, and the decoder rejects any length or
   count above it: such an encoding is not valid
   `[X1 §4.10, §4.11, §4.13: CONFIRMED]`, which call encoding a value above
   the maximum an error. The cap may be lower than the declared bound in
   bytes; then IronAFS refuses some valid input, and the message's
   specification says why and what the user sees `[OWN]`. A cap is never
   higher than the declared bound allows: for opaque data and strings, m
   bytes; for an array, m times the largest encoding of its element type,
   which the element's own caps make finite.
5. **How a specification declares caps.** Each protocol or format
   specification has, per message, a table with these columns `[OWN]`:

   | Column | Content |
   |---|---|
   | Cap name | stable, lower case, dotted: `<interface>.<message>.<field path>`, for example `vl.get_entry.reply.name`; the message cap is `<interface>.<message>`; the row of a named type (below, "An item that is no field" and "A secret named type with no cap") is `<interface>.<type>` |
   | Field | the field the cap applies to, "message", or, for a named type's row, the values of that type it covers |
   | Kind | opaque, string, array or message; for a secret field or a secret named type with no cap (below), its type: integer, unsigned integer, hyper, unsigned hyper, enumeration, boolean, fixed-length opaque, fixed-length array, structure or union |
   | Declared bound | m, its unit (bytes or elements) and its source, or "none" |
   | Cap | the value, in bytes, or "none" for a secret field or a secret named type with no cap; the Declared bound is then "none" too |
   | Direction | in (decoded by IronAFS), out (encoded by IronAFS), or both |
   | Memory budget | message rows only: the memory budget in bytes (point 9) |
   | Secret | yes for a secret field (5.1, point 8), or, on a named type's row (below, "An item that is no field" and "A secret named type with no cap"), for a named type whose values are secret; otherwise empty |
   | Why | the reason for the value |

   **A secret field with no cap.** Whether a field has a row depends
   on whether the field itself needs a cap (point 1), not on its width.
   Only a variable-length opaque, string or variable-length array field
   needs one. Every other field has no cap of its own, and so no row,
   unless it is secret: an integer, unsigned integer, hyper, unsigned
   hyper, enumeration or boolean, a union's discriminant, a fixed-length
   opaque, a fixed-length array (whose encoded size may vary, as
   `pair`'s does in 10.1, but whose variable-length elements each have
   a row, their named type's, below), and a field whose type is a
   structure or a union.
   A secret one has a row of its own: its cap name names the field as
   above, its Kind is its type, its Cap and Declared bound are "none",
   and its Secret is yes. Such a row sets no cap constant; it records
   only the flag. A closed enumeration or boolean marked secret is still
   held as the declared value it names (5.1, point 8, first item and
   limits).

   **An item that is no field.** A variable-length item that is an
   array's element, fixed-length or variable-length, or that is the
   whole value of a type decoded on its own, has no field path. A
   definition gives such an element a named type, as RFC 4506's
   language requires, since there a declaration of variable-length
   opaque data, a string or a variable-length array takes no further
   array suffix `[X1 §6.3: CONFIRMED]`, whatever notation the
   definitions use (section 13, Q4). Its cap is that
   named type's, in one row with the cap name `<interface>.<type>`: one
   row per named type, written once in the specification that declares
   the type, which applies wherever such a value of the type is decoded
   or encoded, in every array of it and in every message. A component or
   arm whose type is that named type is a field, and keeps a row of its
   own by its path, with a cap of its own: in 10.1, `test.label` caps
   each `label` in `pair`, `names` and `tags`, while `test.note.owner`
   and `test.sealed.tag` cap the `label` fields. A Secret mark on a
   named type's row, this one or the cap-free row below, sets that named
   type's own flag, so every array, structure and message that holds
   the type is secret through its components (below).

   **A secret named type with no cap.** A named type whose values are
   not a variable-length item (an integer, enumeration or boolean type,
   a fixed-length opaque or array, a structure or a union) needs no item
   cap, and so has no item row of its own, unless its values are secret,
   as a named fixed-length opaque key's are. Used as a whole message, it
   still has a message row with its message cap (point 3). A secret one
   has an item row of its own: its cap name is `<interface>.<type>`, its
   Field names the type, its Kind is the type's own kind, its Cap and
   Declared bound are "none", and its Secret is yes. Such a row sets
   that named type's own flag and emits no cap constant; like a secret
   field's row with no cap, it records only the flag.

   The generated codec carries each cap as a named constant derived from
   the cap name, and the tests that check it cite the name. A cap name is
   never reused for another field. A message row's cap name, cap and
   memory budget are carried as the descriptor of the message's type.

   **Which type's flag a marked row sets.** A named type's row is
   declared by that named type itself, and a mark on it sets that type's
   own flag. Every other row names a field by its
   path from the message, and the path's last step is the field. That
   field is declared by exactly one structure or union, whatever the
   path through which the message reaches it:
   - a component of a structure is declared by that structure;
   - an arm of a union is declared by that union, and so is the
     union's discriminant, named as its declaration names it;
   - a field reached through the elements of an array, fixed or
     variable, is declared by the element type, as a step of the path
     through any other component is declared by that component's type;
   - a component or arm held out of line (point 10.2) is declared as if
     it were held inline: holding it out of line changes where it is
     stored, not which type declares it.

   A row marked secret sets the own flag of that declaring type. The
   flag belongs to the type, not to the message whose table holds the
   row: the type has one flag (5.1, point 8), so the field is secret in
   every message that reaches it, through any path. Every row, in any
   message's table, that reaches the same field of the same declaring
   type carries the same Secret mark; the generator refuses tables that
   disagree `[OWN]`. A mark does not set the flag of the field's own
   type, nor clear it. That type keeps the flag its own rows and its
   components give it: an integer type stays plain, so a parent with a
   secret integer field says so through its own flag, and a field whose
   type is secret by its own rows or components makes its parent secret whether or
   not the parent's row marks the field. A type's one flag is its own
   flag or the flag of any of its components, elements and arms. So
   whether a message is secret-bearing, that is whether its type's own
   marks or the flags of its components, elements and arms set its
   flag, which covers every marked field and every marked named type
   below it, is carried as its type's flag, not in the descriptor.
6. **A claimed count must be backed by input.** Before it reserves memory
   for an array of n elements, or enters the loop over them, the decoder
   checks that n × `s_min` of the element type is no more than the bytes
   remaining in the input, no more than the array's cap, and does not
   carry the offset past the tightest limit of the arrays enclosing it
   `[OWN]`. The same check runs on entry to a fixed-length array, with its
   declared n. Since an
   element's `s_min` ≥ 4 (5.1, point 5), a count can never reserve more
   element slots than a quarter of the input holds. As each element is
   decoded, the bounded cursor keeps its exact bytes within the array's
   limit, and so within its cap (5.4, point 5).
7. **Caps nest, and are checked before the child allocates.** Each item
   is checked against its own cap; an array's cap also bounds the sum of
   its elements; the message cap bounds everything. A variable-length item
   inside an array is checked, right after its length word and before any
   byte is copied or memory reserved, against its own cap and against the
   tightest limit of the arrays that enclose it (5.4, point 5). An
   element can therefore never allocate past what its enclosing arrays
   still allow. No cap is computed from the input `[OWN]`.
8. **Caps are constants.** A cap is set by the specification and compiled
   in. Configuration may lower a cap where a specification says a site
   may; nothing raises a cap above the specification's value `[OWN]`.
9. **Decoded memory has a budget, counted by the decoder and proved
   before coding.** `[OWN]`, from coding standards rule 3. The budget
   measures requested bytes (section 4), which the decoder knows exactly,
   not the capacity the allocator returns. Rust guarantees only that a
   reservation gives at least the capacity requested, and the allocator
   may give more `[R1: CONFIRMED]`; a `Vec` of a zero-sized type reports a
   capacity of `usize::MAX` and allocates nothing `[R1: CONFIRMED]`. So
   neither capacity nor any claim about the allocator enters the budget.
   1. Each message's specification sets its memory budget in bytes (the
      cap table's "Memory budget" column).
   2. **The ledger.** Every heap request the decoder makes goes through
      one reservation routine, which keeps a running total of requested
      bytes for the decode. Before each request, it computes the
      request's bytes and the new total with checked arithmetic (section
      6, A9 and A11); if the new total would pass the memory budget, it
      returns `BudgetExceeded` and requests nothing. The crate's Clippy
      configuration denies the allocating constructors and methods that
      bypass the routine (`Vec::with_capacity`, `Vec::reserve`,
      `Box::new`, `to_vec`, `collect` and the like) through
      `disallowed-methods`, with the routine itself the one named
      exception; review covers anything the list misses. A
      safe test hook, an argument of a test entry point, records each
      request (10.2, P6; 10.3). That entry point, like the ordinary one,
      takes the message's type and no descriptor (5.1, point 8). Its
      budget argument can lower the type's memory budget for one decode
      and never raises it: the ledger's budget is the smaller of the
      two, the effective budget that `BudgetExceeded` reports.

      **One decode, one ledger.** Each call of an entry point creates
      one decoder, with its own cursor, its own ledger and the secrecy
      of the type it names. The guarantees of this point, of point 9.3
      and point 9.5, and the trace of the test hook, cover the requests
      made through that decoder, and only those. A profile-compliant
      decoder, generated or handwritten, decodes every component,
      element and arm through the decoder it is given, so through the
      one cursor and the one ledger, and under the outer type's secrecy
      (5.1, point 8). A decoder that instead calls an entry point on
      bytes it holds starts a separate decode: a fresh ledger with the
      inner call's effective budget (the inner type's, or, through the
      test entry point, the smaller of that and the budget the call
      passes), the inner type's cap, and the inner type's secrecy. Its
      requests are outside the outer budget, the outer
      bounds r × cap and q × cap ÷ 4, and the outer trace; and if the inner
      type is plain, it may copy bytes of a secret-bearing outer
      message, which the first item of 5.1, point 8 forbids. The codec
      does not prevent such a call: the entry points are public
      functions, and nothing tells them they were called from inside
      another decode. That a decoder makes no such call rests on the
      generator, which never emits one, and on review of any
      handwritten decoder. The entry points do redact what such a
      decoder returns (section 7, point 2), which is a guarantee about
      the returned error only.
   3. **The static proof.** The generator also computes, for each type T,
      a heap ratio r(T): an upper bound on the bytes requested for a
      decoded value of T per byte of its encoding. Integers, enumerations,
      booleans, hypers, fixed-length opaque data held inline and void
      request nothing: r = 0. A variable-length opaque or string requests
      at most its n data bytes: r = 1. A structure or a union holds its
      components inline: r is the largest r of its components or arms. A
      variable-length array of n elements requests n element slots, each
      `size_of` the element's Rust type, and n ≤ its encoded bytes ÷
      `s_min`: r = ⌈slot size ÷ `s_min`⌉ + r(element). A fixed-length
      array of n elements decodes them into n element slots reserved
      through the ledger, then moves them, with no further request, into
      the array, which is held inline; no element is made up to fill a
      slot. Its n ≤ its encoded bytes ÷ `s_min` too, so its r is that of
      a variable-length array of its element type: r = ⌈slot size ÷
      `s_min`⌉ + r(element). A fixed-length array of no elements
      reserves nothing and holds nothing: r = 0. A value held out of
      line (point 10) counts as a one-element array of its type: r = ⌈slot size ÷ `s_min`⌉ +
      r(T), where `s_min` ≥ 4, because a type of zero width is never held
      out of line (point 10.2). A slot of a zero-sized Rust type is 0 bytes,
      so it adds nothing. The worst case for a message is
      r(message type) × the message cap, in checked 64-bit arithmetic
      (section 6, A8). Slot sizes depend on the target, so the generator
      emits this bound as a compile-time constant assertion that the
      compiler evaluates on each target: a message whose worst case
      overflows, or is above its memory budget, does not build there.
      The assertion checks the message type against its own descriptor,
      the one the decode entry points use, so what is proven is what is
      decoded.
      The ledger (point 2) still checks every request at run time, so a
      fault in the proof becomes `BudgetExceeded`, never an allocation
      past the budget.
   4. **Reservations.** Each buffer is reserved once, fallibly
      (`try_reserve_exact`, which requests no speculative extra
      `[R1: CONFIRMED]`), for exactly the element count it will hold. A
      reservation that fails returns `AllocationFailed`. The buffer is
      then filled only by `push`, which Rust guarantees never reallocates
      while the reported capacity is sufficient `[R1: CONFIRMED]`, and
      never by a bulk insertion such as `extend_from_slice`, which may
      reallocate even when it need not `[R1: CONFIRMED]`. Since the
      capacity is at least the count reserved and no more than the count
      is pushed, no buffer grows, and every request the decoder makes is
      one the ledger saw. A buffer that becomes a box, the slot of a
      value held out of line or the bytes of a fixed-length opaque held
      out of line (point 10.2), must have a capacity of exactly its
      count, or the decode fails with `AllocationFailed`: Rust converts
      a buffer to a box in place only when its capacity equals its
      length, and otherwise reallocates `[R1: CONFIRMED]`, a request the
      ledger would not see. So the conversion never shrinks or moves the
      block. A buffer of a zero-sized type has no block, and is boxed
      whatever capacity it reports. The decoder makes no temporary heap
      request outside the ledger.
   5. **Not measured.** The allocator's per-allocation overhead and any
      capacity it gives beyond a request are outside the budget. So the
      generator also bounds how many reservations a decode makes, as a
      count per unit of encoding, q(T), alongside r(T) and in the same
      checked arithmetic (section 6, A8): q = 0 for a type that reserves
      nothing; q = 1 for a variable-length opaque or string, one
      reservation for an item of at least one unit; the largest q of its
      components or arms for a structure or a union; max(1, q(element))
      for a variable-length array, one reservation for its buffer plus
      its elements' over at least one unit of count word; 1 + q(element)
      for a fixed-length array, one reservation for its slots, which no
      count word backs, made over the n × `s_min` ≥ 4 bytes of its
      elements, present in the input before the slots are reserved (5.4,
      point 2), plus its elements'; q = 0 for a fixed-length array of no
      elements, which reserves nothing; and 1 + q(T) for a value of type
      T held out of line, which reserves its own slot over at least one unit of T, present
      in the input before the slot is reserved (point 10.2; 5.4, point
      2). Nested out-of-line values therefore each count,
      though they rest on the same encoded bytes. One decode makes at
      most q(message type) × message cap ÷ 4 reservations, and the
      generator emits that count beside the heap bound. Every
      reservation goes through the ledger and counts, including one of
      zero elements or of a zero-sized type, which requests 0 bytes. A
      deployment sizes its memory from the budgets and that count, not
      from this codec.
   6. The caller that assembles the input enforces the message cap while
      it buffers: it stops and fails as soon as the bytes received pass
      the cap, and never buffers a whole message to check it afterwards
      (5.4, point 1). The Rx and RPC specifications carry this out.
10. **Inline storage and nesting are bounded.** `[OWN]` Heap is not the
    only memory a short input can make the decoder use: a fixed-length
    opaque or a union arm is held inline, and a Rust enum for a union is
    as large as its largest arm whichever arm the input selects. So:
    1. every generated type's inline size is at most `xdr.inline_max`,
       256 bytes. The generator emits a compile-time constant assertion
       per type, evaluated on each target; a type above the limit does
       not build there;
    2. a fixed-length opaque longer than 64 bytes is always held out of
       line, in a buffer reserved through the ledger and boxed only at
       exactly its count (points 9.2 and 9.4), or, in a
       secret-bearing message, borrowed from the input (5.1, point 8); a
       definition may also mark a union arm or a structure component
       out of line, held in a one-element buffer reserved the same way.
       That is how a type with a large arm meets point 1. The generator
       refuses to hold out of line a type whose `s_min` is 0 (5.1,
       point 5), such as a void arm: no encoded bytes would back its
       reservation (points 9.3 and 9.5). Before it reserves the slot,
       the decoder checks, through the bounded cursor, that `s_min` of
       the held type fits within the tightest limit and the bytes
       remaining, as for a one-element array (5.4, point 2), so a
       truncated input never reserves a slot that no input bytes back;
    3. a message's nesting depth, counted in structures, unions and
       arrays from the message down, is at most `xdr.depth_max`, 16. The
       type graph is acyclic (5.1, point 4), so the generator computes
       the depth and refuses a deeper definition;
    4. the decoder holds at most one value in progress per nesting level,
       on the stack, and moves it into its parent's storage when it is
       complete. Its stack use is then bounded by the depth limit times
       the inline limit, plus the compiler's frames, which no
       specification can bound; the test `decode_fits_small_stack`
       (10.2) checks the result on each target.

    The values of `xdr.inline_max` and `xdr.depth_max` hold profile-wide
    (section 13, Q7).

### 5.3 Versions

Not applicable: XDR has no magic value or version on the wire, and
IronAFS does not control it ([README](README.md), "Formats IronAFS does
not control").

### 5.4 Decoding

1. **Message size first.** If the input is longer than the message cap,
   the decoder returns `MessageTooLarge` and reads nothing `[OWN]`. This
   is a second check: the caller has already enforced the cap while it
   buffered the input (5.2, point 9.6).
2. **Fixed-size items.** For an integer, enumeration, boolean, hyper or
   fixed-length opaque, the decoder takes the item's bytes through the
   bounded cursor (point 5): if they would pass the tightest limit, it
   returns `CapExceeded` naming that array's cap; otherwise, if they
   would pass the input's end, `Truncated` `[OWN]`. A
   fixed-length array's encoded size is not known on entry when its
   elements vary in size, so on entry the decoder checks n × `s_min` of
   the element type, as for a variable-length array (point 4, steps 3 and
   4), before it reserves anything or starts its loop `[OWN]`. A value
   held out of line (5.2, point 10.2) is checked on entry the same way,
   as a one-element array: if `s_min` of its type would pass the
   tightest limit, `CapExceeded`; otherwise, if it would pass the
   input's end, `Truncated`. Only then is its slot reserved through the
   ledger. So every reservation, accepted input or not, rests on input
   bytes already present: a length or count word read, the n × `s_min`
   bytes of a fixed-length array's elements, or the `s_min` bytes of an
   out-of-line value `[OWN]`.
3. **Variable-length opaque data and strings,** in this order `[OWN]`:
   1. the length word is taken through the bounded cursor, as a
      fixed-size item (point 2): `CapExceeded` if its 4 bytes would pass
      the tightest limit, else `Truncated` if they would pass the input's
      end;
   2. n is above the declared bound: `LengthOverBound`;
   3. n is above the cap, or the offset where the item would end (after
      its n + pad(n) bytes) passes the tightest limit of the arrays
      enclosing it: `CapExceeded`, naming the cap that failed;
   4. n + pad(n) is above the bytes remaining: `Truncated`;
   5. the n data bytes are taken through the bounded cursor: borrowed
      from the input, always in a secret-bearing message (5.1, point 8),
      or copied into a buffer reserved through the ledger for exactly n
      bytes (5.2, point 9), which can return `BudgetExceeded` or
      `AllocationFailed`;
   6. any padding byte is not zero: `NonZeroPadding` (point 8).

   No allocation happens before step 5.
4. **Variable-length arrays,** in this order `[OWN]`:
   1. the count word is taken through the bounded cursor, as in point 3,
      step 1: `CapExceeded` or `Truncated`;
   2. n is above the declared bound: `LengthOverBound`;
   3. n × `s_min` is above the array's cap, or the offset after the count
      word plus n × `s_min` passes the tightest limit of the arrays
      enclosing it: `CapExceeded`, naming the cap that failed;
   4. n × `s_min` is above the bytes remaining: `Truncated`;
   5. room for exactly n element slots is reserved through the ledger,
      fallibly (5.2, point 9): `BudgetExceeded` or `AllocationFailed`;
      the elements are then decoded in order by a loop (5.1, point 4;
      point 5).
5. **Array elements, limits and the bounded cursor.** Each
   variable-length array being decoded sets its limit (section 4) and
   keeps it until its last element ends; the bounded cursor's ends are the
   tightest limit in force and the input's end. Every read and every copy, at any depth, is
   taken through the cursor: fixed-size items, length and count words,
   data bytes and padding (points 2 to 4, 8). A read that would pass the
   cursor's end at a limit returns `CapExceeded` naming the cap of the
   array whose limit it is; one that would pass the input's end returns
   `Truncated`; the first applies when both would `[OWN]`. Every
   variable-length item inside an array is also checked against the
   tightest limit before it allocates (point 3, step 3; point 4, step 3),
   and so is the entry check of a fixed-length array inside it (point 2).
   No element can therefore end past its array's limit, and no byte past
   a limit is ever read. A fixed-length array has no cap of its own; its
   elements count against the limits of the arrays around it.
6. **Structures** decode their components in declaration order
   `[X1 §4.14: CONFIRMED]`. The first error stops the decode.
7. **Discriminated unions** decode the discriminant, then the arm it
   selects `[X1 §4.15: CONFIRMED]`. In this order `[OWN]`:
   1. the discriminant is decoded and validated as a value of its own
      type: a boolean other than 0 or 1 is `InvalidBool`, and an
      undeclared value of a closed enumeration is `UnknownEnumValue`
      (point 9), unless a tolerance for this union's discriminant
      (point 11) normalises it first;
   2. the valid, normalised value selects the arm by 5.1, point 2. If it
      selects no arm, the decode returns `UnknownDiscriminant`;
   3. the arm is decoded.

   An arm the decoder does not know cannot be skipped: XDR gives an arm
   no length `[X1 §4.15: CONFIRMED]`. A protocol that adds arms in later
   releases is handled only by a sourced arm, a default arm, or a variant
   framed separately (for example inside variable-length opaque data),
   as that protocol's specification states.
8. **Padding bytes must be zero.** XDR's padding is zero bytes
   `[X1 §3: CONFIRMED]`, so that the same data encodes the same way
   everywhere and can be compared or checksummed `[X1 §5 (5): CONFIRMED]`.
   `[OWN]` The decoder rejects a non-zero padding byte with
   `NonZeroPadding`: accepting it would let two different byte strings
   decode to one value, which point 12's integrity rule and the
   round-trip property P2 forbid. Point 11 says how a field may relax this.
9. **Enumerations and booleans are closed.** Encoding an integer that the
   enumeration does not declare is an error
   `[X1 §4.3: CONFIRMED]`, and a boolean is the enumeration FALSE = 0,
   TRUE = 1 `[X1 §4.4: CONFIRMED]`. `[OWN]` The decoder rejects an
   undeclared enumeration value with `UnknownEnumValue`, and a boolean
   other than 0 or 1 with `InvalidBool`. A protocol specification may
   declare an enumeration field **open**: the decoder then keeps an
   undeclared value as its raw signed integer, and the encoder writes it
   back unchanged. An open enumeration is an IronAFS extension, a
   departure from X1 §4.3, which makes encoding an undeclared value an
   error. An open field says why, for example that the peer adds values
   in later releases, and needs the same source as a tolerance (point
   11).
10. **Trailing bytes.** When the message's value is complete and bytes
    remain, the decoder returns `TrailingBytes` `[OWN]`. Fewer than 4
    remaining bytes are trailing bytes too, since every item is whole
    units `[X1 §3: CONFIRMED]`. A message's specification may instead
    declare "ignore trailing bytes", with a source, under point 11.
    Ignored bytes still count against the message cap and are never
    interpreted.
11. **Tolerance is per field, sourced, and tested.** A protocol
    specification may relax point 8, 9 or 10 for one named field or
    message, and only with a SPECIFICATION source or an OBSERVED one that
    shows a supported peer needs it `[OWN]`, under the wire-format rule
    that tolerance is a recorded decision. Each relaxation states:
    - what is accepted (for example, "any non-zero value is TRUE");
    - the decoded value;
    - that the encoder still writes only the canonical form (5.5);
    - its test, which checks the decoded value and the canonical
      re-encoding.

    Points 1 to 6 and steps 2 and 3 of point 7 are never relaxed. A
    tolerance under point 9 applies to a union's discriminant only where
    the specification names that discriminant, and the normalised value
    then selects the arm (point 7, step 1). No tolerance applies to a cap.
12. **Integrity comes first.** The decoder runs only on bytes that the
    security class has already accepted. Any checksum, MAC or signature
    is verified over the bytes received, never over a re-encoding of the
    decoded value (the `wire-format` skill). This codec verifies nothing
    itself; the Rx and security-class specifications say what is
    checked, and before which decode `[OWN]`. Where the check itself
    needs fields decoded first (for example a challenge, or the header of
    a token that names its key), the security-class specification names
    those fields. They are decoded with this codec, under the same caps
    and strict rules, and nothing else in the message is used before the
    check passes.
13. **Nothing partial.** On any error, the decode returns the error and
    no value, not even part of one `[OWN]`. What was decoded before the
    error is dropped. In a secret-bearing message nothing was copied
    from the input through the decoder (5.1, point 8), so no copy of a
    secret is left, within
    the limits that point states. The codec keeps no state between calls
    (ADR 0004, C3, point 1), so nothing is left behind.

### 5.5 Encoding

1. **Canonical form.** The encoder writes every item as 5.1 sets out:
   most significant byte first, zero padding, lengths and counts equal to
   the data's, booleans as 0 or 1, and enumerations only as declared
   values `[X1 §3 to §4.16: CONFIRMED]`. An open field keeps its raw
   value (5.4, point 9) `[OWN]`. A union is written as its discriminant
   followed by the arm that value selects under 5.1, point 2
   `[X1 §4.15: CONFIRMED]`.
2. **The encoder refuses what the decoder rejects** (the `wire-format`
   skill), using the caps for the output direction. The sizing pass also
   checks that lengths and counts fit in 32 bits and applies the
   encode-only `DefaultArmMismatch` refusal to preserve the value round
   trip (P1). The checks below are `[OWN]` except where marked.
   - for opaque data and strings, n is checked against the declared bound
     (bytes) and then against the cap in the "out" direction (bytes);
   - for a variable-length array, the element count n is checked against
     the declared bound (elements), and the encoded bytes of its elements
     against the array's cap (bytes); a count is never compared with a
     byte cap, nor a byte total with a bound in elements;
   - the failures are `LengthOverBound` and `CapExceeded`; XDR itself
     makes encoding a length above the declared maximum an error
     `[X1 §4.10, §4.11, §4.13: CONFIRMED]`;
   - a length or count that does not fit in 32 bits is
     `LengthNotRepresentable`;
   - a union's explicit arm supplies its case discriminant. Its default
     arm keeps a discriminant of the declared type, including any raw
     value the type admits for an open field. When sizing a default-arm
     value, apply 5.1, point 2 to that discriminant. If it selects an
     explicit arm, return `DefaultArmMismatch` naming the discriminant
     field (section 7). If it selects the default arm, size the arm and
     encode it with the stored discriminant. A mismatch violates P1
     because a decoder can accept the resulting bytes as the explicit
     arm when the payload fits both layouts.

   For every discriminant kind, keeping the declared type lets the
   default arm use the existing value representation and sizing pass.
   Decoding, sizing and writing share the union's arm selection. Sizing
   checks the stored discriminant before sizing the arm, even if every
   arm has the same encoded size. The check is part of sizing the union
   wherever it occurs, including inside a structure, array or another
   union.
3. **The wipe comes first, then the output cap, and the allocation
   never moves.** `[OWN]`, from coding standards rules 3 and 8. These
   steps run in this order, and are the same for every message,
   secret-bearing or not. They describe the entry point that writes
   into the caller's buffer; the entry point that returns a buffer of
   the encoder's own runs them on a new, empty buffer of its own
   (point 6):
   1. First, the encoder overwrites the output buffer's bytes and its
      spare capacity with zeros, and leaves it empty. It writes the
      spare capacity only with `push`, up to the capacity it read first,
      never with a bulk insertion, so the wipe itself never grows, moves
      or frees the allocation `[R1: CONFIRMED]`. So a message refused by
      any later step, the cap check included, still leaves a buffer that
      held earlier bytes empty and zeroed, as best it can (step 6).
   2. Then it computes the message's encoded size, with checked
      arithmetic and the checks in point 2 in the same pass. It returns
      `MessageTooLarge` if the size exceeds the message cap, or does not
      fit in 64 bits or in the platform's `usize` (section 6, A10).
      Sizing follows the wipe. It finishes before any allocator request,
      before the reservation call in step 3, and before any message byte
      is written.
   3. A buffer with an allocation too small for that size is refused
      with `AllocationFailed`: no byte of the message is written, and
      the buffer stays as step 1 left it, empty and zeroed, with its
      allocation. Every other buffer gets one reservation call at that
      size, made fallibly (`try_reserve_exact`; `AllocationFailed` if the
      call fails). The call and an allocator request are not the same:
      the call does nothing when the capacity already covers the size,
      a size of 0 on an empty buffer included `[R1: CONFIRMED]`, and it
      may fail its own capacity check before it reaches the allocator.
      So a buffer whose allocation already holds that size is written
      in place, with no allocator request, and a buffer with no
      allocation gets at most one allocator request: none if the size is
      0 or the call fails before it reaches the allocator. A call that
      fails leaves the buffer as it was, with no allocation acquired.
   4. The encoder never grows, shrinks, moves or frees an allocation the
      caller's buffer already holds, nor the one step 3's call acquired
      for a buffer that had none, which stays with the caller's buffer. Rust
      guarantees a capacity of at least the request, not exactly it
      `[R1: CONFIRMED]`, which is enough: the encoder writes exactly the
      computed size, only with `push` (5.2, point 9.4), so the buffer
      never grows, and every byte the encoder writes into the output
      stays in the one allocation the caller's buffer holds. Any spare capacity
      holds no message byte, and the length of the written output, not
      the capacity, is what the caller sends. The bytes left to write
      are one count, shared by the encoders of the message and of every
      component, element and arm in it, and each write takes its bytes
      from that count before it pushes them. Each write is also refused
      unless the buffer's capacity has room for it, so the allocation's
      staying in place does not rest on the count alone. Either check
      refuses with `MessageTooLarge` naming the message. So a value that
      catches a component's unwind and writes on is held to the same
      size, and the allocation still does not move.
   5. On every error, whatever raises it, and when a value's encoding
      unwinds out of the encoder, the encoder zeroes and empties the
      buffer again, as in step 1 (point 6).
   6. The encoder's zeroing is best effort: the compiler may remove
      ordinary writes to memory that is about to be overwritten or
      released, and the crate cannot write any other way (rule 5). The
      wipe that holds is the caller's. Since the allocation never moves,
      the caller's wiper (rule 8), applied after use, on error, before
      reuse and on drop, reaches every byte the encoder wrote into the
      caller's allocation. It does not reach the temporaries the encoder
      forms on the way, such as the bytes of an integer before they are
      pushed (5.1, point 8, limits).
4. **Types carry the rest.** Where the Rust type makes an invalid value
   unrepresentable (a closed enumeration, a union without a default arm),
   the compiler enforces point 1 and no runtime check is written
   `[OWN]` (coding standards rule 15, "a contract the compiler enforces").
5. **No reserved fields.** XDR defines no reserved field of its own
   (X1 §4 lists none) `[X1 §4: CONFIRMED]`. A protocol's reserved field is
   the protocol specification's, and that specification states its value
   on encode.
6. **On error,** the encoder returns the error, and what it leaves
   depends on the entry point `[OWN]`:
   - the entry point that writes into the caller's buffer leaves that
     buffer empty, zeroed as best it can (point 3). A buffer that came
     with an allocation keeps it, at the same address and capacity. A
     buffer that came with none keeps an allocation only if point 3,
     step 3's reservation call acquired one: it has none after an error
     before that call or from it, or when the call requested nothing,
     and otherwise keeps the allocation the call acquired, which is now
     the caller's to wipe and free. An unwind
     out of the encoder leaves the buffer the same way. Its caller sends
     nothing from that buffer;
   - the entry point that returns a buffer of the encoder's own zeroes
     that buffer as best it can and then frees it, so no caller can
     wipe it. That is why it does not build for a secret-bearing
     message (5.1, point 8).

### 5.6 States and ordering

Not applicable: the codec is stateless between calls (ADR 0004, C3,
point 1).

## 6. Arithmetic

Every length and count is untrusted. All of this uses checked arithmetic
and fallible conversions (coding standards rule 2); none of it saturates.

| Id | Operation | Operands' range | Bound and result at overflow |
|---|---|---|---|
| A1 | pad(n) = (4 − (n mod 4)) mod 4 | n < 2³² | 0 to 3; cannot overflow |
| A2 | n + pad(n) | up to 2³² + 2 | computed in 64 bits; cannot overflow there. Reached only after the cap check, so in practice no more than cap + 3 |
| A3 | offset + item size, against the input's length | offset ≤ input length | `checked_add`; overflow is `Truncated` |
| A4 | n × `s_min` | n < 2³², `s_min` small | `checked_mul` in 64 bits; overflow is `CapExceeded` |
| A5 | an array's limit (offset + 4 + cap), and an item's end offset against the tightest limit | offsets ≤ input length, cap < 2³² | `checked_add` in 64 bits; overflow is `CapExceeded` |
| A6 | a 64-bit size or count to the platform's `usize`, on decode | any | `try_from`; failure is `CapExceeded` (no cap exceeds 32 bits). On encode the conversion is A10's |
| A7 | a length to the 32-bit length word, on encode | any `usize` | `try_from`; failure is `LengthNotRepresentable` |
| A8 | heap ratios and r × message cap, and reservation counts q and q × message cap ÷ 4, in the generator (5.2, points 9.3 and 9.5) | caps < 2³², slot sizes small | `checked_mul`, `checked_add` and a ceiling division in 64 bits; overflow refuses the definition |
| A9 | n × element slot size, the requested bytes of a reservation | n already checked under A4 | `checked_mul` in 64 bits; overflow is `BudgetExceeded` |
| A10 | the encoder's sum of encoded sizes, and that size as the platform's `usize` (5.5, point 3) | any value | `checked_add` in 64 bits and `try_from`; overflow or a failed conversion is `MessageTooLarge`, naming the message |
| A11 | the ledger's running total plus a request (5.2, point 9.2) | total ≤ memory budget, request from A9 | `checked_add` in 64 bits; overflow, or a sum above the budget, is `BudgetExceeded`, before anything is requested |
| A12 | inline sizes and nesting depth, in the generator and the compile-time assertions (5.2, point 10) | sizes small, depth ≤ 16 | `checked_add` and `checked_mul`; overflow refuses the definition or fails the build |

`[OWN]` for every row; the operations follow from 5.1, 5.2, 5.4 and 5.5.

## 7. Errors and failure

1. **One closed error set for the codec** `[OWN]`. Tests assert the exact
   kind (coding standards rule 15). Outside a secret-bearing message, a
   decode error carries the byte offset, from the start of the input, of
   the item that failed: the length or count word for a length error, the
   first non-zero byte for a padding error, the discriminant or value
   word for a value error, the item whose read would pass a limit or the
   input's end for a cursor error, and, for `BudgetExceeded` or
   `AllocationFailed`, the length or count word whose reservation
   failed, or the start of an item without one (a fixed-length array or
   opaque, or a value held out of line) whose reservation or boxing
   failed. An error a decode entry point returns from a secret-bearing
   message carries no offset (point 2). The encoder builds its own errors
   with no offset. The table's Decode and Encode columns cover the
   errors the codec builds itself. Any error kind can reach an encode
   caller: an encode entry point returns an error a handwritten encoder
   passes on, of any kind, as that encoder made it, except a
   `MessageTooLarge` from `encoded_len`, which it replaces with the
   message's own (point 2).

   | Kind | When | Also carries | Decode | Encode |
   |---|---|---|---|---|
   | `MessageTooLarge` | the message is longer than its cap; on encode, also a computed size that overflows 64 bits or does not fit in the platform's `usize` (section 6, A10), a value that writes more or fewer bytes than its computed size, or a write past the buffer's capacity (5.5, point 3) | cap name, cap; the input's length (decode, not from a secret-bearing message) | yes | yes |
   | `Truncated` | the input ends inside an item | bytes available (not from a secret-bearing message) | yes | no |
   | `LengthOverBound` | a length or count is above the declared bound | field, bound | yes | yes |
   | `CapExceeded` | a length, count or running total is above its cap, or a read or an item passes an enclosing array's limit | cap name, cap | yes | yes |
   | `NonZeroPadding` | a padding byte is not zero | none | yes | no |
   | `InvalidBool` | a boolean is neither 0 nor 1 | field | yes | no |
   | `UnknownEnumValue` | a closed enumeration's value is not declared | field | yes | no |
   | `UnknownDiscriminant` | a union's discriminant has no arm and there is no default | field | yes | no |
   | `DefaultArmMismatch` | a value holds the default arm but its discriminant selects an explicit arm (5.5, point 2) | discriminant field | no | yes |
   | `TrailingBytes` | bytes remain after the message | count (not from a secret-bearing message) | yes | no |
   | `BudgetExceeded` | a reservation would carry the requested bytes past the decode's effective memory budget, or its size overflows (5.2, point 9.2; section 6, A9 and A11) | message cap name; the effective memory budget: the type's, or the lower budget the test entry point was given (5.2, point 9.2) | yes | no |
   | `AllocationFailed` | a fallible reservation failed (5.2, point 9.4; 5.5, point 3); on decode, a buffer to be boxed, of an element type of nonzero size, has a capacity other than its count, checked when the item is complete and reported at its start (a buffer of a zero-sized element type is boxed whatever capacity it reports, 5.2, point 9.4); on encode, the caller's output buffer has an allocation too small for the message (5.5, point 3) | none | yes | yes |
   | `LengthNotRepresentable` | a length or count does not fit in 32 bits | field | no | yes |

2. **No content in errors.** `[OWN]`, from coding standards rule 8. An
   error carries only its kind, an offset, a field or cap name, and the
   specification's own constants (a cap, a bound, a memory budget), or
   the lower budget a test decode was given, plus the input's
   length and the bytes available, which come from the buffer's size
   rather than its content. It never carries a value read from the input:
   not a byte, not an invalid boolean, enumeration or discriminant word,
   and not a claimed length or count. A misframed input can put a key's
   bytes where a length word or a boolean is expected, so any word read
   from the input may be secret. A field or cap name is a constant of the
   generated codec. Its display text is lower case with no trailing
   period, and the type is `Send + Sync + 'static` (coding standards rule
   4).

   **Redaction happens where the error is made, not where it is
   reported, and again where it is returned.** In a secret-bearing
   message, an offset, the input's length, the bytes available and a
   trailing count all reflect lengths read from the input, which may be
   a secret's bytes. So:
   - **where it is made:** every error built through the decoder that a
     secret-bearing message's decode is given (5.2, point 9.2, "One
     decode, one ledger") is built without them: it carries its kind,
     the field or cap name, and the specification's constants only, and
     the position is absent;
   - **where it is returned:** the decode entry points remove the position from
     every error a secret-bearing message's decode returns, whatever
     built it. That covers an error that a handwritten decoder passes
     on from a separate decode it started: such an error was built under
     that decode's own type, and if that type is plain and the error is
     positioned, it carries an offset and may carry an extent (an
     `InvalidBool` carries an offset only), both relative to the bytes
     the separate decode was given;
   - **between the two:** while such a forwarded error is still inside
     the handwritten decoder, before the entry point returns it, it
     holds that position. A handwritten decoder keeps any positioned
     error it holds to itself until it returns it: it does not display,
     log, count or otherwise report it. The codec cannot check that; it
     rests on review, with the other limits of 5.1, point 8.

   Every reporting boundary outside the decode, `Display`, `Debug`, a
   log line, a counter label, a reply over local IPC or a test's failure
   message, then sees only what the returned error holds, and no caller
   needs a redaction rule of its own. The encoder's errors carry no
   value from its input either: a length it refuses is named by its
   field, never printed. The encoder builds its own errors with no
   position. The encode entry points return an error from a value's
   `encode` or `encoded_len` as that code made it, except that a
   `MessageTooLarge` from `encoded_len` is replaced by the message's
   own; they remove no position. A handwritten encoder that runs a
   decode and passes on its error passes on that decode's position, so
   a handwritten encoder of a secret-bearing message does not return
   such an error. The codec cannot check that; it rests on review
   (5.1, point 8, limits).
3. **No panic is reachable** from any input (coding standards rule 1). The
   codec uses no indexing, `unwrap` or `expect` on input-derived values.
4. **Fail closed.** A decode error means the message is not used. What the
   caller does next (abort the call, count the event, report it) is the
   caller's specification. The codec only reports. An encode error leaves
   the caller's output buffer empty, keeping an allocation it came with
   and any the encoder's reservation call acquired for it, or, from the
   entry point that returns the encoder's own buffer, no buffer at all
   (5.5, point 6), so nothing of a refused message can be sent from it.

## 8. Security

Who can send the bytes this codec decodes: any fileserver or volume
location server the client talks to; any host that can reach the
callback service's port; and, where the security class protects no
integrity, anyone on the network path (ADR 0004, A13). They can try:

| Threat | What stops it |
|---|---|
| Exhausting memory with a huge length or count | caps checked before allocation (5.2, points 1 to 3; 5.4, points 3 and 4); counts backed by input (5.2, point 6); enclosing limits checked before a child allocates, and every read bounded by the cursor (5.2, point 7; 5.4, point 5); the memory budget over requested bytes, checked by the ledger before each request and proved by the generator, with fallible reservations that never grow (5.2, point 9), per decode: a separate decode a handwritten decoder starts has its own ledger and budget, and the generator and review keep decoders from starting one (5.2, point 9.2); the caller enforces the message cap while buffering (5.2, point 9.6) |
| Exhausting CPU with many tiny elements | byte caps on arrays, not element counts (5.2, point 2); no zero-width array elements (5.1, point 5); the message cap |
| Exhausting the stack with deep nesting, a long array or a large inline value | no optional data and no recursive types (5.1, points 3 and 4) `[X1 §8: CONFIRMED]`; array elements decoded by a loop (5.1, point 4); limits on inline size and nesting depth, large values held out of line, and `decode_fits_small_stack` (5.2, point 10) |
| Crashing the daemon | no panics, checked arithmetic (section 6; 7, point 3); the fuzz target (10.4) |
| Two encodings of one value, used to slip a change past a check or a cache key | strict padding, booleans and enumerations (5.4, points 8 and 9); integrity over the bytes received (5.4, point 12); round-trip property P2 |
| Leaking a secret through an error, a log or a stray copy | no value read from the input in errors; no offset or length in an error that a decode entry point returns from a secret-bearing message: the decoder builds its own errors without one, and the decode entry points remove one from any error a handwritten decoder passes on from a separate decode, which holds its position only inside that decoder, where it must not be reported (7, point 2); the encoder builds its own errors without one, and review keeps a handwritten encoder of a secret-bearing message from passing on a positioned error, which the encode entry points return as it is, except a `MessageTooLarge` from `encoded_len`, replaced by the message's own (7, point 2); in a secret-bearing message no input byte copied into the decoded value through the decoder it is given, whatever field misframing puts it in, and no value, length or count of any field of a secret-bearing message shown by `Debug` or `Display`; a secret-bearing message encoded only into the caller's buffer, whose allocation, whether it came with the buffer or the encoder's one reservation call acquired it, the encoder never grows, moves or frees, even when a value's encoding unwinds, so the caller's wipe reaches every byte written into it; and secrecy carried by one flag per type, set from the rows its cap table marks, fields capped or not and named types, and combined with the flags of its components, elements and arms, which the encoder and the decoder both read, so a type that can hold a secret does not build into a buffer the encoder returns, and each decode is made under its own type's caps, budget and secrecy (5.1, point 8, with its stated limits, among them a separate decode started inside a decoder, a handwritten encoder's forwarded error and the encoder's temporaries; 5.2, points 5 and 9.2; 5.5, points 3 and 6) |
| Slipping an invalid discriminant past a default arm | the discriminant is validated as its own type before dispatch (5.4, point 7) |
| A name with NUL or `/` reaching the operating system | out of this codec's scope by design (5.1, point 6); the consumer checks it `[X1 §8: CONFIRMED]` |
| An integer used as a size without a check | integers are not converted here (5.1, point 7); the consumer applies rule 2 |

## 9. How the invariants are kept

| Invariant (section 1) | Kept by | Test that fails if it is broken |
|---|---|---|
| Constraint 4: no panics, checked arithmetic, byte limits | 5.2; 5.4; section 6; 7, point 3 | fuzz target `ironafs-xdr_decode` (10.4); reject corpus (10.3), cases R2 to R4, R5, R5b and R12; the workspace lints (coding standards rule 1) |
| C3, point 1: pure, no socket, clock or randomness | the crate has no dependencies and no I/O (section 3, point 6) | PROPOSED: a dependency-graph check in `cargo xtask check` that `ironafs-xdr` has no dependencies and imports no `std::net`, `std::time` or `std::fs` (gap, #46) |
| C4, point 1: no I/O, no `unsafe`, fuzzed, size caps | as C3; `unsafe_code = "forbid"`; 5.2 | the compiler (workspace lint); fuzz target (10.4); reject corpus (10.3), cases R2 to R4 |
| C4, point 4: no pure crate depends on a driver or the client | no dependencies at all | the same dependency-graph check (gap, #46) |
| C6, points 1, 2 and 4: written from a specification with named sources; nothing coded from an unverified fact | this document; every point labelled; section 12 | review of this specification (section 14); a point without a label is a review finding |
| N31, points 1 and 2: wire facts only from an allowlisted specification passage or an independent fixture | every wire fact here cites a section of X1 that register row E1 allowlists; golden vectors use IronAFS's own test types and values, derived by hand (10.1); no capture is used (12.1) | review; each vector's derivation comment |
| O47, point 2: our own seeded generator and shrinker | 10.2 | the generator's own tests, in the crate that owns it: the same seed replays the same cases, and a planted failing property shrinks to its known smallest case. Here, each property takes its seed from that generator and prints it on failure (coding standards rule 15) |
| O47, points 3 and 4: XDR is fuzzed, in cargo-fuzz's layout | 10.4 | the fuzz target builds and runs on the Linux runner |

## 10. Tests

All paths are in the `ironafs-xdr` crate, laid out as coding standards
rules 15 and 20 set out. The tests use small test-only types of IronAFS's
own, built on the codec, with names, values and caps chosen for these tests
`[OWN]`. No codec generator exists yet: these test types are written by
hand to what the generator must emit (5.1 and 5.2), their caps, flags,
ratios and compile-time assertions included. The tests this section
calls the generator's are the generator's own: they belong to the
generator's specification (section 2; section 13, Q4) and land with
the generator, and nothing runs them before then (10.6):

```
enum shade { PALE = 1, DEEP = 4, VIVID = 9 };     /* closed */
union shaded switch (shade tone) {
case PALE: unsigned hyper pale_value;
case DEEP: unsigned hyper deep_value;
default:   unsigned hyper vivid_value;
};
enum kind { PLAIN = 0, LINKED = 1, STAMPED = 2, RETIRED = 3 };
union entry switch (kind k) {                     /* no arm for RETIRED, */
case PLAIN:   void;                               /* and no default      */
case LINKED:  string target<16>;
case STAMPED: unsigned hyper stamp;
};
typedef string label<8>;
struct note {
    string title<32>;
    entry  what;
    label  owner;
    opaque body<64>;
};
label pair[2];
label names<>;
label tags<3>;
enum solo { ONLY = 5 };                           /* one value */
solo solos<>;
struct quad { opaque a[60]; opaque b[60]; opaque c[60]; opaque d[60]; };
union wide switch (bool big) {
case FALSE: void;
case TRUE:  quad block;
};
struct sealed {                                   /* secret-bearing */
    opaque key<16>;                               /* secret */
    unsigned int serial;
    label tag;
};
```

| Cap name | Field | Kind | Declared bound | Cap | Direction | Memory budget | Secret | Why |
|---|---|---|---|---|---|---|---|---|
| `test.note` | message | message | none | 128 | both | 128 | | room for G12 (44 bytes) and the cases of 10.3; R12 is one byte over. The budget is r(`note`) = 1 times the cap |
| `test.note.title` | `title` | string | 32 bytes | 32 | both | | | equal to its bound, so R1 and P4 pin the bound's check before the cap's |
| `test.note.what.target` | `what.target` | string | 16 bytes | 16 | both | | | equal to its bound |
| `test.note.owner` | `owner` | string | 8 bytes | 8 | both | | | equal to its bound (`label`) |
| `test.blob` | a `blob` value (`opaque x<>`; G7, R3), on its own | opaque | none | 16 | both | | | small, so R3's length word is far above it; the named type's row |
| `test.ints` | an `ints` value (`unsigned int x<>`; G9, R4), on its own | array | none | 64 | both | | | room for G9; the named type's row |
| `test.label` | each `label` that is no field: the elements of `pair`, `names` and `tags`, and a `label` decoded on its own | string | 8 bytes | 8 | both | | | equal to its bound; the named type's row (5.2, point 5, "An item that is no field") |
| `test.note.body` | `body` | opaque | 64 bytes | 16, below its bound | both | | | below its bound, so R2 and P4 test `CapExceeded` apart from the bound |
| `test.names` | the array | array | none | 16 | both | | | small, so two or three labels cross its limit (R5, R5b, P4) |
| `test.tags` | the array | array | 3 elements | 16, below the 36 bytes its bound allows | both | | | below what its bound allows, so R1a and P4 isolate `LengthOverBound` |
| `test.solos` | the array | array | none | 16 | both | | | small like `test.names`, so a short count crosses it; a zero-sized element type, so the ledger, not a capacity, is what the budget reads (5.2, point 9) |
| `test.deep` | message | message | none | 192 | both | 1024 | | exactly one value of the 16-level chain; the budget is the test types' default |
| `test.wide` | message | message | none | 244 | both | 1024 | | exactly the TRUE arm and its discriminant; the budget is the test types' default |
| `test.shaded` | message | message | none | 12 | both | 1024 | | a four-byte discriminant and one eight-byte arm, the same size for each arm |
| `test.every` | message | message | none | 256 | both | 4096 | | room for one value of each profile type; the budget covers its out-of-line field and arrays |
| `test.sealed` | message | message | none | 64 | both | 64 | | room for a full `sealed` value; the decoded value holds no copy of the secret field beyond what 5.1, point 8 allows |
| `test.sealed.key` | `key` | opaque | 16 bytes | 16 | both | | yes | equal to its bound; the secret field of 5.1, point 8 |
| `test.sealed.tag` | `tag` | string | 8 bytes | 8 | both | | | equal to its bound (`label`) |

Each `label` in `pair`, `names` and `tags` has the cap 8, from the
named type's row `test.label`. Each test type
decoded on its own, without a message row above, is a message with the
cap 128 and the memory budget 1024, named by the type, so `label`,
`blob`, `names`, `tags`, `solos` and `ints` each have a message cap and
a cap of their own under one name, told apart by their kind; the
largest worst case among them,
the cap-based bound the static proof checks (5.2, point 9.3), is that of
`names`, `tags` and `pair`, ⌈24 ÷ 4⌉ + 1 = 7 per byte on a 64-bit
target, so 896 bytes.
`tags` isolates its guards: its cap of 16 bytes is below the 3 × 12 =
36 bytes its bound allows (5.2, point 4), and a count of 4 empty labels
is above the bound while 4 × 4 = 16 bytes is within the cap, so only
`LengthOverBound` applies. `solos` has a zero-sized Rust element type
on every target, while each element is 4 bytes on the wire: a closed
enumeration with one declared value is held as a type with no stored
field, since the value is known, as the generator must emit it, and the
test crate
asserts at compile time that `size_of` of the element type is 0 on each
target CI builds, so a target where it is not does not build. `solos`
checks that the ledger, not a capacity, is what the budget reads (5.2,
point 9). `sealed` is the secret-bearing test type (5.1, point 8; 7,
point 2).

A further structure, `every`, holds one field of each profile type,
with small caps, including a fixed-length opaque of 100 bytes, held out
of line (5.2, point 10); its own message row gives it room for one
value of each. For the stack and inline limits there are a chain
`deep` of 16 nested structures, each holding an `unsigned hyper` and the
next, the innermost also `opaque pad[64]`, so every value of it is 16 ×
8 + 64 = 192 bytes on the wire, its message cap; and the union `wide`,
whose TRUE arm holds 240 bytes of fixed-length opaque inline, 244 bytes
on the wire with the discriminant, its message cap. The generator's
own tests, when it lands, are to check that it refuses a chain of 17,
and that it emits the inline-size and memory-budget assertions (5.2,
points 9.3 and 10) for every type and message; here the test types
carry those assertions by hand. Every test type has the same cap in both
directions, so P1, P2 and P4 compare like with like; a type whose caps
differ by direction is tested by its own cases. Protocol messages are
tested in their own crates, from their own specifications.

### 10.1 Golden vectors

In `crates/ironafs-xdr/tests/it/golden.rs`. Each expected byte string is
derived by hand from 5.1 and the cited section of X1, and the derivation
is a comment beside the vector. The values are IronAFS's own; none is
taken from X1's examples. Each vector is checked both ways: encoding the
value gives exactly the bytes, and decoding the bytes gives exactly the
value. Bytes are in hexadecimal, grouped by unit.

| Id | Value | Expected bytes | Proves |
|---|---|---|---|
| G1 | `int` 0, −1, −2147483648, 2147483647 | `00000000`, `ffffffff`, `80000000`, `7fffffff` | §4.1: two's complement, most significant byte first |
| G2 | `unsigned int` 1, 4294967295 | `00000001`, `ffffffff` | §4.2 |
| G3 | `hyper` −1; `unsigned hyper` 0x0102030405060708 | `ffffffff ffffffff`; `01020304 05060708` | §4.5 |
| G4 | `bool` FALSE, TRUE | `00000000`, `00000001` | §4.4 |
| G5 | `shade` VIVID | `00000009` | §4.3 |
| G6 | `opaque x[5]` = bytes `61 62 63 64 65` | `61626364 65000000` | §4.9: three zero bytes of padding |
| G7 | `opaque x<>` of length 0, 1, 2, 3, 4 and 5 (bytes `01` to `05`) | `00000000`; `00000001 01000000`; `00000002 01020000`; `00000003 01020300`; `00000004 01020304`; `00000005 01020304 05000000` | §4.10, §3: padding 0, 3, 2, 1, 0, 3 |
| G8 | `label` = "files" | `00000005 66696c65 73000000` | §4.11 |
| G9 | `unsigned int a<>` = [7, 8] | `00000002 00000007 00000008` | §4.13: count, then elements |
| G10 | `pair` = ["a", ""] | `00000001 61000000 00000000` | §4.12: elements of one type, different sizes |
| G11 | `entry` PLAIN (void arm); LINKED with target "x"; STAMPED with stamp 0x0102030405060708 | `00000000`; `00000001 00000001 78000000`; `00000002 01020304 05060708` | §4.15, §4.16 |
| G12 | `note` with title "readme", what LINKED with target "docs", owner "root", body bytes `01` to `05` | `00000006 72656164 6d650000 00000001 00000004 646f6373 00000004 726f6f74 00000005 01020304 05000000` (44 bytes: 12, 12, 8 and 12 per field) | §4.14, with §4.10, §4.11 and §4.15 nested in one value. X1 §7 works a similar example; it is a cross-check of the method only, and none of its schema, values or bytes is used |
| G13 | `void` | empty | §4.16 |
| G14 | `shaded` PALE with `pale_value` 0x0102030405060708 | `00000001 01020304 05060708` | §4.3, §4.5, §4.15: declared discriminant selects its explicit arm |
| G15 | `shaded` VIVID with `vivid_value` 0xfedcba9876543210 | `00000009 fedcba98 76543210` | §4.3, §4.5, §4.15: declared discriminant selects the default arm |

### 10.2 Round-trip properties

In `crates/ironafs-xdr/tests/it/roundtrip.rs`, with the workspace's own
seeded generator and shrinker (ADR 0004, O47, point 2). Each property runs
over the test types above.

- **P1** decode(encode(x)) = x for every value x the encoder accepts.
- **P2** encode(decode(b)) = b for every input b the decoder accepts. With
  no tolerance declared, every accepted input is canonical. A test type
  with a declared tolerance (5.4, point 11) is checked separately: its
  decoded value, and its canonical re-encoding.
- **P3** Every encoding is a whole number of units, and its length equals
  the sum 5.1 gives for the value.

  For `shaded`, exercise both G14 and G15 in P1 to P3. The valid
  discriminant and the selected arm survive the value round trip. The
  byte round trip keeps the original discriminant, including VIVID on
  the default arm. Both messages are exactly 12 bytes.

  Also attempt to encode `shaded` holding the default arm with `PALE`
  and G14's payload. All arms have the same width and the message fits
  `test.shaded`. G14 supplies the valid explicit arm for that
  discriminant and payload. The invalid value's `encoded_len` and both
  encode entry points return `DefaultArmMismatch` naming
  `test.shaded.tone`, with no value, offset or other detail. Check that
  both encode entry points refuse during sizing, before any allocator
  request, reservation call or message write. For the caller's-buffer
  entry point, use two buffers that hold earlier bytes, one with
  `0 < capacity < 12` and one with `capacity >= 12`. Both end empty and
  zeroed as best the encoder can, at the same address and capacity. A
  fresh zero-capacity buffer stays empty with capacity zero.
  The encoder's buffer tests below check the zero writes while the
  bytes are still initialised. These integration cases use the #167
  fixture and are delivered under #59. Run them with
  `cargo nextest run -p ironafs-xdr`.
- **P4** The encoder refuses exactly what the decoder rejects, with the
  same cap in both directions, for values that can be expressed both as
  encoder input and as wire bytes. P1 covers the additional encode-only
  `DefaultArmMismatch` refusal. For each variable-length item, P4 builds
  the smallest over-size value that one guard alone decides, encodes it
  where the encoder can represent it, builds the same bytes by hand, and
  expects the same kind and the same cap or field name from both.
  - opaque data or a string whose cap is below its declared bound, or
    with no declared bound: n = cap + 1 gets `CapExceeded` (`note.body`);
  - opaque data or a string whose cap equals its declared bound: n =
    cap + 1 is above both, and gets `LengthOverBound`, because the bound
    is checked first (5.4, point 3, step 2; 5.5, point 2) (`note.title`).
    A byte cap can never be above a byte bound (5.2, point 4), so for
    these kinds the bound is never tested apart from the cap; this case
    pins the order;
  - an array with a declared bound below 2³² − 1: a count of bound + 1
    whose elements fit within the cap gets `LengthOverBound`, and only
    that guard applies (`tags`: 4 empty labels, 16 bytes, its cap below
    what its bound allows);
  - an array: the smallest encodable element total above the cap, with
    a count within any declared bound, gets `CapExceeded`; its elements
    are whole units, so it is never one byte over (`names`: two labels of
    4 and 8 data bytes, 20 bytes).

  An item with no declared bound has no `LengthOverBound` case: no count
  or length above 2³² − 1 is representable.
- **P5** Every proper prefix of a valid encoding of a message with no
  trailing-bytes tolerance fails with `Truncated`, and never panics.
- **P6** Every decode runs with the ledger's test hook (5.2, point 9.2),
  which records each request: its requested bytes and the cap name of the
  item that made it. For every input of L bytes, accepted or rejected,
  the recorded total is within the message's memory budget, and for an
  accepted input it is at most r(T) × L. The hook records requests, not
  capacities, so a zero-sized element type (`solos`, whose element is
  asserted zero-sized on each target) records 0 bytes for its buffer and
  is checked like any other, and an allocator that gives more capacity
  than requested changes nothing the test reads. No counting allocator
  is used: a Rust global allocator needs `unsafe`, which the workspace
  forbids (coding standards rule 5). That no request bypasses the ledger
  is kept by the `disallowed-methods` list and by review (5.2, point
  9.2). For every input of L bytes, accepted or rejected, the number of
  requests recorded is at most q(T) × L ÷ 4 (5.2, point 9.5), which
  `every`'s out-of-line field and the arrays of labels exercise; every
  proper prefix of a valid `every` checks it on truncated input, and
  R17 (10.3) pins the out-of-line field's entry check. The generator's
  tests, when it lands, are to check that a definition whose worst case
  is above its memory budget gets the failing assertion, that a definition holding a zero-width
  type out of line is refused (5.2, point 10.2), and that two nested
  out-of-line values give q = 2 over one unit. Three cases of 5.2,
  point 9 are unit tests of the decoder's private helpers in
  `src/decode/tests.rs`, not properties over the test types:
  - a fixed-length array of no elements has r = q = 0 (5.2, points 9.3
    and 9.5): a type holding one and a word is proven, by the private
    proof routine, against a message whose memory budget is 0, and
    decodes with no request recorded
    (`fixed_array_of_no_elements_has_no_ratio_and_reserves_nothing`);
  - a buffer to be boxed whose capacity is above its count is refused
    with `AllocationFailed` (`boxed_refuses_a_buffer_with_excess_capacity`),
    and a box keeps the block that was reserved
    (`boxed_keeps_the_reserved_block`);
  - a buffer of a zero-sized type is boxed whatever capacity it reports
    (`boxed_takes_a_zero_sized_type_whatever_its_capacity`) (5.2,
    point 9.4).
- **P7** Setting any one padding byte of a valid encoding to a non-zero
  value gives `NonZeroPadding` at that byte's offset.
- **P8** A decode of `sealed` copies nothing from the input: every byte
  and integer field of the decoded value refers into the input buffer
  (checked by comparing addresses, through test-only accessors of the
  generated value that return the references it holds), and
  the hook records no request for a byte field (5.1, point 8). For any
  two accepted inputs of `sealed`, the `Debug` text of the decoded value,
  and of the wrapper each field accessor returns, is the same, so no
  field's value, length or count reaches it, whichever field misframing
  put a key's bytes in. A generator test, when the generator lands, is
  to check that it emits no `Display` for these types.
- **The encoder's buffer** (5.5, points 3 and 6). The contract holds
  for every message and every error; the suite checks these cases of
  it directly. Unit tests in `src/encode/tests.rs`, most through the
  crate's private routine that encodes under a given descriptor, with
  plain values:
  - a buffer with no allocation receives the message, and a buffer
    that already has room is written in place, at the same address
    (`encode_into_requests_once_and_never_grows`); a message over its
    cap leaves a buffer with no allocation without one
    (`encode_over_message_cap_reserves_nothing`);
  - an allocated buffer given a message larger than the capacity it
    actually reports is refused with `AllocationFailed`, and keeps its
    address and capacity, empty, for a plain message and, through the
    caller's-buffer entry point, a secret-bearing one
    (`encode_into_never_grows_an_allocated_buffer_for_any_message`);
  - four selected errors, `LengthOverBound`, `MessageTooLarge` over
    the cap, `AllocationFailed` and a value that writes past its size,
    each leave a buffer that came with an allocation empty, at the same
    address and capacity (`encode_into_empties_the_buffer_on_every_error`);
  - a secret-bearing message is written through the caller's-buffer
    entry point in place, and into a buffer with no allocation
    (`secret_encode_into_writes_in_place_or_into_a_fresh_buffer`);
  - a write past the buffer's capacity is refused whatever the count of
    bytes left says, takes nothing from that count, and leaves the
    address and capacity unchanged
    (`a_write_past_the_capacity_is_refused_whatever_the_count`);
  - the zeroing step of the wipe leaves the address and capacity
    unchanged and zeroes every byte up to the capacity
    (`zero_to_capacity_overwrites_contents_and_spare_capacity`).

  Integration tests in `tests/it/unwind.rs`, with plain fixtures,
  through the caller's-buffer entry point: a component that writes the
  message's whole size and then unwinds, caught by its parent, which
  then writes one word more, gets `MessageTooLarge` naming the message,
  and the buffer keeps its address and capacity, empty
  (`a_caught_unwind_leaves_the_count_of_what_the_component_wrote`); an
  unwind out of the encoder reaches the caller and leaves the buffer
  empty at the same address and capacity
  (`an_unwind_out_of_the_encoder_leaves_the_buffer_wiped_in_place`).
  An error after the reservation call, in a buffer that came with no
  allocation, is exercised by
  `value_writing_past_its_size_fails_and_leaves_nothing`, which checks
  that the buffer is left empty; that it keeps the allocation the call
  acquired (5.5, point 6) is not checked. Not exercised directly: the
  other error kinds, an unwind in a secret-bearing message, and the
  freeing of the owned-output entry point's buffer on error.
- **Secrecy on encode** (5.1, point 8) is checked by doctests that
  must fail to build, each beside a passing doctest with the same
  fixtures that differs from it in one line, so the failure has one
  cause; the stable toolchain does not enforce the error code a failing
  example names. The pairs are: a type that declares itself plain and
  writes a secret component, against one that takes its flag from what
  it holds (on `Encoder::put`); a type that writes a component with its
  own encoder, against one that writes it with the component's (on
  `Encode::encode`); a secret-bearing message passed through a generic
  caller to the entry point that returns the encoder's own buffer,
  against a plain one (on `encode`); and a descriptor with a secrecy
  field, against one without (on `Message`). Beside them, in
  `src/encode/tests.rs`, compile-time assertions check that a holder of
  a holder of a secret is secret while a holder of a holder of a plain
  value is not, and that a generic caller reads the same flag through a
  type's encoding and through its message; and the unit test
  `a_holder_of_a_secret_encodes_only_into_the_callers_buffer` encodes a
  holder of a holder of a secret through the caller's-buffer entry
  point and a holder of a plain value through the owned-output one.
  That a holder of a secret is classified secret is the compile-time
  assertions' to show; that a secret-bearing message does not build
  through the owned-output entry point is the generic-caller doctest's,
  which passes `Key` itself, not a holder of it. `Key` is a type whose
  own field, a word, is secret, and which declares the flag itself, as
  5.2, point 5 has it; a generator test, when the generator lands, is
  to check that a cap-table row marking a field with no cap secret sets
  the flag of the type that declares the field.
- **The decode entry points** (5.1, point 8; 5.2, point 9.2) take the
  message's type only; integration tests in `tests/it/entry.rs`. The
  cap is the type's: an input one word over it gets `MessageTooLarge`
  naming the type's message. The budget is the type's: a decode that
  needs 8 bytes of a type whose budget is 7 is refused with
  `BudgetExceeded`, and a traced decode with a lowered budget is
  refused before the hook records the request. The traced entry point
  never raises it: budgets of 2⁶⁴ − 1, 8, 7 and 0, against a type
  budget of 7, all give `BudgetExceeded` for a decode that needs 8
  bytes. The secrecy is the type's: the same bytes and decoder under a
  plain and a secret-bearing type give errors of the same kind, and
  the secret-bearing type's `Truncated`, `MessageTooLarge` and
  `TrailingBytes` carry no offset or length, through both entry points.
  A last fixture tests the return boundary only: a handwritten
  secret-bearing decoder that starts a separate plain decode of its own
  bytes, which section 5.2, point 9.2 rules out for a compliant
  decoder. It declares the r and q of the plain type whose decode it
  starts, since that decode reserves what the plain type's decode
  does, and a compile-time assertion checks that, like that plain
  type, it is not proven (5.2, point 9.3). The tested input gives the
  separate decode the first 10 of its 12 bytes, so it fails with
  `Truncated` at the array's entry check, before any reservation. The
  test checks only that the positioned error the decoder passes on is
  returned without its position, through both entry points, and not
  the budget, the trace or the decoder's handling of that error before
  it returns it (section 7, point 2).
- **`decode_fits_small_stack`** decodes the largest valid inputs of
  `deep` (192 bytes), `wide` (its TRUE arm, 244 bytes) and every other
  test type, each within its message cap, on a thread started with a
  64 KiB stack, on each target CI builds (5.2, point 10.4). It asserts
  that each decode succeeds and returns the value that was encoded, so a
  rejection cannot pass for a fit. A stack overflow aborts the test
  process, which fails the run. Protocol crates
  run the same test over their own messages.

### 10.3 Reject corpus

Files under `crates/ironafs-xdr/tests/fixtures/reject/`, one per case, each
with its expected error kind and the section it breaks; checked by
`crates/ironafs-xdr/tests/it/reject.rs`, which asserts the exact kind and
offset, and, through the ledger's test hook (5.2, point 9.2), the exact
reservation requests made before the error. A request made and dropped
before an error is returned is recorded, so "no request" is checked, not
inferred from a value that does not exist.

| Id | Input | Expected | Breaks |
|---|---|---|---|
| R1 | a string whose length is the declared bound + 1 (`note.title`, bound 32: length 33) | `LengthOverBound` | 5.4, point 3.2 |
| R1a | `tags` with count 4 (bound 3) and four empty labels, 16 bytes of elements, within its cap | `LengthOverBound`, the hook records no request | 5.4, point 4.2 |
| R2 | an opaque field one byte over its cap, cap below the declared bound (`note.body`, cap 16, bound 64: length 17) | `CapExceeded` | 5.4, point 3.3 |
| R3 | an `opaque x<>` with a cap of 16: length word `ffffffff` and nothing after it | `CapExceeded`, the hook records no request | 5.4, point 3.3 before 3.4 |
| R4 | array of `unsigned int`, count `40000000`, and 8 bytes after it | `CapExceeded` (n × `s_min` is 4 GiB, above every cap), the hook records no request | 5.2, point 6; 5.4, point 4.3 |
| R4a | the same array type with a cap of 64 bytes, count `00000010` (16 × 4 = 64, within the cap), and 8 bytes after it | `Truncated`, the hook records no request | 5.4, point 4.4 |
| R5 | `names` (cap 16, so its limit is offset 20) with count 2: "abcd" (offsets 4 to 12), then a length word of 8, within the label's own cap of 8 data bytes, with 8 data bytes after it (the item would end at offset 24) | `CapExceeded` naming `test.names`, at the second length word's offset (12); the hook records the request for the array's two slots and the 4 bytes of "abcd", and none for the second element | 5.2, point 7; 5.4, points 3.3 and 5 |
| R5b | `names` with count 3: "abcd" and "abcd" (offsets 4 to 20), then a third length word `00000000` at offset 20, so the input is 24 bytes | `CapExceeded` naming `test.names`, at offset 20: the word itself would pass the array's limit (20), though the input holds it; the hook records the slots and the two 4-byte requests only | 5.4, point 3.1 and point 5 |
| R5a | `pair` with only 4 bytes of input | `Truncated` on entry (2 × 4 > 4), the hook records no request | 5.4, point 2 |
| R6 | `opaque x<>` of length 1, 2 and 3, with each padding byte in turn set to `01` | `NonZeroPadding` | 5.4, point 8 |
| R7 | `bool` `00000002`, and `bool` `ffffffff` | `InvalidBool` | 5.4, point 9 |
| R8 | `shade` `00000002` | `UnknownEnumValue` | 5.4, point 9 |
| R9 | `entry` with discriminant `00000003` (RETIRED: declared, no arm) | `UnknownDiscriminant` | 5.4, point 7.2 |
| R9a | `entry` with discriminant `00000007` (not a declared `kind`) | `UnknownEnumValue`, before any arm is chosen | 5.4, point 7.1 |
| R9b | standalone `shaded` with undeclared discriminant `00000002` and a complete eight-byte payload: `00000002 fedcba98 76543210` | `UnknownEnumValue` at offset 0, before any arm is chosen | 5.4, point 7.1 |
| R10 | a valid `note` followed by `00000000` | `TrailingBytes`, count 4 | 5.4, point 10 |
| R11 | a valid `note` followed by one byte `00` | `TrailingBytes`, count 1 | 5.4, point 10 |
| R12 | a `note` input of 129 bytes, one over its message cap | `MessageTooLarge` | 5.4, point 1 |
| R13 | empty input for an `int` | `Truncated` | 5.4, point 2 |
| R14 | a string's length word, then fewer data bytes than it claims | `Truncated` | 5.4, point 3.4 |
| R15 | `sealed` with a key length of 17; and `sealed` cut off inside `serial` | `LengthOverBound` naming `test.sealed.key`, and `Truncated`; neither error holds an offset or a byte count, and the `Display` and `Debug` text of each names only the kind and the field | 7, point 2 |
| R16 | G12's `note`, decoded with a test ledger whose budget is lowered to 8 bytes | `BudgetExceeded` at offset 16 (the target's length word): the title's 6 bytes fit, and 6 + 4 > 8; the hook records only the 6-byte request | 5.2, point 9.2 |
| R17 | a valid `every` cut 4 bytes into its 100-byte out-of-line fixed-length opaque, so at least one unit of it is present but fewer than its `s_min` of 100 bytes | `Truncated` at the field's offset; the hook records no request for that field, only those of the fields before it | 5.2, point 10.2; 5.4, point 2 |

### 10.4 Fuzz target

`fuzz/fuzz_targets/ironafs-xdr_decode.rs`, named `<crate>_<subject>` under
coding standards rule 20; where `fuzz/` sits rests on ADR 0004's
assumption A12. It decodes arbitrary bytes as each test type of section 10
and asserts:

1. no panic, no overflow, no out-of-bounds access;
2. for every input, accepted or rejected, the requested bytes the hook
   records are within the memory budget, and on success within the bound
   of P6;
3. on success, re-encoding the value gives exactly the whole input, byte
   for byte (P2), so a trailing byte the decoder wrongly accepted fails
   the target;
4. on error, its offset, where it carries one, is no more than the
   input's length;
5. the same input always gives the same result.

Exact error kinds and offsets are asserted by the reject corpus (10.3),
not here.

Its seed corpus is the golden vectors and the reject corpus. Fuzzing runs
on the Linux runner (ADR 0004, O47, point 4).

The runner includes `shaded`. Its tracked seeds include G14, G15 and
R9b. Their executable delivery under issue #59 is class I because it
adds fixtures for the existing discriminant rule (5.4, point 7). Run
`cargo nextest run -p ironafs-xdr` and Linux seed replay with
`cargo --locked xtask fuzz`. As a control for the reject case, a planted
decoder that chooses the default arm before validating `shade` would
accept R9b and must fail that case's exact kind and offset assertion.

### 10.5 Acceptance cases

Not applicable: this is a protocol specification; sections 10.1 to 10.4
are its tests.

### 10.6 Gaps

- The dependency-graph check of section 9 (PROPOSED, #46).
- The deterministic order of sizing errors across a message (#168).
  The later of the arm selection amendment (#165) and the sizing order
  amendment (#168) places `DefaultArmMismatch` in that order. Its
  check at the union precedes sizing the arm (5.5, point 2).
- The codec generator and its own tests (section 10's opening; the
  generator's specification, section 13, Q4). Until it lands, the test
  types are written by hand, and what 5.1 and 5.2 require of the generator
  (reading cap tables, refusing tables that disagree, emitting the
  assertions) is not run by any test here.

## 11. Revisit if

From the working decisions this rests on:

- ADR 0004, C3: "the M2a simulator cannot replay a failing schedule from
  its seed, or a measured workload needs more concurrent waits than a
  bounded pool of threads can hold."
- ADR 0004, C4: "a boundary below forces a dependency that point 4
  forbids, or the first implementation of a crate shows it has no content
  of its own."
- ADR 0004, O47: "the fuzzing engine's platform limits change, or A12 is
  settled differently when the first fuzz target lands."

This specification's own choices:

- an observation shows a supported peer sends non-zero padding, a boolean
  other than 0 or 1, or trailing bytes in a message: that field gets a
  tolerance under 5.4, point 11, and this list says which;
- a protocol specification needs a type outside the profile (5.1,
  point 3);
- a protocol message needs more decoded memory than its memory budget
  can sensibly allow (5.2, point 9), or the heap ratio's bound proves too
  loose to set useful budgets;
- RFC 4506 is updated, obsoleted, or gains an erratum that changes an
  encoding rule (register row E1);
- a toolchain bump changes the `Vec` guarantees, or the in-place
  conversion of a buffer to a box, that 5.2, point 9 and 5.5, point 3
  rest on (register row E2);
- a protocol message cannot meet `xdr.inline_max` or `xdr.depth_max`
  without marking so much out of line that its decode is slower than a
  measured need allows (5.2, point 10).

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| X1 | SPECIFICATION | RFC 4506, "XDR: External Data Representation Standard", May 2006, register of editions row E1 ([README](README.md)), <https://www.rfc-editor.org/rfc/rfc4506>. Also claim record row `[ADR-007]` ([claim record](../reviews/2026-11-20-adr-0004-claims.md)). RFC 4506 §2 states it makes no technical change to RFC 1832, which it obsoletes. | 3, 4, 5.1, 5.2, 5.4, 5.5, 8, 10 |
| R1 | DOCUMENTATION | The Rust standard library documentation for `Vec`, Rust 1.100.0, register of editions row E2 ([README](README.md)), <https://doc.rust-lang.org/1.100.0/std/vec/struct.Vec.html>: the "Guarantees" section (a reservation gives at least the requested capacity and the allocator may give more; `push` never reallocates while the capacity suffices; bulk insertion may reallocate), `Vec::capacity` (a zero-sized element type reports a capacity of `usize::MAX`), `Vec::try_reserve_exact` (no speculative over-allocation; capacity not guaranteed minimal; does nothing when the capacity already suffices) and `Vec::into_boxed_slice` (excess capacity is discarded before the conversion); and, on <https://doc.rust-lang.org/1.100.0/std/boxed/struct.Box.html>, the conversion of a `Vec<T>` into a `Box<[T; N]>` (in place when the capacity equals N, a reallocation otherwise). Facts about allocation only, never wire facts. | 5.2, 5.5 |
| D1 | DECISION | [ADR 0004](../adr/0004-architecture.md): constraint 4; C3, C4, C6 ([part 1](../adr/0004-architecture-1-core.md)); O47 ([part 3](../adr/0004-architecture-3-operations.md)); N31 ([part 4](../adr/0004-architecture-4-negotiation.md)); A12, A13 | 1, 2, 8, 9, 11 |
| D2 | DECISION | [ADR 0001](../adr/0001-licence-and-dont-copy.md), "Don't copy" and "Test tools and test data" | 10, 12 |
| D3 | DECISION | [Coding standards](../coding-standards.md), rules 1 to 5, 8, 9, 15 and 20 | 5.1, 5.2, 6, 7, 10 |
| D4 | DECISION | The `wire-format` skill, `.claude/skills/wire-format/SKILL.md` | 5.4, 5.5, 10 |
| O | OWN | Every point tagged `[OWN]`, with its reason in place | 5, 6, 7 |

No other implementation's source, test data or interface definitions were
read for this specification, and none are cited. It describes XDR in
IronAFS's own words, and quotes none of its text (register row E1). The
only things taken from X1 are its type notation, used in 5.1's table and
10's test types, and the encoding rules themselves, which are functional
facts. The test types and values of section 10 are IronAFS's own.

### 12.1 Observed behaviour

None. Nothing here rests on observation. The strict choices of 5.4,
points 8 to 10, are tested against peers in the protocol specifications'
observations, under section 11.

## 13. Open questions

- **Q1. Strictness against real peers.** No public source says whether
  supported fileservers ever send non-zero padding, a boolean other than
  0 or 1, or trailing bytes. The profile rejects all three, and a protocol
  specification relaxes one field under 5.4, point 11, when observation
  shows the need (#51, the observation work). This does not block the
  codec; it may add tolerances later.
- **Q2. Delimiting a message.** The codec decodes one complete, bounded
  buffer. The Rx and RPC specifications (#55 and #65) decide how a call's
  bytes become that buffer, and how bulk data after an XDR header is read
  without passing through it. If they need incremental decoding, this
  specification changes first.
- **Q3.** Closed: the decoded-memory bound is now set before coding, by
  each message's memory budget and the generator's heap ratio (5.2,
  point 9).
- **Q4. The notation of IronAFS's interface definitions.** Whether they
  are written in RFC 4506's §6 language, restricted to this profile with
  a cap on every variable-length item, or in another form, is the
  generator's specification's choice. This specification only sets
  the rules a definition must meet.
- **Q5. The fuzz directory's place** rests on ADR 0004's assumption A12,
  settled when this first fuzz target lands. The target's name follows
  rule 20 as written; if a hyphen in a fuzz target name is a problem for
  cargo-fuzz, rule 20's naming is the thing to amend.
- **Q6.** Closed: G12 and every other vector use IronAFS's own test types
  and values (section 10), and X1 §7 is a cross-check of the method only,
  so nothing from X1 is incorporated and no `docs/provenance.md` row is
  needed.

- **Q7. The inline and depth limits.** Closed by the owner:
  profile-wide values. `xdr.inline_max` is 256 bytes per generated type,
  fixed-length opaque data longer than 64 bytes is held out of line, and
  `xdr.depth_max` is 16 levels, with `decode_fits_small_stack` run on a
  64 KiB stack (5.2, point 10). One profile-wide pair is the simplest to
  check and review. Revisited when the first protocol specifications
  list their messages; a change moves constants, not structure. Not
  taken: per-message limits in each cap table; no inline limit.

## 14. Review

Not yet reviewed. The record will be in `docs/reviews/`.
