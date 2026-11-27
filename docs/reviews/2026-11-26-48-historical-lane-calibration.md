<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Integration of the first lane calibration (#48)

Class I. [PR #106](https://github.com/ironafs/ironafs/pull/106) carries the
[November 21 calibration](2026-11-21-lane-calibration.md) and its original
index entry. The archived file is byte-identical to `d521f50`, SHA-256
`019edab1bb3ee7b1d1c3a74c837b1a5ba256c7201d38e5a2059524986ea6eede`.

## Review

GLM 5.4 High via the Coding Plan and Gemini 4 Carbon Medium via agy
independently passed `05a038c..f769976` with zero findings. The controller
checked the exact inputs, verdicts and terminal readbacks. The agy result
records the requested model pin. The CLI envelope has no separate served
model field. Quota headroom was unavailable.

The independent path inventory agreed with the controller. The substantive
change adds a historical Markdown file and one index row. It adds zero
functions. The full-index binary diff SHA-256 is
`91f36dd861f1c43d784749237d0acd6dd046475090e3b25db57bc7e17612375c`.

The scores, result caveats and routing at the time are retained. The original
Gemini attestation limit remains explicit. Small differences between the
legacy rounded usage totals and per-run lines cannot be resolved from the
published evidence. Current model routing comes from the
[workflow](../workflow.md#2-roles-and-lanes) and the later calibration records
for [GLM High](2026-11-24-glm54-pin.md), [GLM Max](2026-11-24-glm54-max-pin.md)
and [Gemini's security seat](2026-11-22-gemini-security-seat.md).

## Evidence

EXECUTED at `f769976` on macOS ARM64. Formatting, workspace Clippy with
warnings denied, 1,036 nextest cases, doc-tests, REUSE and the frozen
license/dependency checks passed. [Hosted CI](https://github.com/ironafs/ironafs/actions/runs/45883404008)
passed all 17 jobs. This integrates one historical record. The remaining
calibration work stays open under #48.
