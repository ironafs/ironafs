<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Licensing

IronAFS is free software under the
[GNU General Public License, version 3 or later](LICENSE)
(`GPL-3.0-or-later`). The decision and its reasons are in
[ADR 0001](docs/adr/0001-licence-and-dont-copy.md), and the research behind it in
[docs/research/2026-11-18-licence-options.md](docs/research/2026-11-18-licence-options.md).

- **You may** use, study, change and share IronAFS.
- **If you distribute it,** or a work based on it, license it under the same
  terms and make its complete corresponding source available to your
  recipients, by one of the methods GPLv3 §6 allows.
- **Changes you only use yourself,** or inside your organisation, need not be
  published.
- **Separate programs** merely shipped alongside IronAFS (an aggregate) are
  not covered by its licence (GPLv3 §5).

## The name and logo

GPL-3.0-or-later covers the project's code and documentation, and grants
no trademark rights in the name or the logo. IronAFS™ and the IronAFS logo
are trademarks of Adrian Ana;
[TRADEMARKS.md](TRADEMARKS.md) explains how to refer to the project and to
modified versions, without limiting any right the licence gives.

## File headers

Every file declares its copyright and licence in the form of the
[REUSE](https://reuse.software/) specification, version 3.3, with SPDX tags
and no licence boilerplate. The header is exactly two lines at the top of
the file, one tag per line, in the file's own comment syntax:

```rust
// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later
```

```markdown
<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
```

- **TOML, YAML and shell** use `#`. A shebang line, if any, stays first.
- **New files:** copy the two lines from a neighbouring file and change the
  name, email and year. (`reuse annotate` writes a different layout, so it
  is not used.)
- **Contributors** may add their own `SPDX-FileCopyrightText` line, with the
  year and the name they sign off with, to a file they create or change
  substantially. Existing lines are never removed or changed.
- **Files that cannot carry a header** (generated files, JSON, files whose
  front matter must come first) are covered by an entry in `REUSE.toml`.
- **Licence texts** are in `LICENSES/`, named by their SPDX identifier.
- **Third-party material** keeps its original header and licence, and gets a
  `docs/provenance.md` row.
- **Checked by** `reuse lint`, which must pass before any change lands. CI
  runs it on pull requests. The opt-in pre-push hook runs it if `reuse` is
  installed.

## Where the code comes from

IronAFS contains no code from OpenAFS, AuriStor or the Linux kernel's AFS and
Rx implementations. It is written from published specifications and observed
wire behaviour, under the don't-copy rule in
[ADR 0001](docs/adr/0001-licence-and-dont-copy.md).

Third-party material copied into the repository, if any, keeps its own
licence and notices and is listed in [docs/provenance.md](docs/provenance.md).
Dependency licences follow ADR 0001: an allowlist in `deny.toml`, checked
with `cargo-deny`, plus per-package exceptions the owner decides. The checks
run in CI.
