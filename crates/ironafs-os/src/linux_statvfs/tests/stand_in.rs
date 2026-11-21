// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tests of `linux-statvfs` (ADR 0005, sheet 22) that only the Miri
//! stand-in can run, since it answers as planned: a query interrupted by a
//! signal is made again, and a failure is returned with its errno. A
//! wrapper that did not retry, or that dropped the errno, fails here.

use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;

use super::super::abi::stand_in::{Plan, calls, plan};
use super::super::measure;
use crate::space::SpaceError;

#[test]
fn an_interrupted_query_is_made_again() {
    let (socket, _peer) = UnixStream::pair().unwrap();
    let before = calls();
    plan(Plan::Interrupt(2));
    assert_eq!(measure(socket.as_fd()).unwrap(), 4_096_000);
    assert_eq!(calls() - before, 3);
}

#[test]
fn a_failed_query_is_returned_with_its_errno_and_not_made_again() {
    let (socket, _peer) = UnixStream::pair().unwrap();
    let before = calls();
    plan(Plan::Fail(38));
    assert!(matches!(
        measure(socket.as_fd()),
        Err(SpaceError::Failed(error)) if error.raw_os_error() == Some(38)
    ));
    assert_eq!(calls() - before, 1);
}
