// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for a connection's codec state (fuse-codec.md 5.6, point 3).

use super::{FinishError, Session};
use crate::init::InitConfig;
use crate::reply::InitAnswer;
use crate::request::{DecodeError, Malformed, Request};

/// A request with this opcode, unique and body.
fn frame(opcode: u32, unique: u64, body: &[u8]) -> Vec<u8> {
    let len = u32::try_from(40 + body.len()).unwrap();
    [
        &len.to_le_bytes()[..],
        &opcode.to_le_bytes(),
        &unique.to_le_bytes(),
        &1u64.to_le_bytes(),
        &[0; 16],
        body,
    ]
    .concat()
}

/// An INIT at 7.`minor` with no flags.
fn init(unique: u64, minor: u32) -> Vec<u8> {
    let body = [7u32, minor, 4096, 0].map(u32::to_le_bytes).concat();
    frame(26, unique, &body)
}

fn config() -> InitConfig {
    InitConfig::new(4096, 131_072, 12, 9, 1).unwrap()
}

/// A session whose INIT at 7.`minor` was answered and finished.
fn ready(minor: u32) -> Session {
    let mut session = Session::new();
    let bytes = init(1, minor);
    let Ok((_, Request::Init { reply })) = session.decode(&bytes) else {
        panic!("init refused");
    };
    let (reply, answer) = reply.answer(&config());
    assert!(matches!(answer, InitAnswer::Ready(_)));
    session.finish(reply).unwrap();
    session
}

#[track_caller]
fn refused(session: &mut Session, bytes: &[u8]) -> Malformed {
    match session.decode(bytes) {
        Err(DecodeError::Request(error)) => error.malformed,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn a_frame_decoded_twice_is_refused_the_second_time() {
    let mut session = ready(40);
    let getattr = frame(3, 9, &[0; 16]);
    let Ok((_, Request::Getattr { reply, .. })) = session.decode(&getattr) else {
        panic!("first decode refused");
    };
    match session.decode(&getattr) {
        Err(DecodeError::Request(error)) => {
            assert_eq!(error.malformed, Malformed::DuplicateUnique);
            assert_eq!(error.errno(), None);
            assert_eq!(error.into_reply(), None);
        }
        other => panic!("second decode not refused: {other:?}"),
    }
    // Once the first is answered, the id may come again.
    session.finish(reply.error(crate::Errno::EIO)).unwrap();
    assert!(session.decode(&getattr).is_ok());
}

#[test]
fn a_duplicate_of_a_refused_request_is_refused_too() {
    let mut session = ready(40);
    let short = frame(3, 9, &[0; 15]);
    assert!(matches!(
        refused(&mut session, &short),
        Malformed::BodyLength { .. }
    ));
    assert_eq!(refused(&mut session, &short), Malformed::DuplicateUnique);
}

#[test]
fn requests_without_a_reply_take_no_id() {
    let mut session = ready(40);
    let forget = frame(2, 9, &3u64.to_le_bytes());
    assert!(session.decode(&forget).is_ok());
    assert!(session.decode(&forget).is_ok());
    assert_eq!(session.pending(), 0);
}

#[test]
fn finish_refuses_a_reply_of_another_session() {
    let mut first = ready(40);
    let mut second = ready(40);
    let getattr = frame(3, 9, &[0; 16]);
    let Ok((_, Request::Getattr { reply, .. })) = first.decode(&getattr) else {
        panic!("refused");
    };
    assert!(second.decode(&getattr).is_ok());
    assert_eq!(
        second
            .finish(reply.error(crate::Errno::EIO))
            .unwrap_err()
            .error,
        FinishError::OtherSession
    );
    assert_eq!(second.pending(), 1);
}

#[test]
fn the_phase_moves_only_when_a_settled_init_is_finished() {
    let mut session = Session::new();
    let bytes = init(1, 40);
    let Ok((_, Request::Init { reply })) = session.decode(&bytes) else {
        panic!("init refused");
    };
    let (reply, _) = reply.answer(&config());
    // Before the reply is finished, the session still awaits INIT.
    assert_eq!(session.minor(), None);
    assert_eq!(
        refused(&mut session, &frame(3, 2, &[0; 16])),
        Malformed::BeforeInit
    );
    session.finish(reply).unwrap();
    assert_eq!(session.minor(), Some(40));
}

#[test]
fn init_answers_in_the_shape_its_major_allows() {
    // Major 7: the INIT reply.
    let mut session = Session::new();
    let bytes = init(1, 46);
    let Ok((_, Request::Init { reply })) = session.decode(&bytes) else {
        panic!("refused");
    };
    let (reply, answer) = reply.answer(&config());
    assert!(matches!(answer, InitAnswer::Ready(negotiated) if negotiated.minor() == 46));
    assert_eq!(session.finish(reply).unwrap().len(), 80);

    // A newer major: the major alone, and the session still awaits INIT.
    let mut session = Session::new();
    let bytes = frame(26, 1, &8u32.to_le_bytes());
    let Ok((_, Request::Init { reply })) = session.decode(&bytes) else {
        panic!("refused");
    };
    let (reply, answer) = reply.answer(&config());
    assert_eq!(answer, InitAnswer::NewInitAwaited);
    let written = session.finish(reply).unwrap();
    assert_eq!(written.len(), 20);
    assert_eq!(written[16..20], 7u32.to_le_bytes());
    assert_eq!(session.minor(), None);

    // Below the floor: EPROTO.
    let mut session = Session::new();
    let bytes = init(1, 22);
    let Ok((_, Request::Init { reply })) = session.decode(&bytes) else {
        panic!("refused");
    };
    let (reply, answer) = reply.answer(&config());
    assert!(matches!(answer, InitAnswer::Refused(_)));
    let written = session.finish(reply).unwrap();
    assert_eq!(written[4..8], (-71i32).to_le_bytes());
    assert_eq!(session.minor(), None);
}

#[test]
fn statx_needs_7_39() {
    let body = [0u8; 24];
    let mut session = ready(38);
    let statx = frame(52, 9, &body);
    match session.decode(&statx) {
        Err(DecodeError::Request(error)) => {
            assert_eq!(error.malformed, Malformed::AboveMinor);
            assert_eq!(error.errno(), Some(crate::Errno::EOPNOTSUPP));
        }
        other => panic!("not refused: {other:?}"),
    }
    let mut session = ready(39);
    assert!(matches!(
        session.decode(&statx),
        Ok((_, Request::Statx { .. }))
    ));
}

#[test]
fn a_second_init_while_one_is_pending_is_refused() {
    let mut session = Session::new();
    let first = init(1, 40);
    let second = init(2, 46);
    let Ok((_, Request::Init { reply })) = session.decode(&first) else {
        panic!("first init refused");
    };
    let refusal = match session.decode(&second) {
        Err(DecodeError::Request(error)) => {
            assert_eq!(error.malformed, Malformed::InitPending);
            assert_eq!(error.errno(), Some(crate::Errno::EIO));
            error.into_reply().unwrap()
        }
        other => panic!("second init not refused: {other:?}"),
    };
    // The refusal answers the second INIT and settles nothing.
    session.finish(refusal).unwrap();
    assert_eq!(session.minor(), None);
    let (reply, _) = reply.answer(&config());
    session.finish(reply).unwrap();
    assert_eq!(session.minor(), Some(40));
}

#[test]
fn a_new_init_may_follow_a_finished_major_only_reply() {
    let mut session = Session::new();
    let newer = frame(26, 1, &8u32.to_le_bytes());
    let Ok((_, Request::Init { reply })) = session.decode(&newer) else {
        panic!("refused");
    };
    session.finish(reply.answer(&config()).0).unwrap();
    let Ok((_, Request::Init { reply })) = session.decode(&init(2, 46)) else {
        panic!("the kernel's new init refused");
    };
    session.finish(reply.answer(&config()).0).unwrap();
    assert_eq!(session.minor(), Some(46));
}

#[test]
fn a_no_reply_request_with_a_pending_id_is_refused() {
    let mut session = ready(40);
    assert!(session.decode(&frame(3, 9, &[0; 16])).is_ok());
    for (opcode, body) in [(2, 3u64.to_le_bytes()), (36, 5u64.to_le_bytes())] {
        match session.decode(&frame(opcode, 9, &body)) {
            Err(DecodeError::Request(error)) => {
                assert_eq!(error.malformed, Malformed::DuplicateUnique, "{opcode}");
                assert_eq!(error.into_reply(), None);
            }
            other => panic!("opcode {opcode} not refused: {other:?}"),
        }
    }
}

#[test]
fn decode_refuses_past_the_pending_cap() {
    let mut session = ready(40);
    let cap = super::MAX_PENDING;
    assert_eq!(cap, 4096);
    for unique in 0..u64::try_from(cap).unwrap() {
        assert!(session.decode(&frame(3, 1000 + unique, &[0; 16])).is_ok());
    }
    assert_eq!(session.pending(), cap);
    match session.decode(&frame(3, 99, &[0; 16])) {
        Err(DecodeError::Request(error)) => {
            assert_eq!(error.malformed, Malformed::Overloaded);
            assert_eq!(error.into_reply(), None);
        }
        other => panic!("not refused: {other:?}"),
    }
    assert_eq!(session.pending(), cap);
    // A request that takes no reply still decodes.
    assert!(session.decode(&frame(2, 99, &3u64.to_le_bytes())).is_ok());
}

#[test]
fn a_wrong_session_finish_hands_the_reply_back() {
    let mut first = ready(40);
    let mut second = ready(40);
    let getattr = frame(3, 9, &[0; 16]);
    let Ok((_, Request::Getattr { reply, .. })) = first.decode(&getattr) else {
        panic!("refused");
    };
    let refused = second.finish(reply.error(crate::Errno::EIO)).unwrap_err();
    assert_eq!(refused.error, FinishError::OtherSession);
    assert_eq!(first.finish(refused.reply).unwrap().len(), 16);
    assert_eq!(first.pending(), 0);
}
