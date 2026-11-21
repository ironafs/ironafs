// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exhaustive sweeps over a known wire message's variants: every single-bit
//! flip, the length field near its value and at its edges, each field at
//! all zeros and all ones.

use ironafs_test_support::{Exhaustive, Source, SweepFailure, wire};

/// A planted frame: a big-endian `u32` length, then that many bytes.
const GOLDEN: [u8; 7] = [0, 0, 0, 3, b'a', b'f', b's'];

/// The payload of a frame, refusing a length past the end.
fn decode(frame: &[u8]) -> Result<&[u8], &'static str> {
    let (prefix, rest) = frame.split_first_chunk::<4>().ok_or("short header")?;
    let length = usize::try_from(u32::from_be_bytes(*prefix)).map_err(|_| "length")?;
    rest.get(..length).ok_or("length past the end")
}

/// A planted wrong decoder: it adds the header's length before the
/// bounds check, so an all-ones length wraps and is accepted.
fn decode_wrapping(frame: &[u8]) -> Result<&[u8], &'static str> {
    let (prefix, rest) = frame.split_first_chunk::<4>().ok_or("short header")?;
    let length = u32::from_be_bytes(*prefix);
    if usize::try_from(length.wrapping_add(4)).map_err(|_| "length")? <= frame.len() {
        let length = usize::try_from(length).map_err(|_| "length")?;
        // Trusts the check above, so takes what is there.
        Ok(rest.get(..length).unwrap_or(rest))
    } else {
        Err("length past the end")
    }
}

/// The length fields swept: near the golden frame's length, and at both
/// edges.
fn lengths() -> Vec<u32> {
    wire::near(3_u32, 2)
        .into_iter()
        .chain(wire::edges::<u32>())
        .collect()
}

/// The golden frame with one of [`lengths`] in its length field.
pub(crate) fn frame_with_length(source: &mut Source) -> Vec<u8> {
    let mut frame = GOLDEN.to_vec();
    frame.splice(..4, source.pick(&lengths()).to_be_bytes());
    frame
}

/// `decoder` returns a payload exactly as long as the frame's length
/// field, or refuses the frame.
fn payload_matches_length(
    decoder: fn(&[u8]) -> Result<&[u8], &'static str>,
    frame: &[u8],
) -> Result<(), String> {
    let header = frame
        .first_chunk::<4>()
        .map(|prefix| u32::from_be_bytes(*prefix));
    match decoder(frame) {
        Ok(payload) if u32::try_from(payload.len()).ok() == header => Ok(()),
        Ok(payload) => Err(format!(
            "accepted a payload of {} bytes for length {header:?}",
            payload.len()
        )),
        Err(_) => Ok(()),
    }
}

/// [`decode`] returns a payload exactly as long as the length field, or
/// refuses the frame.
#[expect(
    clippy::ptr_arg,
    reason = "rule 15: a sweep and its pinned regression pass the generator's own `&Vec<u8>`"
)]
pub(crate) fn decoded_payload_matches_length(frame: &Vec<u8>) -> Result<(), String> {
    payload_matches_length(decode, frame)
}

/// One single-bit flip of the golden frame, by bit index.
pub(crate) fn flipped_golden(source: &mut Source) -> Vec<u8> {
    let bit = source.usize_in(0..=GOLDEN.len() * 8 - 1);
    wire::flip_bit(&GOLDEN, bit)
}

/// The frame decodes to a payload no longer than its bytes, or is refused
/// for its length.
#[expect(
    clippy::ptr_arg,
    reason = "rule 15: a sweep and its pinned regression pass the generator's own `&Vec<u8>`"
)]
pub(crate) fn decodes_within_frame(frame: &Vec<u8>) -> Result<(), String> {
    match decode(frame) {
        Ok(payload) if payload.len() + 4 <= frame.len() => Ok(()),
        Ok(payload) => Err(format!("payload of {} bytes", payload.len())),
        Err("length past the end") => Ok(()),
        Err(other) => Err(other.to_owned()),
    }
}

#[test]
fn check_every_single_bit_flip_decodes_or_is_refused() {
    let bits = u64::try_from(GOLDEN.len() * 8).unwrap();
    Exhaustive::new("every bit flip of a frame", bits).check(flipped_golden, decodes_within_frame);
}

#[test]
fn run_single_bit_flips_counts_every_bit() {
    let swept = Exhaustive::new("bit flip count", 1000)
        .run(flipped_golden, |_: &Vec<u8>| ())
        .unwrap();
    assert_eq!(swept, 56);
}

#[test]
fn check_length_near_and_at_edges_decodes_or_is_refused() {
    Exhaustive::new("lengths near and at the edges", 7)
        .check(frame_with_length, decoded_payload_matches_length);
}

#[test]
fn run_length_edges_find_planted_wrapping_bug() {
    let failure = Exhaustive::new("lengths near and at the edges", 7)
        .run(frame_with_length, |frame: &Vec<u8>| {
            payload_matches_length(decode_wrapping, frame)
        })
        .unwrap_err();
    let SweepFailure::Failed(failed) = failure else {
        panic!("expected a failed input, got {failure}");
    };
    assert_eq!(failed.value()[..4], [0xff; 4]);
    assert_eq!(failed.choices(), &[6]);
    assert_eq!(
        failed.message(),
        "accepted a payload of 3 bytes for length Some(4294967295)"
    );
}

#[test]
fn run_field_edges_all_decode_or_are_refused() {
    let variants: Vec<Vec<u8>> = wire::field_edges(&GOLDEN, 4).collect();
    let swept = Exhaustive::new("frame fields at their edges", 4)
        .run(
            |source| source.pick(&variants).clone(),
            decodes_within_frame,
        )
        .unwrap();
    assert_eq!(swept, 4);
}

#[test]
fn run_domain_over_bound_is_refused_not_sampled() {
    let failure = Exhaustive::new("every u32", 1 << 16)
        .run(Source::u32, |_: &u32| ())
        .unwrap_err();
    assert!(matches!(
        failure,
        SweepFailure::TooLarge { bound: 65_536, .. }
    ));
}
