<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review record: wording tidy before going public

Class E, for issue #3. An independent privacy check before the repository
goes public asked for wording changes. The Windows research notes, the
research index and one review record now describe measurements as the
maintainer's without describing their source; two review records reduce
withdrawn planning material to generic wording while keeping the facts they
record. Two
statements of the SMB route's measurement gain its stated limit (to the
client's first packet). No other claim, evidence label or result changes.
Written by `claude-code:claude-opus-5-5`.

| Round | Range | Seat | Served model | Effort | Verdict |
|---|---|---|---|---|---|
| 1, review | `dda1473..2f44b3f` | GPT read-only | gpt-6.2-sol | high | PASS: 3 minor |
| 2, final-state check | `dda1473..24df47c` | GPT read-only, same session | gpt-6.2-sol | high | PASS: 2 minor |
| 2, second family | `dda1473..24df47c` | DeepSeek (peak window, owner's permission) | deepseek-flash | max | FAIL: 1 major, 2 minor |
| 3, fix verification | `24df47c..33889f9` | DeepSeek (peak window, owner's permission) | deepseek-flash | max | PASS: 1 minor, fixed in this record's wording |

Round 1's three minor findings were fixed in `126d58c`: the review record
again states that ADR 0001 dropped a licensing reference, without naming the
component; it again states the timing of the FSKit distribution check; and
the withdrawn plan file's name is gone from its remaining mentions.

Round 2's findings were fixed together:

- **Major (DeepSeek):** the 2026-11-19 research review record still
  described the Cloud Files measurements' source; so did the research
  index's definition of MEASURED and one row of the UNC note. All now say
  the maintainer's measurements.
- **Minor (DeepSeek):** the UNC note's candidates table and the Cloud Files
  note's answer stated the SMB route's measurement without its limit; both
  now say "to the client's first packet", with a full session not yet
  measured.
- **Minor (GPT):** this record now marks minor-only rounds PASS, records the
  rounds that followed, and names its writer by agent id.
