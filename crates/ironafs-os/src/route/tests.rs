// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of what a route socket's calls, its wake's transfers and its wait
//! mean (ADR 0005, sheet 23): the sender check of point 6.5 (short
//! addresses, a wrong family, a port other than the kernel's), truncation
//! and the empty datagram, the outcomes and errors of each call, the
//! timeout's rounding, and the validator of what the wait reported (point
//! 6.5: invalid bits, a count that disagrees with the entries, both entries
//! ready).

use std::cell::Cell;
use std::io;
use std::time::Duration;

use super::{
    LINUX, LINUX_POLL, PollRules, Ready, Receipt, Received, RouteError, Rules, Sent, Waited,
    checked, judged, milliseconds, receive_failed, send_failed, sent, transferred, wait_failed,
};

const RULES: Rules = LINUX;

const POLL: PollRules = LINUX_POLL;

/// A whole 100-byte datagram from the kernel, into a 4096-byte buffer.
const KERNEL: Received = Received {
    count: 100,
    capacity: 4096,
    flags: 0,
    name_length: 12,
    family: 16,
};

/// The check of `received` from a sender whose port id is `port`.
fn check(received: &Received, port: u32) -> Result<usize, RouteError> {
    checked(RULES, received, || port)
}

fn errno(code: i32) -> io::Error {
    io::Error::from_raw_os_error(code)
}

#[test]
fn a_whole_datagram_from_the_kernel_is_returned_with_its_length() {
    assert_eq!(check(&KERNEL, 0).unwrap(), 100);
    let one = Received { count: 1, ..KERNEL };
    assert_eq!(check(&one, 0).unwrap(), 1);
    let full = Received {
        count: 4096,
        ..KERNEL
    };
    assert_eq!(check(&full, 0).unwrap(), 4096);
}

#[test]
fn an_empty_datagram_is_an_error_never_a_datagram() {
    let empty = Received { count: 0, ..KERNEL };
    assert!(matches!(check(&empty, 0), Err(RouteError::Empty)));
    // A forged or truncated empty datagram is named as such first.
    assert!(matches!(check(&empty, 7), Err(RouteError::NotKernel(7))));
    let truncated = Received { flags: 32, ..empty };
    assert!(matches!(check(&truncated, 0), Err(RouteError::Truncated)));
}

#[test]
fn an_address_of_another_length_or_family_is_no_sender() {
    for length in [0, 2, 4, 8, 11, 13, 16, 28, 128, usize::MAX] {
        let short = Received {
            name_length: length,
            ..KERNEL
        };
        assert!(
            matches!(check(&short, 0), Err(RouteError::BadSender)),
            "{length}"
        );
    }
    for family in [0, 1, 2, 10, 17, u16::MAX] {
        let wrong = Received { family, ..KERNEL };
        assert!(
            matches!(check(&wrong, 0), Err(RouteError::BadSender)),
            "{family}"
        );
    }
}

#[test]
fn a_port_other_than_the_kernel_s_is_refused_after_the_address_checks() {
    for port in [1, 4242, u32::MAX] {
        assert!(
            matches!(check(&KERNEL, port), Err(RouteError::NotKernel(found)) if found == port),
            "{port}"
        );
    }
    // The port is read only from an address that passed the other two
    // checks: a read before them panics here.
    let refuse_to_read = || -> u32 { panic!("the port was read before the address was checked") };
    for unchecked in [
        Received {
            name_length: 8,
            ..KERNEL
        },
        Received {
            family: 2,
            ..KERNEL
        },
        Received {
            family: 0,
            ..KERNEL
        },
    ] {
        assert!(matches!(
            checked(RULES, &unchecked, refuse_to_read),
            Err(RouteError::BadSender)
        ));
    }
}

#[test]
fn a_truncated_datagram_is_not_returned() {
    let truncated = Received {
        flags: 32,
        ..KERNEL
    };
    assert!(matches!(check(&truncated, 0), Err(RouteError::Truncated)));
    let with_others = Received {
        flags: 32 | 64 | 0x4000_0000,
        ..KERNEL
    };
    assert!(matches!(check(&with_others, 0), Err(RouteError::Truncated)));
    let longer = Received {
        count: 4097,
        ..KERNEL
    };
    assert!(matches!(check(&longer, 0), Err(RouteError::Truncated)));
    // A forged sender is named as such even when its datagram is long.
    assert!(matches!(
        check(&truncated, 7),
        Err(RouteError::NotKernel(7))
    ));
}

#[test]
fn the_sweep_of_every_field_returns_only_the_kernel_s_whole_datagram() {
    for name_length in [0, 11, 12, 13] {
        for family in [0, 16, 17] {
            for port in [0, 1] {
                for flags in [0, 32] {
                    for count in [0, 1, 4096, 4097] {
                        let received = Received {
                            count,
                            capacity: 4096,
                            flags,
                            name_length,
                            family,
                        };
                        let valid = name_length == 12
                            && family == 16
                            && port == 0
                            && flags == 0
                            && count != 0
                            && count <= 4096;
                        assert_eq!(
                            check(&received, port).is_ok(),
                            valid,
                            "{received:?}, port {port}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn a_receive_error_tells_nothing_queued_an_interruption_and_an_overflow_apart() {
    assert!(matches!(
        receive_failed(RULES, errno(11)),
        Ok(Receipt::NothingQueued)
    ));
    assert!(matches!(
        receive_failed(RULES, errno(4)),
        Ok(Receipt::Interrupted)
    ));
    assert!(matches!(
        receive_failed(RULES, errno(105)),
        Err(RouteError::Overflow(error)) if error.raw_os_error() == Some(105)
    ));
    for other in [9, 22, 111] {
        assert!(matches!(
            receive_failed(RULES, errno(other)),
            Err(RouteError::Receive(error)) if error.raw_os_error() == Some(other)
        ));
    }
    assert!(matches!(
        receive_failed(RULES, io::Error::other("no errno")),
        Err(RouteError::Receive(_))
    ));
}

#[test]
fn a_send_error_tells_an_interruption_and_would_block_apart() {
    assert!(matches!(
        send_failed(RULES, errno(4)),
        Ok(Sent::Interrupted)
    ));
    assert!(matches!(
        send_failed(RULES, errno(11)),
        Err(RouteError::WouldBlock)
    ));
    for other in [9, 105, 111] {
        assert!(matches!(
            send_failed(RULES, errno(other)),
            Err(RouteError::Send(error)) if error.raw_os_error() == Some(other)
        ));
    }
}

#[test]
fn a_send_of_fewer_bytes_than_the_request_is_short() {
    assert_eq!(sent(20, 20).unwrap(), Sent::All);
    for count in [0, 19, 21] {
        assert!(matches!(
            sent(count, 20),
            Err(RouteError::ShortSend { sent, length: 20 }) if sent == count
        ));
    }
}

#[test]
fn a_timeout_is_rounded_up_to_whole_milliseconds_and_clamped() {
    let nanos = Duration::from_nanos;
    assert_eq!(milliseconds(Duration::ZERO), 0);
    // A remainder below one millisecond never becomes 0.
    assert_eq!(milliseconds(nanos(1)), 1);
    assert_eq!(milliseconds(nanos(999_999)), 1);
    assert_eq!(milliseconds(Duration::from_millis(1)), 1);
    assert_eq!(milliseconds(nanos(1_000_001)), 2);
    assert_eq!(milliseconds(Duration::from_secs(1)), 1000);
    let largest = Duration::from_millis(u64::try_from(i32::MAX).unwrap());
    assert_eq!(milliseconds(largest), i32::MAX);
    assert_eq!(milliseconds(largest + nanos(1)), i32::MAX);
    assert_eq!(milliseconds(Duration::MAX), i32::MAX);
}

#[test]
fn a_wait_error_tells_an_interruption_apart() {
    assert!(matches!(
        wait_failed(RULES, errno(4)),
        Ok(Waited::Interrupted)
    ));
    for other in [9, 11, 22] {
        assert!(matches!(
            wait_failed(RULES, errno(other)),
            Err(RouteError::Wait(error)) if error.raw_os_error() == Some(other)
        ));
    }
}

#[test]
fn each_report_on_the_route_socket_is_kept_apart_and_the_wake_marks_it_signalled() {
    let ready = |count, socket, wake| judged(POLL, count, socket, wake).unwrap();
    assert_eq!(
        ready(1, 1, 0),
        Ready {
            readable: true,
            ..Ready::default()
        }
    );
    assert_eq!(
        ready(1, 8, 0),
        Ready {
            error: true,
            ..Ready::default()
        }
    );
    assert_eq!(
        ready(1, 16, 0),
        Ready {
            hangup: true,
            ..Ready::default()
        }
    );
    assert_eq!(
        ready(1, 1 | 8 | 16, 0),
        Ready {
            readable: true,
            error: true,
            hangup: true,
            woken: false,
        }
    );
    assert_eq!(
        ready(1, 0, 1),
        Ready {
            woken: true,
            ..Ready::default()
        }
    );
    // Both ready at once: both are reported.
    assert_eq!(
        ready(2, 1, 1),
        Ready {
            readable: true,
            woken: true,
            ..Ready::default()
        }
    );
}

#[test]
fn an_invalid_descriptor_a_wake_error_and_an_unnamed_bit_are_refused() {
    let refused = |count, socket, wake| {
        matches!(
            judged(POLL, count, socket, wake),
            Err(RouteError::Readiness { count: c, socket: s, wake: w })
                if (c, s, w) == (count, socket, wake)
        )
    };
    // POLLNVAL on either entry.
    assert!(refused(1, 32, 0));
    assert!(refused(1, 0, 32));
    assert!(refused(2, 1 | 32, 1));
    // Any bit but POLLIN on the wake: an overflowed counter's POLLERR, a
    // hangup, POLLOUT.
    assert!(refused(1, 0, 8));
    assert!(refused(1, 0, 1 | 8));
    assert!(refused(1, 0, 16));
    assert!(refused(1, 0, 4));
    // A bit outside those named on the route socket: POLLPRI, POLLOUT, the
    // sign bit.
    assert!(refused(1, 2, 0));
    assert!(refused(1, 4, 0));
    assert!(refused(1, i16::MIN, 0));
}

#[test]
fn a_count_that_disagrees_with_the_entries_is_refused() {
    for (count, socket, wake) in [
        (2, 1, 0),
        (2, 0, 1),
        (1, 1, 1),
        (3, 1, 1),
        (0, 0, 0),
        (1, 0, 0),
        (-1, 1, 0),
        (i32::MAX, 1, 1),
    ] {
        assert!(
            matches!(
                judged(POLL, count, socket, wake),
                Err(RouteError::Readiness { .. })
            ),
            "{count} {socket} {wake}"
        );
    }
}

#[test]
fn the_sweep_of_reports_accepts_exactly_what_the_sheet_allows() {
    // Every combination of POLLIN, POLLOUT, POLLERR, POLLHUP and POLLNVAL
    // on each entry, at each count from -1 to 3, against the sheet's rule
    // stated afresh.
    let bits = [1_i16, 4, 8, 16, 32];
    let combination = |mask: usize| -> i16 {
        bits.iter()
            .enumerate()
            .filter(|(at, _)| mask & (1 << at) != 0)
            .fold(0, |sum, (_, bit)| sum | bit)
    };
    for socket_mask in 0..32 {
        for wake_mask in 0..32 {
            let socket = combination(socket_mask);
            let wake = combination(wake_mask);
            for count in -1..=3 {
                let entries = i32::from(socket != 0) + i32::from(wake != 0);
                let allowed =
                    count >= 1 && count == entries && socket & !(1 | 8 | 16) == 0 && wake & !1 == 0;
                let judged = judged(POLL, count, socket, wake);
                assert_eq!(judged.is_ok(), allowed, "{count} {socket} {wake}");
                if let Ok(ready) = judged {
                    assert_eq!(ready.readable, socket & 1 != 0);
                    assert_eq!(ready.error, socket & 8 != 0);
                    assert_eq!(ready.hangup, socket & 16 != 0);
                    assert_eq!(ready.woken, wake & 1 != 0);
                }
            }
        }
    }
}

/// A transfer that answers each call with the next of `answers`, and
/// counts its calls.
fn scripted(
    answers: Vec<io::Result<usize>>,
    calls: &Cell<usize>,
) -> impl FnMut() -> io::Result<usize> + '_ {
    let mut answers = answers.into_iter();
    move || {
        calls.set(calls.get() + 1);
        answers
            .next()
            .unwrap_or_else(|| panic!("a transfer was made once too often"))
    }
}

#[test]
fn a_wake_transfer_of_8_bytes_moves_and_would_block_does_not() {
    let calls = Cell::new(0);
    assert!(transferred(RULES, RouteError::WakeSignal, scripted(vec![Ok(8)], &calls)).unwrap());
    assert!(
        !transferred(
            RULES,
            RouteError::WakeSignal,
            scripted(vec![Err(errno(11))], &calls)
        )
        .unwrap()
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn a_wake_transfer_is_made_once_more_after_one_interruption_and_no_more() {
    let calls = Cell::new(0);
    assert!(
        transferred(
            RULES,
            RouteError::WakeClear,
            scripted(vec![Err(errno(4)), Ok(8)], &calls)
        )
        .unwrap()
    );
    assert_eq!(calls.get(), 2);
    let calls = Cell::new(0);
    assert!(matches!(
        transferred(
            RULES,
            RouteError::WakeClear,
            scripted(vec![Err(errno(4)), Err(errno(4))], &calls)
        ),
        Err(RouteError::WakeClear(error)) if error.raw_os_error() == Some(4)
    ));
    assert_eq!(calls.get(), 2);
}

#[test]
fn a_wake_transfer_of_other_than_8_bytes_or_another_error_fails() {
    let calls = Cell::new(0);
    for bytes in [0, 1, 4, 7, 9, 16] {
        assert!(matches!(
            transferred(RULES, RouteError::WakeSignal, scripted(vec![Ok(bytes)], &calls)),
            Err(RouteError::WakeTransfer(moved)) if moved == bytes
        ));
    }
    assert!(matches!(
        transferred(
            RULES,
            RouteError::WakeSignal,
            scripted(vec![Err(errno(9))], &calls)
        ),
        Err(RouteError::WakeSignal(error)) if error.raw_os_error() == Some(9)
    ));
    assert!(matches!(
        transferred(
            RULES,
            RouteError::WakeClear,
            scripted(vec![Err(errno(22))], &calls)
        ),
        Err(RouteError::WakeClear(error)) if error.raw_os_error() == Some(22)
    ));
}
