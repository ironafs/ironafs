---
name: rust-review
description: Use when writing or reviewing Rust code in this repo — the house-rules checklist. Triggers on "review", "check my code", before committing nontrivial Rust changes, and before pushing an implementer's or a model lane's diff (its consistency pass).
---

# Rust house-rules review

Walk-through order for a change. The rules, with their numbers, are in
`docs/coding-standards.md`; ADR 0001 and `docs/workflow.md` §3a–3c apply too.
Cite a rule number in any `#[expect(..., reason)]`.

## Every change

1. **Panics**: no `unwrap`/`expect`/`panic!`/slice-indexing reachable from
   untrusted input (clippy denies them workspace-wide). Fileserver replies,
   callbacks, XDR, local IPC requests and config files are all untrusted.
   Tests may use them (`clippy.toml` allows them in tests only).
2. **Arithmetic on untrusted offsets/lengths** (rule 2): checked operations
   and fallible conversions; saturation only where the format or algorithm
   defines clamping.
   `overflow-checks = true` is on in release, but don't rely on it.
3. **Suppressions**: `#[expect(lint, reason = "…")]`, never bare `#[allow]`.
4. **Errors** (rules 4 and 21): IronAFS's own error types, in libraries and
   binaries alike; no third-party error crates. Display messages lowercase,
   no trailing period.
5. **SPDX header** present and in the form of `LICENSING.md`, "File
   headers": `GPL-3.0-or-later`. Nothing from OpenAFS,
   AuriStor or kernel AFS/Rx source (ADR 0001's don't-copy rule); permissive
   third-party material keeps its notices and gets a `docs/provenance.md` row
   in the same commit.
6. **Structure** (rules 10–12): each new item sits with the subject whose
   invariant it implements. No split or abstraction justified by size alone.
   A file over 3,000 lines carries its written note. Names say what code
   does.
7. **Resource limits** (rule 3): wire lengths and counts are capped before
   allocation, and limits on queues, buffers and caches count bytes.
8. **Time and randomness** (rule 7): no direct `Instant::now`,
   `SystemTime::now` or random calls outside the injected seam.
9. **Secrets** (rule 8): zeroed on drop, compared in constant time, never in
    `Debug`, logs or errors.
10. **Formats**: any wire or on-disk format change follows the `wire-format`
    skill.
11. **Comments and docs** (rule 14): no development narrative; a stated
    guarantee names what carries it; items cited by name; falsified comments
    fixed; `//!` on every module; `///` on public items and on private items
    with a contract; `# Errors`, `# Panics`, `# Safety` and
    `# Cancellation safety` where they apply; no bare `TODO` or `FIXME`.
12. **Tests and layout** (rules 15, 20): unit tests in `src/foo/tests.rs`,
    no `foo/mod.rs`; integration tests in one `tests/it/` binary starting
    with `#![cfg(test)]`; versions only in `[workspace.dependencies]`;
    named for the behaviour; from the contract, with the right kind of red
    evidence; the most specific assertion, no `#[should_panic]`;
    deterministic; evidence lists the expected tests, the result, the exit
    status, the revision and the machine.
13. **Lint inheritance**: every crate's `Cargo.toml` has
    `[lints] workspace = true`.

## Consistency pass

Run by someone other than the author, on the implementer's diff, before the
review seats start.

- **Inventory** every new, removed or re-declared function, tests included.
  The commands below are a first pass: the regex misses some forms, such as
  `extern fn` without an ABI string or an attribute on the same line. Check
  the result against the full diff.

  ```sh
  RE='(pub(\([^)]*\))?[[:space:]]+)?((const|async|unsafe|extern "[^"]*")[[:space:]]+)*fn[[:space:]]'
  git diff <base> -- crates/ | grep -E "^[-+][[:space:]]*$RE"
  git ls-files -z --others --exclude-standard -- crates/ |
    xargs -0 grep -nE "^[[:space:]]*$RE" /dev/null
  ```

- **For each new function, find its nearest existing equivalent** and reuse
  it, or extract the shared step. A partial copy that drops a step of the
  original is the defect to look for.
- **Compare with the neighbours** in the same module: error construction,
  lock and check order, naming, doc comments, test fixtures. Tests, test
  doubles and simulators are in scope; a fake that copies a production
  sequence drifts like any other copy.
- **Departures** from the neighbours are named, with the reason, in the
  change's description; an unnamed one is a finding.
- **Record** each finding with the item it should reuse or match, or which
  sites were compared when nothing was found.

## Async code additionally

14. Any fn used in `select!` documents `# Cancellation safety`.
15. No blocking syscalls or sustained CPU in async context.
16. Cleanup is an explicit `async fn shutdown()`; `Drop` never needs to await.
17. External I/O is wrapped in a timeout; prefer `JoinSet` and a cancellation
    token over bare `spawn`.

## Unsafe

18. `unsafe_code` is `forbid` workspace-wide. A crate that needs it (FSKit, SSPI or
    other OS bindings; never C Rx, ADR 0001) gets an ADR first, then runs
    at `deny` with named `#[expect(unsafe_code, reason = "…")]` sites (rule 5).
19. Every `unsafe` block and `unsafe impl` has `// SAFETY:` naming the
    invariant it upholds.

## Before commit

20. `cargo fmt`, `cargo clippy --all-targets` (with `--target` for every OS
    when code behind an OS `cfg` is touched; `docs/workflow.md` §3b) with no warnings, `cargo doc` without warnings,
    tests green.
21. New dependency: the `add-dependency` skill applies.
22. Commit message lowercase and short; attribution in the trailers and
    `docs/reviews/`, never in source (ADR 0002).
