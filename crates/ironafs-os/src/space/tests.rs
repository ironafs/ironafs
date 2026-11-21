// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of the free-space arithmetic (ADR 0005, sheet 22).

use super::{SpaceError, available};

#[test]
fn available_multiplies_the_free_blocks_by_the_fragment_size() {
    assert_eq!(available(1000, 4096).unwrap(), 4_096_000);
    assert_eq!(available(0, 4096).unwrap(), 0);
    assert_eq!(available(7, 0).unwrap(), 0);
    assert_eq!(available(u64::MAX, 1).unwrap(), u64::MAX);
}

#[test]
fn available_refuses_a_product_that_does_not_fit_64_bits() {
    for (blocks, fragment) in [(u64::MAX, 2), (1 << 52, 1 << 12), (u64::MAX, u64::MAX)] {
        match available(blocks, fragment) {
            Err(SpaceError::Overflow {
                blocks: found_blocks,
                fragment: found_fragment,
            }) => assert_eq!((found_blocks, found_fragment), (blocks, fragment)),
            other => panic!("{blocks} x {fragment}: {other:?}"),
        }
    }
    assert_eq!(
        available((1 << 52) - 1, 1 << 12).unwrap(),
        u64::MAX - (1 << 12) + 1
    );
}
