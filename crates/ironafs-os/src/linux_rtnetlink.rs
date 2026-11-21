// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Site `linux-rtnetlink` (ADR 0005, row 23 and sheet 23): route-netlink
//! sockets, which `std` cannot make. A notify socket joins the fixed
//! group set (links, the addresses and routes of both families, the policy
//! rules of both families); a request socket joins none. Either sends one
//! request to the kernel and receives one datagram at a time into the
//! caller's buffer.
//!
//! Every socket is close-on-exec and adopted as an `OwnedFd` as soon as
//! `socket` returns, so a failed `bind` closes it. A send always names the
//! kernel (port id 0, no groups), since whether a plain write reaches the
//! kernel is not established for every candidate kernel; a short send is
//! an error. A receive returns a datagram only if
//! [`crate::route::checked`] finds it whole and from the kernel; an
//! overflow (`ENOBUFS`), a truncation and a sender other than the kernel
//! are distinct errors. Both retry `EINTR`.
//!
//! The bytes are not decoded here: the netlink messages go to a pure,
//! fuzzed decoder outside this crate. What to do on each error
//! (invalidate and take a fresh snapshot; ignore a forged datagram) is the
//! caller's: the network-change monitor's (ADR 0005, sheet 23).

mod abi;

use core::ffi::{c_int, c_void};
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

use crate::fdpass::retried;
use crate::route::{Received, RouteError, Rules, checked, receive_error, sent};
use abi::{
    AF_NETLINK, ENOBUFS, MSG_TRUNC, NETLINK_ROUTE, NOTIFY_GROUPS, RTMGRP_IPV4_IFADDR,
    RTMGRP_IPV4_ROUTE, RTMGRP_IPV4_RULE, RTMGRP_IPV6_IFADDR, RTMGRP_IPV6_ROUTE, RTMGRP_LINK,
    RTNLGRP_IPV6_RULE, SOCK_CLOEXEC, SOCK_RAW, SOCKADDR_NL_SIZE, iovec, msghdr, sa_family_t,
    sockaddr, sockaddr_nl, socklen_t,
};

#[cfg(test)]
mod tests;

/// The notify socket's groups: the owner's set (ADR 0005, sheet 23),
/// whose mask the ABI check proves against the kernel's header.
const GROUPS: u32 = NOTIFY_GROUPS;

/// The socket's type: raw (netlink's own), close-on-exec.
const KIND: c_int = SOCK_RAW | SOCK_CLOEXEC;

/// `AF_NETLINK` as an address's family field.
const FAMILY: sa_family_t = 16;

/// `sizeof(struct sockaddr_nl)`, as the calls' length type.
const NAME_LENGTH: socklen_t = SOCKADDR_NL_SIZE;

/// The receive's flags: none (truncation is read from the returned flags).
const RECEIVE_FLAGS: c_int = 0;

/// The send's flags: none.
const SEND_FLAGS: c_int = 0;

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
);

/// The C facts the receive's checks compare with.
const RULES: Rules = Rules {
    family: FAMILY,
    name_length: NAME_LENGTH as usize,
    truncated: MSG_TRUNC,
    overflow: ENOBUFS,
};

/// A route-netlink socket, close-on-exec: a notify socket subscribed to
/// the fixed group set, or a request socket.
#[derive(Debug)]
pub struct RouteSocket {
    socket: OwnedFd,
}

impl RouteSocket {
    /// A socket subscribed to the notifications of links, of the addresses
    /// and routes of both families, and of the policy rules of both
    /// families.
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

    /// Sends `request` (one or more netlink messages) to the kernel.
    ///
    /// # Errors
    ///
    /// [`RouteError::Send`] with the errno, and [`RouteError::ShortSend`]
    /// if the call reports fewer bytes than `request` holds.
    pub fn send(&self, request: &[u8]) -> Result<(), RouteError> {
        let kernel = address(0);
        let count =
            retried(|| send_to(self.socket.as_fd(), request, &kernel)).map_err(RouteError::Send)?;
        sent(count, request.len())
    }

    /// Receives one datagram from the kernel into `buffer`, and returns
    /// its length. Blocks until one arrives.
    ///
    /// After an error the buffer's content is unspecified: never decode
    /// it.
    ///
    /// # Errors
    ///
    /// [`RouteError::Overflow`] if the kernel dropped notifications,
    /// [`RouteError::Truncated`] if the datagram was larger than
    /// `buffer`, [`RouteError::BadSender`] or [`RouteError::NotKernel`] if
    /// it did not come from the kernel, and [`RouteError::Receive`] with
    /// the errno for any other failure.
    pub fn receive(&self, buffer: &mut [u8]) -> Result<usize, RouteError> {
        let capacity = buffer.len();
        // Family 0, not `AF_NETLINK`: an address the call did not write
        // can never pass for the kernel's.
        let mut sender = unwritten();
        let mut data = iovec {
            iov_base: buffer.as_mut_ptr().cast(),
            iov_len: capacity,
        };
        let mut message = msghdr::new((&raw mut sender).cast(), NAME_LENGTH, &raw mut data);
        let count = retried(|| receive_message(self.socket.as_fd(), &mut message))
            .map_err(|error| receive_error(RULES, error))?;
        let received = Received {
            count,
            capacity,
            flags: message.flags(),
            name_length: message.name_length(),
            family: sender.nl_family,
        };
        checked(RULES, &received, || sender.nl_pid)
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

/// A new route-netlink socket, close-on-exec, owned.
fn open() -> io::Result<OwnedFd> {
    let domain: c_int = AF_NETLINK;
    let kind: c_int = KIND;
    let protocol: c_int = NETLINK_ROUTE;
    #[expect(unsafe_code, reason = "linux-rtnetlink")]
    // SAFETY: `socket(2)` takes three integers and reads no memory of
    // ours; any values are answered with a new descriptor or -1 and
    // errno, never undefined behaviour. The call shares no state with
    // other threads. The declaration is in `abi`, proved by the ABI check.
    let descriptor = unsafe { abi::socket(domain, kind, protocol) };
    if descriptor < 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(abi::last_error());
    }
    Ok(adopt(descriptor))
}

/// Takes ownership of the descriptor `socket` returned.
fn adopt(descriptor: c_int) -> OwnedFd {
    #[expect(unsafe_code, reason = "linux-rtnetlink")]
    // SAFETY: `OwnedFd::from_raw_fd` requires an open descriptor that
    // nothing else owns or will close. `open` passes only the non-negative
    // value `socket` just returned: a new descriptor of this process, which
    // no other code has seen, passed once. So the `OwnedFd` is its only
    // owner, and dropping it closes it.
    unsafe {
        OwnedFd::from_raw_fd(descriptor)
    }
}

/// Binds `socket` to `local` (port id 0, so the kernel chooses one, and
/// the groups to join).
fn bind_to(socket: BorrowedFd<'_>, local: &sockaddr_nl) -> io::Result<()> {
    let descriptor: c_int = socket.as_raw_fd();
    let address: *const sockaddr = (&raw const *local).cast();
    let length: socklen_t = NAME_LENGTH;
    #[expect(unsafe_code, reason = "linux-rtnetlink")]
    // SAFETY: `bind(2)` reads `addrlen` bytes at `addr`, here one `struct
    // sockaddr_nl` (`netlink(7)`), and writes nothing of ours. `address`
    // points to `local`, borrowed from the caller's zero-padded local for
    // the call, and `length` is its size, so the call reads only memory it
    // may read, and nothing keeps the pointer afterwards. The cast to
    // `struct sockaddr *` is how every address is passed (`bind(2)`).
    // `descriptor` is borrowed from the caller's `OwnedFd` for the call. A
    // failure is -1 and errno, never undefined behaviour. The call shares
    // no state with other threads. The declaration is in `abi`, proved by
    // the ABI check.
    let status = unsafe { abi::bind(descriptor, address, length) };
    if status != 0 {
        // At once, with no call in between, on the thread that made it.
        return Err(abi::last_error());
    }
    Ok(())
}

/// One `sendto` call of `request` on `socket`, to `kernel`: the bytes it
/// sent.
fn send_to(socket: BorrowedFd<'_>, request: &[u8], kernel: &sockaddr_nl) -> io::Result<usize> {
    let descriptor: c_int = socket.as_raw_fd();
    let buffer: *const c_void = request.as_ptr().cast();
    let length: usize = request.len();
    let flags: c_int = SEND_FLAGS;
    let address: *const sockaddr = (&raw const *kernel).cast();
    let address_length: socklen_t = NAME_LENGTH;
    #[expect(unsafe_code, reason = "linux-rtnetlink")]
    // SAFETY: `sendto(2)` reads `len` bytes at `buf` and `addrlen` bytes
    // at `dest_addr`, and writes nothing of ours. `buffer` and `length`
    // are the caller's slice, borrowed for the call; `address` points to
    // `kernel`, the caller's local `struct sockaddr_nl` naming the kernel
    // (port id 0, no groups), and `address_length` is its size; so the
    // call reads only memory it may read, and nothing keeps either pointer
    // afterwards. `descriptor` is borrowed from the socket's `OwnedFd`
    // for the call. A failure is -1 and errno, never undefined behaviour.
    // The call shares no state with other threads. The declaration is in
    // `abi`, proved by the ABI check.
    let count = unsafe { abi::sendto(descriptor, buffer, length, flags, address, address_length) };
    // At once, with no call in between, on the thread that made it.
    usize::try_from(count).map_err(|_| abi::last_error())
}

/// One `recvmsg` call into `message` on `socket`: the bytes it received.
fn receive_message(socket: BorrowedFd<'_>, message: &mut msghdr) -> io::Result<usize> {
    let descriptor: c_int = socket.as_raw_fd();
    let pointer: *mut msghdr = message;
    let flags: c_int = RECEIVE_FLAGS;
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
    // any pointer afterwards. A failure is -1 and errno, never undefined
    // behaviour. The call shares no state with other threads. The
    // declarations are in `abi`, proved by the ABI check.
    let count = unsafe { abi::recvmsg(descriptor, pointer, flags) };
    // At once, with no call in between, on the thread that made it.
    usize::try_from(count).map_err(|_| abi::last_error())
}
