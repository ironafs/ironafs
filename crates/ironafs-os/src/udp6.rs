// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! An IPv6-only UDP socket (ADR 0005, sheet 21): which addresses the site
//! takes, and its errors, whatever made the calls. These are safe
//! functions over owned values, so the tests reach every branch without a
//! socket.

use std::error::Error;
use std::fmt;
use std::io;
use std::net::Ipv6Addr;

#[cfg(test)]
mod tests;

/// Why no socket was made.
#[derive(Debug)]
pub enum Udp6Error {
    /// The address needs an interface zone (a link-local unicast address,
    /// `fe80::/10`, or an interface- or link-local multicast address),
    /// which this site cannot carry: refused before any call.
    Scoped(Ipv6Addr),
    /// The system offers no IPv6 sockets (`EAFNOSUPPORT`: IPv6 absent at
    /// boot, or a sandbox's address-family filter).
    Unsupported(io::Error),
    /// Making the socket failed for another reason; the error carries its
    /// errno.
    Socket(io::Error),
    /// Setting IPv6-only failed; the socket was closed, never bound.
    IpV6Only(io::Error),
    /// The address and port are in use (`EADDRINUSE`); the socket was
    /// closed.
    InUse(io::Error),
    /// Binding failed for another reason; the socket was closed.
    Bind(io::Error),
}

impl fmt::Display for Udp6Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scoped(address) => write!(
                formatter,
                "{address} needs an interface zone, which an IPv6-only UDP socket here cannot carry"
            ),
            Self::Unsupported(error) => {
                write!(formatter, "the system offers no IPv6 sockets: {error}")
            }
            Self::Socket(error) => write!(formatter, "making the IPv6 socket failed: {error}"),
            Self::IpV6Only(error) => {
                write!(formatter, "setting the socket IPv6-only failed: {error}")
            }
            Self::InUse(error) => write!(formatter, "the address is in use: {error}"),
            Self::Bind(error) => write!(formatter, "binding the IPv6 socket failed: {error}"),
        }
    }
}

impl Error for Udp6Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scoped(_) => None,
            Self::Unsupported(error)
            | Self::Socket(error)
            | Self::IpV6Only(error)
            | Self::InUse(error)
            | Self::Bind(error) => Some(error),
        }
    }
}

/// The C facts the classification compares with, from the site's
/// declarations.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) struct Rules {
    /// `EAFNOSUPPORT`.
    pub(crate) unsupported: i32,
    /// `EADDRINUSE`.
    pub(crate) in_use: i32,
}

/// Whether `address` needs an interface zone: a link-local unicast
/// address (`fe80::/10`), or a multicast address of interface-local (1)
/// or link-local (2) scope (RFC 4291, sections 2.5.6 and 2.7).
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn needs_zone(address: &Ipv6Addr) -> bool {
    let [first, ..] = address.segments();
    let link_local_unicast = first & 0xffc0 == 0xfe80;
    let link_scoped_multicast = address.is_multicast() && matches!(first & 0x000f, 1 | 2);
    link_local_unicast || link_scoped_multicast
}

/// The error of a failed `socket` call.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn socket_error(rules: Rules, error: io::Error) -> Udp6Error {
    if error.raw_os_error() == Some(rules.unsupported) {
        Udp6Error::Unsupported(error)
    } else {
        Udp6Error::Socket(error)
    }
}

/// The error of a failed `bind` call.
#[cfg_attr(
    all(not(target_os = "linux"), not(test)),
    expect(dead_code, reason = "used by the Linux site")
)]
pub(crate) fn bind_error(rules: Rules, error: io::Error) -> Udp6Error {
    if error.raw_os_error() == Some(rules.in_use) {
        Udp6Error::InUse(error)
    } else {
        Udp6Error::Bind(error)
    }
}
