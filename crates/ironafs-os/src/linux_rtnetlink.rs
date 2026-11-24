// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Site `linux-rtnetlink` (ADR 0005, row 23 and sheet 23): route-netlink
//! sockets, their wakes and their wait, which `std` cannot make. A notify
//! socket joins the fixed group set (links, the addresses and routes of
//! both families, the policy rules of both families, nexthop objects); a
//! request socket joins none. Either sends one request to the kernel and
//! receives one datagram at a time into the caller's buffer, neither
//! waiting (`MSG_DONTWAIT`). A [`Wake`] is one `eventfd`, which any thread
//! may signal; [`RouteSocket::wait`] waits on one route socket and one
//! wake together, for a finite time.
//!
//! Every descriptor is close-on-exec and adopted as an `OwnedFd` as soon
//! as its call returns, so a failed `bind` closes the socket. A send always
//! names the kernel (port id 0, no groups), since whether a plain write
//! reaches the kernel is not established for every candidate kernel; a
//! short send is an error. A receive returns a datagram only if
//! [`crate::route::checked`] finds it whole, not empty and from the
//! kernel; an overflow (`ENOBUFS`), a truncation, an empty datagram and a
//! sender other than the kernel are distinct errors. A send, a receive and
//! a wait that a signal interrupted (`EINTR`) return "interrupted" and are
//! never made again here: the time left is the caller's to recompute from
//! its deadline.
//!
//! The bytes are not decoded here: the netlink messages go to a pure,
//! fuzzed decoder outside this crate. What to do on each outcome
//! (invalidate and take a fresh snapshot; ignore a forged datagram; when
//! to signal and clear a wake) is the caller's: the network-change
//! monitor's (ADR 0005, sheet 23).

mod abi;

use core::ffi::{c_int, c_void};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::time::Duration;

use crate::route::{
    LINUX, LINUX_POLL, PollRules, Receipt, Received, RouteError, Rules, Sent, WAKE_BYTES, Waited,
    checked, judged, milliseconds, receive_failed, send_failed, sent, transferred, wait_failed,
};
use abi::{
    AF_NETLINK, EAGAIN, EFD_CLOEXEC, EFD_NONBLOCK, EINTR, ENOBUFS, MSG_DONTWAIT, MSG_TRUNC,
    NETLINK_ROUTE, NOTIFY_GROUPS, POLLERR, POLLHUP, POLLIN, POLLNVAL, RTMGRP_IPV4_IFADDR,
    RTMGRP_IPV4_ROUTE, RTMGRP_IPV4_RULE, RTMGRP_IPV6_IFADDR, RTMGRP_IPV6_ROUTE, RTMGRP_LINK,
    RTNLGRP_IPV6_RULE, RTNLGRP_NEXTHOP, SOCK_CLOEXEC, SOCK_RAW, SOCKADDR_NL_SIZE, iovec, msghdr,
    nfds_t, pollfd, sa_family_t, sockaddr, sockaddr_nl, socklen_t,
};

#[cfg(test)]
mod tests;

/// The notify socket's groups: the owner's set (ADR 0005, sheet 23),
/// with the nexthop group the network-change monitor's specification
/// proposes (docs/specs/network-generation.md, 5.1, point 3), whose mask
/// the ABI check proves against the kernel's header.
const GROUPS: u32 = NOTIFY_GROUPS;

/// The socket's type: raw (netlink's own), close-on-exec.
const KIND: c_int = SOCK_RAW | SOCK_CLOEXEC;

/// `AF_NETLINK` as an address's family field.
const FAMILY: sa_family_t = 16;

/// `sizeof(struct sockaddr_nl)`, as the calls' length type.
const NAME_LENGTH: socklen_t = SOCKADDR_NL_SIZE;

/// The wake's flags: close-on-exec, and transfers that never wait.
const WAKE_FLAGS: c_int = EFD_CLOEXEC | EFD_NONBLOCK;

/// The wait's entries: the route socket's and its wake's.
const ENTRIES: nfds_t = 2;

const _: () = assert!(
    NAME_LENGTH as usize == size_of::<sockaddr_nl>()
        && FAMILY as c_int == AF_NETLINK
        && GROUPS
            == RTMGRP_LINK
                | RTMGRP_IPV4_IFADDR
                | RTMGRP_IPV4_ROUTE
                | RTMGRP_IPV4_RULE
                | RTMGRP_IPV6_IFADDR
                | RTMGRP_IPV6_ROUTE
                | (1 << (RTNLGRP_IPV6_RULE - 1))
                | (1 << (RTNLGRP_NEXTHOP - 1))
);

/// The C facts the send's and receive's checks compare with.
const RULES: Rules = Rules {
    family: FAMILY,
    name_length: NAME_LENGTH as usize,
    truncated: MSG_TRUNC,
    overflow: ENOBUFS,
    would_block: EAGAIN,
    interrupted: EINTR,
};

/// The `poll` bits the wait's validator knows.
const POLL_RULES: PollRules = PollRules {
    readable: POLLIN,
    error: POLLERR,
    hangup: POLLHUP,
    invalid: POLLNVAL,
};

const _: () = assert!(
    RULES.family == LINUX.family
        && RULES.name_length == LINUX.name_length
        && RULES.truncated == LINUX.truncated
        && RULES.overflow == LINUX.overflow
        && RULES.would_block == LINUX.would_block
        && RULES.interrupted == LINUX.interrupted
        && POLL_RULES.readable == LINUX_POLL.readable
        && POLL_RULES.error == LINUX_POLL.error
        && POLL_RULES.hangup == LINUX_POLL.hangup
        && POLL_RULES.invalid == LINUX_POLL.invalid
);

/// A route-netlink socket, close-on-exec: a notify socket subscribed to
/// the fixed group set, or a request socket.
#[derive(Debug)]
pub struct RouteSocket {
    socket: OwnedFd,
}

impl RouteSocket {
    /// A socket subscribed to the notifications of links, of the addresses
    /// and routes of both families, of the policy rules of both families,
    /// and of nexthop objects.
    ///
    /// # Errors
    ///
    /// [`RouteError::Open`] or [`RouteError::Bind`], with the errno; no
    /// descriptor is left open.
    pub fn notify() -> Result<Self, RouteError> {
        Self::bound(GROUPS)
    }

    /// A socket that joins no group, for requests and their answers.
    ///
    /// # Errors
    ///
    /// As [`RouteSocket::notify`].
    ///
    /// ```
    /// let socket = ironafs_os::RouteSocket::request()?;
    /// # drop(socket);
    /// # Ok::<(), ironafs_os::RouteError>(())
    /// ```
    pub fn request() -> Result<Self, RouteError> {
        Self::bound(0)
    }

    fn bound(groups: u32) -> Result<Self, RouteError> {
        let socket = open().map_err(RouteError::Open)?;
        let local = address(groups);
        bind_to(socket.as_fd(), &local).map_err(RouteError::Bind)?;
        Ok(Self { socket })
    }

    /// Sends `request` (one or more netlink messages) to the kernel,
    /// without waiting for buffer space.
    ///
    /// # Errors
    ///
    /// [`RouteError::WouldBlock`] if the send would have waited,
    /// [`RouteError::Send`] with the errno for any other failure, and
    /// [`RouteError::ShortSend`] if the call reports fewer bytes than
    /// `request` holds. An interrupted send is [`Sent::Interrupted`].
    pub fn send(&self, request: &[u8]) -> Result<Sent, RouteError> {
        match send_to(self.socket.as_fd(), request) {
            Ok(count) => sent(count, request.len()),
            Err(error) => send_failed(RULES, error),
        }
    }

    /// Receives one datagram from the kernel into `buffer`, without
    /// waiting for one: its length, [`Receipt::NothingQueued`] or
    /// [`Receipt::Interrupted`].
    ///
    /// After an error the buffer's content is unspecified: never decode
    /// it.
    ///
    /// # Errors
    ///
    /// [`RouteError::Overflow`] if the kernel dropped notifications,
    /// [`RouteError::Truncated`] if the datagram was larger than
    /// `buffer`, [`RouteError::Empty`] for a datagram of 0 bytes,
    /// [`RouteError::BadSender`] or [`RouteError::NotKernel`] if it did
    /// not come from the kernel, and [`RouteError::Receive`] with the
    /// errno for any other failure.
    pub fn receive(&self, buffer: &mut [u8]) -> Result<Receipt, RouteError> {
        let capacity = buffer.len();
        // Family 0, not `AF_NETLINK`: an address the call did not write
        // can never pass for the kernel's.
        let mut sender = unwritten();
        let mut data = iovec {
            iov_base: buffer.as_mut_ptr().cast(),
            iov_len: capacity,
        };
        let mut message = msghdr::new((&raw mut sender).cast(), NAME_LENGTH, &raw mut data);
        let count = match receive_message(self.socket.as_fd(), &mut message) {
            Ok(count) => count,
            Err(error) => return receive_failed(RULES, error),
        };
        let received = Received {
            count,
            capacity,
            flags: message.flags(),
            name_length: message.name_length(),
            family: sender.nl_family,
        };
        checked(RULES, &received, || sender.nl_pid).map(Receipt::Datagram)
    }

    /// Waits until this socket or `wake` is reported, for at most
    /// `timeout`, rounded up to whole milliseconds (clamped at
    /// `i32::MAX`), subject to the kernel's rounding and scheduling delay.
    /// Clears nothing: the caller clears the wake.
    ///
    /// # Errors
    ///
    /// [`RouteError::Wait`] with the errno if the wait failed, and
    /// [`RouteError::Readiness`] if it reported what sheet 23 does not
    /// allow. An interrupted wait is [`Waited::Interrupted`].
    pub fn wait(&self, wake: &Wake, timeout: Duration) -> Result<Waited, RouteError> {
        let mut entries = [asked(self.socket.as_fd()), asked(wake.descriptor.as_fd())];
        let count = match poll_on(&mut entries, milliseconds(timeout)) {
            Ok(0) => return Ok(Waited::TimedOut),
            Ok(count) => count,
            Err(error) => return wait_failed(RULES, error),
        };
        let [socket, woken] = entries;
        judged(POLL_RULES, count, socket.revents, woken.revents).map(Waited::Ready)
    }
}

/// A wake: one `eventfd`, close-on-exec and non-blocking, held as a
/// `File`. Any thread may signal it; a signal made before a wait starts is
/// kept by the counter, so it is never lost.
#[derive(Debug)]
pub struct Wake {
    descriptor: File,
}

impl Wake {
    /// A wake, not signalled.
    ///
    /// # Errors
    ///
    /// [`RouteError::WakeOpen`] with the errno.
    pub fn new() -> Result<Self, RouteError> {
        let descriptor = make_wake().map_err(RouteError::WakeOpen)?;
        Ok(Self {
            descriptor: File::from(descriptor),
        })
    }

    /// Signals the wake: writes the value 1, in native byte order. A
    /// counter at its maximum is already signalled, which is success.
    ///
    /// # Errors
    ///
    /// [`RouteError::WakeSignal`] or [`RouteError::WakeTransfer`].
    pub fn signal(&self) -> Result<(), RouteError> {
        let value = 1_u64.to_ne_bytes();
        transferred(RULES, RouteError::WakeSignal, || {
            (&self.descriptor).write(&value)
        })
        .map(|_| ())
    }

    /// Clears the wake: reads its counter, and says whether it was
    /// signalled.
    ///
    /// # Errors
    ///
    /// [`RouteError::WakeClear`] or [`RouteError::WakeTransfer`].
    pub fn clear(&self) -> Result<bool, RouteError> {
        let mut value = [0_u8; WAKE_BYTES];
        transferred(RULES, RouteError::WakeClear, || {
            (&self.descriptor).read(&mut value)
        })
    }
}

/// A netlink address: the family, port id 0 and `groups`, every other
/// field zero. Bound with port id 0, it lets the kernel choose the
/// socket's port id; sent to, it is the kernel.
fn address(groups: u32) -> sockaddr_nl {
    sockaddr_nl {
        nl_family: FAMILY,
        nl_pad: 0,
        nl_pid: 0,
        nl_groups: groups,
    }
}

/// The kernel's address, the only one a send names: the family, port id 0
/// and no groups, every other field zero.
const KERNEL: sockaddr_nl = sockaddr_nl {
    nl_family: FAMILY,
    nl_pad: 0,
    nl_pid: 0,
    nl_groups: 0,
};

/// The receive's sender address before the call: every field zero, the
/// family included, so only an address the call wrote can pass the
/// checks.
fn unwritten() -> sockaddr_nl {
    sockaddr_nl {
        nl_family: 0,
        nl_pad: 0,
        nl_pid: 0,
        nl_groups: 0,
    }
}

/// A wait's entry for `descriptor`: zero-initialised, then the descriptor
/// and `POLLIN`, nothing else asked for.
fn asked(descriptor: BorrowedFd<'_>) -> pollfd {
    let mut entry = pollfd {
        fd: 0,
        events: 0,
        revents: 0,
    };
    entry.fd = descriptor.as_raw_fd();
    entry.events = POLLIN;
    entry
}

/// A new route-netlink socket, close-on-exec, owned.
fn open() -> io::Result<OwnedFd> {
    #[expect(unsafe_code, reason = "linux-rtnetlink")]
    // SAFETY: `socket(2)` takes three integers and reads no memory of
    // ours; any values are answered with a new descriptor or -1 and
    // errno, never undefined behaviour. Its arguments are the row's
    // constants: `AF_NETLINK`, `SOCK_RAW | SOCK_CLOEXEC` and
    // `NETLINK_ROUTE`. The call shares no state with other threads. The
    // declaration is in `abi`, proved by the ABI check.
    let descriptor = unsafe { abi::socket(AF_NETLINK, KIND, NETLINK_ROUTE) };
    if descriptor < 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(abi::last_error());
    }
    Ok(adopt(descriptor))
}

/// A new wake's descriptor, close-on-exec, non-blocking, owned.
fn make_wake() -> io::Result<OwnedFd> {
    #[expect(unsafe_code, reason = "linux-rtnetlink")]
    // SAFETY: `eventfd(2)` takes two integers and reads no memory of ours;
    // any values are answered with a new descriptor or -1 and errno, never
    // undefined behaviour. Its arguments are the row's: an initial value of
    // 0, and `EFD_CLOEXEC | EFD_NONBLOCK`. The call shares no state with
    // other threads. The declaration is in `abi`, proved by the ABI check.
    let descriptor = unsafe { abi::eventfd(0, WAKE_FLAGS) };
    if descriptor < 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(abi::last_error());
    }
    Ok(adopt(descriptor))
}

/// Takes ownership of the descriptor `socket` or `eventfd` returned.
fn adopt(descriptor: c_int) -> OwnedFd {
    #[expect(unsafe_code, reason = "linux-rtnetlink")]
    // SAFETY: `OwnedFd::from_raw_fd` requires an open descriptor that
    // nothing else owns or will close. `open` and `make_wake` pass only the
    // non-negative value `socket` or `eventfd` just returned: a new
    // descriptor of this process, which no other code has seen, passed
    // once. So the `OwnedFd` is its only owner, and dropping it closes it.
    unsafe {
        OwnedFd::from_raw_fd(descriptor)
    }
}

/// Binds `socket` to `local` (port id 0, so the kernel chooses one, and
/// the groups to join).
fn bind_to(socket: BorrowedFd<'_>, local: &sockaddr_nl) -> io::Result<()> {
    let descriptor: c_int = socket.as_raw_fd();
    let address: *const sockaddr = (&raw const *local).cast();
    #[expect(unsafe_code, reason = "linux-rtnetlink")]
    // SAFETY: `bind(2)` reads `addrlen` bytes at `addr`, here one `struct
    // sockaddr_nl` (`netlink(7)`), and writes nothing of ours. `address`
    // points to `local`, borrowed from the caller's zero-padded local for
    // the call, and `NAME_LENGTH` is its size, so the call reads only
    // memory it may read, and nothing keeps the pointer afterwards. The
    // cast to `struct sockaddr *` is how every address is passed
    // (`bind(2)`). `descriptor` is borrowed from the caller's `OwnedFd` for
    // the call. A failure is -1 and errno, never undefined behaviour. The
    // call shares no state with other threads. The declaration is in
    // `abi`, proved by the ABI check.
    let status = unsafe { abi::bind(descriptor, address, NAME_LENGTH) };
    if status != 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(abi::last_error());
    }
    Ok(())
}

/// One `sendto` call of `request` on `socket`, to the kernel ([`KERNEL`]),
/// with `MSG_DONTWAIT`: the bytes it sent.
fn send_to(socket: BorrowedFd<'_>, request: &[u8]) -> io::Result<usize> {
    let descriptor: c_int = socket.as_raw_fd();
    let buffer: *const c_void = request.as_ptr().cast();
    let length: usize = request.len();
    let kernel: sockaddr_nl = KERNEL;
    let address: *const sockaddr = (&raw const kernel).cast();
    #[expect(unsafe_code, reason = "linux-rtnetlink")]
    // SAFETY: `sendto(2)` reads `len` bytes at `buf` and `addrlen` bytes
    // at `dest_addr`, and writes nothing of ours. `buffer` and `length`
    // are the caller's slice, borrowed for the call; `address` points to
    // `kernel`, this function's local copy of `KERNEL`, the `struct
    // sockaddr_nl` naming the kernel (port id 0, no groups), and
    // `NAME_LENGTH` is its size; so the call
    // reads only memory it may read, and nothing keeps either pointer
    // afterwards. The flags are `MSG_DONTWAIT` alone, so the call does not
    // wait for buffer space. `descriptor` is borrowed from the socket's
    // `OwnedFd` for the call. A failure is -1 and errno, never undefined
    // behaviour. The call shares no state with other threads. The
    // declaration is in `abi`, proved by the ABI check.
    let count = unsafe {
        abi::sendto(
            descriptor,
            buffer,
            length,
            MSG_DONTWAIT,
            address,
            NAME_LENGTH,
        )
    };
    // At once, with no call in between, on the thread that made it.
    usize::try_from(count).map_err(|_| abi::last_error())
}

/// One `recvmsg` call into `message` on `socket`, with `MSG_DONTWAIT`:
/// the bytes it received.
fn receive_message(socket: BorrowedFd<'_>, message: &mut msghdr) -> io::Result<usize> {
    let descriptor: c_int = socket.as_raw_fd();
    let pointer: *mut msghdr = message;
    #[expect(unsafe_code, reason = "linux-rtnetlink")]
    // SAFETY: `recvmsg(2)` writes at most `msg_namelen` bytes of the
    // sender's address at `msg_name` and the length it wrote back into
    // `msg_namelen`, at most `iov_len` bytes into each of the `msg_iovlen`
    // buffers, no control data (`msg_controllen` is 0), and `msg_flags`.
    // `pointer` points to `message`, exclusively borrowed for the call,
    // which `RouteSocket::receive` built over its own locals and its
    // caller's buffer, live and unused for the whole call: a zeroed
    // `struct sockaddr_nl` of exactly `msg_namelen` bytes, and one `iovec`
    // over the caller's exclusively borrowed slice, of its length. So the
    // kernel writes only memory this call may write, and nothing keeps
    // any pointer afterwards. The flags are `MSG_DONTWAIT` alone, so the
    // call does not wait for a datagram. A failure is -1 and errno, never
    // undefined behaviour. The call shares no state with other threads.
    // The declarations are in `abi`, proved by the ABI check.
    let count = unsafe { abi::recvmsg(descriptor, pointer, MSG_DONTWAIT) };
    // At once, with no call in between, on the thread that made it.
    usize::try_from(count).map_err(|_| abi::last_error())
}

/// One `poll` call on the two `entries`, for `timeout` milliseconds: the
/// count it returned.
fn poll_on(entries: &mut [pollfd; 2], timeout: c_int) -> io::Result<c_int> {
    let pointer: *mut pollfd = entries.as_mut_ptr();
    #[expect(unsafe_code, reason = "linux-rtnetlink")]
    // SAFETY: `poll(2)` reads `nfds` entries at `fds` and writes each
    // entry's `revents`, and nothing else of ours. `pointer` points to
    // `entries`, exclusively borrowed for the call: exactly `ENTRIES` (2)
    // zero-initialised `struct pollfd`, the route socket's and the wake's
    // descriptors with `POLLIN` asked, each descriptor borrowed by
    // `RouteSocket::wait` from its owner for the whole call; so the call
    // reads and writes only memory it may, and nothing keeps the pointer
    // afterwards. `timeout` comes from `route::milliseconds`, which is
    // never negative, so the wait is never infinite. A failure is -1 and
    // errno, never undefined behaviour. The call shares no state with
    // other threads. The declarations are in `abi`, proved by the ABI
    // check.
    let count = unsafe { abi::poll(pointer, ENTRIES, timeout) };
    if count < 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(abi::last_error());
    }
    Ok(count)
}
