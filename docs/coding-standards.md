<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Coding standards

The rules for Rust code in IronAFS. `AGENTS.md` summarises them for agents,
and the `rust-review` skill walks through them for a change.

**Rule numbers never change.** Source cites them, for example in
`#[expect(clippy::..., reason = "rule 4: …")]`. A retired rule keeps its
number and is marked retired; new rules are appended.

## MUST

1. **No panics on untrusted input.** Fileserver replies, callbacks, XDR, local
   IPC requests and configuration files are untrusted. No `unwrap`, `expect`,
   `panic!`, `todo!`, `unimplemented!`, `unreachable!`, slice indexing or
   string slicing is reachable from them; the workspace lints deny these, and
   `dbg!`. Tests may use `unwrap`, `expect`, `panic!` and indexing
   (`clippy.toml`).

2. **Arithmetic on untrusted values is checked.** Validating offsets, lengths
   and counts from the wire, building ranges and sizing allocations use
   checked arithmetic and fallible conversions (`checked_*`, `try_from`), and
   reject invalid results. Saturating arithmetic is only for values the format
   or algorithm defines as clamped: saturation can turn an invalid length into
   a valid-looking zero.
   `overflow-checks = true` stays on in release builds.

3. **Resource limits are measured in bytes.** Limits on queued replies,
   concurrent calls, buffers and caches count bytes, not just items. Every
   length or count read from the wire is checked against a hard cap before
   anything is allocated.

4. **Errors.** Libraries define their own error types. Binaries may use a
   general error type. Public error types are `Send + Sync + 'static`. Display
   messages are lowercase, with no trailing period.

5. **`unsafe` is forbidden** workspace-wide (`unsafe_code = "forbid"`).
   - A crate that needs it (FSKit, SSPI or other OS bindings) gets an ADR first.
     Check first whether a safe binding crate removes the need.
   - Every crate inherits the workspace lints (`[lints] workspace = true`);
     a crate that does not is a finding. The one exception is `ironafs-os`
     (ADR 0005, point 1.5): its manifest carries its own `[lints]` table,
     equal to the workspace's except for `unsafe_code = "deny"`.
   - That crate's ADR says how it keeps the other workspace lint levels while
     lowering `unsafe_code` to `deny` for that crate only. Cargo cannot
     combine `[lints] workspace = true` with a crate-level override, and an
     `#[expect]` in source cannot lower a `forbid`.
   - Within it, unsafe sites carry named `#[expect(unsafe_code, reason = "…")]`.
   - Every `unsafe` block and `unsafe impl` carries a `// SAFETY:` comment
     naming the invariant it upholds, and `clippy::undocumented_unsafe_blocks`
     is denied.
   - A Miri job is added with the first such crate.

6. **Warnings are errors in CI, not in source.** CI sets
   `CARGO_BUILD_WARNINGS=deny` for Clippy; `cargo xtask check`'s lint
   refuses every warning but the ones `check-bans`' canary must draw. Source never says
   `#![deny(warnings)]`, which breaks builds on every new compiler lint.

7. **Time and randomness go through a seam.** No direct `Instant::now`,
   `SystemTime::now` or random-number calls outside an injected clock and
   random source. Clippy's `disallowed-methods` enforces this once the seam
   exists. Callback expiry, token lifetimes and Rx retransmission timers must
   be testable without waiting. Tests are never retried; a retry that passes
   hides a nondeterminism bug.

8. **Secrets.** Tokens, session keys and passwords:
   - are zeroed on drop;
   - are compared in constant time;
   - never appear in `Debug` output, logs or error messages.

   The limits of the crates used for this are written down where they are
   used.

9. **Wire and on-disk formats** follow the `wire-format` skill: caps before
   allocation, an encoder that refuses what the decoder rejects, checksums
   over the bytes received, and for every format a spec, a fuzz target, golden
   vectors, and round-trip tests where an encoder exists (independent fixtures
   and invariant checks otherwise).

10. **Structure follows ownership, not size.**
    - A module holds one subject. An item belongs to the subject whose
      invariant it enforces.
    - A file is split where two subjects meet at a one-way seam, never at a
      line count.
    - A file over 3,000 lines carries a written note in its first lines
      saying why it stays whole. An xtask check (planned, with the first
      crate) will enforce the note, not a split; until then the consistency
      pass checks it.
    - A crate is extracted only at a real boundary: a different dependency
      set, platform, or set of consumers.

11. **No speculative abstraction.** A trait or generic earns its place with a
    second implementation, or with a test double that needs it.

12. **Names say what code does**, never when or in which task it was written.

13. **Reuse and neighbours** (`docs/workflow.md` §3a). Call what exists. Match
    the module around you. Name any deliberate departure, with its reason.

14. **Comments and docs describe the code as it is now.**
    - **What a comment is for:** what the code cannot say for itself. That is
      intent, an invariant, a non-obvious mechanism, a limitation, or why a
      plausible alternative was not taken. A comment never restates the code.
    - **Keep:** the reason for a choice; references that resolve, in this
      repository or upstream (RFC 4120 §5.3, a `docs/specs/` file and
      section); a reproducible example; a stated limitation ("does not see
      renames across volumes").
    - **Never narrate the code's development.** That means:
      - how it used to behave;
      - which review, task, session or model produced it;
      - who suggested it;
      - a decision number a reader of the published tree cannot resolve.

      Say the rule the decision settled instead. The test is the sentence's
      meaning, not its words: "the previous protocol version" describing a
      real compatibility path is fine. Provenance goes in the commit message
      and the review record.
    - **A stated guarantee is true, and a reader can see why.** Where it is not
      obvious from the code, say what carries it: a type that cannot hold the
      bad state, a mechanism, a check or a test. Where nothing enforces it,
      say so. This is not a call for a citation beside every sentence.
    - **Cite code by item name**, never by line number; line numbers go stale
      with the next edit above them.
    - **A change that falsifies a comment fixes it in the same change.**
      Search for the old claim, not only the lines being edited.
    - **Kinds of comment:**
      - every module opens with a `//!` comment naming its subject (rule 10);
      - public items, and private items with a contract worth stating, have
        `///` doc comments; a private helper whose only possible summary
        would restate its code needs none;
      - `//` is for implementation notes;
      - `// SAFETY:` explains `unsafe` (rule 5).

      Comments are plain sentences.
    - **Public items** follow the Rust API Guidelines' documentation chapter:
      - a one-line summary;
      - `# Errors`, `# Panics` and `# Safety` sections where they apply (the
        Rust API Guidelines), and an IronAFS-specific `# Cancellation safety`
        section for async functions used in `select!` (rule 18);
      - intra-doc links;
      - doc-tests as the usage examples.

      `cargo doc` builds without warnings.
    - **No bare `TODO` or `FIXME`.** Each names its public issue, as
      `TODO(#123): …`. That issue reference is the one task id allowed in
      source: it resolves for every reader. Unfinished work is tracked in the
      issue, not in the comment.

15. **Tests.**
    - **Where they live.**
      - A module's unit tests always live in their own file:
        `src/foo.rs` declares `#[cfg(test)] mod tests;`, and the tests are in
        `src/foo/tests.rs`. For the crate root, `src/lib.rs` uses
        `src/tests.rs`. This is IronAFS's choice: the Rust Book describes
        inline tests as the convention, while the Rust project requires
        separate files for its compiler and standard library. The reason is
        rebuild cost: editing a test then rebuilds only the crate's test
        build, not its normal build or the crates that depend on it. It also
        keeps production files about production code. No lint enforces this
        yet; an xtask check will, and until then the consistency pass does.
      - A module with children is `foo.rs` with a sibling `foo/` directory,
        never `foo/mod.rs` (clippy `mod_module_files`), as the Rust Reference
        encourages.
      - A test belongs to the subject it asserts about, not to every name it
        touches. A shared fixture lives with what it depends on.
      - A large test file is a signal about the module: it usually holds more
        than one subject (rule 10).
      - Integration tests are one test binary per crate, laid out as rule 20
        says, and use only the public API. A crate with no meaningful public
        API has no integration tests.
      - Fixtures live under `tests/fixtures/<subject>/`, and are synthetic or
        reduced under ADR 0001.
      - Shared test helpers live in one place and are reused (rule 13), never
        copied.
    - **Names say the behaviour:** `subject_condition_outcome`, for example
      `decode_rejects_length_over_cap`.
    - **Tests come from the written contract.**
      - A change to runtime behaviour, and every bug fix, has a test that
        fails without the change on the intended assertion: shown red, then
        green.
      - For a new API with no earlier version to fail against, the test is
        shown failing against a compiling stub or a deliberately wrong
        implementation.
      - A contract the compiler enforces is shown by the compiler's
        rejection, not by a runtime test.
      - A change that only adds coverage for existing behaviour gives the
        acceptance evidence its brief names.
      - For P and above, the tests are checked against a plausible wrong
        implementation.
      - A behaviour-preserving refactor relies on the existing tests, or
        names the type or mechanism that enforces the invariant.
    - **Assert exactly.**
      - Assert the most specific result the contract exposes: an error
        variant or kind, an exit status, or text that is part of the contract.
        Never just `is_err()`.
      - No `#[should_panic]`. For a documented panic, assert it explicitly
        (catch the unwind, or run a subprocess and check its status).
      - Synchronous assertion helpers carry `#[track_caller]`. The attribute
        has no effect on an `async fn`, so async helpers assert at the call
        site or pass the location explicitly.
    - **Parsers and decoders** get property tests and a fuzz target (rule
      20 places it; fuzzing needs nightly Rust and runs on Linux and macOS
      only). Round
      trips are tested where an encoder exists (the `wire-format` skill).
      Read-only parsers are tested against independent fixtures and their own
      invariants.
    - **Deterministic, always.**
      - No real time or sleeps: use the clock seam (rule 7).
      - Random inputs come from a seed that is printed on failure.
      - No network beyond localhost, on ephemeral ports.
      - Writable files, sockets and other mutable state use unique temporary
        paths. Read-only fixtures are read from their stable paths in the
        repository.
      - No dependence on test order; tests are safe to run in parallel.
    - **No retries, and a hang is a failure.**
      - The runner's profiles retry nothing: a retry that passes hides a
        determinism bug. Acceptance runs pass `--retries 0` explicitly,
        because a command-line option or environment variable overrides the
        profile.
      - Nextest terminates a test that runs too long, so a deadlock fails
        instead of hanging (`.config/nextest.toml`).
      - Doc-tests run outside nextest. They get a deadline from their CI step;
        by hand, a run that does not finish is a failure.
    - **Platforms.** Code behind an OS `cfg` is tested on that OS. CI runs
      the tests on Linux and lints the Windows and macOS targets from Linux;
      the change that adds the first code behind a Windows or macOS `cfg`
      adds that system's runner to CI. The evidence names the machine.
    - **Evidence** (`docs/workflow.md` §3b, §3c). Record:
      - the tests expected to run, by name or filter, and check they ran;
      - the result and the command's exit status;
      - the revision, date and machine.

      A test that ran and failed, including one the runner terminated for
      running too long, is an executed failure. Zero tests selected, or a
      build, setup or infrastructure failure that stopped tests running, is
      *missing evidence*, never a pass.
    - **The runner** is `cargo nextest run --retries 0` (CI uses
      `--profile ci`). Doc-tests run with `cargo test --doc --workspace`.

16. **Checks pass only on proof of success** (`docs/workflow.md` §3b).

## SHOULD

17. **Lints.**
    - clippy `pedantic` at `warn`, with `priority = -1`, and named `#[expect]`
      exceptions;
    - never blanket `restriction` or `nursery`;
    - rustc `unreachable_pub`, `missing_debug_implementations` and
      `elided_lifetimes_in_paths` at `warn`;
    - clippy `mod_module_files` at `warn` (rule 15's module layout).

18. **Structured concurrency.** Prefer a `JoinSet` with a cancellation token
    over bare `spawn`. A function used in `select!` documents its cancellation
    safety. No blocking calls in async code. External I/O has a timeout.
    Cleanup is an explicit `async fn shutdown()`; `Drop` never needs to await.

19. **Toolchain and platform floors.**
    - A toolchain bump is its own pull request, and reads the platform-support
      notes of every release it crosses.
    - Raising a supported OS floor (Linux kernel and glibc, the macOS version
      for FSKit, Windows) is the owner's decision.
    - CI stays green on the old floor until the change lands.

## Layout

20. **Workspace and crate layout.** The research behind this rule is in
    `docs/research/2026-11-18-project-layout-and-tests.md`.
    - **The workspace root** is a virtual manifest: it lists the crates and
      holds no code. Profiles live only there.
    - **Crates sit flat under `crates/`**, each folder named exactly after its
      crate, prefix included (`crates/ironafs-cli`). No nested groups.
    - **Dependencies:** for workspace members, every version is stated once,
      in `[workspace.dependencies]`; crates write `name.workspace = true`.
      Shared package fields come from `[workspace.package]`, and lints from
      `[workspace.lints]`. The excluded `fuzz/` crate cannot inherit these:
      it declares its own dependencies and equivalent lints, under the same
      rule 21, `add-dependency` checklist, `deny.toml` policy and `--locked`
      builds, with its own lock file.
    - **Inside a crate:**
      - modules are `foo.rs` with a sibling `foo/` holding their children,
        one flat level at a time, recursively (rule 15 has the unit-test
        files);
      - `src/lib.rs` holds module declarations and a deliberately chosen
        public surface;
      - a crate with a library and a binary keeps the binary under
        `src/bin/<name>/`. A thin binary that only calls the library sets
        `test = false` and keeps no logic of its own.
    - **Integration tests:** one test binary per crate.
      - `tests/it/main.rs` starts with `#![cfg(test)]` (so Clippy's test
        allowances cover its helpers) and declares a module per subject:
        `tests/it/<subject>.rs`.
      - Shared helpers are in `tests/it/support.rs`. Fixtures are in
        `tests/fixtures/<subject>/`.
      - A second binary only for a test that needs its own harness
        (`harness = false`), with the reason written down.
      - The `afs` binary is tested as a process from `tests/it/`, using
        `CARGO_BIN_EXE_afs`, and writes only under `CARGO_TARGET_TMPDIR` or
        a temporary directory.
    - **Fuzz targets** live in one top-level `fuzz/` crate, excluded from the
      workspace (`exclude = ["fuzz"]`), with targets named
      `<crate>_<subject>`. Its manifest is written or checked by hand, and
      its own lock file is committed. The root's lints and checks do not
      reach it, so it gets its own clippy, fmt and `cargo deny` runs.
    - **Repository automation is `cargo xtask`**, in `crates/xtask`
      (`publish = false`), not shell scripts, so that it can run on Windows
      too. Each xtask command is written and tested for that (the per-OS
      checks in `docs/workflow.md` §3b). Reviewers' one-off command lines and
      CI job steps are not repository automation.
    - **Shared test helpers used by several crates** move into
      `crates/ironafs-test-support` (`publish = false`, a dev-dependency
      only) when a second crate needs them, not before.
    - **No `build.rs` without a written reason.** Generated code is produced
      by an xtask command and checked in.
    - **`Cargo.lock` is committed.** CI and installs use `--locked`.
    - **Later, with the rule already set:**
      - benchmarks go in `benches/<name>/main.rs`;
      - internal crates have no `examples/`, because doc-tests carry the
        examples;
      - snapshot files sit in `snapshots/` beside their test, and
        `REUSE.toml` must cover them;
      - `doctest = false` only where doc-test time is measured to matter.
    - **Per-OS crates and platform code** (proposed; the architecture ADR
      decides):
      - `ironafs-fuse`, `ironafs-fskit` and `ironafs-smb` (with
        `ironafs-cloudfiles` later, if built), flat in `crates/`. Every crate builds on every OS, with OS-only dependencies
        under `[target.'cfg(…)'.dependencies]`;
      - non-Rust code (the Swift FSKit extension, Windows packaging) under a
        top-level `platform/<os>/`.

## Dependencies

21. **Our own code first; a dependency only when we must.** Dependencies are
    the supply chain. crates.io has removed malicious crates whose payloads
    ran in projects that built, ran or tested with them, one of them reached
    through another crate that depended on it ([faster_log and
    async_println](https://blog.rust-lang.org/2025/09/24/crates.io-malicious-crates-fasterlog-and-asyncprintln),
    [evm-units and
    uniswap-utils](https://blog.rust-lang.org/2025/12/03/crates.io-malicious-crates-evm-units-and-uniswap-utils)).
    A build that runs a build script or a procedural macro executes
    third-party code on the developer's or CI machine. A security-critical Rust tool can live on a handful of
    crates: sudo-rs went from about 135 to 3
    ([Prossimo](https://memorysafety.org/blog/reducing-dependencies-in-sudo/)).
    - **Written in IronAFS:**
      - the protocols: Rx and its security classes (including rxkad's fcrypt),
        XDR, the RPC clients, and the FUSE wire protocol over `/dev/fuse`;
      - the event loop and concurrency: blocking code on standard-library
        threads, and no async runtime, neither a third-party one nor one of
        our own (ADR 0004, C3);
      - CLI parsing, error types, logging, configuration parsing, and test
        helpers.
    - **A dependency only where writing it ourselves would be less safe:**
      - mainstream cryptography (AES, SHA-2, HMAC and the like), never
        hand-rolled: well-reviewed crates or the operating system's own
        cryptography;
      - the operating system's own interfaces (Kerberos and GSS-API, Cloud
        Files, FSKit). These include libraries the platform's vendor or
        distribution ships as part of the supported system, even when
        installed as a separate package (for example a distribution's MIT
        Kerberos GSS-API library). Each platform's ADR names the library, how
        it is linked, and the binding: one we write (following rule 5 for
        `unsafe`), or the vendor's own crate.
    - **Test and fuzz manifests follow the same rule.** Property tests use
      IronAFS's own seeded helpers. The one planned exception is the fuzzing
      engine's binding (`libfuzzer-sys`, which cargo-fuzz requires), decided
      by the owner when the first fuzz target lands. Standalone tools that
      run but are not linked (cargo-fuzz, cargo-nextest, cargo-deny) are
      pinned by version.
    - **Every dependency is the owner's decision**, recorded in the commit that
      adds it with the `add-dependency` checklist: what it replaces, its full
      transitive cost, its build-time code, its licence and its owners. The
      target is a closure small enough to read.

