// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the `EINTR` retry.

use std::io;

use super::retried;

#[test]
fn retried_repeats_an_interrupted_call_and_stops_at_any_other_result() {
    let mut calls = 0;
    let result = retried(|| {
        calls += 1;
        if calls < 3 {
            Err(io::Error::from(io::ErrorKind::Interrupted))
        } else {
            Ok(1)
        }
    });
    assert_eq!((result.unwrap(), calls), (1, 3));
    let mut calls = 0;
    let result = retried::<()>(|| {
        calls += 1;
        Err(io::Error::from_raw_os_error(32))
    });
    assert_eq!((result.unwrap_err().raw_os_error(), calls), (Some(32), 1));
}
