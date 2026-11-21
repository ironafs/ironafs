// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Linux name codec against an independent statement of what one path
//! component is: every short name exhaustively, and random names around
//! the length cap.

use ironafs_test_support::{Exhaustive, Property, Source};
use ironafs_vfs::{MAX_NAME_BYTES, linux};

/// One Linux path component, stated without the crate's code: 1 to 255
/// bytes, no `/`, no NUL, and neither `.` nor `..`.
fn is_component(bytes: &[u8]) -> bool {
    (1..=MAX_NAME_BYTES).contains(&bytes.len())
        && bytes != b"."
        && bytes != b".."
        && !bytes.iter().any(|byte| *byte == b'/' || *byte == 0)
}

#[track_caller]
fn assert_codec(bytes: &[u8]) {
    match linux::decode(bytes) {
        Ok(name) => {
            assert!(is_component(bytes), "accepted {bytes:?}");
            assert_eq!(linux::encode(&name), bytes);
        }
        Err(_) => assert!(!is_component(bytes), "refused {bytes:?}"),
    }
}

#[test]
fn linux_codec_accepts_exactly_the_components_of_up_to_two_bytes() {
    Exhaustive::new("every name of up to two bytes", 1 + 256 + 256 * 256).check(
        |source: &mut Source| source.bytes(0..=2),
        |bytes| assert_codec(bytes),
    );
}

#[test]
fn linux_codec_accepts_exactly_the_components_around_the_cap() {
    Property::new("names around the length cap").check(
        |source: &mut Source| {
            // Lengths at and next to the cap, or any length up to past it.
            let near = [1, MAX_NAME_BYTES - 1, MAX_NAME_BYTES, MAX_NAME_BYTES + 1];
            let length = if source.bool() {
                *source.pick(&near)
            } else {
                source.usize_in(0..=MAX_NAME_BYTES + 8)
            };
            // Bytes a component allows, then at most one that it does not.
            let mut bytes: Vec<u8> = source
                .bytes(length..=length)
                .into_iter()
                .map(|byte| {
                    if byte == b'/' || byte == 0 {
                        b'x'
                    } else {
                        byte
                    }
                })
                .collect();
            if !bytes.is_empty() && source.bool() {
                let at = source.usize_in(0..=bytes.len() - 1);
                bytes[at] = *source.pick(&[b'/', 0]);
            }
            bytes
        },
        |bytes| assert_codec(bytes),
    );
}
