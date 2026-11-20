<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0004, part 3: Operations, security and lifecycle

Status: proposed (2026-11-20). Part of ADR 0004 ([overview](0004-architecture.md)); raised in issue #20.

This part decides how IronAFS is observed, managed, secured, upgraded,
tested and phased. The core is in part 1, the OS layers and the
supported-platform matrix in part 2, protocol negotiation and the rules for
long-lived formats in part 4. This part refers to them and does not restate
them. Claim ids such as `[DES-043]` point into the
[claim record](../reviews/2026-11-20-adr-0004-claims.md). A1 to A12 are the
named assumptions in the [overview](0004-architecture.md).

## Context

1. **Fleets run the client without a developer.** IronAFS is meant to be
   installed and watched by central IT. That needs native logs with stable
   ids, an audit stream, health, metrics, diagnostics and central
   configuration on every OS. These are part of the architecture. They are
   not added per OS afterwards.
2. **Each OS has its own log, and each can be written without a
   third-party library.**
   - journald accepts native-protocol entries as datagrams on a Unix
     socket, so a client needs no library `[DES-043]`.
   - macOS unified logging treats dynamic strings as private by default
     and supports `%{public}` and `%{private}` modifiers `[DES-020]`. A
     build script can call `xcrun clang` without a fixed toolchain path
     `[DES-021]`.
   - Syslog's structured form is
     [RFC 5424](https://www.rfc-editor.org/rfc/rfc5424). A private
     structured-data id needs a Private Enterprise Number, which IANA
     assigns free of charge and lists publicly with a contact `[DES-045]`.
   - A Windows service can run as a virtual account with its own SID
     `[DES-074]`. An Event Log channel with custom isolation takes an SDDL
     access descriptor that controls write access to the channel
     ([ChannelType](https://learn.microsoft.com/en-us/windows/win32/wes/eventmanifestschema-channeltype-complextype)).
   - A manifest-based Windows event has a 16-bit id and an 8-bit version,
     and its provider, id and version identify its fields
     ([EVENT_DESCRIPTOR](https://learn.microsoft.com/en-us/windows/win32/api/evntprov/ns-evntprov-event_descriptor)).
   - journald adds trusted fields, prefixed with an underscore, which
     client code cannot alter
     ([systemd.journal-fields](https://www.freedesktop.org/software/systemd/man/latest/systemd.journal-fields.html)).
3. **Metrics can be published without a listener.** node_exporter's
   textfile collector reads `*.prom` files. The documented safe pattern is
   to write a temporary name and move it into place `[DES-044]`.
4. **Service managers report health.** A systemd service with a watchdog
   interval must send keep-alive notifications, and one that stops is
   treated as failed `[DES-042]`. `NoNewPrivileges=` stops a service
   gaining privileges through `execve` `[DES-041]`.
5. **Core dumps on Linux need more than a limit.**
   - A zero core limit does not suppress a dump the system pipes to a
     program `[DES-039]`.
   - A process's dumpable attribute is reset when its effective uid or gid
     changes or it executes a set-id program `[DES-038]`.
   - `MADV_DONTDUMP` excludes pages from a core dump `[DES-040]`.
   - On macOS, `RLIMIT_CORE` is the largest core file that may be created
     (getrlimit(2), macOS 27.2).
6. **rxkad is the security class existing cells use, and it is old.**
   - Existing cells' clients use rxkad with Kerberos 5 tokens `[DRV-089]`.
   - The community Rx specification does not specify security classes
     `[DRV-084]`. No public specification of rxkad is cited in the claim
     record.
   - A published security advisory describes DES-only service keys as
     cheap to brute-force, and describes the fixes rxkad-k5 and rxkad-kdf
     `[DES-049]`.
   - The rxgk base draft describes rxkad as limited to very weak
     cryptography, approximately single-DES
     ([draft-wilkinson-afs3-rxgk-11, section 1](https://datatracker.ietf.org/doc/html/draft-wilkinson-afs3-rxgk-11)).
   - The claim record holds no public source that states, for each rxkad
     level, whether a reply's payload is integrity-protected. IronAFS
     therefore states its own security position in O23.4: it relies on no
     rxkad level for the integrity of a reply's payload.
7. **OpenAFS offers no client-to-fileserver rxgk yet.**
   - The rxgk base draft `[DRV-085]` and "Integrating rxgk with AFS"
     `[DRV-086]` are expired Internet-Drafts. The base draft has
     experimental status in the AFS-3 standardisation group
     `[DRV-087, DES-M05]`.
   - OpenAFS 1.9.0 introduced rxgk only for server-to-server traffic and
     local keys `[DES-048]`. Client tokens are still in progress there
     `[DRV-M25]`.
8. **Server lines.** OpenAFS 1.6 is end-of-life, with no security patches
   after 31 December 2025 `[DES-047]`.
9. **macOS distribution outside the App Store.** Software distributed with
   Developer ID must be notarised, which needs code signing, a Developer ID
   certificate and the hardened runtime `[MAC-063]`. A launch daemon
   registered through the service-management interface does not start
   until an administrator approves it `[MAC-M27]`. Device management can
   pre-approve the launch daemon and install the app; whether it can enable
   an FSKit extension is open (`[MAC-M28]`, corrected form).
10. **Test tooling.**
    - nextest runs each test in its own process `[LAY-034]` and does not
      run doc-tests `[LAY-036]`.
    - cargo-fuzz works only on Unix-like x86-64 and AArch64 systems and
      needs a nightly compiler `[LAY-041]`.
    - `cargo xtask` needs only cargo and rustc `[LAY-045]`.
    - GitHub artifact attestations on their own give SLSA Build Level 2
      `[DES-046]`.
11. **Project rules that bind this part.**
    - [ADR 0001](0001-licence-and-dont-copy.md): the don't-copy rule.
      Tests and fixtures come from published specifications and observed
      behaviour.
    - [Coding standards](../coding-standards.md) rule 21: our own code
      first, and a crate only for mainstream cryptography or the operating
      system's own interfaces.
    - `unsafe` is forbidden outside a named bindings crate that has its own
      ADR.

## Decision

Decisions are numbered O1 to O51. Each is marked **Firm** or **Working**
as the overview defines them. A firm decision changes only by a superseding
ADR. A working decision names what would reopen it.

### Events and logs

### O1. One event catalogue for every component

**Working.** Revisit if: an OS sink cannot carry a catalogue field type, or
generated output for one sink cannot be kept in step by the build.

1. A pure crate, `ironafs-events`, with no dependencies, owns a numbered
   event catalogue.
2. Each event has: an id, a name, a severity, an audience (admin,
   operational, audit or debug), a subsystem, typed fields each with a data
   class, a message template, its meaning, its remedy, and the producer
   roles allowed to emit it.
3. Every component emits catalogue events. None writes free-form strings to
   a log.
4. `cargo xtask events` generates, from the one catalogue: the Windows
   instrumentation manifest, the systemd journal catalogue, the event
   settings in the Windows policy templates, a Swift table for the macOS
   components, and the public `docs/events.md`. CI fails if any generated
   output differs from the catalogue.

Why: collectors and support staff key on ids and fields, which only a
single source keeps consistent across three operating systems. Generation
needs only cargo and rustc `[LAY-045]`.

### O2. A published event keeps its id and meaning for good

**Firm.**

1. An event id is never reused, and an event's meaning never changes. A
   different meaning gets a new id.
2. Admin, operational and audit events are stable. A stable entry is frozen
   when it first ships in a public release candidate or public release.
   Frozen means: meaning, level, channel, field data classes and the
   mapping to each OS sink.
3. Fields are only appended, and only in a new version of the event.
   Existing field names, types and order stay.
4. Debug events carry no compatibility promise to collectors. Their ids are
   still reserved for good once published.
5. A retired event is marked as no longer logged. Its identity stays
   reserved, and its schema stays published so archived logs still decode.
6. The catalogue has its own version. Ids fit in 16 bits and event versions
   in 8 bits, so they map directly to Windows events (Context 2). When an
   event's version space is used up, it gets a new id.
7. `cargo xtask events` checks every change against a cumulative snapshot
   of all public candidates and releases, retired entries included.
8. Change classes ([docs/workflow.md](../workflow.md)): a change to these
   rules, a retirement or a relaxed data class is class R. An added stable
   event or version is class P. A debug change within the id and decoder
   limits is class I.

Why: an event record is a long-lived format. Collectors, alert rules and
archived logs outlive the release that wrote them, and Windows identifies a
manifest-based event's fields by provider, id and version (Context 2).

### O3. Components report through the daemon, which stamps the source

**Working.** Revisit if: the IPC path is shown to delay or lose events that
a direct write would have kept.

1. The FSKit extension, the SMB front end, the Windows helper processes and
   the CLI send event records over the IPC connection they already have
   (C44).
2. At ingress the daemon stamps the authenticated source: role, process and
   OS identity. It checks that the event id is allowed for that role. It
   then applies redaction and rate limiting in one place.
3. Ingestion never blocks the sender. It is bounded. Records it cannot
   accept are counted and reported as a loss marker.
4. Every source has an id and an instance id. Every event carries its
   source's sequence number, so a collector sees gaps and restarts.
5. Each OS request gets a request id at the layer. The core logs it with
   every Rx call the request causes (server, connection, call number).

Why: one ingress gives one place for redaction and limits, and a source
identity that the sender did not write itself.

### O4. A direct write when the daemon cannot take an event

**Working.** Revisit if: testing shows the direct path blocks a request or
drops events a bounded queue would keep.

1. When the daemon cannot take an event within a bound (not running,
   wedged, overloaded, refusing the handshake), a component writes it to
   the OS log directly, under its own identity.
2. A direct write uses the same generated id, validation and redaction, and
   carries a "direct" flag.
3. A direct write happens on a logging thread with its own time bound,
   never on a request's path. A write that misses the bound is dropped and
   counted.
4. A direct writer that cannot reach the daemon's redaction policy or
   reference service (O6) omits the affected field.
5. A direct write is never an audit event (O9).

Why: a daemon that is down is exactly when an operator needs the
component's own account of what happened.

### O5. No secret in a log, a capture or a support bundle

**Firm.**

1. Secret types (tokens, keys, session keys, Kerberos tickets) cannot
   become event fields. The type system refuses them, and formatting a
   secret type prints a redaction marker.
2. Credentials, keys and file contents are excluded from every log, debug
   capture, call trace, state dump and support bundle, at every redaction
   setting.
3. Text supplied by a server is never placed in an event payload as it
   arrived.
4. No setting, policy or debug level relaxes points 1 to 3.

Why: logs are forwarded to collectors and shared with support. A secret
that reaches one has left the machine's protection for good.

### O6. Default redaction and opaque references

**Working.** Revisit if: support cases show the default cannot diagnose
common failures without a policy change, or a privacy review finds the
references re-identify users.

1. Routine logs omit paths, file names, native file ids and volume numbers.
   User ids appear only in audit events.
2. Where records must be correlated, the daemon computes an opaque
   reference: HMAC-SHA-256 under a protected installation key, over a
   canonical encoding of the object kind, the cell and the native id.
3. The key never appears in a support bundle or in configuration output.
   The daemon keeps no reverse table.
4. `afs` can resolve a path to its reference under the caller's existing
   permissions. A raw id is accepted only when the daemon can establish, on
   its own, the caller's right to see that object.
5. Native identifiers may appear in: catalogued audit fields; scoped,
   audited captures (O7); and routine logs under an administrator-locked
   policy whose changes are audited.
6. A user's capture is written only to a file created with user-only
   permissions, and holds only that caller's authorised view (O16).
7. On macOS only cleared values are marked public; everything else is
   private text, which is collectable when private-data capture is enabled
   `[DES-020]`. On Windows the read access of every IronAFS channel is set
   and tested.
8. References are pseudonyms, and the documentation says so. Retention and
   collector access belong to the site.

Why: paths and names in a shared file system are personal data. A keyed
reference lets two records be matched without publishing the name.

### O7. Storm control, run-time levels and scoped captures

**Working.** Revisit if: a real outage shows the summaries hide the first
cause, or a capture's scope proves too narrow to diagnose a reported case.

1. State changes are logged once: one "server unreachable" when a server
   goes down and one when it returns. Failures in between are counted and
   summarised per window.
2. A per-subsystem limit is the last guard. Sampling is used only for debug
   events.
3. A sink never blocks the daemon. A stalled OS logger costs counted drops.
4. Log levels can be raised at run time, per subsystem, always with an
   expiry.
5. `afs debug capture` is scoped to one user or one path and expires.
   Starting a capture is an audit event.
6. A capture can record each Rx call at the RPC level: server, connection
   and call number, the call's name, file ids, security level, result or
   abort code, bytes and latency. It never records keys, data or tokens.
   File names follow O6.

Why: an encrypted Rx connection makes a packet capture opaque, so the
client must be able to say what it called. An expiry stops a debug level
from becoming permanent.

### O8. Native log sinks

**Working.** Revisit if: an OS changes its logging interface, a supported
release cannot retrieve an entry written this way, or the bindings ADR is
refused.

1. **Linux:** journald's native protocol over its Unix socket, written by
   our own code `[DES-043]`, with a journal catalogue. RFC 5424 syslog is
   the fallback.
2. **macOS:** unified logging through a one-file C shim, statically linked
   into the daemon, so entries are attributed to the daemon itself.
   - The shim is built by a first-party build script with the system
     compiler found through `xcrun` `[DES-021]`, with warnings as errors
     and no build crate.
   - Its call sites are named in the ADR for the OS-bindings crate
     (`ironafs-os`), which is later work.
   - The format is fixed: a public event id, a public string and a private
     string `[DES-020]`.
   - `default`, `error` and `fault` are used for events meant to persist.
     `info` and `debug` are kept for the debug audience.
   - The FSKit extension logs natively from Swift, for non-audit events,
     when the daemon is unreachable (O4).
   - If the bindings ADR refuses the shim, a Swift relay over the existing
     IPC carries the events, with the original component's metadata.
3. **Windows:** a manifest-based Event Log and ETW provider with Admin,
   Operational, Audit and Analytic channels. The Audit channel uses custom
   isolation, and its access descriptor lets only the service's own SID
   `[DES-074]` write to it (Context 2).
4. **Syslog structured data.** Until a Private Enterprise Number is
   assigned `[DES-045]`, the client puts the catalogue event id in the
   RFC 5424 `MSGID` and emits no private structured data. Adding
   structured data later does not change those identifiers.
5. The client keeps no log store of its own (O21).

Why: administrators already collect these logs with the tools they have.
Each sink is the operating system's own interface, which rule 21 allows.

### Audit

### O9. Audit comes only from the daemon, and its loss is always visible

**Firm.**

1. Audit is a separate stream with its own ids. Only the daemon emits it,
   and only for operations the daemon observes itself.
2. No other component can cause an audit event to be written by sending
   one. The producer-role check (O3) refuses it, and a direct write (O4)
   is never audit.
3. Each audit event carries a correlation id, the host, the local user (uid
   or SID), the AFS principal, the outcome and a sequence number.
4. Audit is never dropped silently. When audit events are lost, a gap event
   states how many, including across a daemon crash (O10).
5. The audited operations are at least: token set, renewal and loss;
   authentication failures; ACL changes made through the client;
   configuration and policy changes; daemon start and stop; adapter
   authentication failures; journal discards; the start of a debug capture;
   an administrator's removal of another user's token.

Why: an audit record is evidence. It is worth nothing if a process acting
for a user can forge one, or if records can vanish without a trace.

### O10. Audit capacity, durability and collection

**Working.** Revisit if: audit volume in real use makes the durable
checkpoint a measurable cost, or a sink gains an acknowledgement the client
can use.

1. Audit has reserved queue capacity apart from diagnostics. When that is
   exhausted, the daemon emits a gap event with the count lost.
2. The daemon keeps a small checkpoint in its state directory: its instance
   id, the audit sequence ranges admitted, and the highest sequence
   delivered to the sink. The admitted range is written durably before any
   of it is delivered.
3. For token set, ACL change and configuration change, the audit event is
   admitted before the operation's result is returned.
4. At the next start, any difference between admitted and delivered becomes
   a gap event. The record separates confirmed delivery, possible duplicate
   and possible loss.
5. Each sink's acknowledgement is stated in the documentation. journald's
   native protocol is a datagram socket `[DES-043]`, so "sent" is the most
   the client claims there.
6. A stalled audit sink makes health "degraded", with the reason, visible
   through `afs status` and the service status even when normal logging is
   unavailable.
7. Collectors key on trusted origin, never on fields a client could set:
   journald's trusted fields (Context 2), the Windows Audit channel's
   access control, and the macOS daemon's process identity. The
   administrator's guide says so.

Why: the checkpoint is what makes O9's "loss is visible" true across a
crash.

### Metrics, health and diagnostics

### O11. Metrics and health, with no listener by default

**Working.** Revisit if: a site shows a need the metrics file cannot meet,
or a collector the sites use cannot read a text file.

1. Metrics are counters and fixed-bucket histograms with bounded labels,
   never per user or per path. They cover: RPC latency per server and per
   RPC, retransmissions, aborts by class, callback breaks, cache hit ratio
   and occupancy, pending stores and conflicts, queue depths and drops, and
   the token expiry horizon.
2. By default they are read through `afs status --json` and a Prometheus
   text file `[DES-044]`.
3. The file is published as a complete snapshot, by same-directory
   replacement from a temporary name that does not match `*.prom`
   `[DES-044]`.
   It carries a last-successful-update gauge. A failed replacement
   publishes nothing partial and never blocks the daemon.
4. The default metrics file is readable by administrators only.
5. The daemon starts no metrics listener by default. An opt-in HTTP
   listener is built only for a demonstrated need. If built, it binds only
   numeric loopback addresses, serves only `GET /metrics`, validates the
   Host header against the bound address and port, bounds request size,
   time and connections, and publishes only aggregates approved for every
   local process. Its parser is class S. Exposure beyond the host is the
   site's own step.
6. Health is healthy, degraded with reasons, or failed. It is reported to
   systemd by notification and watchdog `[DES-042]`, to the Windows service
   status, and through `afs status --json`.
7. OTLP export is later work (O51).

Why: a file needs no open port, and a monitoring agent that is already
installed can read it.

### O12. `afs status` and `afs diagnose`

**Working.** Revisit if: support cases show a recurring failure neither
command names.

1. `afs status` shows: version, cells, per-server state (up or down, last
   contact, round-trip time, callbacks held, revalidation mode), cache
   occupancy, pending stores and conflicts, the active security protocol,
   and the caller's own token state. It never shows key material.
2. `afs diagnose` checks: clock skew against servers and the KDC; tokens;
   DNS and cell discovery; reachability of the volume location and file
   servers; callback reachability; cache disk health and storage protection
   (O27); and the OS layer's own state (FUSE mount flags, FSKit extension
   enablement, the SMB route and its configuration).
3. `afs diagnose tokens` walks the authentication chain for the caller: the
   credential cache found and its type `[PLT-014]`, ticket lifetimes, the
   KDC reached and how it was found, encryption types offered and accepted,
   clock skew, the `aklog` step and the token it produced.
   - It names each failure, with the Kerberos or GSS status translated into
     a sentence and a remedy.
   - It labels each fact as observed by the user's Kerberos library or
     unavailable.
   - It shows names, times and encryption-type numbers, never ticket or key
     material.
4. Every read command has a versioned `--json` form (O36).

Why: authentication and reachability are the common support cases, and an
administrator should not need a developer to tell them apart.

### O13. Cache inspection and flush

**Working.** Revisit if: stale-data reports are not settled by what the
command shows.

1. `afs cache show <path>` reports what the client holds for that file: the
   cached data version and the server's, the volume incarnation evidence,
   the chunks present, whether a callback is held and until when, the
   caller's cached rights and when they were fetched, pending stores, and
   why the last read was served from cache or fetched again.
2. It needs a current grant on the path, and shows only the caller's own
   view (O16).
3. `afs cache flush` (file, volume, all) acts only on the caller's own
   private entries. Flushing a shared pool or everything is an
   administrator command.

Why: most "stale data" reports end with the question this command answers.

### O14. Support bundles

**Working.** Revisit if: support work shows a bundle lacks what a diagnosis
needs, or a review finds content that should not leave the machine.

1. `afs support-bundle` writes redacted logs, the effective configuration,
   versions, counters and a state dump.
2. It is a plain archive written by our own code. It lists its contents
   before it is shared. It is never a memory image.
3. It is written locally. Nothing uploads it (O20). The project offers a
   private route for submitting one, and explains handling and deletion
   before receiving any.
4. Filtering to the caller's view comes first, then redaction (O5, O6,
   O16).

Why: a bundle the administrator can read before sending is one a security
team will allow.

### O15. No remote debug service

**Working.** Revisit if: a supported fileserver is shown to need an answer
to a diagnostic request before it will deliver callbacks.

1. The client offers no remote debug service and no setting to enable one.
2. The callback service on the client's port `[DES-011]` answers an
   allowlist of calls: callback delivery, state management, probes, and the
   identity, capability and address discovery that supported fileservers
   require. The allowlist is written from published protocol material and
   clean-room interoperability tests (ADR 0001).
3. Legacy Rx diagnostic and version requests are dropped without a reply.
4. A valid, parsed but unsupported callback call gets the documented
   unsupported-operation response when protocol state and the reply budget
   permit. Malformed, incomplete or rate-limited traffic is discarded
   silently.
5. Parsing, allocation, response size and rate-limit state are bounded.
   Required callback traffic stays serviceable under diagnostic load.
   Refusals and drops are counted in aggregate.
6. The documentation says that legacy remote tools receive no diagnostic
   data, and that sites collect local diagnostics with their own remote
   management.

Why: an unauthenticated network query that describes a client's state is
exposure with no local owner. Everything it would answer is available
locally under O12 to O14.

### O16. A user sees only their own state

**Firm.**

1. Every diagnostic surface has two projections. A user sees only their own
   state and events. An administrator sees all.
2. This covers `afs status`, `afs diagnose`, `afs cache show`, `afs debug`,
   captures, call traces and support bundles.
3. Every diagnostic record, capture and counter shown to a user is tied, at
   collection, to the credential context it concerns. Shared fetches, pool
   entries and per-server counters are administrator-only.
4. Filtering happens before export, and redaction after it.

Why: a diagnostic that shows one user what another user opened, or whether
they hold a token, is a cross-user leak by another route.

### Configuration and management

### O17. Layered configuration, and policy locks its keys

**Working.** Revisit if: a supported management system cannot deliver
policy through the mechanism named here.

1. Layers, lowest first: built-in defaults; files with drop-ins; OS policy;
   and expiring run-time overrides, limited to debug levels and captures.
2. A value set by policy locks its key. No lower layer and no run-time
   override changes it.
3. `afs config show --effective --origin` names where each value came from.
4. **Windows:** ADMX and ADML policy templates, writing under a policy key
   of the project's own. **macOS:** managed preferences through
   configuration profiles. **Linux:** files and drop-ins, suited to
   configuration management.
5. Install and upgrade are silent on every OS (O37).

Why: a fleet is configured centrally, and an administrator must be able to
see that a local file did not override the site's policy.

### O18. The configuration file

**Working.** Revisit if: the TOML subset cannot express a setting a later
phase needs, or administrators' tools cannot write it.

1. The format is a strict subset of TOML, parsed by our own code (rule 21):
   `ironafs.toml` plus `ironafs.d/*.toml` drop-ins.
2. An error names the file, line, column, what was expected and a hint.
3. The classic client's `CellServDB` `[DRV-090]` and `ThisCell`
   ([ThisCell(5)](https://docs.openafs.org/Reference/5/ThisCell.html))
   files are read as compatibility inputs. The order is: IronAFS configuration,
   then the classic files, then DNS.
4. `afs config check` rejects unknown keys in local files, with a
   suggestion.
5. `afs config reload` (and SIGHUP on Unix) validates first and keeps the
   last good configuration on error. A setting that needs a restart says
   so.

Why: a typo in a security setting must fail loudly. A small in-house parser
is easier to audit and fuzz than a general one.

### O19. Policy across client versions

**Working.** Revisit if: a management system is shown to drop or reorder
the envelope, or sites need a state this list lacks.

1. OS policy carries a small, stable envelope that every client version
   reads: `min_client_version` and a `required` list of keys.
2. The daemon ignores and reports unknown policy keys, so a newer policy
   works with an older client.
3. The exceptions are a key named in `required` that the client does not
   know, and a `min_client_version` newer than the client. Either makes the
   policy **incompatible**: health is "failed" with the reason, new
   admissions are blocked and existing grants are fenced, at start and on
   reload. It is never silently ignored.
4. The four policy states:
   - **Unmanaged:** no policy is configured. The client runs on defaults
     and local files and reports "unmanaged".
   - **Valid.**
   - **Unreadable:** a policy exists but cannot be read. The client uses
     the last good policy it verified, kept in its state directory, and
     reports "degraded". With no last good policy it does not mount, and
     reports "failed".
   - **Incompatible:** as in point 3.
5. Returning to unmanaged needs the managed source's own removal signal. A
   policy that merely disappears is treated as unreadable.
6. Security floors in policy follow N8: an unknown security value
   refuses the cell.

Why: policy is where a site sets its security floor. An older client that
skipped a key it did not understand would run below that floor without
anyone knowing.

### Data that leaves the machine

### O20. Nothing phones home

**Firm.**

1. IronAFS contains no telemetry, no crash upload, no usage statistics and
   no update check. None exists behind a setting, whatever its default.
2. Operational data goes only where the site sends it: the OS log, the
   site's collector, or a support bundle an administrator chose to share.
3. Protocol negotiation with a cell (N17) is independent of software
   update, and never fetches software or policy from the project.
4. The administrator's guide states points 1 and 2 in one sentence that a
   security review can quote. The README says the same.

Why: a file-system client sees every path a user touches. Sites need a
statement about outbound data they can rely on without reading the source.

### O21. Retention, and what the client tells others by working

**Working.** Revisit if: an OS log cannot apply a site's retention to
IronAFS's entries, or interoperability tests show a smaller disclosure to
servers is possible.

1. Retention is the operating system's. The client keeps no log store.
   journald, unified logging and the Event Log apply the site's retention.
2. The only diagnostic files the client writes are dumps of its in-memory
   event ring, debug captures and support bundles. Each has a size cap and
   an expiry, and is deleted when it expires.
3. The documentation states what the client discloses simply by working:
   - resolving a permitted cell name can contact DNS and that cell's
     servers;
   - the source address a server sees, the callback addresses registered
     with it, and the client identifier sent at callback registration;
   - that unauthenticated AFS traffic is not encrypted.
4. The recipients and fields actually observed are published for each
   supported mode.
5. The client registers as few addresses as callback reachability allows.
   A stable per-cell client identifier derived from a local secret is
   adopted only after interoperability, restart, clone and recovery tests.
6. A site's cell policy applies to discovery and to authentication.

Why: "nothing phones home" (O20) is only honest beside a plain account of
what the protocol itself reveals.

### Security

### O22. The security invariants

**Firm.**

These hold in every phase, on every OS and at every setting. No acceptance
gate waives them (O46).

1. **No cross-user leak.** One local user never obtains file data,
   metadata, rights or diagnostics through another user's AFS authority,
   whether through the client's cache, an OS cache, a handle, a mapping or
   a diagnostic.
2. **No local privilege escalation through AFS content.** Set-uid and
   set-gid bits and device files in a cell grant nothing on the local
   machine.
3. **No silent wrong answer.** The client never presents stale data as
   fresh, a partial listing as complete, or a lost write as saved, and
   never lets a deferred store replace newer server data without reporting
   a conflict. The one named exception is the opt-in stale mode of O34,
   which is explicit and shown as such.
4. **No silent downgrade of protection.** A failure never lowers a security
   floor (N6).
5. **Nothing unauthenticated feeds an authenticated path.** Data fetched
   without authentication, or with weaker protection, is never served,
   cached or trusted as if it had stronger protection (C19, C20, N5).
6. **No secret outside the processes that need it.** No token, key or
   session key appears in a log, capture, bundle (O5), a dump (O25) or
   persistent storage (C42).
7. **Input from outside is never trusted.** Fileserver replies, callbacks,
   Rx packets, local IPC, configuration, OS requests and SMB messages
   before authentication are decoded with bounds and without panics
   ([coding standards](../coding-standards.md) rules 1 to 3).
8. **No copied code.** ADR 0001's rule covers tests, simulators and
   fixtures as much as the client (O45, O48).

Why: these are the failures that make a file-system client unsafe to
deploy even when it otherwise works. Each is stated once here so that every
other decision can be checked against it.

### O23. The threat model

**Firm.**

The model names who the client defends against, with what, and what
remains. The defences are decided in the parts cited. Changing who is in
scope needs a superseding ADR.

| Adversary | Defences | What remains |
|---|---|---|
| A hostile or compromised fileserver | Bounded decoding (O22.7); byte caps per queue; per-server budgets; deadlines; callback rate limits that mark a server's callbacks suspect and never drop a break (C31, C32); loop bounds; `nosuid,nodev` mounts (L3, L7) `[DES-035]` | It can serve wrong data for the files it hosts, because an AFS client trusts its cell's servers. Where a cell's servers share keys, IronAFS treats the whole cell as the compromise domain |
| A network attacker against rxkad | The encrypt level by default; rxkad-k5 and rxkad-kdf `[DES-049]`; cache namespaces private to each credential context (C18), so a tampered reply is reused only in the context whose fetch it answered; rxgk later (O50) | rxkad's age (Context 6), and IronAFS's position (rule 4) that no rxkad level protects a reply's payload. A tampered reply can become the base of that user's own write, and a weakly protected store can change server data. Sites that need write integrity are told that rxgk at the server is the fix |
| Another local user | Per-file, per-principal rights and handles that carry authority (C22, C13); IPC peer credentials; per-user budgets; user projections (O16) | Timing of cache occupancy under shared disk pressure, bounded by per-account floors. In the opt-in shared pool (C21), an authorised reader can tell whether a file range is warm |
| The local administrator | None | Out of scope. An administrator can read tokens in memory and the cache |
| A malicious cell configuration or DNS answer | Configured cells win over DNS; tokens are sent only to their own cell's servers; Kerberos mutual authentication; set-id bits ignored | Unauthenticated traffic to a spoofed server |
| A malicious local IPC client | Bounded decoding; OS peer credentials; per-user budgets; adapter bindings only by the daemon's own registration (C45); producer roles for events (O3) | None known |
| A local attacker against the Windows loopback SMB listener | Authentication in the daemon and a front end that holds no AFS tokens; signing required; a loopback-only listener that no other process can take over while the service is stopped; limits before authentication (L18, L20) | The machine-wide configuration the route needs (L21). A front end compromised while users have sessions open can act within those sessions |

Rules that follow from the table:

1. Encryption at rest is the site's and the OS's job (O27). The client's
   own isolation between credential contexts does not depend on it.
2. A reported server build version is diagnostic only. It is never an
   input to a trust decision (O39).
3. Each residual in the third column is stated in the administrator's
   guide.
4. **IronAFS's stated security position on rxkad:** IronAFS relies on no
   rxkad level for the integrity of a reply's payload. This is the
   project's own position, not a cited fact: the claim record holds no
   public source that settles rxkad's per-level packet protection
   (Context 6). The specification route for rxkad (O50.3) confirms or
   corrects it; until then every decision that depends on reply
   integrity treats rxkad replies as unprotected.

Why: a threat model that leaves out the residuals invites people to rely on
protection that is not there. The local administrator is out of scope
because no user-space client can defend against the account that controls
the machine.

### O24. Least privilege for every process

**Firm.**

1. No IronAFS component runs as root or SYSTEM, except the minimal mount
   step and the installer.
2. On Windows the process that parses SMB messages holds no AFS tokens
   (L18).
3. A component that acts for any user (an OS adapter) is admitted only by
   the daemon's own registration (C45), behind an OS access-control
   barrier.
4. The per-OS hosting that carries this out (the Linux privileged mount
   unit and unprivileged daemon with `NoNewPrivileges` `[DES-041]`, the
   macOS launch daemon and extension, Windows virtual service accounts
   `[DES-074]`) is decided in L7, L14 and L23.

Why: the daemon parses hostile network input and holds every user's
tokens. Whatever compromises it should gain as little of the machine as
possible.

### O25. No secret in a dump

**Firm.**

1. No memory image that IronAFS controls contains a token, key or session
   key.
2. Every process that can hold credentials, or handle data beyond the
   invoking user's own, has automatic memory dumps disabled from startup:
   the daemon, the SMB front end, and the CLI and login service while they
   hold a token.
3. A filtered dump is allowed only by an explicit, per-process
   administrator opt-in for a verified collector and configuration. Buffer
   exclusions are then an additional layer, never the only one.
4. A panic emits a catalogue event with a build id and an approved source
   location. Panic payloads and raw addresses are left out of default
   events.

Why: a core file is written outside the process's protection, where the
client no longer controls who reads it or whether it is attached to a bug
report.

### O26. How dumps are disabled, and what stays outside the client's control

**Working.** Revisit if: an OS changes its dump controls, a test finds a
marker secret in a controlled dump, or an OS offers verified exclusion of
named buffers.

1. Dump controls live in the OS-bindings crate, under its ADR.
2. **Linux:** the core limit is set to zero and the process is made
   non-dumpable. A limit alone is not enough, because a piped dump ignores
   it `[DES-039]`. The non-dumpable setting is verified at startup and
   after any transition that can reset it `[DES-038]`. Secret buffers are
   also excluded with `MADV_DONTDUMP` where available `[DES-040]`.
3. **Windows:** planned as suppression of Windows Error Reporting for
   IronAFS's processes, applied before sensitive work starts. Microsoft
   documents excluding a named application from error reporting
   ([WerAddExcludedApplication](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nf-werapi-weraddexcludedapplication));
   the exact calls are named in the bindings ADR. Machine-wide exclusions
   are the site's policy.
4. **macOS:** the core-file limit (`RLIMIT_CORE`, Context 5) is set to
   zero. The hardened runtime
   `[MAC-063]` is used for its own integrity protections.
5. Tests plant marker secrets in acquisition, transport and teardown
   buffers, in each protected process on each supported OS configuration,
   and search every controlled memory image for them.
6. The documentation lists what the client cannot control: OS crash
   reports with register contents, administrator-configured and live
   dumps, swap, hibernation, whole-machine dumps and VM snapshots.

Why: O25 is only as good as its check on each OS. The marker test is that
check.

### O27. Encryption at rest is the operating system's

**Working.** Revisit if: after the first releases, sites ask for
per-context encryption backed by an OS keystore, or an OS stops reporting
storage protection reliably.

1. OS storage encryption is a site requirement for every persistent
   location that can hold file data: the cache, the journal, the recovery
   store, paging, hibernation and dumps.
2. `afs diagnose` reports protected, unprotected or unknown for the
   resolved storage locations, where the OS exposes a reliable status. It
   raises an event for storage known to be unprotected. The client makes no
   startup decision from that signal alone.
3. A read-only volatile mode writes no journal and no recovery store. It
   uses bounded, memory-backed caching only where paging, hibernation and
   dump policy prevent unprotected persistence. Existing pending and
   recovery data is preserved and quarantined for explicit recovery or
   export.
4. The client does not write tokens or session keys to storage (C42).
   It locks client-controlled secret memory where the OS supports it, and
   reports a failure to do so.

Why: the local administrator is outside the threat model (O23), and the
operating system's disk encryption already protects against a stolen disk.
A second layer of the client's own would add key management without
changing who can read the data.

### O28. Constant-time comparison and zeroisation at named places

**Firm.**

1. Secret-dependent comparisons run in constant time at: rxkad checksums
   and challenge responses, rxgk message integrity checks, token
   replacement, IPC secrets and SMB session keys.
2. Session keys are zeroed at connection teardown. A replaced token's key
   material is zeroed when replaced.
3. The places are named in code and listed in the specification for each
   component, so a review can check the list is complete.

Why: a timing difference in a checksum comparison is a remote oracle, and
key material left in freed memory outlives the session it protected.

### O29. Supply chain and release signing

**Working.** Revisit if: a tool named here is abandoned, GitHub's
attestation level changes, or a platform changes its signing rules.

1. Dependencies: `cargo-deny` (in place), and `cargo-vet` for the few
   crates rule 21 allows.
2. Builds are reproducible, checked in CI. `xtask` generates a software
   bill of materials.
3. Release artefacts carry GitHub artifact attestations, which give SLSA
   Build Level 2 `[DES-046]`, and OS-native signatures.
4. Workflow actions are pinned by commit.
5. Repository write access and release-signing keys are kept off test
   runners. Signing is a separate step, after verifying the digest and
   provenance of the trusted build of the reviewed release commit.
6. How clients discover and trust package-signing keys is frozen with the
   identifiers (O38). Planned and emergency key rotation are both
   supported.

Why: a client installed fleet-wide as a system service is a high-value
target for a tampered build.

### O30. Vulnerability handling

**Working.** Revisit if: GitHub changes private vulnerability reporting, or
legal advice changes the disclosure defaults.

1. `SECURITY.md` names the supported versions, the private reporting
   route, a fallback contact and an acknowledgement target.
2. The default is coordinated disclosure within 90 days, earlier when a fix
   is available or exploitation warrants it. An extension is agreed with
   the parties concerned. Any legal reporting deadline is met regardless
   of an embargo.
3. An embargoed report that affects other AFS implementations is shared
   only with their security contacts, with as little as needed.
4. Fixes under embargo are tested locally or in isolated private CI, and
   get an explicit review and test check before merging through an
   advisory. Embargoed work stays out of public issues until disclosure
   ([docs/workflow.md](../workflow.md) §6).
5. An advisory states affected and fixed versions, workarounds, and how to
   check a version and update by hand. Because nothing phones home (O20),
   administrators are pointed to the repository's published security
   advisories and its releases.
6. A CVE is requested for each advisory.
7. Whether the EU Cyber Resilience Act
   ([Regulation (EU) 2024/2847](https://eur-lex.europa.eu/eli/reg/2024/2847/oj))
   places duties on the project is assessed with advice, and reassessed if
   how IronAFS is supplied changes.
8. Before source implementing a non-standard cipher is published, the
   applicable export classification and notification duties are checked and
   met.

Why: with no update check, the advisory is the only channel that tells an
administrator to act.

### O31. FIPS 140 mode is unscheduled

**Working.** Revisit if: client-to-fileserver rxgk interoperates with an
identified peer, and active CMVP certificates cover the exact operating
environments, module versions and cryptographic services the client would
use.

1. No FIPS 140-3 mode is scheduled until both conditions above hold.
2. A future mode requires authenticated rxgk with integrity or encryption,
   limits its Kerberos profile to RFC 8009 encryption types `[DES-064]`
   whose complete use has been verified, and refuses rxkad, NTLM,
   unauthenticated access and any fallback.
3. In that mode a per-platform table (module, version, operating
   environment, algorithm, mode) decides what may run. An operation outside
   it is refused.
4. A documented OS policy that requires approved cryptography makes
   IronAFS fail closed by default. A site can require the same on any
   platform with `fips = require`.
5. `afs status` reports the active protocol and any unsupported or unknown
   platform capability.
6. The project publishes the covered configurations and describes approved
   use of the modules. It never claims IronAFS itself is validated.
7. The work that prepares for it is in the phase plan (O50): specifying
   rxgk's Kerberos encryption-type operations inside the client's
   cryptography boundary (C5), and documenting the GSS exchange and its
   pseudo-random function as platform dependencies.

Why: the approved block ciphers are AES, TDEA and SKIPJACK, the last two
for legacy use only
([SP 800-140C](https://csrc.nist.gov/projects/cmvp/sp800-140c)); rxkad's
cipher, of roughly single-DES strength (Context 6), is none of them. So a
FIPS mode cannot exist before rxgk does, and no released OpenAFS offers
client-to-fileserver rxgk `[DES-048]`.

### Lifecycle

### O32. Pending writes and durable state survive an upgrade

**Firm.**

1. From the first release that has a write journal, each release reads or
   safely migrates the preceding release's journal, recovery store and
   durable state. The host UUID and the meaning of every record are
   preserved.
2. The clean cache's format is not promised across versions. An unknown
   clean-cache format discards the cache. It never discards the host UUID,
   the journal, the recovery store or other durable state.
3. An upgrade or rollback never discards, forces or silently replays a
   pending or quarantined write. Quarantined records are kept.
4. The record formats, their versioning and the state directory's versioned
   layout are decided in N8 and N32 to N35. The journal's behaviour is
   decided in C28 and C29.

Why: a pending write is a user's data that exists nowhere else. A clean
cache can always be fetched again, so only it may be thrown away.

### O33. The upgrade and rollback procedure

**Working.** Revisit if: upgrade tests show the preflight cannot prove what
it claims, or packaging on a supported OS cannot stop before changing the
installation.

1. An upgrade or rollback first quiesces new writes.
2. It proceeds unattended only when a preflight confirms the target can
   preserve all durable state and replay pending writes, or when a bounded
   drain leaves nothing pending and the remaining state is compatible.
3. Otherwise it exits promptly with the current installation and data
   intact, and says how to recover.
4. Rollback is offered for one release, and only on package and state
   transitions that have been tested.
5. `afs pending export` is the explicit way out when a rollback cannot
   preserve the state.
6. Local IPC between an older `afs` and a newer daemon during an upgrade
   follows C44's compatibility rules.
7. A graceful shutdown journals unsaved data. It does not wait to store it.

Why: an upgrade pushed to a fleet runs with no one watching, so it must
stop safely by itself when it cannot prove it is safe.

### O34. Offline behaviour

**Working.** Revisit if: sites show that the default fails work they could
safely have continued, or the stale mode's entry and exit rules prove
unclear to users in testing.

1. **Normal mode.** Cached data is served while its callback is still
   trusted and the credential and rights grants are valid, with no extra
   time cap since last contact. A callback break, or a known loss of
   callback continuity, ends that. A request that cannot be validated fails
   at its deadline.
2. **Planned, opt-in read-only stale mode** (O51):
   - entered only by an explicit command, or by a machine policy that names
     cells or volumes; never automatically;
   - serves only data previously authorised in the same credential context;
   - ends at token expiry, on a known revocation, or when validation
     succeeds;
   - cache misses and all mutations fail;
   - file bytes are never altered;
   - status shows the mode and the last validation time, and an event marks
     each transition.
3. The stale mode is the one named exception to O22.3.
4. Disconnected writes are not offered.

Why: serving unvalidated data by default would break the promise that a
callback break is honoured. A user who knowingly chooses stale read-only
data has been told what they are reading.

### O35. Errors a user can act on

**Working.** Revisit if: a platform offers a richer per-request error
channel, or desktop notifications prove to be noise in user testing.

1. IronAFS assumes a failed OS request reports only the platform's error
   code to the application, so each mapped error also has a side channel.
   The daemon records the cause for that user. `afs status` prints it, and a rate-limited desktop notification
   states it, with the cell, the time and the remedy.
2. An expired token is a permission error (`EACCES` on Unix), never an I/O
   error.
3. A missing token and an expired token are distinct causes.
4. Message ids are stable (`IRONAFS-E…`) and documented.
5. Each layer maps the core's error set to its platform's codes. The set is
   decided in C46.

Why: "Input/output error" tells a user nothing. "Your tokens for this cell
expired at 14:02; run aklog" tells them what to do.

### O36. Tokens at login, and the command line

**Working.** Revisit if: a platform's login integration changes, or sites
show that a PAM module is needed where `pam_exec` does not serve.

1. A per-user login service on each OS runs `aklog` at login, when the
   user's Kerberos credentials change, and when the daemon restarts. It
   renews before expiry by a margin counted on the boot clock (C37).
2. On Windows the per-session agent is started at logon by a Task Scheduler
   logon trigger. A service that is not LocalSystem cannot obtain a user's
   token itself; the logon trigger is this design's choice among the
   documented ways to start a program at logon (`[DES-075]`, corrected
   form).
3. No PAM module is written, because it would need C and `unsafe`.
   `pam_exec`, a PAM module that runs an external command
   ([pam_exec(8)](https://man7.org/linux/man-pages/man8/pam_exec.8.html)),
   is documented for sites that need it.
4. Token custody, and what may be reacquired after a restart, are decided
   in C42.
5. The native command is `afs`. Every read command has a versioned
   `--json` form. No meaning is carried by colour alone. Man pages are
   generated from the CLI's own definitions.
6. `fs`, `tokens`, `aklog` and `unlog` exist as compatibility aliases. They
   accept the common options, keep output close enough for common parsers,
   and have documented, tested exit codes.
7. The aliases ship in a separate, opt-in package, with file-conflict
   rules for each package manager, so they can be installed beside or
   instead of another AFS client's commands.
8. `aklog`, `unlog` and `tokens` work by cell name without reading `/afs`.

Why: users of an existing AFS client keep their commands and scripts. Sites
that run both clients during a migration are not forced to choose at
install time.

### O37. Packaging and installers

**Working.** Revisit if: a packaging tool's licence or behaviour changes,
or a platform's install rules change.

1. **Linux:** RPM and DEB packages from one staged tree: the privileged
   mount unit and the hardened daemon unit `[DES-041, DES-042]`,
   `sysusers.d` and `tmpfiles.d` entries, and indexer exclusions. A static
   musl archive is also produced.
2. **macOS:** a signed, notarised `.pkg` `[MAC-063]` with the extension's
   app, the launch daemon and the CLI. An uninstaller is included. The
   launch daemon needs an administrator's approval, or pre-approval through
   device management `[MAC-M27]` (`[MAC-M28]`, corrected form). Whether
   device management can enable the FSKit extension is open, and is
   settled by the macOS gate (L13).
3. **Windows:** a plain MSI built with a pinned release of the WiX
   toolset.
   - WiX is a build-time exception to rule 21. Its exact source licence and
     binary terms are recorded through the `add-dependency` checklist
     before it is adopted.
   - Standard MSI actions come first. No bootstrapper, no toolset
     extension custom actions and no DLL custom actions are used.
   - Where an action is needed, it is our own signed executable helper, run
     from an installer-controlled path, with validated inputs.
     State-changing actions are deferred. Rollback and uninstall are
     tested.
4. Install and upgrade are silent on every OS.
5. The Windows publisher and the Apple signing team are settled before the
   first external build that is meant to upgrade (O38).

Why: fleets install with their own deployment tools, which expect the
platform's native package and a silent mode.

### O38. Identifiers: one inventory, and when they freeze

**Working.** Revisit if: a name-derived identifier cannot be cleared before
the freeze point, or qualification of `\\afs` (L34, L41) fails.

1. One inventory lists every identifier the OS, administrators and scripts
   depend on, with its owner, whether it depends on the project's name, and
   what breaks if it changes. It covers:
   - macOS: the app and extension bundle ids, the file system short name,
     the logging subsystem and the managed-preferences domain;
   - Windows: the provider GUID, provider and channel names, the service
     name, the policy registry key and the installer upgrade identity;
   - Linux: mount, unit, package and directory names, journal fields and
     message ids;
   - metric names and syslog structured-data names.
2. Identifiers that do not depend on the name are allocated now: a random
   provider GUID, the installer upgrade code and journal message ids.
3. A distributable build that still contains a reserved placeholder is
   rejected.
4. **Frozen from the first public release:** `/afs` as the Unix default,
   the MSI upgrade-family identity, the event id namespace (O2), and the
   assigned ETW provider GUID.
5. **`\\afs` as the Windows default** is frozen only after its
   qualification (L41): local-only resolution and connection, safe failure when
   the local service is absent, coexistence with Windows' own file sharing,
   and compatibility with the supported Kerberos service identity.
6. **Frozen before the first external build that supports upgrade, and no
   later than the first public release candidate:** the signing identities;
   app and extension ids; access groups; managed-policy domains and keys;
   service and package identities; configuration and state paths;
   protocol-visible names; and any published crate names.
7. Before that freeze, test builds use separate test identities and state.
8. After 1.0, an identity change needs a tested, platform-specific
   migration, with aliases where the platform supports them.
9. A Private Enterprise Number is applied for under the project's name,
   with a role mailbox as the public contact `[DES-045]`. Shipped
   `name@PEN` identifiers stay stable through any later change of the
   assignee's name.

Why: these names end up in policies, alert rules, scripts and installed
systems that the project cannot update. Changing one after release breaks
them quietly.

### O39. Servers supported, and how compatibility is decided

**Working.** Revisit if: another AFS-3 server passes the interoperability
suite, the OpenAFS 1.8 line ends, or a released server offers
client-to-fileserver rxgk.

1. Tier 1 is the maintained OpenAFS 1.8 line. The exact test cell is in
   the platform matrix (L40).
2. OpenAFS 1.6 is unsupported and untested, because it is end-of-life
   `[DES-047]`.
3. Cells that depend on single-DES service keys are unsupported
   `[DES-049]`.
4. The project publishes: the minimum tested 1.8 patch level; the Kerberos
   implementations and service-ticket encryption types tested; and that
   tier 1 offers no client-to-fileserver rxgk `[DES-048]`.
5. Compatibility is decided by the capabilities an operation needs and by
   the configured security policy. A reported build version is diagnostic
   only. The support tier alone never refuses a connection the policy
   permits.
6. Any other AFS-3 server is listed as supported only after it passes the
   interoperability suite (O48).

Why: a version string is supplied by the server and proves nothing. A
capability either works or it does not.

### O40. Protocol details that users notice

**Working.** Revisit if: the specification work for the file service (C6)
shows a server behaviour that contradicts a point here.

1. IronAFS assumes the file service offers 64-bit fetch and store calls
   beside 32-bit ones; the specification work (C6) confirms it. The client
   uses the 64-bit calls by default, and the 32-bit calls only when a
   server lacks them. Files
   across the 2 GiB and 4 GiB boundaries are tested.
2. Server times are converted at receipt. Clock skew is detected and warned
   about.
3. An over-quota or partition-full failure at store time is kept in the
   journal and reported. The data is never dropped (C28, C29).
4. Holes read as zeros. `fallocate` is refused.

Why: each of these is a place where a quiet shortcut would lose data or
corrupt a large file.

### O41. Where the cache and the journal may live

**Working.** Revisit if: crash and power-loss tests fail on a file system
the matrix supports, or a platform's qualified flush sequence changes.

1. The journal, the recovery store and the persistent cache live on a
   qualified, local, writable, non-removable data volume that is available
   before the daemon starts.
2. If the volume is locked, the daemon waits for it. It never creates
   fallback state elsewhere.
3. Network, FUSE, AFS-backed, read-only, removable and unqualified storage
   is rejected, by platform-specific startup probes.
4. The full commit sequence is qualified per platform with crash tests and
   representative power-loss tests that exercise volatile device caches.
   On-disk names are case-independent.
5. Under disk pressure the client evicts clean entries, stops filling the
   clean cache and emits an event. It preserves pending data and its read
   overlay.
6. Journal headroom is reserved. A write is acknowledged as durable only
   after its journal commit succeeds. On failure the native no-space or I/O
   error is returned, and recoverable pending data is kept.

Why: the journal's promise (O32) is only as good as the storage under it.

### O42. Backups, images and clones

**Working.** Revisit if: a platform offers reliable detection of a restored
or cloned machine, or sites show a need for client-side backup of pending
writes.

1. Committed AFS data is backed up at the server, by volume dumps. To
   include queued writes, `afs pending` is checked and the relevant clients
   are drained first.
2. A backup tool walking the mounted namespace sees only what its effective
   AFS identity can read. There is no dedicated AFS backup principal.
3. The cache, the journal and the recovery state are sensitive local data
   outside the supported backup procedure. OS backup and indexing
   exclusions are set where the OS supports them.
4. Before a machine image is captured, pending data is drained or exported
   and client state is removed, by a documented preparation procedure.
5. On a machine-identity mismatch, or a declared restore or clone, the
   client generates a new host UUID, discards the clean cache, and
   quarantines pending and recovery records for explicit recovery. Nothing
   is replayed automatically.
6. The restore procedure is required even on the original hardware. The
   documentation says that a snapshot which preserves all checked state can
   evade automatic detection.

Why: two machines with one client identity, or a restored journal replayed
over newer data, are silent wrong answers (O22.3).

### O43. Release and support policy

**Working.** Revisit if: maintainer capacity at 1.0 cannot carry the
long-term line, or a platform's own support window forces a change.

1. Before 1.0 there is one channel. `SECURITY.md` lists the latest version
   and promises no support window.
2. From 1.0: the latest stable release, plus one long-term-support line per
   year. The length of its security window is set at 1.0. Eighteen months
   is the proposal.
3. The project publishes exact end dates, the supported OS versions, and
   any limit on support for newly released OS versions.
4. `docs/support.md` names the compatibility surfaces: the CLI and its
   JSON, policy and configuration, events (O2), local IPC (C44), and
   durable formats (O32, N8, N32 to N35).
5. Upgrade paths between supported long-term lines are tested.

Why: a site planning a fleet deployment needs dates, and a promise the
project cannot keep is worse than none.

### O44. Generated troubleshooting reference and runbooks

**Working.** Revisit if: support cases show the generated entries are not
what administrators search for.

1. `cargo xtask events` also generates a troubleshooting reference: one
   entry per event, with its meaning, likely causes, what to check (the
   `afs diagnose` step) and the remedy.
2. Hand-written runbooks cover the common failures end to end: no tokens or
   expired tokens, clock skew, an unreachable server, callbacks not
   arriving, stale data, a full quota or partition, and an OS layer that is
   not loaded. Each links the event ids it starts from.
3. A runbook that cites a retired event id fails CI.

Why: documentation generated from the catalogue cannot drift from what the
client logs.

### Testing

### O45. A deterministic whole-system simulator

**Working.** Revisit if: the simulator cannot reproduce a failure seen
against a real cell, or keeping its server models in step with observed
behaviour costs more than the tests return.

1. The simulator runs the real Rx, cache and client code against simulated
   parts, all driven from one printed seed:
   - volume location and file servers, as our own minimal models written
     from the specifications (ADR 0001);
   - a network with loss, delay, reordering, duplication, partitions, NAT
     rebinding, and forged and dropped callbacks;
   - clocks, including suspend;
   - a disk with torn writes and no-space errors.
2. It is delivered with the first phase (O50), not after it.
3. Invariants are checked after every step:
   - no read returns bytes older than a version the client has seen broken;
   - no grant crosses principals or files, and no delivery happens after
     its grant's epochs changed;
   - every dirty byte is on the server, in the journal or in a conflict
     copy;
   - no deferred store lands over a server version it did not check, except
     inside the documented check-to-store window, and every such overwrite
     is reported as a conflict;
   - a lost store reply never counts as success;
   - every OS request completes within its deadline;
   - no queue exceeds its cap.
4. The simulator injects a competing write from a second simulated client
   inside the check-to-store window and during a lost reply.
5. The rule the simulator imposes on the code: threads live only in thin
   drivers, and the core never spawns or sleeps (C3, C39).
6. Event sequences fed to the simulator are fuzzed (O47).

Why: the failures that matter most (a stale read after a missed callback, a
store over newer data) depend on timing no real cell reproduces on demand.
The core's design without I/O (C3) is what makes this possible.

### O46. The acceptance rule

**Firm.**

1. A cross-user leak or a privilege escalation fails acceptance, in every
   phase and on every OS. It is never recorded as a known limitation and
   never waived.
2. Any other acceptance case either passes, or its gap is a documented and
   bounded limitation.
3. A phase gate that cannot show point 1 holds on a platform does not ship
   that platform's layer in the affected mode. The fallback for each layer
   is in L13, L20 and L41.

Why: these two failures are the ones a user cannot detect and an
administrator cannot contain.

### O47. Model-based, property and fuzz tests

**Working.** Revisit if: the fuzzing engine's platform limits change, or
A12 is settled differently when the first fuzz target lands.

1. **Model-based tests** compare the client against a small reference model
   of AFS semantics as IronAFS specifies them: the last closer wins;
   other clients see changes after a close and a break; all local handles
   see unstored bytes.
2. **Property tests** use our own seeded generator and shrinker (rule 21).
3. **Fuzzing** covers every decoder: XDR, Rx, RPC replies, the directory
   format, FUSE messages, local IPC, the configuration parser, DNS replies
   and SMB before authentication.
4. Fuzz targets follow cargo-fuzz's layout `[LAY-038]`. The engine runs
   only on Unix-like x86-64 and AArch64 with a nightly compiler
   `[LAY-041]`, so fuzzing runs on Linux runners. Where the fuzz directory
   sits in the workspace rests on A12.
5. The fuzzing engine's binding is the one planned test-only dependency
   exception, and goes through the `add-dependency` checklist.
6. Unit and integration tests run under nextest `[LAY-034]`, with doc-tests
   run separately `[LAY-036]`, as `AGENTS.md` already requires.

Why: every decoder faces input an attacker controls (O22.7), and a
generator of our own keeps the test stack readable under rule 21.

### O48. Interoperability and acceptance cases

**Working.** Revisit if: the test cell cannot be run in containers on
hosted runners, or a case here cannot be automated on a supported OS.

1. **Interoperability:** OpenAFS 1.8 servers in containers (a KDC, a
   database server, two fileservers and a seeded cell), driven by an
   `xtask` command.
2. Fault injection covers loss and delay, a server restart, and a volume
   release, move and restore during reads.
3. Packet captures are reduced to fixtures under ADR 0001: observed wire
   behaviour, with nothing taken from another implementation's source.
4. **Acceptance cases, on every OS:**
   - a second local user on cached data, including a file whose owner-read
     mode bit is clear;
   - token or ACL revocation during buffered, direct and mapped I/O, and
     with a descriptor passed to another process;
   - token renewal during I/O, with no interruption;
   - a callback break refreshing bytes, attributes and lookups;
   - overlapping lock contention and lock upgrade;
   - a set-uid file that grants nothing;
   - removing a file that is still open;
   - on macOS and on Windows, the layer gates (L13, L22, L41).
5. Every claim about mount behaviour is proven on its own venue
   ([docs/workflow.md](../workflow.md) §5).

Why: a real server settles what a model only assumes. The cases are the
ones where a user-space client is most likely to leak or lie.

### O49. Benchmarks and CI venues

**Working.** Revisit if: GitHub's hosted runners gain an OS target the plan
needs, or a self-hosted runner cannot be isolated as required.

1. Benchmarks in CI count; they do not time. They count: RPCs per long
   listing of N entries, packets per MiB at 1% loss, and bytes fetched on a
   warm restart.
2. Timed benchmarks run nightly on fixed hardware and are recorded as
   numbers.
3. Pull requests run only on GitHub-hosted runners.
4. OS targets that IronAFS does not expect GitHub-hosted runners to
   provide (FSKit activation and mounting on the supported macOS release,
   multi-user Windows) run from a separate
   private CI repository with repository-scoped runners, on fresh, isolated
   machines and reviewed commit SHAs. It is created when the macOS phase
   starts.
5. Domain-joined and Entra-joined Windows tests run in disposable labs with
   narrowly scoped access to dedicated test identity infrastructure.
6. Embargoed security tests run in a separate, access-controlled
   environment. Their inputs, logs and artefacts stay private (O30).
7. Runners never hold repository write access or release-signing keys
   (O29).

Why: counts are stable on shared runners where times are not. A runner
that executes a pull request's code is untrusted by construction.

### Phasing

### O50. The order of work

**Working.** Revisit if: a phase's gate fails, a specification route proves
unavailable, or an rxgk trigger in point 9 fires. The GitHub milestones are
the live record of the phases and their issues
([docs/workflow.md](../workflow.md) §6). This decision records the order
and the reasons. It is a plan, and it promises no dates.

1. **Core and Linux, read-only and unauthenticated (M2a).**
   - XDR; Rx with the null security class; volume location and file-service
     reads; callbacks with their trust and NAT rules.
   - The cache (C18 to C27), the layer contract (C7), the FUSE layer and
     its hosting (L2, L7), the dynamic root (C12), the boot clock (C37).
   - The simulator (O45); `ironafs-events` with the journald sink (O1,
     O8); `afs status` and the core `afs diagnose` checks (O12); the
     configuration parser (O18). The state directory is versioned from this
     phase (N8).
   - Specification gates come first: the volume-incarnation evidence, the
     directory format and the callback procedures.
   - Unauthenticated data is never cached (C20). So acceptance against a
     real cell is fresh anonymous reads. The cache and callbacks are
     exercised in the simulator under an authenticated test profile, and on
     a real cell in the next phase.
2. **A Windows SMB identity spike, in parallel and small.** A prototype
   SMB 3.1.1 listener on disposable Windows VMs settles A3 and A4 early:
   the caller's identity, dialect and signing through the loopback route;
   authentication modes; relay and port-takeover tests; and Windows' own
   file sharing still working. L22 states what a failure changes.
3. **rxkad (M2b).**
   - Gated on a specification route, under ADR 0001, that covers rxkad,
     rxkad-k5, rxkad-kdf, and the ACL and mode-bit rule. The route also
     confirms or corrects the position of O23.4.
   - `aklog` in the CLI with the platform's Kerberos library; token renewal
     and reacquisition on restart; `afs diagnose tokens` (O12); audit
     events (O9); security floor keys (N3, N8).
   - The cross-user and revocation acceptance cases on Linux (O48).
   - The rxgk preparation of point 9.
4. **Linux writes and locks (M3).** The journal, stores, creates, renames,
   deletes, removing open files, ACL and mount-point commands, the lock
   service, quotas at store, `afs pending`, and upgrade with pending writes
   (O32, O33).
5. **macOS (M4).** First the macOS gate (L13). Then per-user FSKit mounts
   (read, then write if the durability gate passes), the logging shim
   (O8), managed preferences (O17) and the `.pkg` (O37).
6. **Windows (M5).** First the rest of the Windows gate (L22, L41), with the
   prototype front end. Then the SMB surface, the Event Log provider (O8),
   the policy templates (O17), the MSI (O37) and the session agent (O36).
7. **rxgk and other security work (M6).**
8. **Proposed later work (M7):** see O51.
9. **rxgk timing.**
   - In the rxkad phase: pin the public base and AFS-integration drafts
     `[DRV-085, DRV-086]`; record the wire profile and its open decisions;
     verify the specification route under ADR 0001; define the Kerberos
     encryption-type operations in the client's cryptography boundary;
     check the GSS pseudo-random function rxgk needs `[DES-060]` and its
     exact output on each platform.
   - rxgk implementation starts when one of these holds: a released
     OpenAFS offers client-to-fileserver rxgk; a third-party peer and wire
     profile are agreed; or an IronAFS server exists (issue #22).
   - Until then rxgk work, and what waits on it (the shared cache pool,
     FIPS mode, protocol extensions and learned pins: C21, N18, N22, N9),
     is tested against the simulator and independent peers.
   - "Interoperable" is said only of a profile tested live against an
     independent implementation.
10. Rx's own code, with rxkad against OpenAFS 1.8, comes before everything
    that depends on it. The riskiest unknowns (the specification route for
    rxkad, the Windows identity path) are scheduled first.

Why: the core and the Linux layer are the test bed for everything else, so
they come first. rxgk waits because no released OpenAFS offers it to
clients `[DES-048, DRV-M25]`.

### O51. Proposed later work

**Working.** Revisit if: a site asks for one of these earlier, or the phase
before it shows it is not worth building.

Proposed, and scheduled only when their issues are approved:

1. the Windows offline companion (the Cloud Files replica, L27, resting on
   A6 and A7);
2. the opt-in read-only stale mode (O34);
3. OTLP metrics export (O11);
4. `/afs` surviving a daemon restart on Linux (L7);
5. per-context encryption at rest backed by an OS keystore (O27);
6. an in-memory token handoff across planned restarts (C42).

Why: each is useful and none is needed for a correct client. Listing them
keeps them out of the earlier phases.

## Consequences

1. **More is built before the first file is read.** The event catalogue,
   the journald sink, `afs status`, `afs diagnose` and the simulator are
   part of the first phase. That is deliberate: every later phase is
   debugged with them.
2. **What users of an existing AFS client will notice.**
   - An expired token gives a permission error and a notification, not an
     I/O error (O35).
   - Without validation the client refuses to serve cached data. There is
     no automatic offline mode (O34).
   - Legacy remote debugging tools get no answer from an IronAFS client
     (O15).
   - There is no PAM module (O36).
   - The compatibility commands are a separate, opt-in package (O36).
   - Logs carry references, not paths, unless the site's policy raises the
     level (O6).
   - No core dump is produced by the daemon by default (O25).
   - `fallocate` is refused (O40).
3. **What administrators get.** Native logs with stable ids on every OS,
   an audit stream whose gaps are visible, metrics without an open port,
   central policy that an older client cannot silently skip, and a
   one-sentence statement that nothing leaves the machine.
4. **Costs.**
   - The generated outputs and the compatibility snapshot (O1, O2) are
     build tooling the project must maintain.
   - The firm event rule (O2) means a badly designed event is permanent.
     Each stable event is reviewed before its first public candidate.
   - Disabling dumps (O25) makes crashes harder to debug in the field. The
     panic event, the in-memory event ring and the simulator's seed are the
     substitutes.
   - No update check (O20) means security fixes reach sites only if
     administrators follow advisories (O30).
5. **What is ruled out:** telemetry of any kind; a network listener by
   default; a remote debug service; persisted tokens; a client-side log
   store; third-party parsing, logging and test-generation crates outside
   rule 21's exceptions.
6. **What the project needs and does not yet have:** an Apple Developer ID
   and the FSKit entitlement; a Windows code-signing certificate;
   multi-user test machines for the tier 1 rows (L37); domain-joined
   and Entra-joined Windows test environments; a test cell with a KDC;
   self-hosted runners for the macOS and Windows targets (O49); a Private
   Enterprise Number (O38).
7. **Later documents this part requires**, each tracked as an issue in its
   phase's milestone:
   - the ADR for the OS-bindings crate (dump controls, the logging shim,
     clocks);
   - the event catalogue and its specification;
   - the specification for the callback allowlist (O15);
   - `docs/support.md` and the administrator's guide;
   - the identifier inventory (O38).
8. **Milestones.** The GitHub milestones are renamed to match O50 when the
   plan for the phases after M1 is written
   ([docs/workflow.md](../workflow.md) §6 is updated with them).

## Alternatives considered

- **Free-form log lines in a common text format.** Simpler to write. Not
  taken: collectors would key on message text, which changes with every
  release, and nothing would stop a secret being formatted into a line
  (O5).
- **Each component logs for itself, always.** Not taken as the main path:
  redaction and rate limits would exist in several places and drift apart,
  and a component could claim any identity in its own records. It is kept
  as the bounded fallback (O4).
- **A logging or tracing crate, and a general TOML crate.** Not taken: rule
  21. The journald protocol needs no library `[DES-043]`, and the
  configuration needs only a strict subset.
- **Audit records from the component nearest the user action.** Not taken:
  a compromised adapter could then forge or suppress audit. Only the
  daemon, which performs the operation, can attest it (O9).
- **A metrics endpoint on by default.** Not taken: it is an open port on
  every machine in a fleet, for data a file already delivers `[DES-044]`.
- **Answering legacy remote debug queries.** It would keep existing
  administrator tools working. Not taken: it discloses client state to
  anyone who can reach the callback port, without authentication (O15).
- **Excluding secret buffers from dumps and leaving dumps on.** Not taken
  as the default: exclusion cannot be verified for every copy of a secret
  on every OS, and a piped dump ignores the limit `[DES-039]`. It remains
  an extra layer under an administrator's opt-in (O25).
- **The client encrypts its own cache.** Not taken now: the key would sit
  beside the data, readable by the same administrator the threat model
  already excludes. Left as proposed later work (O51).
- **An update check that can be switched off.** Not taken: "off by
  default" is still code that can contact the project, and sites could not
  rely on the one-sentence statement (O20).
- **Promising the clean cache's format across versions.** Not taken: it
  would freeze a format that can always be rebuilt from the server, and
  slow every cache improvement. The promise is kept for what cannot be
  rebuilt (O32).
- **Draining every pending write before any upgrade.** Not taken as the
  only path: a write that cannot be stored (no token, a conflict, a full
  quota) would block upgrades indefinitely. The preflight path lets a
  compatible target carry them over (O33).
- **Serving cached data when the cell is unreachable, by default.** It is
  what users of a laptop expect. Not taken: it would present unvalidated
  data as current. The explicit stale mode is the bounded form (O34).
- **A PAM module.** Not taken: it must be written in C or with `unsafe`,
  and runs inside the login path of every user. `pam_exec` serves the
  sites that need it (O36).
- **Testing against real servers only.** Not taken: timing faults cannot
  be reproduced on demand, and a failure cannot be replayed from a seed
  (O45). Real servers remain the check on the models (O48).
- **A general property-testing crate.** Not taken: rule 21. The fuzzing
  engine's binding is the single planned test-only exception (O47).
- **Self-hosted runners for pull requests.** Not taken: a pull request's
  code would run on machines the project must keep trustworthy (O49).
- **rxgk before the OS layers.** Not taken: no released OpenAFS offers it
  to clients `[DES-048]`, so it could be tested only against the project's
  own models, while existing cells need rxkad to be usable at all.
- **A FIPS mode on rxkad.** Not possible: rxkad's cipher is not among the
  approved block ciphers (O31), so the mode waits for rxgk.
