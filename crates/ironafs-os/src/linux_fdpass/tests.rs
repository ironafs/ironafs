// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of `linux-fdpass` (ADR 0005, sheet 14) that Miri runs too,
//! through the calls' stand-ins: a descriptor sent over one pair arrives
//! as a new descriptor for the same socket; the end of the stream; and the
//! flags each call passes. The checks of a receive are `fdpass`'s. The
//! tests that need the running kernel are in `kernel`.

use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::UnixStream;

use super::abi::{MSG_CMSG_CLOEXEC, MSG_NOSIGNAL};
use super::{RECEIVE_FLAGS, SEND_FLAGS, receive_fd, send_fd};
use crate::fdpass::FdPassError;

#[cfg(not(miri))]
mod kernel;
#[cfg(miri)]
mod stand_in;

#[test]
fn a_sent_descriptor_arrives_as_a_new_descriptor_for_the_same_socket() {
    let (sender, receiver) = UnixStream::pair().unwrap();
    let (passed, mut far) = UnixStream::pair().unwrap();
    send_fd(&sender, passed.as_fd()).unwrap();
    let arrived = receive_fd(&receiver).unwrap();
    assert_ne!(arrived.as_raw_fd(), passed.as_raw_fd());
    drop(passed);
    let mut arrived = UnixStream::from(arrived);
    arrived.write_all(b"x").unwrap();
    let mut byte = [0_u8; 1];
    far.read_exact(&mut byte).unwrap();
    assert_eq!(&byte, b"x");
}

#[test]
fn a_receive_at_the_end_of_the_stream_is_closed() {
    let (sender, receiver) = UnixStream::pair().unwrap();
    drop(sender);
    assert!(matches!(receive_fd(&receiver), Err(FdPassError::Closed)));
}

#[test]
fn the_send_raises_no_signal_and_the_receive_sets_close_on_exec() {
    assert_eq!(SEND_FLAGS, MSG_NOSIGNAL);
    assert_eq!(RECEIVE_FLAGS, MSG_CMSG_CLOEXEC);
}
