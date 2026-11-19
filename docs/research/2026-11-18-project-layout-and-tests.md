<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Project layout and test organisation: checking the decisions

Research note, 2026-11-18. All web sources were accessed on 2026-11-18. It
supports coding standards rules 15 and 20.

## Answer

The layout and test decisions hold against the current Cargo Book, Rust
Reference, Clippy, rustc-dev-guide and nextest documentation, with one
exception: fuzz-target placement. Cargo does not accept a per-crate `fuzz/`
directory under `members = ["crates/*"]` without extra configuration; one
top-level `fuzz/`, excluded from the workspace, works. The checks also found
three gaps:

- No Clippy lint enforces out-of-line unit tests.
- Clippy's test allowances do not cover helpers in `tests/it/support.rs`
  unless the test crate is marked `#![cfg(test)]`.
- A crate's binary target gets its own, usually empty, unit-test binary.

There is no single best practice for unit-test placement. The one normative
source (rust-lang/rust) prescribes out-of-line tests. The Rust Book and
`cargo new` describe inline tests as the convention. The projects read for
this note that moved tests out did so for large files.

## Method and limits

- Quotes were taken through a page-to-text fetch. Wording is as returned;
  punctuation may differ slightly from the page. Three sources were read raw
  and are marked "(raw)".
- EXECUTED means a command run in a scratch workspace outside the repository,
  with cargo 1.97.1, cargo-nextest 0.9.143 and cargo-fuzz 0.13.2, on macOS
  (aarch64), on 2026-11-18. The scratch workspace had two crates: `demo-core`
  (a library, a `src/bin/demo/main.rs` and `tests/it/`) and `demo-cli`
  (depending on `demo-core`).
- Commands were run only on mechanical points where the documentation is
  silent or ambiguous.
- No count of inline versus out-of-line tests across projects was made, and
  none is quoted.
- Could not open (HTTP 404): the Cargo fingerprint module documentation
  (`doc.rust-lang.org/stable/nightly-rustc/cargo/core/compiler/fingerprint/`),
  and `github.com/openai/codex/blob/main/AGENTS.md`. Nothing here relies on
  either.

## Verdict table

| # | Claim | Verdict | Quoted source | URL |
|---|---|---|---|---|
| 1a | Virtual workspace root | CONFIRM | "a `Cargo.toml` file can be created with a `[workspace]` section but without a `[package]` section. This is called a virtual manifest. This is typically useful when there isn't a 'primary' package" | https://doc.rust-lang.org/cargo/reference/workspaces.html |
| 1b | A virtual workspace must set `resolver` | CONFIRM (the root sets `resolver = "3"`) | "`resolver` must be set explicitly in virtual workspaces as they have no `package.edition` to infer it from" | same |
| 1c | Commands at the root cover all members | CONFIRM | "In the case of a virtual workspace, all members will be used (as if `--workspace` were specified on the command-line)." | same |
| 1d | Flat `crates/`, directory name equals crate name, prefix kept | CONFIRM (opinion source, not normative) | "Don't succumb to the temptation to strip common prefix from folder names." "tree structure tends to deteriorate over time, while flat structure doesn't need maintenance." | https://matklad.github.io/2021/08/22/large-rust-workspaces.html |
| 1e | `[workspace.package]`, `[workspace.dependencies]`, `[workspace.lints]` inheritance | CONFIRM | "These keys can be inherited by defining them in the member package with `{key}.workspace = true`." "Dependencies from this table cannot be declared as `optional`" | https://doc.rust-lang.org/cargo/reference/workspaces.html |
| 2a | `src/foo.rs` with a sibling `src/foo/` is valid | CONFIRM | "Ancestor module path components are directories, and the module's contents are in a file with the name of the module plus the `.rs` extension." | https://doc.rust-lang.org/reference/items/modules.html |
| 2b | `foo/mod.rs` is the non-preferred form | CONFIRM | "Prior to `rustc` 1.30, using `mod.rs` files was the way to load a module with nested children. It is encouraged to use the new naming convention as it is more consistent, and avoids having many files named `mod.rs` within a project." | same |
| 2c | Edition Guide on `mod.rs` | CONFIRM | "In Rust 2018 the restriction that a module with submodules must be named `mod.rs` is lifted." | https://doc.rust-lang.org/edition-guide/rust-2018/path-changes.html |
| 2d | Enforced by Clippy `mod_module_files` | CONFIRM (`restriction` group, since 1.57.0) | (raw) "Checks that module layout uses only self named module files; bans `mod.rs` files." | https://github.com/rust-lang/rust-clippy/blob/master/clippy_lints/src/module_style.rs |
| 2e | The lint fires in practice | CONFIRM. EXECUTED: `cargo clippy --all-targets` with `mod_module_files = "warn"` and a `src/bar/mod.rs`. | "warning: `mod.rs` files are not allowed, found `crates/demo-core/src/bar/mod.rs`" | executed |
| 2f | `self_named_module_files` is the opposite lint | CONFIRM. Never enable both. | (raw) "Checks that module layout uses only `mod.rs` files." | same file as 2d |
| 2g | A split only adds files | CONFIRM, by the rule in 2a: `foo.rs` keeps its path when `foo/child.rs` is added. Under the `mod.rs` form the same split renames `foo.rs` to `foo/mod.rs`. | "It is not allowed to have both `util.rs` and `util/mod.rs`." | https://doc.rust-lang.org/reference/items/modules.html |
| 3a | Lib plus bin: binary under `src/bin/<name>/` | CONFIRM | "If a binary, example, bench, or integration test consists of multiple source files, place a `main.rs` file along with the extra modules within a subdirectory of the `src/bin`, `examples`, `benches`, or `tests` directory. The name of the executable will be the directory name." | https://doc.rust-lang.org/cargo/guide/project-layout.html |
| 3b | A binary is also a test target | AMEND: set `test = false` on thin binaries. EXECUTED: `cargo test` printed `Running unittests src/bin/demo/main.rs` with 0 tests. | "The default is `true` for lib, bins, and tests." | https://doc.rust-lang.org/cargo/reference/cargo-targets.html |
| 4a | `#[cfg(test)] mod tests;` in `src/foo.rs` resolves to `src/foo/tests.rs` | CONFIRM, by the rule in 2a. EXECUTED: `cargo test` ran `foo::tests::two_is_two` from `src/foo/tests.rs`. | as 2a | https://doc.rust-lang.org/reference/items/modules.html |
| 4b | The Rust project requires out-of-line unit tests, enforced by tidy | CONFIRM | "All standard library and compiler unit tests are placed in separate `tests` file (which is enforced in tidy)." | https://rustc-dev-guide.rust-lang.org/tests/intro.html |
| 4c | The reason is rebuild cost | CONFIRM | "This ensures that when the test file is changed, the crate does not need to be recompiled." | same |
| 4d | The mechanism | CONFIRM | ehuss: "during a normal build, Cargo will ignore any changes to the `tests.rs` file." | https://github.com/rust-lang/rust/pull/110619 |
| 4e | Editing tests rebuilds only the test target | CONFIRM, with a limit. EXECUTED: after editing `src/foo/tests.rs`, `cargo build -v` reported both crates `Fresh`; `cargo test --no-run -v` reran `rustc --test` for the library and left `demo-cli` `Fresh`. After editing `src/foo.rs`, both builds of the library reran and `demo-cli` became `Dirty`. | Limit, Mark-Simulacrum: "we would still do ~2x the work since we'd compile both the cfg=test crate and the non-cfg=test crate." | executed; https://github.com/rust-lang/rust/pull/110619 |
| 4f | History of the rule | CONFIRM. Issue opened 2019-05-23; tidy check merged 2019-07-28; migration merged 2019-08-02, excluding libstd. | "I haven't touched libstd though, it had a lot of tests and I'm not sure the people maintaining it want this." | https://github.com/rust-lang/rust/issues/61097, https://github.com/rust-lang/rust/pull/62996, https://github.com/rust-lang/rust/pull/63207 |
| 4g | A proposal to allow inline tests was refused | CONFIRM. Closed unmerged 2023-04-21. | jyn514: "it would regress compile times today." | https://github.com/rust-lang/rust/pull/110619 |
| 4h | rust-lang/rust PRs 135937 and 144399 | About packages, not file layout; not cited for the test-placement rule. | 135937: "Put the core unit tests in a separate coretests package". 144399: "Add a ratchet for moving all standard library tests to separate packages". | https://github.com/rust-lang/rust/pull/135937, https://github.com/rust-lang/rust/pull/144399 |
| 4i | "Always out of line" is the general convention | REJECT as a general claim; it is this project's choice. The Book describes the opposite convention, and Cargo declined to change the `cargo new` template (issue closed 2022-04-12). | "The convention is to create a module named `tests` in each file to contain the test functions and to annotate the module with `cfg(test)`." ehuss: "I don't think we'll be making this change at this time." | https://doc.rust-lang.org/book/ch11-03-test-organization.html, https://github.com/rust-lang/cargo/issues/10531 |
| 4j | A lint enforces it | REJECT. The proposed lint is an open issue with no PR. The `inline_modules` lint (clippy 1.97.0) exempts test modules. | "New `inline_test_mod` lint to encouage independent `test.rs` files for rebuild performance" (open). (raw) "Inline test modules (anything annotated with `#[cfg(test)]`) are not linted." | https://github.com/rust-lang/rust-clippy/issues/13589, https://github.com/rust-lang/rust-clippy/pull/16732 |
| 4k | `tests.rs` works with coverage tools | CONFIRM. cargo-llvm-cov excludes `tests.rs` and `*_tests.rs` by default. | default pattern `(tests.rs\|[0-9a-zA-Z_-]+[_-]tests.rs)$` | https://github.com/taiki-e/cargo-llvm-cov |
| 5a | Each file in `tests/` is a separate linked binary | CONFIRM | "Each integration test results in a separate executable binary, and `cargo test` will run them serially." | https://doc.rust-lang.org/cargo/reference/cargo-targets.html |
| 5b | `tests/it/main.rs` is one target; `tests/it/foo.rs` is not | CONFIRM. EXECUTED: `cargo metadata --no-deps` listed exactly `lib demo_core`, `bin demo` and `test it`; `cargo test` printed one `Running tests/it/main.rs`. | "Files in subdirectories of the tests directory don't get compiled as separate crates or have sections in the test output." | https://doc.rust-lang.org/book/ch11-03-test-organization.html |
| 5c | One integration test crate per project is recommended | CONFIRM (opinion source) | "That is why I recommend that large projects should have only one integration test crate with several modules." | https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html |
| 5d | Cargo's own consolidation: about 3x compile time | CONFIRM as reported by the post, not re-measured | "The time to compile the test suite decreased 3x." "The size of on-disk artifacts decreased 5x." | same |
| 5e | The saving is universal | AMEND. Another measurement found no gain from one binary; its gain came from extracting nine crates. | "Other techniques to improve compilation time produced marginal or no improvements." | https://blog.waleedkhan.name/rust-incremental-test-times/ |
| 5f | Helpers in `tests/it/support.rs`, not `tests/common/mod.rs` | CONFIRM. The Cargo Book idiom is `tests/common/mod.rs`, which `mod_module_files` rejects. EXECUTED: the lint fired on `tests/it/common/mod.rs`. | "you can place it in a separate module such as `tests/common/mod.rs`" | https://doc.rust-lang.org/cargo/reference/cargo-targets.html |
| 5g | nextest runs each test in its own process | CONFIRM | "A key factor distinguishing nextest from cargo test is that nextest runs each test in a separate process." | https://nexte.st/docs/design/why-process-per-test/ |
| 5h | That model is free | AMEND: it has a stated cost on two of the three target systems. | "It is quite slow on Windows, though, and it can be slow on macOS" | same |
| 6a | Doc-tests run outside nextest, with `cargo test --doc` | CONFIRM | "Doctests are currently not supported because of limitations in stable Rust." "For now, run doctests in a separate step with `cargo test --doc`." | https://nexte.st/docs/running/ |
| 6b | `.config/nextest.toml` at the workspace root | CONFIRM | "supports repository-specific configuration at the location `.config/nextest.toml` from the Cargo workspace root." | https://nexte.st/docs/configuration/ |
| 6c | cargo-fuzz layout: `fuzz/Cargo.toml`, `fuzz/fuzz_targets/<name>.rs` | CONFIRM | "The source code for this fuzz target by default lives in `fuzz/fuzz_targets/<fuzz target name>.rs`." | https://rust-fuzz.github.io/book/cargo-fuzz/tutorial.html |
| 6d | A per-crate `fuzz/` works under `members = ["crates/*"]` as is | REJECT. EXECUTED: `cargo metadata` in `crates/demo-core/fuzz` failed with "current package believes it's in a workspace when it's not"; `exclude = ["crates/*/fuzz"]` did not fix it. A top-level `fuzz/` with `exclude = ["fuzz"]` worked, and `cargo fuzz list` then worked from the root. | "The `members` list also supports globs" (globs are documented for `members` only) | executed; https://doc.rust-lang.org/cargo/reference/workspaces.html |
| 6e | `cargo fuzz init` output can be used unchecked | AMEND. EXECUTED: run inside a member of the virtual workspace, cargo-fuzz 0.13.2 wrote a manifest depending on the wrong member. Write or check the manifest by hand. | none | executed |
| 6f | Fuzzing runs on every contributor platform | REJECT | (raw) "this only works on x86-64 and Aarch64, and only on Unix-like operating systems (not Windows). This also needs a nightly compiler" | https://github.com/rust-fuzz/cargo-fuzz/blob/main/README.md |

## What the sources say about unit-test placement

- **Normative, one project.** rust-lang/rust requires out-of-line unit tests
  and enforces it with tidy (rows 4b–4g), because every edit to `core` would
  otherwise rebuild the standard library and the compiler.
- **Teaching material.** The Rust Book describes inline `mod tests` as "the
  convention". It describes; it does not forbid a separate file.
- **Projects that moved tests out.** chatmail/core moved its largest test
  modules out (merged 2025-01-20): it "is said to improve compilation speed",
  and grep then separates test calls from production calls. It named the
  files `chat_tests.rs` rather than `tests.rs`, so editor tabs differ.
  https://github.com/chatmail/core/pull/6456. A Zed PR that moved a large
  test block out was closed unmerged on 2026-06-19 as "a huge, redundant PR
  that changes nothing semantically".
  https://github.com/zed-industries/zed/pull/59596
- **Different changes, not cited for this rule.** Zed PR 47596 moved a
  crate's tests to the integration layer ("6.5s instead of 18s"); DataFusion
  PR 23040 moved tests next to their subject; the sqlx post measured macro
  expansion; `fast-rust-builds` covers the number of linked artifacts.
- **The Linux kernel** documents tests "in the same file where we have our
  function"; the page read does not discuss Cargo's rebuild behaviour.
- **rust-analyzer's style guide** has no placement rule; it says "Do not use
  `#[should_panic]` tests."
- **hyper** keeps seven separate `[[test]]` targets, a counter-example to one
  integration binary.
- **arXiv 2604.19826** studies model code generation with doctests versus
  `#[test]` blocks on one task; it says nothing about file placement.
- **Forum threads** (opinion) take both sides.

## Gaps and recommendations

**G1. Enforcing out-of-line unit tests. Yes, with the first crate.** No Clippy
lint does it (row 4j). An xtask check fails on `#[cfg(test)]` followed by an
inline `mod … {` under `src/`. Until then it is a review item.

**G2. Clippy's test allowances in `tests/it/`. Yes, now.** EXECUTED: with
`unwrap_used = "deny"` and `allow-unwrap-in-tests = true`, an `unwrap` was
accepted in a `#[test]` function in `tests/it/foo.rs` and in a helper in
`src/foo/tests.rs`, but rejected in a plain helper in `tests/it/support.rs`.
Clippy documents the scope as "test functions or `#[cfg(test)]`"
(https://doc.rust-lang.org/clippy/lint_configuration.html). The error went
away with `#![cfg(test)]` at the top of `tests/it/main.rs`. Only
`unwrap_used` was run; the other allowances share the documented wording.

**G3. `test = false` and `doctest = false`.** Thin binaries: yes, set
`test = false` on a binary whose `main.rs` only calls the library; its file
then stays free of logic. `doctest = false`: later; doc-tests are the usage
examples, so keep them until their run time is measurable.

**G4. `cargo xtask` instead of shell scripts. Yes, from the first
automation.** "It doesn't require any other binaries besides `cargo` and
`rustc`"; "Unlike bash, it can more easily be cross platform, as it doesn't
use the shell." It is "not an officially recommended workflow, but it is a
somewhat common pattern across the ecosystem"
(https://github.com/matklad/cargo-xtask). Windows contributors decide it.
Place it at `crates/xtask`, `publish = false`, with the alias
`xtask = "run --package xtask --"` in `.cargo/config.toml`.

**G5. A shared test-support crate. Later**, when a second crate needs the same
helper: `crates/ironafs-test-support`, `publish = false`, used only as a
dev-dependency.

**G6. `[workspace.dependencies]`. Yes, from the first dependency.** Every
version is stated once, in the root; members write `dep.workspace = true`;
internal crates are listed there with `path`. Limits: "Dependencies from this
table cannot be declared as `optional`". The documentation says a member-level
`default-features` override needs a later toolchain than the pinned one, so
set `default-features = false` in the workspace table and add features per
member.

**G7. `benches/` and `examples/`. Later.** Benchmarks follow the integration
layout: `benches/<name>/main.rs`. Examples are compiled by every `cargo test`;
internal crates use doc-tests for usage examples instead.

**G8. Per-OS crates. Name and place them now, create them later.**
`ironafs-fuse`, `ironafs-fskit`, `ironafs-cloudfiles`, flat in `crates/`. Every
member must build on every OS, because root commands cover all members (row
1c): OS-only dependencies go under `[target.'cfg(…)'.dependencies]`, and the
crate body is gated with a crate-level `cfg`. Non-Rust code (the Swift FSKit
extension, Windows packaging) has no primary-source convention; the
recommendation is a top-level `platform/<os>/`, outside `crates/`. The
architecture ADR decides this.

**G9. Snapshot files. Later.** insta stores snapshots "in the `snapshots` folder
right next to the test file". Golden wire vectors stay in
`tests/fixtures/<subject>/`. `REUSE.toml` must cover `.snap` files.
https://insta.rs/docs/snapshot-types/

**G10. Build scripts. No, by default.** A crate that needs a `build.rs`
states why. Code generation goes through xtask, with the output checked in.

**G11. `Cargo.lock`. Committed** (it is). "Cargo uses the lockfile to provide
deterministic builds at different times and on different systems". Installs
and CI use `--locked`. https://doc.rust-lang.org/cargo/faq.html

**G12. Fuzz layout. One top-level `fuzz/`, excluded from the workspace.**
`exclude = ["fuzz"]` in the root; targets named `<crate>_<subject>`. The fuzz
crate is outside the workspace, so the root's clippy, fmt, `cargo deny` and
workspace lints do not reach it; each runs separately there. Its lock file is
committed. Fuzzing needs `cargo +nightly` and runs only on Linux and macOS.

**G13. Other points to settle before the first crate.**
- Profiles live only in the root manifest.
- `lib.rs` and `main.rs` are "mod-rs" files: `#[cfg(test)] mod tests;` in
  `src/lib.rs` is `src/tests.rs`, and in `src/bin/afs/main.rs` it is
  `src/bin/afs/tests.rs`. This follows from the quoted rule; it was not run.
- CLI tests: integration tests get `CARGO_BIN_EXE_<name>` and
  `CARGO_TARGET_TMPDIR`, so `afs` can be tested as a process from `tests/it/`.
- A crate with no meaningful public API needs no `tests/` directory.
- nextest coverage must be merged with doc-test coverage.
- Process-per-test has a stated cost on Windows and macOS; watch it as suites
  grow.

## Sources

Cargo, Rust and Clippy documentation:
- https://doc.rust-lang.org/cargo/guide/project-layout.html
- https://doc.rust-lang.org/cargo/guide/tests.html
- https://doc.rust-lang.org/cargo/reference/cargo-targets.html
- https://doc.rust-lang.org/cargo/reference/workspaces.html
- https://doc.rust-lang.org/cargo/reference/manifest.html
- https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html
- https://doc.rust-lang.org/cargo/reference/profiles.html
- https://doc.rust-lang.org/cargo/reference/environment-variables.html
- https://doc.rust-lang.org/cargo/faq.html
- https://doc.rust-lang.org/reference/items/modules.html
- https://doc.rust-lang.org/edition-guide/rust-2018/path-changes.html
- https://doc.rust-lang.org/book/ch11-03-test-organization.html
- https://rustc-dev-guide.rust-lang.org/tests/intro.html
- https://github.com/rust-lang/rust-clippy/blob/master/clippy_lints/src/module_style.rs
- https://doc.rust-lang.org/clippy/lint_configuration.html

Issues and pull requests:
- https://github.com/rust-lang/rust/issues/61097
- https://github.com/rust-lang/rust/pull/62996
- https://github.com/rust-lang/rust/pull/63207
- https://github.com/rust-lang/rust/pull/110619
- https://github.com/rust-lang/rust/pull/135937
- https://github.com/rust-lang/rust/pull/144399
- https://github.com/rust-lang/rust-clippy/issues/13589
- https://github.com/rust-lang/rust-clippy/pull/16732
- https://github.com/rust-lang/cargo/issues/10531
- https://github.com/chatmail/core/pull/6456
- https://github.com/zed-industries/zed/pull/47596
- https://github.com/zed-industries/zed/pull/59596
- https://github.com/apache/datafusion/pull/23040

Tools:
- https://nexte.st/docs/design/why-process-per-test/
- https://nexte.st/docs/design/how-it-works/
- https://nexte.st/docs/running/
- https://nexte.st/docs/configuration/
- https://nexte.st/docs/integrations/test-coverage/
- https://github.com/rust-fuzz/cargo-fuzz/blob/main/README.md
- https://rust-fuzz.github.io/book/cargo-fuzz/tutorial.html
- https://github.com/taiki-e/cargo-llvm-cov
- https://github.com/matklad/cargo-xtask
- https://insta.rs/docs/snapshot-types/

Posts, papers and other projects' files:
- https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html
- https://matklad.github.io/2021/08/22/large-rust-workspaces.html
- https://matklad.github.io/2021/09/04/fast-rust-builds.html
- https://blog.waleedkhan.name/rust-incremental-test-times/
- https://kobzol.github.io/rust/2026/06/21/optimizing-sqlx-test-rebuild-time.html
- https://arxiv.org/abs/2604.19826
- https://github.com/rust-lang/rust-analyzer/blob/master/docs/book/src/contributing/style.md
- https://github.com/hyperium/hyper/blob/master/Cargo.toml
- https://github.com/torvalds/linux/blob/master/Documentation/rust/testing.rst
- https://users.rust-lang.org/t/should-unit-tests-really-be-put-in-the-same-file-as-the-source/62153 (opinion)
- https://users.rust-lang.org/t/real-world-tips-for-organising-unit-tests-for-larger-projects-and-files/130749 (opinion)
