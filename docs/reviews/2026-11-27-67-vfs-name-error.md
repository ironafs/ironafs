<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# PR #206 review record

## Review target

PR #206 is Class S and part of #67. This record summarizes the reported review and execution evidence for signed source A.

| Field | Value |
| --- | --- |
| Reviewed source A | `b17c70a3bfb7117020666750fd1b4d497e45388c` |
| Base | `b787a58dadc95f2f00f5e13195a665944dad9c69` |
| Tree | `7d70b7e92f6e43b15db57634288068d3637a67bc` |

## Design and authoring

Anthropic Opus 5.5 High, served as `claude-opus-5-5-high` through Antigravity, authored the Class S design. Anthropic Opus 5.5, served as `claude-opus-5-5` at High through the Anthropic code lane, authored the standalone wiring and corrected the single root workspace dependency row. The carried source and tests also originated with Anthropic Opus 5.5 and retain their exact #130 blobs.

The brief records six formal pre-code review dispatches across three exact design revisions, with two seats per revision.

- `0b9bbb86` received **REVISE** on evidence definition and sequence from Responses API `gpt-6.2-sol` at xhigh and **PASS WITH MINORS** from GLM 5.4 High through Z.ai Coding Plan.
- `4934159f` addressed those findings. Responses API `gpt-6.2-sol` at xhigh and GLM 5.4 High through Z.ai Coding Plan both returned **PASS**. The initial standalone wiring then exposed Cargo 1.100 unused workspace dependency warnings under the denied-warning build and standalone `check-bans`. The owner approved Class S scope amendment `fc597c23` to move the one workspace dependency row to its first consumer. That approval did not authorize merge.
- `bc305f32` cleared both seats before the isolated row-only correction. Responses API `gpt-6.2-sol` at xhigh returned **PASS**, and GLM 5.4 High through Z.ai Coding Plan returned **PASS WITH MINORS**.

## Review seats

| Seat | Model and effort | Service | Result | IronAFS calibration |
| --- | --- | --- | --- | --- |
| First independent consistency and source | `glm-5.4` high | Z.ai Coding Plan | PASS WITH MINORS | Code-calibrated |
| Second independent source | `gpt-6.2-sol` xhigh | OpenAI Responses API | PASS | Uncalibrated on IronAFS for this source seat |
| Security | `gpt-6.2-astra` xhigh | OpenAI Responses API | PASS | Uncalibrated on IronAFS for this seat |

GLM and GPT are distinct review families from the Anthropic author. The two GPT models differ and share a family. Headroom was not reported. Prior review history fills no seat for A.

## Scope and source checks

The `ironafs-vfs` crate uses `no_std` with `alloc` for raw names and a closed semantic error vocabulary. It exports `Name`, `NameError`, `MAX_NAME_BYTES`, `linux`, `VfsError`, and `Surface`.
Cargo discovers the package through the existing workspace member glob. The root `Cargo.toml` is byte-identical to base. `Cargo.lock` adds one package entry. There are no production dependencies. Existing test support is dev-only.
The change adds no capabilities or layer errno translation and makes no mounting or runtime claim.

Original #130 and #135 retain their gates. Parent #130 remains parked. The #135 consumer owns the root Cargo row under its separate scope.

- `Name::new` in `crates/ironafs-vfs/src/name.rs` at lines 45–64 checks the 255-byte cap and rejects empty names, dot, dot-dot, slash, and NUL before the sole sized copy.
- The Linux wrapper in `crates/ironafs-vfs/src/name/linux.rs` at lines 22–29 delegates validation and returns the original bytes. The controller checked that it imports the local `Name` and `NameError`, that `Name` storage uses `alloc`, and that production code does not use `std`.
- The incomplete-match compile-fail doctest in `crates/ironafs-vfs/src/error.rs` is separate from exhaustive unit coverage of all 29 positions and their surfaces. The unit coverage includes four permission cases, `OutOfRange` as invalid argument, and 24 self-mappings.
- GLM and the controller matched 35 source function declarations plus the illustrative compile-fail function and the focused counts of 22 unit tests, 2 integration tests, and 1 compile-fail doctest.
- The historical literal 255/256 boundary test and the macro-generated error-table oracle were retained.

## Carried blobs

These seven source and test blobs were carried byte-identically from #130 at `47c3bd9214ec72e2053060abcc05bd1a16f65fb0`. Each retained mode `100644`.

| Repository-relative path | Blob ID |
| --- | --- |
| `crates/ironafs-vfs/src/error.rs` | `7f40373f7707053efad7a1beaa2252d6731c8ea3` |
| `crates/ironafs-vfs/src/error/tests.rs` | `4278cdbba592522e26e95475bb6b46b40715eed7` |
| `crates/ironafs-vfs/src/name.rs` | `97a7a6911e037465e52bee317f7067ff3271f36d` |
| `crates/ironafs-vfs/src/name/linux.rs` | `c8747ee0b68d9a4863eadd8f29972c3f5e0e6be3` |
| `crates/ironafs-vfs/src/name/linux/tests.rs` | `176023adcdbd3e213f168dab23c726b10445ab73` |
| `crates/ironafs-vfs/src/name/tests.rs` | `5fe2dbd8d1ca467d8719f1c4d1382bf365af6b44` |
| `crates/ironafs-vfs/tests/it/name.rs` | `f8c375c972116e80ff53b9d4b473326b4ed445e7` |

## GLM minor observations

`gpt-6.2-sol` xhigh independently checked all three observations through the OpenAI Responses API.

| Observation | Location | Origin | Kind | Verified disposition |
| --- | --- | --- | --- | --- |
| Test naming | `crates/ironafs-vfs/src/error/tests.rs` line 136 | Inherited and unchanged | `other` | Closed. The test also covers `NotYetStored`. The coverage was clarified and the extra assertion is valid. |
| Proposed extra test | `crates/ironafs-vfs/src/name.rs` line 101 | Inherited and unchanged | `other` | Closed. The six messages already satisfy rule 4. A dedicated `Display` test is not required. |
| Proposed maintainability change | `crates/ironafs-vfs/src/name.rs` line 109 | Inherited and unchanged | `other` | Closed. The literal `255` matches the pinned VFS contract and the exact output value is correct. |

The independent readback closed all three observations and established no current correctness or contract defect. No source revision followed these reviews. Adding this record is review-only bookkeeping.

## Execution on A

| Check | Reported result |
| --- | --- |
| Local Nextest | 1102 tests passed |
| Local workspace doctests | 47 passed |
| Local focused tests included in those runs | 25 passed (22 unit, 2 integration, 1 compile-fail) |
| Direct `rustc` check of the extracted exact doctest | Only `E0004` |
| [Hosted execution](https://github.com/ironafs/ironafs/actions/runs/46045606469) | 17/17 passed on A |

The locked workspace build and Clippy passed with warnings denied. Formatting, the normal dependency graph, exact test discovery, seams, purity, `check-bans`, and `xtask check` including REUSE and deny also passed.
The controller corrected the draft description to add the required draft trailers and reran CI at the same code SHA. Only the corrected hosted run is counted here.
