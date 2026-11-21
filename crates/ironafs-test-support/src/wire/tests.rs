// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the wire-message variants.

use std::panic;

use super::{FixedWidth, edges, field_edges, flip_bit, near, single_bit_flips};

#[test]
fn flip_bit_counts_bits_from_first_byte_most_significant_first() {
    assert_eq!(flip_bit(&[0x00, 0x00], 0), vec![0x80, 0x00]);
    assert_eq!(flip_bit(&[0x00, 0x00], 7), vec![0x01, 0x00]);
    assert_eq!(flip_bit(&[0x00, 0x00], 8), vec![0x00, 0x80]);
    assert_eq!(flip_bit(&[0xff, 0xff], 15), vec![0xff, 0xfe]);
}

#[test]
fn flip_bit_past_end_panics_naming_bit_and_length() {
    let panic = panic::catch_unwind(|| flip_bit(&[0x00], 8)).unwrap_err();
    let message = panic.downcast_ref::<String>().unwrap();
    assert_eq!(message, "flip_bit: bit 8 is past a message of 1 bytes");
}

#[test]
fn single_bit_flips_gives_every_flip_once_in_bit_order() {
    let message = [0x0f, 0xa5, 0x00];
    let flips: Vec<Vec<u8>> = single_bit_flips(&message).collect();
    assert_eq!(flips.len(), 24);
    for (bit, flipped) in flips.iter().enumerate() {
        assert_eq!(*flipped, flip_bit(&message, bit), "bit {bit}");
        let differing: u32 = message
            .iter()
            .zip(flipped)
            .map(|(original, changed)| (original ^ changed).count_ones())
            .sum();
        assert_eq!(differing, 1, "bit {bit}");
    }
}

#[test]
fn single_bit_flips_of_empty_message_is_empty() {
    assert_eq!(single_bit_flips(&[]).count(), 0);
}

#[test]
fn near_gives_ascending_values_within_distance() {
    assert_eq!(near(100_u32, 2), vec![98, 99, 100, 101, 102]);
    assert_eq!(near(-1_i32, 1), vec![-2, -1, 0]);
    assert_eq!(near(7_u8, 0), vec![7]);
}

#[test]
fn near_stops_at_the_type_limits() {
    assert_eq!(near(1_u32, 3), vec![0, 1, 2, 3, 4]);
    assert_eq!(
        near(u32::MAX, 2),
        vec![u32::MAX - 2, u32::MAX - 1, u32::MAX]
    );
    assert_eq!(near(i64::MIN, 1), vec![i64::MIN, i64::MIN + 1]);
    assert_eq!(near(u64::MAX, 1), vec![u64::MAX - 1, u64::MAX]);
    assert_eq!(near(0_u8, u32::MAX).len(), 256);
}

#[test]
fn edges_are_all_zero_then_all_ones() {
    assert_eq!(edges::<u8>(), [0, 0xff]);
    assert_eq!(edges::<u16>(), [0, 0xffff]);
    assert_eq!(edges::<u32>(), [0, u32::MAX]);
    assert_eq!(edges::<u64>(), [0, u64::MAX]);
    assert_eq!(edges::<i8>(), [0, -1]);
    assert_eq!(edges::<i16>(), [0, -1]);
    assert_eq!(edges::<i32>(), [0, -1]);
    assert_eq!(edges::<i64>(), [0, -1]);
}

#[test]
fn fixed_width_all_ones_has_every_bit_set() {
    assert_eq!(i32::ALL_ONES.count_ones(), 32);
    assert_eq!(u64::ALL_ONES.count_ones(), 64);
}

#[test]
fn field_edges_sets_each_field_to_zero_then_ones() {
    let message = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    let variants: Vec<Vec<u8>> = field_edges(&message, 4).collect();
    assert_eq!(
        variants,
        vec![
            vec![0x00, 0x00, 0x00, 0x00, 0x55, 0x66, 0x77, 0x88],
            vec![0xff, 0xff, 0xff, 0xff, 0x55, 0x66, 0x77, 0x88],
            vec![0x11, 0x22, 0x33, 0x44, 0x00, 0x00, 0x00, 0x00],
            vec![0x11, 0x22, 0x33, 0x44, 0xff, 0xff, 0xff, 0xff],
        ]
    );
}

#[test]
fn field_edges_covers_a_short_last_field() {
    let variants: Vec<Vec<u8>> = field_edges(&[0x11, 0x22, 0x33], 2).collect();
    assert_eq!(
        variants,
        vec![
            vec![0x00, 0x00, 0x33],
            vec![0xff, 0xff, 0x33],
            vec![0x11, 0x22, 0x00],
            vec![0x11, 0x22, 0xff],
        ]
    );
}

#[test]
fn field_edges_of_zero_width_panics() {
    let panic = panic::catch_unwind(|| field_edges(&[0x11], 0).count()).unwrap_err();
    let message = panic.downcast_ref::<&str>().unwrap();
    assert_eq!(
        *message,
        "field_edges needs a field width of at least one byte"
    );
}
