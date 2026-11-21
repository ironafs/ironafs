// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Route-notification sockets (ADR 0005, sheet 23): the checks of what a
//! receive returned, and the errors, whatever made the calls. These are
//! safe functions over owned values, so the tests (and, once the fuzzing
//! engine is decided, the sender check's fuzz target, point 6.5) reach
//! every branch without a socket.
//!
//! **Untrusted.** A datagram is returned only if its sender is the
//! kernel, as the receive reported it: an address of exactly
//! `struct sockaddr_nl`'s size, of family `AF_NETLINK`, and port id 0,
//! checked in that order, the port read only after the other two; and only
//! if it is whole: no `MSG_TRUNC`, and a length within the buffer. The
//! bytes themselves are not decoded here: the netlink messages go to a
//! pure, fuzzed decoder outside this crate (Decision 1, point 3).

use std::error::Error;
use std::fmt;
use std::io;

#[cfg(test)]
mod tests;

/// Why a route socket was not made, or a datagram not sent or returned.
#[derive(Debug)]
pub enum RouteError {
    /// Making the socket failed; the error carries its errno.
    Open(io::Error),
    /// Binding it (and, on a notify socket, joining its groups) failed;
    /// the socket was closed.
    Bind(io::Error),
    /// Sending failed; the error carries its errno.
    Send(io::Error),
    /// The send reported fewer bytes than the request held.
    ShortSend {
        /// The bytes the call reported.
        sent: usize,
        /// The request's length.
        length: usize,
    },
    /// Receiving failed; the error carries its errno.
    Receive(io::Error),
    /// The kernel dropped notifications for lack of room (`ENOBUFS`):
    /// what the caller knows is stale.
    Overflow(io::Error),
    /// The datagram was larger than the buffer (`MSG_TRUNC`): its tail was
    /// discarded, so it is not returned.
    Truncated,
    /// The receive reported no well-formed netlink sender (an address of
    /// another length or family): the datagram is not returned.
    BadSender,
    /// The datagram came from a netlink port other than the kernel's
    /// (port id 0): it is not returned.
    NotKernel(u32),
}

impl fmt::Display for RouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(error) => write!(formatter, "making the route socket failed: {error}"),
            Self::Bind(error) => write!(formatter, "binding the route socket failed: {error}"),
            Self::Send(error) => write!(formatter, "sending to the kernel failed: {error}"),
            Self::ShortSend { sent, length } => write!(
                formatter,
                "sending to the kernel sent {sent} of {length} bytes"
            ),
            Self::Receive(error) => write!(formatter, "receiving from the kernel failed: {error}"),
            Self::Overflow(error) => {
                write!(formatter, "route notifications were dropped: {error}")
            }
            Self::Truncated => {
                formatter.write_str("a route datagram was larger than the buffer and was dropped")
            }
            Self::BadSender => formatter.write_str("a route datagram had no netlink sender"),
            Self::NotKernel(port) => {
                write!(
                    formatter,
                    "a route datagram came from port {port}, not the kernel"
                )
            }
        }
    }
}

impl Error for RouteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Open(error)
            | Self::Bind(error)
            | Self::Send(error)
            | Self::Receive(error)
            | Self::Overflow(error) => Some(error),
            Self::ShortSend { .. } | Self::Truncated | Self::BadSender | Self::NotKernel(_) => None,
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
    /// `AF_NETLINK`.
    pub(crate) family: u16,
    /// `sizeof(struct sockaddr_nl)`.
    pub(crate) name_length: usize,
    /// `MSG_TRUNC`.
    pub(crate) truncated: i32,
    /// `ENOBUFS`.
    pub(crate) overflow: i32,
}

/// What one receive returned, read out of the receiver's own buffers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) struct Received {
    /// The bytes the call reported.
    pub(crate) count: usize,
    /// The buffer's size.
    pub(crate) capacity: usize,
    /// The message's returned flags.
    pub(crate) flags: i32,
    /// The returned address length.
    pub(crate) name_length: usize,
    /// The sender's family and port id, as the call wrote them.
    pub(crate) family: u16,
    pub(crate) port: u32,
}

/// The length of a datagram the kernel sent whole, or why it is not
/// returned.
///
/// # Errors
///
/// [`RouteError::BadSender`], [`RouteError::NotKernel`] or
/// [`RouteError::Truncated`], in that order of precedence.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn checked(rules: Rules, received: &Received) -> Result<usize, RouteError> {
    if received.name_length != rules.name_length || received.family != rules.family {
        return Err(RouteError::BadSender);
    }
    if received.port != 0 {
        return Err(RouteError::NotKernel(received.port));
    }
    if received.flags & rules.truncated != 0 || received.count > received.capacity {
        return Err(RouteError::Truncated);
    }
    Ok(received.count)
}

/// The error of a failed receive.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn receive_error(rules: Rules, error: io::Error) -> RouteError {
    if error.raw_os_error() == Some(rules.overflow) {
        RouteError::Overflow(error)
    } else {
        RouteError::Receive(error)
    }
}

/// The result of one send of a `length`-byte request.
///
/// # Errors
///
/// [`RouteError::ShortSend`] for any count but `length`.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn sent(count: usize, length: usize) -> Result<(), RouteError> {
    if count == length {
        Ok(())
    } else {
        Err(RouteError::ShortSend {
            sent: count,
            length,
        })
    }
}
