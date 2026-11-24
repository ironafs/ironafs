// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Route-notification sockets, their wakes and their wait (ADR 0005, sheet
//! 23): what each call's outcome means, and the errors, whatever made the
//! calls. These are safe functions over owned values, so the tests and the
//! fuzz targets of point 6.5 (the sender check, and the validator of what
//! the wait reported) reach every branch without a socket.
//!
//! **Untrusted.** A datagram is returned only if its sender is the
//! kernel, as the receive reported it: an address of exactly
//! `struct sockaddr_nl`'s size, of family `AF_NETLINK`, and port id 0,
//! checked in that order, the port read only after the other two; and only
//! if it is whole and not empty: no `MSG_TRUNC`, a length within the
//! buffer, and not 0. The bytes themselves are not decoded here: the
//! netlink messages go to a pure, fuzzed decoder outside this crate
//! (Decision 1, point 3). What the wait reported is accepted only as
//! sheet 23 allows it ([`judged`]).
//!
//! Nothing here retries a call: an interrupted send, receive or wait is
//! returned as "interrupted", so the caller decides again from its own
//! deadline. Only a wake's transfer, which a non-blocking descriptor is
//! not expected to have interrupted, is made once more ([`transferred`]).

use std::error::Error;
use std::fmt;
use std::io;
use std::time::Duration;

#[cfg(test)]
mod tests;

/// Why a route socket or a wake was not made, or a datagram not sent or
/// returned, or a wait not judged.
#[derive(Debug)]
pub enum RouteError {
    /// Making the socket failed; the error carries its errno.
    Open(io::Error),
    /// Binding it (and, on a notify socket, joining its groups) failed;
    /// the socket was closed.
    Bind(io::Error),
    /// Sending failed; the error carries its errno.
    Send(io::Error),
    /// The send would have waited for buffer space (`EAGAIN`): nothing
    /// was sent, and nothing is retried here.
    WouldBlock,
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
    /// The receive reported a datagram of 0 bytes, which is never a
    /// datagram.
    Empty,
    /// The receive reported no well-formed netlink sender (an address of
    /// another length or family): the datagram is not returned.
    BadSender,
    /// The datagram came from a netlink port other than the kernel's
    /// (port id 0): it is not returned.
    NotKernel(u32),
    /// Making the wake failed; the error carries its errno.
    WakeOpen(io::Error),
    /// Signalling the wake failed; the error carries its errno.
    WakeSignal(io::Error),
    /// Clearing the wake failed; the error carries its errno.
    WakeClear(io::Error),
    /// A signal or a clear moved this many bytes, not the counter's 8.
    WakeTransfer(usize),
    /// The wait failed; the error carries its errno.
    Wait(io::Error),
    /// The wait reported what sheet 23 does not allow: `POLLNVAL`, a bit
    /// outside those named, any bit but `POLLIN` on the wake, or a count
    /// that is not the number of entries reported.
    Readiness {
        /// The count the call returned.
        count: i32,
        /// The route socket's `revents`.
        socket: i16,
        /// The wake's `revents`.
        wake: i16,
    },
}

impl fmt::Display for RouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(error) => write!(formatter, "making the route socket failed: {error}"),
            Self::Bind(error) => write!(formatter, "binding the route socket failed: {error}"),
            Self::Send(error) => write!(formatter, "sending to the kernel failed: {error}"),
            Self::WouldBlock => {
                formatter.write_str("sending to the kernel would have waited for buffer space")
            }
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
            Self::Empty => formatter.write_str("a route receive returned an empty datagram"),
            Self::BadSender => formatter.write_str("a route datagram had no netlink sender"),
            Self::NotKernel(port) => {
                write!(
                    formatter,
                    "a route datagram came from port {port}, not the kernel"
                )
            }
            Self::WakeOpen(error) => write!(formatter, "making the wake failed: {error}"),
            Self::WakeSignal(error) => write!(formatter, "signalling the wake failed: {error}"),
            Self::WakeClear(error) => write!(formatter, "clearing the wake failed: {error}"),
            Self::WakeTransfer(bytes) => {
                write!(formatter, "the wake moved {bytes} bytes, not 8")
            }
            Self::Wait(error) => write!(formatter, "waiting on the route socket failed: {error}"),
            Self::Readiness {
                count,
                socket,
                wake,
            } => write!(
                formatter,
                "the wait reported {count} entries, {socket:#x} on the route socket and \
                 {wake:#x} on the wake, which is not allowed"
            ),
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
            | Self::Overflow(error)
            | Self::WakeOpen(error)
            | Self::WakeSignal(error)
            | Self::WakeClear(error)
            | Self::Wait(error) => Some(error),
            Self::WouldBlock
            | Self::ShortSend { .. }
            | Self::Truncated
            | Self::Empty
            | Self::BadSender
            | Self::NotKernel(_)
            | Self::WakeTransfer(_)
            | Self::Readiness { .. } => None,
        }
    }
}

/// What a send did, short of an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sent {
    /// Every byte of the request went to the kernel.
    All,
    /// A signal interrupted the call before it sent anything (`EINTR`):
    /// it is not made again here.
    Interrupted,
}

/// What a receive returned, short of an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Receipt {
    /// A whole datagram from the kernel, of this many bytes, in the
    /// buffer.
    Datagram(usize),
    /// Nothing was queued (`EAGAIN`): a wait's report does not promise a
    /// datagram, and the receive never waits for one.
    NothingQueued,
    /// A signal interrupted the call before it received anything
    /// (`EINTR`): it is not made again here.
    Interrupted,
}

/// What a wait on a route socket and its wake returned, short of an
/// error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waited {
    /// At least one of the two was reported.
    Ready(Ready),
    /// Nothing was reported before the timeout.
    TimedOut,
    /// A signal interrupted the wait (`EINTR`): it is not made again
    /// here, so the caller recomputes the time left from its deadline.
    Interrupted,
}

/// What a wait reported: for the route socket, each of readable, error
/// and hangup as it came (never merged), and whether the wake is
/// signalled. Both may be ready at once. The wait clears nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "four reports the sheet keeps apart, each as it came"
)]
pub struct Ready {
    /// `POLLIN` on the route socket.
    pub readable: bool,
    /// `POLLERR` on the route socket.
    pub error: bool,
    /// `POLLHUP` on the route socket.
    pub hangup: bool,
    /// `POLLIN` on the wake: it is signalled.
    pub woken: bool,
}

impl Ready {
    /// Whether anything was reported on the route socket.
    #[must_use]
    pub fn socket(self) -> bool {
        self.readable || self.error || self.hangup
    }
}

/// The C facts the send's and receive's checks compare with, from the
/// site's declarations.
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
    /// `EAGAIN`.
    pub(crate) would_block: i32,
    /// `EINTR`.
    pub(crate) interrupted: i32,
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
    /// The sender's family, as the call wrote it. Its port id is read
    /// only once the address has passed the other checks.
    pub(crate) family: u16,
}

/// The length of a whole, non-empty datagram the kernel sent, or why it
/// is not returned. `port` reads the sender's port id, and is called only
/// after the address's length and family have passed (ADR 0005, sheet
/// 23).
///
/// # Errors
///
/// [`RouteError::BadSender`], [`RouteError::NotKernel`],
/// [`RouteError::Truncated`] or [`RouteError::Empty`], in that order of
/// precedence.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn checked(
    rules: Rules,
    received: &Received,
    port: impl FnOnce() -> u32,
) -> Result<usize, RouteError> {
    if received.name_length != rules.name_length || received.family != rules.family {
        return Err(RouteError::BadSender);
    }
    let port = port();
    if port != 0 {
        return Err(RouteError::NotKernel(port));
    }
    if received.flags & rules.truncated != 0 || received.count > received.capacity {
        return Err(RouteError::Truncated);
    }
    if received.count == 0 {
        return Err(RouteError::Empty);
    }
    Ok(received.count)
}

/// What a failed receive means: "nothing queued" (`EAGAIN`) and
/// "interrupted" (`EINTR`) are outcomes, `ENOBUFS` the overflow error, and
/// any other errno a failed receive.
///
/// # Errors
///
/// [`RouteError::Overflow`] or [`RouteError::Receive`].
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn receive_failed(rules: Rules, error: io::Error) -> Result<Receipt, RouteError> {
    match error.raw_os_error() {
        Some(code) if code == rules.would_block => Ok(Receipt::NothingQueued),
        Some(code) if code == rules.interrupted => Ok(Receipt::Interrupted),
        Some(code) if code == rules.overflow => Err(RouteError::Overflow(error)),
        _ => Err(RouteError::Receive(error)),
    }
}

/// What a failed send means: "interrupted" (`EINTR`) is an outcome,
/// `EAGAIN` the "would block" error, and any other errno a failed send.
///
/// # Errors
///
/// [`RouteError::WouldBlock`] or [`RouteError::Send`].
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn send_failed(rules: Rules, error: io::Error) -> Result<Sent, RouteError> {
    match error.raw_os_error() {
        Some(code) if code == rules.interrupted => Ok(Sent::Interrupted),
        Some(code) if code == rules.would_block => Err(RouteError::WouldBlock),
        _ => Err(RouteError::Send(error)),
    }
}

/// The result of one send of a `length`-byte request that reported
/// `count` bytes.
///
/// # Errors
///
/// [`RouteError::ShortSend`] for any count but `length`.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn sent(count: usize, length: usize) -> Result<Sent, RouteError> {
    if count == length {
        Ok(Sent::All)
    } else {
        Err(RouteError::ShortSend {
            sent: count,
            length,
        })
    }
}

/// The wait's timeout in whole milliseconds: `timeout` rounded up, so a
/// remainder below one millisecond never becomes 0, and clamped at
/// `i32::MAX`. Never negative, so never the infinite wait.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn milliseconds(timeout: Duration) -> i32 {
    i32::try_from(timeout.as_nanos().div_ceil(1_000_000)).unwrap_or(i32::MAX)
}

/// The `poll` bits the validator knows, from the site's declarations.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) struct PollRules {
    /// `POLLIN`.
    pub(crate) readable: i16,
    /// `POLLERR`.
    pub(crate) error: i16,
    /// `POLLHUP`.
    pub(crate) hangup: i16,
    /// `POLLNVAL`.
    pub(crate) invalid: i16,
}

/// What a wait that returned a positive `count` reported, judged from the
/// two `revents` values copied out of its entries (ADR 0005, sheet 23): on
/// the route socket, `POLLIN`, `POLLERR` and `POLLHUP` each as they came;
/// on the wake, `POLLIN` marks it signalled.
///
/// # Errors
///
/// [`RouteError::Readiness`] for `POLLNVAL` on either, any bit but
/// `POLLIN` on the wake (an overflowed counter's `POLLERR` included), a
/// bit outside those named on the route socket, or a count that does not
/// equal the number of entries with a non-zero `revents` (a count below 1
/// included: such a wait reported nothing).
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn judged(
    rules: PollRules,
    count: i32,
    socket: i16,
    wake: i16,
) -> Result<Ready, RouteError> {
    let refused = RouteError::Readiness {
        count,
        socket,
        wake,
    };
    let reported = i32::from(socket != 0) + i32::from(wake != 0);
    let named = rules.readable | rules.error | rules.hangup;
    if count < 1
        || count != reported
        || (socket | wake) & rules.invalid != 0
        || socket & !named != 0
        || wake & !rules.readable != 0
    {
        return Err(refused);
    }
    Ok(Ready {
        readable: socket & rules.readable != 0,
        error: socket & rules.error != 0,
        hangup: socket & rules.hangup != 0,
        woken: wake & rules.readable != 0,
    })
}

/// What a failed wait means: "interrupted" (`EINTR`) is an outcome, any
/// other errno a failed wait.
///
/// # Errors
///
/// [`RouteError::Wait`].
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn wait_failed(rules: Rules, error: io::Error) -> Result<Waited, RouteError> {
    if error.raw_os_error() == Some(rules.interrupted) {
        Ok(Waited::Interrupted)
    } else {
        Err(RouteError::Wait(error))
    }
}

/// The bytes a wake's signal or clear moves: the counter's 8.
pub(crate) const WAKE_BYTES: usize = 8;

/// One transfer of a wake (a signal's write or a clear's read), made by
/// `transfer`: `true` if it moved 8 bytes, `false` if it would have
/// waited (`EAGAIN`: for a signal, the counter is at its maximum, so it
/// is already signalled; for a clear, it was not signalled). `EINTR` is
/// made once more; `failed` names any other error.
///
/// # Errors
///
/// A second `EINTR` or any other error, through `failed`, and
/// [`RouteError::WakeTransfer`] for a transfer of other than 8 bytes.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn transferred(
    rules: Rules,
    failed: fn(io::Error) -> RouteError,
    mut transfer: impl FnMut() -> io::Result<usize>,
) -> Result<bool, RouteError> {
    let mut outcome = transfer();
    if matches!(&outcome, Err(error) if error.raw_os_error() == Some(rules.interrupted)) {
        outcome = transfer();
    }
    match outcome {
        Ok(WAKE_BYTES) => Ok(true),
        Ok(bytes) => Err(RouteError::WakeTransfer(bytes)),
        Err(error) if error.raw_os_error() == Some(rules.would_block) => Ok(false),
        Err(error) => Err(failed(error)),
    }
}
