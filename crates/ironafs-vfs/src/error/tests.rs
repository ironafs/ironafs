// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for how each contract error reaches applications, and for its
//! message (coding standards rule 4: lowercase, no trailing period).

use alloc::string::ToString;

use super::{Surface, VfsError};

/// Builds, from one list, both `ALL` (every value, in the list's order) and
/// `expected`, an exhaustive match with no wildcard giving each value its
/// literal position and how it reaches applications. A new value does not
/// compile until it is in the list, and once it is, it is in `ALL`.
macro_rules! table {
    ($($error:ident = $position:literal => $surface:ident,)*) => {
        const ALL: &[VfsError] = &[$(VfsError::$error),*];

        fn expected(error: VfsError) -> (usize, Surface) {
            match error {
                $(VfsError::$error => ($position, Surface::$surface),)*
            }
        }
    };
}

// The surfaces are ADR 0004, C46, point 1's, written out without the
// crate's code; the last seven values are the contract specification's
// (5.12, point 9), which all reach applications as themselves.
table! {
    NotFound = 0 => Itself,
    Exists = 1 => Itself,
    Access = 2 => Itself,
    TokenRequired = 3 => Permission,
    TokenExpired = 4 => Permission,
    TokenRevoked = 5 => Permission,
    SecurityFloor = 6 => Permission,
    ReadOnly = 7 => Itself,
    NoSpace = 8 => Itself,
    QuotaExceeded = 9 => Itself,
    CrossDevice = 10 => Itself,
    Stale = 11 => Itself,
    Busy = 12 => Itself,
    Offline = 13 => Itself,
    TimedOut = 14 => Itself,
    NotYetStored = 15 => Itself,
    Interrupted = 16 => Itself,
    NotSupported = 17 => Itself,
    WouldBlock = 18 => Itself,
    Unreachable = 19 => Itself,
    Conflict = 20 => Itself,
    OutOfRange = 21 => InvalidArgument,
    NotADirectory = 22 => Itself,
    IsADirectory = 23 => Itself,
    NotEmpty = 24 => Itself,
    NoAttribute = 25 => Itself,
    TooSmall = 26 => Itself,
    PastEnd = 27 => Itself,
    Protocol = 28 => Itself,
}

#[test]
fn table_pins_each_error_at_its_literal_position() {
    for (index, error) in ALL.iter().copied().enumerate() {
        let (position, _) = expected(error);
        assert_eq!(position, index, "{error:?} is out of the table's order");
        assert_eq!(error as usize, position, "{error:?} is declared elsewhere");
    }
    assert_eq!(ALL.len(), 29);
}

#[test]
fn every_error_reaches_applications_as_c46_says() {
    for error in ALL.iter().copied() {
        assert_eq!(error.surface(), expected(error).1, "{error:?}");
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
fn added_errors_name_their_condition() {
    assert_eq!(VfsError::NotADirectory.to_string(), "not a directory");
    assert_eq!(VfsError::IsADirectory.to_string(), "is a directory");
    assert_eq!(VfsError::NotEmpty.to_string(), "the directory is not empty");
    assert_eq!(
        VfsError::NoAttribute.to_string(),
        "no such extended attribute"
    );
    assert_eq!(
        VfsError::TooSmall.to_string(),
        "the caller's buffer is too small"
    );
    assert_eq!(VfsError::PastEnd.to_string(), "past the end of the file");
    assert_eq!(
        VfsError::Protocol.to_string(),
        "the server's reply cannot be carried"
    );
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
