<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review records

IronAFS is built with AI models from five families ([AI.md](../../AI.md)).
These records are the evidence of how each change was reviewed: which models
reviewed it, what they found, and what was done about the findings. Records
are redacted summaries: findings are listed one by one where it matters, and
some rounds group small fixes together. A refuted finding is recorded with
the evidence that refutes it.

## Records

| Record | What was reviewed |
|---|---|
| [2026-11-18-first-public-tree](2026-11-18-first-public-tree.md) | The first commit of the public history: mechanical checks, an independent pre-publication audit, and every review round |
| [2026-11-19-workflow-pr-limits](2026-11-19-workflow-pr-limits.md) | Issue #4: the limit on open pull requests, and settling and closing issues |
| [2026-11-19-ci-and-xtask](2026-11-19-ci-and-xtask.md) | Issue #1: the CI workflow and the `xtask` checks |
| [2026-11-19-trailer-checks](2026-11-19-trailer-checks.md) | Issue #2: commit trailer checks in CI and the `commit-msg` hook |
| [2026-11-19-research-refute-check](2026-11-19-research-refute-check.md) | Issue #16: the refute check of the driverless-clients research note (three families and an adjudicator), the macOS options note, and the reviews of the Windows Cloud Files and UNC notes |
| [2026-11-19-judgement-seat](2026-11-19-judgement-seat.md) | Issue #14: the judgement seat and the strongest tiers |
| [2026-11-19-model-routing](2026-11-19-model-routing.md) | Issue #17: model routing by seat, class and role |
| [2026-11-20-pre-public-tidy](2026-11-20-pre-public-tidy.md) | Issue #3: wording tidy before going public |
| [2026-11-20-adr-0004-claims](2026-11-20-adr-0004-claims.md) | Issue #20: the factual claims behind ADR 0004, each re-checked at its source |
| [2026-11-20-windows-smb-first-docs](2026-11-20-windows-smb-first-docs.md) | Issue #21: the public description of Windows as SMB first |
| [2026-11-20-review-authorship](2026-11-20-review-authorship.md) | Issue #26: who counts as an author for review seats |
| [2026-11-20-adr-amendments](2026-11-20-adr-amendments.md) | Issue #28: firm and working ADR decisions, and amendments |
| [2026-11-20-kernel-code-research](2026-11-20-kernel-code-research.md) | The research note on kernel code against user space, and README corrections |
| [2026-11-20-readme-adr-0004](2026-11-20-readme-adr-0004.md) | The README follows ADR 0004: goals, sources for the Why section, planned platforms and order of work |
| [2026-11-20-workflow-milestones](2026-11-20-workflow-milestones.md) | Milestones follow ADR 0004, O50 |
| [2026-11-20-workflow-implementer](2026-11-20-workflow-implementer.md) | Issue #40: Claude implements from M2a; label rules and triage |
| [2026-11-21-adr-0005-review](2026-11-21-adr-0005-review.md) | Issues #43 and #49: ADR 0005 (`ironafs-os`) and the ADR 0004 C5 amendment |
| [2026-11-21-m2a-10-clock-seam](2026-11-21-m2a-10-clock-seam.md) | Issue #58: the clock and randomness seam, `ironafs-time`, and the bans that keep rule 7 |
| [2026-11-21-m2a-11-test-support](2026-11-21-m2a-11-test-support.md) | Issue #44: `ironafs-test-support`, the seeded generator and shrinker |
| [2026-11-21-m2a-13-check-deps](2026-11-21-m2a-13-check-deps.md) | Issue #46: the `xtask` dependency, pure-crate and grant checks |
| [2026-11-21-m2-plus-plan](2026-11-21-m2-plus-plan.md) | Issue #41: the M2+ plan and wave one's issue texts |
| [2026-11-21-m1-close-sweep](2026-11-21-m1-close-sweep.md) | The documentation sweep against ADR 0004 at M1's exit |
| [2026-11-21-m2a-90-convergence](2026-11-21-m2a-90-convergence.md) | Issue #54: convergence rules for review rounds |
| [2026-11-21-m2a-13-remainder](2026-11-21-m2a-13-remainder.md) | Issue #46: `check-pure`'s exact matrix of compiled crates, targets and profiles, and the crate root's form |
| [2026-11-21-m2a-92-fuse-codec](2026-11-21-m2a-92-fuse-codec.md) | Issue #92: Class S integration of the read-only FUSE codec and VFS errno mappings |
| [2026-11-21-lane-calibration](2026-11-21-lane-calibration.md) | Issue #48: the review lanes calibrated on IronAFS ranges with planted defects (GLM 5.3 high and Gemini Carbon medium, first suite) |
| [2026-11-21-m2a-01-xdr-spec](2026-11-21-m2a-01-xdr-spec.md) | Issue #42: `docs/specs/`, its template and the XDR profile with caps |
| [2026-11-21-m2a-11-remainder](2026-11-21-m2a-11-remainder.md) | Issue #44: the exhaustive sweep and wire-message variants |
| [2026-11-21-commit-message-errata](2026-11-21-commit-message-errata.md) | Errata: squash messages on `main` that lack the disclosure line, and the wrapping style |
| [2026-11-21-m2a-68-crypto-part1](2026-11-21-m2a-68-crypto-part1.md) | Issue #68, part one: `ironafs-crypto`, the random source interface and the production `Rng` |
| [2026-11-21-m2a-47-cell-part1](2026-11-21-m2a-47-cell-part1.md) | Issue #47, first part: `cargo xtask cell`'s pinned definition, engine runner, teardown and verified install |
| [2026-11-21-m2a-64-corpora](2026-11-21-m2a-64-corpora.md) | Issue #64: rules for published conformance corpora in `docs/specs/README.md` |
| [2026-11-21-m2a-56-directory-format](2026-11-21-m2a-56-directory-format.md) | Issue #56: the AFS directory format, the version from documents only |
| [2026-11-21-m2a-71-seams](2026-11-21-m2a-71-seams.md) | Issue #71: the `Rpc` and `Disk` seams and the scheduling model |
| [2026-11-21-m2a-67-contract](2026-11-21-m2a-67-contract.md) | Issue #67: the contract between the core and the layers |
| [2026-11-21-adr-0006-review](2026-11-21-adr-0006-review.md) | Issue #52: ADR 0006, what an unprotected Rx packet may change |
| [2026-11-21-m2a-55-rx](2026-11-21-m2a-55-rx.md) | Issue #55: the Rx transport and null class specification, first version from documents only |
| [2026-11-21-m2a-65-vl-fs-read](2026-11-21-m2a-65-vl-fs-read.md) | Issue #65: volume location and file-service reads and operation grants, the version from documents only |
| [2026-11-21-m2a-60-dns-client](2026-11-21-m2a-60-dns-client.md) | Issue #60: the first-party DNS client for Linux and Unix builds |
| [2026-11-21-m2a-63-testing](2026-11-21-m2a-63-testing.md) | Issue #63: the test strategy |
| [2026-11-21-m2a-07-wire-time](2026-11-21-m2a-07-wire-time.md) | Issue #82: the wire time-field table, first version from the participating specifications |
| [2026-11-21-m2a-62-local-ipc](2026-11-21-m2a-62-local-ipc.md) | Issue #62: the local IPC framework specification, in three parts; stopped with five blockers recorded |
| [2026-11-21-m2a-71-types](2026-11-21-m2a-71-types.md) | Issue #71: the seam interface types in their crates, and `check-seams` |
| [2026-11-21-m2a-66-callbacks](2026-11-21-m2a-66-callbacks.md) | Issue #66: the callback service specification, procedures and allowlist, first version from documents only |
| [2026-11-21-m2a-78-incarnation](2026-11-21-m2a-78-incarnation.md) | Issue #78: volume incarnation evidence and the restore discriminator, the version from documents only |
| [2026-11-21-m2a-14-xdr](2026-11-21-m2a-14-xdr.md) | Issue #59: `ironafs-xdr`, the XDR codec with caps, and `check-bans`' configuration checks |
| [2026-11-21-m2a-45-fuzz](2026-11-21-m2a-45-fuzz.md) | Issue #45: the excluded `fuzz/` crate, its pinned tooling, the deny-file and replay checks, and the XDR decode target |
| [2026-11-21-m2a-81-keepalive](2026-11-21-m2a-81-keepalive.md) | Issue #81: the keepalive, callback trust and server time specification, first version from documents only, with the callback specification's lowering count |
| [2026-11-22-gemini-security-seat](2026-11-22-gemini-security-seat.md) | The owner's decision: Gemini Carbon may hold the security seat, at `gemini-4-carbon-medium` |
| [2026-11-21-m2a-57-os-part-a](2026-11-21-m2a-57-os-part-a.md) | Issue #57, part A: `ironafs-os`'s first sites (clocks, random source, wipe), `check-os` and `check-abi` |
| [2026-11-21-m2a-93-config-syntax](2026-11-21-m2a-93-config-syntax.md) | Issue #93, part (a) and the cell name: `ironafs-config`'s strict TOML subset and the canonical cell name |
| [2026-11-21-m2a-93-config-schema](2026-11-21-m2a-93-config-schema.md) | Issue #93, part (b): the configuration schema, its unknown-key rules and layering (draft; four recorded blockers) |
| [2026-11-21-m2a-57-os-parts-bc](2026-11-21-m2a-57-os-parts-bc.md) | Issue #57, parts B and C: `ironafs-os`'s dump controls, secret buffer, peer credentials and descriptor passing, and their checks |
| [2026-11-21-m2a-57-os-part-e](2026-11-21-m2a-57-os-part-e.md) | Issue #57, part E: `ironafs-os`'s IPv6-only UDP socket and free-space query, and their checks |
| [2026-11-23-gpt-api-route](2026-11-23-gpt-api-route.md) | The owner's decision: GPT read-only seats may also run on the OpenAI API, the same models by another route |
| [2026-11-24-glm54-pin](2026-11-24-glm54-pin.md) | Issue #48: calibrating GLM 5.4 for the high-effort seats |
| [2026-11-24-glm54-max-pin](2026-11-24-glm54-max-pin.md) | Issue #48: calibrating GLM 5.4 for the strongest seats at max |
| [2026-11-24-146-os-guards](2026-11-24-146-os-guards.md) | Issue #146: strengthen the operating-system ABI and wipe checks |
| [2026-11-24-166-wait-order](2026-11-24-166-wait-order.md) | Issue #166: take the wait signal snapshot before the deadline clock reading |
| [2026-11-25-167-xdr-default](2026-11-25-167-xdr-default.md) | Issue #167: specify a closed-enum fixture with a default arm |
| [2026-11-25-53-m2a-working-amendments](2026-11-25-53-m2a-working-amendments.md) | Issue #53: the M2a Working amendments to ADR 0004 |
| [2026-11-26-59-d9-boundary-seeds](2026-11-26-59-d9-boundary-seeds.md) | Issue #59: the shared XDR fuzz target at its message cap and one byte over |
| [2026-11-26-48-historical-lane-calibration](2026-11-26-48-historical-lane-calibration.md) | Issue #48: integration review for the first lane-calibration record |
| [2026-11-26-45-fuzz-input-bounds](2026-11-26-45-fuzz-input-bounds.md) | Issue #45: engine input bounds for tracked XDR and configuration seed replay |
| [2026-11-26-71-q12-issue-brief](2026-11-26-71-q12-issue-brief.md) | Issue #71 Q12 issue brief review and filing decision |
| [2026-11-26-45-stable-seed-replay](2026-11-26-45-stable-seed-replay.md) | Issue #45: ordinary stable replay of the existing fuzz seed corpus |
| [2026-11-26-200-blocking-standards](2026-11-26-200-blocking-standards.md) | Issue #200: align coding standards rules 14 and 18 with the blocking model |
| [2026-11-26-xdr-fixture-coverage-split](2026-11-26-xdr-fixture-coverage-split.md) | Class I: split XDR unit and golden test coverage around one shared Shaded fixture |
| [2026-11-27-156-early-seam-census](2026-11-27-156-early-seam-census.md) | Issue #156: the early written-source seam census and its import resolution checks |
| [2026-11-27-67-vfs-name-error](2026-11-27-67-vfs-name-error.md) | Issue #67: VFS name and error vocabulary split |
| [2026-11-27-48-glm54-document-calibration](2026-11-27-48-glm54-document-calibration.md) | [#48](https://github.com/ironafs/ironafs/issues/48): GLM-5.4 High document-review calibration: 6/6 complete, 11/12 planted, 6/6 critical, 0 false blockers, 0 false majors; hold-out sealed |
| [2026-11-27-173-seam-fixture-errors](2026-11-27-173-seam-fixture-errors.md) | Issue #173: fail the seam self-test on unreadable fixtures |
| [2026-11-27-156-testing-cross-reference](2026-11-27-156-testing-cross-reference.md) | Issue #156: testing.md 10.6 names #156 for the seam conformance bodies |
| [2026-11-27-48-sol-xhigh-code-calibration](2026-11-27-48-sol-xhigh-code-calibration.md) | [#48](https://github.com/ironafs/ironafs/issues/48): GPT-6.2 Sol xhigh code-review calibration: 6/6 finished, 12/12 planted, 6/6 critical, 0 false blockers, 0 false majors; hold-out sealed |
| [2026-11-27-156-c4-disk-precursor-issue-brief](2026-11-27-156-c4-disk-precursor-issue-brief.md) | Issue #156: review of the brief for a phased `ironafs-disk` row in ADR 0004 C4 |
| [2026-11-27-213-c4-disk-amendment](2026-11-27-213-c4-disk-amendment.md) | Issue #213: the phased `ironafs-disk` row and name identity in ADR 0004 C4, and its `check-deps` rules |

ADRs carry their review records in their own Review sections
([docs/adr/](../adr/README.md)).

## The format

One file per reviewed change, `YYYY-MM-DD-<slug>.md`, added in the same squash
commit as the change ([ADR 0002](../adr/0002-ai-use-and-attribution.md)). Each
record lists:

- **every seat:** agent id, served model, effort and verdict, and for a
  GPT seat its route (the codex CLI or the API, `docs/workflow.md` §2);
- **every finding:** what it was, its `origin:` (pre-existing, or
  introduced by fix round N) and `kind:` (correctness, contract,
  consistency between parts, missing mechanism, or other), and whether it
  was fixed, refuted (with the evidence) or deferred (with where it is
  tracked);
- **every round:** each dispatch's quota headroom where the lane reports
  it (no cost figures), and "uncalibrated on IronAFS" beside each verdict
  from an uncalibrated pin ([workflow](../workflow.md) §2, "Model routing");
- **from round 3 on:** the convergence note ([workflow](../workflow.md) §3,
  step 7), and for each fix round past the second, the controller's
  request, its recommendation, the owner's answer with its date, and the
  controller's check of the updated note before that round.

Records are written from the lanes' output and redacted: no paths outside the
repository, hostnames, cell names, keys or other projects. Raw transcripts are
not published, because they can contain secrets and personal data.
