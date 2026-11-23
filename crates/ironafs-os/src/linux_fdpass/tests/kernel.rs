// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tests of `linux-fdpass` (ADR 0005, sheet 14) that need the running
//! kernel itself, so Miri does not build them. Each case counts this
//! process's descriptors in `/proc/self/fd` before and after: one
//! descriptor; none; two (both closed); three, which the receive buffer
//! truncates to two with `MSG_CTRUNC` (closed, not leaked); a closed peer;
//! the end of the stream; and, through the checks with this site's own
//! adoption, a wrong control type whose payload holds the number of an
//! open descriptor of this process (it stays open), a short control length
//! and a header length past it. Each arrived descriptor is close-on-exec.
//! Where the kernel offers `SO_PASSPIDFD` (Linux 6.5), a receive on a
//! socket a helper process set it on closes the pidfd the kernel installed.

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::process::Stdio;

use super::super::abi::{SCM_RIGHTS, SOL_SOCKET, cmsghdr, iovec, msghdr};
use super::super::{DATA, ONE, RULES, SPACE, adopt, receive_fd, send_fd, send_message};
use crate::dump::harness::Command;
use crate::fdpass::{FdPassError, Received, accept};

/// `O_CLOEXEC` in `/proc/self/fdinfo`'s octal `flags`.
const O_CLOEXEC: u32 = 0o2_000_000;

/// The number of this process's open descriptors.
fn open_descriptors() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

/// Whether `descriptor` is open in this process.
fn is_open(descriptor: i32) -> bool {
    std::fs::symlink_metadata(format!("/proc/self/fd/{descriptor}")).is_ok()
}

/// A control buffer of three descriptors, for a sender of more than the
/// site sends.
#[repr(C)]
struct Three {
    header: cmsghdr,
    descriptors: [i32; 3],
    padding: i32,
}

/// Sends the data byte and `descriptors` (one to three) in one message,
/// through the site's own send call.
fn send_many(stream: &UnixStream, descriptors: &[BorrowedFd<'_>]) {
    let mut byte = DATA;
    let mut buffer = iovec {
        iov_base: (&raw mut byte).cast(),
        iov_len: 1,
    };
    let count = descriptors.len();
    let length = 16 + 4 * count;
    let mut control = Three {
        header: cmsghdr::new(length, SOL_SOCKET, SCM_RIGHTS).unwrap(),
        descriptors: [0; 3],
        padding: 0,
    };
    for (slot, descriptor) in control.descriptors.iter_mut().zip(descriptors) {
        *slot = descriptor.as_raw_fd();
    }
    let space = (length + 7) & !7;
    let message = msghdr::new(&raw mut buffer, (&raw mut control).cast(), space).unwrap();
    assert_eq!(send_message(stream.as_fd(), &message).unwrap(), 1);
}

#[test]
fn one_descriptor_arrives_close_on_exec_and_nothing_else_stays_open() {
    let (sender, receiver) = UnixStream::pair().unwrap();
    let passed = File::open("/proc/self/status").unwrap();
    let before = open_descriptors();
    send_fd(&sender, passed.as_fd()).unwrap();
    let arrived = receive_fd(&receiver).unwrap();
    assert_eq!(open_descriptors(), before + 1);
    let info =
        std::fs::read_to_string(format!("/proc/self/fdinfo/{}", arrived.as_raw_fd())).unwrap();
    let flags = info
        .lines()
        .find_map(|line| line.strip_prefix("flags:"))
        .map(|flags| u32::from_str_radix(flags.trim(), 8).unwrap())
        .unwrap();
    assert_eq!(flags & O_CLOEXEC, O_CLOEXEC, "{info}");
    drop(arrived);
    assert_eq!(open_descriptors(), before);
}

#[test]
fn a_message_with_no_descriptor_is_refused() {
    let (mut sender, receiver) = UnixStream::pair().unwrap();
    sender.write_all(&[DATA]).unwrap();
    let before = open_descriptors();
    assert!(matches!(
        receive_fd(&receiver),
        Err(FdPassError::NoDescriptor)
    ));
    assert_eq!(open_descriptors(), before);
}

#[test]
fn two_descriptors_are_both_closed() {
    let (sender, receiver) = UnixStream::pair().unwrap();
    let first = File::open("/proc/self/status").unwrap();
    let second = File::open("/proc/self/stat").unwrap();
    let before = open_descriptors();
    send_many(&sender, &[first.as_fd(), second.as_fd()]);
    assert!(matches!(receive_fd(&receiver), Err(FdPassError::TooMany)));
    assert_eq!(open_descriptors(), before);
}

#[test]
fn three_descriptors_are_truncated_and_what_was_delivered_is_closed() {
    let (sender, receiver) = UnixStream::pair().unwrap();
    let files: Vec<File> = (0..3)
        .map(|_| File::open("/proc/self/status").unwrap())
        .collect();
    let before = open_descriptors();
    let borrowed: Vec<BorrowedFd<'_>> = files.iter().map(AsFd::as_fd).collect();
    send_many(&sender, &borrowed);
    assert!(matches!(receive_fd(&receiver), Err(FdPassError::Truncated)));
    assert_eq!(open_descriptors(), before);
}

#[test]
fn a_send_to_a_closed_peer_is_an_epipe_error() {
    let (sender, receiver) = UnixStream::pair().unwrap();
    drop(receiver);
    let passed = File::open("/proc/self/status").unwrap();
    let before = open_descriptors();
    match send_fd(&sender, passed.as_fd()) {
        Err(FdPassError::Send(error)) => assert_eq!(error.raw_os_error(), Some(32)),
        other => panic!("{other:?}"),
    }
    assert_eq!(open_descriptors(), before);
}

#[test]
fn the_end_of_the_stream_leaves_nothing_open() {
    let (sender, receiver) = UnixStream::pair().unwrap();
    let before = open_descriptors();
    drop(sender);
    assert!(matches!(receive_fd(&receiver), Err(FdPassError::Closed)));
    assert_eq!(open_descriptors(), before - 1);
}

#[test]
fn a_control_message_that_fails_a_check_closes_no_descriptor_of_this_process() {
    let mut open = File::open("/proc/self/status").unwrap();
    let number = open.as_raw_fd();
    let valid = Received {
        count: 1,
        flags: 0,
        control: SPACE,
        length: ONE,
        level: SOL_SOCKET,
        kind: SCM_RIGHTS,
        payload: [number, 0],
        byte: DATA,
    };
    for (case, arrived) in [
        ("a wrong control type", Received { kind: 2, ..valid }),
        (
            "a short control length",
            Received {
                control: ONE - 1,
                ..valid
            },
        ),
        (
            "a header length past the control length",
            Received {
                control: ONE,
                length: ONE + 4,
                ..valid
            },
        ),
    ] {
        let before = open_descriptors();
        assert!(
            matches!(accept(&RULES, arrived, adopt), Err(FdPassError::BadControl)),
            "{case}"
        );
        assert!(is_open(number), "{case}");
        assert_eq!(open_descriptors(), before, "{case}");
    }
    let mut text = String::new();
    open.read_to_string(&mut text).unwrap();
    assert!(text.contains("Pid:"));
}

/// Sets `SO_PASSPIDFD` on `socket` from a helper process, through a
/// duplicate as its standard input: no site of ADR 0005 sets the option,
/// and the option belongs to the socket, which the duplicate shares.
/// `SO_PASSPIDFD` is 76 and `ENOPROTOOPT` 92 on both allowlisted
/// architectures; the helper exits 3 if the kernel lacks the option.
fn pass_pidfd(socket: &UnixStream) {
    let duplicate = std::os::fd::OwnedFd::from(socket.try_clone().unwrap());
    let status = Command::new("python3")
        .args([
            "-I",
            "-c",
            "import socket, sys\n\
             s = socket.socket(fileno=0)\n\
             try:\n    s.setsockopt(socket.SOL_SOCKET, 76, 1)\n\
             except OSError as error:\n    sys.exit(3 if error.errno == 92 else 1)\n\
             s.detach()\n",
        ])
        .stdin(Stdio::from(duplicate))
        .status()
        .unwrap();
    assert_ne!(
        status.code(),
        Some(3),
        "the kernel lacks SO_PASSPIDFD (Linux 6.5): this case cannot run here"
    );
    assert!(status.success(), "{status:?}");
}

#[test]
#[ignore = "needs Linux 6.5 or later and python3, to set SO_PASSPIDFD: run by name in the \
            os-linux lanes"]
fn a_pidfd_the_kernel_installs_is_closed_and_the_message_refused() {
    let (sender, receiver) = UnixStream::pair().unwrap();
    pass_pidfd(&receiver);
    // A data byte sent with no descriptor: the receive's only control
    // message is the pidfd the kernel installs.
    let before = open_descriptors();
    (&sender).write_all(&[DATA]).unwrap();
    let found = receive_fd(&receiver);
    assert!(matches!(found, Err(FdPassError::PeerPidfd)), "{found:?}");
    assert_eq!(open_descriptors(), before);
    // A descriptor sent: one control message fits, whichever the kernel
    // writes first, and nothing it delivered stays open.
    let passed = File::open("/proc/self/status").unwrap();
    let before = open_descriptors();
    send_fd(&sender, passed.as_fd()).unwrap();
    let found = receive_fd(&receiver);
    assert!(
        matches!(found, Err(FdPassError::PeerPidfd | FdPassError::Truncated)),
        "{found:?}"
    );
    assert_eq!(open_descriptors(), before);
}
