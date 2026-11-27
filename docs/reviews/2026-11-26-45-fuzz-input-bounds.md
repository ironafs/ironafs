<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record for #45 tracked seed replay bounds

[Pull request #199](https://github.com/ironafs/ironafs/pull/199) implements the Class I engine input bound for the tracked XDR and configuration seed replays. The fdpass model has no declared total input cap. The separate stable ordinary-test replay and scheduled fuzzing work remain open.

| Scope | Seat | Result |
| --- | --- | --- |
| `05a038c5e025379dd5e23e06b647dbe198d28454..6f91f7717c0efe7d0822518ef803b47891e168e1` | GLM `glm:glm-5.4`, high | PASS, three diagnostic or maintenance notes. Exact verdict and range returned after the original review reached its turn limit and resumed in the same session. |
| `6f91f7717c0efe7d0822518ef803b47891e168e1..2b7a5ccdb93a3a24c483ba605daff83587867033` | GLM `glm:glm-5.4`, high | PASS, zero findings. The original result retained its verdict and the session read back the exact range. |

Both reviews used the served GLM 5.4 High code-review pin. The lane reported the off-peak window and omitted quota headroom. The controller matched the full ranges, scope digests, served-model evidence and readbacks. The accompanying review record and index edit carry those results without altering their reviewed code.

The independent consistency pass found one engine replay invocation, the existing regular tracked-seed selection, the same toolchain and argument separator, and a length check before the engine can truncate an explicit input. It compared the hard-coded caps to the two governing specs. The controller checked the same function inventory. `replay_command` reuses the existing fuzz command setup. `engine_limit` keeps the two declared replay bounds together. `replay` keeps its captured-output and refusal pattern. The fixture derives its scratch root from the actual test executable because unit tests do not receive the self-test workspace's `target_dir`. It uses the existing fixture creation and teardown. [Testing D9](../specs/testing.md) · [Configuration cap](../specs/config-syntax.md#52-caps) · [XDR fuzz target](../specs/xdr.md#104-fuzz-target)

- **Round 1, new diagnostic choice, other.** The oversized-seed refusal prints the target and cap only. This deliberately keeps the seed out of that diagnostic. The first over-limit input is sufficient to reject the target before engine launch.
- **Round 1, new diagnostic choice, other.** An I/O error reading size fails the whole check using its existing I/O pattern. Subsequent targets cannot be credited by a partial report.
- **Round 1, new mapping, other.** The literal-name map covers both existing declared caps. The fdpass case and the existing behavior for uncapped targets are verified. Future targets with a declared cap need their own table entry.
- **Controller finding, introduced in the initial boundary test, correctness.** The test used the shared default temp root covered by [#177](https://github.com/ironafs/ironafs/issues/177). The one-site correction places it in the build output and keeps the same boundary assertions. The scoped GLM verification closed the finding.

**EXECUTED.** At the first code checkpoint all 30 existing tracked seeds ran on the pinned nightly. The local suite passed 1,040 tests and 46 doctests. At the scratch-path checkpoint all 19 focused fuzz tests passed, including the inert cap and cap-plus-one controls. The focused test passed with direct Cargo and with nextest using a custom target directory. The scratch parent was under that build's wrapper output, and the test removed its temporary run directory. The workspace suite passed 1,040 tests and 46 doctests. One unrelated crypto test was marked leaky in the full run and passed without that flag in isolation. The offline aggregate, formatting, Clippy, REUSE and dependency checks passed. Hosted evidence is linked from the PR.

Written by codex:gpt-6.2-sol.
