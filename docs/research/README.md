<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Research notes

The research that IronAFS's decisions rest on. Each note is dated, cites its
sources, and marks how each claim is known:

- **EXECUTED:** run, with the revision, the machine, the date and the result;
- **INSPECTED:** read in a cited source;
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
