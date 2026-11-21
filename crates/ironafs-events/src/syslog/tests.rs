// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the RFC 5424 header fields (`docs/specs/events.md` §5.9).

use alloc::string::String;

use super::{AppName, HOSTNAME_MAX, Origin, OriginError, SYSLOG_FALLBACK_MAX, SyslogEncoder};
use crate::redaction::Policy;

#[test]
fn app_name_refuses_empty_long_and_unprintable_names() {
    assert_eq!(AppName::new(""), Err(OriginError::EmptyAppName));
    assert_eq!(
        AppName::new(&"a".repeat(49)),
        Err(OriginError::AppNameTooLong)
    );
    assert_eq!(
        AppName::new("iron afs"),
        Err(OriginError::AppNameNotPrintable)
    );
    assert_eq!(
        AppName::new("ironafs\u{e9}"),
        Err(OriginError::AppNameNotPrintable)
    );
    assert_eq!(
        AppName::new(&"a".repeat(48)).map(|name| name.as_str().len()),
        Ok(48)
    );
}

#[test]
fn a_host_name_outside_the_rules_is_the_nil_value() {
    let app = AppName::new("ironafsd").unwrap();
    for bad in [
        String::new(),
        "a".repeat(HOSTNAME_MAX + 1),
        String::from("host name"),
    ] {
        assert_eq!(
            Origin::new(&bad, app.clone(), None).hostname,
            None,
            "{bad:?}"
        );
    }
    let longest = "a".repeat(HOSTNAME_MAX);
    assert_eq!(Origin::new(&longest, app, None).hostname, Some(longest));
}

#[test]
fn with_cap_never_raises_the_cap() {
    let origin = Origin::new("h", AppName::new("ironafsd").unwrap(), None);
    let encoder = SyslogEncoder::new(origin, Policy::Routine);
    assert_eq!(
        encoder.clone().with_cap(usize::MAX).cap,
        SYSLOG_FALLBACK_MAX
    );
    assert_eq!(encoder.with_cap(100).cap, 100);
}
