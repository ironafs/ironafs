<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# XDR shared-target boundary seeds (#59)

Class I fixture work for the at-cap and one-byte-over inputs in
[testing §5.2.1](../specs/testing.md#521-decoders-and-encoders-every-wire-ipc-configuration-and-on-disk-codec).
The shared XDR target tries all test types. Its largest message cap is
256 bytes for `test.every`. The two new tracked seeds share the same
synthetic 208-byte canonical prefix.

## Review

GLM 5.4 High reviewed `b260ec7..e5949c8` and passed with zero
findings. The controller checked the served model, effort, original
review marker and binary-inclusive diff SHA-256
`67483f07c506737b026f27b7f969116343e36b3ac0143d532cb57783b55e8413`.
The first launch stopped before review because the background process
could not find its CLI. The retry reached a conclusive result on the
same committed diff. Quota headroom was unavailable.

The independent inventory agrees with the controller's inventory.
`every_boundary_seed` uses `hex` and the adjacent fixture reader's
manifest-relative path convention. The at-cap test uses `traced` and
`encoded` for the accepted prefix and the local `reject` helper for the
longer input. The over-cap test uses the same reject helper. These
are the three added functions. The reviewer inspected the committed
seed bytes and both refusal mapping rows. The controller independently
verified the binary diff and seed digests and ran the tests.

## Evidence

- EXECUTED on arm64 macOS at the `e5949c8` tree. Package nextest passed
  146 tests, including both new controls. The two seeds have SHA-256
  `f0efca2b19d952639b2ce6519eed3f6bdfe660fabd2a9ad35fbac18f8ce2369b`
  and `f0c339463e122c4eeb9f1c5c052f283c566419ce8e66dca5d732403f49042f28`.
- EXECUTED on the same tree. `cargo --locked xtask check` exited zero.
  It ran 1,036 workspace tests, the doctests, the repository checks and
  license checks. Workspace Clippy with warnings denied and
  `cargo fmt --all --check` also passed.
- EXECUTED on the same tree. Local pinned-nightly replay and both
  [hosted Linux replay jobs](https://github.com/ironafs/ironafs/actions/runs/45868443503)
  each passed all eight XDR seeds and all 30 tracked seeds.

## Testing §5.2.1 checklist

| Rule | Coverage for this slice |
| --- | --- |
| D1–D3 | R10a asserts `TrailingBytes` at 208, extent 48, and the same ledger as the accepted prefix. It re-encodes the prefix exactly. The existing golden and property tests retain their coverage. |
| D4 | R10a reaches the maximum message cap. R12a asserts `MessageTooLarge` for `test.every` at offset 0, limit 256, extent 257 and zero reservations. |
| D5–D8 | The existing prefix, padding, discriminant, checked arithmetic and redaction tests retain their coverage. |
| D9 | The existing target executes each committed seed. The stable integration tests assert both exact outcomes. Shared stable replay and the engine length option remain under #45. |
| D10 | The existing small-stack tests retain their coverage. |

This records a partial delivery under #59. The remaining acceptance
inventory and dated Linux timed run stay open. #75 owns scheduled
execution.
