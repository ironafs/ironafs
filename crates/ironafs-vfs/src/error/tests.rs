// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for how each contract error reaches applications, and for its
//! message (coding standards rule 4: lowercase, no trailing period).

use alloc::string::ToString;

use super::{Surface, VfsError};

/// Every value of the error set, each once.
const ALL: [VfsError; 22] = [
    VfsError::NotFound,
    VfsError::Exists,
    VfsError::Access,
    VfsError::TokenRequired,
    VfsError::TokenExpired,
    VfsError::TokenRevoked,
    VfsError::SecurityFloor,
    VfsError::ReadOnly,
    VfsError::NoSpace,
    VfsError::QuotaExceeded,
    VfsError::CrossDevice,
    VfsError::Stale,
    VfsError::Busy,
    VfsError::Offline,
    VfsError::TimedOut,
    VfsError::NotYetStored,
    VfsError::Interrupted,
    VfsError::NotSupported,
    VfsError::WouldBlock,
    VfsError::Unreachable,
    VfsError::Conflict,
    VfsError::OutOfRange,
];

#[test]
fn token_and_floor_errors_reach_applications_as_permission() {
    for error in [
        VfsError::TokenRequired,
        VfsError::TokenExpired,
        VfsError::TokenRevoked,
        VfsError::SecurityFloor,
    ] {
        assert_eq!(error.surface(), Surface::Permission, "{error:?}");
    }
}

#[test]
fn out_of_range_reaches_applications_as_invalid_argument() {
    assert_eq!(VfsError::OutOfRange.surface(), Surface::InvalidArgument);
}

#[test]
fn every_other_error_reaches_applications_as_itself() {
    let indirect = [
        VfsError::TokenRequired,
        VfsError::TokenExpired,
        VfsError::TokenRevoked,
        VfsError::SecurityFloor,
        VfsError::OutOfRange,
    ];
    for error in ALL.into_iter().filter(|error| !indirect.contains(error)) {
        assert_eq!(error.surface(), Surface::Itself, "{error:?}");
    }
}

#[test]
fn error_messages_are_lowercase_without_a_trailing_period() {
    for error in ALL {
        let message = error.to_string();
        assert!(!message.is_empty(), "{error:?}");
        assert_eq!(message, message.to_lowercase(), "{error:?}");
        assert!(!message.ends_with('.'), "{error:?}");
    }
}

#[test]
fn error_messages_are_distinct() {
    for (index, error) in ALL.iter().enumerate() {
        for other in ALL.iter().skip(index + 1) {
            assert_ne!(error.to_string(), other.to_string(), "{error:?} {other:?}");
        }
    }
}

#[test]
fn token_errors_say_which_token_failure() {
    assert_eq!(VfsError::TokenRequired.to_string(), "no token for the cell");
    assert_eq!(VfsError::TokenExpired.to_string(), "the token has expired");
    assert_eq!(VfsError::TokenRevoked.to_string(), "the token was revoked");
    assert_eq!(
        VfsError::NotYetStored.to_string(),
        "not yet stored on the server"
    );
}
