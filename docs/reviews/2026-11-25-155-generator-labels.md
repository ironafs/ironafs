<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Review of property generator label minimums (#155)

Class I. This delivers the generator-reach check in
[testing §5.4, point 2](../specs/testing.md#54-quality-of-the-tests).
It extends the seeded property runner from issue #44.

## Evidence

- EXECUTED on arm64 macOS. The seed `0x42` control failed with label
  counting planted as a no-op. The labelled property incorrectly
  returned success below the declared thresholds.
- EXECUTED on the implemented runner. The below-minimum test fails
  the property and reports both a label reached once and an unreached
  label. The paired control passes at the exact minimum and reports
  its zero-count label. Other tests cover repeated marks in one case,
  counts through the first failing case, and independence of seed,
  choice bytes, shrinking and replay.
- EXECUTED on the implementation. The workspace passed 1,010 tests
  and its doctests. After aligning the below-minimum test's name with
  testing §10 at `5baadf4`, the controller reran all 207 package tests,
  formatting and package Clippy with warnings denied.

## Review

GLM 5.4 High reviewed `7873bac..1699800` and passed with zero
findings. The controller checked the model receipt, review check
code and exact patch hash. The reviewer noted the difference between
the acceptance-case name and the implementation's name. The controller
changed the identifier to `property_label_below_minimum_fails` and
verified that this rename was the only code change after the review.

The implementation follows the existing `Property` builder and seeded
runner. Reports use the existing notice and counterexample reporting.
The generator marks labels through `Source`; the doctest names only
`Property` and `Source`. A minimum shortfall fails the whole run with its
seed and label counts. The generated-input failure path still returns
its counterexample for replay.
