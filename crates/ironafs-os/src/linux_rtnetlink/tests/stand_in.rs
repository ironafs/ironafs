// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tests of `linux-rtnetlink` (ADR 0005, sheet 23) that only the Miri
//! stand-ins can run: they record each call and return queued replies and
//! wait answers, so these check each call's arguments, the receive of
//! every reply the sheet names, and every wait outcome, through the
//! wrapper's own buffers. The stand-ins return on demand what the kernel
//! cannot be made to (point 6.5): "interrupted" for the send, the receive
//! and the wait; `POLLNVAL` on either entry; `POLLERR` on the wake; a short
//! sender address and a wrong family; an error or a hangup with nothing
//! queued.

use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::time::Duration;

use super::super::abi::stand_in::{
    Answer, Call, Opening, Plan, Reply, answer, calls, fail, peer, plan, reply, short_sends,
};
use super::super::abi::{
    AF_NETLINK, EFD_CLOEXEC, EFD_NONBLOCK, MSG_DONTWAIT, NETLINK_ROUTE, POLLERR, POLLHUP, POLLIN,
    POLLNVAL, SOCK_CLOEXEC, SOCK_RAW, sockaddr_nl,
};
use super::super::{GROUPS, RouteSocket, Wake};
use crate::route::{Ready, Receipt, RouteError, Sent, Waited};

/// The kernel's address, or one with `groups`.
fn netlink(port: u32, groups: u32) -> sockaddr_nl {
    sockaddr_nl {
        nl_family: 16,
        nl_pad: 0,
        nl_pid: port,
        nl_groups: groups,
    }
}

/// A reply of `bytes` from the kernel.
fn from_kernel(bytes: &[u8]) -> Reply {
    Reply {
        bytes: bytes.to_vec(),
        sender: netlink(0, 0),
        name_length: 12,
        untouched: false,
    }
}

/// A request socket and a wake, with their calls forgotten.
fn pair() -> (RouteSocket, Wake) {
    let socket = RouteSocket::request().unwrap();
    let wake = Wake::new().unwrap();
    let _ = calls();
    (socket, wake)
}

#[test]
fn a_notify_socket_binds_with_the_groups_and_a_request_socket_with_none() {
    let _ = calls();
    let notify = RouteSocket::notify().unwrap();
    let request = RouteSocket::request().unwrap();
    let socket = Call::Socket(AF_NETLINK, SOCK_RAW | SOCK_CLOEXEC, NETLINK_ROUTE);
    assert_eq!(
        calls(),
        [
            socket.clone(),
            Call::Bind(netlink(0, GROUPS), 12),
            socket,
            Call::Bind(netlink(0, 0), 12),
        ]
    );
    drop((notify, request));
}

#[test]
fn a_wake_is_an_eventfd_of_0_close_on_exec_and_non_blocking() {
    let _ = calls();
    let wake = Wake::new().unwrap();
    assert_eq!(calls(), [Call::Eventfd(0, EFD_CLOEXEC | EFD_NONBLOCK)]);
    drop(wake);
}

#[test]
fn a_send_names_the_kernel_carries_every_byte_and_does_not_wait() {
    let socket = RouteSocket::request().unwrap();
    let _ = calls();
    let request: Vec<u8> = (0..20).collect();
    assert_eq!(socket.send(&request).unwrap(), Sent::All);
    assert_eq!(
        calls(),
        [Call::Send(request, MSG_DONTWAIT, netlink(0, 0), 12)]
    );
}

#[test]
fn a_short_send_is_an_error() {
    let socket = RouteSocket::request().unwrap();
    short_sends();
    assert!(matches!(
        socket.send(&[1; 20]),
        Err(RouteError::ShortSend {
            sent: 19,
            length: 20
        })
    ));
}

#[test]
fn a_whole_reply_from_the_kernel_is_returned_into_the_buffer() {
    let socket = RouteSocket::request().unwrap();
    reply(from_kernel(&[7; 40]));
    let mut buffer = [0_u8; 64];
    assert_eq!(socket.receive(&mut buffer).unwrap(), Receipt::Datagram(40));
    assert_eq!(buffer[..40], [7; 40]);
    assert_eq!(buffer[40..], [0; 24]);
}

#[test]
fn an_empty_reply_is_an_error() {
    let socket = RouteSocket::request().unwrap();
    reply(from_kernel(&[]));
    assert!(matches!(
        socket.receive(&mut [0_u8; 8]),
        Err(RouteError::Empty)
    ));
}

#[test]
fn a_short_sender_address_and_a_wrong_family_are_no_sender() {
    let socket = RouteSocket::notify().unwrap();
    let mut short = from_kernel(&[1; 8]);
    short.name_length = 8;
    reply(short);
    let mut wrong = from_kernel(&[1; 8]);
    wrong.sender.nl_family = 2;
    reply(wrong);
    let mut buffer = [0_u8; 64];
    for _ in 0..2 {
        assert!(matches!(
            socket.receive(&mut buffer),
            Err(RouteError::BadSender)
        ));
    }
}

#[test]
fn a_reply_from_another_port_is_refused() {
    let socket = RouteSocket::notify().unwrap();
    let mut forged = from_kernel(&[1; 8]);
    forged.sender = netlink(4242, 1);
    reply(forged);
    let mut buffer = [0_u8; 64];
    assert!(matches!(
        socket.receive(&mut buffer),
        Err(RouteError::NotKernel(4242))
    ));
}

#[test]
fn a_reply_longer_than_the_buffer_is_truncated() {
    let socket = RouteSocket::request().unwrap();
    reply(from_kernel(&[1; 100]));
    let mut buffer = [0_u8; 16];
    assert!(matches!(
        socket.receive(&mut buffer),
        Err(RouteError::Truncated)
    ));
    assert_eq!(buffer, [1; 16]);
}

#[test]
fn a_receive_does_not_wait() {
    let socket = RouteSocket::request().unwrap();
    reply(from_kernel(&[1; 4]));
    let _ = calls();
    socket.receive(&mut [0_u8; 8]).unwrap();
    assert_eq!(calls(), [Call::Receive(MSG_DONTWAIT), Call::Named(12)]);
}

#[test]
fn a_receive_with_nothing_queued_returns_nothing_queued() {
    let socket = RouteSocket::request().unwrap();
    let _ = calls();
    assert_eq!(
        socket.receive(&mut [0_u8; 8]).unwrap(),
        Receipt::NothingQueued
    );
    assert_eq!(calls(), [Call::Receive(MSG_DONTWAIT)]);
}

#[test]
fn an_interrupted_send_and_receive_are_returned_and_never_made_again() {
    let socket = RouteSocket::request().unwrap();
    let _ = calls();
    plan(Plan::Interrupt(1));
    assert_eq!(socket.send(&[1; 20]).unwrap(), Sent::Interrupted);
    assert_eq!(calls(), [Call::Failed]);
    reply(from_kernel(&[1; 4]));
    plan(Plan::Interrupt(1));
    assert_eq!(
        socket.receive(&mut [0_u8; 8]).unwrap(),
        Receipt::Interrupted
    );
    assert_eq!(calls(), [Call::Receive(MSG_DONTWAIT)]);
    // The reply is still there for the caller's next receive.
    assert_eq!(
        socket.receive(&mut [0_u8; 8]).unwrap(),
        Receipt::Datagram(4)
    );
}

#[test]
fn an_overflow_and_another_failure_are_told_apart_through_the_receive() {
    let socket = RouteSocket::notify().unwrap();
    plan(Plan::Fail(105));
    assert!(matches!(
        socket.receive(&mut [0_u8; 8]),
        Err(RouteError::Overflow(error)) if error.raw_os_error() == Some(105)
    ));
    plan(Plan::Fail(9));
    assert!(matches!(
        socket.receive(&mut [0_u8; 8]),
        Err(RouteError::Receive(error)) if error.raw_os_error() == Some(9)
    ));
}

#[test]
fn a_send_that_would_block_and_a_failed_send_are_told_apart() {
    let socket = RouteSocket::request().unwrap();
    plan(Plan::Fail(11));
    assert!(matches!(socket.send(&[1; 20]), Err(RouteError::WouldBlock)));
    plan(Plan::Fail(111));
    assert!(matches!(
        socket.send(&[1; 20]),
        Err(RouteError::Send(error)) if error.raw_os_error() == Some(111)
    ));
}

#[test]
fn a_sender_address_is_written_only_to_the_length_the_reply_says_and_the_buffer_holds() {
    // As the kernel does: the address is written up to the shorter of the
    // reply's address length and the caller's buffer, and the reply's
    // length is reported whole; either length other than the struct's is
    // no sender.
    let socket = RouteSocket::notify().unwrap();
    for (length, written) in [(4, 4), (0, 0), (16, 12)] {
        let mut reply_of = from_kernel(&[1; 8]);
        reply_of.name_length = length;
        reply(reply_of);
        let _ = calls();
        assert!(matches!(
            socket.receive(&mut [0_u8; 16]),
            Err(RouteError::BadSender)
        ));
        assert_eq!(calls(), [Call::Receive(MSG_DONTWAIT), Call::Named(written)]);
    }
}

#[test]
fn a_receive_that_writes_no_sender_is_no_sender() {
    // A call that reports a datagram but leaves the address buffer and its
    // length as the wrapper set them: the wrapper's own initial address
    // must not pass for the kernel's.
    let socket = RouteSocket::notify().unwrap();
    let mut untouched = from_kernel(&[1; 8]);
    untouched.untouched = true;
    reply(untouched);
    assert!(matches!(
        socket.receive(&mut [0_u8; 16]),
        Err(RouteError::BadSender)
    ));
}

#[test]
fn a_wait_asks_pollin_on_exactly_the_two_entries_with_the_timeout_rounded_up() {
    let (socket, wake) = pair();
    // The route socket's descriptor first, the wake's second.
    let entries = vec![
        (socket.socket.as_raw_fd(), POLLIN, 0),
        (wake.descriptor.as_raw_fd(), POLLIN, 0),
    ];
    assert_ne!(entries[0].0, entries[1].0);
    for (timeout, milliseconds) in [
        (Duration::ZERO, 0),
        (Duration::from_nanos(1), 1),
        (Duration::from_micros(1500), 2),
        (Duration::from_secs(1), 1000),
        (Duration::MAX, i32::MAX),
    ] {
        answer(Answer::Report(0, 0, 0));
        assert_eq!(socket.wait(&wake, timeout).unwrap(), Waited::TimedOut);
        assert_eq!(calls(), [Call::Poll(entries.clone(), 2, milliseconds)]);
    }
}

/// Whether the descriptor whose other end is `peer` is closed: a read on
/// the other end then finds the end of the stream at once.
fn closed(mut peer: std::os::unix::net::UnixStream) -> bool {
    peer.set_nonblocking(true).unwrap();
    match peer.read(&mut [0_u8; 1]) {
        Ok(0) => true,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_failed_socket_or_eventfd_is_returned_with_its_errno_and_adopts_nothing() {
    // A -1 adopted would be undefined behaviour, which Miri reports.
    let _ = calls();
    fail(Opening::Socket, 24);
    assert!(matches!(
        RouteSocket::request(),
        Err(RouteError::Open(error)) if error.raw_os_error() == Some(24)
    ));
    assert_eq!(calls().len(), 1, "no bind after a failed socket");
    fail(Opening::Eventfd, 23);
    assert!(matches!(
        Wake::new(),
        Err(RouteError::WakeOpen(error)) if error.raw_os_error() == Some(23)
    ));
}

#[test]
fn a_failed_bind_is_returned_with_its_errno_and_closes_the_socket() {
    let _ = calls();
    fail(Opening::Bind, 98);
    assert!(matches!(
        RouteSocket::notify(),
        Err(RouteError::Bind(error)) if error.raw_os_error() == Some(98)
    ));
    assert!(closed(peer().unwrap()), "the socket was left open");
    // A socket that is kept is not closed: the check can tell them apart.
    let kept = RouteSocket::request().unwrap();
    assert!(!closed(peer().unwrap()));
    drop(kept);
}

#[test]
fn an_interrupted_wait_is_returned_and_another_failure_is_an_error() {
    let (socket, wake) = pair();
    answer(Answer::Fail(4));
    assert_eq!(
        socket.wait(&wake, Duration::from_secs(1)).unwrap(),
        Waited::Interrupted
    );
    assert_eq!(calls().len(), 1, "made once, never again");
    answer(Answer::Fail(22));
    assert!(matches!(
        socket.wait(&wake, Duration::from_secs(1)),
        Err(RouteError::Wait(error)) if error.raw_os_error() == Some(22)
    ));
}

#[test]
fn pollnval_on_either_entry_and_pollerr_on_the_wake_are_errors() {
    let (socket, wake) = pair();
    for (count, on_socket, on_wake) in [
        (1, POLLNVAL, 0),
        (1, 0, POLLNVAL),
        (1, 0, POLLERR),
        (1, 0, POLLIN | POLLERR),
        (2, POLLIN, POLLNVAL),
    ] {
        answer(Answer::Report(count, on_socket, on_wake));
        assert!(
            matches!(
                socket.wait(&wake, Duration::from_secs(1)),
                Err(RouteError::Readiness { count: c, socket: s, wake: w })
                    if (c, s, w) == (count, on_socket, on_wake)
            ),
            "{count} {on_socket} {on_wake}"
        );
    }
}

#[test]
fn a_count_that_disagrees_with_the_entries_is_an_error() {
    let (socket, wake) = pair();
    for (count, on_socket, on_wake) in [(2, POLLIN, 0), (1, POLLIN, POLLIN), (1, 0, 0)] {
        answer(Answer::Report(count, on_socket, on_wake));
        assert!(matches!(
            socket.wait(&wake, Duration::from_secs(1)),
            Err(RouteError::Readiness { .. })
        ));
    }
}

#[test]
fn both_entries_ready_are_both_reported() {
    let (socket, wake) = pair();
    answer(Answer::Report(2, POLLIN | POLLERR, POLLIN));
    assert_eq!(
        socket.wait(&wake, Duration::from_secs(1)).unwrap(),
        Waited::Ready(Ready {
            readable: true,
            error: true,
            hangup: false,
            woken: true,
        })
    );
}

#[test]
fn an_error_or_a_hangup_with_nothing_queued_reports_the_socket_and_receives_nothing() {
    let (socket, wake) = pair();
    for (reported, ready) in [
        (
            POLLERR,
            Ready {
                error: true,
                ..Ready::default()
            },
        ),
        (
            POLLHUP,
            Ready {
                hangup: true,
                ..Ready::default()
            },
        ),
    ] {
        answer(Answer::Report(1, reported, 0));
        assert_eq!(
            socket.wait(&wake, Duration::from_secs(1)).unwrap(),
            Waited::Ready(ready)
        );
        assert_eq!(
            socket.receive(&mut [0_u8; 8]).unwrap(),
            Receipt::NothingQueued
        );
    }
}
