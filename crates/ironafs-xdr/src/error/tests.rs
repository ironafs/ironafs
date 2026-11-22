// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

#![expect(
    clippy::disallowed_macros,
    reason = "the tests format errors to check their text"
)]

use alloc::format;

use super::{Error, ErrorKind};

const fn is_send_sync_static<T: Send + Sync + 'static>() {}
const _: () = is_send_sync_static::<Error>();

#[test]
fn display_names_kind_field_limit_and_position() {
    let error = Error::new(ErrorKind::CapExceeded)
        .named("test.note.body")
        .limit(16)
        .at(12, None);
    assert_eq!(
        format!("{error}"),
        "cap exceeded: test.note.body, limit 16, at offset 12"
    );
}

#[test]
fn display_of_truncated_gives_bytes_available() {
    let error = Error::new(ErrorKind::Truncated).at(8, Some(3));
    assert_eq!(
        format!("{error}"),
        "input truncated, at offset 8, 3 bytes there"
    );
}

#[test]
fn display_of_bare_kind_is_its_text() {
    assert_eq!(
        format!("{}", Error::new(ErrorKind::NonZeroPadding)),
        "non-zero padding"
    );
}

#[test]
fn accessors_return_what_was_built() {
    let error = Error::new(ErrorKind::TrailingBytes).at(44, Some(4));
    assert_eq!(error.kind(), ErrorKind::TrailingBytes);
    assert_eq!(error.name(), None);
    assert_eq!(error.limit_value(), None);
    assert_eq!(error.offset(), Some(44));
    assert_eq!(error.extent(), Some(4));
}

#[test]
fn every_kind_has_lower_case_text_without_trailing_period() {
    for kind in ErrorKind::ALL {
        let text = kind.text();
        assert!(!text.ends_with('.'), "{text}");
        assert_eq!(text, text.to_lowercase(), "{text}");
    }
}
