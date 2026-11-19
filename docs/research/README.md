<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Research notes

The research that IronAFS's decisions rest on. Each note is dated, cites its
sources, and marks how each claim is known:

- **EXECUTED:** run, with the revision, the machine, the date and the result;
- **INSPECTED:** read in a cited primary source (documentation,
  specifications, release notes, licence texts);
- **RELAYED:** read only in a secondary source, such as a forum post, an
  issue on a third-party tracker or a mailing-list post;
- **MEASURED:** measured outside this repository, in the owner's earlier
  work, with the conditions stated; evidence for design, not proof that
  IronAFS works;
- **INFERRED:** a design conclusion drawn from labelled facts;
- **NOT VERIFIED:** stated, but not checked.

A note is re-checked by a second model family before a decision rests on it
(the `phase-lifecycle` skill).

**ADR 0001's don't-copy rule applies.** Public specifications, documentation
and observed behaviour are valid inputs for IronAFS code. Anything a note
learned by reading another AFS implementation's source is marked "(from
source)" and is background only, never an input for writing IronAFS code.

## Notes

| Note | What it covers |
|---|---|
| [2026-11-18-driverless-afs-clients](2026-11-18-driverless-afs-clients.md) | Why a client without kernel code is possible now; the macOS, Linux and Windows options; licences of the existing implementations; protocol documentation |
| [2026-11-18-project-layout-and-tests](2026-11-18-project-layout-and-tests.md) | Workspace and module layout, unit and integration test placement, fuzzing layout, checked against primary sources |
| [2026-11-18-platform-support](2026-11-18-platform-support.md) | Support floors for Linux (including RHEL 7), macOS and Windows, and a proposed support matrix |
| [2026-11-18-licence-options](2026-11-18-licence-options.md) | Licence options against the project's goals, compatibility, distribution, contribution models and precedents |
| [2026-11-19-macos-options](2026-11-19-macos-options.md) | macOS options using only what Apple ships: FSKit by version, File Provider, loopback NFSv3 and NFSv4, SMB and WebDAV; a comparison, a recommendation per macOS version and the acceptance cases |
| [2026-11-19-windows-cloud-files](2026-11-19-windows-cloud-files.md) | Cloud Files against ProjFS for an AFS client on Windows: caller identity, per-user access, hydration, writing, failure modes and open problems |
