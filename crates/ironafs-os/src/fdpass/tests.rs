// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of the receive checks (ADR 0005, sheet 14), with a stand-in
//! descriptor that records when it is adopted and when it is dropped, so
//! each case shows which integers were adopted and that none outlives a
//! refusal. The sweep crosses the header's fields with the flags, the
//! count and the data byte.

use std::cell::RefCell;
use std::io;
use std::rc::Rc;

use super::{FdPassError, LINUX, Received, accept, retried, sent};

mod held;

use held::{Log, adopter};

/// A valid receive of descriptor 7.
const ONE: Received = Received {
    count: 1,
    flags: 0,
    control: 24,
    length: 20,
    level: 1,
    kind: 1,
    payload: [7, 0],
    byte: 1,
};

/// Runs `accept` over `received`; returns the descriptor handed out, if
/// any, the error, if any, and the log, after the result was dropped (so a
/// descriptor handed out is in `dropped` too).
fn run(received: &Received) -> (Option<i32>, Option<FdPassError>, Log) {
    let log = Rc::new(RefCell::new(Log::default()));
    let result = accept(&LINUX, *received, adopter(&log));
    let (handed, error) = match result {
        Ok(held) => (Some(held.0), None),
        Err(error) => (None, Some(error)),
    };
    let mut log = Rc::try_unwrap(log).ok().unwrap().into_inner();
    // The order in which they are dropped does not matter.
    log.dropped.sort_unstable();
    (handed, error, log)
}

#[test]
fn accept_hands_out_the_one_descriptor_of_a_valid_message() {
    let (handed, error, log) = run(&ONE);
    assert_eq!(handed, Some(7));
    assert!(error.is_none(), "{error:?}");
    assert_eq!(log.adopted, [7]);
}

#[test]
fn accept_adopts_and_closes_both_of_two_descriptors() {
    let two = Received {
        length: 24,
        payload: [7, 8],
        ..ONE
    };
    let (handed, error, log) = run(&two);
    assert_eq!(handed, None);
    assert!(matches!(error, Some(FdPassError::TooMany)), "{error:?}");
    assert_eq!(log.adopted, [7, 8]);
    assert_eq!(log.dropped, [7, 8]);
}

#[test]
fn accept_adopts_and_closes_what_a_truncated_message_delivered() {
    for (length, payload, delivered) in [(20, [7, 0], vec![7]), (24, [7, 8], vec![7, 8])] {
        let truncated = Received {
            flags: 8,
            length,
            payload,
            ..ONE
        };
        let (handed, error, log) = run(&truncated);
        assert_eq!(handed, None);
        assert!(matches!(error, Some(FdPassError::Truncated)), "{error:?}");
        assert_eq!(log.adopted, delivered);
        assert_eq!(log.dropped, delivered);
    }
}

#[test]
fn accept_closes_a_delivered_descriptor_on_a_wrong_data_byte_or_the_end_of_the_stream() {
    for (received, expected) in [
        (Received { byte: 0, ..ONE }, "BadData"),
        (Received { count: 0, ..ONE }, "Closed"),
    ] {
        let (handed, error, log) = run(&received);
        assert_eq!(handed, None);
        assert!(
            format!("{error:?}").starts_with(&format!("Some({expected}")),
            "{error:?}"
        );
        assert_eq!(log.adopted, [7]);
        assert_eq!(log.dropped, [7]);
    }
}

#[test]
fn accept_adopts_no_integer_of_a_header_that_fails_a_check() {
    for (case, received) in [
        ("a wrong level", Received { level: 0, ..ONE }),
        ("a wrong type", Received { kind: 2, ..ONE }),
        ("a short control length", Received { control: 19, ..ONE }),
        (
            "a control length past the buffer",
            Received { control: 25, ..ONE },
        ),
        (
            "a header length past the control length",
            Received {
                control: 20,
                length: 24,
                payload: [7, 8],
                ..ONE
            },
        ),
        (
            "a header length of no descriptor",
            Received { length: 16, ..ONE },
        ),
        (
            "a header length between counts",
            Received { length: 21, ..ONE },
        ),
        ("a header length of three", Received { length: 28, ..ONE }),
    ] {
        let (handed, error, log) = run(&received);
        assert_eq!(handed, None, "{case}");
        assert!(
            matches!(error, Some(FdPassError::BadControl)),
            "{case}: {error:?}"
        );
        assert_eq!(log.adopted, Vec::<i32>::new(), "{case}");
    }
}

/// A receive that delivered the peer's pidfd, 9, with the data byte.
const PIDFD: Received = Received {
    kind: 4,
    payload: [9, 0],
    ..ONE
};

#[test]
fn accept_adopts_and_closes_a_pidfd_and_refuses() {
    let (handed, error, log) = run(&PIDFD);
    assert_eq!(handed, None);
    assert!(matches!(error, Some(FdPassError::PeerPidfd)), "{error:?}");
    assert_eq!(log.adopted, [9]);
    assert_eq!(log.dropped, [9]);
}

#[test]
fn accept_closes_a_pidfd_whatever_else_the_receive_reported() {
    for (case, received, expected) in [
        (
            "the end of the stream",
            Received { count: 0, ..PIDFD },
            "Closed",
        ),
        ("truncation", Received { flags: 8, ..PIDFD }, "Truncated"),
        (
            "a wrong data byte",
            Received { byte: 0, ..PIDFD },
            "PeerPidfd",
        ),
    ] {
        let (handed, error, log) = run(&received);
        assert_eq!(handed, None, "{case}");
        assert!(
            format!("{error:?}").starts_with(&format!("Some({expected}")),
            "{case}: {error:?}"
        );
        assert_eq!(log.adopted, [9], "{case}");
        assert_eq!(log.dropped, [9], "{case}");
    }
}

#[test]
fn accept_adopts_no_integer_of_a_pidfd_header_that_fails_a_check() {
    for (case, received) in [
        ("a wrong level", Received { level: 0, ..PIDFD }),
        (
            "a length of two",
            Received {
                length: 24,
                payload: [9, 8],
                ..PIDFD
            },
        ),
        (
            "a short control length",
            Received {
                control: 19,
                ..PIDFD
            },
        ),
        (
            "a header length past the control length",
            Received {
                control: 20,
                length: 24,
                ..PIDFD
            },
        ),
    ] {
        let (handed, error, log) = run(&received);
        assert_eq!(handed, None, "{case}");
        assert!(
            matches!(error, Some(FdPassError::BadControl)),
            "{case}: {error:?}"
        );
        assert_eq!(log.adopted, Vec::<i32>::new(), "{case}");
    }
}

#[test]
fn accept_never_adopts_a_negative_pidfd() {
    let (handed, error, log) = run(&Received {
        payload: [-1, 0],
        ..PIDFD
    });
    assert_eq!(handed, None);
    assert!(matches!(error, Some(FdPassError::PeerPidfd)), "{error:?}");
    assert_eq!(log.adopted, Vec::<i32>::new());
}

#[test]
fn accept_with_no_control_data_is_no_descriptor() {
    let (handed, error, log) = run(&Received {
        control: 0,
        length: 0,
        level: 0,
        kind: 0,
        payload: [0, 0],
        ..ONE
    });
    assert_eq!(handed, None);
    assert!(
        matches!(error, Some(FdPassError::NoDescriptor)),
        "{error:?}"
    );
    assert_eq!(log.adopted, Vec::<i32>::new());
}

#[test]
fn accept_never_adopts_a_negative_value_and_closes_the_other_one() {
    for (length, payload, adopted) in [(20, [-1, 0], vec![]), (24, [-1, 8], vec![8])] {
        let (handed, error, log) = run(&Received {
            length,
            payload,
            ..ONE
        });
        assert_eq!(handed, None);
        assert!(
            matches!(error, Some(FdPassError::BadDescriptor)),
            "{error:?}"
        );
        assert_eq!(log.adopted, adopted);
        assert_eq!(log.dropped, adopted);
    }
}

#[test]
fn accept_hands_out_only_the_valid_case_and_never_leaks_across_the_sweep() {
    for control in [0, 15, 16, 19, 20, 21, 23, 24, 25, 32] {
        for length in [0, 15, 16, 19, 20, 21, 23, 24, 25, 28] {
            for (level, kind) in [(1, 1), (0, 1), (1, 2), (2, 0), (1, 4), (0, 4)] {
                for (flags, count, byte) in [(0, 1, 1), (8, 1, 1), (0, 0, 1), (0, 1, 0)] {
                    let received = Received {
                        count,
                        flags,
                        control,
                        length,
                        level,
                        kind,
                        payload: [7, 8],
                        byte,
                    };
                    let (handed, error, log) = run(&received);
                    let header = (20..=24).contains(&control)
                        && (length == 20 || length == 24)
                        && length <= control
                        && (level, kind) == (1, 1);
                    let pidfd =
                        (20..=24).contains(&control) && length == 20 && (level, kind) == (1, 4);
                    let delivered = match (header, length) {
                        (false, _) if pidfd => vec![7],
                        (false, _) => vec![],
                        (true, 20) => vec![7],
                        (true, _) => vec![7, 8],
                    };
                    let valid = header && length == 20 && (flags, count, byte) == (0, 1, 1);
                    assert_eq!(log.adopted, delivered, "{received:?}");
                    // Every adopted descriptor is dropped by the time `run`
                    // returns: by `accept` on a refusal, by `run` after the
                    // one it handed out.
                    assert_eq!(log.dropped, delivered, "{received:?}");
                    if valid {
                        assert_eq!(handed, Some(7), "{received:?}");
                    } else {
                        assert!(handed.is_none() && error.is_some(), "{received:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn sent_accepts_exactly_the_one_data_byte() {
    assert!(sent(1).is_ok());
    for count in [0, 2] {
        assert!(matches!(sent(count), Err(FdPassError::ShortSend(found)) if found == count));
    }
}

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
    let result = retried(|| {
        calls += 1;
        Err(io::Error::from_raw_os_error(32))
    });
    assert_eq!((result.unwrap_err().raw_os_error(), calls), (Some(32), 1));
}

#[test]
fn fd_pass_errors_name_what_failed() {
    let send = FdPassError::Send(io::Error::from_raw_os_error(32));
    assert!(
        send.to_string()
            .starts_with("sending the descriptor failed: "),
        "{send}"
    );
    assert!(std::error::Error::source(&send).is_some());
    assert_eq!(
        FdPassError::TooMany.to_string(),
        "the message carried more than one descriptor; all were closed"
    );
    assert!(std::error::Error::source(&FdPassError::Closed).is_none());
}
