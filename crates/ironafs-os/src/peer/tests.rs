// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of the credential checks every peer-credential site shares
//! (ADR 0005, sheet 13: a short and an over-long length are each an error).

use std::io;

use super::{PeerCredentials, PeerError, Read, checked};

/// The size of Linux's `struct ucred`.
const UCRED: usize = 12;

fn read(result: io::Result<()>, returned: usize) -> Read {
    Read {
        result,
        returned,
        expected: UCRED,
        pid: 41,
        uid: 1000,
        gid: 100,
    }
}

#[test]
fn checked_hands_out_the_values_of_a_read_of_exactly_the_structure_s_size() {
    let credentials = checked(read(Ok(()), UCRED)).unwrap();
    assert_eq!(
        credentials,
        PeerCredentials {
            pid: 41,
            uid: 1000,
            gid: 100
        }
    );
    assert_eq!(
        (credentials.pid(), credentials.uid(), credentials.gid()),
        (41, 1000, 100)
    );
}

#[test]
fn checked_refuses_a_short_and_an_over_long_length() {
    for returned in [0, 4, 8, UCRED - 1, UCRED + 1, 16, usize::MAX] {
        match checked(read(Ok(()), returned)) {
            Err(PeerError::Length {
                returned: found,
                expected: UCRED,
            }) => assert_eq!(found, returned),
            other => panic!("{returned}: {other:?}"),
        }
    }
}

#[test]
fn checked_refuses_the_ids_a_socket_with_no_peer_credentials_reports() {
    // The kernel answers -1 for each id of a socket with no peer
    // credentials; -1 is no user's or group's id.
    for (uid, gid) in [(u32::MAX, 100), (1000, u32::MAX), (u32::MAX, u32::MAX)] {
        let found = checked(Read {
            uid,
            gid,
            ..read(Ok(()), UCRED)
        });
        assert!(matches!(found, Err(PeerError::NoCredentials)), "{found:?}");
    }
}

#[test]
fn checked_passes_a_failed_call_on_with_its_errno_whatever_the_length() {
    for returned in [0, UCRED] {
        match checked(read(Err(io::Error::from_raw_os_error(88)), returned)) {
            Err(PeerError::Failed(error)) => assert_eq!(error.raw_os_error(), Some(88)),
            other => panic!("{returned}: {other:?}"),
        }
    }
}

#[test]
fn peer_errors_name_what_failed() {
    let failed = PeerError::Failed(io::Error::from_raw_os_error(88));
    assert!(
        failed
            .to_string()
            .starts_with("reading the peer's credentials failed: "),
        "{failed}"
    );
    assert!(std::error::Error::source(&failed).is_some());
    let length = PeerError::Length {
        returned: 8,
        expected: UCRED,
    };
    assert_eq!(
        length.to_string(),
        "the peer's credentials came back 8 bytes long, not 12"
    );
}
