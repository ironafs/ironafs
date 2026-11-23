// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for `wipe`. Under Miri they run the site's real block (ADR 0005,
//! sheet 24: every length from 0 to 257, each buffer first filled with a
//! non-zero pattern, the whole slice zero afterwards).

use super::wipe;

#[test]
fn wipe_zeroes_every_byte_of_every_length_from_0_to_257() {
    for length in 0..=257_usize {
        let mut buffer: Vec<u8> = (0..length)
            .map(|index| u8::try_from(index % 255).unwrap() + 1)
            .collect();
        assert!(buffer.iter().all(|byte| *byte != 0));
        wipe(&mut buffer);
        assert_eq!(buffer.len(), length);
        assert!(buffer.iter().all(|byte| *byte == 0), "{length}");
    }
}

#[test]
fn wipe_touches_only_the_slice_it_is_given() {
    let mut buffer = [0xFF_u8; 16];
    wipe(buffer.get_mut(4..12).unwrap());
    assert_eq!(buffer[..4], [0xFF; 4]);
    assert_eq!(buffer[4..12], [0; 8]);
    assert_eq!(buffer[12..], [0xFF; 4]);
}
