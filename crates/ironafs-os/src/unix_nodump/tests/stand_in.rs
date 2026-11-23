// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The wrapper's failure paths, driven through `disable_dumps` itself with
//! the calls' Miri stand-ins answering as planned (ADR 0005, sheet 11): a
//! failure of each call is reported with its errno, and a read-back of
//! anything but 0 is refused. A wrapper that ignored a call's status, or
//! reported another errno, fails here. Built only under Miri, where the
//! stand-ins replace the calls.

use super::super::abi::stand_in::{Plan, plan};
use super::super::disable_dumps;
use crate::dump::DumpError;

/// `EPERM`.
const EPERM: i32 = 1;

/// `EINVAL`.
const EINVAL: i32 = 22;

/// `EFAULT`.
const EFAULT: i32 = 14;

#[test]
fn a_failure_of_each_call_is_reported_with_its_errno() {
    plan(Plan::LimitFail(EPERM));
    match disable_dumps() {
        Err(DumpError::Limit(error)) => assert_eq!(error.raw_os_error(), Some(EPERM)),
        other => panic!("{other:?}"),
    }
    plan(Plan::SetFail(EINVAL));
    match disable_dumps() {
        Err(DumpError::Set(error)) => assert_eq!(error.raw_os_error(), Some(EINVAL)),
        other => panic!("{other:?}"),
    }
    plan(Plan::ReadFail(EFAULT));
    match disable_dumps() {
        Err(DumpError::ReadBack(error)) => assert_eq!(error.raw_os_error(), Some(EFAULT)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_read_back_of_anything_but_zero_is_refused() {
    for value in [1, 2] {
        plan(Plan::Reads(value));
        match disable_dumps() {
            Err(DumpError::StillDumpable(found)) => assert_eq!(found, value),
            other => panic!("{value}: {other:?}"),
        }
    }
}
