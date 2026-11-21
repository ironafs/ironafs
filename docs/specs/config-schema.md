<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Configuration schema: M2a's keys, layers and unknown-key rules

Status: draft (2026-11-21). Raised in issue #93, part (b).

Kind: behaviour (the meaning of parsed configuration). Class: S. Issue
#93 classes the schema and its diagnostics P, a public contract; this page
also decides who may set a key and which values refuse a cell or a whole
policy, which the same issue classes S (the fail-closed refusals and the
per-cell protocol identifier), and a specification that mixes parts takes
the class of its highest part (the specifications README, "Review class
per kind").

Sections keep their numbers once the specification is reviewed. A section
that does not apply stays, with one line saying why.

## 1. What this implements

- **ADR 0004, O17** (Working), points 1 and 2:
  > 1. Layers, lowest first: built-in defaults; files with drop-ins; OS
  >    policy; and expiring run-time overrides, limited to debug levels
  >    and captures.
  > 2. A value set by policy locks its key. No lower layer and no run-time
  >    override changes it.

  Point 3: "`afs config show --effective --origin` names where each value
  came from." Point 4, Linux: "policy files under a root-owned policy
  directory, `/etc/ironafs/policy.d/`, separate from the configuration
  files of O18 and validated by O19's rules".
- **ADR 0004, O18** (Working), points 1, 3, 4 and 5:
  > 1. The format is a strict subset of TOML, parsed by our own code (rule
  >    21): `ironafs.toml` plus `ironafs.d/*.toml` drop-ins.
  >
  > 3. ... The order is: IronAFS configuration, then the classic files,
  >    then DNS.
  > 4. `afs config check` rejects unknown keys in local files, with a
  >    suggestion.
  > 5. `afs config reload` (and SIGHUP on Unix) validates first and keeps
  >    the last good configuration on error. A setting that needs a
  >    restart says so.
- **ADR 0004, O19** (Working, except points 1, 3, 5 (the directive's
  meaning), 6 and 7, which are Firm):
  > 1. OS policy carries a small, stable envelope that every client
  >    version reads: `min_client_version`, a `required` list of keys, and
  >    the mode directive of point 5.
  > 2. The daemon ignores and reports unknown policy keys, so a newer
  >    policy works with an older client.
  > 3. The exceptions are a key named in `required` that the client does
  >    not know, and a `min_client_version` newer than the client. Either
  >    makes the policy **incompatible** ...
  >
  > 6. ... An unknown value of a known security key (N8, point 2) refuses
  >    the cells it applies to, and only those. It is the one value
  >    failure that does not make the whole policy incompatible.
  > 7. The daemon validates the whole policy, envelope included, as one
  >    unit. A readable policy that fails schema validation, or a value
  >    validation other than point 6's, or has no value for a key named in
  >    `required`, is **incompatible**, as in point 3. It never falls back
  >    to an older copy.
- **ADR 0004, N8, point 2** (Firm): "An unknown **value** of a known
  security key refuses that cell. It is never ignored."
- **ADR 0004, N3** (Firm: the key names, their values, and points 1 to
  5; Working: the defaults): the floor keys `min_class`, `min_level`,
  `legacy_fileservers`, `anonymous`, `learned` and `rxgk`, "Per cell, with
  machine-wide defaults". Point 1: "OS policy can lock these keys like any
  other (O17)." N8, point 1, makes them exist "from the first release that
  authenticates (M2b in the plan)".
- **ADR 0004, N27** (Firm), points 1, 2 and 5: "Each cell has a
  provisioned protocol identifier. It is `afs3` by default." "An
  unsupported value is refused before any network I/O." "The identifier
  is never learned from the network."
- **ADR 0004, C33** (Working), points 1, 4, 5 and 8: configuration first,
  then DNS SRV, then AFSDB; the flat-list confirmation; explicit servers;
  "Discovery can be turned off globally and per cell."
- **ADR 0004, C34** (Working), point 1: "Every address inside the core is
  an address of either family ... That covers configuration". Point 2 as
  issue #53, point 4, amends it (owner's decision, 2026-11-20; the
  amendment is not on `main` yet):
  > Single-family operation, with one family's socket only, is entered
  > only by the administrator's choice in configuration, never
  > automatically, and startup diagnostics and `afs status` show it.
- **ADR 0004, C12, point 1** (Working): "`/afs` is a dynamic root. It
  lists configured and discovered cells without contacting them."
- **ADR 0004, L34, point 4** (Working): "`@sys` and `@cell` substitution
  in the core, with an ordered, configurable system-name list."
- **ADR 0004, C27** (Working), points 4 and 8: per-account budgets; "Chunk
  size ..., pool sizes and eviction constants are left to the builder and
  to measurement."
- **Issue #93** (the brief, owner's decisions of 2026-11-20): the keys for
  "cells, cache size, protocol identifier", the flat-list confirmation and
  the discovery switches "set only by a root-owned file or by policy,
  never through the control endpoint", the address-family mode "dual by
  default; a single family only when the administrator selects it", and
  "`min_class` exists as a reserved name whose every value is unknown to
  this release".
- **The configuration syntax** ([config-syntax.md](config-syntax.md)),
  §2, point 2, which leaves to this page "which keys exist, their types,
  defaults and who may set them; the unknown-key rules of O18, point 4
  and O19, points 2, 3 and 6" and "the order of `ironafs.toml`, the
  drop-ins and OS policy, and how a later layer may change an earlier one
  (O17)".

## 2. Scope

1. **Covered:** the keys M2a defines, their types, ranges, defaults and
   layers (5.2, 5.3); what a key that this release does not know, or a
   value it does not know, does in each layer (5.4); how one file is
   validated and what refuses it (5.5); how the layers combine into the
   effective configuration, with origins (5.6); how the schema grows
   (5.7); the diagnostics (5.9); and the named interfaces through which
   parts (c) to (f) attach (5.8).
2. **Not covered, and where it is:**
   - the bytes of a file and the cell-name rule:
     [config-syntax.md](config-syntax.md);
   - the policy envelope's validation, the four policy states, whether a
     policy file that fails to parse is unreadable or incompatible, and
     how several policy files form one policy: part (c). This page only
     names the envelope keys and their types (5.2.7) and the interface
     (5.8);
   - the classic `CellServDB` and `ThisCell` grammar: part (d). This page
     fixes only where their contribution sits in the order (5.6, point 7);
   - which reload outcome applies to which failure, and what the daemon
     does with it: part (e) and M2a-73 (planned). This page supplies the
     refusals (section 7) that part (e) maps;
   - the last good policy record: part (f);
   - what each DNS key means: the DNS client specification (#60), 5.6.4,
     which proposed the names used in 5.2.4;
   - the default system-name list and the per-name rules of the list: the
     namespace specification (#80), 5.6.5, points 8 and 10;
   - the security floor keys' values and meaning: M2b-17 (planned). M2a
     reserves their names (5.4.5);
   - finding the files, reading them, their order on disk and their
     ownership: the caller. It hands this crate each source's bytes, name
     and trust (5.1, point 3);
   - run-time overrides (debug levels and captures, O17, point 1): the
     events specification and the control endpoint (#70). No key of this
     schema is a run-time override.

## 3. The design in short

1. The schema is a fixed table in `ironafs-config`: each known key has a
   path, a type, a range, a default, the layers that may set it, and
   whether a change applies on reload or needs a restart.
2. A local file is validated as a whole. Any unknown key, wrong type,
   out-of-range value, or key its source may not set refuses the whole
   file, with the position and, for an unknown key, a suggestion. The one
   exception is an unknown value of a security key, which refuses the
   cells it applies to (5.4, point 4).
3. A policy is validated as a whole. An unknown key is ignored and
   reported; every other failure makes the policy incompatible, except an
   unknown value of a security key, which refuses only its cells.
4. The layers combine key by key: defaults, then the local files, then
   policy. A key set by policy is locked. Within the local files a key is
   set at most once.
5. Cell tables are keyed by the canonical cell name, so two spellings of
   one cell are one cell.
6. Configuration files carry no version. The schema grows only by adding
   keys and values; a key is never given a second meaning. A policy that
   needs a newer client says so with `min_client_version` or `required`.

## 4. Terms

- **Layer:** one of O17, point 1's four: defaults (D), local files (F),
  OS policy (P), run-time overrides (R).
- **Source:** one file's bytes as the caller hands them over, with its
  layer, a name for diagnostics and its trust (5.1, point 3).
- **Schema path:** a key's path with each cell name replaced by `*`, for
  example `cells.*.protocol`. Every known key has one.
- **Known key:** a schema path in 5.2's tables. Any other path is
  **unknown**.
- **Security key:** a known key whose unknown value refuses cells, never
  the whole file or policy (N8, point 2; O19, point 6). In M2a these are
  the reserved floor keys of 5.4.5.
- **Reserved key:** a known security key for which this release knows no
  value, so every value is unknown (issue #93, "`min_class` exists as a
  reserved name").
- **Root-only key:** a key that a local file may set only if its source
  is trusted (5.1, point 3).
- **Effective configuration:** the value of every known key after 5.6,
  each with its origin.
- **Origin:** where a value came from: the default, a named local file and
  the key's position in it, or a named policy source.
- **Refusal:** a validation result of section 7: a file refused, a policy
  incompatible, or a cell refused.

## 5. Specification

### 5.1 Layers and sources

1. **The layers,** lowest first, are D, F, P and R
   `[ADR 0004 O17.1: Working]`. F holds `ironafs.toml` and every
   `ironafs.d/*.toml` `[ADR 0004 O18.1: Working]`; on Linux, P holds the
   files under `/etc/ironafs/policy.d/` `[ADR 0004 O17.4: Working]`. No
   key in 5.2 may be set in R `[ADR 0004 O17.1: Working]`.
2. **This crate parses bytes.** The caller finds the files, reads each
   within `MAX_FILE_BYTES` (config-syntax.md, 5.2), puts F's files in
   their order, and hands over one source per file
   ([config-syntax.md](config-syntax.md), §2) `[OWN]`.
3. **Trust.** Each source carries a trust the caller decides: `trusted`
   when the file, and every directory on its path, is owned by uid 0 and
   writable by no other user or group; `untrusted` otherwise `[OWN]`, from
   issue #93 ("set only by a root-owned file or by policy") and the DNS
   client specification (#60), 5.6.4, point 6, which states the same
   ownership test. A P source is always handed over as trusted: the policy
   directory is root-owned by O17, point 4, and a caller that finds it
   otherwise reports the policy unreadable (part (c)) rather than handing
   it over. Whether the daemon loads an untrusted F source at all is
   section 13, Q2.
4. **What a source may set:** each key's "Layers" column in 5.2. A key
   marked `F*` may be set in F only from a trusted source.

### 5.2 Keys

Columns: **Path** (as written in a file; `<cell>` is a quoted cell name);
**Type** (5.3); **Range**; **Default** (the D layer's value; "none"
means the key has no value unless set); **Layers** that may set it;
**Applies** on `reload` or needs a `restart` `[ADR 0004 O18.5: Working]`;
**Source** of the rule. No key is required in any layer: a host with no
file and no policy runs on defaults `[ADR 0004 O19.4: Working]`
("Unmanaged ... runs on defaults and local files").

The table names below are this page's proposal `[OWN]` (Q1): one table
per concern, so a drop-in can hold one concern, and the cell-scoped keys
under one `cells` table, so a cell's keys are found together.

#### 5.2.1 The home cell

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `home_cell` | cell name | 5.3.1 | none | F, P | reload | L34, point 4 (`@cell`); namespace specification (#80), 5.6.5, point 7 |

1. With no value here, the home cell is the classic `ThisCell`'s, if part
   (d) supplies one (5.6, point 7) `[ADR 0004 O18.3: Working]`.

#### 5.2.2 Cells

One table per cell: `[cells."<cell>"]`. The quoted segment is a cell
name (5.3.1).

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `cells.<cell>.protocol` | enumeration | `afs3` | `afs3` | F, P | reload, as a generation change (point 3) | N27, points 1 and 2 |
| `cells.<cell>.servers` | list of server addresses | 0 to `config.cell_servers_max` entries, no two equal (5.3.3) | empty | F, P | reload | C33, point 1 |
| `cells.<cell>.discovery` | boolean | | `true` | F*, P | reload | C33, point 8; DNS client specification (#60), 5.6.4, point 2 |
| `cells.<cell>.dns_scope` | enumeration | `host`, `servers` | `host` | F*, P | reload | C33, point 5; #60, 5.6.4, point 5 |
| `cells.<cell>.dns_servers` | list of server addresses | 0 to `dns.explicit_servers_max` (8) entries | empty | F*, P | reload | as above |
| `cells.<cell>.dns_interface` | interface name | 1 to 15 bytes of `a` to `z`, `A` to `Z`, `0` to `9`, `.`, `_`, `-` (Q6) | none | F*, P | reload | as above |
| `cells.<cell>.min_class` | security key | reserved (5.4.5) | none | F, P | reload | N3; issue #93 |

1. **A cell table makes the cell configured** (C12, point 1), with or
   without keys `[OWN]`.
2. **Servers.** `cells.<cell>.servers` holds the cell's database servers.
   A non-empty list is used, and DNS is not asked for that cell
   `[ADR 0004 C33.1: Working]` ("Configuration always wins"). An empty or
   absent list leaves the cell to the classic files, then DNS
   `[ADR 0004 O18.3: Working]`.
3. **Protocol.** `afs3` is the only value this release supports, and any
   other is refused before any network I/O `[ADR 0004 N27.2: Firm]`: in F
   it refuses the file, in P it makes the policy incompatible (section 7)
   `[ADR 0004 O19.7: Firm]`; it is not a security key (N8, point 1, does
   not list it). A change of the effective value is a change of the
   cell's protocol identifier, which moves its configuration generation
   (N27, point 6); this page reports the change, and the generation record
   (M2a-58, planned) acts on it.
4. **No key in `cells` comes from the network.** Discovery and the
   classic files add servers for a cell; they never set a key of this
   table `[ADR 0004 N27.5: Firm]` for the protocol identifier, `[OWN]`
   for the rest.

#### 5.2.3 Network

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `network.families` | enumeration | `dual`, `ipv4`, `ipv6` | `dual` | F, P | restart | C34, point 2, as #53, point 4, amends it; issue #93 (owner's decision, 2026-11-20) |

1. Only an explicit value other than `dual` selects single-family
   operation; nothing else ever does (C34, point 2, as amended).
2. It needs a restart: it decides which sockets the daemon binds at start
   `[OWN]`. A reload that changes it reports "restart needed" and keeps
   the running value (O18, point 5).

#### 5.2.4 Discovery

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `discovery.enabled` | boolean | | `true` | F*, P | reload | C33, point 8; #60, 5.6.4, point 1 |
| `discovery.flat_list_confirmed` | list of addresses (no port) | 0 to `dns.name_servers_max` (3) entries, order kept | empty | F*, P | reload | C33, point 4; #60, 5.6.4, point 3 |
| `discovery.servers` | list of server addresses | 0 to `dns.explicit_servers_max` (8) entries, order kept | empty | F*, P | reload | C33, point 5; #60, 5.6.4, point 4 |

1. Every key in this table and the three `dns_*` keys and `discovery` of
   5.2.2 are root-only: settable only by a trusted F source or by policy,
   never by R and never through the control endpoint (issue #93; #60,
   5.6.4, point 6).
2. Their meaning, and what a change does to the DNS client's view, are
   the DNS client specification's (#60, 5.6.4, points 3 and 7).

#### 5.2.5 Cache

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `cache.size_max` | byte count | 5.3.5 | Q4 | F, P | reload, as a lowered or raised soft cap (point 1) | issue #93 ("cache size"); C27, points 4 and 8 |

1. It is the total of resident clean bytes the cache may hold, across all
   accounts and the pool. The per-account floors and caps of C27, point
   4, are derived from it by the cache, which C27, point 8, leaves to the
   builder and to measurement `[OWN]`. A lower value applies by eviction,
   never by refusing a reload `[OWN]`.

#### 5.2.6 Namespace

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `namespace.sysname` | list of strings | 0 to `ns.sysname.count_max` (16) entries, each 1 to `ns.sysname.len_max` (64) bytes, under the namespace specification's per-name rules | the built-in list per target (#80, 5.6.5, point 10) | F, P | reload | L34, point 4; #80, 5.6.5, points 8 and 9 |

1. A value, even an empty list, replaces the default list entirely (#80,
   5.6.5, point 8). A list that breaks a per-name rule refuses its source
   (section 7).

#### 5.2.7 The policy envelope

Valid only in P. In F each is an unknown key (5.4, point 1) `[OWN]`: an
envelope in a local file would let a local edit pose as policy, which O17,
point 4, separates.

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `policy.min_client_version` | version | 5.3.6 | none (no minimum) | P | reload | O19, points 1 and 3 |
| `policy.required` | list of schema paths | 0 to 256 entries, no two equal | empty | P | reload | O19, points 1, 3 and 7 |
| `policy.mode` | enumeration | `unmanaged` | none (managed) | P | reload | O19, points 1 and 5 |

1. These names, types and the table they sit in are part of the envelope
   every later client reads, so they never change once released
   `[ADR 0004 O19.1: Firm]`. Their placement in a `policy` table is this
   page's proposal (Q1).
2. What each means, the order in which they are checked, and how they set
   the policy state: part (c).

#### 5.2.8 Bounds this page sets

| Name | Value | Why |
|---|---|---|
| `config.cell_servers_max` | 16 | the same bound as `dns.targets_max` (#60, 5.2), so a configured cell and a discovered one carry at most the same number of servers; within config-syntax.md's `MAX_ARRAY_ELEMENTS` `[OWN]` |
| `config.required_max` | 256 | config-syntax.md's `MAX_ARRAY_ELEMENTS` `[OWN]` |
| `config.unknown_reported_max` | 64 | the unknown policy keys named one by one in a report; the rest are counted (5.9, point 3) `[OWN]` |

### 5.3 Value types

Each type names the syntax value it takes (config-syntax.md, 5.1) and what
it refuses. A value of another syntax type is a type error (section 7).

1. **Cell name:** a string, passed to the one cell-name constructor
   (config-syntax.md, 5.7); what it refuses, the key refuses. A cell name
   that is a table segment (`cells."<cell>"`) passes it too. A name of
   one label is accepted here; a use that needs more is that use's rule
   (config-syntax.md, 5.7).
2. **Enumeration:** a string equal, byte for byte, to one of the listed
   values. Case matters `[OWN]` (config-syntax.md, 5.1, point 8, compares
   keys the same way).
3. **Address** and **server address:** a string. An address is an IPv4
   address in dotted decimal (four decimal parts 0 to 255, no leading
   zero) or an IPv6 address in the text forms of RFC 4291, section 2.2,
   without a zone index; a server address adds an optional port, written
   `192.0.2.1:7003` or `[2001:db8::1]:7003`, 1 to 65535. Without a port,
   the service's registered port is used, as the location service and DNS
   specifications state `[OWN]`. A host name is refused (Q5). Two entries
   equal as addresses and ports (after parsing, so `::1` and
   `0:0:0:0:0:0:0:1` are equal) refuse the list `[OWN]`. Either family is
   accepted whatever `network.families` says
   `[ADR 0004 C34.1: Working]`; using an address of a family that is off
   is the user's rule. RFC 4291 is not yet in the register of editions;
   it is registered before this page is reviewed.
4. **Interface name:** a string; its range is in 5.2.2.
5. **Byte count:** an integer, in bytes, from 64 MiB (67,108,864) to
   2^62 `[OWN]`: the floor keeps the cache above a few dozen chunks of C27,
   point 8's proposed 1 MiB; the ceiling leaves room for sums of two
   counts in 64 bits (section 6). Unit suffixes are not offered: the
   syntax has no unit, and `_` separators (`1_073_741_824`) keep a large
   value readable.
6. **Version:** a string `MAJOR.MINOR.PATCH`, each part a decimal
   integer from 0 to 2^32 − 1 without a leading zero, compared part by
   part `[OWN]`. Pre-release and build suffixes are refused (Q7).
7. **Schema path:** a string of the form of a schema path (section 4),
   segments separated by `.`, `*` standing for a cell name. Whether a path
   is known is checked against 5.2; an unknown path is not a type error
   here, and part (c) decides what it does (O19, point 3).
8. **List of T:** an array whose elements are all T (the syntax already
   refuses mixed arrays), within the stated count.

### 5.4 Unknown keys and unknown values

1. **F: an unknown key refuses the file**, with the key's position and a
   suggestion (5.9, point 2) `[ADR 0004 O18.4: Working]`. This holds
   for the daemon's load and reload as for `afs config check` `[OWN]`:
   issue #93 names "An unknown key in a local file ignored" as a failure
   case, and O18's "Why" says "a typo in a security setting must fail
   loudly". An unknown table counts as unknown keys.
2. **P: an unknown key is ignored and reported**
   `[ADR 0004 O19.2: Working]`. It does not take part in validation: its
   value is not checked and not stored. The report names its path and its
   position, never its value `[OWN]` (a value may be sensitive, O6).
3. **P: the exceptions** are a key named in `policy.required` that this
   release does not know, and a `policy.min_client_version` newer than
   this release: either makes the policy incompatible
   `[ADR 0004 O19.3: Firm]`. Part (c) implements them on 5.8's
   interface.
4. **An unknown value of a security key refuses the cells it applies to,
   and only those,** in either layer `[ADR 0004 N8.2: Firm]`
   `[ADR 0004 O19.6: Firm]`. In F the rest of the file applies; the file
   is not refused for it, and its cells do not fall back to the last good
   configuration, because falling back would ignore the value N8, point
   2, says is "never ignored" `[OWN]` (Q3). A value of the wrong syntax
   type (an integer for `min_class`) is a type error, not an unknown
   value: it refuses the file in F and makes the policy incompatible in P
   `[OWN]` (Q8).
5. **Reserved security keys in M2a.** `cells.<cell>.min_class` is a
   reserved key: every string value is unknown, so it refuses its cell
   (issue #93). Whether the other five floor keys of N3, the `fips` key
   (O31, point 4) and a machine-wide default for each are reserved the
   same way is Q9.
6. **Known key, wrong layer.** An envelope key in F is unknown (5.2.7). A
   root-only key in an untrusted F source refuses that file (section 7),
   with a hint naming the trust it lacks `[OWN]`.

### 5.5 Validating one source

1. A source is parsed (config-syntax.md); a parse error is that source's
   refusal.
2. The document's entries are checked in file order: path known (5.4),
   layer and trust allowed (5.1, point 4), type and range (5.3). Every cell
   segment passes the cell-name constructor.
3. **Two spellings of one cell in one source refuse it:** two cell tables
   whose segments construct equal cell names (for example
   `cells."Example.org"` and `cells."example.org."`) are a redefinition,
   though the syntax compares segments byte for byte
   (config-syntax.md, 5.1, point 8), and the error names both positions
   `[OWN]`, from config-syntax.md, 5.7, point 6.
4. **The first refusal of F stops** the source, as the parser does
   (config-syntax.md, 7, point 2) `[OWN]`. Unknown values of security keys
   (5.4, point 4) do not stop it; each is collected as a cell refusal.
5. **P collects:** in a policy source, unknown keys are collected for the
   report and validation continues; the first other failure makes the
   policy incompatible (section 7).
6. **Nothing applies before the whole configuration validates.** The
   result of validating all F sources and the policy is one value, built
   before any part of it is used `[ADR 0004 O18.5: Working]`
   `[ADR 0004 O19.7: Firm]` (issue #93's failure case "part of a new
   configuration applied before the whole of it validated").

### 5.6 Layering

1. **Per key.** The effective value of each known key is its value in the
   highest layer that sets it, D at the bottom `[ADR 0004 O17.1:
   Working]`. Cell-scoped keys combine per cell and key: a cell's table
   in P and the same cell's table in F are one cell, and each of its keys
   is taken from its highest layer `[OWN]`.
2. **Policy locks** every key it sets. No F source and no R override
   changes it `[ADR 0004 O17.2: Working]`. An F value for a locked key is
   not an error; it is recorded as overridden, and `afs config show
   --origin` shows both `[OWN]`, so an administrator sees that a local
   file did not override the site's policy (O17, "Why").
3. **Within F, a key is set at most once.** The same key in two F sources
   (for example in `ironafs.toml` and a drop-in) refuses the local
   configuration, naming both sources and positions `[OWN]` (Q10): no
   source silently replaces another's value (issue #93's failure case "a
   later layer silently weakening an earlier one").
4. **Lists are values.** A list key set in a higher layer replaces the
   lower list; lists are never concatenated across sources `[OWN]`.
5. **Policy is one unit.** How several P sources form one policy, and
   what the same key in two of them does: part (c). This page sees the
   policy only as one validated set of keys (5.8, point 2).
6. **Origins.** Every effective value carries its origin (section 4);
   `afs config show --effective --origin` prints them
   `[ADR 0004 O17.3: Working]`.
7. **The classic files are not a layer.** They supply cells and their
   servers, and a home cell, consulted after the effective configuration
   and before DNS `[ADR 0004 O18.3: Working]`: a cell's servers from
   `cells.<cell>.servers` win over the classic files, and the classic
   files win over DNS; `home_cell` wins over `ThisCell`. They set no key
   of 5.2 and never override one `[OWN]`. Part (d) defines their grammar
   and contribution (5.8, point 3).

### 5.7 Versions of the schema

1. **The files carry no version** (config-syntax.md, 5.3). The schema's
   version is the client's version, which `policy.min_client_version`
   compares against `[ADR 0004 O19.1: Firm]`.
2. **The schema grows only by addition** `[OWN]`, from O19's "Why": a
   later release may add a key, a table or a value of an enumeration.
   It never gives a released path, or a released value, a second meaning,
   never narrows a released range, and never makes a key required in F.
3. **A removed key is retired, not forgotten** `[OWN]`: it stays in the
   schema, marked retired; in F it refuses the file with a hint naming its
   replacement, and in P it is ignored and reported as retired.
4. **A changed default** is allowed only for a Working default, and the
   release notes name it `[OWN]`, from N3's split between Firm keys and
   Working defaults.
5. **The envelope** (5.2.7) never changes `[ADR 0004 O19.1: Firm]`.
6. **A newer policy on this client** is handled by 5.4, points 2 to 4: a
   key it adds is ignored and reported, unless `required` names it; a
   value it adds to a security key refuses its cells; a value it adds to
   any other enumeration makes the policy incompatible
   `[ADR 0004 O19.7: Firm]`. A site that adds such a value states
   `min_client_version` (N27, point 4, says the same for the protocol
   identifier).

### 5.8 Interfaces for parts (c) to (f)

These are names and shapes only. What each part decides is its own.

1. **Validated layers.** Part (b) provides `validate_local(sources) ->
   Result<LocalLayer, LocalRefusal>` for F, and
   `validate_policy_keys(sources) -> PolicyKeys` for P, where `PolicyKeys`
   holds the known keys with their values, the envelope's raw values, the
   unknown keys for the report, the cell refusals, and the first other
   failure if any. Neither reads a file or a clock.
2. **The envelope and the policy state** (part (c)): part (c) takes a
   `PolicyKeys` (or the caller's "no policy source" or "unreadable") and
   returns the policy state of O19, point 4, as a typed value
   `PolicyState { Unmanaged, Valid(PolicyLayer), Unreadable,
   Incompatible(reason) }`. This page gives it `is_known(schema_path)`,
   `has_value(schema_path)` and the client's own version, for O19, points
   3 and 7.
3. **The classic files** (part (d)): part (d) returns a
   `ClassicContribution` (cells with their servers, and a home cell), each
   cell name built by the one constructor. The resolution order of 5.6,
   point 7, consumes it; it never enters `merge`.
4. **Merging:** `merge(LocalLayer, Option<PolicyLayer>) -> Effective`,
   with per-key origins and the lists of locked-and-overridden keys and
   refused cells.
5. **Reload** (part (e)): part (e) compares the running `Effective` with a
   new validation's result, and returns the typed decision the daemon
   applies (issue #93, "Reload"). This page gives it the refusal kinds of
   section 7 and, per key, whether a change applies on reload or needs a
   restart (5.2's "Applies" column).
6. **The last good policy** (part (f)): part (f) stores and loads a
   `PolicyLayer`, the policy as validated. Its encoding, checksum and
   atomic write are part (f)'s format.

### 5.9 Diagnostics

1. Every refusal names the source, the position of the offending key or
   value (config-syntax.md, 7, point 1), what was expected, and a hint
   `[ADR 0004 O18.2: Working]`.
2. **The suggestion** for an unknown key in F: the known paths in the
   same table, then known paths anywhere, whose Levenshtein distance from
   the unknown key is at most 2, closest first, ties in 5.2's order, at
   most three `[OWN]`. Distances are computed only between strings of at
   most `MAX_KEY_BYTES` (config-syntax.md, 5.2), so the work is bounded. No
   candidate within 2 gives no suggestion, never a guess.
3. **The unknown-policy-key report** names up to
   `config.unknown_reported_max` paths with positions, then a count of
   the rest `[OWN]`. It is an event, raised once per policy load or
   reload (the event's catalogue entry: the events specification).
4. **Cell refusals** name the cell, the key and the source of the value,
   never the value when it comes from policy `[OWN]`.

## 6. Arithmetic

| Value | Bound | Method |
|---|---|---|
| byte count | 64 MiB to 2^62 | compared after the syntax's checked parse; at most 2^62 so a sum of two stays below 2^63 |
| port | 1 to 65535 | parsed digit by digit with `checked_mul` and `checked_add` into `u16`; overflow refuses |
| version parts | 0 to 2^32 − 1 each | parsed into `u32` with checked arithmetic; compared as a triple |
| suggestion distance | 2 | one row of at most `MAX_KEY_BYTES` + 1 counters; no sum exceeds 2 × `MAX_KEY_BYTES` |
| list counts | 5.2's ranges | compared before the list is stored |

## 7. Errors and failure

| Refusal | Raised for | Effect |
|---|---|---|
| **Local refusal** (`LocalRefusal`) | in F: a parse error; an unknown key or table; a type or range error; a cell name the constructor refuses; two spellings of one cell; an envelope key; a root-only key in an untrusted source; the same key in two sources; a retired key | no F source is used: the whole local configuration is refused, and part (e) keeps the last good one `[ADR 0004 O18.5: Working]` (start without one: Q11) |
| **Policy incompatible** | in P: a parse error (if part (c) classes it so, Q12); a type or range error; an unknown enumeration value of a key that is not a security key, `protocol` included; a cell name the constructor refuses; two spellings of one cell; and part (c)'s envelope checks. A retired key is not among them: it is ignored and reported (5.7, point 3) | the whole policy is incompatible: failed health, admissions blocked, grants fenced, never the last good copy `[ADR 0004 O19.3: Firm]` `[ADR 0004 O19.7: Firm]` |
| **Cell refused** | in F or P: an unknown value of a security key | that cell, or every cell a machine-wide value applies to, is refused with the reason; the rest serves `[ADR 0004 N8.2: Firm]` `[ADR 0004 O19.6: Firm]` |

Each fails closed: no partial file, policy or cell is used. A refused
cell is listed in the effective configuration, so status shows it.

## 8. Security

1. **Who writes each layer:** F, the administrator, and for a
   misconfigured host any user who can write a file the daemon loads
   (config-syntax.md, 8, point 1); P, the site's management system. The
   root-only keys are the ones whose misuse would send a cell's name or
   queries where the host's policy does not (C33, points 4, 5 and 8), so
   they need a trusted source (5.1, point 3; 5.4, point 6).
2. **A typo in F fails loudly** (5.4, point 1).
3. **A newer policy is never undercut silently.** An unknown policy key is
   reported, and `required` and `min_client_version` let a site make it
   fatal (O19, points 2 and 3). The residual is O19's own: a site that
   uses neither gets the report only. Q9 asks whether M2a should reserve
   the N3 keys so that a floor stated for a later client refuses cells on
   M2a instead of being reported.
4. **No lower layer weakens policy** (5.6, point 2); no local source
   silently replaces another (5.6, point 3); the classic files and the
   network never set a key (5.2.2, point 4; 5.6, point 7).
5. **One cell, one name** (5.5, point 3).
6. **Reports carry no policy values** (5.4, point 2; 5.9, point 4).

## 9. How the invariants are kept

| Invariant | Kept by | Test that fails if broken |
|---|---|---|
| O17.2: policy locks its key | 5.6, point 2 | a key in F and P: the effective value is P's, origin P, F recorded as overridden |
| O18.4: unknown key in F refused, with a suggestion | 5.4, point 1; 5.9, point 2 | a misspelt key refuses the file and suggests the known one |
| O19.2: unknown policy key ignored and reported | 5.4, point 2 | an unknown policy key leaves the policy valid and is in the report, without its value |
| O19.3: unknown `required` key, newer `min_client_version` | 5.4, point 3; 5.8, point 2 (part (c)) | part (c)'s tests |
| O19.6 / N8.2: unknown security value refuses only its cells | 5.4, point 4 | one policy, two cells, one with `min_class = "rxgk"`: that cell refused, the other serves; the same in F |
| O19.7: any other validation failure is incompatible | section 7 | an unknown `protocol` value in P makes it incompatible |
| N27.2: unsupported protocol refused before I/O | 5.2.2, point 3 | `protocol = "x"` refuses F; the validated value is only `afs3` |
| C34.2 (as amended): single family only by choice | 5.2.3 | the default is `dual`; only an explicit value changes it |
| issue #93: root-only keys | 5.1, point 3; 5.4, point 6 | each root-only key from an untrusted source refuses the file |
| one cell name | 5.3.1; 5.5, point 3 | two spellings of one cell refused |

## 10. Tests

Unit tests in `crates/ironafs-config/src/schema/tests.rs`; property tests
in `crates/ironafs-config/tests/it/`.

1. **Each key:** its default; one accepted value; each range bound
   accepted and one past it refused; the wrong syntax type refused.
2. **Unknown keys:** in F, refused with the suggestion of 5.9, point 2
   (for example `cells."x".protocl` suggests `protocol`); in P, ignored and
   reported, with more than `config.unknown_reported_max` counted.
3. **Security values:** the O19, point 6, test in P and in F, red then
   green.
4. **Layers:** the same key in two F sources refused, naming both; a P
   value locks the key; a root-only key from an untrusted source refused;
   an envelope key in F refused as unknown.
5. **Cells:** two spellings of one cell refused; a cell in F and in P
   combined per key.
6. **Property:** any document of known keys with valid values validates,
   and merging is deterministic: the same sources give the same effective
   configuration and origins.
7. **No partial use:** a refusal returns no layer (the types make a
   partial `LocalLayer` impossible to build).

## 11. Revisit if

- O17: "a supported management system cannot deliver policy through the
  mechanism named here."
- O18: "the TOML subset cannot express a setting a later phase needs, or
  administrators' tools cannot write it."
- O19: "a management system is shown to drop or reorder the envelope, or
  sites need a state this list lacks."
- C33: the triggers of C33's "Revisit if".
- C34: "assumption A11 fails, a negotiated extension (N22) supplies
  fileserver addresses other than IPv4, or support cases show that hosts
  with one family disabled need a start that point 2 does not allow" (as
  #53 amends it).
- C27: "M2a measurements show the filesystem-as-index layout or the
  eviction policy misses its targets on a supported filesystem", and the
  cache size default of Q4 then.
- The DNS client (#60) or namespace (#80) specification renames or
  changes a key this page lists.

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| ADR | DECISION | ADR 0004, C12, C27, C33, C34, L34, N3, N8, N27, O17, O18, O19 | throughout |
| AMD | DECISION | Issue #53, point 4 (C34, point 2; owner's decision, 2026-11-20), not on `main` at this draft | 5.2.3 |
| DEC-1 | DECISION | Issue #93, its owner's decisions of 2026-11-20 and its outcome | 1, 5.2, 5.4 |
| SYN | SPECIFICATION | [config-syntax.md](config-syntax.md) (draft, 2026-11-21) | throughout |
| DNS | SPECIFICATION | The DNS client specification, issue #60 (draft), 5.2, 5.6.3 and 5.6.4 | 5.2.2, 5.2.4 |
| NS | SPECIFICATION | The namespace specification, issue #80 (draft), 5.2 and 5.6.5 | 5.2.1, 5.2.6 |
| S1 | SPECIFICATION | RFC 4291, "IP Version 6 Addressing Architecture", section 2.2 (to be registered) | 5.3.3 |
| OWN | OWN | This page's own choices, each with its reason where it is made | throughout |

### 12.1 Observed behaviour

None.

## 13. Open questions

Each has a recommendation; none is decided here.

- **Q1. Table and key names.** The ADR fixes only the envelope's three
  key names and N3's floor-key names; the tables (`cells`, `network`,
  `discovery`, `cache`, `namespace`, `policy`) are this page's proposal.
  Recommendation: adopt them; the envelope's placement in a `policy`
  table is frozen with the first release (O19, point 1), so it is
  reviewed now. Alternative: envelope keys at the policy file's root,
  which no other key uses.
- **Q2. Untrusted local files.** The DNS specification (#60, 5.6.4, point
  6) lets an untrusted file contribute keys other than the root-only ones.
  Recommendation: the daemon refuses an untrusted F source as a whole
  (a local refusal), so every F source is trusted and the root-only
  marking is a second check; evidence: config-syntax.md, 8, point 1,
  names a writable configuration file as a misconfiguration, and a
  non-root-owned file under `/etc` is one. `afs config check` reports it.
  The `F*` marking stays, so a later relaxation cannot widen these keys.
- **Q3. A security value in a local file.** Recommended as 5.4, point 4:
  the cell is refused and the rest of the file applies, rather than
  refusing the file and keeping the last good configuration, which would
  keep serving the cell under its old floor; evidence: N8, point 2
  ("never ignored", Firm, not limited to policy). The issue states the
  rule only for policy; this needs the reviewers' reading of N8.
- **Q4. The cache size default.** C27, point 8, leaves sizes to the
  builder and to measurement, and no decision gives a number.
  Recommendation: a fixed default of 1 GiB `[OWN]` until M2a's
  measurements (C27's "Revisit if"), rather than a share of the disk,
  which makes behaviour depend on the host; to be confirmed by the cache
  specification's author.
- **Q5. Host names in server lists.** CellServDB-style configuration
  names addresses; resolving a configured name would send a query C33,
  point 5, may not allow for a scoped cell. Recommendation: addresses only
  in M2a; a host name is refused with a hint.
- **Q6. The interface-name rule.** Linux limits an interface name to 15
  bytes; no registered source states the character set yet.
  Recommendation: register the kernel's documented rule (or an
  administrator's manual page) and replace 5.2.2's provisional set with it
  before review.
- **Q7. The version format.** O19 names `min_client_version` without a
  format. Recommendation: `MAJOR.MINOR.PATCH` only (5.3.6), no
  pre-release suffix, because ordering pre-releases invites a policy that
  admits an untested build; the release process (O33) confirms the
  client's own version has this form.
- **Q8. A wrong type for a security key** (`min_class = 3`).
  Recommendation: a type error (refuse F, incompatible P), not an unknown
  value; evidence: O19, point 7, separates schema validation from point
  6's value failure, and N8, point 2, speaks of an unknown "value".
- **Q9. Which floor keys M2a reserves.** The issue reserves `min_class`
  only. A policy written for M2b with `anonymous = "deny"` would, on M2a,
  be an unknown key, ignored and reported, while M2a serves anonymous
  reads. Recommendation: reserve all six N3 keys per cell, and
  `fips`, so any value of them refuses its cells on M2a; and add the
  machine-wide defaults N3 names ("Per cell, with machine-wide
  defaults") under a `security` table, reserved the same way; evidence:
  O19's "Why" ("An older client that skipped a key it did not understand
  would run below that floor"). The cost: a site cannot state those keys
  in a policy that M2a clients also read, which M2a's own scope (no
  production use) makes acceptable.
- **Q10. Drop-ins that override.** 5.6, point 3, refuses the same key in
  two F sources, where many tools let a later drop-in win. Recommendation:
  keep the refusal; the defaults live in the code, not in a shipped
  `ironafs.toml`, so no drop-in has to override a packaged value, and
  issue #93 names silent weakening as a failure case.
- **Q11. A refused local configuration at start,** with no last good one
  in memory. Recommendation: refuse to start, with the reason, rather than
  run on defaults: defaults turn discovery on and could send a cell's
  name where a refused file had turned discovery off (C33, point 5). Part
  (e) decides; this page records the refusal only.
- **Q12. A policy file that does not parse.** O19, point 4, calls a policy
  that "cannot be read" unreadable (last good copy, degraded); point 7
  calls one that "fails schema validation" incompatible. Recommendation
  for part (c): an I/O failure is unreadable, and bytes read that the
  syntax refuses are incompatible, so a malformed delivery is never
  answered with an older copy. Part (c) decides.

## 14. Review

No review record yet.
