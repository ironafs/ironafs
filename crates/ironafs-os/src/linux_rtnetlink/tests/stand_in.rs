// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tests of `linux-rtnetlink` (ADR 0005, sheet 23) that only the Miri
//! stand-ins can run: they record each call and return queued replies, so
//! these check each call's arguments and the receive of every reply the
//! sheet names, through the wrapper's own buffers.

use super::super::abi::stand_in::{Call, Plan, Reply, calls, plan, reply, short_sends};
use super::super::abi::{AF_NETLINK, NETLINK_ROUTE, SOCK_CLOEXEC, SOCK_RAW, sockaddr_nl};
use super::super::{GROUPS, RouteSocket};
use crate::route::RouteError;

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
fn a_send_names_the_kernel_and_carries_every_byte() {
    let socket = RouteSocket::request().unwrap();
    let _ = calls();
    let request: Vec<u8> = (0..20).collect();
    socket.send(&request).unwrap();
    assert_eq!(calls(), [Call::Send(request, 0, netlink(0, 0), 12)]);
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
    assert_eq!(socket.receive(&mut buffer).unwrap(), 40);
    assert_eq!(buffer[..40], [7; 40]);
    assert_eq!(buffer[40..], [0; 24]);
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
fn a_receive_passes_no_flags() {
    let socket = RouteSocket::request().unwrap();
    reply(from_kernel(&[1; 4]));
    let _ = calls();
    socket.receive(&mut [0_u8; 8]).unwrap();
    assert_eq!(calls(), [Call::Receive(0)]);
}

#[test]
fn an_interrupted_send_and_receive_are_each_made_again() {
    let socket = RouteSocket::request().unwrap();
    let _ = calls();
    plan(Plan::Interrupt(2));
    socket.send(&[1; 20]).unwrap();
    let sends = calls();
    assert_eq!(sends.len(), 3, "{sends:?}");
    assert_eq!(sends[..2], [Call::Failed, Call::Failed]);
    reply(from_kernel(&[1; 4]));
    plan(Plan::Interrupt(1));
    assert_eq!(socket.receive(&mut [0_u8; 8]).unwrap(), 4);
    assert_eq!(calls(), [Call::Receive(0), Call::Receive(0)]);
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
fn a_failed_send_is_returned_with_its_errno() {
    let socket = RouteSocket::request().unwrap();
    plan(Plan::Fail(111));
    assert!(matches!(
        socket.send(&[1; 20]),
        Err(RouteError::Send(error)) if error.raw_os_error() == Some(111)
    ));
}

#[test]
fn a_receive_with_nothing_to_return_after_an_interruption_ends_with_its_own_errno() {
    let socket = RouteSocket::request().unwrap();
    plan(Plan::Interrupt(1));
    assert!(matches!(
        socket.receive(&mut [0_u8; 8]),
        Err(RouteError::Receive(error)) if error.raw_os_error() == Some(22)
    ));
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
