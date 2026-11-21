// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for how each contract error reaches applications, and for its
//! message (coding standards rule 4: lowercase, no trailing period).

use alloc::string::ToString;

use super::{Surface, VfsError};

/// Every value of the error set, each once, in the order of `position`.
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

/// Each value's place in `ALL`. The match has no wildcard, so a new value
/// does not compile here until it is given a place, and
/// `all_lists_every_error_once` then fails until `ALL` holds it.
fn position(error: VfsError) -> usize {
    match error {
        VfsError::NotFound => 0,
        VfsError::Exists => 1,
        VfsError::Access => 2,
        VfsError::TokenRequired => 3,
        VfsError::TokenExpired => 4,
        VfsError::TokenRevoked => 5,
        VfsError::SecurityFloor => 6,
        VfsError::ReadOnly => 7,
        VfsError::NoSpace => 8,
        VfsError::QuotaExceeded => 9,
        VfsError::CrossDevice => 10,
        VfsError::Stale => 11,
        VfsError::Busy => 12,
        VfsError::Offline => 13,
        VfsError::TimedOut => 14,
        VfsError::NotYetStored => 15,
        VfsError::Interrupted => 16,
        VfsError::NotSupported => 17,
        VfsError::WouldBlock => 18,
        VfsError::Unreachable => 19,
        VfsError::Conflict => 20,
        VfsError::OutOfRange => 21,
    }
}

/// How each value reaches applications, from ADR 0004, C46, point 1,
/// written out without the crate's code and with no wildcard arm.
fn expected_surface(error: VfsError) -> Surface {
    match error {
        VfsError::TokenRequired
        | VfsError::TokenExpired
        | VfsError::TokenRevoked
        | VfsError::SecurityFloor => Surface::Permission,
        VfsError::OutOfRange => Surface::InvalidArgument,
        VfsError::NotFound
        | VfsError::Exists
        | VfsError::Access
        | VfsError::ReadOnly
        | VfsError::NoSpace
        | VfsError::QuotaExceeded
        | VfsError::CrossDevice
        | VfsError::Stale
        | VfsError::Busy
        | VfsError::Offline
        | VfsError::TimedOut
        | VfsError::NotYetStored
        | VfsError::Interrupted
        | VfsError::NotSupported
        | VfsError::WouldBlock
        | VfsError::Unreachable
        | VfsError::Conflict => Surface::Itself,
    }
}

#[test]
fn all_lists_every_error_once() {
    for (index, error) in ALL.into_iter().enumerate() {
        assert_eq!(position(error), index, "{error:?}");
    }
}

#[test]
fn every_error_reaches_applications_as_c46_says() {
    for error in ALL {
        assert_eq!(error.surface(), expected_surface(error), "{error:?}");
    }
}

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
