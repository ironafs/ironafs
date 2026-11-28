<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: the phased `ironafs-disk` row in ADR 0004 C4

Issue #213, Class S. The change amends ADR 0004 C4 in place:

- `ironafs-disk` joins point 3 as the one home of the Cargo feature `fault`, in a gate-only phase and a driver phase;
- point 3 also says who may turn `fault` on;
- point 4 gains the graph rules for `ironafs-disk`, the test-only crates and the three crates that do not depend on one another;
- three "Revisit if" triggers are added;
- after a redesign, a paragraph makes `ironafs-disk`, `ironafs-net` and `ironafs-sim` known by their names.

`check-deps` enforces the text. The seams specification, the test strategy and the ADR overview follow it. The base is `main` at `67b5075abd7893255fc1c6872552c21730e3e2b3`.

The amendment text and the code were written by `claude-code:claude-opus-5-5`. The wording of the name-identity paragraph was proposed by `glm:glm-5.4` (see "Departures").

## Seats and routes

| Seat | Served model, effort | Route | Calibration on IronAFS |
| --- | --- | --- | --- |
| A | `codex:gpt-6.2-astra`, xhigh | OpenAI Responses API (`openai_lane.py`), read-only repository tools | uncalibrated on IronAFS |
| B (text) | `glm:glm-5.4`, max | Claude Code on the Z.ai Coding Plan, read-only repository tools | calibrated at max on code ranges ([2026-11-24-glm54-max-pin](2026-11-24-glm54-max-pin.md)); its document-review calibration ([2026-11-27-48-glm54-document-calibration](2026-11-27-48-glm54-document-calibration.md)) covers High, not max, so it is uncalibrated on IronAFS for documents at max |
| B (code) | `glm:glm-5.4`, High | as above | calibrated for code review at High ([2026-11-24-glm54-pin](2026-11-24-glm54-pin.md)) |
| C (code, security) | `gemini:gemini-4-carbon-medium` | Antigravity CLI, inlined and tool-free | calibrated for code review ([2026-11-21-lane-calibration](2026-11-21-lane-calibration.md)); admitted to the security seat by [2026-11-22-gemini-security-seat](2026-11-22-gemini-security-seat.md) |
| D (name-identity text) | `gemini:gemini-4-carbon-high` | Antigravity CLI, inlined and tool-free | uncalibrated on IronAFS |

Every run passed its lane's completion gate:

- the OpenAI and GLM logs attest the served model, and every GLM run echoed its terminal marker;
- the Gemini runs echoed their markers and reported their thinking-token counts.

Quota headroom was not reported.

## Rounds

### The amendment text, first version

| Round | Range | Seat | Result |
| --- | --- | --- | --- |
| 1 | `67b5075..2de4f98` | A, uncalibrated on IronAFS | FAIL: six majors |
| 1 | `67b5075..2de4f98` | B max, uncalibrated on IronAFS for documents at max | PASS: three minors, the same defects as three of A's majors |
| 2 (fix round 1) | `2de4f98..6e71354` | A, uncalibrated on IronAFS | FAIL: four fixed, two partly (majors) |
| 2 (fix round 1) | `2de4f98..6e71354` | B max | FAIL: three fixed, one new minor (introduced by fix round 1; the same defect as A's first) |
| 3 (fix round 2) | `6e71354..0523c54` | A, uncalibrated on IronAFS | PASS: no finding |
| 3 (fix round 2) | `6e71354..0523c54` | B max | PASS: no finding; two observations, not findings |
| 4 (fix round 3, owner-permitted) | `0523c54..1a68d96` | A, uncalibrated on IronAFS | PASS: no finding |
| 4 (fix round 3, owner-permitted) | `0523c54..1a68d96` | B max | PASS: no finding |

Both seats judged in round 2 that `xtask` as the only requester of `fault` is sound. The owner accepted the exact text on 2026-11-27, at `1a68d96`, by the SHA-256 of the cumulative C4 diff from the base: `2aa56e26bccd83b59227a369fd332f24beec40ebceb4ec2e4a747f865de9d5be`. The reply was "accept #213 C4 text 2aa56e26".

### The code, binding declarations to resolved packages

| Round | Range | Seat | Result |
| --- | --- | --- | --- |
| 1 | `1a68d96..9c08e23` | A, uncalibrated on IronAFS | FAIL: six majors, five minors |
| 1 | `1a68d96..9c08e23` | B High, with the consistency pass | PASS: two minors |
| 1 | `1a68d96..9c08e23` | C, security | FAIL: two majors |
| 2 (code fix round 1, owner-permitted) | `9c08e23..8347286` | A, uncalibrated on IronAFS | FAIL: A1 to A11 fixed; four new majors |
| 2 (code fix round 1) | `9c08e23..8347286` | B High; refutation seat for C's two majors | PASS: B1 and B2 fixed; both refutations hold, residual a specification item |
| 2 (code fix round 1) | `9c08e23..8347286` | C, security | FAIL: its two routes refused; one new major (introduced by code fix round 1), one minor |
| 3 (code fix round 2, owner-permitted) | `8347286..9b0c60a` | A, uncalibrated on IronAFS | FAIL: two fixed, one partly; one not fixed; one new major (introduced by code fix round 2); one minor |
| 3 (code fix round 2) | `8347286..9b0c60a` | C, security | FAIL: one new major (introduced by code fix round 2); its minor still open |
| 3 | — | B | not run: the redesign was proposed instead |

### The redesign: the name-identity text (a new count)

| Round | Range | Seat | Result |
| --- | --- | --- | --- |
| 1 | `9b0c60a..e9d542a` | A, uncalibrated on IronAFS | PASS: two minors |
| 1 | `9b0c60a..e9d542a` | D, uncalibrated on IronAFS | FAIL: two majors, one minor |
| 2 (fix round 1) | `e9d542a..08f95ca` | A, uncalibrated on IronAFS; refutation seat on D1's build-dependency claim | PASS: no finding; the refutation holds |
| 2 (fix round 1) | `e9d542a..08f95ca` | D, uncalibrated on IronAFS | PASS: no finding |

The owner accepted the exact text on 2026-11-27, at `08f95ca`, by the SHA-256 of the cumulative C4 diff from the base: `505f4f45d20776ccb6e5655fe7f58e0f5e4f549f2592ac288edce3309db0dec1`. The reply was "accept #213 C4 text 505f4f45". This acceptance supersedes the first one.

### The redesign: the code (a new count)

| Round | Range | Seat | Result |
| --- | --- | --- | --- |
| 1 | `1a68d96..7d156d4` | A, uncalibrated on IronAFS | FAIL: two majors, seven minors |
| 1 | `1a68d96..7d156d4` | B High, with the consistency pass | PASS: three minors |
| 1 | `1a68d96..7d156d4`, production code | C, security | FAIL: two majors, one minor |
| 1 | `1a68d96..7d156d4`, tests | C, security | FAIL: five majors, one minor |
| 2 (fix round 1) | `7d156d4..97d3363` | A, uncalibrated on IronAFS | PASS: four minors |
| 2 (fix round 1) | `7d156d4..97d3363` | B High; refutation seat for four majors | PASS; all four refutations hold |
| 2 (fix round 1) | `7d156d4..97d3363`, the `crates/` diff | C, security | PASS: no finding |

Only minors remained, so under the stop rule they were fixed and recorded, with no further review round. The fix is `aae7732`, and the full check list passed on it.

## Executed evidence

- **Build dependencies (refutes the build-dependency part of D1).**
  - Setup: a scratch workspace whose member has a `[build-dependencies]` entry and no `build.rs`, an optional dependency no feature enables, and dependencies for `cfg(windows)` and `cfg(target_os = "haiku")`. A third-party crate in it has an optional dependency no feature enables.
  - Command: `cargo metadata --format-version 1 --all-features`, under cargo 1.100.0 (the pinned toolchain) and 1.97.1.
  - Both versions resolve:
    - the build dependency, with kind `build`;
    - the member's optional dependency;
    - both other-platform dependencies.
  - The third-party crate's unactivated optional dependency has no edge, and its package is not reported.
- **Lockfile and edges.** Probe: a patched registry helper with an unactivated optional dependency on the workspace crate. Neither `Cargo.lock` nor the resolved graph records the dependency.
- **Library names, renames and versions.**
  - Cargo refuses a library target name containing a hyphen.
  - An explicit `package = "ironafs-disk"` is reported as a rename.
  - Cargo refuses two local packages of one name and version.
- **Red tests.**
  - Each new self-test case of the redesign was run against the code before its fix, red first.
  - Where a case could not be red, the record names the mutation of production code that turns it red: the production table lists, the namesake id, and the distinct-crate predicate.

## Findings by origin and kind

The appendix lists every finding.

- **First text.** The findings were wrong rules (driver operations read as feature-only), missing owners (the reverse edge), and incomplete rules (requests from a crate's own features, indirect enabling).
- **Binding code.** After the first round, every open finding was of one kind: binding a declaration to the real `ironafs-disk` package. Each refinement of the binding opened a new identity case.
- **Redesign.** The text findings were scope and enforceability. The code findings were a shared dependency list for the direct and graph rules, the production policy in the self-test, and test coverage.

## Convergence

First text:

- A's open blockers and majors were 6, 2, 0, 0 across rounds 1 to 4.
- B's open findings were 3 minors, 1, 0, 0.
- Fix round 1 introduced one defect, and fix round 2 none.
- The kinds narrowed to wording.

Binding code:

- A's open blockers and majors were 6, 4, 3. C's were 2, 1, 1. B went from two minors to a pass.
- Code fix round 1 introduced at least three of the six findings open after it, and code fix round 2 at least three of four. Every open finding was declaration-to-package identity.
- The controller recommended stopping the binding and amending the text instead (below).

Name-identity text: A went from two minors to a pass; D went from two majors and one minor to a pass.

Redesign code:

- A went from two majors and seven minors to four minors; B passed throughout; C went from seven majors and two minors to a pass.
- The fix round introduced no blocker or major; the residual minors were refinements of its own fixes.

## Owner decisions and permissions

- **Text fix round 3 (past the second).**
  - Request: the controller recommended accepting the text as it stood, and offered a third round for B's two round-3 observations:
    - the `fuzz` sentence said "requests it neither" while the text separated a request from forwarding;
    - an optional `xtask` dependency turned on by its own feature was named neither a request nor forwarding.
  - Convergence note at the time: A 6, 2, 0; B 3, 1, 0; fix round 1 introduced one defect, fix round 2 none.
  - Owner, 2026-11-27: "I think we can go for a third fix round, we are not set in stone when it could help".
- **Code fix round 1 (past the change's two text fix rounds, so asked).**
  - Request: one fix round, the same three seats verifying, the text frozen.
  - Owner, 2026-11-27, as sent: "ermit code fix round 1 on #213".
- **Code fix round 2.**
  - Request: one round scoped to binding declarations to the real package. If identity gaps remained, there would be no third round, and a simplifying text change would be proposed instead.
  - Convergence note before the request: A 6, 4; C 2, 1; the fix had introduced at least three of the open findings; one kind.
  - Owner, 2026-11-27: "permit code fix round 2 on #213".
- **Redesign.**
  - Recommendation (the controller, after a refute-first read by `glm:glm-5.4` max): stop patching the binding. Amend C4 with a name-identity rule for the three crates, remove the binding code, and start a new count.
  - Owner, 2026-11-27: "Lets go with your recommendations".
- **Acceptances.**
  - First text: "accept #213 C4 text 2aa56e26".
  - Name-identity text: "accept #213 C4 text 505f4f45".

## Refutations and rulings

- **Binding code, C's two majors (round 1).**
  - The routes through a local crate excluded from the workspace, or a `[patch]` to a local path, were already refused: a local package outside the workspace is refused as not a member.
  - Refusing every edge from a non-member to a member is not a rule C4 gave `check-deps`.
  - B, as refutation seat, ruled that both refutations hold. The residual, a registry or git package that shares an architecture crate's name, was a specification item, which the name-identity text then settled.
- **Name-identity text, D1's claim** (that build dependencies of a crate with no build script are left out of the resolved graph). Refuted by the executed evidence above. A, as refutation seat, agreed. The rest of D1 was fixed.
- **Redesign code, four majors.** B, as refutation seat, ruled that all four refutations hold.
  - **Cp1** (`xtask`'s request unified into shipped binaries): C4 says the feature being on by Cargo's feature unification is not a request. It gives "no shipped build compiles `fault`" to S4's checks and the release-build check, and nothing to `check-deps` about default members or the resolver.
  - **Cp2:**
    - another crate's own `fault` feature that touches no `ironafs-disk` declaration is review's, by C4's text;
    - renaming the home is an amendment of C4, and a folder matching its crate's name is coding standards rule 20's;
    - requiring `ironafs-disk` to be present contradicts "the name's absence, with no declaration of it, is not refused".
  - **Ct4** (a two-hop reverse edge): C4 states the reverse edge as a rule on declarations, and point 4 lists the reachability bans. This was settled in the binding code's first round.
  - **Ct5** (a loop through declared edges): refuted by a run. A control and a unit test with a cycle and a diamond end with exactly one report. The coverage it asked for was added.

## Departures

- **GLM held no seat on the name-identity text.**
  - Why: `glm:glm-5.4` proposed the paragraph's wording, so GLM was an author family for that text.
  - Who sat instead: A and D, on the strongest row. DeepSeek was outside its off-peak window.
  - What GLM did do: it held seats on the code, and the refutation seat on the code's majors.
- **A and D are uncalibrated on IronAFS.** This record marks their verdicts so; their findings were each checked at source before being acted on.

No blocker, major or confirmed correctness or contract finding is open.

## Appendix: every finding

FR*n* is a fix round of the same count; CFR*n* a code fix round of the binding approach; R the redesign. Kinds: correctness, contract, consistency between parts, missing mechanism, other.

### First text

| Id | Origin | Severity | Kind | Finding | Disposition |
| --- | --- | --- | --- | --- | --- |
| T-A1 | pre-existing | major | correctness | Driver operations read as compiled only with `fault` | fixed, FR1 |
| T-A2 | pre-existing | major | contract | Requests from a crate's own features, and `default` reaching `fault` through another feature, not covered | fixed, FR1 and FR2 (partly fixed in FR1) |
| T-A3 | pre-existing | major | contract | "Never the reverse" had no owner, and its negative was not exercised | fixed, FR1 (owner) and FR2 (the driver phase's fixture) |
| T-A4 | pre-existing | major | contract | The shipped-build rule named only a positive control | fixed, FR1 |
| T-A5 | pre-existing | major | consistency between parts | The test-only rule granted development edges over other rules | fixed, FR1 |
| T-A6 | pre-existing | major | correctness | "`check-deps` refuses a break of each of these rules" was not true | fixed, FR1 |
| T-B1 | pre-existing | minor | contract | The same as T-A3 | fixed, FR1 |
| T-B2 | pre-existing | minor | contract | The same as T-A2 | fixed, FR1 |
| T-B3 | pre-existing | minor | correctness | The same as T-A6 | fixed, FR1 |
| T-A7 | FR1 | major | consistency between parts | `xtask` allowed to request from its own features while forwarding was refused (T-A2's residual) | fixed, FR2 |
| T-A8 | FR1 | major | contract | The reverse edge's negative not exercised by the driver phase's evidence (T-A3's residual) | fixed, FR2 |
| T-B4 | FR1 | minor | consistency between parts | The same as T-A7 | fixed, FR2 |
| T-B5 | pre-existing | observation | other | The `fuzz` sentence did not name forwarding | fixed, FR3 (owner-permitted) |
| T-B6 | pre-existing | observation | other | An optional `xtask` dependency turned on by its own feature named neither a request nor forwarding | fixed, FR3 |

### Binding code

| Id | Origin | Severity | Kind | Finding | Disposition |
| --- | --- | --- | --- | --- | --- |
| C-A1 | pre-existing | major | correctness | A layer reaching `ironafs-disk` through another crate not refused | fixed, CFR1 |
| C-A2 | pre-existing | major | correctness | A request on an optional dependency not refused as a request | fixed, CFR1 |
| C-A3 | pre-existing | major | correctness | A strong `d/x` turning on an optional dependency that asks for `fault` not seen | fixed, CFR1 |
| C-A4 | pre-existing | major | missing mechanism | Requests from packages outside the workspace not read | fixed, CFR1 |
| C-A5 | pre-existing | major | correctness | Declarations bound to the home by name only | fixed, CFR1, by a resolution binding; its residuals led to the redesign |
| C-A6 | pre-existing | major | contract | The reverse edge applied over reachability | fixed, CFR1 |
| C-A7 | pre-existing | minor | correctness | The three-crate rule did not require two distinct crates | fixed, CFR1 |
| C-A8 | pre-existing | minor | other | Forwarding plants ignored the enabled value | fixed, CFR1 |
| C-A9 | pre-existing | minor | contract | Self-test fixtures used their own third-party lists | fixed, CFR1 |
| C-A10 | pre-existing | minor | consistency between parts | The overview dropped "through normal or build dependencies" and overstated the plants | fixed, CFR1 |
| C-A11 | pre-existing | minor | consistency between parts | The test strategy's C4, point 7 row equated two kinds of evidence | fixed, CFR1 |
| C-B1 | pre-existing | minor | consistency between parts | Two readers of one feature table | fixed, CFR1 |
| C-B2 | pre-existing | minor | correctness | The reverse-edge diagnostic said "declares" for a transitive path | fixed, CFR1 |
| C-C1 | pre-existing | major | missing mechanism | Requests and edges from packages outside the workspace | request part fixed, CFR1; the local-path routes refuted (already refused); the rest a specification item; refutation held |
| C-C2 | pre-existing | major | missing mechanism | An architecture crate in the resolved graph outside the workspace | local-path route refuted (already refused); the registry and git residual settled by the name-identity text; refutation held |
| C-A12 | CFR1 | major | correctness | A custom-named or `rlib` library broke the binding | fixed, CFR2 |
| C-A13 | CFR1 | major | correctness | Declarations not bound one-to-one to their edges | partly fixed, CFR2; closed by the redesign |
| C-A14 | pre-existing | major | missing mechanism | An unactivated optional request in a non-member never seen | not fixed in CFR2; closed by the redesign |
| C-A15 | CFR1 | major | correctness | The reverse edge and the development legs matched by name against a namesake | fixed, CFR2; superseded by the redesign, which refuses the namesake |
| C-C3 | CFR1 | major | correctness | An explicit self-rename slipped the request check | fixed, CFR2 |
| C-C4 | pre-existing | minor | missing mechanism | The same as C-A14 | closed by the redesign |
| C-A16 | CFR2 | major | correctness | An inactive renamed alias for a registry namesake took the patched home's edge | closed by the redesign |
| C-A17 | CFR2 | major | correctness | An unrenamed declaration of a member with no library bound to nothing | closed by the redesign; regression plant kept |
| C-A18 | CFR2 | minor | other | A unit test did not exercise its case | closed by the redesign (test removed with the binding) |
| C-C5 | CFR2 | major | correctness | Two declarations merging into one resolved edge bound only one | closed by the redesign; regression plant kept |

### Name-identity text

| Id | Origin | Severity | Kind | Finding | Disposition |
| --- | --- | --- | --- | --- | --- |
| N-A1 | pre-existing | minor | contract | The name ban not scoped to the packages Cargo reports; `fuzz` not placed outside | fixed, FR1 |
| N-A2 | pre-existing | minor | contract | A declaration of a reserved name with no member left undecided | fixed, FR1 |
| N-D1 | pre-existing | major | contract | Point 4's rules over unresolved declarations, and forwarding | fixed in part, FR1 (they count as edges; forwarding added); the build-dependency claim refuted by execution, refutation held |
| N-D2 | pre-existing | major | missing mechanism | How `check-deps` reads Cargo's metadata not stated | fixed, FR1 |
| N-D3 | pre-existing | minor | consistency between parts | "This point" inside point 3, and the remedy's wording | fixed, FR1 |

### Redesign code

| Id | Origin | Severity | Kind | Finding | Disposition |
| --- | --- | --- | --- | --- | --- |
| R-A1 | pre-existing | major | correctness | Direct-list rules read only resolved edges | fixed, FR1 |
| R-A2 | pre-existing | major | contract | Table and empty-workspace plants did not use the production policy | fixed, FR1; residual (R-V2) fixed after the stop rule |
| R-A3 | pre-existing | minor | correctness | The home not read as a declarer of itself | fixed, FR1 |
| R-A4 | pre-existing | minor | contract | Inactive plants did not prove the edge absent | fixed, FR1 |
| R-A5 | pre-existing | minor | other | Namesake ids not matched; fixture crates addressed by name only | fixed, FR1; residual (R-V3) fixed after the stop rule |
| R-A6 | pre-existing | minor | other | A non-member requester not identified | fixed, FR1 |
| R-A7 | pre-existing | minor | consistency between parts | Two lists of the three names | fixed, FR1 |
| R-A8 | pre-existing | minor | consistency between parts | Declarations parsed twice | fixed, FR1 |
| R-A9 | pre-existing | minor | consistency between parts | The seams page's pure-crate sentence not qualified by kind | fixed, FR1 |
| R-B1 | pre-existing | minor | consistency between parts | The same as R-A7 | fixed, FR1 |
| R-B2 | pre-existing | minor | consistency between parts | The gate-only name written three times | fixed, FR1 |
| R-B3 | pre-existing | minor | consistency between parts | The same as R-A8 | fixed, FR1 |
| R-Cp1 | pre-existing | major | other | `xtask`'s request unified into shipped builds | refuted; refutation held |
| R-Cp2 | pre-existing | major | other | Another crate's own `fault`; renaming the home | refuted; refutation held |
| R-Cp3 | pre-existing | minor | correctness | An implicit optional-dependency feature `fault` reached by a strong path | fixed, FR1; residual (R-V1) fixed after the stop rule |
| R-Ct1 | pre-existing | major | other | Forwarding not tested through an enabling feature | coverage added, FR1 |
| R-Ct2 | pre-existing | major | other | Build declarations not tested | coverage added, FR1 |
| R-Ct3 | pre-existing | major | other | The target plant on an active platform | fixed, FR1 |
| R-Ct4 | pre-existing | major | other | A two-hop reverse edge | refuted; refutation held |
| R-Ct5 | pre-existing | major | other | A loop through declared edges | refuted by a run; coverage added; refutation held |
| R-Ct6 | pre-existing | minor | other | A vacuous no-home test; shadowing by alias | coverage added, FR1 |
| R-V1 | FR1 | minor | correctness | The strong-path walk followed a required dependency's name | fixed after the stop rule, `aae7732` |
| R-V2 | FR1 | minor | contract | `run` and the policy fixtures did not share one production entry point | fixed after the stop rule, `aae7732` |
| R-V3 | FR1 | minor | other | Namesake ids compared by substring | fixed after the stop rule, `aae7732` |
| R-V4 | FR1 | minor | correctness | The fixture writer set a crate's dependencies through a name lookup | fixed after the stop rule, `aae7732` |

## Measures

- Review runs: 8 for the first text; 8 for the binding code; 4 for the name-identity text; 7 for the redesign code. Three of those runs were also refutation seats.
- Acceptance defects found at pickup or in code review: the C4 text needed one redesign amendment, after two code fix rounds.

This record was written by `claude-code:claude-opus-5-5` for the maintainer.
