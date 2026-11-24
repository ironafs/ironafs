// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of the interim driver's turn: what a receive after a wait hands
//! over (ADR 0005, sheet 23: an error or a hangup the receive does not
//! explain is a failed receive), and the bounded retry of an interrupted
//! send.

use std::cell::Cell;
use std::io;

use ironafs_os::{Ready, Receipt, RouteError, Sent};

use super::{Turn, received, sent};

const READABLE: Ready = Ready {
    readable: true,
    error: false,
    hangup: false,
    woken: false,
};

#[test]
fn nothing_queued_after_an_error_or_a_hangup_is_a_failed_receive() {
    for ready in [
        Ready {
            readable: false,
            error: true,
            ..READABLE
        },
        Ready {
            readable: false,
            hangup: true,
            ..READABLE
        },
        Ready {
            error: true,
            ..READABLE
        },
        Ready {
            hangup: true,
            woken: true,
            ..READABLE
        },
    ] {
        assert!(
            matches!(
                received(ready, Ok(Receipt::NothingQueued)),
                Turn::Unexplained
            ),
            "{ready:?}"
        );
    }
}

#[test]
fn nothing_queued_after_readable_alone_and_an_interruption_are_idle() {
    assert!(matches!(
        received(READABLE, Ok(Receipt::NothingQueued)),
        Turn::Idle
    ));
    let hangup = Ready {
        hangup: true,
        ..READABLE
    };
    assert!(matches!(
        received(hangup, Ok(Receipt::Interrupted)),
        Turn::Idle
    ));
}

#[test]
fn a_datagram_and_an_error_are_handed_over() {
    assert!(matches!(
        received(READABLE, Ok(Receipt::Datagram(40))),
        Turn::Datagram(40)
    ));
    assert!(matches!(
        received(
            READABLE,
            Err(RouteError::Overflow(io::Error::from_raw_os_error(105)))
        ),
        Turn::Failed(RouteError::Overflow(_))
    ));
}

/// A send that answers each call with the next of `answers` (every byte
/// once they run out), and counts its calls.
fn scripted(
    answers: Vec<Sent>,
    calls: &Cell<usize>,
) -> impl FnMut() -> Result<Sent, RouteError> + '_ {
    let mut answers = answers.into_iter();
    move || {
        calls.set(calls.get() + 1);
        Ok(answers.next().unwrap_or(Sent::All))
    }
}

#[test]
fn an_interrupted_send_is_made_once_more_and_no_more() {
    let calls = Cell::new(0);
    assert!(sent(scripted(vec![Sent::Interrupted, Sent::All], &calls)));
    assert_eq!(calls.get(), 2);
    let calls = Cell::new(0);
    assert!(!sent(scripted(
        vec![Sent::Interrupted, Sent::Interrupted, Sent::All],
        &calls
    )));
    assert_eq!(calls.get(), 2);
}

#[test]
fn a_failed_send_is_not_made_again() {
    let calls = Cell::new(0);
    let send = || {
        calls.set(calls.get() + 1);
        Err(RouteError::WouldBlock)
    };
    assert!(!sent(send));
    assert_eq!(calls.get(), 1);
}
