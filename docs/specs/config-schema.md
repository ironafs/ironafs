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
   - the policy envelope's validation, the four policy states and how
     several policy files form one policy: part (c). This page names the
     envelope keys and their types (5.2.8), the interface (5.8), and the
     refusals part (c) applies (section 7: an untrusted policy source and
     a policy file the syntax refuses are incompatible, Q2 and Q12);
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
     reserves their names (5.4, point 5);
   - finding the files and reading them, in the order of 5.1, point 2,
     and by the walk of 5.1, point 3: the caller. It hands this crate one
     outcome per source, and per directory whose walk or listing fails
     (5.8, points 1 and 2): a trusted file's name and bytes, or an
     unreadable or untrusted source, which carries no bytes;
   - run-time overrides (debug levels and captures, O17, point 1): the
     events specification and the control endpoint (#70). No key of this
     schema is a run-time override.

## 3. The design in short

1. The schema is a fixed table in `ironafs-config`: each known key has a
   path, a type, a range, a default, the layers that may set it, and
   whether a change applies on reload or needs a restart.
2. Only root-owned files that nobody else can write are used, for local
   files and policy alike (5.1). The local files are validated as a whole.
   Any unknown key, wrong type or out-of-range value refuses the whole
   local configuration, with the position and, for an unknown key, a suggestion. The one
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
- **Source:** one file as the caller hands it over, with its layer and a
  name for diagnostics: its bytes when it is trusted and was read (5.1,
  point 3), or else the outcome `Unreadable` or `Untrusted`, which carries
  no bytes (5.1, point 7). An untrusted or unreadable source never
  carries bytes.
- **Schema path:** a key's path with each cell name replaced by `*`, for
  example `cells.*.protocol`. Every known key has one.
- **Known key:** a schema path in 5.2's tables that is not retired
  (5.7, point 3). Any other path is **unknown**. A **known table** is a
  path that 5.2 uses as a table: `cells`, `cells.<cell>`, `network`,
  `discovery`, `cache`, `namespace` and `security`, and `policy` in P
  only (in F it is unknown, 5.4, point 6).
- **Security key:** a known key whose unknown value refuses cells, never
  the whole file or policy (N8, point 2; O19, point 6). In M2a these are
  the reserved floor keys of 5.4, point 5.
- **Reserved key:** a known security key for which this release knows no
  value, so every value is unknown (issue #93, "`min_class` exists as a
  reserved name").
- **Trusted source:** a source that passes 5.1, point 3's test. Only
  trusted sources are used (5.1, point 4).
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
   key in 5.2 may be set in R `[ADR 0004 O17.1: Working]`. In both
   directories a source is an entry whose name ends in `.toml` and does
   not start with `.`; any other entry is not a source and is not read
   `[OWN]`, so an editor's or a package manager's temporary or backup file
   (`.policy.toml.swp`, `policy.toml.dpkg-new`) is never taken as policy or
   configuration.
2. **This crate parses bytes.** The caller finds the files, reads at most
   `MAX_FILE_BYTES` + 1 bytes of each trusted file and hands over all it
   read, so a larger file is seen and refused (config-syntax.md, 5.2), and
   hands over one source per file ([config-syntax.md](config-syntax.md),
   §2) `[OWN]`. A source that is unreadable or untrusted (point 7) is
   handed over as that outcome, with no bytes, and the caller goes on to
   the next source: it collects the outcomes of `ironafs.toml` and of the
   drop-in directory without stopping at the first failure, as it does
   for the policy directory (5.8, points 1 and 2) `[OWN]`. **At most
   `config.sources_max` sources** (5.2.9) are taken from the drop-in
   directory, and at most as many from the policy directory: the caller
   counts the sources (point 1) as it lists, skips other entries without
   storing them, and when the count passes the bound it stops listing and
   reads no file of that directory; the directory's outcome is then
   `TooMany`, which is malformed (point 7) `[OWN]`, so the bytes held for
   one layer are at most `config.sources_max` + 1 times
   (`MAX_FILE_BYTES` + 1). F's sources are in this order: `ironafs.toml`, then the
   drop-ins sorted by name, byte by byte; P's sources are sorted the same
   way `[OWN]`. So which refusal is reported first never depends on the
   order in which a directory lists its entries.
3. **Trust: one walk, the same for F and P.** The caller opens every
   source by walking its path from `/` by descriptor, one component at a
   time: each directory with `openat` and `O_NOFOLLOW | O_DIRECTORY`
   relative to the previous descriptor, checking each directory's owner
   and mode on its descriptor, `/` included, before it opens the next
   component. Before the file is opened, `fstatat` with
   `AT_SYMLINK_NOFOLLOW`, relative to its directory's descriptor, must
   show a regular file; when it succeeds and shows any other type (a
   device, a FIFO, a socket, a symbolic link), the source is untrusted
   without being opened, so no open can block on it or fail on it first
   `[OWN]`. A failing `fstatat` is not an untrust condition: point 7
   classifies it. The directory has passed the checks,
   so only root can replace the entry between that call and the open
   (8, point 1's residual). The file is then opened with `openat` and
   `O_NOFOLLOW | O_NONBLOCK | O_NOCTTY | O_CLOEXEC` relative to the same
   descriptor; `fstat` on the opened descriptor must again show a regular
   file, with the owner and mode below, before any byte is read, and the
   bytes are then read from that same descriptor. The source is `trusted` only when
   every component is owned by uid 0 and writable by no other user or
   group, and the file is a regular file; a symbolic link anywhere on the
   path (including one that makes an `openat` fail with `ELOOP` or
   `ENOTDIR`), a component with another owner, a component writable by
   group or others, or a file that is not a regular file makes it
   `untrusted` `[OWN]`, from issue #93 ("set only by a root-owned file or
   by policy") and the DNS client specification (#60), 5.6.4, point 6,
   which states the same ownership test. The directory listing that finds
   the drop-ins and policy files is read from the walked directory's own
   descriptor, and each file it names is opened with `openat` relative to
   that same descriptor, the one that was checked and listed, never by a
   new walk. Because every check is on the descriptor the bytes come from,
   no component can be swapped between the check and the read. O17, point 4, guarantees only a "root-owned
   policy directory"; a file in it that another user can write would let
   a local edit pose as policy, which O19's "Why" names as the reason the
   directory is separate ("A separate policy directory on Linux keeps a
   local edit from posing as policy"). So a P source takes exactly the
   walk an F source takes.
4. **An untrusted source is never used** (decided, Q2):
   - an untrusted F source refuses the whole local configuration (a local
     refusal, section 7), so every value in the F layer comes from a
     root-owned file nobody else can write;
   - an untrusted P source makes the policy incompatible, with the reason
     "untrusted policy source" `[OWN]`: its bytes may not be used, and
     falling back to the last good copy would let whoever can write it pin
     an older policy, which O19, point 7, forbids for a readable policy
     that fails validation ("It never falls back to an older copy"). Part
     (c) applies the state.
5. **What a source may set:** each key's "Layers" column in 5.2. `F` there
   means a trusted local file, the only kind point 4 lets through; `P`
   means a trusted policy source. Every key of 5.2 can therefore be set
   only by a root-owned file nobody else can write, or by policy, and
   none through the control endpoint or a run-time override (issue #93;
   O17, point 1).
6. **Defaults never widen.** A missing or empty source contributes
   nothing, and an untrusted one is refused (point 4), never replaced by
   defaults. The defaults of the flat-list confirmation, the explicit
   servers and the scope add no destination: no confirmation, no servers,
   and scope `host`, which keeps the cell on the host's own resolver path
   (#60, 5.6.3) `[OWN]`. Discovery is on by
   default `[OWN]`: C33, point 1, puts DNS after configuration in the
   lookup order and point 8 lets an administrator turn it off; DNS then
   goes only where 5.6.3 of the DNS client specification lets it. A
   refused local configuration at start does not fall back to defaults
   (Q11).
7. **Every source state has one result** `[OWN]`. "Absent" means the
   walk ends in `ENOENT` at `ironafs.toml`, or at the drop-in or policy
   directory or any directory above it, every directory opened before it
   having passed point 3's checks: there is no such source. A drop-in or
   policy directory that holds no source (point 1) is absent too. At a
   file, `ENOENT` from `fstatat` on `ironafs.toml` is absent; a file that
   the directory listing named, or that `fstatat` showed, but that is gone
   at its `fstatat` or its `openat` is unreadable, never absent.
   "Unreadable" means any other failure to walk, list, `fstatat`, open,
   `fstat` or read that is not an untrust condition of point 3 (for
   example `EACCES` or `EIO`, at `fstatat` as anywhere else). "Malformed"
   means the syntax or the schema refuses the bytes, or a directory holds
   more than `config.sources_max` sources (point 2).

   | Layer | Absent | Unreadable | Untrusted | Malformed | Valid |
   |---|---|---|---|---|---|
   | F (`ironafs.toml`, the drop-in directory or a directory above it, each drop-in) | contributes nothing (`ironafs.toml` or the drop-in directory) | local refusal | local refusal | local refusal | used |
   | P (the policy directory, each policy file) | the whole policy is absent (the policy directory): unmanaged if `managed_before` is false, unreadable if it is true (O19, point 5) | the policy is unreadable | the policy is incompatible | the policy is incompatible (Q12) | used, if every policy file is valid |

   Across the policy files, and the policy directory or a directory
   above it when the walk or listing fails there or the directory holds
   too many sources, the worst state decides: untrusted or malformed (the policy is incompatible), then
   unreadable, then valid `[OWN]`.

   A local refusal keeps the last good local configuration, or stops the
   start if there is none (section 7). An unreadable policy uses the last
   good policy (degraded) or, with none, fails (O19, points 4 and 9).
   **`managed_before`** is an explicit input to part (c): a durable flag
   in the state directory, set when a policy is first applied as valid and
   cleared only by the `mode = "unmanaged"` directive or the audited
   `afs config unmanage` command (O19, point 5). It is separate from
   whether a last good copy exists: a lost or corrupt last good copy with
   the flag set is "unreadable with no last good policy", which fails.
   Only the loss of the whole state directory loses the flag, and then the
   host starts unmanaged (O19, point 10). Its storage is part (f)'s.

### 5.2 Keys

Columns: **Path** (as written in a file; `<cell>` is a cell-name segment, quoted or bare: the syntax keeps no record of quoting, and a name with a dot needs quotes);
**Type** (5.3); **Range**; **Default** (the D layer's value; "none"
means the key has no value unless set); **Layers** that may set it;
**Applies** on `reload` or needs a `restart`: O18, point 5, requires that
a setting needing a restart says so, and each key's entry here is this
page's own `[OWN]`; **Source**: the decision a key serves. A key's name,
default and layers are this page's own `[OWN]` wherever the source does
not state them; the source column names only what the cited decision
itself fixes. No key is required in any layer: a host with no
file and no policy runs on defaults `[ADR 0004 O19.4: Working]`
("Unmanaged ... runs on defaults and local files").

The table names below are decided (Q1) `[OWN]`: one table
per concern, so a drop-in can hold one concern, and the cell-scoped keys
under one `cells` table, so a cell's keys are found together.

#### 5.2.1 The home cell

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `home_cell` | cell name | 5.3.1 | none | F, P | reload | L34, point 4, requires `@cell`; namespace specification (#80), 5.6.5, point 7, uses the home cell; the key itself `[OWN]` |

1. With no value here, the home cell is the classic `ThisCell`'s, if part
   (d) supplies one (5.6, point 7) `[ADR 0004 O18.3: Working]`.
2. A home cell that is refused (5.4, point 4) stays the home cell: `@cell`
   resolves to it, and every request to it fails with the refusal's
   reason, as for any refused cell (section 7) `[OWN]`.

#### 5.2.2 Cells

One table per cell: `[cells."<cell>"]`. The segment, quoted or bare, is
a cell name (5.3.1).

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `cells.<cell>.protocol` | enumeration | `afs3` | `afs3` | F, P | reload, as a generation change (point 3) | N27, points 1 and 2 |
| `cells.<cell>.servers` | list of server addresses | 0 to `config.cell_servers_max` entries, no two equal (5.3.3) | empty | F, P | reload | C33, point 1 |
| `cells.<cell>.discovery` | boolean | | `true` | F, P | reload | C33, point 8; DNS client specification (#60), 5.6.4, point 2 |
| `cells.<cell>.dns_scope` | enumeration | `host`, `servers` | `host` | F, P | reload | C33, point 5; #60, 5.6.4, point 5 |
| `cells.<cell>.dns_servers` | list of server addresses | 0 to `dns.explicit_servers_max` (8) entries | empty | F, P | reload | as above |
| `cells.<cell>.dns_interface` | interface name | 1 to 15 bytes of `a` to `z`, `A` to `Z`, `0` to `9`, `.`, `_`, `-`, other than `.` and `..` (Q6) | none | F, P | reload | as above |
| `cells.<cell>.min_class`, `min_level`, `legacy_fileservers`, `anonymous`, `learned`, `rxgk` | security key | reserved (5.4, point 5) | none | F, P | reload | N3 (names); N8, point 2; issue #93 (`min_class`); Q9 (the rest) |

1. **A configured cell** is one whose `cells."<cell>"` table has an
   explicit header or holds a known key, in a trusted source (5.1, point
   4) `[OWN]`. This one definition is used everywhere: for the cells the
   dynamic root lists (C12, point 1), for the cell-name check and the
   two-spellings check (5.5), and for the cells `policy.required` counts
   (5.8, point 2). A `cells.<x>` table created only implicitly by an
   unknown subtree (`[cells."x".foo]` in P, which 5.4, point 2, ignores) is
   not a configured cell: its segment is not checked, it is not counted,
   and it creates nothing; the subtree is reported as one unknown path.
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
5. **Discovery for a cell** happens only when both `discovery.enabled`
   and `cells.<cell>.discovery` are effectively true `[ADR 0004 C33.8:
   Working]` (off "globally and per cell": either switch turns it off)
   `[OWN]` for the conjunction. The switches govern DNS only; the classic
   files are configuration inputs (O18, point 3, lists them apart from
   DNS) and part (d) decides whether they have a switch of their own.
6. **The scope group.** `dns_scope`, `dns_servers` and `dns_interface` of
   one cell are one group `[OWN]`: the group is taken whole from the
   highest layer that sets any of its keys, and a lower layer's value for
   another key of the group is not used (it is shown as overridden, 5.6,
   point 2). Otherwise a policy that sets `dns_scope = "servers"` could be
   completed by a local file's server list or interface, which would let a
   lower layer decide where a policy-scoped cell's queries go. A member the
   winning layer leaves unset takes its default, never a lower layer's
   value, so scope `servers` without servers or an interface sends
   nothing. How a scoped cell relates to `discovery.servers`, the flat-list
   confirmation and the host's resolver is the DNS client specification's
   (#60, 5.6.3, point 2: a scoped cell uses none of them).

#### 5.2.3 Network

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `network.families` | enumeration | `dual`, `ipv4`, `ipv6` | `dual` | F, P | restart | C34, point 2, as #53, point 4, amends it; issue #93 (owner's decision, 2026-11-20) |

1. Only an explicit value other than `dual` selects single-family
   operation; nothing else ever does (C34, point 2, as amended).
2. It needs a restart: it decides which sockets the daemon binds at start
   `[OWN]`. A reload that changes it keeps the running sockets, and the
   effective configuration then holds both values for the key: **active**
   (what the sockets are) and **pending** (what the layers now say), with
   the pending value's origin. Status shows both and "restart needed"
   (O18, point 5) until a restart applies the pending value. A pending
   value set by policy is locked like any other: no layer can change it,
   and no reload can drop it short of the policy changing `[OWN]`.
   **Until a restart, the active value serves:** the daemon keeps its
   sockets and the traffic on them as they are, and the pending value
   changes nothing until the restart applies it `[OWN]`.
3. The daemon's own duties under C34, point 2, as amended, are not this
   page's: refusing to start in dual mode when either family's socket
   cannot be created or bound, and showing single-family operation in
   startup diagnostics and `afs status` (M2a-16, planned).

#### 5.2.4 Discovery

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `discovery.enabled` | boolean | | `true` | F, P | reload | C33, point 8; #60, 5.6.4, point 1 |
| `discovery.flat_list_confirmed` | list of addresses (no port) | 0 to `dns.name_servers_max` (3) entries, order kept | empty | F, P | reload | C33, point 4; #60, 5.6.4, point 3 |
| `discovery.servers` | list of server addresses | 0 to `dns.explicit_servers_max` (8) entries, order kept | empty | F, P | reload | C33, point 5; #60, 5.6.4, point 4 |

1. These keys and the `discovery` and `dns_*` keys of 5.2.2 are the ones
   issue #93 and the DNS client specification (#60, 5.6.4, point 6) say a
   root-owned file or policy alone may set. 5.1, points 3 to 5, makes that
   true of every key of 5.2, so no key needs a separate mark.
2. Their meaning, and what a change does to the DNS client's view, are
   the DNS client specification's (#60, 5.6.4, points 3 and 7).

#### 5.2.5 Cache

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `cache.size_max` | byte count | 5.3.5 | 1 GiB (1,073,741,824), decided (Q4) | F, P | reload, as a lowered or raised soft cap (point 1) | issue #93 ("cache size"); C27, points 4 and 8 |

1. It is the total of resident clean bytes the cache may hold, across all
   accounts and the pool. The per-account floors and caps of C27, point
   4, are derived from it by the cache, which C27, point 8, leaves to the
   builder and to measurement `[OWN]`. A lower value applies by eviction,
   never by refusing a reload `[OWN]`.
2. Pinned chunks (dirty or in use, C27, point 3) are not evictable, so
   resident bytes may exceed this cap while they are pinned; the hard caps
   on pinned bytes are the cache's (C27, point 4) `[OWN]`.
3. The 1 GiB default is decided (Q4): a fixed value, until M2a's
   measurements (C27's "Revisit if").

#### 5.2.6 Namespace

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `namespace.sysname` | list of strings | 0 to `ns.sysname.count_max` (16) entries, each 1 to `ns.sysname.len_max` (64) bytes, under the namespace specification's per-name rules | the built-in list per target (#80, 5.6.5, point 10) | F, P | reload | L34, point 4; #80, 5.6.5, points 8 and 9 |

1. A value, even an empty list, replaces the default list entirely (#80,
   5.6.5, point 8). A list that breaks a per-name rule refuses its source
   (section 7).
2. There is no key that lowers `ns.sysname.count_max` `[OWN]`. The
   namespace specification (#80, 5.2, point 2) lets configuration lower it
   per site; a site does so by configuring a shorter list, which is the
   only thing the count bounds. #80's point is to be reworded to say so
   (a cross-specification note for its next round).

#### 5.2.7 Security: machine-wide values

Decided (Q9): every floor key of N3 is reserved in M2a, per cell (5.2.2)
and machine-wide here, and so is the `fips` key (O31, point 4).

| Path | Type | Range | Default | Layers | Applies | Source |
|---|---|---|---|---|---|---|
| `security.min_class`, `min_level`, `legacy_fileservers`, `anonymous`, `learned`, `rxgk` | security key | reserved (5.4, point 5) | none | F, P | reload | N3 ("Per cell, with machine-wide defaults"; the names); Q9 |
| `security.fips` | security key | reserved (5.4, point 5) | none | F, P | reload | N8, point 1; O31, point 4; Q9 |

1. **Which value covers a cell,** highest first: a per-cell value set by
   policy, a machine-wide value set by policy, a per-cell value from F, a
   machine-wide value from F, the default `[OWN]`. So a floor set by
   policy, machine-wide or per cell, is never lowered by a local value
   (O17, point 2: "A value set by policy locks its key"; N3, point 1:
   "OS policy can lock these keys like any other"); a local per-cell value
   it displaces is shown as overridden (5.6, point 2). Within one layer a
   cell's own value wins over the machine-wide one (N3: "Per cell, with
   machine-wide defaults"). `security.fips` covers every cell `[OWN]`.
2. **A cell refusal is a rule over cell names, not a list** `[OWN]`: a
   machine-wide value that refuses covers every cell, configured or not,
   so a cell from the classic files or from discovery is refused by it
   too (5.8, point 4).
3. M2b-17 (planned) gives these keys their values on this same
   mechanism; nothing here decides their meaning.

#### 5.2.8 The policy envelope

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
   `[ADR 0004 O19.1: Firm]`. Their placement in a `policy` table is
   decided (Q1).
2. What each means, the order in which they are checked, and how they set
   the policy state: part (c).

#### 5.2.9 Bounds this page sets

| Name | Value | Why |
|---|---|---|
| `config.cell_servers_max` | 16 | the same bound as `dns.targets_max` (#60, 5.2), so a configured cell and a discovered one carry at most the same number of servers; within config-syntax.md's `MAX_ARRAY_ELEMENTS` `[OWN]` |
| `config.required_max` | 256 | config-syntax.md's `MAX_ARRAY_ELEMENTS` `[OWN]` |
| `config.unknown_reported_max` | 64 | the unknown policy keys named one by one in a report; the rest are counted (5.9, point 3) `[OWN]` |
| `config.sources_max` | 64 | the sources taken from one directory, the drop-in directory or the policy directory (5.1, point 2); a site's drop-ins and policy files are a handful, and the bound keeps the bytes held for one layer at about 16 MiB `[OWN]` |

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
   without a zone index, whose embedded IPv4 part, if any, follows the same
   dotted-decimal rule `[OWN]`; a server address adds an optional port,
   written `192.0.2.1:7003` or `[2001:db8::1]:7003`, 1 to 65535 in decimal
   without a leading zero `[OWN]`. Without a port, the port is the one the
   specification of the service asked states: 7003, the VL service's port
   in the location service specification (#65, 5.1.1, point 5), for
   `servers`; 53 in the DNS client specification (#60, 5.6.4, point 4) for
   `dns_servers` and `discovery.servers` `[OWN]`. A host name is refused, decided (Q5). Two entries
   equal as addresses and ports (after parsing, so `::1` and
   `0:0:0:0:0:0:0:1` are equal) refuse the list `[OWN]`. Either family is
   accepted whatever `network.families` says
   `[ADR 0004 C34.1: Working]`; using an address of a family that is off
   is the user's rule. RFC 4291 is not yet in the register of editions;
   it is registered before this page is reviewed.
4. **Interface name:** a string; its range is in 5.2.2.
5. **Byte count:** an integer, in bytes, from 64 MiB (67,108,864) to
   2^62 − 1 `[OWN]`: the floor keeps the cache above a few dozen chunks of
   C27, point 8's proposed 1 MiB; the ceiling keeps the sum of two counts
   below 2^63 (section 6). Unit suffixes are not offered: the
   syntax has no unit, and `_` separators (`1_073_741_824`) keep a large
   value readable.
6. **Version:** a string `MAJOR.MINOR.PATCH`, each part a decimal
   integer from 0 to 2^32 − 1 without a leading zero, compared part by
   part `[OWN]`. Pre-release and build suffixes are refused, decided
   (Q7). The client's own version for this comparison is its release
   triple. A build whose own version is not a plain triple (a development
   or pre-release build) meets no `policy.min_client_version`: any value
   there is newer than it, so the policy is incompatible (O19, point 3)
   `[OWN]`, failing closed: a build that is not a release cannot claim to
   meet a site's minimum. No ADR fixes the release version's form; the
   release process adopts this one when it is specified.
7. **Schema path:** a string of bare-key segments (config-syntax.md, 5.1,
   point 8) separated by `.`, with `*` as the whole segment that stands
   for a cell name, as in `cells.*.min_class`. A concrete cell name is not
   allowed in a schema path `[OWN]`. Whether a path is known is checked
   against 5.2; an unknown path is not a type error here (5.8, point 2).
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
   value is not checked and not stored. Every unknown key is reported: the
   report names up to `config.unknown_reported_max` of them by path and
   position and counts the rest (5.9, point 3), never with its value
   `[OWN]` (a value may be sensitive, O6).
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
   2, says is "never ignored" (decided, Q3). A cell is refused when the
   effective value of a security key for it (5.2.7, point 1) is unknown; a value that a higher layer overrides is not effective
   (O17, point 2). A value of the wrong syntax type (an integer for
   `min_class`) is a type error, not an unknown value: it refuses the file
   in F and makes the policy incompatible in P (decided, Q8).
5. **Reserved security keys in M2a.** The six floor keys of N3, per cell
   and machine-wide, and `security.fips` are reserved: every string value
   is unknown, so each refuses the cells it applies to (issue #93 for
   `min_class`; decided, Q9, for the rest).
6. **Known key, wrong layer.** In F the `policy` table itself, and every
   envelope key, is unknown (5.2.8).

### 5.5 Validating one source

1. A source is parsed (config-syntax.md); a parse error is that source's
   refusal.
2. The document's explicit tables and entries are checked together, in
   file order `[OWN]`:
   - **a table** (an explicit header, or a table a header creates
     implicitly) must be a known table; an unknown table, even an empty
     one, is an unknown key (5.4, points 1 and 2), reported once by its
     path: the tables and entries under it are neither checked nor
     reported `[OWN]`; a table at the path of a known key that holds a
     value (`[cells."x".protocol]`) is a type error;
   - **an entry** must be a known key; a value at the path of a known
     table (`network = 1` at the root) is a type error; then layer
     (5.1, point 5), type and range (5.3);
   - the segment of every configured cell (5.2.2, point 1), quoted or
     bare, passes the cell-name constructor; a segment that only an
     unknown subtree creates is not checked.
3. **Two spellings of one cell in one source refuse it:** two configured
   cells (5.2.2, point 1) whose segments construct equal cell names (for example
   `cells."Example.org"` and `cells."example.org."`) are a redefinition,
   though the syntax compares segments byte for byte
   (config-syntax.md, 5.1, point 8), and the error names both positions
   `[OWN]`, from config-syntax.md, 5.7, point 6.
4. **The first refusal of F stops** the source's other checks, as the
   parser does (config-syntax.md, 7, point 2) `[OWN]`. Unknown values of
   security keys (5.4, point 4) do not stop it; each is collected as a
   cell refusal. **Cell refusals survive a local refusal:** after the first
   refusal, the rest of every trusted F source that parsed is still
   scanned for security keys, and the cells each unknown value covers are
   carried in the `LocalRefusal` as refusal rules (5.2.7, point 2), each
   a security key with its scope and the source and position of its
   value: a per-cell value refuses its cell, a
   machine-wide value every cell, in both cases unless a policy value
   covers that cell for that key (5.2.7, point 1) `[OWN]`. Validating F
   reads no policy, so that exception is applied by `merge` (5.8, point
   4), not here. In the scan, a cell segment the cell-name constructor
   refuses names no cell and yields no rule, and a value of the wrong
   syntax type is a type error (5.4, point 4) and yields none `[OWN]`.
   Untrusted and unreadable sources carry no bytes and are never scanned
   (5.1, point 4); when one of them is the refusal, every trusted F
   source that parsed is still scanned, the same way. Part (e), keeping
   the last good local configuration, still refuses those cells (5.8,
   point 5), so a refused file cannot keep a cell serving under a floor
   its new value raised. A source that does not parse yields no cell
   refusals; the whole configuration is refused and reported, which is
   the residual. The rules of an earlier refusal are not dropped by it
   (5.8, point 5).
5. **P collects:** in a policy source, unknown keys are collected for the
   report and validation continues; the first other failure makes the
   policy incompatible (section 7). An unknown subtree ignored in P
   configures no cell (5.2.2, point 1).
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
   schema, marked retired, and is not a known key (section 4); in F it
   refuses the file with a hint naming its replacement, in P it is ignored
   and reported as retired, and in `policy.required` it is unknown
   (5.8, point 2).
4. **A changed default** is allowed only for a Working default, and the
   release notes name it `[OWN]`, from N3's split between Firm keys and
   Working defaults.
5. **The envelope** (5.2.8) never changes `[ADR 0004 O19.1: Firm]`.
6. **A newer policy on this client** is handled by 5.4, points 2 to 4: a
   key it adds is ignored and reported, unless `required` names it; a
   value it adds to a security key refuses its cells; a value it adds to
   any other enumeration makes the policy incompatible
   `[ADR 0004 O19.7: Firm]`. A site that adds such a value states
   `min_client_version` (N27, point 4, says the same for the protocol
   identifier).

### 5.8 Interfaces for parts (c) to (f)

These are names and shapes only. What each part decides is its own.

1. **Validated layers.** Part (b) provides `validate_local(outcomes) ->
   Result<LocalLayer, LocalRefusal>` for F (a `LocalRefusal` carries the
   refusal rules of 5.5, point 4). `outcomes` is F as the caller found it
   (5.1, point 2): a list, empty when everything is absent, of outcomes,
   each `Unreadable` or `Untrusted` (neither carries bytes; a trust
   failure may come before any byte is read), `TooMany` (for the drop-in
   directory, 5.1, point 2), or, for a trusted file, `Read` with its name
   and bytes. An outcome is for `ironafs.toml` or a drop-in, in the order
   of 5.1, point 2, or for the drop-in directory or a directory above it,
   when the walk or the listing fails there, whether or not the listing
   has already returned entries; that outcome comes before the drop-ins'
   outcomes already obtained `[OWN]`. Any outcome other than `Read`
   refuses the local configuration (5.1, point 7), and `validate_local`
   still scans every `Read` source that parses for the refusal rules of
   5.5, point 4. A list with more than `config.sources_max` drop-in
   outcomes is refused as `TooMany` is, and only `ironafs.toml` and the
   first `config.sources_max` drop-ins are parsed and scanned `[OWN]`.
   Part (b) also provides
   `validate_policy_source(source) -> PolicyKeys` for one P source, where
   `PolicyKeys` holds that source's known keys with their values, the
   envelope's raw values, the unknown keys for the report, the cell
   refusals, and the first other failure if any. How the sources' keys
   form one policy is part (c)'s (5.6, point 5). Neither reads a file or a
   clock.
2. **The envelope and the policy state** (part (c)): part (c) takes the
   policy's sources as the caller found them: either "absent" (5.1, point
   7), or a list of outcomes, each `Unreadable`, `Untrusted` (a trust
   failure, which may come before any byte is read), `TooMany` (for the
   policy directory, 5.1, point 2) or, for a file, its `PolicyKeys`. An
   outcome is for a policy file, in the order of 5.1, point 2, or for the
   policy directory or a directory above it, when the walk or the listing
   fails there, whether or not the listing has
   already returned entries; that outcome comes first, followed by any
   file outcomes already obtained `[OWN]`. A list with more than
   `config.sources_max` file outcomes makes the policy incompatible, as
   `TooMany` does `[OWN]`; together with `managed_before`
   (5.1, point 7) and whether a last good policy exists (part (f)). It
   returns the policy state of O19, point 4, as a typed value
   `PolicyState { Unmanaged, Valid(PolicyLayer), Unreadable,
   Incompatible(reason) }`, the worst outcome, of a directory or a file,
   deciding (5.1, point 7), so an untrusted directory makes the policy
   incompatible, never unreadable. An absent policy with `managed_before` true is unreadable, never
   unmanaged (O19, point 5: "A policy that merely disappears is treated as
   unreadable"). The policy state is computed independently of F: a local
   refusal never changes it, and a policy state never refuses F `[OWN]`.
   This page gives part (c), for O19, points 3 and 7 `[OWN]`:
   - `is_known(path)`: true when the path is a known key of 5.2, reserved
     keys included; false for a retired or unknown path, and for an
     envelope path (the envelope cannot be "required" of itself);
   - `has_value(path)`: for a path without `*`, true when the policy itself
     sets it (a default, or a value from F, does not count); for a path
     with `*`, true when at least one configured cell of the policy
     (5.2.2, point 1) sets that key `[OWN]`, so a policy can require a
     per-cell key with no neutral value (`dns_interface`) that it sets on
     some of its cells only. A reserved key's value counts as a value,
     though every such value refuses its cells;
   - the client's own version (5.3, point 6).
3. **The classic files** (part (d)): part (d) returns a
   `ClassicContribution` (cells with their servers, and a home cell), each
   cell name built by the one constructor. The resolution order of 5.6,
   point 7, consumes it; it never enters `merge`.
4. **Merging:** `merge(LocalLayer, RefusalRules, PolicyInput, Running) ->
   Effective`, where `RefusalRules` are the refusal rules part (e) keeps in
   force (point 5; empty when the local configuration was accepted), each
   with the source and position of the value that raised it, kept with
   the rule while part (e) keeps it, so the refusal's reason names them
   (5.9, point 4) `[OWN]`, and to which `merge` applies 5.2.7, point 1's exception for a cell a policy
   value covers; `Running` holds the active values of the restart keys and their
   origins (from the running daemon, or empty at start, when the pending
   value becomes active), and
   `PolicyInput` is `Unmanaged` or `Policy(PolicyLayer)`: the valid
   policy, or in the degraded state the last good one part (f) loaded.
   Part (c)'s other states (incompatible, or unreadable with no last good
   policy) give no `PolicyInput`: the daemon then blocks admissions and
   fences grants (O19, points 3 and 9) whatever F holds. `Effective`
   carries per-key origins, the locked-and-overridden keys, the active
   and pending values of restart keys (5.2.3, point 2), and the refused
   cells as a predicate `is_refused(cell) -> Option<reason>` over any cell
   name (5.2.7, point 2), which every consumer checks before any
   network I/O for a cell (N27, point 2's rule, applied to every refusal)
   `[OWN]`.
5. **Reload** (part (e)): part (e) compares the running `Effective`, if
   there is one, with a new validation's result, and returns the typed
   decision the daemon applies (issue #93, "Reload"). There is no running
   `Effective` at start, nor while the policy gives no `PolicyInput`;
   part (e) also holds the last good `LocalLayer`, if any, so a reload can
   leave a fenced state `[OWN]`. The local and policy outcomes are taken
   separately `[OWN]`: a new valid policy with a refused local
   configuration is merged with the last good `LocalLayer` (plus the
   refusal's cell rules); a valid local configuration with a policy that
   gives no `PolicyInput` fences service whatever F holds. The refusal
   rules of every local refusal since the last good `LocalLayer` was
   accepted stay in force until a local configuration is accepted
   `[OWN]`, so a later refusal that yields fewer rules (a file that no
   longer parses) does not let a refused cell serve again. A failing layer
   keeps its last good value where its rule allows one (O18, point 5, for
   F; the last good policy only when unreadable, for P); a passing layer
   takes its new value. This page gives it the refusal kinds of
   section 7 and, per key, whether a change applies on reload or needs a
   restart (5.2's "Applies" column).
6. **The last good policy** (part (f)): part (f) stores and loads a
   `PolicyLayer`, the policy as validated, security-key values included,
   so the cells a policy refuses are refused again when the last good copy
   is used. Its encoding, checksum and atomic write are part (f)'s
   format.

### 5.9 Diagnostics

1. Every refusal names the source, the position of the offending key or
   value (config-syntax.md, 7, point 1), what was expected, and a hint
   `[ADR 0004 O18.2: Working]`.
2. **The suggestion** for an unknown key in F compares the unknown key's
   last segment with the last segment of each known key valid in F (not
   the envelope, 5.4, point 6): first the known keys of the same table,
   then those of other tables, which are suggested by their full path. An
   unknown table is compared the same way with the known tables valid in
   F `[OWN]`. Candidates within a Levenshtein distance
   of 2 are offered, closest first, ties in 5.2's order, at most three
   `[OWN]`. So `cells."x".protocl` suggests `protocol`. Distances are
   computed only between segments of at most `MAX_KEY_BYTES`
   (config-syntax.md, 5.2), so the work is bounded. No candidate within 2
   gives no suggestion, never a guess.
3. **The unknown-policy-key report** names up to
   `config.unknown_reported_max` paths with positions, then a count of
   the rest `[OWN]`. It is an event, raised once per policy load or
   reload (the event's catalogue entry: the events specification). A path
   is rendered segment by segment: a segment that is a bare key as is, any
   other quoted as a basic string, so a dot inside a cell name stays
   visible as part of its segment; every octet outside printable ASCII
   (0x20 to 0x7E) is written as `\xHH`, so no format or bidirectional
   character reaches a terminal or log `[OWN]`.
4. **Cell refusals** name the cell, the key and the source of the value,
   never the value when it comes from policy `[OWN]`.

## 6. Arithmetic

| Value | Bound | Method |
|---|---|---|
| byte count | 64 MiB to 2^62 − 1 | compared after the syntax's checked parse; a sum of two is at most 2^63 − 2, below `i64::MAX` |
| port | 1 to 65535, no leading zero | parsed digit by digit with `checked_mul` and `checked_add` into `u16`; overflow refuses |
| version parts | 0 to 2^32 − 1 each | parsed into `u32` with checked arithmetic; compared as a triple |
| suggestion distance | 2 | one row of at most `MAX_KEY_BYTES` + 1 counters; no sum exceeds 2 × `MAX_KEY_BYTES` |
| list counts | 5.2's ranges | compared before the list is stored |

## 7. Errors and failure

| Refusal | Raised for | Effect |
|---|---|---|
| **Local refusal** (`LocalRefusal`) | in F: a parse error; an unknown key or table; a type or range error; a cell name the constructor refuses; two spellings of one cell; an envelope key; an untrusted source or directory (5.1, point 4); an unreadable source or directory, or a drop-in directory holding more than `config.sources_max` sources (5.1, point 7); a table or value at the wrong kind of path (5.5, point 2); the same key in two sources; a retired key | no F source is used: the whole local configuration is refused; part (e) keeps the last good one `[ADR 0004 O18.5: Working]` and still applies the cell refusals carried in the refusal (5.5, point 4); at start without a last good one, the daemon does not start (decided, Q11) |
| **Policy incompatible** | in P: an untrusted source or directory (5.1, point 4); a policy directory holding more than `config.sources_max` sources (5.1, point 2); bytes the syntax refuses (decided, Q12; an I/O failure is unreadable instead); a type or range error; a table or value at the wrong kind of path; an unknown enumeration value of a key that is not a security key, `protocol` included; a cell name the constructor refuses; two spellings of one cell; and part (c)'s envelope checks. A retired key is not among them: it is ignored and reported (5.7, point 3) | the whole policy is incompatible: failed health, admissions blocked, grants fenced, never the last good copy `[ADR 0004 O19.3: Firm]` `[ADR 0004 O19.7: Firm]` |
| **Cell refused** | in F or P: an unknown value of a security key | that cell, or every cell a machine-wide value applies to, is refused with the reason; the rest serves `[ADR 0004 N8.2: Firm]` `[ADR 0004 O19.6: Firm]` |

Each fails closed: no partial file, policy or cell is used. A refused
cell is listed in the effective configuration with its reason; no network
I/O is made for it, every request to it fails with the reason, and status
shows it (5.8, point 4) `[OWN]`.

## 8. Security

1. **Who writes each layer:** F, the administrator, and for a
   misconfigured host any user who can write a file the daemon loads
   (config-syntax.md, 8, point 1); P, the site's management system. Such a
   misconfigured file is refused, never read (5.1, point 4), so no key,
   least of all those whose misuse would send a cell's name or queries
   where the host's policy does not (C33, points 4, 5 and 8), takes a value
   from a file another user can write. The same holds for policy files.
   The residual: the check is made on the file read, and a root process
   that writes a hostile file is outside this page's reach.
2. **A typo in F fails loudly** (5.4, point 1).
3. **A newer policy is never undercut silently.** An unknown policy key is
   reported, and `required` and `min_client_version` let a site make it
   fatal (O19, points 2 and 3). The residual is O19's own: a site that
   uses neither gets the report only, except for the floor keys, which M2a
   reserves so that a floor stated for a later client refuses cells on M2a
   instead of being reported (decided, Q9).
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
| issue #93: only a root-owned file or policy sets a key | 5.1, points 3 to 5 | an F source and a P source each failing one part of the test (file owner, file mode, a directory's owner, a directory's mode) are refused, F as a local refusal and P as incompatible |
| N8.2 survives a refused file | 5.5, point 4; 5.8, point 1 | a file with `min_class` for a cell and an unknown key elsewhere: the configuration is refused and that cell is still refused; the same with the refusal an unreadable or an untrusted drop-in, whose bytes are not read |
| one cell name | 5.3.1; 5.5, point 3 | two spellings of one cell refused |

## 10. Tests

Unit tests in `crates/ironafs-config/src/schema/tests.rs`; property tests
in `crates/ironafs-config/tests/it/`.

1. **Each key:** its default; one accepted value; each range bound
   accepted and one past it refused; the wrong syntax type refused;
   `dns_interface` values `.` and `..` refused.
2. **Unknown keys:** in F, refused with the suggestion of 5.9, point 2
   (for example `cells."x".protocl` suggests `protocol`); in P, ignored and
   reported, with more than `config.unknown_reported_max` counted.
3. **Security values:** the O19, point 6, test in P and in F, red then
   green.
4. **Layers:** the same key in two F sources refused, naming both; a P
   value locks the key; untrusted F and P sources refused (one case per
   part of the test, and a symbolic link as the file and as a directory
   on the path; a file that is not a regular file, such as a FIFO or a
   device, refused as untrusted without being opened); each cell of 5.1, point 7's
   table, for F and for P, with `managed_before` true and false; a missing
   `/etc/ironafs` gives absent for F and P; an empty policy directory is
   absent; a listed file gone at its `fstatat` or at its open is
   unreadable; `ENOENT` at `fstatat` on `ironafs.toml` is absent, and
   `EACCES` or `EIO` there is unreadable, not untrusted; an unreadable or
   untrusted drop-in, or drop-in directory, is handed over without bytes
   and refuses the local configuration, with the other sources' outcomes
   kept; a drop-in or policy directory with `config.sources_max` sources
   is read and with one more is refused (F) or incompatible (P), no file
   of it read; an untrusted
   policy directory gives incompatible and an unlistable one unreadable;
   a listing that returns a policy entry and then fails gives the
   directory's unreadable outcome first, with that entry's outcome kept,
   and the worst decides; policy files in
   two different failing states give the worst; an entry not named
   `*.toml`, or starting with `.`, is not read; F's sources taken in the
   order of 5.1, point 2, whatever the listing order; a refused reload
   followed by one that does not parse keeps the first one's cell
   refusals; a policy machine-wide floor with a
   local per-cell value for the same key: the policy's value covers the
   cell; an envelope key in F refused as unknown; a policy
   `dns_scope` with a local `dns_servers` uses neither local value (the
   scope group); `discovery.enabled = false` in one layer with a cell's
   `discovery = true` in another sends nothing for that cell.
5. **Tables:** an empty unknown table; `[cells."x".protocol]`;
   `network = 1`; an unknown subtree in P configures no cell.
6. **Versions and `required`:** a development build with any
   `min_client_version` gives incompatible; `has_value` for a `cells.*`
   path gives false when no configured policy cell sets the key, and true
   when one of two does.
7. **Cells:** two spellings of one cell refused; a cell in F and in P
   combined per key.
8. **Property:** any F document of keys valid in F, with valid values,
   each key once, and no two cell tables constructing one cell name,
   validates (a security key's value may refuse cells, never the file),
   and merging is deterministic: the same `LocalLayer`, `RefusalRules`,
   `PolicyInput` and `Running` give the same effective configuration,
   origins and refused cells.
9. **No partial use:** a refusal returns no layer (the types make a
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
- The DNS client (#60) or namespace (#80) specification changes a bound
  this page copies (`dns.targets_max`, `dns.name_servers_max`,
  `dns.explicit_servers_max`, `ns.sysname.count_max`, `ns.sysname.len_max`).
- The DNS client (#60) or namespace (#80) specification renames or
  changes a key this page lists.

## 12. Sources

| Id | Kind | Source, version or date, locator | Used in |
|---|---|---|---|
| ADR | DECISION | ADR 0004, C12, C27, C33, C34, L34, N3, N8, N27, O17, O18, O19, O31, and O50, points 1 and 3 (on `main`: M2a has the configuration parser, M2b the floor keys; Linux policy in M2a only as #53 amends O50, not on `main`) | throughout |
| AMD | DECISION | Issue #53, point 4 (C34, point 2; owner's decision, 2026-11-20), not on `main` at this draft | 5.2.3 |
| DEC-1 | DECISION | Issue #93, its owner's decisions of 2026-11-20 and its outcome | 1, 5.2, 5.4 |
| SYN | SPECIFICATION | [config-syntax.md](config-syntax.md) (draft, 2026-11-21) | throughout |
| DNS | SPECIFICATION | The DNS client specification, issue #60 (draft), 5.2, 5.6.3 and 5.6.4 | 5.2.2, 5.2.4 |
| NS | SPECIFICATION | The namespace specification, issue #80 (draft), 5.2 and 5.6.5 | 5.2.1, 5.2.6 |
| VLFS | SPECIFICATION | The location service specification, issue #65 (draft, 2026-11-21), 5.1.1, point 5 (the VL service's port, marked unverified there) | 5.3.3 |
| S1 | SPECIFICATION | RFC 4291, "IP Version 6 Addressing Architecture", section 2.2 (to be registered) | 5.3.3 |
| CTL | DECISION | The controller's rulings of 2026-11-21 on Q1 to Q12 | 13 |
| OWN | OWN | This page's own choices, each with its reason where it is made | throughout |

### 12.1 Observed behaviour

None.

## 13. Decisions taken, and what stays open

Q1 to Q12 are decided by the controller (CTL). Each stays a Working choice
of this page.

- **Q1. Table and key names:** the tables `cells`, `network`,
  `discovery`, `cache`, `namespace`, `security` and `policy`, and the key
  names of 5.2. The ADR fixes only the envelope's three key names and
  N3's floor-key names; the envelope's placement in a `policy` table is
  frozen with the first release (O19, point 1), which is why it is
  settled now.
- **Q2. Untrusted sources:** an untrusted F source refuses the local
  configuration, and an untrusted P source makes the policy incompatible
  (5.1, point 4). Reason: config-syntax.md, 8, point 1, names a writable
  configuration file as a misconfiguration; under the alternative (#60,
  5.6.4, point 6, where an untrusted file sets the other keys) a non-root
  user could set a cell's servers, its protocol identifier or the address
  families. #60's point is to be reworded to cite this rule.
- **Q3. A security value in a local file:** the cell is refused and the
  rest applies (5.4, point 4). Reason: N8, point 2, is Firm and not
  limited to policy; keeping the last good file would serve the cell
  under its old floor.
- **Q4. The cache size default:** 1 GiB, fixed (5.2.5). Reason: C27,
  point 8, leaves the number to the builder; a share of the disk would
  make behaviour depend on the host.
- **Q5. Host names in server lists:** refused; addresses only (5.3.3).
  Reason: resolving a configured name would send a query C33, point 5,
  may not allow, and make the server set depend on the resolver.
- **Q6. The interface-name rule:** the kernel's documented rule is to be
  registered and to replace 5.2.2's provisional set. **Still open in
  substance:** the rule is not yet registered. Until it is, the
  provisional set stands; it is stricter than the kernel's, so it fails
  closed.
- **Q7. The version format:** `MAJOR.MINOR.PATCH` only, and a build that
  is not a release meets no minimum (5.3.6). Reason: ordering
  pre-releases invites a policy that admits an untested build. No ADR
  fixes the release version's form.
- **Q8. A wrong type for a security key:** a type error (5.4, point 4).
  Reason: O19, point 7, separates schema validation from point 6's value
  failure.
- **Q9. Reserved floor keys:** all six N3 keys, per cell and machine-wide,
  and `security.fips` (5.2.2, 5.2.7). Reason: O19's "Why" ("An older
  client that skipped a key it did not understand would run below that
  floor"). Cost: a site cannot state those keys in a policy M2a clients
  also read, acceptable for M2a, which is not for production use.
- **Q10. Drop-ins that override:** refused (5.6, point 3). Reason: the
  defaults live in the code, so no drop-in needs to override a packaged
  value, and issue #93 names silent weakening as a failure case.
- **Q11. A refused local configuration at start:** the daemon does not
  start, and says why (section 7). Reason: defaults could turn discovery
  on where the refused file turned it off (C33, point 5). Part (e) applies
  it.
- **Q12. A policy file that does not parse:** incompatible; an I/O
  failure is unreadable (section 7). Reason: a malformed delivery is never
  answered with an older copy (O19, point 7). Part (c) applies it.

Open: Q6's registration (above), and the cross-specification notes for
#60 (5.6.4, point 6; Q2) and #80 (5.2, point 2; 5.2.6, point 2).

### 13.1 Blockers before this page leaves draft

The first four findings were open when the review's fix rounds stopped;
the fifth came from the security review. Each fix is now in the text; the
page stays draft, and no code is written from it, until a scoped
re-review of each fix passes. Each is quoted as the reviewer wrote it.

1. "The per-source table gives no precedence when policy sources fail in
   different ways. It also treats a single missing policy file as absence
   of the whole policy. The part (c) interface still lists no explicit
   `managed_before` or typed input for a trust failure that occurs before
   bytes are available."
   - Fix: across policy files the worst state decides; a file listed but
     gone at its open is unreadable, and only a missing or empty policy
     directory (or a missing directory above it) is absent; part (c)'s
     input is the list of outcomes, for the policy directory when the walk
     or listing fails there (also after the listing has returned
     entries) and for each file, plus `managed_before`.
   - In: 5.1, point 7; 5.8, points 1 and 2.
2. "The walk pins each opened component, and listing uses a checked
   directory descriptor. The text still permits opening listed files
   through a fresh walk that reaches a replacement directory."
   - Fix: each listed file is opened with `openat` relative to the very
     directory descriptor that was checked and listed, never by a new walk.
   - In: 5.1, point 3.
3. "The final open requires only `O_NOFOLLOW`. The walk does not require
   a regular file or prevent blocking before it can check the
   descriptor."
   - Fix: `fstatat` with `AT_SYMLINK_NOFOLLOW` on the checked directory's
     descriptor must show a regular file before the open, so a special
     file is untrusted without being opened; the file is then opened with
     `O_NOFOLLOW | O_NONBLOCK | O_NOCTTY | O_CLOEXEC`, and `fstat` must
     again show a regular file, else the source is untrusted, before any
     byte is read.
   - In: 5.1, point 3, and its test in 10, point 4.
4. "The configured-cell rule excludes an implicit parent created solely by
   ignored policy content. The validation section still checks every cell
   segment and compares cell tables for aliases. The `required` interface
   still quantifies over every cell table."
   - Fix: 5.5, points 2 and 3, check the segment of, and compare, only
     configured cells (5.2.2, point 1); 5.8, point 2's `has_value` counts
     configured cells only.
   - In: 5.5, points 2 and 3; 5.8, point 2.
5. "The F-side interface cannot express an unreadable or byte-less
   untrusted source, so the spec's own N8.2-survival invariant cannot be
   implemented."
   - Fix: `validate_local` takes F's outcomes as the caller found them,
     each `Unreadable`, `Untrusted`, `TooMany` or a trusted file's bytes,
     for a file or for the directory where the walk or listing failed;
     the caller collects them without stopping at the first failure;
     unreadable and untrusted sources never carry bytes; every trusted
     source that parses is still scanned for refusal rules; section 7's
     local refusal names an unreadable source or directory.
   - In: 2, point 2; 4 ("Source"); 5.1, point 2; 5.5, point 4; 5.8, point
     1; 7; 9; 10, point 4.

## 14. Review

[2026-11-21-m2a-93-config-schema](../reviews/2026-11-21-m2a-93-config-schema.md).
