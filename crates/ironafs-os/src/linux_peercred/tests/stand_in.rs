// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The wrapper's failure paths, driven through `peer_credentials` itself
//! with the call's Miri stand-in answering as planned (ADR 0005, sheet 13:
//! the stand-in returns a short length and an over-long length for each
//! read, and a type other than `SOCK_STREAM`, each an error), so a wrapper
//! that ignored a call's status, length or type, or passed a constant on,
//! fails here. Built only under Miri, where the
//! stand-in replaces the call.

use std::os::unix::net::UnixStream;

use super::super::abi::stand_in::{Plan, plan};
use super::super::peer_credentials;
use crate::peer::PeerError;

/// `EBADF`.
const EBADF: i32 = 9;

#[track_caller]
fn planned(next: Plan) -> Result<crate::PeerCredentials, PeerError> {
    let (ours, _theirs) = UnixStream::pair().unwrap();
    plan(next);
    peer_credentials(&ours)
}

#[test]
fn a_short_and_an_over_long_length_from_the_call_are_refused() {
    for length in [8, 16] {
        match planned(Plan::Length(length)) {
            Err(PeerError::Length {
                returned,
                expected: 12,
            }) => assert_eq!(returned, usize::try_from(length).unwrap()),
            other => panic!("{length}: {other:?}"),
        }
    }
}

#[test]
fn a_short_and_an_over_long_length_from_the_type_read_are_refused() {
    for length in [2, 8] {
        match planned(Plan::TypeLength(length)) {
            Err(PeerError::Length {
                returned,
                expected: 4,
            }) => assert_eq!(returned, usize::try_from(length).unwrap()),
            other => panic!("{length}: {other:?}"),
        }
    }
}

#[test]
fn a_type_other_than_a_stream_is_refused() {
    // `SOCK_DGRAM` and `SOCK_SEQPACKET`.
    for kind in [2, 5] {
        match planned(Plan::Type(kind)) {
            Err(PeerError::NotStream(found)) => assert_eq!(found, kind),
            other => panic!("{kind}: {other:?}"),
        }
    }
}

#[test]
fn a_failed_type_read_is_refused_with_its_errno() {
    match planned(Plan::TypeFail(EBADF)) {
        Err(PeerError::Failed(error)) => assert_eq!(error.raw_os_error(), Some(EBADF)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_failed_call_is_refused_with_its_errno() {
    match planned(Plan::Fail(EBADF)) {
        Err(PeerError::Failed(error)) => assert_eq!(error.raw_os_error(), Some(EBADF)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn ids_of_minus_one_from_the_call_are_refused() {
    assert!(matches!(
        planned(Plan::NoCredentials),
        Err(PeerError::NoCredentials)
    ));
}

#[test]
fn a_socket_with_no_connected_peer_is_refused_before_the_call() {
    assert!(matches!(
        planned(Plan::NotConnected),
        Err(PeerError::NotConnected(_))
    ));
}

#[test]
fn the_planned_answer_is_used_once() {
    let _ = planned(Plan::Length(8));
    let (ours, _theirs) = UnixStream::pair().unwrap();
    assert!(peer_credentials(&ours).is_ok());
}
