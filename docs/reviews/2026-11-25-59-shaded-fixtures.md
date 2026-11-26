<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# XDR closed-enum default-arm fixtures (#59)

Class I test slice. The [XDR profile](../specs/xdr.md#10-tests) specifies
G14, G15 and R9b and assigns their executable delivery to #59. The
fixture uses a closed `Shade` and three equal-width arms. PALE and DEEP
select explicit arms, and VIVID selects the default. The type represents
only those valid pairings. The encoder's refusal of an inconsistent pair
belongs to the separate #165 implementation under #59.

## Review

GLM 5.4 High reviewed `2c1fda3..ce6ef89`, with binary-inclusive patch
SHA-256 `50647a12e1457c9fb3bd052c6bc80273070cf31aeab8de1d20c6977d181089c0`.
The first turn ended at its turn limit. The continuation matched the
original session and marker and passed with one minor. GLM 5.4 High
verified `ce6ef89..556edc9` and passed with zero findings. Both
conclusive verdicts matched their pinned files and check codes. Quota
headroom was unavailable in the results.

- F1 (`origin: pre-existing`, `kind: contract`). The original seeded
  property exercised encode/decode and size and omitted the shared
  canonical re-encoding and proper-prefix checks. The correction uses
  `all_properties` with the same three-arm generator and an independent
  12-byte size calculation. The final verification also reviewed the
  `Shaded` addition to the existing small-stack test.

The consistency pass confirmed use of `Shade::declared`, the existing
corpus reader and trace assertions, the shared property and stack
helpers, and the fuzz target's shared test types. The N1 claim names the
existing corpus test, the specification, and R9b. The repo specifies
those three fields. A checker for the claim file is future test work.

## Evidence

- INSPECTED the [XDR union and enumeration rules in RFC 4506](https://www.rfc-editor.org/rfc/rfc4506.html#section-4.15)
  and the profile's enum validation, arm selection, and field naming.
  An independent byte calculation matched all three 12-byte seed files.
- EXECUTED. On arm64 macOS, the corpus case initially failed because
  the new type was unregistered. After the test-only type was added,
  129 XDR tests passed. Temporarily selecting the default arm for an
  undeclared discriminant in the test fixture made the R9b assertion
  fail with successful decode. G14, G15 and the original round-trip
  control still passed. The plant was removed and the focused suite
  passed. The corpus checks `UnknownEnumValue`, `test.shaded.tone`,
  offset zero, and zero reservation requests.
- EXECUTED the final focused property and small-stack tests. The latter
  succeeds with a full 12-byte default-arm value on the 64 KiB stack.
- EXECUTED local `cargo --locked xtask fuzz` on arm64 macOS. Six XDR
  seeds and all 28 workspace seeds ran clean. Required Linux replay
  runs in the pull request's x64 and arm64 fuzz jobs.

## Testing §5.2.1 checklist

| Rule | Coverage for this slice |
| --- | --- |
| D1–D3 | G14 and G15 check both directions. R9b checks exact kind, field, offset and empty ledger. The seeded properties round-trip values and re-encode accepted inputs. |
| D4 | Both valid vectors meet the fixture's 12-byte message cap. The existing decode entry-point tests cover message-cap refusal. |
| D5–D6 | The shared test checks every proper prefix. R9b exercises the undeclared discriminant, while G14 and G15 exercise distinct declared selectors. |
| D7–D8 | The fixture uses the existing fixed-width primitives and contains public values. Arithmetic and secret-field controls remain in the codec suite. |
| D9 | The shared XDR target checks repeatability, byte equality on re-encode, error offsets, and the ledger. Its three new 12-byte seeds cover these fixture cases. Target-wide input limit and over-cap corpus obligations remain under #59. |
| D10 | The existing 64 KiB test checks successful decoding at the fixture's full width. |
