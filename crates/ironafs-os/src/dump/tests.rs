// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the dump-control steps, with stand-ins for the system calls
//! (ADR 0005, point 6.5: Miri runs these).

use std::cell::RefCell;
use std::io;

use super::{DumpError, disable_with};

const EPERM: i32 = 1;
const EINVAL: i32 = 22;

fn errno(value: i32) -> io::Error {
    io::Error::from_raw_os_error(value)
}

/// Runs `disable_with` over stand-ins returning the given results, and
/// returns its result with the steps taken, in order.
fn run(
    limit: io::Result<()>,
    set: io::Result<()>,
    read: io::Result<i32>,
) -> (Result<(), DumpError>, Vec<&'static str>) {
    let taken = RefCell::new(Vec::new());
    let result = disable_with(
        || {
            taken.borrow_mut().push("limit");
            limit
        },
        || {
            taken.borrow_mut().push("set");
            set
        },
        || {
            taken.borrow_mut().push("read");
            read
        },
    );
    (result, taken.into_inner())
}

#[test]
fn disable_with_succeeds_when_every_step_succeeds_and_the_read_back_is_zero() {
    let (result, taken) = run(Ok(()), Ok(()), Ok(0));
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(taken, ["limit", "set", "read"]);
}

#[test]
fn disable_with_fails_on_any_read_back_but_zero() {
    for value in [1, 2, 3, -1, i32::MIN] {
        let (result, _) = run(Ok(()), Ok(()), Ok(value));
        assert!(
            matches!(result, Err(DumpError::StillDumpable(found)) if found == value),
            "{value}: {result:?}"
        );
    }
}

#[test]
fn disable_with_takes_every_step_after_a_failed_limit_and_reports_the_limit() {
    let (result, taken) = run(Err(errno(EPERM)), Ok(()), Ok(0));
    assert_eq!(taken, ["limit", "set", "read"]);
    match result {
        Err(DumpError::Limit(error)) => assert_eq!(error.raw_os_error(), Some(EPERM)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn disable_with_reports_a_failed_set_before_the_read_back() {
    let (result, taken) = run(Ok(()), Err(errno(EINVAL)), Ok(1));
    assert_eq!(taken, ["limit", "set", "read"]);
    match result {
        Err(DumpError::Set(error)) => assert_eq!(error.raw_os_error(), Some(EINVAL)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn disable_with_reports_a_failed_read_back() {
    let (result, _) = run(Ok(()), Ok(()), Err(errno(EINVAL)));
    match result {
        Err(DumpError::ReadBack(error)) => assert_eq!(error.raw_os_error(), Some(EINVAL)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn dump_errors_name_the_step_and_carry_the_errno() {
    let error = DumpError::Limit(errno(EPERM));
    assert!(error.to_string().contains("core-file limit"), "{error}");
    assert!(error.to_string().contains("os error 1"), "{error}");
    let error = DumpError::StillDumpable(2);
    assert_eq!(
        error.to_string(),
        "the dumpable attribute reads 2 after it was set to 0"
    );
}
