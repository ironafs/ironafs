// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Traced decode and per-input invariants for the XDR tests and fuzz target.

use ironafs_xdr::{Decode, Encode, Error, MessageType, Reservation, decode_traced, encode_into};

use super::types::{
    Blob, Deep, Entry, Every, Five, Flag, Ints, Label, Names, Note, Pair, Sealed, Shade, Shaded,
    Solos, Tags, Wide,
};

/// One traced decode: the result and every request the ledger let
/// through, under the given budget.
pub(super) fn traced_with_budget<'a, T: Decode<'a> + MessageType>(
    input: &'a [u8],
    budget: u64,
) -> (Result<T, Error>, Vec<Reservation>) {
    let mut seen = Vec::new();
    let result = decode_traced(input, budget, &mut |reservation| {
        seen.push(reservation);
    });
    (result, seen)
}

/// One decode's outcome: the re-encoding of the accepted value, or the
/// error, with the requests recorded. Comparing re-encodings rather than
/// values also covers the secret-bearing type, which has no equality.
type Outcome = (Result<Vec<u8>, Error>, Vec<Reservation>);

/// Decodes `input` once as `T` and checks 10.4's points 2 to 4.
#[track_caller]
fn decode_once<'a, T: Decode<'a> + Encode + MessageType>(input: &'a [u8]) -> Outcome {
    let (result, seen) = traced_with_budget::<T>(input, T::MESSAGE.budget);
    let message = T::MESSAGE;
    let total = seen.iter().try_fold(0_u64, |total, reservation| {
        total.checked_add(reservation.bytes)
    });
    assert!(
        total.is_some_and(|total| total <= message.budget),
        "{}: requests over the memory budget",
        message.name
    );
    let outcome = match result {
        Ok(value) => {
            let ratio = <T as Decode<'a>>::HEAP_RATIO;
            let length = u64::try_from(input.len()).ok();
            let bound = ratio
                .zip(length)
                .and_then(|(ratio, length)| ratio.checked_mul(length));
            assert!(
                total
                    .zip(bound)
                    .is_some_and(|(total, bound)| total <= bound),
                "{}: requests over r × L on an accepted input",
                message.name
            );
            let mut encoded = Vec::new();
            let encoding = encode_into(&value, &mut encoded);
            assert!(
                encoding.is_ok() && encoded == input,
                "{}: an accepted input does not re-encode to itself",
                message.name
            );
            Ok(encoded)
        }
        Err(error) => {
            assert!(
                error.offset().is_none_or(|offset| offset <= input.len()),
                "{}: an error offset past the input",
                message.name
            );
            Err(error)
        }
    };
    (outcome, seen)
}

/// Checks 10.4's points 2 to 5 for one type on one input: point 5 by
/// decoding it twice.
#[track_caller]
fn check<'a, T: Decode<'a> + Encode + MessageType>(input: &'a [u8]) {
    let first = decode_once::<T>(input);
    let second = decode_once::<T>(input);
    assert!(
        first == second,
        "{}: the same input gave another result",
        T::MESSAGE.name
    );
}

/// Checks every test type on one input, as specified in XDR §10.4.
///
/// # Panics
///
/// Panics if a decode or re-encode violates the fuzz target's checks.
#[track_caller]
pub(super) fn check_input(input: &[u8]) {
    check::<Shade>(input);
    check::<Shaded>(input);
    check::<Flag>(input);
    check::<Entry>(input);
    check::<Label>(input);
    check::<Note>(input);
    check::<Pair>(input);
    check::<Names>(input);
    check::<Tags>(input);
    check::<Solos>(input);
    check::<Ints>(input);
    check::<Five>(input);
    check::<Blob>(input);
    check::<Wide>(input);
    check::<Deep>(input);
    check::<Sealed<'_>>(input);
    check::<Every>(input);
}
