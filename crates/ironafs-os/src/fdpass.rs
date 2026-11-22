// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Passing one descriptor over a local stream socket (ADR 0005, sheet 14):
//! the checks of what a receive returned, and the errors, whatever made the
//! calls. These are safe functions over owned values, so the tests (and,
//! once the fuzzing engine is decided, a fuzz target) reach every branch
//! without a socket.
//!
//! **Authenticate the header, then adopt, then decide.** A control message
//! holds descriptors only if the returned control length and the header
//! pass every check: the control length at least one descriptor's
//! `CMSG_LEN` and at most the buffer, the header's length within the
//! control length and exactly one or two descriptors' worth, its level
//! `SOL_SOCKET` and its type `SCM_RIGHTS`. Then **every** non-negative
//! descriptor it delivered is adopted, before anything else is decided, so
//! a delivered descriptor is never leaked: on every refusal the adopted
//! ones are dropped, which closes them. An integer from a message that
//! fails a check is never adopted (it may be another descriptor of this
//! process), and a negative one never is.
//!
//! **A pidfd is the second descriptor-carrying type.** On Linux 6.5 and
//! later, a receiving Unix socket with `SO_PASSPIDFD` set (on it, or
//! inherited from the listener it was accepted from; no site sets it) gets
//! an `SCM_PIDFD` control message whenever a receive delivers no
//! `SCM_RIGHTS` message and leaves room for it, and the kernel installs
//! that pidfd in this process. A header of level `SOL_SOCKET`, type
//! `SCM_PIDFD` and length exactly one descriptor's `CMSG_LEN`, within the
//! control length, holds one descriptor: if non-negative it is adopted,
//! whatever else the receive reported, and the message is then refused
//! ([`FdPassError::PeerPidfd`]), which closes it.
//!
//! **Where a received descriptor is an integer** (ADR 0005, sheet 14):
//! between the checked read of the control buffer and its adoption, that
//! is, the receive (`linux_fdpass::receive_fd`), which reads its control
//! buffer into a [`Received`] and hands it by value to [`accept`], which
//! checks and adopts, with no fallible step or early return between the
//! read and the adoption. The checks are a safe function over owned values
//! so that the tests, and a fuzz target once the fuzzing engine is decided
//! (ADR 0005, point 6.5), reach every branch without a socket.

use std::error::Error;
use std::fmt;
use std::io;

#[cfg(test)]
mod tests;

/// Why a descriptor was not passed.
#[derive(Debug)]
pub enum FdPassError {
    /// Sending failed; the error carries its errno (`EPIPE` for a closed
    /// peer: no signal is raised).
    Send(io::Error),
    /// Receiving failed; the error carries its errno.
    Receive(io::Error),
    /// The send reported this many bytes, not the one data byte.
    ShortSend(usize),
    /// The peer closed the stream: nothing was received.
    Closed,
    /// The message carried no control message, so no descriptor.
    NoDescriptor,
    /// The kernel truncated the control data (`MSG_CTRUNC`): the peer sent
    /// more than fits; what was delivered is closed.
    Truncated,
    /// The control message failed a check (its length, level or type); none
    /// of its integers was adopted.
    BadControl,
    /// The message delivered more than one descriptor; all are closed.
    TooMany,
    /// The message delivered a negative descriptor value.
    BadDescriptor,
    /// The message carried the peer's pidfd (`SCM_PIDFD`), not a
    /// descriptor; it was closed.
    PeerPidfd,
    /// The data byte was not the one the sender writes.
    BadData,
}

impl fmt::Display for FdPassError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Send(error) => write!(formatter, "sending the descriptor failed: {error}"),
            Self::Receive(error) => write!(formatter, "receiving a descriptor failed: {error}"),
            Self::ShortSend(count) => write!(
                formatter,
                "sending the descriptor sent {count} data bytes, not 1"
            ),
            Self::Closed => formatter.write_str("the peer closed the stream before a descriptor"),
            Self::NoDescriptor => formatter.write_str("the message carried no descriptor"),
            Self::Truncated => formatter
                .write_str("the message's control data was truncated; its descriptors were closed"),
            Self::BadControl => formatter.write_str("the message's control data failed a check"),
            Self::TooMany => {
                formatter.write_str("the message carried more than one descriptor; all were closed")
            }
            Self::BadDescriptor => formatter.write_str("the message carried a negative descriptor"),
            Self::PeerPidfd => formatter
                .write_str("the peer's pidfd arrived instead of a descriptor; it was closed"),
            Self::BadData => formatter.write_str("the message's data byte was not the sender's"),
        }
    }
}

impl Error for FdPassError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Send(error) | Self::Receive(error) => Some(error),
            _ => None,
        }
    }
}

/// The C facts the checks compare with, from the site's declarations.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rules {
    /// `SOL_SOCKET`.
    pub(crate) level: i32,
    /// `SCM_RIGHTS`.
    pub(crate) rights: i32,
    /// `SCM_PIDFD`.
    pub(crate) pidfd: i32,
    /// `MSG_CTRUNC`.
    pub(crate) truncated: i32,
    /// `CMSG_LEN` of one descriptor, and of two.
    pub(crate) one: usize,
    pub(crate) two: usize,
    /// The receive buffer's size.
    pub(crate) capacity: usize,
    /// The one data byte the sender writes.
    pub(crate) data: u8,
}

/// What one receive returned, read out of the receiver's own buffers.
///
/// It holds the delivered descriptors as integers, so it is not `Copy` or
/// `Clone` outside the tests: the receive builds it and hands it, by
/// value, straight to [`accept`], which adopts or drops what it holds.
#[derive(Debug, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(Clone, Copy))]
pub(crate) struct Received {
    /// The data bytes received: 0 at the end of the stream.
    pub(crate) count: usize,
    /// The message's returned flags.
    pub(crate) flags: i32,
    /// The returned control length.
    pub(crate) control: usize,
    /// The header's length, level and type.
    pub(crate) length: usize,
    pub(crate) level: i32,
    pub(crate) kind: i32,
    /// The two integers after the header.
    pub(crate) payload: [i32; 2],
    /// The data byte.
    pub(crate) byte: u8,
}

/// Checks a receive, adopts every descriptor a valid `SCM_RIGHTS` or
/// `SCM_PIDFD` message delivered with `adopt`, then hands out the one
/// descriptor of a valid `SCM_RIGHTS` message, or drops every adopted one
/// and refuses.
///
/// # Errors
///
/// [`FdPassError::Closed`], [`FdPassError::Truncated`],
/// [`FdPassError::NoDescriptor`], [`FdPassError::PeerPidfd`],
/// [`FdPassError::BadControl`], [`FdPassError::BadDescriptor`],
/// [`FdPassError::TooMany`] or [`FdPassError::BadData`], in that order of
/// precedence.
#[cfg_attr(
    not(test),
    expect(
        clippy::needless_pass_by_value,
        reason = "the receive hands over what it read, so no copy of the descriptors' integers \
                  stays with it"
    )
)]
pub(crate) fn accept<T>(
    rules: &Rules,
    received: Received,
    mut adopt: impl FnMut(i32) -> T,
) -> Result<T, FdPassError> {
    let message = delivered(rules, &received);
    let Received {
        count,
        flags,
        control,
        payload,
        byte,
        ..
    } = received;
    let number = match message {
        Message::Rights(number) | Message::Pidfd(number) => Some(number),
        Message::Other => None,
    };
    let carried = number.and_then(|number| payload.get(..number));
    // Adopt first: from here every delivered descriptor is owned, and
    // dropped on any refusal below.
    let mut adopted = [None, None];
    for (slot, &value) in adopted.iter_mut().zip(carried.unwrap_or_default()) {
        if value >= 0 {
            *slot = Some(adopt(value));
        }
    }
    if count == 0 {
        return Err(FdPassError::Closed);
    }
    if flags & rules.truncated != 0 {
        return Err(FdPassError::Truncated);
    }
    if control == 0 {
        return Err(FdPassError::NoDescriptor);
    }
    let delivered = match (message, carried) {
        (Message::Pidfd(_), _) => return Err(FdPassError::PeerPidfd),
        (Message::Rights(_), Some(delivered)) => delivered,
        _ => return Err(FdPassError::BadControl),
    };
    if delivered.iter().any(|value| *value < 0) {
        return Err(FdPassError::BadDescriptor);
    }
    if delivered.len() != 1 {
        return Err(FdPassError::TooMany);
    }
    if byte != rules.data {
        return Err(FdPassError::BadData);
    }
    let [Some(descriptor), None] = adopted else {
        return Err(FdPassError::BadDescriptor);
    };
    Ok(descriptor)
}

/// What a receive's control message holds, by its header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Message {
    /// `SCM_RIGHTS`: this many descriptors, one or two.
    Rights(usize),
    /// `SCM_PIDFD`: this many descriptors, always one, the peer's pidfd.
    Pidfd(usize),
    /// No control message, or one that fails a check: no descriptor.
    Other,
}

/// Which descriptors a receive's control message delivered: only if the
/// control length and the header pass every check, one or two of
/// `SCM_RIGHTS`, or one of `SCM_PIDFD`.
fn delivered(rules: &Rules, received: &Received) -> Message {
    let control = (rules.one..=rules.capacity).contains(&received.control);
    let length = received.length <= received.control;
    if !(control && length && received.level == rules.level) {
        return Message::Other;
    }
    if received.kind == rules.rights {
        if received.length == rules.one {
            return Message::Rights(1);
        }
        if received.length == rules.two {
            return Message::Rights(2);
        }
    } else if received.kind == rules.pidfd && received.length == rules.one {
        return Message::Pidfd(1);
    }
    Message::Other
}

/// The result of one send of the data byte.
///
/// # Errors
///
/// [`FdPassError::ShortSend`] for any count but 1.
pub(crate) fn sent(count: usize) -> Result<(), FdPassError> {
    if count == 1 {
        Ok(())
    } else {
        Err(FdPassError::ShortSend(count))
    }
}

/// Makes `call` until it is not interrupted by a signal (`EINTR`).
///
/// # Errors
///
/// The first error that is not an interruption.
pub(crate) fn retried(mut call: impl FnMut() -> io::Result<usize>) -> io::Result<usize> {
    loop {
        match call() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            result => return result,
        }
    }
}
