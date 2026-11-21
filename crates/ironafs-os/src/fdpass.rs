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
//! **Open, pending an ADR 0005 amendment: `SCM_PIDFD`.** On Linux 6.5 and
//! later, a receiving Unix socket with `SO_PASSPIDFD` set (on it, or
//! inherited from the listener it was accepted from) gets an `SCM_PIDFD`
//! control message, and the kernel installs that pidfd in this process.
//! When a receive delivers no `SCM_RIGHTS` message and leaves room for it
//! (a data byte sent without a descriptor, or rights the kernel did not
//! deliver; on a datagram socket wrapped as a stream, even with no data),
//! the only message is `SCM_PIDFD`: these checks refuse it as a control
//! message of another type, without adopting its integer, so the pidfd
//! stays open, and repeated refusals can exhaust the descriptor table. With
//! an `SCM_RIGHTS` message the buffer is full, the kernel creates no pidfd
//! and sets `MSG_CTRUNC`, and the delivered descriptor is closed. No site
//! of ADR 0005 sets `SO_PASSPIDFD`, so the exposure is a socket configured
//! outside IronAFS (a socket unit's `PassPIDFD=`, or foreign code in the
//! process). Recognising `SCM_PIDFD`, adopting and closing its descriptor,
//! then refusing, needs a new constant, which ADR 0005, point 7.1, puts to
//! the owner before the code (#57's review record holds the proposal).
//!
//! **Where a received descriptor is an integer.** Sheet 14 keeps it an
//! integer "only between the checked read of the control buffer and its
//! adoption, in one private function". Here that span is the receive
//! (`linux_fdpass::receive_fd`), which reads its control buffer into a
//! [`Received`] and hands it by value to [`accept`], which checks and
//! adopts: two functions, with no fallible step or early return between
//! the read and the adoption. The checks are a safe function over owned
//! values so that the tests, and a fuzz target once the fuzzing engine is
//! decided (ADR 0005, point 6.5), reach every branch without a socket.
//! This departure is recorded for sheet 14's next amendment.

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
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) struct Rules {
    /// `SOL_SOCKET`.
    pub(crate) level: i32,
    /// `SCM_RIGHTS`.
    pub(crate) rights: i32,
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
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
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

/// Checks a receive, adopts every descriptor a valid `SCM_RIGHTS` message
/// delivered with `adopt`, then hands out the one descriptor of a valid
/// message, or drops every adopted one and refuses.
///
/// # Errors
///
/// [`FdPassError::Closed`], [`FdPassError::Truncated`],
/// [`FdPassError::NoDescriptor`], [`FdPassError::BadControl`],
/// [`FdPassError::BadDescriptor`], [`FdPassError::TooMany`] or
/// [`FdPassError::BadData`], in that order of precedence.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
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
    let number = delivered(rules, &received);
    let Received {
        count,
        flags,
        control,
        payload,
        byte,
        ..
    } = received;
    let delivered = number.and_then(|number| payload.get(..number));
    // Adopt first: from here every delivered descriptor is owned, and
    // dropped on any refusal below.
    let mut adopted = [None, None];
    for (slot, &value) in adopted.iter_mut().zip(delivered.unwrap_or_default()) {
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
    let Some(delivered) = delivered else {
        return Err(FdPassError::BadControl);
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

/// How many integers a receive's control message delivered as
/// descriptors: one or two, only if the control length and the header pass
/// every check.
fn delivered(rules: &Rules, received: &Received) -> Option<usize> {
    let control = (rules.one..=rules.capacity).contains(&received.control);
    let length = received.length <= received.control;
    let kind = received.level == rules.level && received.kind == rules.rights;
    let count = if received.length == rules.one {
        1
    } else if received.length == rules.two {
        2
    } else {
        return None;
    };
    (control && length && kind).then_some(count)
}

/// The result of one send of the data byte.
///
/// # Errors
///
/// [`FdPassError::ShortSend`] for any count but 1.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
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
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn retried(mut call: impl FnMut() -> io::Result<usize>) -> io::Result<usize> {
    loop {
        match call() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            result => return result,
        }
    }
}
