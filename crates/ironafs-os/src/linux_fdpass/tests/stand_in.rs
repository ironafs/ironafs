// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The wrappers' retries and failures, driven through `send_fd` and
//! `receive_fd` themselves with the calls' Miri stand-ins answering as
//! planned: a call interrupted by a signal is made again, and a failure is
//! returned with its errno. A wrapper that did not retry, or that dropped
//! the errno, fails here. Built only under Miri, where the stand-ins
//! replace the calls.

use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;

use super::super::abi::stand_in::{Plan, calls, plan};
use super::super::{receive_fd, send_fd};
use crate::fdpass::FdPassError;

/// `EPIPE` and `ECONNRESET`.
const EPIPE: i32 = 32;
const ECONNRESET: i32 = 104;

#[test]
fn an_interrupted_send_and_receive_are_each_made_again() {
    let (sender, receiver) = UnixStream::pair().unwrap();
    let (passed, _far) = UnixStream::pair().unwrap();
    let before = calls();
    plan(Plan::Interrupt(2));
    send_fd(&sender, passed.as_fd()).unwrap();
    assert_eq!(calls() - before, 3);
    let before = calls();
    plan(Plan::Interrupt(2));
    receive_fd(&receiver).unwrap();
    assert_eq!(calls() - before, 3);
}

#[test]
fn a_failed_send_is_returned_with_its_errno_and_not_made_again() {
    let (sender, _receiver) = UnixStream::pair().unwrap();
    let (passed, _far) = UnixStream::pair().unwrap();
    let before = calls();
    plan(Plan::Fail(EPIPE));
    match send_fd(&sender, passed.as_fd()) {
        Err(FdPassError::Send(error)) => assert_eq!(error.raw_os_error(), Some(EPIPE)),
        other => panic!("{other:?}"),
    }
    assert_eq!(calls() - before, 1);
}

#[test]
fn a_failed_receive_is_returned_with_its_errno() {
    let (_sender, receiver) = UnixStream::pair().unwrap();
    plan(Plan::Fail(ECONNRESET));
    match receive_fd(&receiver) {
        Err(FdPassError::Receive(error)) => assert_eq!(error.raw_os_error(), Some(ECONNRESET)),
        other => panic!("{other:?}"),
    }
}
