// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The callback envelope round-trips every request and answer, and
//! rejects every truncation, any trailing byte and an unknown tag.

use super::{
    CbAnswer, CbProc, CbRequest, Reset, decode_cb_answer, decode_cb_request, encode_cb_answer,
    encode_cb_request,
};
use crate::model::envelope::EnvelopeError;
use crate::model::types::{Addr, ClientId, Fid};

fn requests() -> Vec<CbRequest> {
    [
        CbProc::CallBack {
            fids: vec![Fid::new(100, 2, 1), Fid::new(u32::MAX, 0, 7)],
        },
        CbProc::CallBack { fids: Vec::new() },
        CbProc::InitState(Reset::One),
        CbProc::InitState(Reset::Two),
        CbProc::InitState(Reset::Three),
        CbProc::Probe,
    ]
    .into_iter()
    .map(|procedure| CbRequest {
        from: Addr([10, 0, 0, 2]),
        to: ClientId(3),
        procedure,
    })
    .collect()
}

#[test]
fn every_request_and_answer_round_trips() {
    for request in requests() {
        let bytes = encode_cb_request(&request);
        assert_eq!(decode_cb_request(&bytes), Ok(request));
    }
    for answer in [CbAnswer::Empty, CbAnswer::Abort(-455)] {
        assert_eq!(decode_cb_answer(&encode_cb_answer(&answer)), Ok(answer));
    }
}

#[test]
fn every_truncation_and_a_trailing_byte_are_refused() {
    for request in requests() {
        let bytes = encode_cb_request(&request);
        for cut in 0..bytes.len() {
            assert_eq!(
                decode_cb_request(bytes.get(..cut).unwrap()),
                Err(EnvelopeError::Truncated),
                "{request:?} cut at {cut}"
            );
        }
        let mut long = bytes.clone();
        long.push(0);
        assert_eq!(decode_cb_request(&long), Err(EnvelopeError::Trailing));
    }
    let answer = encode_cb_answer(&CbAnswer::Abort(-455));
    assert_eq!(
        decode_cb_answer(answer.get(..3).unwrap()),
        Err(EnvelopeError::Truncated)
    );
    let mut long = encode_cb_answer(&CbAnswer::Empty);
    long.push(9);
    assert_eq!(decode_cb_answer(&long), Err(EnvelopeError::Trailing));
}

#[test]
fn an_unknown_tag_and_a_count_past_the_bytes_are_refused() {
    let mut bytes = encode_cb_request(&requests().remove(5));
    *bytes.last_mut().unwrap() = 9;
    assert_eq!(decode_cb_request(&bytes), Err(EnvelopeError::Tag));
    let mut reset = encode_cb_request(&requests().remove(2));
    *reset.last_mut().unwrap() = 3;
    assert_eq!(decode_cb_request(&reset), Err(EnvelopeError::Tag));
    assert_eq!(decode_cb_answer(&[2]), Err(EnvelopeError::Tag));
    // A callback whose count claims more file ids than the bytes hold.
    let mut huge = vec![10, 0, 0, 2, 0, 0, 0, 3, 0];
    huge.extend_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(decode_cb_request(&huge), Err(EnvelopeError::Truncated));
}

#[test]
fn each_procedure_has_its_specification_name() {
    let names: Vec<&str> = requests()
        .iter()
        .map(|request| request.procedure.name())
        .collect();
    assert_eq!(
        names,
        [
            "cb.callback",
            "cb.callback",
            "cb.init_state",
            "cb.init_state2",
            "cb.init_state3",
            "cb.probe"
        ]
    );
}
