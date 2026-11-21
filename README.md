<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# IronAFS

[![Project status: Concept, minimal or no implementation yet](https://www.repostatus.org/badges/latest/concept.svg)](https://www.repostatus.org/#concept)
[![Licence: GPL-3.0-or-later](https://img.shields.io/badge/licence-GPL--3.0--or--later-blue)](LICENSING.md)
[![Built with AI models from five families](https://img.shields.io/badge/AI--assisted-5%20model%20families-informational)](AI.md)

A modern AFS client, written in Rust, with one shared core for every
operating system and no kernel code of its own.

> [!IMPORTANT]
> **Design phase: not ready for use.** There is no working client and no
> release yet. This repository holds the decisions, the rules and the plan.
> Progress is tracked in the
> [milestones](https://github.com/ironafs/ironafs/milestones).

## Why

The widely deployed AFS clients depend on kernel components: a kernel module
on Linux, a kernel extension on macOS, a driver on Windows. (OpenAFS also
includes an experimental FUSE-based client.) Kernel code is getting harder
to ship everywhere.

- **On macOS,** third-party kernel extensions need Reduced Security on Apple
  silicon, set by the user in Recovery. Apple's user-space alternative for
  file systems is FSKit.
- **On Linux,** the kernel keeps no stable interface for modules, some
  symbols are only for GPL-tagged modules, and on Debian, for example,
  Secure Boot blocks unsigned modules built outside the kernel tree. FUSE
  lets a file system run in user space instead. For per-user correctness,
  IronAFS starts with FUSE's entry and attribute caching off and uses
  neither passthrough nor the writeback cache, so it expects
  metadata-heavy work to be slower than it could be, until measured.
- **On Windows,** new kernel drivers must, with some exceptions, be signed
  through Microsoft, and current releases are removing trust in older
  cross-signed drivers. IronAFS plans a small SMB server of its own on the
  local machine instead, reached through Windows' own SMB client (see
  below).

All three systems offer a way to expose files without third-party kernel
code: FUSE and FSKit as supported filesystem interfaces, and on Windows, if
the gate passes, Windows' own SMB client talking to a local server. IronAFS
is a client built for that world. The sources are in two research notes:
[kernel code against user space](docs/research/2026-11-20-kernel-code-and-user-space.md)
and [AFS clients without kernel code](docs/research/2026-11-18-driverless-afs-clients.md).

## Goals

- **Do what users of an AFS client rely on.** `/afs`, tokens and `aklog`,
  the `fs` commands, mount points, ACLs and volume access, against existing
  cells; then add what a client written today can. Where IronAFS will
  differ (no PAGs; programs in AFS never gain set-uid privileges),
  [ADR 0004](docs/adr/0004-architecture.md) says so.
- **Share as much as possible.** One core does everything that is not
  operating-system specific; each platform adds only a thin layer.
- **No kernel code of our own.** FUSE on Linux, FSKit on macOS, and on
  Windows a small SMB server of our own on the local machine, reached through
  Windows' own SMB client. Other systems with FUSE, or a similar interface,
  can follow.
- **Safe by construction.** Everything that parses data from the network is
  safe Rust, with no `unsafe` and no panics on bad input.
- **Few dependencies, all of them readable.** IronAFS writes its own protocol
  code (Rx, XDR, the FUSE wire protocol) and its own plumbing. In shipped
  code, third-party crates are used only where writing them ourselves would
  be less safe: mainstream cryptography, and each operating system's own
  interfaces. Testing adds one planned exception, the fuzzing engine. Every
  dependency is a recorded decision by the maintainer. The reasons are
  supply-chain security and code anyone can audit (rule 21 of the
  [coding standards](docs/coding-standards.md)).

## The planned design

Three thin platform layers (FUSE on Linux, FSKit on macOS, a local SMB
server on Windows) sit on one shared Rust core, which the `afs` command line
uses too:

```
                 afs command line
                        |
   +--------------------------------------------+
   |  IronAFS core (Rust, shared by all)        |
   |  Rx and its security classes, XDR,         |
   |  protocol clients, cache manager,          |
   |  tokens, configuration                     |
   +--------------------------------------------+
        |                |                 |
   FUSE layer      FSKit layer       SMB layer
     (Linux)         (macOS)          (Windows)
```

This is the intended shape, not code that exists. The architecture is
decided in [ADR 0004](docs/adr/0004-architecture.md), accepted on
2026-11-20.

**Windows, planned:** an online `\\afs` path through Windows' own SMB client to
an SMB 3.1.1 server of IronAFS's own on the local machine, behind a gate on
identity, security, application compatibility and coherency. What has been
measured is the transport: Windows' SMB client reaching a listener on the
local machine beside Windows' own file sharing, on Windows 10 22H2 and
Windows 11 24H2 and 26H2, to the client's first packet. A full SMB session has not been
measured yet ([the UNC note](docs/research/2026-11-19-windows-unc-without-a-driver.md)).
A Cloud Files replica may follow later as an optional offline companion,
and becomes the Windows surface if the gate fails; if both fail,
Windows has no surface until one passes. ADR 0004 records this
decision ([part 2](docs/adr/0004-architecture-2-os-layers.md), L15).

**Planned platforms** ([part 2](docs/adr/0004-architecture-2-os-layers.md),
L37 to L39):

- tier 1: Ubuntu 24.04 and 26.04 (x86_64 and aarch64), on the kernels of
  GitHub's hosted runners; macOS 27 on Apple silicon; Windows 11 Pro 26H2
  (x64), in a workgroup at first;
- tier 2: Windows 11 Pro 26H2 (Arm64), in a workgroup at first; Windows 10
  Pro 22H2 (x86_64), in a workgroup, SMB only, with Extended Security
  Updates, until 2027-10-12;
- candidates: Ubuntu's generic kernels; RHEL 7.9, 8.10, 9 and 10; Debian 12
  and 13; other Windows 11 versions and editions; static musl builds; and
  FreeBSD. Each becomes a target only on the conditions L39 names, such as
  a named test venue that qualifies it.

**Order of work** ([part 3](docs/adr/0004-architecture-3-operations.md),
O50). A plan, with no dates:

1. M2a: the core and Linux, read-only and unauthenticated; a small Windows
   SMB identity spike runs beside it.
2. M2b: rxkad, the security class existing cells use, with `aklog`.
3. M3: writes and locks on Linux.
4. M4: macOS.
5. M5: Windows.
6. M6: rxgk and other security work.
7. M7: proposed later work.

## Compatibility

IronAFS is being designed to speak the AFS-3 protocol family (Rx, with its
security classes) and to work with existing AFS cells. Nothing is tested yet.
OpenAFS servers are the first test target. Other AFS-3 servers, AuriStor
included, are a goal, and each is listed as supported only once it is tested.

## How IronAFS is built

One person builds IronAFS with AI models from **five families**. Each has a
fixed role, and **no required review is ever done by the family that wrote
the change**: models from other families review it, and they tend to miss
different things. The more a mistake could cost, the more independent
reviews a change needs ([docs/workflow.md](docs/workflow.md)).

| Family | Models | Role |
|---|---|---|
| Anthropic | Claude Opus 5.5; Claude Mythos 5.2 for security reviews, judgements and architecture decisions | Plans, routes and checks the work; reviews |
| OpenAI | GPT-6.2 Sol (through codex) | Writes the code; design and research reviews |
| Zhipu | GLM 5.3 | First reviewer of codex's code |
| Google | Gemini 4 Carbon | Second-family reviews |
| DeepSeek | DeepSeek V4.1 Flash | Second-family reviews |

Model versions change; [AI.md](AI.md) always has the current list.

**The human stays in charge.** Every commit that lands on `main` is shown to
the maintainer with its exact message, and made only after their approval.
Every commit on `main` says in its trailers which models wrote it and which
reviewed it.
Every decision carries its full review history, findings included.
[AI.md](AI.md) explains how to read it all.

**Contributors may use AI too.** Disclose it in the commit trailers, write
your own sign-off, and answer for the change
([CONTRIBUTING.md](CONTRIBUTING.md)).

## Where to start reading

- [docs/adr/](docs/adr/README.md): the decisions, each with its full review
  history.
- [AI.md](AI.md): how AI models are used to build IronAFS, and how commits
  credit them.
- [docs/workflow.md](docs/workflow.md): how a change is classed, made and
  reviewed.
- [docs/coding-standards.md](docs/coding-standards.md): the rules for Rust
  code, tests and layout.
- [docs/research/](docs/research/): the research the decisions rest on.

There is more process here than code, on purpose. A filesystem client parses
untrusted network data and handles users' credentials, so the rules come
first. To contribute, [CONTRIBUTING.md](CONTRIBUTING.md) is all you need to
read.

## Building

```sh
cargo build
cargo run --bin afs
```

Today this builds a placeholder `afs` binary that prints its version.
Development uses the exact toolchain pinned in `rust-toolchain.toml`
(1.100.0). `rust-version` in `Cargo.toml` (1.100) is the minimum. A lower
minimum is set only once CI tests it.

Run `cargo xtask check` for the local checks (install
[cargo-nextest](https://nexte.st/) first).
[CONTRIBUTING.md](CONTRIBUTING.md) explains the optional git hooks and how
to run each check on its own. CI runs the tests on Linux, and Clippy for
Linux, Windows and macOS.

## Contributing

Questions, ideas and bug reports are welcome as
[issues](https://github.com/ironafs/ironafs/issues). Pull requests are
welcome too: [CONTRIBUTING.md](CONTRIBUTING.md) explains how a change gets
in, including how to disclose AI use. Everyone taking part follows the
[code of conduct](CODE_OF_CONDUCT.md).

## Security

Please report vulnerabilities privately: see [SECURITY.md](SECURITY.md).

## Licence

Copyright © 2026 Adrian Ana. IronAFS is free software under the
[GNU General Public License, version 3 or later](LICENSE)
(`GPL-3.0-or-later`). You may use, study, change and share it. If you
distribute IronAFS, or a work based on it, you must license it under the
same terms and make its complete source available to the people you
distribute it to, by one of the methods the GPL allows. (Shipping it beside
other, separate programs does not put those programs under the GPL.) That
way IronAFS, and every version of it that is distributed, stays free and open
source, and improvements can flow back to its users and the wider community.
[LICENSING.md](LICENSING.md) has the details.

IronAFS contains no code from OpenAFS, AuriStor or the Linux kernel's AFS and
Rx implementations ([ADR 0001](docs/adr/0001-licence-and-dont-copy.md)).

AFS is the Andrew File System. IronAFS is not affiliated with or endorsed by
IBM, the OpenAFS Foundation or AuriStor. OpenAFS, AuriStor and other names
used here are trademarks of their respective owners, used only to identify
software discussed in this documentation or intended as interoperability
targets.
