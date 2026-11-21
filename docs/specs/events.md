<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Events: catalogue, redaction, limits and sink encodings

Status: draft (2026-11-21). Raised in issue #94.

Kind: protocol. Class: S. The encodings are bytes sent to receivers
IronAFS does not control (journald, a syslog receiver), and redaction is
the guard that keeps tokens and keys out of logs ([README](README.md),
"Review class per kind"; [docs/workflow.md](../workflow.md) §1). The
catalogue and the encoded fields are also a public contract that
operators parse (class P), which class S covers.

Sections keep their numbers once the specification is reviewed. Labels
follow [README](README.md), "Labelling claims". Source ids (J1, R1 and so
on) are in section 12.

## 1. What this implements

ADR 0004, O1, points 1 to 3 (**Working**):

> 1. A pure crate, `ironafs-events`, with no dependencies, owns a numbered
>    event catalogue.
> 2. Each event has: an id, a name, a severity, an audience (admin,
>    operational, audit or debug), a subsystem, typed fields each with a data
>    class, a message template, its meaning, its remedy, and the producer
>    roles allowed to emit it.
> 3. Every component emits catalogue events. None writes free-form strings to
>    a log.

ADR 0004, O1, point 4 (**Working**), carried out in part (2, point 2):

> 4. `cargo xtask events` generates, from the one catalogue: [...]

ADR 0004, O2, point 6 (**Firm**):

> 6. The catalogue has its own version. Ids fit in 16 bits and event versions
>    in 8 bits, so they map directly to Windows events (Context 2). [...]

ADR 0004, O3, points 2 and 3 (**Working**), the parts this crate carries:

> 2. [...] It checks that the event id is allowed for that role. It
>    then applies redaction and rate limiting in one place.
> 3. Ingestion never blocks the sender. It is bounded. Records it cannot
>    accept are counted and reported as a loss marker.

ADR 0004, O4, points 2 and 5 (**Working**):

> 2. A direct write uses the same generated id, validation and redaction, and
>    carries a "direct" flag.
> 5. A direct write is never an audit event (O9).

ADR 0004, O5, points 1 to 4 (**Firm**):

> 1. Secret types (tokens, keys, session keys, Kerberos tickets) cannot
>    become event fields. The type system refuses them, and formatting a
>    secret type prints a redaction marker.
> 2. Credentials, keys and file contents are excluded from every log, debug
>    capture, call trace, state dump and support bundle, at every redaction
>    setting.
> 3. Text supplied by a server is never placed in an event payload as it
>    arrived.
> 4. No setting, policy or debug level relaxes points 1 to 3.

ADR 0004, O6, points 1 and 5 (**Working**), the parts this crate carries:

> 1. Routine logs omit paths, file names, native file ids and volume numbers.
>    User ids appear only in audit events.
> 5. Native identifiers may appear in: catalogued audit fields; scoped,
>    audited captures (O7); and routine logs under an administrator-locked
>    policy whose changes are audited.

ADR 0004, O7, points 1 to 3 (**Working**):

> 1. State changes are logged once: [...] Failures in between are counted and
>    summarised per window.
> 2. A per-subsystem limit is the last guard. Sampling is used only for debug
>    events.
> 3. A sink never blocks the daemon. A stalled OS logger costs counted drops.

ADR 0004, O8, points 1 and 4 (**Working**), the encoding part:

> 1. **Linux:** journald's native protocol over its Unix socket, written by
>    our own code `[DES-043]`, with a journal catalogue. RFC 5424 syslog is
>    the fallback.
> 4. **Syslog structured data.** Until a Private Enterprise Number is
>    assigned `[DES-045]`, the client puts the catalogue event id in the
>    RFC 5424 `MSGID` and emits no private structured data. [...]

ADR 0004, O9, points 1 and 2 (**Firm**):

> 1. Audit is a separate stream with its own ids. Only the daemon emits it,
>    and only for operations the daemon observes itself.
> 2. No other component can cause an audit event to be written by sending
>    one. The producer-role check (O3) refuses it, and a direct write (O4)
>    is never audit.

ADR 0004, O11, points 1 and 6 (**Working**), the value types:

> 1. Metrics are counters and fixed-bucket histograms with bounded labels,
>    never per user or per path. [...]
> 6. Health is healthy, degraded with reasons, or failed. [...]

ADR 0004, C4, point 1 (**Working**), the table row:

> | `ironafs-events` | the event catalogue, redaction, rate limiting, metrics and health (O1 to O8, O11) |

ADR 0005, point 1.7 (**Working**), the part that falls to the encoder:

> Fitting an event to the cap is the encoder's (`ironafs-events`): a
> truncated field is marked, the event's identity and required fields
> are kept, and the loss is counted (O7, point 3). If the identity and
> required fields alone, encoded, exceed the cap, the event is **not
> delivered**, and its caller treats it as a loss, never as sent:
> - a diagnostic event with its own catalogue id (O2, point 1), "event
>   dropped: over the size limit", carrying the dropped event's id [...]

and the two caps it names, `SYSLOG_FALLBACK_MAX` = 480 octets and
`JOURNALD_MAX_DATAGRAM` = 16 KiB.

Section 9 shows where each is kept and which test fails if it is broken.

## 2. Scope

1. **Covered:** the catalogue's types and its first entries (5.1); field
   values, data classes and the refusal of secret types (5.2); escaping
   of server-supplied text (5.3); redaction (5.4); building an event and
   the producer-role and direct-write checks (5.5); rate limiting and
   loss counters (5.6); metrics value types and health (5.7); the
   journald native-protocol encoding (5.8); the RFC 5424 encoding (5.9);
   fitting an encoding to its cap and marking truncation (5.10).
2. **Not covered yet, and where it goes:**
   - `cargo xtask events` and its generated outputs (O1, point 4; O2,
     point 7): the catalogue is written by hand in
     `crates/ironafs-events/src/catalogue/entries.rs` until the
     generator exists (issue #94, part (a)). The generator then writes
     that file and checks it in (coding standards rule 20);
   - the sinks' transport (the datagram writes, the journal socket's
     absence, stalls): `ironafs-os` (ADR 0005, point 1.7; issue #94,
     part (d));
   - the audit stream's events beyond the gap event, the checkpoint and
     intents, and splitting a gap event whose ranges do not fit (ADR
     0005, point 1.7): the audit specification (issue #94 names the
     audit issue that owns it);
   - opaque references: computing the HMAC is the daemon's (O6, point
     2); this crate carries the 32-byte result only;
   - the Prometheus text file and its publication (O11, points 2 and 3),
     and systemd notification (O11, point 6): later parts of issue #94;
   - debug sampling (O7, point 2), run-time levels (O7, point 4) and
     captures (O7, points 5 and 6): not in this part.

## 3. The design in short

1. The catalogue is a static table of event definitions. Each names its
   stream (diagnostic or audit), id, version, name, severity, audience,
   subsystem, fields, template, meaning, remedy and producer roles.
2. A field value can be built only from a closed list of types, through a
   sealed trait. Secret types are not on the list, and no type outside
   this crate can be added to it, so the compiler refuses a secret as a
   field.
3. Server-supplied text is held only as `ServerText`, which keeps at most
   256 bytes and renders only as escaped printable ASCII.
4. Each field has a data class. Redaction looks only at the class and the
   policy: identifiers are omitted unless an administrator-locked policy
   reveals them. No policy changes how server text is escaped.
5. The rate limiter keeps one bucket per subsystem and takes the instant
   as an argument. Every event it refuses is counted. Audit is never
   rate limited.
6. Each encoder writes a complete message, checks it against its cap,
   and fits it by truncating server text and then dropping optional
   fields, marking the truncation. If the event's identity and required
   fields alone do not fit, it returns `OverSizeLimit`, and the caller
   sends the catalogue's "over the size limit" event instead.

## 4. Terms

- **Event definition:** one catalogue entry (5.1).
- **Stream:** diagnostic or audit. Each stream has its own id space.
- **Producer role:** the kind of component that emits an event: the
  daemon, an adapter (the FUSE or FSKit layer), the SMB front end, a
  Windows helper, the `afs` CLI, or the login service.
- **Data class:** what a field's value may reveal, and so how redaction
  treats it (5.4).
- **Direct write:** an event a component writes to the OS log itself
  because the daemon could not take it (ADR 0004, O4).
- **Cap:** the largest complete encoded message a sink accepts, after
  escaping and framing.
- **Identity of an event:** its stream, id, version and name.

## 5. Specification

### 5.1 The catalogue

1. Each event definition has: a stream; an id, unsigned 16-bit, unique
   within its stream; a version, unsigned 8-bit, starting at 1; a name of
   lower-case ASCII letters, digits, `.` and `_`, at most 32 bytes,
   unique in the catalogue; a severity (5.1, point 2); an audience (admin,
   operational, audit or debug); a subsystem (5.1, point 3); at most 8
   fields; a message template; a meaning; a remedy; and a non-empty set of
   producer roles. `[ADR 0004 O1.2, O2.6: Working, Firm]`
2. Severities are the eight of RFC 5424, section 6.2.1, with the same
   numbers: emergency 0 to debug 7 `[R1 §6.2.1: CONFIRMED]`. journald's
   `PRIORITY` uses the same numbers `[J2: CONFIRMED]`.
3. Subsystems are a closed list: `events`, `daemon`, `rx`, `server`,
   `volume`, `cache`, `config`, `adapter`. A closed list bounds the rate
   limiter's state (5.6). `[OWN]`
4. A field has a name (lower-case ASCII letters, digits and `_`, at most
   24 bytes, unique within its event), a kind (5.2), a data class (5.4),
   and whether it is required. `[OWN]`, so that the journald field name
   of 5.8, point 4, stays within 5.8, point 3.
5. The template is text with `{name}` placeholders. Every placeholder
   names a field of the event, and the template holds no other `{` or
   `}`. `[OWN]`
6. An event is in the audit stream if and only if its audience is audit.
   Every audit event's producer roles are exactly the daemon. `[ADR 0004
   O9.1, O9.2: Firm]`
7. The catalogue has a version, `CATALOGUE_VERSION`, unsigned 16-bit.
   `[ADR 0004 O2.6: Firm]`
8. The first entries:

   | Stream | Id | Name | Severity | Audience | Subsystem | Fields (kind, class, required) | Producers |
   |---|---|---|---|---|---|---|---|
   | diagnostic | 1 | `events.lost` | warning | operational | events | `reason` (static, public, yes), `count` (unsigned, public, yes) | all |
   | diagnostic | 2 | `events.suppressed` | notice | operational | events | `subsystem` (static, public, yes), `count` (unsigned, public, yes) | all |
   | diagnostic | 3 | `events.over_size` | warning | operational | events | `stream` (static, public, yes), `event` (unsigned, public, yes) | all |
   | diagnostic | 100 | `server.unreachable` | error | admin | server | `server` (reference, reference, yes) | daemon |
   | diagnostic | 101 | `server.reachable` | notice | admin | server | `server` (reference, reference, yes) | daemon |
   | diagnostic | 200 | `volume.offline` | warning | operational | volume | `volume` (unsigned, identifier, yes), `message` (server text, server text, no) | daemon |
   | audit | 1 | `audit.gap` | warning | audit | daemon | `first` (unsigned, public, yes), `last` (unsigned, public, yes), `kind` (static, public, yes) | daemon |

   `events.over_size` is the "event dropped: over the size limit" event
   of ADR 0005, point 1.7. Its `stream` and `event` fields name the
   dropped event. `audit.gap`'s `kind` is one of `known-loss`,
   `uncertain` and `possible-loss` (ADR 0004, O9, point 4); its other
   audit fields (O9, point 3) are the audit specification's. No entry is
   published yet, so none is frozen (O2, point 2). `[OWN]`

### 5.2 Field values and secret types

1. A field's kind is one of: unsigned (from `u8`, `u16`, `u32`, `u64`),
   signed (from `i32`, `i64`), boolean, static text (`&'static str`),
   server text (`ServerText`, 5.3), and reference (`Reference`, 32
   bytes). `[OWN]`
2. A value is built only through the trait `FieldSource`, which is
   sealed: it is implemented for exactly the types of point 1, and no
   other crate can implement it. So no type outside that list can become
   a field: a token, a key, a session key or a Kerberos ticket is
   refused by the compiler. `[ADR 0004 O5.1: Firm]`
3. Static text is text written in IronAFS's own source. Text that
   arrives at run time from outside the process reaches an event only as
   server text. `[ADR 0004 O5.3: Firm]`
4. A value's kind must equal its field's kind; a value for a field the
   event does not have, a second value for one field, and a missing
   required field are each refused (7). `[OWN]`
5. `REDACTION_MARKER` is the text `[redacted]`. A secret type's `Debug`
   and `Display` print only the marker, written through
   `write_redacted`. This crate cannot force that on a type it does not
   own: each secret type's owner does it, and its tests check it.
   `[ADR 0004 O5.1: Firm]`
6. The limit: the type system refuses secret *types*. Code that copies a
   secret's bytes into an allowed type (an integer, a static string)
   defeats it. Secret types offer no accessor that does that, and review
   checks it. `[OWN]`

### 5.3 Server-supplied text

1. `ServerText::from_wire` takes the bytes as they arrived and keeps at
   most `SERVER_TEXT_MAX` = 256 bytes of them. Longer input is cut there
   and the value is marked truncated. `[OWN]`, a bound on memory and on
   the encoded size.
2. Server text is rendered only through escaping. Each byte:
   - `0x20` to `0x7E`, except `\` and `"`, is written as itself;
   - `\` is written as `\\`, and `"` as `\"`;
   - every other byte (controls, `0x7F`, and every byte of a non-ASCII
     character) is written as `\x` and two lower-case hex digits.

   So a rendering holds only printable ASCII, never a newline, never `=`
   at the start, and is at most four times the kept bytes.
   `[ADR 0004 O5.3: Firm]`, `[OWN]` for the escape alphabet.
3. A truncated rendering never splits an escape: it holds the escapes of
   a prefix of the kept bytes, and is followed by `TRUNCATION_MARKER`,
   `[truncated]`. `[OWN]`
4. `ServerText`'s `Debug` prints the escaped form, never the bytes.
   `[ADR 0004 O5.3: Firm]`

### 5.4 Redaction

1. Data classes: **public** (counts, states, static text), **identifier**
   (paths, file names, native file ids, volume numbers), **server text**,
   **reference** (an opaque reference, O6, point 2). No class holds a
   secret. `[ADR 0004 O5.1, O6.1: Firm, Working]`
2. Allowed pairs of kind and class: unsigned and signed with public or
   identifier; boolean and static text with public; server text with
   server text or identifier; reference with reference. The catalogue
   test refuses any other pair. `[OWN]`
3. The policy is `Policy::Routine` (the default) or
   `Policy::RevealIdentifiers`, which the configuration may set only
   under an administrator-locked policy whose changes are audited (O6,
   point 5; enforced by the configuration, not here).
   `[ADR 0004 O6.5: Working]`
4. Under `Routine`, a field of class identifier is omitted: no encoded
   field is written for it, and the message shows `[omitted]` in its
   place. Under `RevealIdentifiers` it is written. In the audit stream an
   identifier is written under either policy (O6, point 5: "catalogued
   audit fields"). `[ADR 0004 O6.1, O6.5: Working]`
5. Server text is escaped (5.3) under every policy. No policy reveals a
   secret, because no field can hold one (5.2). `[ADR 0004 O5.4: Firm]`
6. An optional field with no value is written nowhere, and the message
   shows `-` in its place. `[OWN]`

### 5.5 Building an event

1. `Event::builder(definition, role)` refuses a role the definition does
   not list (`RoleNotAllowed`). `[ADR 0004 O3.2, O9.2: Working, Firm]`
2. `EventBuilder::direct()` marks a direct write. It is refused for an
   audit definition (`DirectAudit`). `[ADR 0004 O4.5: Working]`
3. `EventBuilder::with(name, value)` sets one field (5.2, point 4).
   `EventBuilder::build()` refuses a missing required field.
4. An event holds at most 8 values; the builder never allocates more.
   `[OWN]`

### 5.6 Rate limiting and loss counters

1. `RateLimiter` keeps one bucket per subsystem, with a rate in events
   per second and a burst, both at least 1 and at most
   `LIMIT_MAX` = 1,000,000. The default is 10 per second with a burst of
   50. `[ADR 0004 O7.2: Working]`, `[OWN]` for the numbers.
2. `admit(definition, now)` takes `now` as nanoseconds on the boot clock,
   read by the caller through the `Clock` seam (`ironafs-time`); this
   crate has no dependencies (O1, point 1), so it takes the reading, not
   the seam's type. `[ADR 0004 O1.1: Working]`
3. The bucket is a virtual-scheduling rate check: with interval
   `T = 1e9 / rate` nanoseconds (integer division, at least 1) and
   tolerance `tau = T * (burst - 1)`, an event at `now` is admitted when
   `now + tau >= tat`, and then `tat = max(tat, now) + T`. A sum that
   would overflow 64 bits refuses the event (and counts it). `[OWN]`, a
   standard integer-only rate check.
4. An audit definition is always admitted and never touches a bucket:
   audit has its own capacity (O10, point 1). `[ADR 0004 O9.4: Firm]`
5. Every refusal increments that subsystem's suppressed count.
   `take_suppressed` returns and clears the counts as `events.suppressed`
   events, one per subsystem with a non-zero count. A refused event is
   never lost without a count. `[ADR 0004 O3.3, O7.1: Working]`
6. `LossCounters` counts, per reason, events not delivered: over the
   size limit, queue full, sink stalled, sink unavailable, and truncated
   (delivered but cut). Rate-limit refusals are counted by the limiter
   (point 5), not here. Callers record the reasons this crate cannot see
   (queue full and the sink reasons) and the encoders' results.
   `take_lost` returns and clears them as `events.lost` events.
   `[ADR 0004 O3.3, O7.3: Working]`
7. Counters saturate at `u64::MAX`: a count is a clamped value (coding
   standards rule 2), and a counter that stops at its maximum still says
   "at least this many". `[OWN]`

### 5.7 Metrics and health

1. `Counter` is a saturating 64-bit count. `Histogram<N>` has `N` fixed,
   strictly increasing upper bounds, `N` from 1 to 32, a count per
   bucket, an overflow count, a total count and a saturating sum.
   `[ADR 0004 O11.1: Working]`
2. `Health` is healthy, degraded or failed, with a set of reasons. Each
   reason has its level; the health's level is the worst of its reasons'
   levels, and healthy when it has none. The reasons in this part are:
   audit sink stalled (degraded, O10, point 6), audit admission
   unavailable (degraded, O10, point 10), log sink stalled (degraded),
   events lost (degraded), state unusable (failed). `[ADR 0004 O11.6,
   O10.6, O10.10: Working]`

### 5.8 journald native-protocol encoding

1. A message is a sequence of fields. A field whose value holds no
   newline is written as the name, `=`, the value and a newline. A field
   whose value holds a newline is written as the name, a newline, the
   value's length as a little-endian unsigned 64-bit integer, the value,
   and a newline `[J1: CONFIRMED]`. Escaping (5.3) means no value this
   encoder writes holds a newline; the second form is still implemented
   and tested.
2. Field names may not hold `=`, control or non-ASCII characters, or be
   empty `[J1: CONFIRMED]`. Names starting with `_` are trusted fields the
   journal sets itself, and are ignored if a client sends them
   `[J1: CONFIRMED]`, `[J2: CONFIRMED]`.
3. This encoder writes only names of upper-case ASCII letters, digits
   and `_`, starting with a letter, at most 64 bytes: a stricter rule
   than point 2, so a name is never refused or treated as trusted.
   `[OWN]`
4. Fields, in this order:
   - `MESSAGE`: the template with each placeholder replaced (5.4);
   - `MESSAGE_ID`: 32 lower-case hex digits `[J2: CONFIRMED]`: the
     constant prefix `771546f4bcf0d0b7054f7530`, then the stream (`0001`
     diagnostic, `0002` audit), then the id, each 4 hex digits. One
     identifier per stream and id, the same for every version, so the
     journal catalogue keys on the event's meaning (O2, point 1). `[OWN]`
   - `PRIORITY`: the severity's number as a decimal string
     `[J2: CONFIRMED]`;
   - `SYSLOG_IDENTIFIER`: the application name (5.9, point 3);
   - `IRONAFS_EVENT`, `IRONAFS_EVENT_VERSION`, `IRONAFS_EVENT_NAME`,
     `IRONAFS_STREAM`, `IRONAFS_SUBSYSTEM`, `IRONAFS_AUDIENCE`;
   - `IRONAFS_DIRECT=1` for a direct write; `IRONAFS_TRUNCATED=1` if any
     value was cut or any optional field dropped (5.10);
   - each written field, in catalogue order, as `IRONAFS_F_` and the
     field's name in upper case.
5. Values: unsigned and signed as decimal; boolean as `true` or `false`;
   static and server text escaped (5.3); a reference as 64 lower-case hex
   digits. `[OWN]`
6. The cap is `JOURNALD_MAX_DATAGRAM` = 16,384 bytes over the whole
   message (ADR 0005, point 1.7). An encoder may be given a lower cap,
   never a higher one. `[ADR 0005 1.7: Working]`

### 5.9 RFC 5424 encoding

1. The message is `<PRI>1 TIMESTAMP HOSTNAME APP-NAME PROCID MSGID SD
   MSG`, fields separated by one space `[R1 §6: CONFIRMED]`.
2. `PRI` is the facility times 8 plus the severity `[R1 §6.2.1:
   CONFIRMED]`. The facility is 3 (system daemons) for the diagnostic
   stream and 13 (log audit) for the audit stream `[R1 §6.2.1:
   CONFIRMED]` for the numbers, `[OWN]` for the choice.
3. `TIMESTAMP` is the nil value `-` `[R1 §6: CONFIRMED]`: the local
   receiver stamps the time it receives. `HOSTNAME` is the configured
   host name if it is 1 to 64 bytes of printable US-ASCII (`0x21` to
   `0x7E`), else `-`. `APP-NAME` is 1 to 48 bytes of printable US-ASCII
   `[R1 §6: CONFIRMED]`, checked when the origin is built. `PROCID` is
   the process id in decimal, or `-`. The 64-byte host name limit is
   `[OWN]` (RFC 5424 allows 255), so the size bound of point 7 holds.
4. `MSGID` is `ironafs.diag.<id>` or `ironafs.audit.<id>`, the id in
   decimal, at most 19 bytes, within RFC 5424's 32
   `[ADR 0004 O8.4: Working]`, `[OWN]` for the form.
5. `SD` is `-`: no structured data until a Private Enterprise Number is
   assigned `[ADR 0004 O8.4: Working]`.
6. `MSG` is printable ASCII only, so it is RFC 5424's `MSG-ANY` with no
   byte-order mark `[R1 §6.4: CONFIRMED]`: the message (5.8, point 4),
   then ` event=<name> v=<version> subsystem=<subsystem>
   audience=<audience>`, then ` direct=1` and ` truncated=1` where they
   apply, then for each written field ` f_<name>=<value>`, text values
   in double quotes.
7. The cap is `SYSLOG_FALLBACK_MAX` = 480 octets over the whole message:
   RFC 5424, section 6.1, requires every receiver to accept that much
   `[R1 §6.1: CONFIRMED]`, `[ADR 0005 1.7: Working]`. With the largest
   header this section allows, `events.over_size` and an `audit.gap`
   with one range, at the largest values their fields can take, encode
   within it (10.5).

### 5.10 Fitting to the cap and marking truncation

1. Each encoder first writes the whole message. If it fits the cap, it
   is the result.
2. Otherwise the encoder finds the largest rendering length `L` such
   that, with every server text value rendered at most `L` bytes plus the
   marker (5.3, point 3) and `truncated` set, the message fits. It
   searches by bisection over `0..` the longest rendering (at most 1,024
   bytes, 5.3, point 2).
3. If even `L = 0` does not fit, it drops every optional field, marks
   `truncated`, tries the server text whole, and then searches again as
   point 2 says. With point 1, that is at most 24 encodings of one event.
4. If that does not fit either, it returns `OverSizeLimit` with the
   event's stream and id, and nothing else. The identity and required
   fields are never cut beyond point 2, so an encoding that is returned
   always holds them. `[ADR 0005 1.7: Working]`
5. The result says whether it was truncated; the caller records it in
   `LossCounters` (5.6, point 6). For `OverSizeLimit`, the caller builds
   `events.over_size` with `Event::over_size(stream, id, role)`, sends
   that, and records the loss. `[ADR 0005 1.7: Working]`

## 6. Arithmetic

1. Rate limiter: `now + tau`, `max(tat, now) + T` and `T * (burst - 1)`
   are checked; an overflow refuses the event and counts it (5.6, point
   3). `T` is at least 1.
2. Counters and the histogram's sum saturate (5.6, point 7).
3. Encoded lengths: every message is bounded by its inputs (at most 8
   fields, 256 kept bytes of server text, static strings), and the cap
   check compares the output length with the cap; no length from the
   outside is used to size anything.
4. The journald length prefix is the value's length as `u64`, from a
   `usize` by a checked conversion.

## 7. Errors and failure

1. `EventError`: `RoleNotAllowed`, `DirectAudit`, `UnknownField`,
   `KindMismatch`, `DuplicateField`, `MissingField`, each naming the event
   and, where it applies, the field. No error carries a field's value.
2. `EncodeError::OverSizeLimit { stream, id }`.
3. `OriginError`: an application name that is empty, longer than 48
   bytes, or not printable US-ASCII.
4. `LimitError`: a rate or burst out of range. `HistogramError`: bounds
   not strictly increasing.
5. Nothing in this crate panics; every failure is one of these values.

## 8. Security

1. **A component wants a token in a log.** It cannot make the token a
   field (5.2, point 2). A secret type's own formatting prints the marker
   (5.2, point 5).
2. **A server sends text with a newline and `MESSAGE=` or `PRIORITY=`,**
   hoping to add a journal field, or a control sequence for a terminal.
   Escaping (5.3) writes no newline and no control byte, so the text stays
   inside its own field.
3. **A server sends very long text** to push the event's identity out of
   the cap. Text is kept to 256 bytes, and fitting cuts server text first
   (5.10).
4. **A component other than the daemon sends an audit event.** The role
   check refuses it, and a direct write is never audit (5.5).
5. **A flood of events** from one subsystem is cut by its bucket, and
   every refusal is counted (5.6).
6. **A trusted journald field:** no name this encoder writes starts with
   `_` (5.8, point 3).

## 9. How the invariants are kept

| Invariant | Kept by | Test that fails if broken |
|---|---|---|
| O1.2: every field has a data class | `FieldSpec::class` is not optional; 5.4, point 2 | `catalogue::tests::every_field_has_an_allowed_class` |
| O2.6: ids 16-bit, versions 8-bit, catalogue version | `EventDef` field types; 5.1 | `catalogue::tests::ids_are_unique_per_stream` |
| O3.2, O9.2: role check, audit only from the daemon | 5.5, point 1; 5.1, point 6 | `event::tests::builder_refuses_a_role_the_definition_does_not_list`, `catalogue::tests::audit_events_come_only_from_the_daemon` |
| O3.3, O7.3: refused records counted | 5.6, points 5 and 6 | `limit::tests::suppressed_events_are_counted`, `limit::tests::loss_counters_record_and_take`, `it::limit::every_refusal_is_counted` |
| O4.5: a direct write is never audit | 5.5, point 2 | `event::tests::direct_audit_is_refused` |
| O5.1: no secret type as a field; marker | 5.2 | the two `compile_fail` doc-tests on `FieldSource`; `it::redaction::formatting_a_secret_prints_the_marker` |
| O5.3: server text never as it arrived | 5.3 | `field::tests::escape_*`, `it::server_text::server_text_never_reaches_the_output_raw` and `it::server_text::a_newline_in_server_text_cannot_add_a_journal_field` (both shown failing on a planted pass-through) |
| O5.4, O6.1: no policy relaxes escaping; identifiers omitted | 5.4 | `redaction::tests::*`, `it::redaction::routine_logs_omit_identifiers_from_both_encodings`, `it::redaction::no_policy_changes_how_server_text_is_written` |
| O7.2: per-subsystem limit | 5.6 | `limit::tests::subsystems_have_separate_buckets`, `it::limit::a_flood_admits_the_burst_and_counts_the_rest` |
| ADR 0005 1.7: caps, marking, over-size | 5.10 | `it::fit::*`, `it::syslog::the_largest_over_size_event_fits_the_fallback_cap`, `it::syslog::the_largest_one_range_gap_fits_the_fallback_cap`, the cap properties in `it::journald` and `it::syslog` |

## 10. Tests

Unit tests are in `crates/ironafs-events/src/<module>/tests.rs`, and use
only `core` and `alloc`. Integration tests are in
`crates/ironafs-events/tests/it/`, and property tests use
`ironafs-test-support`.

### 10.1 Golden vectors

Written by hand from sections 5.8 and 5.9, in
`crates/ironafs-events/tests/it/journald.rs` and `tests/it/syslog.rs`:
the complete bytes of `volume.offline` with a server message holding a
newline and a non-ASCII byte, under each policy, in both encodings; of
`server.reachable` as a direct write (journald); and of `events.lost` as
a direct write with no process id and an over-long host name (syslog).
One vector in `src/journald/tests.rs` exercises the length-prefixed
journald form.

### 10.2 Round-trip properties

There is no decoder in the product. The tests carry a small journald
reader (`tests/it/support.rs`) and check, over generated events: the
encoding parses as 5.8, point 1, describes; every name meets 5.8, point
3; every value is printable ASCII; the output is within the cap; and a
truncated output is marked.

### 10.3 Reject corpus

The refusals of section 7, each by its variant.

### 10.4 Fuzz target

None yet: the crate decodes nothing. `ServerText::from_wire` takes
untrusted bytes, and the property test over arbitrary bytes covers it; a
fuzz target `ironafs_events_server_text` is added with the fuzz crate
(issue #94).

### 10.5 Acceptance cases

1. The largest `events.over_size` and the largest one-range `audit.gap`
   with the largest syslog header (64-byte host name, 48-byte
   application name, 10-digit process id) encode within 480 octets:
   310 octets (a 150-octet header, a 59-octet message, 101 octets of
   event fields) and 356 octets (152, 72 and 132), counted by hand from
   5.9 and asserted exactly.
2. A flood of 10,000 events from one subsystem at one instant admits the
   burst, and the suppressed count equals the rest exactly.

### 10.6 Gaps

- The generator and its `--self-test` (issue #94, part (a)).
- Read-back through journald and a syslog receiver, and the stalled
  sink: the transport (issue #94, part (d)).

## 11. Revisit if

- O1: an OS sink cannot carry a catalogue field type, or generated output
  for one sink cannot be kept in step by the build.
- O3: the IPC path is shown to delay or lose events that a direct write
  would have kept.
- O4: testing shows the direct path blocks a request or drops events a
  bounded queue would keep.
- O6: support cases show the default cannot diagnose common failures
  without a policy change, or a privacy review finds the references
  re-identify users.
- O7: a real outage shows the summaries hide the first cause.
- O8: an OS changes its logging interface, or a supported release cannot
  retrieve an entry written this way.
- O11: a site shows a need the metrics file cannot meet.
- ADR 0005, point 1.7: a larger syslog limit is qualified on the selected
  receiver, or the journald datagram limit is measured below 16 KiB.
- This specification: journald refuses a name 5.8, point 3, allows, or a
  receiver alters `MSG-ANY` text this encoder writes.

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| J1 | DOCUMENTATION | systemd project, "Native Journal Protocol", <https://systemd.io/JOURNAL_NATIVE_PROTOCOL/>, read 2026-11-21 (claim record `[DES-043]`) | 5.8 |
| J2 | DOCUMENTATION | systemd project, systemd.journal-fields(7), <https://www.freedesktop.org/software/systemd/man/latest/systemd.journal-fields.html>, read 2026-11-21 (claim record `[ADR-037]`) | 5.1, 5.8 |
| R1 | SPECIFICATION | RFC 5424, "The Syslog Protocol", R. Gerhards, March 2009, <https://www.rfc-editor.org/rfc/rfc5424.txt>, sections 6, 6.1, 6.2.1, 6.4 | 5.1, 5.9 |
| D1 | DECISION | ADR 0004, parts 1 and 3 (C4, O1 to O11); ADR 0005, point 1.7 | throughout |
| O1 | OWN | Choices marked `[OWN]`, each with its reason | throughout |

J1, J2 and R1 need rows in the register of editions (`docs/specs/README.md`)
before this specification leaves draft.

### 12.1 Observed behaviour

None yet. Read-back through journald and a syslog receiver belongs to the
transport (2, point 2).

## 13. Open questions

1. Where the sink caps live. ADR 0005, point 1.7, puts the cap check in
   `ironafs-os`, which may not depend on `ironafs-events` (check-deps).
   This crate defines `JOURNALD_MAX_DATAGRAM` and `SYSLOG_FALLBACK_MAX`
   for fitting; the transport needs the same values. Issue #94.
2. Whether `/dev/log` accepts RFC 5424 input (ADR 0005, point 1.7:
   UNVERIFIED). Issue #94, part (d).
3. The canonical secret wrapper and its zeroing belong to the crates that
   hold secrets; they adopt `write_redacted` (5.2, point 5). Issue #94.

## 14. Review

No review record yet.
